//! 1.3 新 API 集中演示（二进制）。
//!
//! - 静态 `Tabs`（值驱动 + `on_change(SharedString)` 按值 + `listener_value`）
//! - `DialogId`（开两框，按标识关下层，上层不受影响）
//! - 树 `TreeEvent::{Selected, Confirmed}`（点击选中 / 回车确认）
//! - `i18n`（全局管理器 + `I18nText::translate_global` + 语言切换）
//! - C1 新签名（`Checkbox(bool)` / `Select(usize, SharedString)` 按值）
//!
//! 运行：
//!
//! ```text
//! cargo run -p v1_3_showcase
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use std::collections::HashMap;

use rgpui::components::Select;
use rgpui::i18n::{I18nManager, I18nText};
use rgpui::tabs::{Tabs, TabsItem};
use rgpui::tree::{TreeEvent, TreeItem, TreeState};
use rgpui::{
    App, Bounds, Button, Checkbox, Context, DialogId, Entity, IntoElement, ParentElement, Render,
    Root, SharedString, Window, WindowBounds, WindowOptions, div, h_flex, prelude::*, px, rgb,
    size, v_flex,
};
use rgpui_platform::application;

/// 1.3 演示根视图（各区本地状态 + 树实体 + 对话框标识）。
struct V13Demo {
    /// 静态页签选中 id。
    tabs_active: Option<SharedString>,
    /// 对话框 A 标识（打开后有值）。
    dialog_a: Option<DialogId>,
    /// 对话框 B 标识（打开后有值）。
    dialog_b: Option<DialogId>,
    /// 对话框区回显。
    dialog_msg: String,
    /// 树状态实体。
    tree: Entity<TreeState>,
    /// 树事件回显。
    tree_msg: String,
    /// 复选框状态（C1 按值回写）。
    notify: bool,
    /// 下拉选中回显。
    fruit_msg: String,
}

impl V13Demo {
    /// 创建视图：组装树实体并订阅选中/确认事件。
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let tree = cx.new(|cx| {
            TreeState::new(cx).items(vec![
                TreeItem::new("src", "src")
                    .child(TreeItem::new("src/lib.rs", "lib.rs"))
                    .child(TreeItem::new("src/main.rs", "main.rs")),
                TreeItem::new("Cargo.toml", "Cargo.toml"),
            ])
        });
        cx.subscribe(&tree, |this, _state, event: &TreeEvent, cx| match event {
            TreeEvent::Selected(id) => {
                this.tree_msg = format!("点击选中：{id}");
                cx.notify();
            }
            TreeEvent::Confirmed(id) => {
                this.tree_msg = format!("回车确认：{id}");
                cx.notify();
            }
            _ => {}
        })
        .detach();
        Self {
            tabs_active: Some("home".into()),
            dialog_a: None,
            dialog_b: None,
            dialog_msg: "尚未打开对话框".to_string(),
            tree,
            tree_msg: "点击树行选中，回车确认文件行".to_string(),
            notify: true,
            fruit_msg: "尚未选择".to_string(),
        }
    }

    /// 打开对话框 A（记录标识供按标识关闭）。
    fn open_dialog_a(view: Entity<Self>, window: &mut Window, cx: &mut App) {
        use rgpui::WindowExt as _;
        let id = window.open_dialog(cx, |dialog, _, _| {
            dialog
                .title("对话框 A（下层）")
                .content(|content, _, _| {
                    content.child(div().child("先开的下层框：按标识关我，上层 B 不受影响。"))
                })
                .width(px(360.0))
        });
        view.update(cx, |this, cx| {
            this.dialog_a = Some(id);
            this.dialog_msg = "已打开 A（下层）".to_string();
            cx.notify();
        });
    }

    /// 打开对话框 B（后开的上层）。
    fn open_dialog_b(view: Entity<Self>, window: &mut Window, cx: &mut App) {
        use rgpui::WindowExt as _;
        let id = window.open_dialog(cx, |dialog, _, _| {
            dialog
                .title("对话框 B（上层）")
                .content(|content, _, _| {
                    content.child(div().child("后开的上层框：关下层 A 时我不受影响。"))
                })
                .width(px(360.0))
        });
        view.update(cx, |this, cx| {
            this.dialog_b = Some(id);
            this.dialog_msg = "已打开 B（上层）".to_string();
            cx.notify();
        });
    }
}

impl Render for V13Demo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        v_flex()
            .id("v13-demo")
            .size_full()
            .gap(px(16.0))
            .p(px(24.0))
            .overflow_scroll()
            .bg(rgb(0xffffff))
            .child(div().text_2xl().child("1.3 新 API 集中演示"))
            // 1. 静态 Tabs（值驱动 + listener_value 按值接入）。
            .child(section(
                "1. 静态 Tabs（无实体）",
                v_flex()
                    .gap(px(8.0))
                    .child(
                        Tabs::new(vec![
                            TabsItem::new("home", "首页"),
                            TabsItem::new("settings", "设置"),
                            TabsItem::new("export", "导出").disabled(true),
                        ])
                        .active(self.tabs_active.clone())
                        .on_change(cx.listener_value(
                            |this, id: SharedString, _, cx| {
                                this.tabs_active = Some(id);
                                cx.notify();
                            },
                        )),
                    )
                    .child(div().text_sm().child(format!(
                        "当前选中：{}",
                        self.tabs_active.as_deref().unwrap_or("无")
                    )))
                    .into_any_element(),
            ))
            // 2. DialogId（按标识关闭下层）。
            .child(section("2. 对话框标识（DialogId）", {
                let dialog_a = self.dialog_a;
                v_flex()
                    .gap(px(8.0))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .child(Button::new("v13-open-a").label("打开 A（下层）").on_click({
                                let view = view.clone();
                                move |_, window, cx| {
                                    Self::open_dialog_a(view.clone(), window, cx);
                                }
                            }))
                            .child(Button::new("v13-open-b").label("打开 B（上层）").on_click({
                                let view = view.clone();
                                move |_, window, cx| {
                                    Self::open_dialog_b(view.clone(), window, cx);
                                }
                            }))
                            .child(Button::new("v13-close-a").label("按标识关闭 A").on_click({
                                let view = view.clone();
                                move |_, window, cx| {
                                    use rgpui::WindowExt as _;
                                    let closed = dialog_a
                                        .map(|id| window.close_dialog_by(cx, id))
                                        .unwrap_or(false);
                                    view.update(cx, |demo, cx| {
                                        if closed {
                                            demo.dialog_a = None;
                                            let b = if demo.dialog_b.is_some() {
                                                "B 仍在"
                                            } else {
                                                "B 未开"
                                            };
                                            demo.dialog_msg =
                                                format!("已按标识关闭 A（{b}，上层不受影响）");
                                        } else {
                                            demo.dialog_msg =
                                                "A 未打开或标识未知（返回 false）".to_string();
                                        }
                                        cx.notify();
                                    });
                                }
                            })),
                    )
                    .child(div().text_sm().child(self.dialog_msg.clone()))
                    .into_any_element()
            }))
            // 3. 树 Selected / Confirmed。
            .child(section(
                "3. 树事件（点击选中 / 回车确认）",
                v_flex()
                    .gap(px(8.0))
                    .child(rgpui::tree::tree(
                        &self.tree,
                        |ix, entry, _selected, _, _| {
                            rgpui::list::ListItem::new(("v13-tree", ix))
                                .child(entry.item().label.clone())
                        },
                    ))
                    .child(div().text_sm().child(self.tree_msg.clone()))
                    .into_any_element(),
            ))
            // 4. i18n（全局管理器 + translate_global）。
            .child(section("4. 国际化（全局管理器）", {
                v_flex()
                    .gap(px(8.0))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .child(Button::new("v13-locale-zh").label("中文").on_click(
                                cx.listener(|_, _, _, cx| {
                                    cx.update_global::<I18nManager, _>(|m, _| {
                                        m.set_locale("zh-CN");
                                    });
                                }),
                            ))
                            .child(Button::new("v13-locale-en").label("English").on_click(
                                cx.listener(|_, _, _, cx| {
                                    cx.update_global::<I18nManager, _>(|m, _| {
                                        m.set_locale("en");
                                    });
                                }),
                            )),
                    )
                    .child(
                        div().text_sm().child(
                            I18nText::new("hello")
                                .with_arg("name", "1.3")
                                .translate_global(cx),
                        ),
                    )
                    .into_any_element()
            }))
            // 5. C1 新签名（值按值传递）。
            .child(section("5. 回调新签名（按值）", {
                let view_fruit = view.clone();
                v_flex()
                    .gap(px(8.0))
                    .child(
                        Checkbox::new("v13-notify")
                            .label("通知我")
                            .checked(self.notify)
                            .on_change(cx.listener_value(|this, checked: bool, _, cx| {
                                this.notify = checked;
                                cx.notify();
                            })),
                    )
                    .child(
                        // 双值回调（索引 + 文本）用实体捕获直连，无需 listener。
                        Select::new(vec!["苹果".into(), "香蕉".into(), "樱桃".into()])
                            .placeholder("选水果")
                            .on_change(move |ix: usize, label: SharedString, _, cx| {
                                view_fruit.update(cx, |this, cx| {
                                    this.fruit_msg = format!("选中 #{ix}：{label}");
                                    cx.notify();
                                });
                            }),
                    )
                    .child(div().text_sm().child(format!(
                        "复选：{}；{}",
                        if self.notify { "开" } else { "关" },
                        self.fruit_msg,
                    )))
                    .into_any_element()
            }))
            // 对话框层挂载。
            .when_some(Root::render_dialog_layer(window, cx), |this, layer| {
                this.child(layer)
            })
    }
}

/// 章节容器辅助函数。
fn section(title: &str, body: impl IntoElement) -> impl IntoElement {
    v_flex()
        .gap(px(8.0))
        .p(px(12.0))
        .rounded_md()
        .border_1()
        .border_color(rgb(0xe0e0e0))
        .child(div().text_lg().child(title.to_string()))
        .child(body)
}

/// 启动示例窗口：初始化树键绑定 + i18n 全局管理器。
fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::tree::init(cx);
        let mut i18n = I18nManager::new("zh-CN");
        i18n.load_translations_map(
            "zh-CN",
            HashMap::from([("hello".to_string(), "你好，{name}！".to_string())]),
        );
        i18n.load_translations_map(
            "en",
            HashMap::from([("hello".to_string(), "Hello, {name}!".to_string())]),
        );
        cx.set_global(i18n);

        let bounds = Bounds::centered(None, size(px(720.0), px(860.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| V13Demo::new(window, cx)),
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
/// WASM 入口：初始化 Web 环境后启动示例。
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
