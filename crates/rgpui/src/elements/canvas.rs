//! 画布元素，提供自定义绘图回调，用于绘制任意图形内容。
//!
//! 画布上的内容不属于元素树，屏幕阅读器默认看不到。需要暴露自绘对象时，
//! 用 [`.id(..)`][Canvas::id] + [`.a11y(..)`][Canvas::a11y] 声明
//! [画布语义](crate::canvas_a11y)：每个 [`CanvasNode`](crate::canvas_a11y::CanvasNode)
//! 会作为合成子节点写入
//! 无障碍树（带矩形，可被辅助技术定位 / 点击），语义节点上的动作经
//! [`.on_canvas_a11y_action(..)`][Canvas::on_canvas_a11y_action] 回到应用。

use std::cell::RefCell;
use std::rc::Rc;

use accesskit::{Action, ActionData, Role};

use crate::canvas_a11y::{CanvasActionHandler, CanvasNodeId, CanvasSemantics};
use crate::canvas_a11y::{push_canvas_a11y, register_canvas_a11y_actions};
use crate::refineable::Refineable as _;

use crate::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, Pixels,
    Point, SharedString, Style, StyleRefinement, Styled, Window,
};

/// 内部不变量：值为 `None` 说明前提被破坏（回调被取走后再次取用等），必须立刻暴露。
///
/// 统一走这里而不是散落的 `unwrap`：语义与原先一致（同样 panic），但把「这是不变量
/// 断言」的意图写进调用点，模块级 `deny` 仍能拦住随手写的 unwrap。
#[inline]
#[allow(clippy::panic)] // 不变量断言：前提被破坏必须立刻暴露
fn invariant<T>(value: Option<T>, message: &str) -> T {
    match value {
        Some(value) => value,
        None => panic!("{message}"),
    }
}

/// 使用给定的绘制回调构造画布元素。
/// 适用于向视图添加短期自定义绘制。
pub fn canvas<T>(
    prepaint: impl 'static + FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> T,
    paint: impl 'static + FnOnce(Bounds<Pixels>, T, &mut Window, &mut App),
) -> Canvas<T> {
    Canvas {
        prepaint: Some(Box::new(prepaint)),
        paint: Some(Box::new(paint)),
        #[cfg(feature = "dom-backend")]
        dom_impl: None,
        style: StyleRefinement::default(),
        id: None,
        a11y: None,
        origin: Point::default(),
        bounds: Bounds::default(),
    }
}

/// 画布的无障碍配置：角色 / 名称 / 本帧语义节点 / 动作处理。
///
/// 由 [`.a11y(..)`][Canvas::a11y] 建立；[`.a11y_label(..)`][Canvas::a11y_label]
/// 与 [`.on_canvas_a11y_action(..)`][Canvas::on_canvas_a11y_action] 只补字段。
struct CanvasA11y {
    /// 画布自身的无障碍角色。
    role: Role,
    /// 画布自身的无障碍名称。
    label: Option<SharedString>,
    /// 本帧的语义节点（应用每帧重建）。
    semantics: CanvasSemantics,
    /// 语义节点上的无障碍动作处理。
    on_action: Option<CanvasActionHandler>,
}

/// 画布元素，用于在不定义完整自定义元素的情况下访问底层绘制 API
pub struct Canvas<T> {
    prepaint: Option<Box<dyn FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> T>>,
    paint: Option<Box<dyn FnOnce(Bounds<Pixels>, T, &mut Window, &mut App)>>,
    /// 可选的 DOM 渲染实现（仅 dom-backend feature 生效）：canvas 元素在纯 DOM
    /// 模式下（canvas 隐藏）不再可见，需要自绘组件（图表/波形/SVG 等）通过该闭包
    /// 输出一个等价的 DOM 节点（如数据 URI 的 `<img>`）以便在覆盖层中显示。
    #[cfg(feature = "dom-backend")]
    dom_impl: Option<Box<dyn Fn(Bounds<Pixels>, &mut Window, &mut App) -> Option<crate::DomNode>>>,
    style: StyleRefinement,
    /// 无障碍：元素 id（进入无障碍树、挂合成子节点的前提）。
    id: Option<ElementId>,
    /// 无障碍：角色 / 名称 / 语义节点 / 动作处理。
    a11y: Option<CanvasA11y>,
    /// 最近一次预绘制的画布原点（语义节点是局部坐标，发布时需换算）。
    origin: Point<Pixels>,
    /// 最近一次预绘制的画布矩形（用于填充零尺寸的整块摘要节点）。
    bounds: Bounds<Pixels>,
}

impl<T: 'static> Canvas<T> {
    /// 为 canvas 元素附加一个 DOM 渲染实现（仅 dom-backend feature 生效）。
    ///
    /// 闭包接收元素布局 bounds 与窗口/应用上下文，返回要登记的 [`DomNode`]；
    /// 返回 `None` 表示该元素不进入 DOM 树（与默认行为一致）。
    #[cfg(feature = "dom-backend")]
    pub fn with_dom(
        mut self,
        f: impl Fn(Bounds<Pixels>, &mut Window, &mut App) -> Option<crate::DomNode> + 'static,
    ) -> Self {
        self.dom_impl = Some(Box::new(f));
        self
    }

    /// 设置元素 id。
    ///
    /// 只有带 id 的元素才会被分配 [`GlobalElementId`]，也才能把画布语义写进
    /// 无障碍树 —— 因此 [`a11y`][Self::a11y] 需要配合本方法使用。
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// 声明画布的无障碍角色与本帧的语义节点。
    ///
    /// `semantics` 是应用对本帧画布上自绘对象的描述（见 [`CanvasSemantics`]）；
    /// 每个节点会作为画布元素的合成子节点发布。需要先 [`.id(..)`][Self::id]。
    /// 宽高都为 0 的节点在发布时会展开成整块画布，适合走势图 / 波形这类整体摘要节点
    /// （构建语义时还不知道自己的布局大小）。
    pub fn a11y(mut self, role: Role, semantics: CanvasSemantics) -> Self {
        let existing = self.a11y.take();
        self.a11y = Some(CanvasA11y {
            role,
            label: existing.as_ref().and_then(|a11y| a11y.label.clone()),
            semantics,
            on_action: existing.and_then(|a11y| a11y.on_action),
        });
        self
    }

    /// 设置画布自身的无障碍名称（辅助技术播报的容器名）。
    ///
    /// 尚未调用 [`a11y`][Self::a11y] 时以 [`Role::Canvas`] 建立配置。
    pub fn a11y_label(mut self, label: impl Into<SharedString>) -> Self {
        let a11y = self.a11y.get_or_insert_with(CanvasA11y::default);
        a11y.label = Some(label.into());
        self
    }

    /// 处理语义节点上的无障碍动作。
    ///
    /// 闭包收到被操作节点的 [`CanvasNodeId`] 与 [`Action`]（以及 AccessKit 附带的
    /// [`ActionData`]），可据此更新应用状态（移动焦点、选中对象、改数值…）。
    /// 只有节点自己声明过的动作
    /// （[`CanvasNode::with_action`](crate::canvas_a11y::CanvasNode::with_action)）才会被派发。
    pub fn on_canvas_a11y_action(
        mut self,
        handler: impl FnMut(CanvasNodeId, Action, Option<&ActionData>, &mut Window, &mut App) + 'static,
    ) -> Self {
        let handler: CanvasActionHandler = Rc::new(RefCell::new(handler));
        let a11y = self.a11y.get_or_insert_with(CanvasA11y::default);
        a11y.on_action = Some(handler);
        self
    }
}

impl Default for CanvasA11y {
    fn default() -> Self {
        Self {
            role: Role::Canvas,
            label: None,
            semantics: CanvasSemantics::new(),
            on_action: None,
        }
    }
}

impl<T: 'static> IntoElement for Canvas<T> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<T: 'static> Element for Canvas<T> {
    type RequestLayoutState = Style;
    type PrepaintState = Option<T>;

    fn id(&self) -> Option<ElementId> {
        self.id.clone()
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (crate::LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.refine(&self.style);
        let layout_id = window.request_layout(style.clone(), [], cx);
        (layout_id, style)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Style,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<T> {
        // 记下画布原点 / 矩形：语义节点是画布局部坐标，发布合成子节点时要换算成窗口坐标。
        self.origin = bounds.origin;
        self.bounds = bounds;
        // prepaint 回调构造时必带，且每次 prepaint 只取一次
        Some(
            invariant(self.prepaint.take(), "画布：prepaint 回调必存在")(
                bounds, window, cx,
            ),
        )
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        style: &mut Style,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // prepaint 闭包由本帧 prepaint 取用、paint 闭包构造时必带，各只取一次
        let prepaint = invariant(prepaint.take(), "画布：本帧 prepaint 状态必存在");
        let paint = invariant(self.paint.take(), "画布：paint 回调必存在");

        // 无障碍：登记语义节点上的动作监听器。先于自绘完成，避免与绘制闭包争借用。
        if window.is_a11y_active()
            && let (Some(global_id), Some(a11y)) = (id, self.a11y.as_ref())
            && let Some(handler) = a11y.on_action.clone()
        {
            register_canvas_a11y_actions(window, global_id, &a11y.semantics, handler);
        }

        style.paint(bounds, window, cx, |window, cx| {
            paint(bounds, prepaint, window, cx)
        });
    }

    fn a11y_role(&self) -> Option<Role> {
        self.a11y.as_ref().map(|a11y| a11y.role)
    }

    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        if let Some(a11y) = &self.a11y
            && let Some(label) = &a11y.label
        {
            node.set_label(label.to_string());
        }
    }

    fn a11y_synthetic_children(
        &mut self,
        _prepaint: &mut Self::PrepaintState,
        builder: &mut crate::A11ySubtreeBuilder,
    ) {
        if let Some(a11y) = &self.a11y {
            // 零尺寸节点（整体摘要）按画布整块区域发布；有尺寸的节点原样保留。
            let mut semantics = a11y.semantics.clone();
            semantics.fill_empty_bounds(Bounds::new(Point::default(), self.bounds.size));
            push_canvas_a11y(builder, &semantics, self.origin);
        }
    }

    /// 返回可选的自绘 DOM 节点（图表/波形/SVG 等在纯 DOM 模式下的显示）。
    #[cfg(feature = "dom-backend")]
    fn dom(
        &self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<crate::DomNode> {
        self.dom_impl.as_ref().and_then(|f| f(bounds, window, cx))
    }
}

impl<T> Styled for Canvas<T> {
    fn style(&mut self) -> &mut crate::StyleRefinement {
        &mut self.style
    }
}
