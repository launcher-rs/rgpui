//! 侧边栏示例。
//!
//! 演示 `Sidebar`：导航切换、折叠为图标栏、角标、条目级自定义选中颜色，
//! 以及亮色 / 暗色主题下侧栏 token（`sidebar_accent` 系列）与通用 `accent` 的对比。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Button, ButtonVariants as _, Colorize as _, Context, Hsla, IconName, Render,
    SharedString, Theme, ThemeMode, Window, WindowBounds, WindowOptions, div, h_flex, prelude::*,
    px, rgb, size, v_flex,
};
use rgpui_platform::application;

/// 侧边栏演示视图状态。
struct SidebarDemo {
    /// 当前选中的条目 ID。
    selected: SharedString,
    /// 是否折叠为图标栏。
    collapsed: bool,
}

impl SidebarDemo {
    /// 创建演示状态。
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            selected: "home".into(),
            collapsed: false,
        }
    }

    /// 当前选中条目对应的内容区标题。
    fn content_title(&self) -> &str {
        match self.selected.as_ref() {
            "home" => "首页",
            "inbox" => "收件箱",
            "search" => "搜索",
            "alerts" => "通知（自定义选中色）",
            "settings" => "设置",
            _ => "未知",
        }
    }

    /// 切换全局主题模式（亮色 / 暗色）。
    fn switch_mode(&mut self, mode: ThemeMode, cx: &mut Context<Self>) {
        Theme::change(mode, None, cx);
        cx.refresh_windows();
    }
}

impl Render for SidebarDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected.clone();
        let collapsed = self.collapsed;
        let title = self.content_title().to_string();
        let theme = cx.theme();
        let is_dark = theme.is_dark();

        h_flex()
            .size_full()
            .child(
                Sidebar::new()
                    .item(SidebarItem::new("home", "首页").with_icon(IconName::LayoutDashboard))
                    .item(
                        SidebarItem::new("inbox", "收件箱")
                            .with_icon(IconName::Inbox)
                            .with_badge("12"),
                    )
                    .item(SidebarItem::new("search", "搜索").with_icon(IconName::Search))
                    .item(
                        SidebarItem::new("alerts", "通知")
                            .with_icon(IconName::Bell)
                            .with_badge("3")
                            // 条目级自定义选中颜色：覆盖主题 sidebar_accent 系列，仅作用于本条目。
                            .with_selected_background(rgb(0xfee2e2))
                            .with_selected_foreground(rgb(0xb91c1c)),
                    )
                    .item(SidebarItem::new("settings", "设置").with_icon(IconName::Settings))
                    .selected(selected)
                    .collapsible(true)
                    .collapsed(collapsed)
                    .on_select(cx.listener(|this, id: &SharedString, _, _| {
                        this.selected = id.clone();
                    }))
                    .on_toggle_collapsed(cx.listener(|this, collapsed: &bool, _, _| {
                        this.collapsed = *collapsed;
                    })),
            )
            .child(
                v_flex()
                    .flex_1()
                    .size_full()
                    .gap(px(12.0))
                    .p(px(24.0))
                    .bg(cx.theme().tokens.background)
                    .child(div().text_xl().child(format!("当前页面：{title}")))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().tokens.muted_foreground)
                            .child("点击左侧导航切换；点底部按钮折叠为图标栏。"),
                    )
                    // 主题切换：对比亮色 / 暗色下侧栏选中态的实际效果。
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .items_center()
                            .child(div().text_sm().child("主题："))
                            .child(
                                Button::new("theme-light")
                                    .label("亮色")
                                    .icon(IconName::Sun)
                                    .when(is_dark, |b| b.ghost())
                                    .when(!is_dark, |b| b.primary())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.switch_mode(ThemeMode::Light, cx);
                                    })),
                            )
                            .child(
                                Button::new("theme-dark")
                                    .label("暗色")
                                    .icon(IconName::Moon)
                                    .when(is_dark, |b| b.primary())
                                    .when(!is_dark, |b| b.ghost())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.switch_mode(ThemeMode::Dark, cx);
                                    })),
                            ),
                    )
                    // 主题色板：侧栏专用 token vs 通用 accent，直观展示选中态用色来源。
                    .child(div().text_sm().child("侧栏选中态用色（当前主题）："))
                    .child(theme_swatches(cx))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().tokens.muted_foreground)
                            .child("“通知”条目演示了条目级自定义：with_selected_background / with_selected_foreground。"),
                    ),
            )
    }
}

/// 渲染当前主题的侧栏 / 通用强调色色板（含十六进制值）。
fn theme_swatches(cx: &Context<SidebarDemo>) -> impl IntoElement {
    /// 单个色块。
    fn swatch(name: &str, color: Hsla, cx: &Context<SidebarDemo>) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .gap(px(6.0))
            .child(
                div()
                    .size(px(56.0))
                    .rounded(px(8.0))
                    .bg(color)
                    .border_1()
                    .border_color(theme.tokens.border),
            )
            .child(div().text_xs().child(name.to_string()))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.tokens.muted_foreground)
                    .child(color.to_hex()),
            )
    }

    let theme = cx.theme();
    h_flex()
        .gap(px(12.0))
        .flex_wrap()
        .child(swatch("侧栏选中底", theme.tokens.sidebar_accent.color, cx))
        .child(swatch(
            "侧栏选中字",
            theme.tokens.sidebar_accent_foreground.color,
            cx,
        ))
        .child(swatch("通用强调底", theme.tokens.accent.color, cx))
        .child(swatch(
            "通用强调字",
            theme.tokens.accent_foreground.color,
            cx,
        ))
}

/// 启动示例应用。
fn run_example() {
    application().run(|cx: &mut App| {
        // 初始化主题系统（亮色 / 暗色切换依赖主题注册表）。
        rgpui::theme::init(cx);

        let bounds = Bounds::centered(None, size(px(880.0), px(560.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| SidebarDemo::new(window, cx)),
        )
        .unwrap();
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
