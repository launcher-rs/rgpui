//! WebView 组件 —— 基于 wry 的网页视图，用于在 rgpui 应用中嵌入网页内容。
//!
//! 本 crate 提供 [`WebView`] 和 [`WebViewHandle`] 两个核心类型：
//! - [`WebView`] 是 rgpui 实体，管理网页视图的生命周期和布局。
//! - [`WebViewHandle`] 是可克隆的轻量句柄，可在组件间传递以控制底层 wry WebView。
//!
//! # 示例
//!
//! ```rust,ignore
//! use rgpui_webview::{WebView, WebViewBuilder};
//!
//! // 创建 WebView 实体
//! let webview = cx.new(|window, cx| {
//!     let wry_webview = WebViewBuilder::new()
//!         .with_url("https://example.com")
//!         .build_as_child(window.raw_handle())
//!         .unwrap();
//!     WebView::new(wry_webview, window, cx)
//! });
//! ```

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

/// WebView 的可克隆句柄。
///
/// 持有此句柄会延长底层原生 WebView 的生命周期。即使拥有者 [`WebView`] 被丢弃，
/// 只要句柄或帧克隆仍存在，原生 WebView 就不会被销毁。所有句柄必须在父窗口销毁前丢弃。
#[derive(Clone)]
pub struct WebViewHandle(Rc<wry::WebView>);

impl WebViewHandle {
    /// 获取底层 wry WebView 的引用。
    pub fn raw(&self) -> &wry::WebView {
        &self.0
    }
}

/// 基于 wry WebView 的网页视图组件。
///
/// 将原生 WebView 嵌入 rgpui 布局树，支持显示网页内容、JavaScript 执行等。
/// WebView 渲染为平台原生覆盖层（macOS WebKit / Windows WebView2），
/// 位于 rgpui GPU 渲染层之上。
pub struct WebView {
    /// 焦点句柄。
    focus_handle: FocusHandle,
    /// 底层 wry WebView。
    webview: Rc<wry::WebView>,
    /// 是否可见。
    visible: bool,
    /// 当前布局边界。
    bounds: Bounds<Pixels>,
}

impl Drop for WebView {
    fn drop(&mut self) {
        self.hide();
    }
}

impl WebView {
    /// 创建新的 WebView 实体。
    ///
    /// # 参数
    /// * `webview` - 由 `wry::WebViewBuilder` 构建的原生 WebView 实例
    /// * `_window` - 当前窗口引用
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

    /// 显示 WebView。
    pub fn show(&mut self) {
        let _ = self.webview.set_visible(true);
        self.visible = true;
    }

    /// 隐藏 WebView 并将焦点返回父窗口。
    pub fn hide(&mut self) {
        _ = self.webview.focus_parent();
        _ = self.webview.set_visible(false);
        self.visible = false;
    }

    /// 获取 WebView 当前可见状态。
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 获取 WebView 当前布局边界。
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    /// 浏览器历史后退。
    pub fn back(&mut self) -> anyhow::Result<()> {
        Ok(self.webview.evaluate_script("history.back();")?)
    }

    /// 浏览器历史前进。
    pub fn forward(&mut self) -> anyhow::Result<()> {
        Ok(self.webview.evaluate_script("history.forward();")?)
    }

    /// 刷新当前页面。
    pub fn reload(&mut self) {
        let _ = self.webview.reload();
    }

    /// 加载指定 URL。
    pub fn load_url(&mut self, url: &str) {
        let _ = self.webview.load_url(url);
    }

    /// 加载 HTML 字符串内容。
    pub fn load_html(&mut self, html: &str) {
        let _ = self.webview.load_html(html);
    }

    /// 获取可克隆的 WebView 句柄。
    pub fn handle(&self) -> WebViewHandle {
        WebViewHandle(self.webview.clone())
    }

    /// 获取底层 wry WebView 的引用。
    pub fn raw(&self) -> &wry::WebView {
        &self.webview
    }

    /// 执行 JavaScript 脚本。
    pub fn eval_script(&self, script: &str) -> anyhow::Result<()> {
        Ok(self.webview.evaluate_script(script)?)
    }
}

impl Deref for WebView {
    type Target = wry::WebView;

    fn deref(&self) -> &Self::Target {
        &self.webview
    }
}

impl Focusable for WebView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<DismissEvent> for WebView {}

impl Render for WebView {
    fn render(
        &mut self,
        window: &mut Window,
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

/// WebView 元素 —— 实现 rgpui `Element` trait，负责将原生 WebView 嵌入布局树。
pub struct WebViewElement {
    /// 父 WebView 实体。
    parent: Entity<WebView>,
    /// 底层 wry WebView。
    view: Rc<wry::WebView>,
}

impl WebViewElement {
    /// 创建新的 WebView 元素。
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

        let id = window.request_layout(style, [], cx);
        (id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&rgpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        if !self.parent.read(cx).is_visible() {
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

        Some(window.insert_hitbox(bounds, rgpui::HitboxBehavior::Normal))
    }

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
                    let _ = webview.focus_parent();
                }
            });
        });
    }
}
