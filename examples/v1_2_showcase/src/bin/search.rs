//! 搜索实战演示（#14，提前合入的 1.2 内容）：SearchPanelState 嵌入 + 跳转 +
//! 匹配标黄（装饰 API）+ 只读滚动。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, HighlightStyle, KeyBinding, Render, Window, WindowBounds, WindowOptions,
    components::SearchPanelState,
    div, h_flex,
    input_ui::{
        Backspace, Copy, Cut, Delete, Enter, Escape, Input, InputEvent, InputState, MoveDown,
        MoveEnd, MoveHome, MoveLeft, MoveRight, MoveUp, Paste, Redo, SelectAll, TextDecoration,
        TextDecorationCollection, Undo,
    },
    prelude::*,
    px, size, v_flex, yellow,
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
    highlight: Option<TextDecorationCollection>,
}

impl SearchDemo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text = cx.new(|cx| {
            let mut state = InputState::new(window, cx).multi_line(true);
            state.replace(SAMPLE, window, cx);
            state
        });

        // 嵌入搜索面板：导航回调直接跳选区 + 只读滚动。
        let jump_text = text.clone();
        let panel = cx.new(|cx| {
            SearchPanelState::new(window, cx).on_navigate(move |line, start, end, _, cx| {
                let offset = jump_text.read_with(cx, |state, _| {
                    let full = state.text().to_string();
                    let base = offset_of(&full, line, 0);
                    (
                        offset_of(&full, line, start) - base,
                        offset_of(&full, line, end) - base,
                    )
                });
                // 上面只读了一次文本，这里重新算基址（演示从简，两次一致）。
                jump_text.update(cx, |state, cx| {
                    let full = state.text().to_string();
                    let base = offset_of(&full, line, 0);
                    let range = base + offset.0..base + offset.1;
                    state.set_selected_range(range.clone(), cx);
                    state.reveal_offset(range.start, cx);
                });
            })
        });
        panel.update(cx, |panel, cx| {
            panel.set_source(SAMPLE.to_string(), cx);
        });

        // 文本一改就同步面板 source（否则匹配/跳转/标黄按旧文本算，全错位），
        // 有标黄时顺带重标，保持搜/改/标三者一致。
        cx.subscribe(&text, move |this, _text, event, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            let full = this.text.read_with(cx, |state, _| state.text().to_string());
            this.panel.update(cx, |panel, cx| {
                panel.set_source(full, cx);
            });
            if this.highlight.is_some() {
                this.highlight_all(cx);
            }
        })
        .detach();

        // 替换接线：替换当前匹配 / 全部匹配（从后往前保偏移），文本变更经上面的
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

        Self {
            text,
            panel,
            highlight: None,
        }
    }

    /// 把当前全部匹配标黄（装饰 API）。
    fn highlight_all(&mut self, cx: &mut Context<Self>) {
        // 匹配经 panel.state() 读取（Entity<InputState>::read 需要 Context，按回调内聚拢写法改写）：
        let full = self.text.read_with(cx, |state, _| state.text().to_string());
        let matches = self.panel.read_with(cx, |panel, cx| {
            panel
                .state()
                .read(cx)
                .matches()
                .iter()
                .map(|m| {
                    let base = offset_of(&full, m.line, 0);
                    base + m.start_col..base + m.end_col
                })
                .map(|range| {
                    TextDecoration::new(
                        range,
                        HighlightStyle {
                            background_color: Some(yellow()),
                            ..Default::default()
                        },
                    )
                })
                .collect::<Vec<_>>()
        });
        if let Some(ref collection) = self.highlight {
            collection.set(matches, cx);
        } else {
            self.highlight = Some(self.text.update(cx, |state, cx| {
                state.create_decorations_collection(matches, cx)
            }));
        }
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
                    .child(
                        rgpui::Button::new("search-highlight-all")
                            .label("标黄全部匹配")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.highlight_all(cx);
                            })),
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
