//! 侧边栏示例。
//!
//! 演示 `Sidebar`：导航切换、折叠为图标栏、与右侧内容区联动。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, IconName, Render, SharedString, Window, WindowBounds, WindowOptions, div,
    h_flex, prelude::*, px, rgb, size, v_flex,
};
use rgpui_platform::application;

struct SidebarDemo {
    selected: SharedString,
    collapsed: bool,
}

impl SidebarDemo {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            selected: "home".into(),
            collapsed: false,
        }
    }

    fn content_title(&self) -> &str {
        match self.selected.as_ref() {
            "home" => "首页",
            "search" => "搜索",
            "settings" => "设置",
            _ => "未知",
        }
    }
}

impl Render for SidebarDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected.clone();
        let collapsed = self.collapsed;
        let title = self.content_title().to_string();

        h_flex()
            .size_full()
            .child(
                Sidebar::new()
                    .item(SidebarItem::new("home", "首页").with_icon(IconName::LayoutDashboard))
                    .item(SidebarItem::new("search", "搜索").with_icon(IconName::Search))
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
                    .gap(px(8.0))
                    .p(px(24.0))
                    .bg(rgb(0xffffff))
                    .child(div().text_xl().child(format!("当前页面：{title}")))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x999999))
                            .child("点击左侧导航切换；点底部按钮折叠为图标栏。"),
                    ),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(720.0), px(480.0)), cx);
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
