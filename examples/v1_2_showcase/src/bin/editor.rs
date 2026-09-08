//! Editor 演示：`EditorState` + `Editor` 组件（P3b 迁移）。
//!
//! 主窗格走编辑器状态（多行 + 行号/折叠配置/大纲缓存/符号跳转/高亮接入内聚在
//! `EditorState` 里，渲染走 `Editor` 自带行号列号状态行）；只读窗格保留表单
//! `Input` 做对照。tree-sitter 高亮/折叠 + 行操作 + 多光标（键位来自全局
//! `init_all` 默认注册）。LSP 区接三个假 provider（补全/诊断/悬停，传输层
//! 由真应用实现后注入，见 `EditorState::set_*_provider`）。

#![cfg_attr(target_family = "wasm", no_main)]

use std::rc::Rc;

use rgpui::{
    App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div, h_flex,
    input_ui::{Editor, EditorState, InputEvent, InputState, TextArea},
    lsp::{
        CompletionProvider, DiagnosticEntry, DiagnosticsProvider, HoverContent, HoverProvider,
        HoverResponse,
    },
    prelude::*,
    px, rgb, size, v_flex,
};
use rgpui_platform::application;

const SAMPLE: &str = "fn main() {\n    let name = \"rgpui\";\n    println!(\"hello, {name}\");\n}\n\nstruct Point {\n    x: f32,\n    y: f32,\n}\n\nimpl Point {\n    fn len(&self) -> f32 {\n        (self.x * self.x + self.y * self.y).sqrt()\n    }\n}\n";

const READONLY_SAMPLE: &str = "只读预览：可选可复制，不可编辑。右键菜单的剪切/粘贴/撤销自动禁用。";

/// 演示用假补全 provider：固定词表（真 provider 由应用实现传输后注入）。
struct DemoCompletionProvider;

impl CompletionProvider for DemoCompletionProvider {
    fn completions(
        &self,
        _text: &rgpui::input_ui::Rope,
        _offset: usize,
        _trigger: lsp_types::CompletionContext,
        _window: &mut Window,
        _cx: &mut App,
    ) -> rgpui::Task<anyhow::Result<lsp_types::CompletionResponse>> {
        rgpui::Task::ready(Ok(lsp_types::CompletionResponse::Array(
            ["println", "print", "private", "pub", "Point"]
                .into_iter()
                .map(|label| lsp_types::CompletionItem {
                    label: label.to_string(),
                    kind: Some(lsp_types::CompletionItemKind::FUNCTION),
                    detail: Some("演示词条".to_string()),
                    ..Default::default()
                })
                .collect(),
        )))
    }
}

/// 演示用假诊断 provider：首行一个 warning（真 provider 按 URI 取数）。
struct DemoDiagnosticsProvider;

impl DiagnosticsProvider for DemoDiagnosticsProvider {
    fn diagnostics(
        &self,
        _uri: &lsp_types::Uri,
        _window: &mut Window,
        _cx: &mut App,
    ) -> rgpui::Task<anyhow::Result<Vec<DiagnosticEntry>>> {
        use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
        rgpui::Task::ready(Ok(vec![DiagnosticEntry::from_diagnostic(Diagnostic {
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 2),
            },
            severity: Some(DiagnosticSeverity::WARNING),
            message: "演示诊断：示例警告（假 provider）".to_string(),
            ..Default::default()
        })]))
    }
}

/// 演示用假悬停 provider：固定返回代码块 + 文本。
struct DemoHoverProvider;

impl HoverProvider for DemoHoverProvider {
    fn hover(
        &self,
        _text: &rgpui::input_ui::Rope,
        offset: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> rgpui::Task<anyhow::Result<Option<HoverResponse>>> {
        rgpui::Task::ready(Ok(Some(HoverResponse {
            range: offset..offset,
            contents: vec![
                HoverContent::CodeBlock {
                    language: "rust".to_string(),
                    code: "fn main()".to_string(),
                },
                HoverContent::Text("演示悬停：假 provider".to_string()),
            ],
        })))
    }
}

/// 演示用假 inlay provider：首行末尾一个类型提示。
struct DemoInlayProvider;

impl rgpui::input_ui::InlayProvider for DemoInlayProvider {
    fn inlay_hints(
        &self,
        text: &rgpui::input_ui::Rope,
        _visible: std::ops::Range<usize>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> rgpui::Task<anyhow::Result<Vec<rgpui::input_ui::InlayHint>>> {
        // 首行末尾（无 ropey 直接依赖，用 chars 数字节）。
        let end: usize = text
            .chars()
            .take_while(|c| *c != '\n')
            .map(|c| c.len_utf8())
            .sum();
        rgpui::Task::ready(Ok(vec![rgpui::input_ui::InlayHint {
            offset: end,
            text: ": demo".to_string(),
        }]))
    }
}

/// 演示用桩语言高亮器（注册表机制演示；真语言按三步加 grammar）。
struct DemoLangStub;

impl rgpui::highlight::Highlighter for DemoLangStub {
    fn language(&self) -> rgpui::SharedString {
        "demo".into()
    }

    fn update(
        &mut self,
        _edit: Option<rgpui::highlight::TextEdit>,
        _text: &rgpui::input_ui::Rope,
        _folding: bool,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn styles(
        &self,
        range: &std::ops::Range<usize>,
        _resolver: &dyn rgpui::highlight::HighlightStyleResolver,
    ) -> Vec<(std::ops::Range<usize>, rgpui::HighlightStyle)> {
        vec![(range.clone(), rgpui::HighlightStyle::default())]
    }

    fn fold_ranges(&self, _text: &rgpui::input_ui::Rope) -> Vec<rgpui::highlight::FoldRange> {
        Vec::new()
    }

    fn document_symbols(
        &self,
        _text: &rgpui::input_ui::Rope,
    ) -> Vec<rgpui::highlight::DocumentSymbol> {
        vec![rgpui::highlight::DocumentSymbol {
            kind: rgpui::highlight::SymbolKind::Function,
            name: "Demo".into(),
            range: 0..2,
            start_row: 0,
        }]
    }
}

/// 悬停内容首条预览（演示状态行用）。
fn hover_preview(state: &EditorState) -> String {
    let hover = state.hover_state();
    if !hover.visible {
        return "悬停：未请求".to_string();
    }
    let first = hover
        .response
        .as_ref()
        .and_then(|r| r.contents.first())
        .map(|c| match c {
            HoverContent::Text(s) | HoverContent::Markdown(s) => s.clone(),
            HoverContent::CodeBlock { code, .. } => code.clone(),
        })
        .unwrap_or_default();
    format!("悬停：{first}")
}

/// LSP 操作按钮（演示用样式；`action` 操作内部 `EditorState`）。
fn lsp_button(
    demo: &rgpui::Entity<EditorDemo>,
    id: &'static str,
    label: &'static str,
    action: impl Fn(&mut EditorState, &mut Window, &mut Context<EditorState>) + 'static,
) -> impl IntoElement {
    let demo = demo.clone();
    div()
        .id(id)
        .px(px(10.0))
        .py(px(4.0))
        .rounded_md()
        .cursor_pointer()
        .bg(rgb(0x000000).opacity(0.05))
        .hover(|this| this.bg(rgb(0x000000).opacity(0.1)))
        .text_xs()
        .child(label)
        .on_click(move |_, window, cx| {
            demo.update(cx, |this, cx| {
                this.editor.update(cx, |state, cx| {
                    action(state, window, cx);
                });
            });
        })
}

struct EditorDemo {
    editor: rgpui::Entity<EditorState>,
    readonly: rgpui::Entity<InputState>,
}

impl EditorDemo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 编辑器状态一次配好（多行 + 行号 + 折叠 + 键入体验 + 大纲订阅）。
        let editor = cx.new(|cx| EditorState::new(window, cx, SAMPLE));
        // 演示语言注册（注册表机制；真语言按三步加 grammar）。
        rgpui::highlight::register_highlighter(
            "demo",
            Box::new(|_| Some(Box::new(DemoLangStub) as Box<dyn rgpui::highlight::Highlighter>)),
        );
        editor.update(cx, |state, cx| {
            // 高亮器经编辑器状态透传接入（大纲同步刷新）。
            state.set_highlighter(Some(rgpui::highlight::rust_highlighter()), window, cx);
            // LSP 假 provider 注入（传输层由真应用实现后替换）。
            state.set_completion_provider(Some(Rc::new(DemoCompletionProvider)), cx);
            state.set_diagnostics_provider(Some(Rc::new(DemoDiagnosticsProvider)), cx);
            state.set_hover_provider(Some(Rc::new(DemoHoverProvider)), cx);
            state.set_document_uri(Some("file:///demo.rs".parse().unwrap()), cx);
            // inlay 默认开启（演示绘制；关开关即零开销）。
            state.set_inlay_provider(Some(Rc::new(DemoInlayProvider)), cx);
            state.set_inlay_hints_enabled(true, cx);
        });
        let input = editor.read_with(cx, |state, _| state.input().clone());
        let readonly = cx.new(|cx| {
            let mut state = InputState::new(window, cx).multi_line(true);
            // 初始内容不进撤销栈（与 `EditorState::new` 同理）。
            state.set_value(READONLY_SAMPLE, window, cx);
            state
        });

        // 大纲缓存由编辑器状态维护，这里只在文本变更时重渲染。
        cx.subscribe(&input, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();

        Self { editor, readonly }
    }

    fn goto(&mut self, symbol: &rgpui::highlight::DocumentSymbol, cx: &mut Context<Self>) {
        let symbol = symbol.clone();
        self.editor.update(cx, |state, cx| {
            state.goto_symbol(&symbol, cx);
        });
    }
}

impl Render for EditorDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let symbols = self
            .editor
            .read_with(cx, |state, _| state.outline().to_vec());
        let (diag_count, hover_text) = self.editor.read_with(cx, |state, _| {
            (state.diagnostics().len(), hover_preview(state))
        });
        let lang_text = self.editor.read_with(cx, |state, _| {
            state
                .language()
                .map(|s| s.to_string())
                .unwrap_or("纯文本".to_string())
        });
        let demo = cx.entity();
        // 补全弹窗（相对容器左上角弹出，点击行即确认插入）。
        let popup = self
            .editor
            .read_with(cx, |state, _| state.completion_popup().clone());
        let demo_for_popup = demo.clone();
        let popup_el = rgpui::lsp::CompletionPopup::new(popup).on_select(move |ix, window, cx| {
            demo_for_popup.update(cx, |this, cx| {
                this.editor.update(cx, |state, cx| {
                    state.accept_completion(Some(ix), window, cx);
                });
            });
        });
        h_flex()
            .size_full()
            .items_stretch()
            .gap(px(12.0))
            .p(px(12.0)).child(
            v_flex()
                .w(px(220.0))
                .gap(px(4.0))
                .child(div().text_sm().child("大纲（符号）"))
                .children(symbols.into_iter().map(|symbol| {
                    let label = format!("{} · {}行", symbol.name, symbol.start_row + 1);
                    let demo = demo.clone();
                    div()
                        .id(("outline-symbol", symbol.start_row))
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|this| this.bg(rgb(0x000000).opacity(0.05)))
                        .child(div().text_sm().child(label))
                        .on_click(move |_, _, cx| {
                            demo.update(cx, |this, cx| {
                                this.goto(&symbol, cx);
                            });
                        })
                })),
        )
        .child(
            v_flex()
                .flex_1()
                .gap(px(8.0))
                .child(
                    div().text_xs().child(
                        "行操作：Shift+Alt+↓复制行 Ctrl+Shift+K删行 Alt+↑↓移行 Ctrl+/注释 Ctrl+J合行 ｜ \
                         多光标：Ctrl+Alt+↑↓加光标 Alt+点击 ｜ 键入：自动补括号/电缩进/括号匹配/当前行高亮",
                    ),
                )
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .items_center()
                        .child(lsp_button(&demo, "lsp-complete", "请求补全", |state, window, cx| {
                            state.request_completions(window, cx);
                        }))
                        .child(lsp_button(&demo, "lsp-diagnostics", "请求诊断", |state, window, cx| {
                            state.request_diagnostics(window, cx);
                        }))
                        .child(lsp_button(&demo, "lsp-hover", "悬停光标处", |state, window, cx| {
                            let offset = state.cursor(cx);
                            state.request_hover(offset, window, cx);
                        }))
                        .child(lsp_button(&demo, "lsp-inlay", "请求 inlay", |state, window, cx| {
                            state.request_inlay_hints(window, cx);
                        }))
                        .child(
                            div()
                                .text_xs()
                                .child(format!("诊断 {diag_count} ｜ {hover_text}")),
                        ),
                )
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .items_center()
                        .child(lsp_button(&demo, "snip-expand", "展开片段", |state, window, cx| {
                            state.expand_snippet(
                                "fn ${1:name}(${2:args}) {\n    $0\n}",
                                window,
                                cx,
                            );
                        }))
                        .child(lsp_button(&demo, "snip-next", "下一占位 Tab", |state, _, cx| {
                            state.next_placeholder(cx);
                        }))
                        .child(lsp_button(&demo, "snip-prev", "上一占位 S-Tab", |state, _, cx| {
                            state.prev_placeholder(cx);
                        }))
                        .child(div().text_xs().child(
                            "片段：$1/$2 跳转，$0 收尾；会话内键入只跟踪跳转（镜像/强制不做）",
                        )),
                )
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .items_center()
                        .child(lsp_button(&demo, "lang-rust", "语言: rust", |state, window, cx| {
                            state.set_language("rust", window, cx);
                        }))
                        .child(lsp_button(&demo, "lang-demo", "语言: demo", |state, window, cx| {
                            state.set_language("demo", window, cx);
                        }))
                        .child(lsp_button(
                            &demo,
                            "lang-unknown",
                            "语言: 未知降级",
                            |state, window, cx| {
                                state.set_language("brainfuck-x", window, cx);
                            },
                        ))
                        .child(div().text_xs().child(format!("当前语言：{lang_text}"))),
                )
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .child(Editor::new(&self.editor).flex_1())
                        .child(popup_el),
                )
                .child(div().text_xs().child("只读预览（TextArea）："))
                .child(TextArea::new(&self.readonly).read_only(true)),
        )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(980.0), px(640.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| EditorDemo::new(window, cx)),
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
