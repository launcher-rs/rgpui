//! 元素是 RGPUI 的核心组件。它们负责布局和绘制窗口的所有内容。元素形成一个树结构，并根据 [taffy](https://github.com/DioxusLabs/taffy) 实现的 Web 布局标准进行布局。大多数情况下，您不需要直接与此模块或这些 API 交互。元素提供自己的 API，RGPUI 或其他元素实现使用此模块中的 API 将元素树转换为您在屏幕上看到的像素。
//!
//! # 元素基础
//!
//! 元素是通过调用窗口根视图的 [`Render::render()`] 构建的，这会从应用程序的当前状态递归地构建元素树。然后这些元素由 Taffy 进行布局，并根据它们自己的 [`Element::paint()`] 实现绘制到屏幕上。在下一帧开始之前，整个元素树及其在 RGPUI 中注册的所有回调都会被丢弃，然后过程重复。
//!
//! 但有些状态太简单且数量太多，无法存储在每个需要它的视图中，例如是否已开始悬停。为此，RGPUI 提供了 [`Element::PrepaintState`] 关联类型。
//!
//! # 实现自己的元素
//!
//! 元素旨在成为 RGPUI 的底层命令式 API。它们负责维护或根据需要打破 RGPUI 的功能。例如，大多数 RGPUI 元素应保持在其父元素给定的边界内。但使用 [`Window::with_content_mask`]，您可以忽略此限制并在窗口边界内的任何位置绘制。这对于覆盖层、弹出窗口以及其他显示在其他元素“之上”的内容非常有用。能力越大，责任越大。
//!
//! 但是，大多数情况下，您不需要实现自己的元素。RGPUI 提供了许多开箱即用的元素，应涵盖大多数常见用例，建议您使用这些元素来构建 `components`，使用 [`RenderOnce`] trait 和 `#[derive(IntoElement)]` 宏。仅在需要手动控制布局和绘制过程时才实现元素，例如使用自己的自定义布局算法或渲染代码编辑器时。

#[cfg(feature = "dom-backend")]
use crate::DomNode;
use crate::{
    A11ySubtreeBuilder, App, ArenaBox, AvailableSpace, Bounds, Context, DispatchNodeId, ElementId,
    FocusHandle, InspectorElementId, LayoutId, Pixels, Point, Size, Style, Window,
    util::FluentBuilder, window::with_element_arena,
};
use derive_more::{Deref, DerefMut};
use std::{
    any::Any,
    fmt::{self, Debug, Display},
    mem, panic,
    sync::Arc,
};

/// 参与窗口内容布局和绘制的类型需实现此特征。
/// 元素形成一棵树，按照 Taffy 实现的 Web 布局规则进行布局。
/// 可通过实现此特征创建自定义元素，详见模块文档。
pub trait Element: 'static + IntoElement {
    /// [`Element::request_layout`] 返回的状态类型。其可变引用随后传递给
    /// [`Element::prepaint`] 和 [`Element::paint`]。
    type RequestLayoutState: 'static;

    /// [`Element::prepaint`] 返回的状态类型。其可变引用随后传递给 [`Element::paint`]。
    type PrepaintState: 'static;

    /// 如果此元素有唯一标识符，返回它。用于跨帧追踪元素，
    /// 并将 GlobalElementId 传递给 request_layout、prepaint 和 paint 方法。
    ///
    /// 全局 id 可用于访问跨帧相同 id 元素关联的状态。
    /// 该 id 在第一个拥有 id 的父元素的子元素中必须唯一。
    fn id(&self) -> Option<ElementId>;

    /// 此元素的构造源位置，用于在检查器中区分元素并导航到其源码。
    fn source_location(&self) -> Option<&'static panic::Location<'static>>;

    /// 绘制元素前，需要确定其位置和大小。
    /// 使用此方法向 Taffy 请求布局并初始化元素状态。
    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState);

    /// 布局完成后，需将其边界提交到当前帧用于命中检测。
    /// state 参数即 [`Element::request_layout()`] 返回的状态。
    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState;

    /// 布局完成后，调用此方法将元素绘制到屏幕。
    /// state 参数即 [`Element::request_layout()`] 返回的状态。
    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    );

    /// 返回此元素的无障碍角色，若无则返回 None。
    /// 返回 None 的元素不会包含在无障碍树中。
    ///
    /// 注意：加入无障碍树需要 [`id`][Element::id] 返回非 None。
    ///
    /// 概览见[无障碍指南](crate::_accessibility)。
    fn a11y_role(&self) -> Option<accesskit::Role> {
        None
    }

    /// 如果此元素参与 Web DOM 后端，返回要注册的 DOM 节点。
    ///
    /// 默认返回 None，即不进入 DOM 树。需要映射到 DOM 的元素
    /// （如 `Div`、文本）在 [`Element::paint`] 时基于布局 bounds 与当前样式
    /// 生成 [`DomNode`]；DOM 层作为 canvas 覆盖层渲染，布局沿用 Taffy 结果。
    ///
    /// 见 `docs/web-dom-backend-plan.md` 与 `docs/web-dom-backend-analysis.md`。
    #[cfg(feature = "dom-backend")]
    fn dom(&self, _bounds: Bounds<Pixels>, _window: &mut Window, _cx: &mut App) -> Option<DomNode> {
        None
    }

    /// 将无障碍属性写入给定节点。
    /// 仅在 `a11y_role()` 返回 `Some` 时调用。
    ///
    /// 概览见[无障碍指南](crate::_accessibility)。
    fn write_a11y_info(&self, _node: &mut accesskit::Node) {}

    /// 为拥有 [`.id()`][Element::id] 和 [`.role()`][Element::a11y_role] 的
    /// [`Element`] 添加合成子节点。
    ///
    /// 某些元素需要注入不对应任何 RGPUI 元素的无障碍节点。
    /// 例如自定义文本框元素可能为文本内容注入合成子节点。
    ///
    /// 详见无障碍指南中的[合成子节点](crate::_accessibility#synthetic-children)。
    fn a11y_synthetic_children(
        &mut self,
        _prepaint: &mut Self::PrepaintState,
        _builder: &mut A11ySubtreeBuilder,
    ) {
    }

    /// 将此元素转换为动态类型的 [`AnyElement`]。
    fn into_any(self) -> AnyElement {
        AnyElement::new(self)
    }
}

/// 可转换为元素的类型需实现此特征。
pub trait IntoElement: Sized {
    /// 实现类型转换后的具体元素类型。
    /// 用于自动将其他类型转换为元素，如 String。
    type Element: Element;

    /// 将 self 转换为实现 [`Element`] 的类型。
    fn into_element(self) -> Self::Element;

    /// 将 self 转换为动态类型的 [`AnyElement`]。
    fn into_any_element(self) -> AnyElement {
        self.into_element().into_any()
    }
}

impl<T: IntoElement> FluentBuilder for T {}

/// 可绘制到屏幕的对象。此特征区分"视图"与其他实体。
/// 视图是实现了 `Render` 并绘制到屏幕的 `Entity`。
pub trait Render: 'static + Sized {
    /// 将此视图渲染为元素树。
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement;
}

impl Render for Empty {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// 可对实现此特征的类型派生 [`IntoElement`]。
/// 用于从纯数据构建可复用的 `组件`。组件即特定元素模式的配方。
/// RenderOnce 允许调用此模式，同时保持元素 API 的流式构建器风格。
pub trait RenderOnce: 'static {
    /// 将此组件渲染为元素树。注意此方法获取 self 的所有权，
    /// 而 [`Render::render()`] 接收可变引用。
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement;
}

/// 辅助特征，为可接受任意数量和类型子元素的元素提供统一接口。
pub trait ParentElement {
    /// 用给定子元素扩展此元素的子元素列表。
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>);

    /// 向此元素添加单个子元素。
    fn child(mut self, child: impl IntoElement) -> Self
    where
        Self: Sized,
    {
        self.extend(std::iter::once(child.into_element().into_any()));
        self
    }

    /// 向此元素添加多个子元素。
    fn children(mut self, children: impl IntoIterator<Item = impl IntoElement>) -> Self
    where
        Self: Sized,
    {
        self.extend(children.into_iter().map(|child| child.into_any_element()));
        self
    }
}

/// 元素的全局唯一标识符，用于跨帧追踪状态。
#[derive(Deref, DerefMut, Clone, Default, Debug, Eq, PartialEq, Hash)]
pub struct GlobalElementId(pub(crate) Arc<[ElementId]>);

impl Display for GlobalElementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, element_id) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ".")?;
            }
            write!(f, "{}", element_id)?;
        }
        Ok(())
    }
}

impl GlobalElementId {
    /// 由元素 id 序列构造全局 id（Web DOM 后端等外部 crate 需要自行构造 key）。
    pub fn from_ids(ids: impl IntoIterator<Item = ElementId>) -> Self {
        Self(ids.into_iter().collect())
    }

    pub(crate) fn accesskit_node_id(&self) -> accesskit::NodeId {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::hash::DefaultHasher::default();
        self.hash(&mut hasher);
        accesskit::NodeId(hasher.finish())
    }
}

trait ElementObject {
    fn inner_element(&mut self) -> &mut dyn Any;

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId;

    fn prepaint(&mut self, window: &mut Window, cx: &mut App);

    fn paint(&mut self, window: &mut Window, cx: &mut App);

    fn layout_as_root(
        &mut self,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
    ) -> Size<Pixels>;
}

/// [`Element`] 实现者的包装器，使其可在窗口中绘制。
pub struct Drawable<E: Element> {
    /// 被绘制的元素。
    pub element: E,
    phase: ElementDrawPhase<E::RequestLayoutState, E::PrepaintState>,
}

#[derive(Default)]
enum ElementDrawPhase<RequestLayoutState, PrepaintState> {
    #[default]
    Start,
    RequestLayout {
        layout_id: LayoutId,
        global_id: Option<GlobalElementId>,
        inspector_id: Option<InspectorElementId>,
        request_layout: RequestLayoutState,
    },
    LayoutComputed {
        layout_id: LayoutId,
        global_id: Option<GlobalElementId>,
        inspector_id: Option<InspectorElementId>,
        available_space: Size<AvailableSpace>,
        request_layout: RequestLayoutState,
    },
    Prepaint {
        node_id: DispatchNodeId,
        global_id: Option<GlobalElementId>,
        inspector_id: Option<InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: RequestLayoutState,
        prepaint: PrepaintState,
    },
    Painted,
}

/// [`Element`] 实现者的包装器，使其可在窗口中绘制。
impl<E: Element> Drawable<E> {
    pub(crate) fn new(element: E) -> Self {
        Drawable {
            element,
            phase: ElementDrawPhase::Start,
        }
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        match mem::take(&mut self.phase) {
            ElementDrawPhase::Start => {
                let global_id = self.element.id().map(|element_id| {
                    window.element_id_stack.push(element_id);
                    GlobalElementId(Arc::from(&*window.element_id_stack))
                });

                let inspector_id;
                #[cfg(any(feature = "inspector", debug_assertions))]
                {
                    inspector_id = self.element.source_location().map(|source| {
                        let path = crate::InspectorElementPath {
                            global_id: GlobalElementId(Arc::from(&*window.element_id_stack)),
                            source_location: source,
                        };
                        window.build_inspector_element_id(path)
                    });
                }
                #[cfg(not(any(feature = "inspector", debug_assertions)))]
                {
                    inspector_id = None;
                }

                let (layout_id, request_layout) = self.element.request_layout(
                    global_id.as_ref(),
                    inspector_id.as_ref(),
                    window,
                    cx,
                );

                if global_id.is_some() {
                    window.element_id_stack.pop();
                }

                self.phase = ElementDrawPhase::RequestLayout {
                    layout_id,
                    global_id,
                    inspector_id,
                    request_layout,
                };
                layout_id
            }
            _ => panic!("must call request_layout only once"),
        }
    }

    pub(crate) fn prepaint(&mut self, window: &mut Window, cx: &mut App) {
        match mem::take(&mut self.phase) {
            ElementDrawPhase::RequestLayout {
                layout_id,
                global_id,
                inspector_id,
                mut request_layout,
            }
            | ElementDrawPhase::LayoutComputed {
                layout_id,
                global_id,
                inspector_id,
                mut request_layout,
                ..
            } => {
                if let Some(element_id) = self.element.id() {
                    window.element_id_stack.push(element_id);
                    debug_assert_eq!(&*global_id.as_ref().unwrap().0, &*window.element_id_stack);
                }

                let bounds = window.layout_bounds(layout_id);

                // DOM 后端：在 prepaint 阶段生成 DOM key 并压入 DOM 栈，使
                // `insert_hitbox` 能把 hitbox 关联到当前元素的 DOM key（事件委托用）。
                #[cfg(feature = "dom-backend")]
                let dom_key = if window.dom_builder_active() {
                    // DOM 后端中，可滚动容器的内容偏移由浏览器通过 `scrollTop` 原生承载，
                    // 子元素应按「未滚动」的绝对坐标定位（与画布的 `with_element_offset`
                    // 单层偏移不同，这里若再加 `element_offset` 会让滚动内容被双重偏移）。
                    let dom_node = self.element.dom(bounds, window, cx);
                    window.dom_element(dom_node, global_id.is_some())
                } else {
                    None
                };
                #[cfg(not(feature = "dom-backend"))]
                let _dom_key: Option<()> = None;

                let mut pushed_a11y_node = false;
                if window.a11y.is_active() {
                    if let Some(global_id) = global_id.as_ref() {
                        if let Some(role) = self.element.a11y_role() {
                            let node_id = global_id.accesskit_node_id();
                            let mut node = accesskit::Node::new(role);
                            let scale = window.scale_factor();
                            node.set_bounds(accesskit::Rect {
                                x0: (bounds.origin.x.0 * scale) as f64,
                                y0: (bounds.origin.y.0 * scale) as f64,
                                x1: ((bounds.origin.x.0 + bounds.size.width.0) * scale) as f64,
                                y1: ((bounds.origin.y.0 + bounds.size.height.0) * scale) as f64,
                            });
                            self.element.write_a11y_info(&mut node);
                            window.a11y.node_bounds.insert(node_id, bounds);
                            pushed_a11y_node = window.a11y.nodes.push(node_id, node);
                            #[cfg(debug_assertions)]
                            if pushed_a11y_node {
                                let view = window
                                    .a11y
                                    .view_type_names
                                    .get(&window.current_view())
                                    .copied();
                                let source_location = self.element.source_location();
                                window.a11y.nodes.record_node_info(
                                    node_id,
                                    crate::window::a11y::debug::NodeDebugInfo {
                                        synthetic: false,
                                        view,
                                        element_id: global_id.0.last().map(|id| format!("{id:?}")),
                                        source_location,
                                    },
                                );
                            }
                        }
                    }
                }

                let node_id = window.next_frame.dispatch_tree.push_node();
                let mut prepaint = self.element.prepaint(
                    global_id.as_ref(),
                    inspector_id.as_ref(),
                    bounds,
                    &mut request_layout,
                    window,
                    cx,
                );
                window.next_frame.dispatch_tree.pop_node();

                // DOM 后端：元素 prepaint 完成后弹出其 DOM key，保持 DOM 栈与
                // prepaint 嵌套顺序一致。
                #[cfg(feature = "dom-backend")]
                if dom_key.is_some() {
                    window.dom_exit();
                }

                if pushed_a11y_node {
                    if let Some(global_id) = global_id.as_ref() {
                        #[cfg(debug_assertions)]
                        let creator = crate::window::a11y::debug::NodeCreator {
                            view: window
                                .a11y
                                .view_type_names
                                .get(&window.current_view())
                                .copied(),
                            element_id: global_id.0.last().map(|id| format!("{id:?}")),
                            source_location: self.element.source_location(),
                        };
                        let mut builder = A11ySubtreeBuilder::new(
                            global_id.accesskit_node_id(),
                            &mut window.a11y.nodes,
                        );
                        #[cfg(debug_assertions)]
                        {
                            builder = builder.with_creator(creator);
                        }
                        self.element
                            .a11y_synthetic_children(&mut prepaint, &mut builder);
                    }
                    window.a11y.nodes.pop();
                }

                if global_id.is_some() {
                    window.element_id_stack.pop();
                }

                self.phase = ElementDrawPhase::Prepaint {
                    node_id,
                    global_id,
                    inspector_id,
                    bounds,
                    request_layout,
                    prepaint,
                };
            }
            _ => panic!("must call request_layout before prepaint"),
        }
    }

    pub(crate) fn paint(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> (E::RequestLayoutState, E::PrepaintState) {
        match mem::take(&mut self.phase) {
            ElementDrawPhase::Prepaint {
                node_id,
                global_id,
                inspector_id,
                bounds,
                mut request_layout,
                mut prepaint,
                ..
            } => {
                if let Some(element_id) = self.element.id() {
                    window.element_id_stack.push(element_id);
                    debug_assert_eq!(&*global_id.as_ref().unwrap().0, &*window.element_id_stack);
                }

                window.next_frame.dispatch_tree.set_active_node(node_id);
                self.element.paint(
                    global_id.as_ref(),
                    inspector_id.as_ref(),
                    bounds,
                    &mut request_layout,
                    &mut prepaint,
                    window,
                    cx,
                );

                if global_id.is_some() {
                    window.element_id_stack.pop();
                }

                self.phase = ElementDrawPhase::Painted;
                (request_layout, prepaint)
            }
            _ => panic!("must call prepaint before paint"),
        }
    }

    pub(crate) fn layout_as_root(
        &mut self,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
    ) -> Size<Pixels> {
        if matches!(&self.phase, ElementDrawPhase::Start) {
            self.request_layout(window, cx);
        }

        let layout_id = match mem::take(&mut self.phase) {
            ElementDrawPhase::RequestLayout {
                layout_id,
                global_id,
                inspector_id,
                request_layout,
            } => {
                window.compute_layout(layout_id, available_space, cx);
                self.phase = ElementDrawPhase::LayoutComputed {
                    layout_id,
                    global_id,
                    inspector_id,
                    available_space,
                    request_layout,
                };
                layout_id
            }
            ElementDrawPhase::LayoutComputed {
                layout_id,
                global_id,
                inspector_id,
                available_space: prev_available_space,
                request_layout,
            } => {
                if available_space != prev_available_space {
                    window.compute_layout(layout_id, available_space, cx);
                }
                self.phase = ElementDrawPhase::LayoutComputed {
                    layout_id,
                    global_id,
                    inspector_id,
                    available_space,
                    request_layout,
                };
                layout_id
            }
            _ => panic!("cannot measure after painting"),
        };

        window.layout_bounds(layout_id).size
    }
}

impl<E> ElementObject for Drawable<E>
where
    E: Element,
    E::RequestLayoutState: 'static,
{
    fn inner_element(&mut self) -> &mut dyn Any {
        &mut self.element
    }

    #[inline]
    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        Drawable::request_layout(self, window, cx)
    }

    #[inline]
    fn prepaint(&mut self, window: &mut Window, cx: &mut App) {
        Drawable::prepaint(self, window, cx);
    }

    #[inline]
    fn paint(&mut self, window: &mut Window, cx: &mut App) {
        Drawable::paint(self, window, cx);
    }

    #[inline]
    fn layout_as_root(
        &mut self,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
    ) -> Size<Pixels> {
        Drawable::layout_as_root(self, available_space, window, cx)
    }
}

/// 动态类型元素，可存储任意元素类型。
pub struct AnyElement(ArenaBox<dyn ElementObject>);

impl AnyElement {
    pub(crate) fn new<E>(element: E) -> Self
    where
        E: 'static + Element,
        E::RequestLayoutState: Any,
    {
        let element = with_element_arena(|arena| arena.alloc(|| Drawable::new(element)))
            .map(|element| element as &mut dyn ElementObject);
        AnyElement(element)
    }

    /// 尝试将装箱元素的引用向下转型为特定类型。
    pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.0.inner_element().downcast_mut::<T>()
    }

    /// 获取此 `AnyElement` 存储元素的布局 ID。
    /// 用于在父元素中布局子元素。
    pub fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        self.0.request_layout(window, cx)
    }

    /// 通过存储边界准备绘制元素，使其有机会绘制命中框
    /// 并在最终绘制前请求自动滚动。
    pub fn prepaint(&mut self, window: &mut Window, cx: &mut App) -> Option<FocusHandle> {
        let focus_assigned = window.next_frame.focus.is_some();

        self.0.prepaint(window, cx);

        if !focus_assigned && let Some(focus_id) = window.next_frame.focus {
            return FocusHandle::for_id(focus_id, &cx.focus_handles);
        }

        None
    }

    /// 绘制此 `AnyElement` 存储的元素。
    pub fn paint(&mut self, window: &mut Window, cx: &mut App) {
        self.0.paint(window, cx);
    }

    /// 在给定可用空间内执行此元素的布局并返回其大小。
    pub fn layout_as_root(
        &mut self,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
    ) -> Size<Pixels> {
        self.0.layout_as_root(available_space, window, cx)
    }

    /// 在给定绝对原点处预绘制此元素。
    /// 如果此元素子树中有任何元素获得焦点，返回其 FocusHandle。
    pub fn prepaint_at(
        &mut self,
        origin: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<FocusHandle> {
        window.with_absolute_element_offset(origin, |window| self.prepaint(window, cx))
    }

    /// 在可用空间内执行此元素的布局，然后在给定绝对原点处预绘制。
    /// 如果此元素子树中有任何元素获得焦点，返回其 FocusHandle。
    pub fn prepaint_as_root(
        &mut self,
        origin: Point<Pixels>,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<FocusHandle> {
        self.layout_as_root(available_space, window, cx);
        window.with_absolute_element_offset(origin, |window| self.prepaint(window, cx))
    }
}

impl Element for AnyElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = self.request_layout(window, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.paint(window, cx);
    }
}

impl IntoElement for AnyElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }

    fn into_any_element(self) -> AnyElement {
        self
    }
}

/// 空元素，不渲染任何内容。
pub struct Empty;

impl IntoElement for Empty {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Empty {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (
            window.request_layout(
                Style {
                    display: crate::Display::None,
                    ..Default::default()
                },
                None,
                cx,
            ),
            (),
        )
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}
