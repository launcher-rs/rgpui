//! Editor 演示：单个 `EditorState` + `Editor` 全家桶。
//!
//! 大纲/搜索面板/右键追加档/LSP/片段/inlay/粘性顶栏/语言切换全接这一个编辑器
//! （搜索面板 `attach_editor` 接内部输入；右键经 `Editor::context_menu_extra`
//! 追加转大写，默认菜单开箱即用）。tree-sitter 高亮/折叠 + 行操作 + 多光标
//!（键位来自全局 `init_all` 默认注册）。

#![cfg_attr(target_family = "wasm", no_main)]

use std::rc::Rc;

use rgpui::{
    App, Bounds, Context, PopupMenuItem, Render, Switch, Window, WindowBounds, WindowOptions, blue,
    components::SearchPanelState,
    div, green, h_flex,
    input_ui::{CodeLens, CodeLensOverlay, CodeLensProvider, Editor, EditorState, InputEvent},
    lsp::{
        CompletionProvider, DiagnosticEntry, DiagnosticsProvider, HoverContent, HoverProvider,
        HoverResponse,
    },
    prelude::*,
    px, rgb, size, v_flex, white, yellow,
};
use rgpui_platform::application;

const SAMPLE: &str = "fn main() {\n    let name = \"rgpui\";\n    println!(\"hello, {name}\");\n}\n\nstruct Point {\n    x: f32,\n    y: f32,\n}\n\nimpl Point {\n    fn len(&self) -> f32 {\n        (self.x * self.x + self.y * self.y).sqrt()\n    }\n}\n";

/// 演示用假补全 provider：按光标前单词前缀过滤（真 provider 由应用实现传输后注入）。
///
/// 无匹配时返回空列表，弹窗自动隐藏；前缀为空（如刚键入 `.`）时全量返回。
struct DemoCompletionProvider;

/// 演示词表（标签，种类，说明）。
const DEMO_WORDS: &[(&str, lsp_types::CompletionItemKind, &str)] = &[
    (
        "println",
        lsp_types::CompletionItemKind::FUNCTION,
        "宏 · 打印换行",
    ),
    (
        "print",
        lsp_types::CompletionItemKind::FUNCTION,
        "宏 · 打印不换行",
    ),
    ("private", lsp_types::CompletionItemKind::KEYWORD, "关键字"),
    ("pub", lsp_types::CompletionItemKind::KEYWORD, "关键字"),
    ("Point", lsp_types::CompletionItemKind::STRUCT, "演示结构体"),
    ("len", lsp_types::CompletionItemKind::METHOD, "方法"),
];

/// 光标前连续单词（字母/数字/`_`），字节偏移与框架内 `slice` 口径一致。
fn word_prefix_before(text: &rgpui::input_ui::Rope, offset: usize) -> String {
    let offset = offset.min(text.len());
    let mut start = 0;
    let mut off = 0;
    for ch in text.slice(..offset).chars() {
        off += ch.len_utf8();
        if !(ch.is_alphanumeric() || ch == '_') {
            start = off;
        }
    }
    text.slice(start..offset).to_string()
}

impl CompletionProvider for DemoCompletionProvider {
    fn completions(
        &self,
        text: &rgpui::input_ui::Rope,
        offset: usize,
        _trigger: lsp_types::CompletionContext,
        _window: &mut Window,
        _cx: &mut App,
    ) -> rgpui::Task<anyhow::Result<lsp_types::CompletionResponse>> {
        // 大小写不敏感的前缀匹配；真 LSP 服务端按同样语义过滤后返回。
        let prefix = word_prefix_before(text, offset).to_lowercase();
        let items = DEMO_WORDS
            .iter()
            .filter(|(label, _, _)| prefix.is_empty() || label.to_lowercase().starts_with(&prefix))
            .map(|(label, kind, detail)| lsp_types::CompletionItem {
                label: label.to_string(),
                kind: Some(*kind),
                detail: Some(detail.to_string()),
                ..Default::default()
            })
            .collect();
        rgpui::Task::ready(Ok(lsp_types::CompletionResponse::Array(items)))
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

/// 演示用假透镜 provider：首行运行 + 结构体引用（点击跳光标）。
struct DemoCodelensProvider;

impl CodeLensProvider for DemoCodelensProvider {
    fn codelenses(
        &self,
        _text: &rgpui::input_ui::Rope,
        _visible: std::ops::Range<usize>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> rgpui::Task<anyhow::Result<Vec<CodeLens>>> {
        rgpui::Task::ready(Ok(vec![
            CodeLens {
                line: 0,
                title: "▶ 运行".into(),
            },
            CodeLens {
                line: 5,
                title: "Point · 1 引用".into(),
            },
        ]))
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
    search: rgpui::Entity<SearchPanelState>,
}

/// 行列（字节列）转全文 UTF-8 字节偏移（替换接线用）。
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
            // 透镜假 provider（文本变更自动刷新，点击跳光标到透镜行）。
            state.set_codelens_provider(Some(Rc::new(DemoCodelensProvider)), cx);
            state.set_document_uri(Some("file:///demo.rs".parse().unwrap()), cx);
            // inlay 默认开启（演示绘制；关开关即零开销）。
            state.set_inlay_provider(Some(Rc::new(DemoInlayProvider)), cx);
            state.set_inlay_hints_enabled(true, cx);
        });
        let input = editor.read_with(cx, |state, _| state.input().clone());

        // 大纲缓存由编辑器状态维护，这里只在文本变更时重渲染。
        cx.subscribe(&input, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();

        // 搜索面板：一行接通编辑器（文本同步 + 匹配标黄 + 默认跳转）。
        let search = cx.new(|cx| SearchPanelState::new(window, cx));
        search.update(cx, |panel, cx| panel.attach_editor(&input, cx));
        // 替换接线（替换权在外部：当前匹配 / 全部匹配从后往前保偏移）。
        {
            let text = input.clone();
            let panel_handle = search.clone();
            search.update(cx, |panel, _| {
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
            let text = input.clone();
            let panel_handle = search.clone();
            search.update(cx, |panel, _| {
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

        Self { editor, search }
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
        let auto_complete = self
            .editor
            .read_with(cx, |state, _| state.auto_completion_enabled());
        let minimap_on = self
            .editor
            .read_with(cx, |state, _| state.minimap_enabled());
        let vim_on = self.editor.read_with(cx, |state, cx| state.vim_enabled(cx));
        let demo = cx.entity();
        // 自动补全开关（开后键入单词字符即弹补全，空格/换行自动收起）。
        // 补全弹窗（光标处锚定，`deferred` 浮层，点击行即确认插入）。
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
        // 透镜浮层（行首上方，点击跳光标到透镜行）。
        let demo_for_lens = demo.clone();
        let lens_el = CodeLensOverlay::new(&self.editor).on_lens(move |ix, _, cx| {
            demo_for_lens.update(cx, |this, cx| {
                let offset = this.editor.read_with(cx, |state, _| {
                    state.codelenses().get(ix).map(|lens| lens.offset)
                });
                if let Some(offset) = offset {
                    this.editor.update(cx, |state, cx| {
                        state.set_selected_range(offset..offset, cx);
                    });
                }
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
                         多光标：Ctrl+Alt+↑↓加光标 Alt+点击 ｜ 键入：自动补括号/电缩进/括号匹配/当前行高亮 ｜ \
                         补全：↑↓改选 Enter确认 Esc收起（确认时替换光标处单词）",
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
                        .child({
                            let demo = demo.clone();
                            Switch::new("auto-complete")
                                .checked(auto_complete)
                                .label("输入时自动补全")
                                .on_click(move |checked, _, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.editor.update(cx, |state, cx| {
                                            state.set_auto_completion_enabled(*checked, cx);
                                        });
                                    });
                                })
                        })
                        .child(
                            div()
                                .text_xs()
                                .child("自动：单词前缀满 2 个字符或键入 ./: 才请求，按前缀过滤，无匹配自动隐藏"),
                        )
                        .child({
                            let demo = demo.clone();
                            Switch::new("minimap")
                                .checked(minimap_on)
                                .label("缩略图")
                                .on_click(move |checked, _, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.editor.update(cx, |state, cx| {
                                            state.set_minimap_enabled(*checked, cx);
                                        });
                                    });
                                })
                        })
                        .child({
                            let demo = demo.clone();
                            Switch::new("vim-mode")
                                .checked(vim_on)
                                .label("Vim 模式")
                                .on_click(move |checked, _, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.editor.update(cx, |state, cx| {
                                            state.set_vim_enabled(*checked, cx);
                                        });
                                    });
                                })
                        })
                        .child(
                            div()
                                .text_xs()
                                .child("Vim：hjkl/wb/0/$/gg/G 移动，i/a/o/v 切换，x/dd/yy/p/u 编辑，可视 y/d"),
                        )
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
                        .flex_1()
                        .child(
                            Editor::new(&self.editor)
                                .flex_1()
                                .context_menu_extra({
                                    let upper = self
                                        .editor
                                        .read_with(cx, |state, _| state.input().clone());
                                    move |menu, _, _, _| {
                                        let upper = upper.clone();
                                        menu.item(
                                            PopupMenuItem::new("转为大写 UPPERCASE").on_click(
                                                move |_, window, cx| {
                                                    upper.update(cx, |state, cx| {
                                                        let selected =
                                                            state.selected_value().to_string();
                                                        if !selected.is_empty() {
                                                            state.replace(
                                                                selected.to_uppercase(),
                                                                window,
                                                                cx,
                                                            );
                                                        }
                                                    });
                                                },
                                            ),
                                        )
                                    }
                                }),
                        )
                        .child(popup_el)
                        .child(lens_el),
                )
                .child(div().text_xs().child(
                    "右键=默认菜单 + 追加转大写（已集成进编辑器，无独立演示行）。",
                )),
        )
        .child(
            v_flex()
                .w(px(300.0))
                .gap(px(8.0))
                .child(div().text_sm().child("搜索（接编辑器）"))
                .child(self.search.clone())
                .child(div().text_xs().child("标黄配色："))
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .child({
                            let demo = demo.clone();
                            div()
                                .id("hl-yellow")
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded_md()
                                .cursor_pointer()
                                .text_xs()
                                .child("黄")
                                .on_click(move |_, _, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.search.update(cx, |panel, cx| {
                                            panel.set_highlight_colors(yellow(), None, cx)
                                        });
                                    });
                                })
                        })
                        .child({
                            let demo = demo.clone();
                            div()
                                .id("hl-green")
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded_md()
                                .cursor_pointer()
                                .text_xs()
                                .child("绿")
                                .on_click(move |_, _, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.search.update(cx, |panel, cx| {
                                            panel.set_highlight_colors(green(), None, cx)
                                        });
                                    });
                                })
                        })
                        .child({
                            let demo = demo.clone();
                            div()
                                .id("hl-blue")
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded_md()
                                .cursor_pointer()
                                .text_xs()
                                .child("蓝")
                                .on_click(move |_, _, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.search.update(cx, |panel, cx| {
                                            panel.set_highlight_colors(blue(), Some(white()), cx)
                                        });
                                    });
                                })
                        }),
                ),
        )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
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
