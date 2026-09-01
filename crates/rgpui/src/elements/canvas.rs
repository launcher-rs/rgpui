//! 画布元素，提供自定义绘图回调，用于绘制任意图形内容。

use crate::refineable::Refineable as _;

use crate::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, Pixels,
    Style, StyleRefinement, Styled, Window,
};

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
    }
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
        None
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
        Some(self.prepaint.take().unwrap()(bounds, window, cx))
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        style: &mut Style,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let prepaint = prepaint.take().unwrap();
        style.paint(bounds, window, cx, |window, cx| {
            (self.paint.take().unwrap())(bounds, prepaint, window, cx)
        });
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
