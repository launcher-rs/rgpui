use std::{ops::Deref, rc::Rc};

use wry::{
    Rect,
    dpi::{self, LogicalSize},
};

use rgpui::{
    App, Bounds, ContentMask, DismissEvent, Element, ElementId, Entity, EventEmitter, FocusHandle,
    Focusable, GlobalElementId, Hitbox, InteractiveElement, IntoElement, LayoutId, MouseDownEvent,
    ParentElement as _, Pixels, Render, Size, Style, Styled as _, Window, canvas, div,
};

/// 基于 wry WebView 的网页视图组件。
///
/// [实验性功能]
pub struct WebView {
    /// 焦点处理器
    focus_handle: FocusHandle,
    /// 底层 wry WebView 实例
    webview: Rc<wry::WebView>,
    /// 可见性状态
    visible: bool,
    /// 当前边界尺寸
    bounds: Bounds<Pixels>,
}

impl Drop for WebView {
    fn drop(&mut self) {
        self.hide();
    }
}

impl WebView {
    /// 从 wry WebView 创建新的 WebView 实例
    ///
    /// # 参数
    ///
    /// * `webview` - wry WebView 实例
    /// * `window` - GPUI 窗口
    /// * `cx` - 应用上下文
    pub fn new(webview: wry::WebView, _: &mut Window, cx: &mut App) -> Self {
        let _ = webview.set_bounds(Rect::default());

        Self {
            focus_handle: cx.focus_handle(),
            visible: true,
            bounds: Bounds::default(),
            webview: Rc::new(webview),
        }
    }

    /// 显示网页视图
    pub fn show(&mut self) {
        let _ = self.webview.set_visible(true);
        self.visible = true;
    }

    /// 隐藏网页视图
    pub fn hide(&mut self) {
        _ = self.webview.focus_parent();
        _ = self.webview.set_visible(false);
        self.visible = false;
    }

    /// 获取网页视图的可见性状态
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// 获取网页视图的当前边界尺寸
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    /// 在网页视图中后退到历史记录上一页
    pub fn back(&mut self) -> anyhow::Result<()> {
        Ok(self.webview.evaluate_script("history.back();")?)
    }

    /// 在网页视图中加载指定 URL
    ///
    /// # 参数
    ///
    /// * `url` - 要加载的 URL 地址
    pub fn load_url(&mut self, url: &str) {
        let _ = self.webview.load_url(url);
    }

    /// 获取底层的 wry WebView 引用
    pub fn raw(&self) -> &wry::WebView {
        &self.webview
    }
}

impl Deref for WebView {
    type Target = wry::WebView;

    fn deref(&self) -> &Self::Target {
        &self.webview
    }
}

impl Focusable for WebView {
    fn focus_handle(&self, _cx: &rgpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<DismissEvent> for WebView {}

impl Render for WebView {
    fn render(
        &mut self,
        window: &mut rgpui::Window,
        cx: &mut rgpui::Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity().clone();

        div()
            .track_focus(&self.focus_handle)
            .size_full()
            .child({
                let view = cx.entity().clone();
                canvas(
                    move |bounds, _, cx| view.update(cx, |r, _| r.bounds = bounds),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full()
            })
            .child(WebViewElement::new(self.webview.clone(), view, window, cx))
    }
}

/// 可嵌入 GPUI 布局的 WebView 元素
pub struct WebViewElement {
    /// 父级 WebView 实体
    parent: Entity<WebView>,
    /// wry WebView 实例的引用计数指针
    view: Rc<wry::WebView>,
}

impl WebViewElement {
    /// 从 wry WebView 创建新的 WebView 元素
    ///
    /// # 参数
    ///
    /// * `view` - wry WebView 实例
    /// * `parent` - 父级 WebView 实体
    /// * `window` - GPUI 窗口
    /// * `cx` - 应用上下文
    pub fn new(
        view: Rc<wry::WebView>,
        parent: Entity<WebView>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self {
        Self { view, parent }
    }
}

impl IntoElement for WebViewElement {
    type Element = WebViewElement;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for WebViewElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    /// 请求布局，使用全尺寸样式并设置弹性收缩
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&rgpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = Style {
            size: Size::full(),
            flex_shrink: 1.,
            ..Default::default()
        };

        // 如果父级视图不再可见，则不需要布局网页视图
        let id = window.request_layout(style, [], cx);
        (id, ())
    }

    /// 预绘制阶段，设置网页视图的边界并创建命中框
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&rgpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        if !self.parent.read(cx).visible() {
            return None;
        }

        let _ = self.view.set_bounds(Rect {
            size: dpi::Size::Logical(LogicalSize {
                width: bounds.size.width.into(),
                height: bounds.size.height.into(),
            }),
            position: dpi::Position::Logical(dpi::LogicalPosition::new(
                bounds.origin.x.into(),
                bounds.origin.y.into(),
            )),
        });

        // 创建命中框以处理鼠标事件
        Some(window.insert_hitbox(bounds, rgpui::HitboxBehavior::Normal))
    }

    /// 绘制阶段，设置内容蒙版并处理鼠标点击事件
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&rgpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        _: &mut App,
    ) {
        let bounds = hitbox.clone().map(|h| h.bounds).unwrap_or(bounds);
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            let webview = self.view.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, _, _, _| {
                if !bounds.contains(&event.position) {
                    // 点击空白区域以清除输入焦点
                    let _ = webview.focus_parent();
                }
            });
        });
    }
}
