//! 搜索实战演示（#14，提前合入的 1.2 内容）：SearchPanelState 嵌入 + 跳转 +
//! 匹配标黄（装饰 API）+ 只读滚动 + 输入防抖。

#![cfg_attr(target_family = "wasm", no_main)]

use futures::StreamExt as _;
use rgpui::{
    App, Bounds, Context, HighlightStyle, Render, Window, WindowBounds, WindowOptions,
    components::SearchPanelState,
    div, h_flex,
    input_ui::{Input, InputEvent, InputState, TextDecoration, TextDecorationCollection},
    prelude::*,
    px, size,
    util::debounce::Debouncer,
    v_flex, yellow,
};
use rgpui_platform::application;
use std::time::Duration;

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
    query_input: rgpui::Entity<InputState>,
    debouncer: Debouncer,
    debounced_hits: usize,
    last_query: String,
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

        // 防抖演示：查询输入 300ms 无新输入才统计一次。
        let query_input = cx.new(|cx| InputState::new(window, cx).placeholder("防抖输入…"));
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<String>();
        cx.subscribe(&query_input, move |this, input, event, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            let query = input.read(cx).text().to_string();
            let tx = tx.clone();
            this.debouncer.debounce(
                cx.background_executor(),
                Duration::from_millis(300),
                move || {
                    let _ = tx.unbounded_send(query);
                },
            );
        })
        .detach();
        cx.spawn(async move |this, cx| {
            while let Some(query) = rx.next().await {
                let _ = this.update(cx, |state: &mut SearchDemo, cx| {
                    state.debounced_hits += 1;
                    state.last_query = query;
                    cx.notify();
                });
            }
        })
        .detach();

        Self {
            text,
            panel,
            query_input,
            debouncer: Debouncer::new(),
            debounced_hits: 0,
            last_query: String::new(),
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
        let hits = self.debounced_hits;
        let last_query = self.last_query.clone();
        h_flex()
            .size_full()
            .gap(px(12.0))
            .p(px(12.0))
            .child(
                v_flex()
                    .flex_1()
                    .gap(px(8.0))
                    .child(div().text_sm().child("待搜索全文（导航跳转 + 只读滚动）"))
                    .child(div().flex_1().child(Input::new(&self.text).h_full()))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .child(
                                rgpui::Button::new("search-highlight-all")
                                    .label("标黄全部匹配")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.highlight_all(cx);
                                    })),
                            )
                            .child(Input::new(&self.query_input).w(px(200.0)))
                            .child(
                                div()
                                    .text_sm()
                                    .child(format!("防抖触发 {hits} 次：{last_query}")),
                            ),
                    ),
            )
            .child(div().w(px(360.0)).child(self.panel.clone()))
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
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
