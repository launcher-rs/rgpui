//! 标签页（Tabs）示例。
//!
//! - **静态 Tabs（值驱动，无实体）**：`Tabs::new(items).active(id).on_change(...)`，
//!   选中项是 `Option<SharedString>`，回调用 `cx.listener_value` 按值接收。
//! - **可拖拽排序的标签**：`TabDragState`（实体）+ `TabDragDrop`，拖动重排、可关闭。
//!
//! 运行：
//!
//! ```text
//! cargo run -p showcase --bin tabs
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::tabs::tab_drag::{TabDragDrop, TabDragState, TabItem};
use rgpui::tabs::{Tabs, TabsItem};
use rgpui::{
    App, Bounds, Context, Entity, IntoElement, ParentElement, Render, SharedString, Window,
    WindowBounds, WindowOptions, div, h_flex, prelude::*, px, rgb, size, v_flex,
};
use rgpui_platform::application;

/// 示例根视图。
struct TabsDemo {
    /// 静态 Tabs 当前选中项。
    active: Option<SharedString>,
    /// 可拖拽标签状态。
    drag_state: Entity<TabDragState>,
}

impl TabsDemo {
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let drag_state = cx.new(|_| TabDragState {
            enabled: true,
            tabs: vec![
                TabItem {
                    title: "main.rs".to_string(),
                    id: "t1".to_string(),
                    closable: true,
                },
                TabItem {
                    title: "lib.rs".to_string(),
                    id: "t2".to_string(),
                    closable: true,
                },
                TabItem {
                    title: "mod.rs".to_string(),
                    id: "t3".to_string(),
                    closable: false,
                },
                TabItem {
                    title: "utils.rs".to_string(),
                    id: "t4".to_string(),
                    closable: true,
                },
            ],
            ..Default::default()
        });
        Self {
            active: Some("home".into()),
            drag_state,
        }
    }
}

impl Render for TabsDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab_count = self.drag_state.read(cx).tabs.len();

        v_flex()
            .id("tabs-demo")
            .size_full()
            .gap(px(20.0))
            .p(px(24.0))
            .bg(rgb(0xffffff))
            .child(div().text_2xl().child("标签页（Tabs）"))
            // 1. 静态 Tabs。
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(section_title("1. 静态 Tabs（值驱动）"))
                    .child(
                        Tabs::new(vec![
                            TabsItem::new("home", "首页"),
                            TabsItem::new("settings", "设置"),
                            TabsItem::new("export", "导出").disabled(true),
                        ])
                        .active(self.active.clone())
                        .on_change(cx.listener_value(
                            |this, id: SharedString, _, cx| {
                                this.active = Some(id);
                                cx.notify();
                            },
                        )),
                    )
                    .child(div().text_sm().child(format!(
                        "当前选中：{}",
                        self.active.as_deref().unwrap_or("无")
                    ))),
            )
            // 2. 可拖拽排序的标签。
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(section_title("2. 可拖拽排序的标签（TabDragDrop）"))
                    .child(TabDragDrop::new(self.drag_state.clone()).on_reorder(
                        |_tabs, _window, _cx| {
                            println!("Tab 重新排序完成");
                        },
                    ))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .id("tabs-move-left")
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .rounded_md()
                                    .bg(rgb(0xf0f0f0))
                                    .cursor_pointer()
                                    .child("左移第一个可关闭项")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let state = this.drag_state.read(cx);
                                        if let Some(first) =
                                            state.tabs.iter().position(|t| t.closable)
                                            && first > 0
                                        {
                                            this.drag_state
                                                .update(cx, |s, _| s.move_tab(first, first - 1));
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(div().text_sm().child(format!("Tab 数量：{tab_count}")))
                            .child(div().text_sm().child("拖动 Tab 可排序，蓝色线指示落点")),
                    ),
            )
    }
}

/// 小标题。
fn section_title(text: &str) -> impl IntoElement {
    div().text_lg().child(text.to_string())
}

/// 启动示例。
fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(720.0), px(420.0)), cx);
        let _ = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| TabsDemo::new(window, cx)),
        );
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
