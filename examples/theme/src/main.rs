//! 主题切换示例
//!
//! 此示例演示 rgpui 主题系统的使用方式：
//! - 亮色 / 暗色模式切换
//! - 从 `ThemeRegistry` 读取主题列表并应用任意主题
//! - 展示当前主题的关键颜色色板
//!
//! 运行：
//! ```text
//! cargo run -p rgpui --example theme
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::prelude::*;
use rgpui::{
    ActiveTheme, App, Button, ButtonVariants as _, Context, DropdownMenu, Hsla, IconName,
    IntoElement, ParentElement, PopupMenuItem, Render, Styled, Theme, ThemeMode, ThemeRegistry,
    Window, WindowOptions, div, h_flex, px, v_flex,
};
use rgpui_platform::application;

/// 主题示例视图状态。
struct ThemeExample {
    /// 当前选中的主题名称（跟随全局主题）。
    _active_theme: String,
}

impl ThemeExample {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            _active_theme: String::new(),
        }
    }

    /// 切换全局主题模式（亮色 / 暗色）。
    fn switch_mode(&mut self, mode: ThemeMode, cx: &mut Context<Self>) {
        Theme::change(mode, None, cx);
        cx.refresh_windows();
    }
}

impl Render for ThemeExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let current_name = theme.theme_name().clone();

        v_flex()
            .id("theme-example")
            .size_full()
            .p(px(32.0))
            .gap(px(24.0))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(div().text_2xl().font_semibold().child("主题系统示例"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(format!("当前主题：{}", current_name)),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(12.0))
                    .child(div().text_base().font_medium().child("模式切换"))
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
                    .gap(px(12.0))
                    .child(div().text_base().font_medium().child("主题列表"))
                    .child(theme_selector(cx)),
            )
            .child(
                v_flex()
                    .gap(px(12.0))
                    .child(div().text_base().font_medium().child("颜色色板"))
                    .child(color_palette(cx)),
            )
    }
}

/// 主题选择下拉菜单，列出 `ThemeRegistry` 中的所有主题。
fn theme_selector(cx: &Context<ThemeExample>) -> impl IntoElement {
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
        .label(current_name.clone())
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
fn color_palette(cx: &Context<ThemeExample>) -> impl IntoElement {
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
                        .size(px(56.0))
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

fn run_example() {
    application().run(|cx: &mut App| {
        // 初始化主题系统：注册默认主题并加载亮色模式。
        rgpui::theme::init(cx);
        // 初始化菜单系统：主题下拉菜单（Popover/PopupMenu）依赖
        // 菜单全局状态（GlobalState），缺失会在打开下拉菜单时 panic。
        rgpui::menu::init(cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(rgpui::WindowBounds::centered(
                    rgpui::size(px(720.), px(640.)),
                    cx,
                )),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| ThemeExample::new(window, cx)),
        )
        .ok();

        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
