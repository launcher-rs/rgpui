use std::{
    ops::Range,
    rc::Rc,
    str::FromStr,
    sync::{Arc, RwLock},
    time::Duration,
};

use rgpui::{
    prelude::FluentBuilder as _, px, size, App, AppContext, Context, Entity, Focusable,
    InteractiveElement, IntoElement, ParentElement as _, Render, SharedString, Styled as _, Task,
    StatefulInteractiveElement, Window, WindowBounds, WindowOptions, div,
};
use rgpui_component::{ActiveTheme as _, Root, h_flex, v_flex};
use rgpui_component_assets::Assets;
use rgpui_editor::{
    Editor, EditorState, Rope, RopeExt,
    highlighter::{Diagnostic, DiagnosticSeverity},
    input::{
        CodeActionProvider, CompletionProvider, DefinitionProvider, DocumentColorProvider,
        HoverProvider, InputEvent, Position,
    },
};

/// 模拟 LSP 存储，提供补全、代码操作、诊断等功能。
#[derive(Clone)]
struct MockLspStore {
    completions: Arc<Vec<lsp_types::CompletionItem>>,
    code_actions: Arc<RwLock<Vec<(Range<usize>, lsp_types::CodeAction)>>>,
    diagnostics: Arc<RwLock<Vec<Diagnostic>>>,
}

impl MockLspStore {
    fn new() -> Self {
        let completions = vec![
            make_completion_item("println!", "println!(\"{}\", value)", "打印格式化输出"),
            make_completion_item("eprintln!", "eprintln!(\"{}\", value)", "打印错误输出"),
            make_completion_item("format!", "format!(\"{}\", value)", "格式化字符串"),
            make_completion_item("vec!", "vec![elem1, elem2]", "创建 Vec"),
            make_completion_item("HashMap", "HashMap::new()", "创建 HashMap"),
            make_completion_item("HashSet", "HashSet::new()", "创建 HashSet"),
            make_completion_item("Arc", "Arc::new(value)", "原子引用计数指针"),
            make_completion_item("RwLock", "RwLock::new(value)", "读写锁"),
            make_completion_item("Mutex", "Mutex::new(value)", "互斥锁"),
            make_completion_item("Option", "Some(value)", "可选类型"),
            make_completion_item("Result", "Ok(value)", "结果类型"),
            make_completion_item("String", "String::new()", "字符串类型"),
            make_completion_item("Vec", "Vec::new()", "动态数组"),
            make_completion_item("Box", "Box::new(value)", "堆分配指针"),
            make_completion_item("Rc", "Rc::new(value)", "引用计数指针"),
            make_completion_item("Duration", "Duration::from_secs(1)", "时间间隔"),
            make_completion_item("sleep", "std::thread::sleep(dur)", "线程睡眠"),
            make_completion_item("spawn", "std::thread::spawn(|| {})", "生成线程"),
            make_completion_item("unwrap", ".unwrap()", "解包或 panic"),
            make_completion_item("expect", ".expect(\"msg\")", "解包或自定义 panic"),
        ];

        Self {
            completions: Arc::new(completions),
            code_actions: Arc::new(RwLock::new(vec![])),
            diagnostics: Arc::new(RwLock::new(vec![])),
        }
    }

    fn diagnostics(&self) -> Vec<Diagnostic> {
        self.diagnostics.read().unwrap().clone()
    }

    fn update_diagnostics(&self, diagnostics: Vec<Diagnostic>) {
        *self.diagnostics.write().unwrap() = diagnostics;
    }

    fn code_actions(&self) -> Vec<(Range<usize>, lsp_types::CodeAction)> {
        self.code_actions.read().unwrap().clone()
    }

    fn update_code_actions(&self, code_actions: Vec<(Range<usize>, lsp_types::CodeAction)>) {
        *self.code_actions.write().unwrap() = code_actions;
    }
}

fn make_completion_item(
    label: &str,
    insert_text: &str,
    documentation: &str,
) -> lsp_types::CompletionItem {
    lsp_types::CompletionItem {
        label: label.to_string(),
        kind: Some(lsp_types::CompletionItemKind::FUNCTION),
        insert_text: Some(insert_text.to_string()),
        documentation: Some(lsp_types::Documentation::String(documentation.to_string())),
        ..Default::default()
    }
}

fn slash_command_item(
    replace_range: &lsp_types::Range,
    label: &str,
    insert_text: &str,
    documentation: &str,
) -> lsp_types::CompletionItem {
    lsp_types::CompletionItem {
        label: label.to_string(),
        kind: Some(lsp_types::CompletionItemKind::SNIPPET),
        text_edit: Some(lsp_types::CompletionTextEdit::InsertAndReplace(lsp_types::InsertReplaceEdit {
            new_text: insert_text.to_string(),
            insert: *replace_range,
            replace: *replace_range,
        })),
        documentation: Some(lsp_types::Documentation::String(documentation.to_string())),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        ..Default::default()
    }
}

impl CompletionProvider for MockLspStore {
    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        trigger: lsp_types::CompletionContext,
        _window: &mut Window,
        cx: &mut Context<EditorState>,
    ) -> Task<anyhow::Result<lsp_types::CompletionResponse>> {
        let trigger_character = trigger.trigger_character.unwrap_or_default();
        if trigger_character.is_empty() {
            return Task::ready(Ok(lsp_types::CompletionResponse::Array(vec![])));
        }

        let rope = rope.clone();
        let items = self.completions.clone();
        cx.background_spawn(async move {
            smol::Timer::after(Duration::from_millis(50)).await;

            if trigger_character.starts_with('/') {
                let start = offset.saturating_sub(trigger_character.len());
                let start_pos = rope.offset_to_position(start);
                let end_pos = rope.offset_to_position(offset);
                let replace_range = lsp_types::Range::new(start_pos, end_pos);

                let slash_items = vec![
                    slash_command_item(&replace_range, "/date", &format!("{}", chrono::Local::now().date_naive()), "插入当前日期"),
                    slash_command_item(&replace_range, "/time", &format!("{}", chrono::Local::now().time()), "插入当前时间"),
                    slash_command_item(&replace_range, "/thanks", "Thank you!", "插入感谢语"),
                    slash_command_item(&replace_range, "/+1", "👍", "插入点赞"),
                    slash_command_item(&replace_range, "/-1", "👎", "插入点踩"),
                    slash_command_item(&replace_range, "/smile", "😊", "插入微笑"),
                    slash_command_item(&replace_range, "/launch", "🚀", "插入火箭"),
                ];
                return Ok(lsp_types::CompletionResponse::Array(slash_items));
            }

            let filtered = items
                .iter()
                .filter(|item| {
                    item.label
                        .to_lowercase()
                        .starts_with(&trigger_character.to_lowercase())
                })
                .take(10)
                .cloned()
                .collect();

            Ok(lsp_types::CompletionResponse::Array(filtered))
        })
    }

    fn inline_completion(
        &self,
        rope: &Rope,
        offset: usize,
        _trigger: lsp_types::InlineCompletionContext,
        _window: &mut Window,
        cx: &mut Context<EditorState>,
    ) -> Task<anyhow::Result<lsp_types::InlineCompletionResponse>> {
        let rope = rope.clone();
        cx.background_spawn(async move {
            let point = rope.offset_to_point(offset);
            let line_start = rope.line_start_offset(point.row);
            let current_line = rope.slice(line_start..offset).to_string();

            let suggestion = if current_line.trim_start().starts_with("fn ")
                && !current_line.contains('{')
            {
                Some("() {\n    // 在此编写代码..\n}".into())
            } else if current_line.trim_start().starts_with("let ")
                && !current_line.contains('=')
            {
                Some(" = todo!();".into())
            } else if current_line.trim_start().starts_with("struct ")
                && !current_line.contains('{')
            {
                Some(" {\n    // 字段定义\n}".into())
            } else if current_line.trim_start().starts_with("impl ")
                && !current_line.contains('{')
            {
                Some(" {\n    // 方法实现\n}".into())
            } else {
                None
            };

            if let Some(insert_text) = suggestion {
                Ok(lsp_types::InlineCompletionResponse::Array(vec![
                    lsp_types::InlineCompletionItem {
                        insert_text,
                        filter_text: None,
                        range: None,
                        command: None,
                        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
                    },
                ]))
            } else {
                Ok(lsp_types::InlineCompletionResponse::Array(vec![]))
            }
        })
    }

    fn is_completion_trigger(
        &self,
        _offset: usize,
        new_text: &str,
        _cx: &mut Context<EditorState>,
    ) -> bool {
        !new_text.is_empty()
    }
}

impl CodeActionProvider for MockLspStore {
    fn id(&self) -> SharedString {
        "MockLsp".into()
    }

    fn code_actions(
        &self,
        _state: Entity<EditorState>,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<anyhow::Result<Vec<lsp_types::CodeAction>>> {
        let mut actions = vec![];
        for (node_range, code_action) in self.code_actions().iter() {
            if range.start >= node_range.start && range.end <= node_range.end {
                actions.push(code_action.clone());
            }
        }
        Task::ready(Ok(actions))
    }

    fn perform_code_action(
        &self,
        state: Entity<EditorState>,
        action: lsp_types::CodeAction,
        _push_to_history: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<()>> {
        let Some(edit) = action.edit else {
            return Task::ready(Ok(()));
        };
        let Some((_, text_edits)) = edit.changes.and_then(|m| m.into_iter().next()) else {
            return Task::ready(Ok(()));
        };

        let state = state.downgrade();
        window.spawn(cx, async move |cx| {
            state.update_in(cx, |state, window, cx| {
                state.apply_lsp_edits(&text_edits, window, cx);
            })
        })
    }
}

impl HoverProvider for MockLspStore {
    fn hover(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<anyhow::Result<Option<lsp_types::Hover>>> {
        let word = text.word_at(offset);
        if word.is_empty() {
            return Task::ready(Ok(None));
        }

        let item = self
            .completions
            .iter()
            .find(|item| item.label == word);

        let contents = if let Some(item) = item {
            if let Some(doc) = &item.documentation {
                match doc {
                    lsp_types::Documentation::String(s) => s.clone(),
                    lsp_types::Documentation::MarkupContent(mc) => mc.value.clone(),
                }
            } else {
                "暂无文档说明。".to_string()
            }
        } else {
            return Task::ready(Ok(None));
        };

        let hover = lsp_types::Hover {
            contents: lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String(contents)),
            range: None,
        };

        Task::ready(Ok(Some(hover)))
    }
}

const RUST_DOC_URLS: &[(&str, &str)] = &[
    ("String", "string/struct.String"),
    ("Debug", "fmt/trait.Debug"),
    ("Clone", "clone/trait.Clone"),
    ("Option", "option/enum.Option"),
    ("Result", "result/enum.Result"),
    ("Vec", "vec/struct.Vec"),
    ("HashMap", "collections/hash_map/struct.HashMap"),
    ("HashSet", "collections/hash_set/struct.HashSet"),
    ("Arc", "sync/struct.Arc"),
    ("RwLock", "sync/struct.RwLock"),
    ("Duration", "time/struct.Duration"),
    ("println", "macro.println"),
    ("eprintln", "macro.eprintln"),
    ("format", "macro.format"),
    ("todo", "macro.todo"),
    ("panic", "macro.panic"),
];

impl DefinitionProvider for MockLspStore {
    fn definitions(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<anyhow::Result<Vec<lsp_types::LocationLink>>> {
        let Some(word_range) = text.word_range(offset) else {
            return Task::ready(Ok(vec![]));
        };
        let word = text.slice(word_range.clone()).to_string();

        let document_uri = lsp_types::Uri::from_str("file://example").unwrap();
        let start = text.offset_to_position(word_range.start);
        let end = text.offset_to_position(word_range.end);
        let symbol_range = lsp_types::Range { start, end };

        if word == "Duration" {
            let target_range = lsp_types::Range {
                start: lsp_types::Position {
                    line: 2,
                    character: 4,
                },
                end: lsp_types::Position {
                    line: 2,
                    character: 23,
                },
            };
            return Task::ready(Ok(vec![lsp_types::LocationLink {
                target_uri: document_uri,
                target_range: target_range,
                target_selection_range: target_range,
                origin_selection_range: Some(symbol_range),
            }]));
        }

        for (ix, t) in RUST_DOC_URLS.iter().map(|(name, _)| *name).enumerate() {
            if t == word {
                let url = RUST_DOC_URLS[ix].1;
                let location = lsp_types::LocationLink {
                    target_uri: lsp_types::Uri::from_str(&format!(
                        "https://doc.rust-lang.org/std/{}.html",
                        url
                    ))
                    .unwrap(),
                    target_selection_range: lsp_types::Range::default(),
                    target_range: lsp_types::Range::default(),
                    origin_selection_range: Some(symbol_range),
                };
                return Task::ready(Ok(vec![location]));
            }
        }

        Task::ready(Ok(vec![]))
    }
}

impl DocumentColorProvider for MockLspStore {
    fn document_colors(
        &self,
        _text: &Rope,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<anyhow::Result<Vec<lsp_types::ColorInformation>>> {
        Task::ready(Ok(vec![]))
    }
}

/// 文本转换代码操作提供者，支持大小写转换等。
struct TextConvertor;

impl CodeActionProvider for TextConvertor {
    fn id(&self) -> SharedString {
        "TextConvertor".into()
    }

    fn code_actions(
        &self,
        state: Entity<EditorState>,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Vec<lsp_types::CodeAction>>> {
        let mut actions = vec![];
        if range.is_empty() {
            return Task::ready(Ok(actions));
        }

        let state = state.read(cx);
        let document_uri = lsp_types::Uri::from_str("file://example").unwrap();

        let old_text = state.text().slice(range.clone()).to_string();
        let start = state.text().offset_to_position(range.start);
        let end = state.text().offset_to_position(range.end);
        let range = lsp_types::Range { start, end };

        actions.push(lsp_types::CodeAction {
            title: "转换为大写".into(),
            kind: Some(lsp_types::CodeActionKind::REFACTOR),
            edit: Some(lsp_types::WorkspaceEdit {
                changes: Some(
                    std::iter::once((
                        document_uri.clone(),
                        vec![lsp_types::TextEdit {
                            range,
                            new_text: old_text.to_uppercase(),
                        }],
                    ))
                    .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        });

        actions.push(lsp_types::CodeAction {
            title: "转换为小写".into(),
            kind: Some(lsp_types::CodeActionKind::REFACTOR),
            edit: Some(lsp_types::WorkspaceEdit {
                changes: Some(
                    std::iter::once((
                        document_uri.clone(),
                        vec![lsp_types::TextEdit {
                            range,
                            new_text: old_text.to_lowercase(),
                        }],
                    ))
                    .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        });

        actions.push(lsp_types::CodeAction {
            title: "首字母大写".into(),
            kind: Some(lsp_types::CodeActionKind::REFACTOR),
            edit: Some(lsp_types::WorkspaceEdit {
                changes: Some(
                    std::iter::once((
                        document_uri.clone(),
                        vec![lsp_types::TextEdit {
                            range,
                            new_text: old_text
                                .chars()
                                .enumerate()
                                .map(|(i, c)| {
                                    if i == 0 {
                                        c.to_uppercase().to_string()
                                    } else {
                                        c.to_string()
                                    }
                                })
                                .collect(),
                        }],
                    ))
                    .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        });

        Task::ready(Ok(actions))
    }

    fn perform_code_action(
        &self,
        state: Entity<EditorState>,
        action: lsp_types::CodeAction,
        _push_to_history: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<()>> {
        let Some(edit) = action.edit else {
            return Task::ready(Ok(()));
        };
        let Some((_, text_edits)) = edit.changes.and_then(|m| m.into_iter().next()) else {
            return Task::ready(Ok(()));
        };

        let state = state.downgrade();
        window.spawn(cx, async move |cx| {
            state.update_in(cx, |state, window, cx| {
                state.apply_lsp_edits(&text_edits, window, cx);
            })
        })
    }
}

/// 示例应用主结构体。
struct LspExample {
    editor: Entity<EditorState>,
    lsp_store: MockLspStore,
    line_number: bool,
    soft_wrap: bool,
    show_whitespaces: bool,
    folding: bool,
    _subscriptions: Vec<rgpui::Subscription>,
}

impl LspExample {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let lsp_store = MockLspStore::new();

        let editor = cx.new(|cx| {
            let mut editor = EditorState::new(window, cx)
                .code_editor("rust")
                .line_number(true)
                .soft_wrap(false)
                .folding(true)
                .placeholder("在此输入 Rust 代码...");

            editor.set_value(include_str!("./fixtures/test.rs"), window, cx);

            let lsp_store = Rc::new(lsp_store.clone());
            editor.lsp.completion_provider = Some(lsp_store.clone());
            editor.lsp.code_action_providers =
                vec![lsp_store.clone(), Rc::new(TextConvertor)];
            editor.lsp.hover_provider = Some(lsp_store.clone());
            editor.lsp.definition_provider = Some(lsp_store.clone());
            editor.lsp.document_color_provider = Some(lsp_store.clone());

            editor
        });

        let focus_handle = editor.focus_handle(cx);
        window.defer(cx, move |window, cx| {
            focus_handle.focus(window, cx);
        });

        let _subscriptions = vec![cx.subscribe(&editor, |this, _, _: &InputEvent, cx| {
            this.lint_document(cx);
        })];

        Self {
            editor,
            lsp_store,
            line_number: true,
            soft_wrap: false,
            show_whitespaces: false,
            folding: true,
            _subscriptions,
        }
    }

    fn lint_document(&mut self, cx: &mut Context<Self>) {
        let lsp_store = self.lsp_store.clone();
        let text = self.editor.read(cx).text().clone();

        cx.background_spawn(async move {
            let value = text.to_string();
            let mut diagnostics = vec![];
            let mut code_actions = vec![];

            let document_uri = lsp_types::Uri::from_str("file://example").unwrap();

            for (line_idx, line) in value.lines().enumerate() {
                if line.contains("TODO") || line.contains("todo") {
                    let col = line.find("TODO").or_else(|| line.find("todo")).unwrap();
                    let start = Position::new(line_idx as u32, col as u32);
                    let end =
                        Position::new(line_idx as u32, (col + line[col..].trim_start().len()) as u32);
                    let message = "发现待办事项，请处理".to_string();
                    diagnostics.push(
                        Diagnostic::new(start..end, message)
                            .with_severity(DiagnosticSeverity::Hint),
                    );
                }

                if line.contains("println!") && !line.contains("rgpui") {
                    if let Some(col) = line.find("println!") {
                        let start = Position::new(line_idx as u32, col as u32);
                        let end = Position::new(line_idx as u32, (col + 8) as u32);
                        let message = "建议使用 log 宏替代 println!".to_string();
                        diagnostics.push(
                            Diagnostic::new(start..end, message)
                                .with_severity(DiagnosticSeverity::Info),
                        );
                    }
                }
            }

            for diagnostic in &diagnostics {
                let start_offset = text.position_to_offset(&diagnostic.range.start);
                let end_offset = text.position_to_offset(&diagnostic.range.end);
                let range = start_offset..end_offset;

                let lsp_range = lsp_types::Range {
                    start: diagnostic.range.start,
                    end: diagnostic.range.end,
                };

                code_actions.push((
                    range,
                    lsp_types::CodeAction {
                        title: format!("修复: {}", diagnostic.message),
                        kind: Some(lsp_types::CodeActionKind::QUICKFIX),
                        edit: Some(lsp_types::WorkspaceEdit {
                            changes: Some(
                                std::iter::once((
                                    document_uri.clone(),
                                    vec![lsp_types::TextEdit {
                                        range: lsp_range,
                                        new_text: "/* 已修复 */".to_string(),
                                    }],
                                ))
                                .collect(),
                            ),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                ));
            }

            lsp_store.update_code_actions(code_actions);
            lsp_store.update_diagnostics(diagnostics);
        })
        .detach();
    }

    fn render_line_number_button(
        &self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("line-number-btn")
            .px_2()
            .py_1()
            .rounded_sm()
            .cursor_pointer()
            .when(self.line_number, |el| el.bg(cx.theme().primary.opacity(0.15)))
            .hover(|el| el.bg(cx.theme().primary.opacity(0.1)))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.line_number { "✓ " } else { "" })
                    .child("行号"),
            )
            .on_click(cx.listener(|this, _, window, cx| {
                this.line_number = !this.line_number;
                this.editor.update(cx, |state, cx| {
                    state.set_line_number(this.line_number, window, cx);
                });
                cx.notify();
            }))
    }

    fn render_soft_wrap_button(&self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("soft-wrap-btn")
            .px_2()
            .py_1()
            .rounded_sm()
            .cursor_pointer()
            .when(self.soft_wrap, |el| el.bg(cx.theme().primary.opacity(0.15)))
            .hover(|el| el.bg(cx.theme().primary.opacity(0.1)))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.soft_wrap { "✓ " } else { "" })
                    .child("自动换行"),
            )
            .on_click(cx.listener(|this, _, window, cx| {
                this.soft_wrap = !this.soft_wrap;
                this.editor.update(cx, |state, cx| {
                    state.set_soft_wrap(this.soft_wrap, window, cx);
                });
                cx.notify();
            }))
    }

    fn render_show_whitespaces_button(
        &self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("whitespace-btn")
            .px_2()
            .py_1()
            .rounded_sm()
            .cursor_pointer()
            .when(self.show_whitespaces, |el| el.bg(cx.theme().primary.opacity(0.15)))
            .hover(|el| el.bg(cx.theme().primary.opacity(0.1)))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.show_whitespaces { "✓ " } else { "" })
                    .child("空白字符"),
            )
            .on_click(cx.listener(|this, _, window, cx| {
                this.show_whitespaces = !this.show_whitespaces;
                this.editor.update(cx, |state, cx| {
                    state.set_show_whitespaces(this.show_whitespaces, window, cx);
                });
                cx.notify();
            }))
    }

    fn render_folding_button(&self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("folding-btn")
            .px_2()
            .py_1()
            .rounded_sm()
            .cursor_pointer()
            .when(self.folding, |el| el.bg(cx.theme().primary.opacity(0.15)))
            .hover(|el| el.bg(cx.theme().primary.opacity(0.1)))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.folding { "✓ " } else { "" })
                    .child("代码折叠"),
            )
            .on_click(cx.listener(|this, _, window, cx| {
                this.folding = !this.folding;
                this.editor.update(cx, |state, cx| {
                    state.set_folding(this.folding, window, cx);
                });
                cx.notify();
            }))
    }

    fn render_cursor_position(&self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let position = self.editor.read(cx).cursor_position();
        let cursor = self.editor.read(cx).cursor();

        div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(format!(
                "行 {} 列 {} (字节 {})",
                position.line + 1,
                position.character + 1,
                cursor
            ))
    }

    fn render_status_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .justify_between()
            .text_sm()
            .bg(cx.theme().background)
            .py_1p5()
            .px_4()
            .border_t_1()
            .border_color(cx.theme().border)
            .text_color(cx.theme().muted_foreground)
            .child(
                h_flex()
                    .gap_2()
                    .child(self.render_line_number_button(window, cx))
                    .child(self.render_soft_wrap_button(window, cx))
                    .child(self.render_show_whitespaces_button(window, cx))
                    .child(self.render_folding_button(window, cx)),
            )
            .child(self.render_cursor_position(window, cx))
    }
}

impl Render for LspExample {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.lsp_store.diagnostics().is_empty() {
            let diagnostics = self.lsp_store.diagnostics();
            self.editor.update(cx, |state, cx| {
                if let Some(set) = state.diagnostics_mut() {
                    set.clear();
                    set.extend(diagnostics);
                }
                cx.notify();
            });
        }

        v_flex()
            .id("lsp-example")
            .size_full()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .id("editor-container")
                    .w_full()
                    .flex_1()
                    .child(
                        div()
                            .id("editor-title")
                            .px_4()
                            .py_2()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(rgpui::FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("LSP 编辑器示例 - 自动补全演示"),
                            ),
                    )
                    .child(
                        Editor::new(&self.editor)
                            .bordered(false)
                            .p_0()
                            .h_full()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_size(cx.theme().mono_font_size)
                            .focus_bordered(false),
                    ),
            )
            .child(self.render_status_bar(window, cx))
    }
}

fn main() {
    let app = rgpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        rgpui_component::init(cx);
        rgpui_editor::init(cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1100.), px(750.)), cx)),
            titlebar: Some(rgpui::TitlebarOptions {
                title: Some("rgpui-editor LSP 示例".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        cx.spawn(async move |cx| {
            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|cx| LspExample::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
            })
            .expect("无法打开 rgpui-editor LSP 示例窗口");
        })
        .detach();
    });
}
