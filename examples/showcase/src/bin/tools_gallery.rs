//! 差异 / 编辑器 / 延迟统计 示例集合。
//!
//! 用法：cargo run -p showcase --bin tools_gallery -- <demo>
//! 可用 demo：diff / editor / input_latency
//! 不带参数时默认运行 diff，传 --list 打印清单。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    AnyView, App, Bounds, Button, ButtonVariants, Context, IntoElement, ParentElement, Render,
    Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use rgpui_platform::application;

mod diff {
    //! 差异查看器示例。
    //!
    //! 演示 `DiffView` 组件的行级差异显示能力：对比两段文本，以颜色高亮显示
    //! 新增（绿色）、删除（红色）与未变更行，并可选显示行号。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin diff --features rgpui/diff
    //! ```

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::components::{DiffState, DiffView};
    use rgpui::{
        App, Bounds, Context, Entity, IntoElement, ParentElement, Render, Window, WindowBounds,
        WindowOptions, div, h_flex, prelude::*, px, v_flex,
    };
    use rgpui_platform::application;

    struct DiffDemo {
        /// 差异状态。
        diff_state: Entity<DiffState>,
    }

    impl DiffDemo {
        /// 创建视图并初始化差异状态。
        fn new(cx: &mut Context<Self>) -> Self {
            let old_text = r#"fn main() {
    println!("Hello, world!");
    let x = 5;
    let y = 10;
    println!("Sum: {}", x + y);
}
"#;

            let new_text = r#"fn main() {
    println!("你好，rgpui！");
    let x = 5;
    let z = 15;
    println!("Sum: {}", x + z);
    println!("Done!");
}
"#;

            let diff_state = cx.new(|_| DiffState::new(old_text, new_text));

            Self { diff_state }
        }
    }

    impl Render for DiffDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let added = self.diff_state.read(cx).added_count();
            let removed = self.diff_state.read(cx).removed_count();
            let unchanged = self.diff_state.read(cx).unchanged_count();

            div()
                .id("diff-root")
                .size_full()
                .overflow_scroll()
                .bg(cx.theme().tokens.background.color)
                .child(
                    v_flex()
                        .max_w(px(900.0))
                        .mx_auto()
                        .gap(px(16.0))
                        .p(px(24.0))
                        .child(div().text_2xl().child("差异查看器示例"))
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().tokens.muted_foreground.color)
                                .child("对比两段 Rust 代码，显示行级差异（新增、删除、未变更）。"),
                        )
                        .child(
                            h_flex()
                                .gap(px(16.0))
                                .child(div().text_sm().child(format!("新增: {added} 行")))
                                .child(div().text_sm().child(format!("删除: {removed} 行")))
                                .child(div().text_sm().child(format!("未变更: {unchanged} 行"))),
                        )
                        .child(
                            div()
                                .h(px(500.0))
                                .rounded_md()
                                .border_1()
                                .border_color(cx.theme().tokens.border.color)
                                .bg(cx.theme().tokens.background.color)
                                .child(DiffView::new(&self.diff_state)),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::theme::init(cx);

            let bounds = Bounds::centered(None, rgpui::size(px(960.0), px(720.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_window, cx| cx.new(DiffDemo::new),
            )
            .unwrap();
            cx.activate(true);
        });
    }
}

mod editor {
    //! Editor 演示：单个 `EditorState` + `Editor` 全家桶。
    //!
    //! 大纲/搜索面板/右键追加档/LSP/片段/inlay/粘性顶栏/语言切换全接这一个编辑器
    //! （搜索面板 `attach_editor` 接内部输入；右键经 `Editor::context_menu_extra`
    //! 追加转大写，默认菜单开箱即用）。tree-sitter 高亮/折叠 + 行操作 + 多光标
    //!（键位来自全局 `init_all` 默认注册）。

    #![cfg_attr(target_family = "wasm", no_main)]

    use std::rc::Rc;

    use rgpui::{
        App, Bounds, ClickEvent, Context, MouseButton, Pixels, Point, PopupMenuItem, Render,
        SharedString, Switch, Window, WindowBounds, WindowOptions, anchored, blue,
        components::SearchPanelState,
        deferred, div, green, h_flex,
        input_ui::{
            CodeActionProvider, CodeLens, CodeLensOverlay, CodeLensProvider, Editor, EditorState,
            InputEvent, StickyPosition,
        },
        lsp::{
            CompletionProvider, DefinitionProvider, DiagnosticEntry, DiagnosticsProvider,
            FormattingProvider, HoverContent, HoverProvider, HoverResponse, RenameProvider,
            SemanticTokensProvider, SignatureHelpProvider,
        },
        prelude::*,
        px, rgb, size, v_flex, white, yellow,
    };
    use rgpui_platform::application;

    const SAMPLE: &str = "fn main() {\n    let name = \"rgpui\";\n    println!(\"hello, {name}\");\n}\n\nstruct Point {\n    x: f32,\n    y: f32,\n}\n\nimpl Point {\n    fn len(&self) -> f32 {\n        (self.x * self.x + self.y * self.y).sqrt()\n    }\n}\n";

    struct DemoCompletionProvider;

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
                .filter(|(label, _, _)| {
                    prefix.is_empty() || label.to_lowercase().starts_with(&prefix)
                })
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

    struct DemoDiagnosticsProvider;

    struct DemoSemanticTokensProvider;

    impl SemanticTokensProvider for DemoSemanticTokensProvider {
        fn semantic_tokens_full_with_legend(
            &self,
            _uri: &lsp_types::Uri,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Option<rgpui::lsp::SemanticTokensPayload>>> {
            // 第 0 行第 0 列长度 3（"fn "）为 function，第 3 列长度 4（"main"）为 function。
            rgpui::Task::ready(Ok(Some(rgpui::lsp::SemanticTokensPayload {
                data: vec![0, 0, 3, 0, 0, 0, 3, 4, 0, 0],
                token_types: vec![lsp_types::SemanticTokenType::new("function")],
                token_modifiers: Vec::new(),
            })))
        }
    }
    struct DemoSignatureHelpProvider;

    impl SignatureHelpProvider for DemoSignatureHelpProvider {
        fn signature_help(
            &self,
            _text: &rgpui::input_ui::Rope,
            _offset: usize,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Option<lsp_types::SignatureHelp>>> {
            rgpui::Task::ready(Ok(Some(lsp_types::SignatureHelp {
                signatures: vec![lsp_types::SignatureInformation {
                    label: "fn add(a: i32, b: i32) -> i32".to_string(),
                    documentation: None,
                    parameters: None,
                    active_parameter: None,
                }],
                active_signature: Some(0),
                active_parameter: None,
            })))
        }
    }
    struct DemoDefinitionProvider;

    impl DefinitionProvider for DemoDefinitionProvider {
        fn definition(
            &self,
            text: &rgpui::input_ui::Rope,
            offset: usize,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Vec<rgpui::lsp::DefinitionLocation>>> {
            let content = text.to_string();
            let byte = offset.min(content.len());
            let is_word = |c: char| c.is_alphanumeric() || c == '_';
            let mut start = byte;
            while start > 0 {
                let Some(prev) = content[..start].chars().next_back() else {
                    break;
                };
                if !is_word(prev) {
                    break;
                }
                start -= prev.len_utf8();
            }
            let mut end = byte;
            while end < content.len() {
                let Some(next) = content[end..].chars().next() else {
                    break;
                };
                if !is_word(next) {
                    break;
                }
                end += next.len_utf8();
            }
            if start >= end {
                return rgpui::Task::ready(Ok(Vec::new()));
            }
            let line = content[..start].matches('\n').count() as u32;
            let column = start - content[..start].rfind('\n').map_or(0, |i| i + 1);
            let uri: lsp_types::Uri = "file:///demo.rs".parse().expect("URI");
            rgpui::Task::ready(Ok(vec![rgpui::lsp::DefinitionLocation {
                uri,
                range: lsp_types::Range {
                    start: lsp_types::Position::new(line, column as u32),
                    end: lsp_types::Position::new(line, (column + (end - start)) as u32),
                },
                display_name: None,
            }]))
        }

        fn references(
            &self,
            _text: &rgpui::input_ui::Rope,
            _offset: usize,
            _include_declarations: bool,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Vec<rgpui::lsp::DefinitionLocation>>> {
            rgpui::Task::ready(Ok(Vec::new()))
        }

        fn document_highlights(
            &self,
            text: &rgpui::input_ui::Rope,
            offset: usize,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Vec<rgpui::lsp::DocumentHighlight>>> {
            // 同符号高亮：光标所在的词整体标出。
            let content = text.to_string();
            let byte = offset.min(content.len());
            let is_word = |c: char| c.is_alphanumeric() || c == '_';
            let mut start = byte;
            while start > 0 {
                let Some(prev) = content[..start].chars().next_back() else {
                    break;
                };
                if !is_word(prev) {
                    break;
                }
                start -= prev.len_utf8();
            }
            let mut end = byte;
            while end < content.len() {
                let Some(next) = content[end..].chars().next() else {
                    break;
                };
                if !is_word(next) {
                    break;
                }
                end += next.len_utf8();
            }
            if start >= end {
                return rgpui::Task::ready(Ok(Vec::new()));
            }
            let line = content[..start].matches('\n').count() as u32;
            let column = start - content[..start].rfind('\n').map_or(0, |i| i + 1);
            rgpui::Task::ready(Ok(vec![rgpui::lsp::DocumentHighlight {
                range: lsp_types::Range {
                    start: lsp_types::Position::new(line, column as u32),
                    end: lsp_types::Position::new(line, (column + (end - start)) as u32),
                },
                kind: Some(rgpui::lsp::DocumentHighlightKind::Text),
            }]))
        }
    }
    struct DemoRenameProvider;

    impl RenameProvider for DemoRenameProvider {
        fn rename(
            &self,
            text: &rgpui::input_ui::Rope,
            offset: usize,
            new_name: &str,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Option<lsp_types::WorkspaceEdit>>> {
            // 找出光标所在的词（空白分隔），生成一条替换编辑。
            let content = text.to_string();
            let byte = offset.min(content.len());
            let is_word = |c: char| c.is_alphanumeric() || c == '_';
            let mut start = byte;
            while start > 0 {
                let Some(prev) = content[..start].chars().next_back() else {
                    break;
                };
                if !is_word(prev) {
                    break;
                }
                start -= prev.len_utf8();
            }
            let mut end = byte;
            while end < content.len() {
                let Some(next) = content[end..].chars().next() else {
                    break;
                };
                if !is_word(next) {
                    break;
                }
                end += next.len_utf8();
            }
            if start >= end {
                return rgpui::Task::ready(Ok(None));
            }
            // 行号 / 字节列（LSP 行列按字节列换算）。
            let line = content[..start].matches('\n').count() as u32;
            let column = start - content[..start].rfind('\n').map_or(0, |i| i + 1);
            let end_line = line + content[start..end].matches('\n').count() as u32;
            let end_column = column + (end - start);
            let uri: lsp_types::Uri = "file:///demo.rs".parse().expect("URI");
            rgpui::Task::ready(Ok(Some(lsp_types::WorkspaceEdit {
                changes: Some(std::collections::HashMap::from([(
                    uri,
                    vec![lsp_types::TextEdit {
                        range: lsp_types::Range {
                            start: lsp_types::Position::new(line, column as u32),
                            end: lsp_types::Position::new(end_line, end_column as u32),
                        },
                        new_text: new_name.to_string(),
                    }],
                )])),
                document_changes: None,
                change_annotations: None,
            })))
        }
    }
    struct DemoFormattingProvider;

    impl FormattingProvider for DemoFormattingProvider {
        fn formatting(
            &self,
            text: &rgpui::input_ui::Rope,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Vec<lsp_types::TextEdit>>> {
            let original = text.to_string();
            let lines: Vec<&str> = original.split('\n').collect();
            let formatted: String = lines
                .iter()
                .map(|line| line.trim_end())
                .collect::<Vec<_>>()
                .join("\n");
            if formatted == original {
                return rgpui::Task::ready(Ok(Vec::new()));
            }
            // 整篇替换：结束位置取最后一行及其字节长度（LSP 行列按字节列换算）。
            let last = lines.len().saturating_sub(1);
            let end = lsp_types::Position::new(last as u32, lines[last].len() as u32);
            rgpui::Task::ready(Ok(vec![lsp_types::TextEdit {
                range: lsp_types::Range {
                    start: lsp_types::Position::new(0, 0),
                    end,
                },
                new_text: formatted,
            }]))
        }
    }

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

    struct DemoCodelensProvider;

    impl CodeLensProvider for DemoCodelensProvider {
        fn codelenses(
            &self,
            _text: &rgpui::input_ui::Rope,
            _visible: std::ops::Range<usize>,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Vec<CodeLens>>> {
            rgpui::Task::ready(Ok(vec![CodeLens {
                line: 5,
                title: "Point · 1 引用".into(),
            }]))
        }
    }

    struct DemoCodeActionProvider;

    impl CodeActionProvider for DemoCodeActionProvider {
        fn code_actions(
            &self,
            text: &rgpui::input_ui::Rope,
            range: std::ops::Range<usize>,
            diagnostics: Vec<lsp_types::Diagnostic>,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Vec<lsp_types::CodeActionOrCommand>>> {
            use lsp_types::{CodeAction, CodeActionKind, Range, TextEdit, Uri, WorkspaceEdit};
            let len = text.len();
            let start = range.start.min(len);
            let end = range.end.min(len);
            // 选区所在行行首字节偏移（无 ropey 直接依赖，逐字符扫换行）。
            let mut line_start = 0;
            let mut off = 0;
            for ch in text.slice(..start).chars() {
                off += ch.len_utf8();
                if ch == '\n' {
                    line_start = off;
                }
            }
            let position =
                |offset: usize| rgpui::lsp::PositionMapping::offset_to_position(text, offset);
            let uri: Uri = "file:///demo.rs".parse().unwrap();
            let action = |title: &str,
                          kind: CodeActionKind,
                          edits: Vec<TextEdit>,
                          preferred: Option<bool>| CodeAction {
                title: title.to_string(),
                kind: Some(kind),
                edit: Some(WorkspaceEdit {
                    changes: Some(std::collections::HashMap::from([(uri.clone(), edits)])),
                    ..Default::default()
                }),
                is_preferred: preferred,
                ..Default::default()
            };
            let mut actions = vec![action(
                "行首加注释",
                CodeActionKind::REFACTOR,
                vec![TextEdit {
                    range: Range::new(position(line_start), position(line_start)),
                    new_text: "// ".to_string(),
                }],
                None,
            )];
            if start != end {
                actions.push(action(
                    "选区替换为 demo",
                    CodeActionKind::REFACTOR,
                    vec![TextEdit {
                        range: Range::new(position(start), position(end)),
                        new_text: "demo".to_string(),
                    }],
                    None,
                ));
            }
            // 有诊断时给一条首选修复：在诊断末尾标注（演示 `is_preferred` 默认选中）。
            if let Some(first) = diagnostics.first() {
                actions.insert(
                    0,
                    action(
                        "标注首条诊断",
                        CodeActionKind::QUICKFIX,
                        vec![TextEdit {
                            range: Range::new(first.range.end, first.range.end),
                            new_text: " /* 已标注 */".to_string(),
                        }],
                        Some(true),
                    ),
                );
            }
            rgpui::Task::ready(Ok(actions
                .into_iter()
                .map(lsp_types::CodeActionOrCommand::CodeAction)
                .collect()))
        }
    }

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
        breakpoints: std::collections::HashSet<usize>,
        bookmarks: std::collections::HashSet<usize>,
        gutter_status: String,
        /// 搜索弹窗：`None` 关闭，`Some(show_replace)` 打开（`Ctrl+F` 关替换/`Ctrl+R` 开）。
        search_open: Option<bool>,
        /// gutter 菜单：打开的行 + 窗口坐标（左键/右键都开菜单，不再常驻图标）。
        gutter_menu: Option<(usize, Point<Pixels>)>,
        /// 折叠开关状态（诊断行首遮挡用：关掉后折叠箭头消失）。
        folding: bool,
        /// 布局调试标尺（行首遮挡定位用：四色竖线标出 gutter/行号/折叠/文本边界）。
        debug_chrome: bool,
    }

    fn sorted_vec(set: &std::collections::HashSet<usize>) -> Vec<usize> {
        let mut v: Vec<usize> = set.iter().copied().collect();
        v.sort_unstable();
        v
    }

    fn open_gutter_menu(
        demo: &rgpui::Entity<EditorDemo>,
        row: usize,
        pos: Point<Pixels>,
        cx: &mut App,
    ) {
        demo.update(cx, |this, _| {
            this.gutter_menu = Some((row, pos));
        });
    }

    fn gutter_menu_row(
        demo: &rgpui::Entity<EditorDemo>,
        id: &'static str,
        label: String,
        apply: impl Fn(&mut EditorDemo) + 'static,
    ) -> impl IntoElement {
        let demo = demo.clone();
        div()
            .id(id)
            .px(px(10.0))
            .py(px(6.0))
            .rounded_md()
            .cursor_pointer()
            .hover(|this| this.bg(rgb(0x000000).opacity(0.08)))
            .text_sm()
            .child(label)
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
            })
            .on_mouse_down(MouseButton::Right, move |_, _, cx| {
                cx.stop_propagation();
            })
            .on_click(move |_, _, cx| {
                demo.update(cx, |this, _| {
                    apply(this);
                    this.gutter_menu = None;
                });
            })
    }
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
                Box::new(
                    |_| Some(Box::new(DemoLangStub) as Box<dyn rgpui::highlight::Highlighter>),
                ),
            );
            editor.update(cx, |state, cx| {
                // 高亮器经编辑器状态透传接入（大纲同步刷新）。
                state.set_highlighter(Some(rgpui::highlight::rust_highlighter()), window, cx);
                // 面包屑放状态栏（顶栏永不出现，无高度跳变，编辑区不位移）。
                state.set_sticky_position(StickyPosition::Status, cx);
                // LSP 假 provider 注入（传输层由真应用实现后替换）。
                state.set_completion_provider(Some(Rc::new(DemoCompletionProvider)), cx);
                state.set_diagnostics_provider(Some(Rc::new(DemoDiagnosticsProvider)), cx);
                state.set_hover_provider(Some(Rc::new(DemoHoverProvider)), cx);
                state.set_formatting_provider(Some(Rc::new(DemoFormattingProvider)), cx);
                state.set_rename_provider(Some(Rc::new(DemoRenameProvider)), cx);
                state.set_definition_provider(Some(Rc::new(DemoDefinitionProvider)), cx);
                state.set_signature_help_provider(Some(Rc::new(DemoSignatureHelpProvider)), cx);
                state.set_semantic_tokens_provider(
                    Some(Rc::new(DemoSemanticTokensProvider)),
                    "rust",
                    cx,
                );
                // 透镜假 provider（文本变更自动刷新，点击跳光标到透镜行）。
                state.set_codelens_provider(Some(Rc::new(DemoCodelensProvider)), cx);
                state.set_document_uri(Some("file:///demo.rs".parse().unwrap()), cx);
                // 快速修复：`Alt+Enter` 弹菜单（菜单浮层由 `Editor` 自带渲染）。
                state.set_code_action_provider(Some(Rc::new(DemoCodeActionProvider)), cx);
                // inlay 默认开启（演示绘制；关开关即零开销）。
                state.set_inlay_provider(Some(Rc::new(DemoInlayProvider)), cx);
                state.set_inlay_hints_enabled(true, cx);
            });
            let input = editor.read_with(cx, |state, _| state.input().clone());

            // gutter 单 provider（菜单模式）：已设行才画图标，未设行返回隐形可点格
            // （左键/右键都开菜单选动作，不再逐行常驻图标）。
            // 同行多标记显示规则（格宽 20px 只够一个）：断点 ● > 书签 ⚑ > 运行 ▶；
            // 菜单行实时反映各类型有无（设/取消），全量状态点开即见。
            {
                let demo = cx.entity();
                let text_handle = input.clone();
                editor.update(cx, |state, cx| {
                    state.add_gutter_provider(
                        "menu",
                        move |row, _window, cx| {
                            let demo_for_cell = demo.clone();
                            let (bp_set, mark_set) = demo_for_cell.read_with(cx, |this, _| {
                                (
                                    this.breakpoints.contains(&row),
                                    this.bookmarks.contains(&row),
                                )
                            });
                            // 运行标记只在 `fn main` 行且无断点书签时展示。
                            let runnable = !bp_set
                                && !mark_set
                                && text_handle.read_with(cx, |s, _| {
                                    s.text()
                                        .to_string()
                                        .lines()
                                        .nth(row)
                                        .map(|line| line.trim_start().starts_with("fn main"))
                                        .unwrap_or(false)
                                });
                            let marker: Option<(SharedString, u32)> = if bp_set {
                                Some(("●".into(), 0xef4444))
                            } else if mark_set {
                                Some(("⚑".into(), 0xeab308))
                            } else if runnable {
                                Some(("▶".into(), 0x22c55e))
                            } else {
                                None
                            };
                            let open_menu_row = row;
                            let demo_for_click = demo_for_cell.clone();
                            let mut cell = div()
                                .w_full()
                                .h_full()
                                .overflow_hidden()
                                .flex()
                                .items_center()
                                .justify_center()
                                .id(("gutter-cell", row))
                                .cursor_pointer()
                                .on_click(move |event, _, cx| {
                                    if let ClickEvent::Mouse(e) = event {
                                        open_gutter_menu(
                                            &demo_for_click,
                                            open_menu_row,
                                            e.down.position,
                                            cx,
                                        );
                                    }
                                })
                                // 右键走捕获阶段先手 + 停传播：压住编辑器自身的右键菜单，
                                // 否则两层菜单叠加，点 gutter 菜单会误触下层（如粘贴）。
                                // 所有鼠标监听同属窗口级循环（捕获先行），先到先停可靠。
                                .capture_any_mouse_down({
                                    let demo_for_cell = demo_for_cell.clone();
                                    move |event, _, cx| {
                                        if event.button == MouseButton::Right {
                                            open_gutter_menu(
                                                &demo_for_cell,
                                                open_menu_row,
                                                event.position,
                                                cx,
                                            );
                                            cx.stop_propagation();
                                        }
                                    }
                                });
                            if let Some((glyph, color)) = marker {
                                cell = cell.text_xs().text_color(rgb(color)).child(glyph);
                            }
                            Some(cell.into_any_element())
                        },
                        cx,
                    );
                    state.set_gutter_column_enabled(true, cx);
                });
            }

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

            Self {
                editor,
                search,
                breakpoints: std::collections::HashSet::new(),
                bookmarks: std::collections::HashSet::new(),
                gutter_status: "gutter：点格子开菜单（断点/书签/运行），同行多标记只显优先级最高者"
                    .to_string(),
                search_open: None,
                gutter_menu: None,
                folding: true,
                debug_chrome: false,
            }
        }

        fn goto(&mut self, symbol: &rgpui::highlight::DocumentSymbol, cx: &mut Context<Self>) {
            let symbol = symbol.clone();
            self.editor.update(cx, |state, cx| {
                state.goto_symbol(&symbol, cx);
            });
        }
    }

    impl Render for EditorDemo {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            let popup_el =
                rgpui::lsp::CompletionPopup::new(popup).on_change(move |ix, window, cx| {
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
                         多光标：Ctrl+Alt+↑↓加光标 Alt+点击",
                    ),
                )
                .child(
                    div().text_xs().child(
                        "键入：自动补括号/电缩进/括号匹配/当前行高亮 ｜ \
                         补全：↑↓改选 Enter确认 Esc收起（确认时替换光标处单词）｜ \
                         修复：Alt+Enter 弹菜单，↑↓改选 Enter应用 Esc收起",
                    ),
                )
                .child(
                    h_flex()
                        .flex_wrap()
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
                                .on_change(move |checked, _, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.editor.update(cx, |state, cx| {
                                            state.set_auto_completion_enabled(checked, cx);
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
                                .on_change(move |checked, _, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.editor.update(cx, |state, cx| {
                                            state.set_minimap_enabled(checked, cx);
                                        });
                                    });
                                })
                        })
                        .child({
                            let demo = demo.clone();
                            Switch::new("vim-mode")
                                .checked(vim_on)
                                .label("Vim 模式")
                                .on_change(move |checked, _, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.editor.update(cx, |state, cx| {
                                            state.set_vim_enabled(checked, cx);
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
                        .flex_wrap()
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
                        .flex_wrap()
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
                            "lang-python",
                            "语言: python",
                            |state, window, cx| {
                                state.set_language("python", window, cx);
                            },
                        ))
                        .child(lsp_button(
                            &demo,
                            "lang-javascript",
                            "语言: javascript",
                            |state, window, cx| {
                                state.set_language("javascript", window, cx);
                            },
                        ))
                        .child(lsp_button(&demo, "lang-go", "语言: go", |state, window, cx| {
                            state.set_language("go", window, cx);
                        }))
                        .child(lsp_button(&demo, "lang-json", "语言: json", |state, window, cx| {
                            state.set_language("json", window, cx);
                        }))
                        .child(lsp_button(
                            &demo,
                            "format-doc",
                            "格式化文档（LSP）",
                            |state, window, cx| {
                                state.request_formatting(window, cx);
                            },
                        ))
                        .child(lsp_button(
                            &demo,
                            "rename-symbol",
                            "重命名光标处 → renamed",
                            |state, window, cx| {
                                state.request_rename("renamed", window, cx);
                            },
                        ))
                        .child(lsp_button(
                            &demo,
                            "goto-definition",
                            "跳转定义（LSP）",
                            |state, window, cx| {
                                state.goto_definition(window, cx);
                            },
                        ))
                        .child(lsp_button(
                            &demo,
                            "signature-help",
                            "签名提示（LSP）",
                            |state, window, cx| {
                                state.request_signature_help(window, cx);
                            },
                        ))
                        .child(lsp_button(
                            &demo,
                            "document-highlights",
                            "同符号高亮（LSP）",
                            |state, window, cx| {
                                state.request_document_highlights(window, cx);
                            },
                        ))
                        .child(lsp_button(
                            &demo,
                            "semantic-tokens",
                            "语义高亮（LSP）",
                            |state, window, cx| {
                                state.request_semantic_tokens(window, cx);
                            },
                        ))
                        .child(lsp_button(
                            &demo,
                            "lang-unknown",
                            "语言: 未知降级",
                            |state, window, cx| {
                                state.set_language("brainfuck-x", window, cx);
                            },
                        ))
                        .child(div().text_xs().child(format!("当前语言：{lang_text}")))
                        .child(div().text_xs().child("标黄："))
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
                )
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap(px(8.0))
                        .items_center()
                        .child(lsp_button(&demo, "gutter-master", "gutter 开关", |state, _, cx| {
                            let enabled = !state.gutter_column_enabled(cx);
                            state.set_gutter_column_enabled(enabled, cx);
                        }))
                        .child({
                            let demo = demo.clone();
                            let folding = self.folding;
                            Switch::new("folding")
                                .checked(folding)
                                .label("折叠")
                                .on_change(move |checked, window, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.folding = checked;
                                        this.editor.update(cx, |state, cx| {
                                            state.set_folding(checked, window, cx);
                                        });
                                    });
                                })
                        })
                        .child({
                            let demo = demo.clone();
                            let debug_chrome = self.debug_chrome;
                            Switch::new("debug-chrome")
                                .checked(debug_chrome)
                                .label("布局调试")
                                .on_change(move |checked, _, cx| {
                                    demo.update(cx, |this, _| {
                                        this.debug_chrome = checked;
                                    });
                                })
                        })
                        .child(
                            div()
                                .text_xs()
                                .child("标尺：红=gutter缘 绿=行号缘 黄=折叠缘 青=文本首"),
                        )
                        .child(div().text_xs().child(self.gutter_status.clone())),
                )
                .child(
                    div()
                        .relative()
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
                        .child(lens_el)
                        .when_some(self.search_open, |this, show_replace| {
                            // 搜索弹窗（编辑器左上浮动；右侧经常被裁，左上最稳）。
                            // `Ctrl+F` 纯查找/`Ctrl+R` 带替换，`Esc` 关闭。
                            let demo = demo.clone();
                            this.child(
                                div()
                                    .absolute()
                                    .top(px(8.0))
                                    .left(px(8.0))
                                    .child(
                                        v_flex()
                                            .gap(px(4.0))
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .child(div().text_sm().child(if show_replace {
                                                        "查找替换"
                                                    } else {
                                                        "查找"
                                                    }))
                                                    .child(
                                                        div()
                                                            .id("search-popup-close")
                                                            .px(px(8.0))
                                                            .cursor_pointer()
                                                            .text_sm()
                                                            .child("×")
                                                            .on_click(move |_, _, cx| {
                                                                demo.update(cx, |this, cx| {
                                                                    this.search_open = None;
                                                                    // 关弹窗清标黄（装饰不清会一直留在编辑器上）。
                                                                    this.search.update(cx, |panel, cx| {
                                                                        panel.clear_highlights(cx);
                                                                    });
                                                                });
                                                            }),
                                                    ),
                                            )
                                            .child(self.search.clone())
                                            .child(
                                                div().text_xs().child(
                                                    "Esc 关闭 · Enter 下一个 · Shift+Enter 上一个",
                                                ),
                                            ),
                                    ),
                            )
                        }),
                )
                .child(div().text_xs().child(
                    "右键=默认菜单 + 追加转大写（已集成进编辑器，无独立演示行）。",
                )),
        )
        .when_some(
            self.debug_chrome
                .then(|| {
                    self.editor
                        .read_with(cx, |state, cx| state.chrome_geometry(cx))
                })
                .flatten(),
            |this, geo| {
                // 布局调试标尺（窗口坐标竖线，不拦截点击）。
                let h = window.bounds().size.height;
                let rule = |x: Pixels, color: u32| {
                    deferred(
                        div()
                            .absolute()
                            .top_0()
                            .left(x)
                            .w(px(1.))
                            .h(h)
                            .bg(rgb(color)),
                    )
                    .into_any_element()
                };
                this.children([
                    rule(geo.gutter_end, 0xff0000),
                    rule(geo.numbers_end, 0x00ff00),
                    rule(geo.fold_end, 0xffff00),
                    rule(geo.text_start, 0x00ffff),
                ])
            },
        )
        .when_some(self.gutter_menu, |this, (row, pos)| {
            // gutter 菜单（点击行处锚定；透罩点空处关闭）。
            let demo_for_catcher = demo.clone();
            let demo_for_right = demo.clone();
            let bp_set = self.breakpoints.contains(&row);
            let mark_set = self.bookmarks.contains(&row);
            let popover = cx.theme().tokens.popover;
            let border = cx.theme().tokens.border;
            let radius = cx.theme().radius;
            let win_size = window.bounds().size;
            this.children([
                deferred(
                    div()
                        .id("gutter-menu-catcher")
                        .absolute()
                        .top_0()
                        .left_0()
                        .w(win_size.width)
                        .h(win_size.height)
                        // 透罩吞按下（ shield 下层编辑器；点击合成照常触发关闭）。
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            cx.stop_propagation();
                        })
                        .on_mouse_down(MouseButton::Right, move |_, _, cx| {
                            demo_for_right.update(cx, |this, _| {
                                this.gutter_menu = None;
                            });
                            cx.stop_propagation();
                        })
                        .on_click(move |_, _, cx| {
                            demo_for_catcher.update(cx, |this, _| {
                                this.gutter_menu = None;
                            });
                        }),
                )
                .into_any_element(),
                deferred(
                    anchored()
                        .position(pos)
                        .anchor(rgpui::Anchor::TopLeft)
                        .snap_to_window_with_margin(px(8.))
                        .child(
                            v_flex()
                                .w(px(180.))
                                .p(px(4.))
                                .gap(px(2.))
                                .bg(popover)
                                .border_1()
                                .border_color(border)
                                .rounded(radius)
                                .shadow_lg()
                                .child(gutter_menu_row(
                                    &demo,
                                    "gutter-menu-bp",
                                    if bp_set {
                                        "● 取消断点".to_string()
                                    } else {
                                        "○ 设断点".to_string()
                                    },
                                    move |this| {
                                        if !this.breakpoints.remove(&row) {
                                            this.breakpoints.insert(row);
                                        }
                                        this.gutter_status =
                                            format!("断点行：{:?}", sorted_vec(&this.breakpoints));
                                    },
                                ))
                                .child(gutter_menu_row(
                                    &demo,
                                    "gutter-menu-mark",
                                    if mark_set {
                                        "⚑ 取消书签".to_string()
                                    } else {
                                        "⚑ 设书签".to_string()
                                    },
                                    move |this| {
                                        if !this.bookmarks.remove(&row) {
                                            this.bookmarks.insert(row);
                                        }
                                        this.gutter_status =
                                            format!("书签行：{:?}", sorted_vec(&this.bookmarks));
                                    },
                                ))
                                .child(gutter_menu_row(
                                    &demo,
                                    "gutter-menu-run",
                                    "▶ 运行此行".to_string(),
                                    move |this| {
                                        this.gutter_status =
                                            format!("运行第 {} 行（演示，未真跑）", row + 1);
                                    },
                                )),
                        ),
                )
                .with_priority(1)
                .into_any_element(),
            ])
        })
        .capture_key_down({
            let demo = demo.clone();
            move |event: &rgpui::KeyDownEvent, window: &mut Window, cx: &mut App| {
                let mods = &event.keystroke.modifiers;
                let key = event.keystroke.key.as_str();
                // Ctrl+F/R 开搜索弹窗（替换行显隐不同；聚到搜索框）。
                if mods.control && !mods.alt && !mods.shift && (key == "f" || key == "r") {
                    let show_replace = key == "r";
                    demo.update(cx, |this, cx| {
                        this.search_open = Some(show_replace);
                        this.search.update(cx, |panel, cx| {
                            panel.set_show_replace(show_replace, cx);
                            // 重开把关闭期清掉的标黄标回来。
                            panel.refresh_highlights(cx);
                        });
                    });
                    let search = demo.read_with(cx, |this, _| this.search.clone());
                    search.update(cx, |panel, cx| {
                        panel.search_input().update(cx, |input, cx| {
                            input.focus(window, cx);
                        });
                    });
                } else if key == "escape" {
                    // Esc 先关 gutter 菜单，再关搜索弹窗（并回焦编辑器）。
                    let closed = demo.read_with(cx, |this, _| {
                        this.gutter_menu.is_some() || this.search_open.is_some()
                    });
                    demo.update(cx, |this, cx| {
                        this.gutter_menu = None;
                        // Esc 关搜索弹窗同样清标黄（与 × 按钮同语义）。
                        if this.search_open.is_some() {
                            this.search.update(cx, |panel, cx| {
                                panel.clear_highlights(cx);
                            });
                        }
                        this.search_open = None;
                    });
                    if closed {
                        // 回焦编辑器输入框。
                        let input = demo.read_with(cx, |this, cx| {
                            this.editor.read_with(cx, |state, _| state.input().clone())
                        });
                        input.update(cx, |state, cx| {
                            state.focus(window, cx);
                        });
                    }
                }
            }
        })
        }
    }

    pub fn run_example() {
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

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| EditorDemo::new(window, cx)).into()
    }
}

mod input_latency {
    //! 输入延迟直方图示例（`input-latency-histogram` feature 门控）。
    //!
    //! 展示如何读取窗口的输入延迟统计（`window.input_latency_snapshot()`）：
    //! - 输入事件到帧呈现的延迟直方图（纳秒，界面以毫秒展示）
    //! - 每帧合并的输入事件数直方图
    //! - 帧绘制期间到达而被丢弃的事件计数
    //!
    //! 窗口会持续请求动画帧，让统计直方图持续累积数据。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin input_latency --features input-latency
    //! ```

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::{
        ActiveTheme, App, AppContext as _, Bounds, Context, InteractiveElement, IntoElement,
        ParentElement, Render, StatefulInteractiveElement as _, Styled, StyledExt, Window,
        WindowBounds, WindowOptions, div, h_flex, px, size, v_flex,
    };
    use rgpui_platform::application;

    struct InputLatencyExample {
        /// 点击计数：点击窗口任意位置累加。
        clicks: usize,
        /// 演示区悬停状态：鼠标移入/移出改变外观。
        demo_hovered: bool,
    }

    impl Render for InputLatencyExample {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            // 持续请求动画帧，让输入延迟统计持续累积样本。
            window.request_animation_frame();

            // 读取当前窗口的输入延迟统计快照。
            let snapshot = window.input_latency_snapshot();
            let clicks = self.clicks;
            // 悬停高亮背景：提前算好，避免在 `Stateful<Div>` 上调用其不支持的 `.when()`。
            let demo_bg = if self.demo_hovered {
                rgpui::hsla(0.6, 0.5, 0.5, 0.25)
            } else {
                rgpui::hsla(0.0, 0.0, 0.0, 0.0)
            };

            v_flex()
            .id("input-latency-example")
            .size_full()
            .p(px(32.0))
            .gap(px(24.0))
            .on_click(cx.listener(|this, _, _, cx| {
                this.clicks += 1;
                cx.notify();
            }))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(div().text_2xl().font_semibold().child("输入延迟直方图"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "提示：在下方演示区移动鼠标或点击窗口任意位置（已点击响应的事件才会被统计），观察延迟统计实时变化。",
                            ),
                    ),
            )
            .child(
                h_flex()
                    .gap(px(24.0))
                    .child(latency_panel(&snapshot.latency_histogram))
                    .child(events_panel(&snapshot.events_per_frame_histogram)),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "帧绘制期间到达而被丢弃的事件：{}",
                        snapshot.mid_draw_events_dropped
                    )),
            )
            .child(
                div()
                    .id("latency-interaction-demo")
                    .p(px(16.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(rgpui::hsla(0.0, 0.0, 0.5, 0.3))
                    .bg(demo_bg)
                    .on_hover(cx.listener(|this, hovered, _, cx| {
                        this.demo_hovered = *hovered;
                        cx.notify();
                    }))
                    .child(format!(
                        "交互演示区：悬停会高亮，累计点击 {clicks} 次（每次点击/悬停变化都会触发重绘并被统计）",
                    )),
            )
        }
    }

    fn latency_panel(h: &hdrhistogram::Histogram<u64>) -> impl IntoElement {
        let to_ms = |v: u64| format!("{:.3} ms", v as f64 / 1_000_000.0);
        stat_panel(
            "输入→帧呈现延迟",
            vec![
                ("样本数", h.len().to_string()),
                ("均值", to_ms(h.mean() as u64)),
                ("最大值", to_ms(h.max())),
                ("P50", to_ms(h.value_at_quantile(0.5))),
                ("P90", to_ms(h.value_at_quantile(0.9))),
                ("P99", to_ms(h.value_at_quantile(0.99))),
            ],
        )
    }

    fn events_panel(h: &hdrhistogram::Histogram<u64>) -> impl IntoElement {
        stat_panel(
            "每帧合并输入事件数",
            vec![
                ("样本数", h.len().to_string()),
                ("均值", format!("{:.2} 个", h.mean())),
                ("最大值", format!("{} 个", h.max())),
                ("P50", format!("{} 个", h.value_at_quantile(0.5))),
                ("P90", format!("{} 个", h.value_at_quantile(0.9))),
                ("P99", format!("{} 个", h.value_at_quantile(0.99))),
            ],
        )
    }

    fn stat_panel(title: &str, rows: Vec<(&str, String)>) -> impl IntoElement {
        let title = title.to_string();
        v_flex()
            .gap(px(8.0))
            .p(px(16.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgpui::hsla(0.0, 0.0, 0.5, 0.3))
            .child(div().text_base().font_medium().child(title))
            .children(
                rows.into_iter()
                    .map(|(name, value)| stat_row(name.to_string(), value)),
            )
    }

    fn stat_row(name: String, value: String) -> impl IntoElement {
        div()
            .flex()
            .justify_between()
            .w_full()
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::hsla(0.0, 0.0, 0.5, 0.8))
                    .child(name),
            )
            .child(div().text_sm().child(value))
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(720.0), px(480.0)),
                        cx,
                    ))),
                    ..Default::default()
                },
                |_window, cx| {
                    cx.new(|_| InputLatencyExample {
                        clicks: 0,
                        demo_hovered: false,
                    })
                },
            )
            .ok();

            cx.activate(true);
        });
    }
}

/// Gallery 根视图：左侧竖行 tab + 右侧示例内容。
struct GalleryApp {
    titles: Vec<&'static str>,
    /// 当前示例的介绍与用法。
    active: usize,
    views: Vec<AnyView>,
}

impl GalleryApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let titles: Vec<&'static str> = vec!["editor"];
        let builds: &[fn(&mut Window, &mut App) -> AnyView] = &[editor::build];
        let views = builds.iter().map(|build| build(window, cx)).collect();
        Self {
            titles,
            active: 0,
            views,
        }
    }
}

impl Render for GalleryApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let theme = cx.theme();
        div()
            .id("gallery-app")
            .size_full()
            .flex()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                div()
                    .id("gallery-tabs")
                    .h_full()
                    .w(px(180.0))
                    .flex_shrink_0()
                    .flex_col()
                    .overflow_y_scroll()
                    .border_r_1()
                    .border_color(theme.border)
                    .children(self.titles.iter().enumerate().map(|(ix, title)| {
                        let this = this.clone();
                        Button::new(("gallery-tab", ix))
                            .label(*title)
                            .w_full()
                            .when(ix == self.active, |b| b.primary())
                            .when(ix != self.active, |b| b.ghost())
                            .on_click(move |_, _, cx| {
                                this.update(cx, |gallery, cx| {
                                    gallery.active = ix;
                                    cx.notify();
                                });
                            })
                    })),
            )
            .child(
                div()
                    .id("gallery-content")
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .overflow_hidden()
                    .child(self.views[self.active].clone()),
            )
    }
}

fn run() {
    // 传 -- <demo> 时保留原有独立窗口行为；不带参数时打开 gallery 导航界面。
    if let Some(name) = std::env::args().nth(1) {
        match name.as_str() {
            "diff" => diff::run_example(),
            "editor" => editor::run_example(),
            "input_latency" => input_latency::run_example(),
            _ => eprintln!("未知 demo：{name}"),
        }
        return;
    }
    application().run(|cx: &mut App| {
        rgpui::theme::init(cx);
        rgpui::input_ui::init(cx);
        rgpui::menu::init(cx);
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| GalleryApp::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run();
}
