//! keymap 演示：`load_keymap_json` 加载应用 + `HotkeyInput` 录制 + 回显 + 冲突提示。
//!
//! 设置页 recipe 落地（M7）：录制 → 绑定 → `bindings_for_action` 回显 →
//! 同键不同动作即冲突提示。JSON 编辑框预置一条碰撞条目，加载即见提示。

#![cfg_attr(target_family = "wasm", no_main)]

use std::collections::HashMap;

use rgpui::{
    App, Bounds, Context, Render, Window, WindowBounds, WindowOptions,
    components::{HotkeyInput, HotkeyInputState},
    div, h_flex,
    input_ui::{InputState, TextArea},
    prelude::*,
    px, size, v_flex,
};
use rgpui_platform::application;

const SAMPLE_JSON: &str = r#"[
    // 改键示例：Ctrl+S 存盘（input::Undo 只是演示占位，真应用换存盘动作）。
    { "key": "ctrl-s", "action": "input::Undo", "context": "Input" },
    // 碰撞示例：同键不同动作 → 冲突提示。
    { "key": "ctrl-s", "action": "input::Copy", "context": "Input" },
    // 禁用示例：Ctrl+K 发 NoAction。
    { "key": "ctrl-k", "action": null },
]"#;

struct KeymapDemo {
    json: rgpui::Entity<InputState>,
    hotkey: rgpui::Entity<HotkeyInputState>,
    status: String,
    recorded: String,
}

impl KeymapDemo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let json = cx.new(|cx| {
            let mut state = InputState::new(window, cx).multi_line(true).rows(8);
            state.set_value(SAMPLE_JSON, window, cx);
            state
        });
        let hotkey = cx.new(HotkeyInputState::new);
        Self {
            json,
            hotkey,
            status: "未加载".to_string(),
            recorded: "未录制".to_string(),
        }
    }

    /// 加载并应用（冲突 = 同键不同动作名，提示但不阻止）。
    fn apply(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let text = self.json.read_with(cx, |state, _| state.text().to_string());
        match rgpui::load_keymap_json(cx, &text) {
            Ok(bindings) => {
                let count = bindings.len();
                let mut by_key: HashMap<String, Vec<&str>> = HashMap::new();
                for binding in &bindings {
                    let key = binding
                        .keystrokes()
                        .iter()
                        .map(|k| k.unparse())
                        .collect::<Vec<_>>()
                        .join(" ");
                    by_key.entry(key).or_default().push(binding.action().name());
                }
                let mut conflicts: Vec<String> = by_key
                    .iter()
                    .filter(|(_, actions)| {
                        let mut unique = (*actions).clone();
                        unique.sort_unstable();
                        unique.dedup();
                        unique.len() > 1
                    })
                    .map(|(key, actions)| format!("{key} ← {}", actions.join(" / ")))
                    .collect();
                conflicts.sort();
                cx.bind_keys(bindings);
                self.status = if conflicts.is_empty() {
                    format!("已应用 {count} 条，无冲突")
                } else {
                    format!(
                        "已应用 {count} 条，冲突 {} 处：{}",
                        conflicts.len(),
                        conflicts.join("；")
                    )
                };
            }
            Err(error) => {
                self.status = format!("加载失败：{error}");
            }
        }
        cx.notify();
    }

    /// 录制值转按键串（`HotkeyValue` 存解析形 key，直拼即可）。
    fn recorded_keystroke(&self, cx: &mut App) -> Option<String> {
        self.hotkey.read_with(cx, |state, _| {
            state.hotkey().map(|hk| {
                let mut parts = Vec::new();
                if hk.modifiers.control {
                    parts.push("ctrl".to_string());
                }
                if hk.modifiers.alt {
                    parts.push("alt".to_string());
                }
                if hk.modifiers.shift {
                    parts.push("shift".to_string());
                }
                if hk.modifiers.platform {
                    parts.push("secondary".to_string());
                }
                parts.push(hk.key.clone());
                parts.join("-")
            })
        })
    }

    /// 把录制键绑到 `input::Copy`（演示 `bind_keys` 增量路径）。
    fn bind_recorded(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = self.recorded_keystroke(cx) else {
            self.recorded = "先点输入框录制一个按键".to_string();
            cx.notify();
            return;
        };
        match rgpui::KeyBinding::load(
            &key,
            Box::new(rgpui::input_ui::Copy),
            None,
            false,
            None,
            &rgpui::DummyKeyboardMapper,
        ) {
            Ok(binding) => {
                cx.bind_keys([binding]);
                self.recorded = format!("已绑 {key} → input::Copy");
            }
            Err(error) => {
                self.recorded = format!("按键串非法（{key}）：{error}");
            }
        }
        cx.notify();
    }
}

impl Render for KeymapDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let demo = cx.entity();
        // 回显：input::Undo 当前绑定（最后一条优先，引擎语义）。
        let echo = window
            .bindings_for_action(&rgpui::input_ui::Undo)
            .iter()
            .map(|b| {
                b.keystrokes()
                    .iter()
                    .map(|k| k.unparse())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join(" ｜ ");
        let echo_text = if echo.is_empty() {
            "input::Undo：无绑定".to_string()
        } else {
            format!("input::Undo 回显：{echo}")
        };
        let recorded_text = self.recorded.clone();
        let status = self.status.clone();
        let demo_for_record = demo.clone();
        v_flex()
            .size_full()
            .gap(px(8.0))
            .p(px(12.0))
            .child(div().text_sm().child("keymap.json（注释/尾逗号可写）"))
            .child(TextArea::new(&self.json).flex_none().h(px(180.0)))
            .child(
                h_flex()
                    .gap(px(8.0))
                    .items_center()
                    .child({
                        let demo = demo.clone();
                        div()
                            .id("keymap-apply")
                            .px(px(10.0))
                            .py(px(4.0))
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .child("加载并应用")
                            .on_click(move |_, window, cx| {
                                demo.update(cx, |this, cx| this.apply(window, cx));
                            })
                    })
                    .child(div().text_xs().child(status)),
            )
            .child(div().text_sm().child("录制（点框后按键）"))
            .child(
                h_flex()
                    .gap(px(8.0))
                    .items_center()
                    .child(
                        HotkeyInput::new(self.hotkey.clone())
                            .placeholder("点这里，然后按键")
                            .w(px(220.0))
                            .on_change(move |hotkey, _, cx| {
                                demo_for_record.update(cx, |this, cx| {
                                    this.recorded = hotkey
                                        .map(|hk| hk.format_display())
                                        .unwrap_or("已清空".to_string());
                                    cx.notify();
                                });
                            }),
                    )
                    .child({
                        let demo = demo.clone();
                        div()
                            .id("keymap-bind-recorded")
                            .px(px(10.0))
                            .py(px(4.0))
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .child("绑录制键到 input::Copy")
                            .on_click(move |_, window, cx| {
                                demo.update(cx, |this, cx| this.bind_recorded(window, cx));
                            })
                    })
                    .child(div().text_xs().child(recorded_text)),
            )
            .child(div().text_xs().child(echo_text))
            .child(
                div()
                    .text_xs()
                    .child("约束：JSON 是改键语义（文件列=用户层全集）；未知动作报错；谓词原样透传；null=禁用。"),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(720.0), px(560.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| KeymapDemo::new(window, cx)),
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
