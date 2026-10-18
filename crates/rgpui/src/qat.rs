//! 快速访问工具栏（Quick Access Toolbar，QAT）—— 套件风格的一排紧凑命令按钮。
//!
//! QAT 只承载「常用命令的快捷入口」，按钮与 Ribbon/菜单/命令面板一样按
//! **命令 id** 绑定，点击经 [`App::dispatch_command`](crate::App::dispatch_command)
//! 派发，因此可放在标题栏、Ribbon 之上或状态栏等任意位置。
//!
//! ## 用法
//!
//! ```ignore
//! use rgpui::qat::{QatItem, QuickAccessToolbar};
//! use rgpui::IconName;
//!
//! QuickAccessToolbar::new()
//!     .item(QatItem::button("app::Save").icon(IconName::Check))
//!     .item(QatItem::button("app::Undo").icon(IconName::ArrowLeft))
//!     .item(QatItem::separator())
//!     .item(QatItem::button("app::Settings").icon(IconName::Settings))
//! ```

use crate::util::FluentBuilder as _;
use crate::{
    ActiveTheme as _, App, Icon, IconName, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement, RenderOnce, SharedString, Sizable as _, StatefulInteractiveElement as _, Styled,
    Window, div, h_flex, px,
};

/// 快速访问工具栏中的一个项。
pub enum QatItem {
    /// 命令按钮。
    Button(QatButton),
    /// 分组分隔符。
    Separator,
}

impl QatItem {
    /// 创建命令按钮。
    pub fn button(command: impl Into<SharedString>) -> QatButton {
        QatButton::new(command)
    }

    /// 创建分隔符。
    pub fn separator() -> Self {
        QatItem::Separator
    }
}

impl From<QatButton> for QatItem {
    fn from(value: QatButton) -> Self {
        QatItem::Button(value)
    }
}

/// 快速访问工具栏按钮。
pub struct QatButton {
    command: SharedString,
    label: Option<SharedString>,
    icon: Option<IconName>,
}

impl QatButton {
    /// 创建按钮，绑定命令 id。
    pub fn new(command: impl Into<SharedString>) -> Self {
        Self {
            command: command.into(),
            label: None,
            icon: None,
        }
    }

    /// 覆盖显示标签（默认取注册表标签）。
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 设置图标。
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// 快速访问工具栏：一排紧凑的命令按钮，按命令 id 派发。
#[derive(IntoElement)]
pub struct QuickAccessToolbar {
    items: Vec<QatItem>,
}

impl QuickAccessToolbar {
    /// 创建空的快速访问工具栏。
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// 追加一个项。
    pub fn item(mut self, item: impl Into<QatItem>) -> Self {
        self.items.push(item.into());
        self
    }

    /// 批量追加项（每一项都需可转换为 [`QatItem`]）。
    pub fn items<I>(mut self, items: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<QatItem>,
    {
        self.items.extend(items.into_iter().map(Into::into));
        self
    }
}

impl RenderOnce for QuickAccessToolbar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .items_center()
            .gap(px(2.0))
            .px(px(4.0))
            .py(px(2.0))
            .children(self.items.iter().map(|item| render_item(item, cx)))
    }
}

/// 渲染单个 QAT 项（QAT 与迷你工具栏共用）。
pub(crate) fn render_item(item: &QatItem, cx: &App) -> crate::AnyElement {
    match item {
        QatItem::Button(button) => render_button(button, cx).into_any_element(),
        QatItem::Separator => div()
            .w(px(1.0))
            .h(px(16.0))
            .mx(px(2.0))
            .bg(cx.theme().border)
            .into_any_element(),
    }
}

/// 从注册表解析命令标签（未登记时回退为命令 id）。
fn command_label(command: &SharedString, override_label: &Option<SharedString>) -> SharedString {
    match override_label {
        Some(label) => label.clone(),
        None => crate::CommandRegistry::global().label_for(command),
    }
}

/// 派发命令的回调（吞掉错误，仅记日志）。
fn dispatch(
    command: &SharedString,
) -> impl Fn(&crate::ClickEvent, &mut Window, &mut App) + 'static {
    let command = command.clone();
    move |_event, _window, cx| {
        if let Err(error) = cx.dispatch_command(&command, None) {
            log::warn!("qat：派发命令 `{command}` 失败：{error}");
        }
    }
}

/// 渲染一个工具按钮（图标 + 可选标签横排）。
fn render_button(button: &QatButton, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let label = command_label(&button.command, &button.label);
    let foreground = theme.foreground;

    h_flex()
        .id(button.command.clone())
        .items_center()
        .gap(px(4.0))
        .px(px(6.0))
        .py(px(2.0))
        .rounded_sm()
        .cursor_pointer()
        .text_xs()
        .text_color(foreground)
        .hover(|style| style.bg(theme.muted))
        .when_some(button.icon.clone(), |this, icon| {
            this.child(Icon::new(icon).with_size(px(14.0)).text_color(foreground))
        })
        .child(label)
        // 按钮按下时终止冒泡：避免父级的拖拽选择/拖动手势被工具按钮打断。
        .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
            cx.stop_propagation();
        })
        .on_click(dispatch(&button.command))
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{QatItem, QuickAccessToolbar, command_label};
    use crate::{IntoElement, Render, SharedString, Window};

    /// 显式标签优先于注册表标签。
    #[test]
    fn command_label_prefers_override() {
        let command = SharedString::from("rgpui_test::Open");
        assert_eq!(
            command_label(&command, &Some(SharedString::from("自定义"))).as_ref(),
            "自定义"
        );
    }

    /// 未显式指定时从命令注册表读取标签，未登记则回退为命令 id。
    #[test]
    fn command_label_falls_back_to_registry_then_id() {
        let command = SharedString::from("rgpui_test::Open");
        assert_eq!(command_label(&command, &None).as_ref(), "打开");

        let missing = SharedString::from("rgpui_test::Missing");
        assert_eq!(command_label(&missing, &None), missing);
    }

    /// 工具栏零交互代码可绘制。
    #[rgpui::test]
    fn renders(cx: &mut crate::TestAppContext) {
        struct Probe;
        impl Render for Probe {
            fn render(
                &mut self,
                _window: &mut Window,
                _cx: &mut crate::Context<Self>,
            ) -> impl IntoElement {
                QuickAccessToolbar::new()
                    .item(QatItem::button("rgpui_test::Open"))
                    .item(QatItem::separator())
            }
        }

        let (_view, cx) = cx.add_window_view(|_window, _cx| Probe);
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    }
}
