//! 树（Tree）示例。
//!
//! 演示 `TreeState` 实体 + `tree` 视图：层级展开、点击选中、回车确认。
//! 通过订阅 `TreeEvent::{Selected, Confirmed}` 拿到选中/确认的节点 id。
//!
//! 运行：
//!
//! ```text
//! cargo run -p showcase --bin tree
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::tree::{TreeEvent, TreeItem, TreeState, tree};
use rgpui::{
    App, Bounds, Context, Entity, IntoElement, ParentElement, Render, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size, v_flex,
};
use rgpui_platform::application;

/// 示例根视图。
struct TreeDemo {
    /// 树状态实体。
    tree: Entity<TreeState>,
    /// 事件回显文本。
    message: String,
}

impl TreeDemo {
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let tree = cx.new(|cx| {
            TreeState::new(cx).items(vec![
                TreeItem::new("src", "src")
                    .child(TreeItem::new("src/lib.rs", "lib.rs"))
                    .child(TreeItem::new("src/main.rs", "main.rs"))
                    .child(
                        TreeItem::new("src/components", "components")
                            .child(TreeItem::new("src/components/button.rs", "button.rs")),
                    ),
                TreeItem::new("Cargo.toml", "Cargo.toml"),
                TreeItem::new("README.md", "README.md"),
            ])
        });
        // 订阅选中 / 确认事件。
        cx.subscribe(&tree, |this, _state, event: &TreeEvent, cx| match event {
            TreeEvent::Selected(id) => {
                this.message = format!("点击选中：{id}");
                cx.notify();
            }
            TreeEvent::Confirmed(id) => {
                this.message = format!("回车确认：{id}");
                cx.notify();
            }
            _ => {}
        })
        .detach();

        Self {
            tree,
            message: "点击树行选中，回车确认当前行".to_string(),
        }
    }
}

impl Render for TreeDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("tree-demo")
            .size_full()
            .gap(px(16.0))
            .p(px(24.0))
            .bg(rgb(0xffffff))
            .child(div().text_2xl().child("树（Tree）"))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .p(px(12.0))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0xe0e0e0))
                    .child(tree(&self.tree, |ix, entry, _selected, _window, _cx| {
                        rgpui::list::ListItem::new(("tree-demo", ix))
                            .child(entry.item().label.clone())
                    }))
                    .child(div().text_sm().child(self.message.clone())),
            )
    }
}

/// 启动示例：注册树的键绑定。
fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::tree::init(cx);
        let bounds = Bounds::centered(None, size(px(560.0), px(520.0)), cx);
        let _ = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| TreeDemo::new(window, cx)),
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
