//! 底部弹层组件：从底部滑出的面板（Bottom Sheet）。

use crate::{prelude::FluentBuilder as _, *};
use std::{rc::Rc, time::Duration};

/// 底部弹层尺寸。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BottomSheetSize {
    /// 小尺寸。
    Sm,
    /// 中等尺寸（默认）。
    #[default]
    Md,
    /// 大尺寸。
    Lg,
    /// 超大尺寸。
    Xl,
    /// 自定义高度。
    Custom,
}

impl BottomSheetSize {
    /// 获取默认高度。
    fn height(&self) -> Pixels {
        match self {
            Self::Sm => px(300.0),
            Self::Md => px(400.0),
            Self::Lg => px(500.0),
            Self::Xl => px(600.0),
            Self::Custom => px(400.0),
        }
    }
}

/// 底部弹层组件。
#[derive(IntoElement)]
pub struct BottomSheet {
    /// 尺寸。
    size: BottomSheetSize,
    /// 自定义高度。
    custom_height: Option<Pixels>,
    /// 标题。
    title: Option<SharedString>,
    /// 描述。
    description: Option<SharedString>,
    /// 内容。
    content: Option<AnyElement>,
    /// 头部操作区。
    actions: Option<AnyElement>,
    /// 是否显示拖拽手柄。
    show_drag_handle: bool,
    /// 点击背景遮罩是否关闭。
    close_on_backdrop_click: bool,
    /// 关闭回调。
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl BottomSheet {
    /// 创建底部弹层。
    pub fn new() -> Self {
        Self {
            size: BottomSheetSize::default(),
            custom_height: None,
            title: None,
            description: None,
            content: None,
            actions: None,
            show_drag_handle: true,
            close_on_backdrop_click: true,
            on_close: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置尺寸。
    pub fn size(mut self, size: BottomSheetSize) -> Self {
        self.size = size;
        self
    }

    /// 设置自定义高度。
    pub fn height(mut self, height: impl Into<Pixels>) -> Self {
        self.custom_height = Some(height.into());
        self.size = BottomSheetSize::Custom;
        self
    }

    /// 设置标题。
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// 设置描述。
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 设置内容。
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    /// 设置头部操作区。
    pub fn actions(mut self, actions: impl IntoElement) -> Self {
        self.actions = Some(actions.into_any_element());
        self
    }

    /// 设置是否显示拖拽手柄。
    pub fn show_drag_handle(mut self, show: bool) -> Self {
        self.show_drag_handle = show;
        self
    }

    /// 设置点击背景遮罩是否关闭。
    pub fn close_on_backdrop_click(mut self, close: bool) -> Self {
        self.close_on_backdrop_click = close;
        self
    }

    /// 设置关闭回调。
    pub fn on_close<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_close = Some(Rc::new(handler));
        self
    }

    /// 获取面板高度。
    fn get_sheet_height(&self) -> Pixels {
        if let Some(height) = self.custom_height {
            return height;
        }
        self.size.height()
    }
}

impl Default for BottomSheet {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for BottomSheet {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for BottomSheet {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let background = theme.tokens.background;
        let border = theme.tokens.border;
        let radius_lg = theme.radius_lg;
        let foreground = theme.tokens.foreground;
        let muted_foreground = theme.tokens.muted_foreground;
        let font_size = theme.font_size;
        let has_header =
            self.title.is_some() || self.description.is_some() || self.actions.is_some();
        let sheet_height = self.get_sheet_height();
        let on_close = self.on_close.clone();
        let user_style = self.style;

        deferred(
            div()
                .absolute()
                .inset_0()
                .flex()
                .flex_col()
                .bg(hsla(0.0, 0.0, 0.0, 0.6))
                .when(self.close_on_backdrop_click, |this: Div| {
                    let on_close = on_close.clone();
                    this.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        if let Some(handler) = on_close.as_ref() {
                            handler(window, cx);
                        }
                    })
                })
                .child(
                    div()
                        .id("bottom-sheet-panel")
                        .occlude()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(sheet_height)
                        .flex()
                        .flex_col()
                        .bg(background)
                        .border_t_1()
                        .border_color(border)
                        .rounded_tl(radius_lg)
                        .rounded_tr(radius_lg)
                        .shadow(vec![BoxShadow {
                            color: hsla(0.0, 0.0, 0.0, 0.3),
                            offset: point(px(0.0), px(-4.0)),
                            blur_radius: px(24.0),
                            spread_radius: px(0.0),
                            inset: false,
                        }])
                        .map(|this| {
                            let mut div = this;
                            div.style().refine(&user_style);
                            div
                        })
                        .when(self.show_drag_handle, |this| {
                            this.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .pt(px(12.0))
                                    .pb(px(8.0))
                                    .child(
                                        div()
                                            .w(px(40.0))
                                            .h(px(4.0))
                                            .bg(theme.tokens.muted.opacity(0.5))
                                            .rounded(px(2.0)),
                                    ),
                            )
                        })
                        .when(has_header, |this| {
                            this.child(
                                div()
                                    .flex()
                                    .items_start()
                                    .justify_between()
                                    .px(px(24.0))
                                    .pt(px(16.0))
                                    .pb(px(16.0))
                                    .border_b_1()
                                    .border_color(border)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(4.0))
                                            .when_some(self.title, |this: Div, title| {
                                                this.child(
                                                    div()
                                                        .text_size(font_size * 1.2)
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(foreground)
                                                        .child(title),
                                                )
                                            })
                                            .when_some(self.description, |this: Div, desc| {
                                                this.child(
                                                    div()
                                                        .text_size(font_size * 0.85)
                                                        .text_color(muted_foreground)
                                                        .child(desc),
                                                )
                                            }),
                                    )
                                    .when_some(self.actions, |this: Div, actions| {
                                        this.child(
                                            div().flex().items_center().gap(px(8.0)).child(actions),
                                        )
                                    }),
                            )
                        })
                        .when_some(self.content, |this, content| {
                            this.child(div().flex_1().overflow_hidden().child(content))
                        })
                        .on_mouse_down(MouseButton::Left, |_, _, _| {})
                        .with_animation(
                            "bottom-sheet-slide",
                            Animation::new(Duration::from_millis(250))
                                .with_easing(crate::ease_out_cubic),
                            |div, delta| div.mb(px(-600.0 * (1.0 - delta))),
                        ),
                ),
        )
        .with_priority(1)
    }
}
