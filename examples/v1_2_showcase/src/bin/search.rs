//! 搜索实战演示（#14，提前合入的 1.2 内容）：SearchPanelState 嵌入 + 跳转 +
//! 匹配标黄（装饰 API）+ 只读滚动。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, KeyBinding, Render, Window, WindowBounds, WindowOptions, blue,
    components::SearchPanelState,
    div, green, h_flex,
    input_ui::{
        Backspace, Copy, Cut, Delete, Enter, Escape, Input, InputState, MoveDown, MoveEnd,
        MoveHome, MoveLeft, MoveRight, MoveUp, Paste, Redo, SelectAll, Undo,
    },
    prelude::*,
    px, size, v_flex, white, yellow,
};
use rgpui_platform::application;

const SAMPLE: &str = "fn main() {\n    let text = \"hello rgpui\";\n    println!(\"{text}\");\n}\n\n// hello world\nfn render() {\n    draw(\"hello\");\n}\n";

/// 行列（字节列）转全文 UTF-8 字节偏移。
fn offset_of(text: &str, line: usize, col: usize) -> usize {
    let mut offset = 0;
    for (ix, part) in text.split('\n').enumerate() {
        if ix == line {
            return offset + col.min(part.len());
        }
        offset += part.len() + 1;
    }
    offset
}

struct SearchDemo {
    text: rgpui::Entity<InputState>,
    panel: rgpui::Entity<SearchPanelState>,
}

impl SearchDemo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text = cx.new(|cx| {
            let mut state = InputState::new(window, cx).multi_line(true);
            state.replace(SAMPLE, window, cx);
            state
        });

        // 一行接通搜索：文本同步 + 匹配标黄 + 默认跳转（选区 + 只读滚动）。
        // 导航回调未定制，attach 给默认实现；替换执行权仍在外部（见下）。
        let panel = cx.new(|cx| SearchPanelState::new(window, cx));
        panel.update(cx, |panel, cx| panel.attach_editor(&text, cx));

        // 替换接线：替换当前匹配 / 全部匹配（从后往前保偏移），文本变更经 attach 的
        // 订阅自动同步 source，无需手动处理。
        {
            let text = text.clone();
            let panel_handle = panel.clone();
            panel.update(cx, |panel, _| {
                panel.set_on_replace(move |_, replacement, window, cx| {
                    let full = text.read_with(cx, |state, _| state.text().to_string());
                    let range = panel_handle.read_with(cx, |panel, cx| {
                        panel.state().read(cx).current_match().map(|m| {
                            let base = offset_of(&full, m.line, 0);
                            base + m.start_col..base + m.end_col
                        })
                    });
                    if let Some(range) = range {
                        text.update(cx, |state, cx| {
                            state.set_selected_range(range, cx);
                            state.replace(replacement, window, cx);
                        });
                    }
                });
            });
        }
        {
            let text = text.clone();
            let panel_handle = panel.clone();
            panel.update(cx, |panel, _| {
                panel.set_on_replace_all(move |_, replacement, window, cx| {
                    let full = text.read_with(cx, |state, _| state.text().to_string());
                    let mut ranges: Vec<_> = panel_handle.read_with(cx, |panel, cx| {
                        panel
                            .state()
                            .read(cx)
                            .matches()
                            .iter()
                            .map(|m| {
                                let base = offset_of(&full, m.line, 0);
                                base + m.start_col..base + m.end_col
                            })
                            .collect()
                    });
                    // 从后往前替换，前面偏移不受影响。
                    ranges.sort_by_key(|range| std::cmp::Reverse(range.start));
                    text.update(cx, |state, cx| {
                        for range in ranges {
                            state.set_selected_range(range, cx);
                            state.replace(replacement.clone(), window, cx);
                        }
                    });
                });
            });
        }

        Self { text, panel }
    }
}

impl Render for SearchDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            .items_stretch()
            .gap(px(12.0))
            .p(px(12.0))
            .child(
                v_flex()
                    .flex_1()
                    .gap(px(8.0))
                    .child(div().text_sm().child("待搜索全文（导航跳转 + 只读滚动）"))
                    .child(Input::new(&self.text).flex_1()),
            )
            .child(
                v_flex()
                    .w(px(360.0))
                    .gap(px(8.0))
                    .child(self.panel.clone())
                    .child(div().text_xs().child("标黄配色（默认黄底）："))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .child(
                                rgpui::Button::new("hl-yellow")
                                    .label("黄")
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.panel.update(cx, |panel, cx| {
                                            panel.set_highlight_colors(yellow(), None, cx)
                                        });
                                    })),
                            )
                            .child(rgpui::Button::new("hl-green").label("绿").small().on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.panel.update(cx, |panel, cx| {
                                        panel.set_highlight_colors(green(), None, cx)
                                    });
                                }),
                            ))
                            .child(rgpui::Button::new("hl-blue").label("蓝").small().on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.panel.update(cx, |panel, cx| {
                                        panel.set_highlight_colors(blue(), Some(white()), cx)
                                    });
                                }),
                            )),
                    ),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        // 输入框编辑键位（应用级注册；secondary = macOS Cmd / 其他平台 Ctrl）。
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, None),
            KeyBinding::new("delete", Delete, None),
            KeyBinding::new("left", MoveLeft, None),
            KeyBinding::new("right", MoveRight, None),
            KeyBinding::new("up", MoveUp, None),
            KeyBinding::new("down", MoveDown, None),
            KeyBinding::new("home", MoveHome, None),
            KeyBinding::new("end", MoveEnd, None),
            KeyBinding::new(
                "enter",
                Enter {
                    secondary: false,
                    shift: false,
                },
                None,
            ),
            KeyBinding::new("escape", Escape, None),
            KeyBinding::new("secondary-a", SelectAll, None),
            KeyBinding::new("secondary-c", Copy, None),
            KeyBinding::new("secondary-x", Cut, None),
            KeyBinding::new("secondary-v", Paste, None),
            KeyBinding::new("secondary-z", Undo, None),
            KeyBinding::new("secondary-shift-z", Redo, None),
        ]);
        let bounds = Bounds::centered(None, size(px(980.0), px(640.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| SearchDemo::new(window, cx)),
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
