//! 主题示例：模式切换（亮/暗）、主题列表选择与颜色色板预览。

use rgpui::prelude::*;
use rgpui::{
    ActiveTheme, Button, ButtonVariants, Context, DropdownMenu, Hsla, IconName, IntoElement,
    ParentElement, PopupMenuItem, Styled, Theme, ThemeMode, ThemeRegistry, Window, div, h_flex, px,
    v_flex,
};

use super::StoryItem;

/// 主题故事注册表。
pub fn stories() -> Vec<StoryItem> {
    vec![StoryItem {
        title: "主题",
        build: |window, cx| cx.new(|cx| ThemeStory::new(window, cx)).into(),
    }]
}

/// 主题示例视图。
struct ThemeStory;

impl ThemeStory {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }

    /// 切换全局主题模式（亮色 / 暗色）。
    fn switch_mode(&mut self, mode: ThemeMode, cx: &mut Context<Self>) {
        Theme::change(mode, None, cx);
        cx.refresh_windows();
    }
}

impl rgpui::Render for ThemeStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let current_name = theme.theme_name().clone();

        v_flex()
            .id("theme-story")
            .gap(px(16.0))
            .p(px(16.0))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(section_title("当前主题"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(format!("{}（{}）", current_name, mode_label(theme.mode))),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(section_title("模式切换"))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .child(
                                Button::new("switch-light")
                                    .label("亮色")
                                    .icon(IconName::Sun)
                                    .when(theme.is_dark(), |b| b.ghost())
                                    .when(!theme.is_dark(), |b| b.primary())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.switch_mode(ThemeMode::Light, cx);
                                    })),
                            )
                            .child(
                                Button::new("switch-dark")
                                    .label("暗色")
                                    .icon(IconName::Moon)
                                    .when(theme.is_dark(), |b| b.primary())
                                    .when(!theme.is_dark(), |b| b.ghost())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.switch_mode(ThemeMode::Dark, cx);
                                    })),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(section_title("主题列表"))
                    .child(theme_selector(cx)),
            )
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(section_title("颜色色板"))
                    .child(color_palette(cx)),
            )
    }
}

/// 返回主题模式的显示标签。
fn mode_label(mode: ThemeMode) -> &'static str {
    if mode.is_dark() { "暗色" } else { "亮色" }
}

/// 主题选择下拉菜单，列出 `ThemeRegistry` 中的所有主题。
fn theme_selector(cx: &Context<ThemeStory>) -> impl IntoElement {
    let current_name = cx.theme().theme_name().clone();

    // 提前收集主题元数据，克隆进 'static 菜单构建闭包。
    let items: Vec<(rgpui::SharedString, ThemeMode, bool)> = ThemeRegistry::global(cx)
        .sorted_themes()
        .into_iter()
        .map(|theme| {
            let name = theme.name.clone();
            (name.clone(), theme.mode, name == current_name)
        })
        .collect();

    Button::new("theme-list")
        .label(current_name)
        .icon(IconName::Palette)
        .dropdown_menu(move |menu, _, _| {
            let mut menu = menu.label("选择主题");
            for (name, mode, checked) in &items {
                let name = name.clone();
                let mode = *mode;
                menu = menu.item(PopupMenuItem::new(name.clone()).checked(*checked).on_click(
                    move |_, window, cx| {
                        Theme::change(mode, Some(window), cx);
                        cx.refresh_windows();
                    },
                ));
            }
            menu
        })
}

/// 渲染当前主题的关键颜色色板。
fn color_palette(cx: &Context<ThemeStory>) -> impl IntoElement {
    let theme = cx.theme();

    let colors: Vec<(&str, Hsla)> = vec![
        ("背景", theme.background),
        ("前景", theme.foreground),
        ("主色", theme.primary),
        ("主色悬停", theme.primary_hover),
        ("主色文字", theme.primary_foreground),
        ("次要", theme.secondary),
        ("边框", theme.border),
        ("输入框", theme.input),
        ("弱化", theme.muted),
        ("弱化文字", theme.muted_foreground),
        ("强调", theme.accent),
        ("强调文字", theme.accent_foreground),
        ("危险", theme.danger),
        ("成功", theme.success),
        ("警告", theme.warning),
        ("信息", theme.info),
    ];

    h_flex()
        .flex_wrap()
        .gap(px(12.0))
        .children(colors.into_iter().map(|(name, color)| {
            v_flex()
                .gap(px(6.0))
                .child(
                    div()
                        .size(px(48.0))
                        .rounded(px(8.0))
                        .bg(color)
                        .border_1()
                        .border_color(theme.border),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(name),
                )
        }))
}

/// 章节标题辅助函数。
fn section_title(text: impl Into<rgpui::SharedString> + IntoElement) -> impl IntoElement {
    div()
        .text_size(px(14.0))
        .text_color(rgpui::hsla(0.0, 0.0, 0.55, 1.0))
        .child(text)
}
