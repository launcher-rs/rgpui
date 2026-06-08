use rgpui::{
    AnyElement, App, ClickEvent, InteractiveElement as _, IntoElement, MouseButton, ParentElement,
    Pixels, RenderOnce, StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _,
};

use crate::{
    StyledExt as _, WindowExt as _,
    dialog::{
        Dialog, DialogButtonProps, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
    },
    h_flex, v_flex,
};

/// 警告对话框组件
///
/// 一个模态对话框，用于中断用户操作并显示重要内容，期望用户做出响应。
/// 基于 Dialog 组件构建，提供了更简化的 API 和预设默认值：
/// - 底部按钮居中对齐（Dialog 中为右对齐）
/// - 图标可选（默认禁用，可通过 `.show_icon(true)` 启用）
/// - 简化了常见警告场景的 API
/// - 使用声明式的 DialogHeader、DialogTitle、DialogDescription 和 DialogFooter 组件
/// - 同时支持命令式和声明式 API 风格
///
/// # 示例
///
/// ## 命令式 API（使用 WindowExt）
///
/// ```ignore
/// use rgpui_component::{AlertDialog, alert::AlertVariant};
///
/// // 使用 WindowExt trait
/// window.open_alert_dialog(cx, |alert, _, _| {
///     alert
///         .title("未保存的更改")
///         .description("您有未保存的更改。确定要离开吗？")
///         .show_cancel(true)
/// });
/// ```
///
/// ## 声明式 API（使用 trigger 和 content）
///
/// ```ignore
/// use rgpui_component::{AlertDialog, DialogHeader, DialogTitle, DialogDescription, DialogFooter};
///
/// AlertDialog::new(cx)
///     .trigger(Button::new("delete").label("删除"))
///     .content(|content, _, cx| {
///         content
///             .child(
///                 DialogHeader::new()
///                     .items_center()
///                     .child(DialogTitle::new().child("删除文件"))
///                     .child(DialogDescription::new().child("确定要删除吗？"))
///             )
///             .child(
///                 DialogFooter::new()
///                     .justify_center()
///                     .child(Button::new("cancel").label("取消"))
///                     .child(Button::new("confirm").label("删除"))
///             )
///     })
/// ```
#[derive(IntoElement)]
pub struct AlertDialog {
    /// 基础对话框实例
    base: Dialog,
    /// 触发器元素，点击后打开对话框
    trigger: Option<AnyElement>,
    /// 图标元素
    icon: Option<AnyElement>,
    /// 标题元素
    title: Option<AnyElement>,
    /// 描述元素
    description: Option<AnyElement>,
    /// 按钮属性配置
    button_props: DialogButtonProps,
    /// 子元素列表
    children: Vec<AnyElement>,
}

impl AlertDialog {
    /// 创建新的警告对话框
    ///
    /// 默认情况下，对话框不支持点击遮罩层关闭，且只显示确定按钮。
    /// 可通过 `.overlay_closable(true)` 启用遮罩层关闭。
    pub fn new(cx: &mut App) -> Self {
        Self {
            base: Dialog::new(cx).overlay_closable(false).close_button(false),
            trigger: None,
            icon: None,
            title: None,
            description: None,
            button_props: DialogButtonProps::default(),
            children: Vec::new(),
        }
    }

    /// 设置为确认对话框模式，同时显示确定和取消按钮
    ///
    /// 默认的 AlertDialog 只显示确定按钮。
    pub fn confirm(mut self) -> Self {
        self.button_props.show_cancel = true;
        self
    }

    /// 设置触发器元素
    ///
    /// 设置触发器后，对话框将渲染为一个可点击的元素，点击后打开对话框。
    ///
    /// **注意**：使用 `.trigger()` 时，应同时使用 `.content()` 以声明式方式定义对话框内容，
    /// 而不是使用 `.title()`、`.description()` 等方法。
    ///
    /// 当与 `.trigger()` 一起使用时，`title`、`description`、`icon` 和 `button_props` 将被忽略。
    pub fn trigger(mut self, trigger: impl IntoElement) -> Self {
        self.trigger = Some(trigger.into_any_element());
        self
    }

    /// 设置声明式内容构建器
    ///
    /// 使用此方法时，可通过 DialogHeader、DialogTitle、DialogDescription、DialogFooter 等
    /// 声明式组件来定义对话框内容。
    ///
    /// 通常与 `.trigger()` 一起使用，实现完全声明式的 API。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// AlertDialog::new(cx)
    ///     .trigger(Button::new("delete").label("删除"))
    ///     .content(|content, _, cx| {
    ///         content
    ///             .child(DialogHeader::new().child(DialogTitle::new().child("确认")))
    ///             .child(DialogFooter::new().child(Button::new("ok").label("确定")))
    ///     })
    /// ```
    pub fn content<F>(mut self, builder: F) -> Self
    where
        F: Fn(crate::dialog::DialogContent, &mut Window, &mut App) -> crate::dialog::DialogContent
            + 'static,
    {
        self.base = self.base.content(builder);
        self
    }

    /// 设置声明式底部区域构建器
    ///
    /// 用于通过 DialogFooter 等声明式组件定义底部内容。
    ///
    /// 如果未设置，将使用默认的底部区域，包含确定按钮和可选的取消按钮。
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.base = self.base.footer(footer);
        self
    }

    /// 调试断言：确保未同时使用 trigger 和其他属性
    #[track_caller]
    fn debug_assert_no_trigger(&self) {
        debug_assert!(
            self.trigger.is_none() && self.base.content_builder.is_none(),
            "使用 trigger 时不能设置此属性。请使用 content() 定义对话框内容。"
        );
    }

    /// 设置警告对话框的图标，默认为 None
    #[track_caller]
    pub fn icon(mut self, icon: impl IntoElement) -> Self {
        self.debug_assert_no_trigger();
        self.icon = Some(icon.into_any_element());
        self
    }

    /// 设置警告对话框的标题
    #[track_caller]
    pub fn title(mut self, title: impl IntoElement) -> Self {
        self.debug_assert_no_trigger();
        self.title = Some(title.into_any_element());
        self
    }

    /// 设置警告对话框的描述
    #[track_caller]
    pub fn description(mut self, description: impl IntoElement) -> Self {
        self.debug_assert_no_trigger();
        self.description = Some(description.into_any_element());
        self
    }

    /// 设置按钮属性配置
    ///
    /// 用于配置按钮文本、样式变体和可见性。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// alert.button_props(
    ///     DialogButtonProps::default()
    ///         .ok_text("删除")
    ///         .ok_variant(ButtonVariant::Danger)
    ///         .cancel_text("保留")
    ///         .show_cancel(true)
    /// )
    /// ```
    #[track_caller]
    pub fn button_props(mut self, button_props: DialogButtonProps) -> Self {
        self.debug_assert_no_trigger();
        self.button_props = button_props;
        self
    }

    /// 设置警告对话框的宽度，默认为 420px
    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.base = self.base.width(width);
        self
    }

    /// 设置是否显示取消按钮，默认为 false
    pub fn show_cancel(mut self, show_cancel: bool) -> Self {
        self.button_props = self.button_props.show_cancel(show_cancel);
        self
    }

    /// 设置是否支持点击遮罩层关闭对话框，默认为 `false`
    pub fn overlay_closable(mut self, overlay_closable: bool) -> Self {
        self.base = self.base.overlay_closable(overlay_closable);
        self
    }

    /// 设置是否显示关闭按钮，默认为 `false`
    pub fn close_button(mut self, close_button: bool) -> Self {
        self.base = self.base.close_button(close_button);
        self
    }

    /// 设置是否支持键盘 ESC 键关闭对话框，默认为 `true`
    pub fn keyboard(mut self, keyboard: bool) -> Self {
        self.base = self.base.keyboard(keyboard);
        self
    }

    /// 设置对话框关闭时的回调
    ///
    /// 在 `on_action` 或 `on_cancel` 回调之后调用。
    pub fn on_close(
        mut self,
        on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.base = self.base.on_close(on_close);
        self
    }

    /// 设置确定/操作按钮点击时的回调
    ///
    /// 回调返回 `true` 时关闭对话框，返回 `false` 时对话框保持打开。
    pub fn on_ok(
        mut self,
        on_ok: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.button_props = self.button_props.on_ok(on_ok);
        self
    }

    /// 设置取消按钮点击时的回调
    ///
    /// 回调返回 `true` 时关闭对话框，返回 `false` 时对话框保持打开。
    pub fn on_cancel(
        mut self,
        on_cancel: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.button_props = self.button_props.on_cancel(on_cancel);
        self
    }

    /// 将 AlertDialog 转换为已配置的 Dialog
    ///
    /// 此方法组装对话框的头部、内容和底部区域。
    pub(crate) fn into_dialog(self, window: &mut Window, cx: &mut App) -> Dialog {
        let button_props = self.button_props.clone();
        let has_title = self.icon.is_some() || self.title.is_some();
        let has_header = has_title || self.description.is_some();
        let has_footer = self.base.footer.is_some();

        self.base
            .button_props(button_props.clone())
            .when(has_header, |this| {
                this.header(
                    DialogHeader::new().child(
                        h_flex()
                            .gap_2()
                            .items_start()
                            .when_some(self.icon, |row, icon| row.child(icon))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_1()
                                    .when_some(self.title, |this, title| {
                                        this.child(DialogTitle::new().child(title))
                                    })
                                    .when_some(self.description, |this, desc| {
                                        this.child(DialogDescription::new().child(desc))
                                    }),
                            ),
                    ),
                )
            })
            .children(self.children)
            .when(!has_footer, |this| {
                // 用户未提供底部区域时，使用默认底部区域，包含确定按钮和可选取消按钮
                this.footer(
                    DialogFooter::new()
                        .when(button_props.show_cancel, |this| {
                            this.child(button_props.render_cancel(window, cx))
                        })
                        .child(button_props.render_ok(window, cx)),
                )
            })
    }
}

impl Styled for AlertDialog {
    /// 获取可修改的样式引用
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.base.style
    }
}

impl ParentElement for AlertDialog {
    /// 向警告对话框中添加子元素
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl AlertDialog {
    /// 渲染触发器模式的警告对话框
    ///
    /// 点击触发器元素时打开对话框。
    fn render_trigger(self, trigger: AnyElement, _: &mut Window, _: &mut App) -> AnyElement {
        let content_builder = self.base.content_builder.clone();
        let style = self.base.style.clone();
        let props = self.base.props.clone();
        let button_props = self.button_props.clone();

        div()
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                let content_builder = content_builder.clone();
                let style = style.clone();
                let props = props.clone();
                let button_props = button_props.clone();
                window.open_dialog(cx, move |dialog, _, _| {
                    dialog
                        .refine_style(&style)
                        .button_props(button_props.clone())
                        .with_props(props.clone())
                        .when_some(content_builder.clone(), |this, content_builder| {
                            this.content(move |content, window, cx| {
                                content_builder(content, window, cx)
                            })
                        })
                });
                cx.stop_propagation();
            })
            .child(trigger)
            .into_any_element()
    }
}

impl RenderOnce for AlertDialog {
    /// 渲染警告对话框
    ///
    /// 如果设置了触发器，渲染触发器元素；否则直接渲染对话框内容。
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if let Some(trigger) = self.trigger.take() {
            // 有触发器时，渲染点击后打开对话框的触发器元素
            self.render_trigger(trigger, window, cx)
        } else {
            // 无触发器时，直接渲染对话框内容
            self.into_dialog(window, cx).into_any_element()
        }
    }
}
