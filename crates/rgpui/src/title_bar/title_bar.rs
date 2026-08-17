use std::rc::Rc;

use smallvec::SmallVec;

use crate::{
    ActiveTheme, Icon, IconName, InteractiveElementExt as _, Sizable as _, StyledExt, h_flex,
};
use crate::{
    AnyElement, App, ClickEvent, Decorations, Hsla, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, RenderOnce, StatefulInteractiveElement as _, StyleRefinement, Styled,
    TitlebarOptions, Window, WindowControlArea, WindowOptions, div, point,
    prelude::FluentBuilder as _, px,
};

/// 标题栏的默认高度。
pub const TITLE_BAR_HEIGHT: Pixels = px(34.);
/// 标题栏左侧内边距。
#[cfg(target_os = "macos")]
const TITLE_BAR_LEFT_PADDING: Pixels = px(80.);
/// 标题栏左侧内边距。
#[cfg(not(target_os = "macos"))]
const TITLE_BAR_LEFT_PADDING: Pixels = px(12.);

/// TitleBar 用于自定义标题栏的外观。
///
/// 我们可以在标题栏内部放置一些元素。
#[derive(IntoElement)]
pub struct TitleBar {
    style: StyleRefinement,
    children: SmallVec<[AnyElement; 1]>,
    on_close_window: Option<Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>>,
}

impl TitleBar {
    /// 创建一个新的 TitleBar。
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: SmallVec::new(),
            on_close_window: None,
        }
    }

    /// 返回与 [`TitleBar`] 兼容的默认标题栏选项。
    pub fn title_bar_options() -> TitlebarOptions {
        TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: Some(point(px(9.0), px(9.0))),
        }
    }

    /// 返回与 [`TitleBar`] 兼容的默认窗口选项。
    ///
    /// 将其作为渲染 [`TitleBar`] 的任意窗口的 [`WindowOptions`] 基础使用，
    /// 以便标题栏自行处理拖拽与双击事件：
    ///
    /// ```no_run
    /// # use crate::WindowOptions;
    /// # use crate::TitleBar;
    /// let options = WindowOptions {
    ///     window_min_size: None,
    ///     ..TitleBar::window_options()
    /// };
    /// ```
    pub fn window_options() -> WindowOptions {
        WindowOptions {
            titlebar: Some(Self::title_bar_options()),
            // 标题栏自行绘制并通过 `start_window_move` 移动窗口，
            // 因此 AppKit 不得将其视为系统窗口移动区域。否则 macOS
            // 会自行处理标题栏双击（在下方 `on_double_click` 之外）
            // 并在消除双击歧义期间延迟标题栏点击。
            app_owns_titlebar_drag: true,
            ..Default::default()
        }
    }

    /// 为关闭窗口事件添加自定义处理，默认为 None，此时点击 X 按钮将调用 `window.remove_window()`。
    ///
    /// 仅 Linux 平台生效，其他平台无任何作用。
    pub fn on_close_window(
        mut self,
        f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        if cfg!(target_os = "linux") {
            self.on_close_window = Some(Rc::new(Box::new(f)));
        }
        self
    }
}

// Windows 控制按钮的固定宽度为 35px。
//
// 我们不需要为控制按钮实现点击事件。
// 如果用户点击在边界内，窗口事件将被触发。
#[derive(IntoElement, Clone)]
enum ControlIcon {
    Minimize,
    Restore,
    Maximize,
    Close {
        on_close_window: Option<Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>>,
    },
}

impl ControlIcon {
    /// 创建一个最小化按钮。
    fn minimize() -> Self {
        Self::Minimize
    }

    /// 创建一个还原按钮。
    fn restore() -> Self {
        Self::Restore
    }

    /// 创建一个最大化按钮。
    fn maximize() -> Self {
        Self::Maximize
    }

    /// 创建一个关闭按钮，可携带自定义关闭回调。
    fn close(on_close_window: Option<Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>>) -> Self {
        Self::Close { on_close_window }
    }

    /// 返回按钮的元素 id。
    fn id(&self) -> &'static str {
        match self {
            Self::Minimize => "minimize",
            Self::Restore => "restore",
            Self::Maximize => "maximize",
            Self::Close { .. } => "close",
        }
    }

    /// 返回按钮对应的图标名。
    fn icon(&self) -> IconName {
        match self {
            Self::Minimize => IconName::WindowMinimize,
            Self::Restore => IconName::WindowRestore,
            Self::Maximize => IconName::WindowMaximize,
            Self::Close { .. } => IconName::WindowClose,
        }
    }

    /// 返回按钮对应的窗口控制区域。
    fn window_control_area(&self) -> WindowControlArea {
        match self {
            Self::Minimize => WindowControlArea::Min,
            Self::Restore | Self::Maximize => WindowControlArea::Max,
            Self::Close { .. } => WindowControlArea::Close,
        }
    }

    /// 是否为关闭按钮。
    fn is_close(&self) -> bool {
        matches!(self, Self::Close { .. })
    }

    /// 返回按钮悬停时的前景色。
    #[inline]
    fn hover_fg(&self, cx: &App) -> Hsla {
        if self.is_close() {
            cx.theme().danger_foreground
        } else {
            cx.theme().secondary_foreground
        }
    }

    /// 返回按钮悬停时的背景色。
    #[inline]
    fn hover_bg(&self, cx: &App) -> Hsla {
        if self.is_close() {
            cx.theme().danger
        } else {
            cx.theme().secondary_hover
        }
    }

    /// 返回按钮按下时的背景色。
    #[inline]
    fn active_bg(&self, cx: &mut App) -> Hsla {
        if self.is_close() {
            cx.theme().danger_active
        } else {
            cx.theme().secondary_active
        }
    }
}

impl RenderOnce for ControlIcon {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let is_linux = cfg!(target_os = "linux");
        let is_windows = cfg!(target_os = "windows");
        let hover_fg = self.hover_fg(cx);
        let hover_bg = self.hover_bg(cx);
        let active_bg = self.active_bg(cx);
        let icon = self.clone();
        let on_close_window = match &self {
            ControlIcon::Close { on_close_window } => on_close_window.clone(),
            _ => None,
        };

        div()
            .id(self.id())
            .flex()
            .w(TITLE_BAR_HEIGHT)
            .h_full()
            .flex_shrink_0()
            .justify_center()
            .content_center()
            .items_center()
            .text_color(cx.theme().foreground)
            .hover(|style| style.bg(hover_bg).text_color(hover_fg))
            .active(|style| style.bg(active_bg).text_color(hover_fg))
            .when(is_windows, |this| {
                this.window_control_area(self.window_control_area())
            })
            .when(is_linux, |this| {
                this.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    match icon {
                        Self::Minimize => window.minimize_window(),
                        Self::Restore | Self::Maximize => window.zoom_window(),
                        Self::Close { .. } => {
                            if let Some(f) = on_close_window.clone() {
                                f(&ClickEvent::default(), window, cx);
                            } else {
                                window.remove_window();
                            }
                        }
                    }
                })
            })
            .child(Icon::new(self.icon()).small())
    }
}

/// 窗口控制按钮组（最小化 / 最大化 / 关闭）。
#[derive(IntoElement)]
struct WindowControls {
    on_close_window: Option<Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>>,
}

impl RenderOnce for WindowControls {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        if cfg!(target_os = "macos") || cfg!(target_family = "wasm") {
            return div().id("window-controls");
        }

        // 窗口管理器声明它可以支持哪些控件；平铺合成器可能
        // 既不支持最小化也不支持最大化。关闭始终由我们提供。
        let supported = window.window_controls();

        h_flex()
            .id("window-controls")
            .items_center()
            .flex_shrink_0()
            .h_full()
            .when(supported.minimize, |this| {
                this.child(ControlIcon::minimize())
            })
            .when(supported.maximize, |this| {
                this.child(if window.is_maximized() {
                    ControlIcon::restore()
                } else {
                    ControlIcon::maximize()
                })
            })
            .child(ControlIcon::close(self.on_close_window))
    }
}

impl Styled for TitleBar {
    fn style(&mut self) -> &mut crate::StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for TitleBar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// 标题栏内部状态，记录鼠标按下时是否需要开始移动窗口。
struct TitleBarState {
    should_move: bool,
}

impl RenderOnce for TitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let is_client_decorated = matches!(window.window_decorations(), Decorations::Client { .. });
        let is_web = cfg!(target_family = "wasm");
        let is_linux = cfg!(target_os = "linux");
        let is_macos = cfg!(target_os = "macos");

        let state = window.use_state(cx, |_, _| TitleBarState { should_move: false });

        div().flex_shrink_0().child(
            div()
                .id("title-bar")
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .h(TITLE_BAR_HEIGHT)
                .pl(TITLE_BAR_LEFT_PADDING)
                .border_b_1()
                .border_color(cx.theme().title_bar_border)
                .bg(cx.theme().tokens.title_bar)
                .refine_style(&self.style)
                .when(is_linux, |this| {
                    this.on_double_click(|_, window, _| window.zoom_window())
                })
                .when(is_macos, |this| {
                    this.on_double_click(|_, window, _| window.titlebar_double_click())
                })
                .on_mouse_down_out(window.listener_for(&state, |state, _, _, _| {
                    state.should_move = false;
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    window.listener_for(&state, |state, _, _, _| {
                        state.should_move = true;
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    window.listener_for(&state, |state, _, _, _| {
                        state.should_move = false;
                    }),
                )
                .on_mouse_move(window.listener_for(&state, |state, _, window, _| {
                    if state.should_move {
                        state.should_move = false;
                        window.start_window_move();
                    }
                }))
                .child(
                    h_flex()
                        .id("bar")
                        .h_full()
                        .justify_between()
                        .flex_shrink_0()
                        .flex_1()
                        .when(!is_web, |this| {
                            this.window_control_area(WindowControlArea::Drag)
                                .when(window.is_fullscreen(), |this| this.pl_3())
                                .when(is_linux && is_client_decorated, |this| {
                                    this.child(
                                        div()
                                            .top_0()
                                            .left_0()
                                            .absolute()
                                            .size_full()
                                            .h_full()
                                            .on_mouse_down(
                                                MouseButton::Right,
                                                move |ev, window, _| {
                                                    window.show_window_menu(ev.position)
                                                },
                                            ),
                                    )
                                })
                        })
                        .children(self.children),
                )
                .child(WindowControls {
                    on_close_window: self.on_close_window,
                }),
        )
    }
}
