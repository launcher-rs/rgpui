//! Code Action 快速修复（`editor` feature 门控）：`Alt+Enter` 拉取修复项并成文本编辑。
//!
//! 薄封装，与补全/透镜同款管线：provider trait（默认空实现，不断编译）→
//! `EditorState` 请求（epoch 作废旧请求，M2 同款）→ 状态存外壳小字段 → `Editor`
//! 自带浮层渲染（deferred + anchored，补全弹窗同款定位）→ 键盘 ↑↓ 改选、
//! Enter 应用、Esc 收起。
//!
//! v1 约束（文档注明即契约）：只应用**当前文档**的文本编辑（多文件
//! `documentChanges` 里别家文件的改动忽略；未设 `document_uri` 时仅当编辑只涉及
//! 一个文档才应用）；命令型动作（只有 `command` 没有 `edit`）不进框架执行，
//! 只列标题、应用时相当于收起（server 侧自定义命令由应用层自行分发）；
//! 文件创建/重命名/删除类 `documentChanges` 操作不接；布局未就绪（首绘前）请求
//! 自动忽略，避免弹到左上角；诊断按 `DiagnosticEntry` 反查为 LSP 形态喂给
//! provider（`code` 以字符串回传，数字编码原形丢失）。

use std::ops::Range;
use std::rc::Rc;

use lsp_types::{CodeAction, CodeActionOrCommand, DocumentChanges, OneOf, TextEdit, Uri};
use ropey::Rope;

use crate::lsp::{DiagnosticEntry, PositionMapping};
use crate::prelude::FluentBuilder as _;
use crate::{
    ActiveTheme as _, Anchor, AnyElement, App, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, Pixels, Point, SharedString, StatefulInteractiveElement, Styled, StyledExt,
    Task, Window, anchored, deferred, div, h_flex, px,
};

use super::state::EditorState;

/// 修复菜单单屏最多显示项数（同时是列表存储上限，provider 应自行裁剪）。
const MAX_VISIBLE_ACTIONS: usize = 10;

/// 快速修复 provider。
///
/// 实现此 trait 即可为编辑器提供 Code Action；应用层通常经 LSP
/// （`textDocument/codeAction`）实现，手动构造修复项亦可。
pub trait CodeActionProvider {
    /// 请求修复项。
    ///
    /// # 参数
    /// * `text` - 当前文档快照
    /// * `range` - 选区（UTF-8 字节偏移；无选区时是光标处的空区间）
    /// * `diagnostics` - 当前文档诊断（LSP 形态，供 `CodeActionContext` 组装）
    /// * `window` - 窗口引用
    /// * `cx` - 应用上下文
    fn code_actions(
        &self,
        text: &Rope,
        range: Range<usize>,
        diagnostics: Vec<lsp_types::Diagnostic>,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Vec<CodeActionOrCommand>>> {
        let _ = (text, range, diagnostics, window, cx);
        Task::ready(Ok(Vec::new()))
    }
}

/// 修复接入状态（`EditorState` 内嵌小字段）。
pub(super) struct CodeActionsState {
    /// provider（应用注入，传输自理；`None` 即关闭）。
    provider: Option<Rc<dyn CodeActionProvider>>,
    /// 当前修复列表（渲染源）。
    actions: Vec<CodeAction>,
    /// 当前选中下标。
    selected: usize,
    /// 菜单是否可见。
    visible: bool,
    /// 菜单锚点（窗口坐标，光标处左下角）。
    position: Option<Point<Pixels>>,
    /// 请求 epoch（作废旧请求，M2 同款）。
    epoch: u64,
    /// 在途请求任务（持有防取消）。
    _task: Option<Task<()>>,
}

impl CodeActionsState {
    pub(super) fn new() -> Self {
        Self {
            provider: None,
            actions: Vec::new(),
            selected: 0,
            visible: false,
            position: None,
            epoch: 0,
            _task: None,
        }
    }

    /// 下一请求 epoch（旧请求看到 epoch 不一致即作废）。
    fn next_epoch(&mut self) -> u64 {
        self.epoch = self.epoch.wrapping_add(1);
        self.epoch
    }

    /// 清空列表并收起（断开 provider/文本变更/应用后共用）。
    fn clear(&mut self) {
        self.actions.clear();
        self.selected = 0;
        self.visible = false;
    }
}

/// `CodeActionOrCommand` → `CodeAction`（命令型动作补一条无编辑的条目，只列标题）。
fn normalize_action(action: CodeActionOrCommand) -> CodeAction {
    match action {
        CodeActionOrCommand::CodeAction(action) => action,
        CodeActionOrCommand::Command(command) => CodeAction {
            title: command.title,
            ..Default::default()
        },
    }
}

/// 诊断反查为 LSP 形态（provider 侧要组 `CodeActionContext.diagnostics`）。
fn to_lsp_diagnostic(entry: &DiagnosticEntry) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic {
        range: entry.range,
        severity: Some(entry.severity),
        code: entry.code.clone().map(lsp_types::NumberOrString::String),
        source: entry.source.clone(),
        message: entry.message.clone(),
        tags: (!entry.tags.is_empty()).then(|| {
            entry
                .tags
                .iter()
                .copied()
                .map(|tag| match tag {
                    crate::lsp::DiagnosticTag::Unnecessary => lsp_types::DiagnosticTag::UNNECESSARY,
                    crate::lsp::DiagnosticTag::Deprecated => lsp_types::DiagnosticTag::DEPRECATED,
                })
                .collect()
        }),
        related_information: (!entry.related_information.is_empty()).then(|| {
            entry
                .related_information
                .iter()
                .map(|info| lsp_types::DiagnosticRelatedInformation {
                    location: info.location.clone(),
                    message: info.message.clone(),
                })
                .collect()
        }),
        code_description: None,
        data: None,
    }
}

/// 取动作里属于当前文档的文本编辑。
///
/// `changes` 表与 `documentChanges` 两条路都收进同一条列表再按 URI 命中；命中不到
/// （多文件修复的别家文件）即返回空，v1 不跨文档写盘。
fn edits_for_action(action: &CodeAction, uri: Option<&Uri>) -> Vec<TextEdit> {
    let Some(edit) = &action.edit else {
        return Vec::new();
    };
    let mut docs: Vec<(&Uri, Vec<TextEdit>)> = Vec::new();
    if let Some(changes) = &edit.changes {
        docs.extend(changes.iter().map(|(doc, edits)| (doc, edits.clone())));
    }
    if let Some(DocumentChanges::Edits(document_edits)) = &edit.document_changes {
        for document_edit in document_edits {
            let edits = document_edit
                .edits
                .iter()
                .map(|edit| match edit {
                    OneOf::Left(edit) => edit.clone(),
                    OneOf::Right(annotated) => annotated.text_edit.clone(),
                })
                .collect();
            docs.push((&document_edit.text_document.uri, edits));
        }
    }
    match uri {
        Some(uri) => docs
            .into_iter()
            .find(|(doc, _)| *doc == uri)
            .map(|(_, edits)| edits)
            .unwrap_or_default(),
        // 未设文档 URI：只在编辑确实只涉及一个文档时应用，避免猜错目标。
        None if docs.len() == 1 => docs
            .into_iter()
            .next()
            .map(|(_, edits)| edits)
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

/// 默认选中项：server 标了 `is_preferred` 的排前，否则 0。
fn preferred_index(actions: &[CodeAction]) -> usize {
    actions
        .iter()
        .position(|action| action.is_preferred == Some(true))
        .unwrap_or(0)
}

impl EditorState {
    /// 设置修复 provider（传 `None` 断开并收起菜单）。
    pub fn set_code_action_provider(
        &mut self,
        provider: Option<Rc<dyn CodeActionProvider>>,
        cx: &mut Context<Self>,
    ) {
        self.code_actions.provider = provider;
        if self.code_actions.provider.is_none() {
            self.code_actions.next_epoch();
            self.code_actions.clear();
            cx.notify();
        }
    }

    /// 当前修复列表（渲染源；应用层可读标题自行渲染）。
    pub fn code_actions(&self) -> &[CodeAction] {
        &self.code_actions.actions
    }

    /// 修复菜单是否处于可交互状态（可见且有选中项，键盘接管的判断依据）。
    pub fn code_action_menu_active(&self) -> bool {
        self.code_actions.visible
            && self
                .code_actions
                .actions
                .get(self.code_actions.selected)
                .is_some()
    }

    /// 请求修复项（按当前选区；无 provider 或布局未就绪时直接返回）。
    ///
    /// 用户显式触发（`Alt+Enter`），不设防抖；epoch 保证连按只认最后一次。
    pub fn request_code_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(provider) = self.code_actions.provider.clone() else {
            return;
        };
        // 锚点取光标处渲染边界左下角；首绘前无布局即放弃（免得弹到左上角）。
        let Some(position) = self.input.read_with(cx, |state, _| {
            let cursor = state.cursor().min(state.text().len());
            state
                .range_to_bounds(&(cursor..cursor))
                .map(|bounds| bounds.bottom_left())
        }) else {
            return;
        };
        let (text, range) = self.input.read_with(cx, |state, _| {
            let len = state.text().len();
            let selected = state.selected_range();
            (
                state.text().clone(),
                selected.start.min(len)..selected.end.min(len),
            )
        });
        let diagnostics = self
            .diagnostics()
            .iter()
            .map(to_lsp_diagnostic)
            .collect::<Vec<_>>();
        let epoch = self.code_actions.next_epoch();
        self.code_actions.position = Some(position);
        self.code_actions._task = Some(cx.spawn_in(window, async move |this, cx| {
            let task = this
                .update_in(cx, |this, window, cx| {
                    if this.code_actions.epoch != epoch {
                        return None;
                    }
                    Some(provider.code_actions(&text, range, diagnostics, window, cx))
                })
                .ok()
                .flatten();
            let Some(task) = task else { return };
            let response = task.await;
            let _ = this.update_in(cx, |this, _, cx| {
                if this.code_actions.epoch != epoch {
                    return;
                }
                match response {
                    Ok(actions) => {
                        this.code_actions.actions = actions
                            .into_iter()
                            .map(normalize_action)
                            .take(MAX_VISIBLE_ACTIONS)
                            .collect();
                        this.code_actions.selected = preferred_index(&this.code_actions.actions);
                        this.code_actions.visible = !this.code_actions.actions.is_empty();
                    }
                    Err(_) => this.code_actions.clear(),
                }
                cx.notify();
            });
        }));
    }

    /// `Alt+Enter`：菜单开着就收起，否则按当前选区请求一次。
    pub fn toggle_code_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.code_actions.visible {
            self.dismiss_code_actions(cx);
        } else {
            self.request_code_actions(window, cx);
        }
    }

    /// 收起修复菜单（清空列表 + epoch 作废在途请求；本就空着时不触发重绘）。
    pub fn dismiss_code_actions(&mut self, cx: &mut Context<Self>) {
        if self.code_actions.actions.is_empty() && !self.code_actions.visible {
            return;
        }
        self.code_actions.next_epoch();
        self.code_actions.clear();
        cx.notify();
    }

    /// 修复选中下一项（菜单隐藏时无操作）。
    pub fn select_next_code_action(&mut self, cx: &mut Context<Self>) {
        if !self.code_actions.visible || self.code_actions.actions.is_empty() {
            return;
        }
        let len = self.code_actions.actions.len();
        self.code_actions.selected = (self.code_actions.selected + 1) % len;
        cx.notify();
    }

    /// 修复选中上一项（菜单隐藏时无操作）。
    pub fn select_previous_code_action(&mut self, cx: &mut Context<Self>) {
        if !self.code_actions.visible || self.code_actions.actions.is_empty() {
            return;
        }
        self.code_actions.selected = if self.code_actions.selected == 0 {
            self.code_actions.actions.len() - 1
        } else {
            self.code_actions.selected - 1
        };
        cx.notify();
    }

    /// 应用修复（默认当前选中项；下标越界即收起）。
    ///
    /// 编辑按起始偏移倒序应用（改后面的不挪动前面的字节位置），LSP 行列按
    /// UTF-8 字节列换算（与诊断下划线同款口径）；应用后光标回到原位置。
    pub fn accept_code_action(
        &mut self,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = index.unwrap_or(self.code_actions.selected);
        let Some(action) = self.code_actions.actions.get(index).cloned() else {
            self.dismiss_code_actions(cx);
            return;
        };
        let edits = edits_for_action(&action, self.document_uri());
        // 先收起（列表即将过期，文本变更钩子也不会再兜到），再落编辑。
        self.dismiss_code_actions(cx);
        if edits.is_empty() {
            return;
        }
        let caret = self.input.read_with(cx, |state, _| state.cursor());
        let offsets: Vec<(Range<usize>, SharedString)> = self.input.read_with(cx, |state, _| {
            let text = state.text();
            edits
                .iter()
                .map(|edit| {
                    (
                        PositionMapping::position_to_offset(text, edit.range.start)
                            ..PositionMapping::position_to_offset(text, edit.range.end),
                        SharedString::from(edit.new_text.clone()),
                    )
                })
                .collect()
        });
        let mut offsets = offsets;
        // 倒序应用：靠前编辑的字节位置不受后面改动影响。
        offsets.sort_by(|a, b| b.0.start.cmp(&a.0.start));
        for (range, new_text) in offsets {
            self.input.update(cx, |state, cx| {
                state.set_selected_range(range, cx);
                state.replace(new_text, window, cx);
            });
        }
        let caret = caret.min(self.input.read_with(cx, |state, _| state.text().len()));
        self.input
            .update(cx, |state, cx| state.set_selected_range(caret..caret, cx));
    }
}

/// 修复菜单浮层（`Editor` 自带渲染；点行走 [`accept_code_action`](EditorState::accept_code_action)）。
///
/// 与补全弹窗同款定位（窗口坐标锚光标处左下角，`snap_to_window` 防溢出）；
/// 菜单不可见时返回 `None`（零开销）。
pub(super) fn render_code_action_menu(
    editor: &Entity<EditorState>,
    cx: &mut App,
) -> Option<AnyElement> {
    let (actions, selected, position) = editor.read_with(cx, |state, _| {
        if !state.code_actions.visible {
            return (Vec::new(), 0, None);
        }
        (
            state.code_actions.actions.clone(),
            state.code_actions.selected,
            state.code_actions.position,
        )
    });
    let position = position?;
    if actions.is_empty() {
        return None;
    }
    let rows: Vec<AnyElement> = actions
        .into_iter()
        .enumerate()
        .map(|(ix, action)| {
            let is_selected = ix == selected;
            let title: SharedString = action.title.into();
            let kind = action.kind.as_ref().map(|kind| {
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(
                        kind.as_str().rsplit('.').next().unwrap_or("").to_string(),
                    ))
            });
            // 带诊断的条目（quick fix）右侧标一下，与重构类区分。
            let fix_tag = action
                .diagnostics
                .as_ref()
                .map_or(0, |list| list.len())
                .min(1);
            let tag = (fix_tag > 0).then(|| {
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .ml_auto()
                    .child("quick fix")
            });
            let row = h_flex()
                .id(("code-action", ix))
                .w_full()
                .px_2()
                .py_1()
                .gap_2()
                .items_center()
                .text_color(cx.theme().popover_foreground)
                .rounded(cx.theme().radius.min(px(6.)))
                .when(is_selected, |this| {
                    this.bg(cx.theme().tokens.accent)
                        .text_color(cx.theme().accent_foreground)
                })
                .hover(|this| this.bg(cx.theme().tokens.accent.opacity(0.5)))
                .child(title)
                .children(kind)
                .children(tag);
            let editor = editor.clone();
            row.cursor_pointer()
                .on_click(move |_, window, cx| {
                    editor.update(cx, |state, cx| {
                        state.accept_code_action(Some(ix), window, cx);
                    });
                })
                .into_any_element()
        })
        .collect();
    Some(
        deferred(
            anchored()
                .position(position)
                .anchor(Anchor::TopLeft)
                .snap_to_window_with_margin(px(8.))
                .child(
                    div()
                        .w(px(320.))
                        .max_h(px(300.))
                        .overflow_hidden()
                        .popover_style(cx)
                        .p_1()
                        .flex()
                        .flex_col()
                        .gap_y_0p5()
                        .occlude()
                        .children(rows),
                ),
        )
        .with_priority(1)
        .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppContext as _, Render};
    use std::time::Duration;

    /// 持有编辑器状态的测试宿主视图。
    struct Probe {
        state: Entity<EditorState>,
    }

    impl Render for Probe {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> impl crate::IntoElement {
            crate::div()
        }
    }

    /// 假修复 provider：按选区给一条「换成 bar」的当前文档编辑。
    struct FakeCodeActionProvider;

    impl CodeActionProvider for FakeCodeActionProvider {
        fn code_actions(
            &self,
            text: &Rope,
            range: Range<usize>,
            _diagnostics: Vec<lsp_types::Diagnostic>,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Task<anyhow::Result<Vec<CodeActionOrCommand>>> {
            let start = PositionMapping::offset_to_position(text, range.start);
            let end = PositionMapping::offset_to_position(text, range.end);
            let uri: Uri = "file:///demo.rs".parse().unwrap();
            let edit = lsp_types::WorkspaceEdit {
                changes: Some(std::collections::HashMap::from([(
                    uri,
                    vec![TextEdit {
                        range: lsp_types::Range::new(start, end),
                        new_text: "bar".into(),
                    }],
                )])),
                ..Default::default()
            };
            Task::ready(Ok(vec![CodeActionOrCommand::CodeAction(CodeAction {
                title: "换成 bar".into(),
                kind: Some(lsp_types::CodeActionKind::QUICKFIX),
                edit: Some(edit),
                ..Default::default()
            })]))
        }
    }

    /// 泵：虚拟时钟快进 + 执行器跑空。
    fn pump(cx: &mut crate::TestAppContext) {
        cx.dispatcher.advance_clock(Duration::from_millis(2000));
        cx.run_until_parked();
        cx.background_executor.run_until_parked();
        cx.run_until_parked();
    }

    /// 建带 provider 的编辑器测试窗口（三个用例共用桩代码）。
    fn probe_with_provider<'a>(
        cx: &'a mut crate::TestAppContext,
    ) -> (Entity<EditorState>, &'a mut crate::VisualTestContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| {
                let mut state = EditorState::new(window, cx, "fn foo() {}");
                state.set_code_action_provider(Some(Rc::new(FakeCodeActionProvider)), cx);
                state
            });
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        (editor, cx)
    }

    /// 请求出菜单（有 provider + 有布局）。
    #[rgpui::test]
    fn code_action_request_shows_menu(cx: &mut crate::TestAppContext) {
        let (editor, cx) = probe_with_provider(cx);
        assert!(!editor.read_with(cx, |state, _| state.code_action_menu_active()));
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| state.request_code_actions(window, cx));
        });
        pump(cx);
        assert!(
            editor.read_with(cx, |state, _| state.code_action_menu_active()),
            "请求后菜单应激活"
        );
        assert_eq!(
            editor.read_with(cx, |state, _| state.code_actions().len()),
            1
        );
    }

    /// 应用修复：文本替换 + 菜单收起。
    #[rgpui::test]
    fn code_action_accept_edits_text(cx: &mut crate::TestAppContext) {
        let (editor, cx) = probe_with_provider(cx);
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_selected_range(3..6, cx);
                state.request_code_actions(window, cx);
            });
        });
        pump(cx);
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| state.accept_code_action(None, window, cx));
        });
        assert_eq!(
            editor.read_with(cx, |state, cx| state.text(cx)),
            "fn bar() {}"
        );
        assert!(!editor.read_with(cx, |state, _| state.code_action_menu_active()));
    }

    /// 断开 provider 收起菜单。
    #[rgpui::test]
    fn code_action_disconnect_clears(cx: &mut crate::TestAppContext) {
        let (editor, cx) = probe_with_provider(cx);
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| state.request_code_actions(window, cx));
        });
        pump(cx);
        assert!(editor.read_with(cx, |state, _| state.code_action_menu_active()));
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| state.set_code_action_provider(None, cx));
        });
        assert!(!editor.read_with(cx, |state, _| state.code_action_menu_active()));
    }

    /// 文本变更收起菜单（`Alt+Enter` 后继续打字，菜单不留僵尸）。
    #[rgpui::test]
    fn code_action_text_change_dismisses(cx: &mut crate::TestAppContext) {
        let (editor, cx) = probe_with_provider(cx);
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| state.request_code_actions(window, cx));
        });
        pump(cx);
        assert!(editor.read_with(cx, |state, _| state.code_action_menu_active()));
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                let input = state.input().clone();
                input.update(cx, |state, cx| {
                    crate::EntityInputHandler::replace_text_in_range(state, None, "x", window, cx);
                });
            });
        });
        pump(cx);
        assert!(!editor.read_with(cx, |state, _| state.code_action_menu_active()));
    }
}
