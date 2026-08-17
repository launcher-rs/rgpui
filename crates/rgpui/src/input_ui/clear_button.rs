use rgpui::{App, Button, ButtonVariants as _, Icon, IconName, Sizable as _};

/// 创建清除按钮（输入框右侧的小 × 图标按钮）。
#[inline]
pub(crate) fn clear_button(_: &App) -> Button {
    Button::new("clean")
        .icon(Icon::new(IconName::Close))
        .text()
        .xsmall()
        .tab_stop(false)
}