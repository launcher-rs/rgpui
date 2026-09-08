//! LSP 编辑器侧接线（M2，`editor` feature 门控）。
//!
//! 拉模型：应用实现 provider trait（[`CompletionProvider`] /
//! [`DiagnosticsProvider`] / [`HoverProvider`]，传输层自理）后注入
//! `EditorState`；触发经 epoch 防抖（`cx.spawn_in` + 后台 timer，tooltip 同款，
//! 旧请求自动作废）；诊断经内部输入的独立装饰集合下划线渲染；补全状态同步到
//! `CompletionPopupState`（应用渲染 `CompletionPopup`，`on_select` 回写确认）。
//!
//! 边界（文档注明即契约）：推送模型不接（`on_diagnostics` 回调无 cx，进不了 UI
//! 线程，provider 侧需自行缓存、由拉模型取走）；LSP 行列按 UTF-8 字节列换算
//! （与 `Editor` 状态行一致，CJK 以字节计）；补全确认只做纯文本插入，snippet
//! 展开是 M3 的事；悬停浮层的位置锚定（鼠标/光标像素坐标）不做，状态 + 应用层
//! 渲染闭环，见 `editor` 演示页。

use std::rc::Rc;
use std::time::Duration;

use crate::lsp::{
    CompletionPopupState, CompletionProvider, CompletionState, DiagnosticEntry,
    DiagnosticsProvider, HoverProvider, HoverState, PositionMapping,
};
use crate::{App, Context, Entity, HighlightStyle, Hsla, Task, UnderlineStyle, Window, px};
use lsp_types::{
    CompletionContext, CompletionResponse, CompletionTriggerKind, DiagnosticSeverity,
    InsertTextFormat, Uri,
};

use super::super::decorations::{TextDecoration, normalize};
use super::state::EditorState;
use crate::input_ui::TextDecorationCollection;

/// 补全请求防抖（输入后等用户停手再问 provider）。
const COMPLETION_DEBOUNCE: Duration = Duration::from_millis(150);
/// 诊断请求防抖（比补全慢一拍，server 侧多为全量计算）。
const DIAGNOSTICS_DEBOUNCE: Duration = Duration::from_millis(500);
/// 悬停请求防抖（鼠标划过连续触发时只算最后一次）。
const HOVER_DEBOUNCE: Duration = Duration::from_millis(200);

/// LSP 接入状态（`EditorState` 内嵌小字段，不另起 Entity）。
pub(super) struct LspAttach {
    /// 补全 provider（应用注入，传输自理）。
    completion_provider: Option<Rc<dyn CompletionProvider>>,
    /// 诊断 provider（应用注入，传输自理）。
    diagnostics_provider: Option<Rc<dyn DiagnosticsProvider>>,
    /// 悬停 provider（应用注入，传输自理）。
    hover_provider: Option<Rc<dyn HoverProvider>>,
    /// 当前文档 URI（诊断请求按 URI 取数，未设置时不请求）。
    document_uri: Option<Uri>,
    /// 补全状态（选中/可见/列表）。
    completion: CompletionState,
    /// 补全弹窗状态（应用渲染 `CompletionPopup` 用，与 `completion` 同步）。
    popup: Entity<CompletionPopupState>,
    /// 当前文档诊断（装饰集合的渲染源）。
    diagnostics: Vec<DiagnosticEntry>,
    /// 诊断下划线装饰集合（内部输入上独立一层，见 [`TextDecorationCollection`]）。
    diagnostics_collection: Option<TextDecorationCollection>,
    /// 悬停状态。
    hover: HoverState,
    /// 请求 epoch（防抖作废旧请求用，tooltip 同款）。
    epoch: u64,
    /// 在途请求任务（持有防取消，tooltip 同款；新请求覆盖即取消旧请求）。
    _lsp_task: Option<Task<()>>,
}

impl LspAttach {
    /// 创建接入状态（补全弹窗状态需 cx 建实体）。
    pub(super) fn new(popup: Entity<CompletionPopupState>) -> Self {
        Self {
            completion_provider: None,
            diagnostics_provider: None,
            hover_provider: None,
            document_uri: None,
            completion: CompletionState::default(),
            popup,
            diagnostics: Vec::new(),
            diagnostics_collection: None,
            hover: HoverState::default(),
            epoch: 0,
            _lsp_task: None,
        }
    }

    /// 下一请求 epoch（旧请求看到 epoch 不一致即作废）。
    fn next_epoch(&mut self) -> u64 {
        self.epoch = self.epoch.wrapping_add(1);
        self.epoch
    }
}

/// 诊断严重程度 → 下划线颜色。
fn diagnostic_color(severity: DiagnosticSeverity) -> Hsla {
    match severity {
        DiagnosticSeverity::ERROR => crate::red_400(),
        DiagnosticSeverity::WARNING => crate::yellow_400(),
        _ => crate::gray_400(),
    }
}

/// 诊断严重程度 → 下划线形状（错误波浪，警告直线，其余点状）。
fn diagnostic_wavy(severity: DiagnosticSeverity) -> bool {
    severity == DiagnosticSeverity::ERROR
}

impl EditorState {
    /// 设置补全 provider（应用实现传输后注入；传 `None` 断开）。
    pub fn set_completion_provider(
        &mut self,
        provider: Option<Rc<dyn CompletionProvider>>,
        cx: &mut Context<Self>,
    ) {
        self.lsp.completion_provider = provider;
        if self.lsp.completion_provider.is_none() {
            self.dismiss_completion(cx);
        }
    }

    /// 设置诊断 provider（应用实现传输后注入；传 `None` 断开并清空诊断）。
    pub fn set_diagnostics_provider(
        &mut self,
        provider: Option<Rc<dyn DiagnosticsProvider>>,
        cx: &mut Context<Self>,
    ) {
        self.lsp.diagnostics_provider = provider;
        if self.lsp.diagnostics_provider.is_none() {
            self.lsp.next_epoch();
            self.lsp.diagnostics.clear();
            self.refresh_diagnostic_decorations(cx);
        }
    }

    /// 设置悬停 provider（应用实现传输后注入；传 `None` 断开并清空悬停）。
    pub fn set_hover_provider(
        &mut self,
        provider: Option<Rc<dyn HoverProvider>>,
        cx: &mut Context<Self>,
    ) {
        self.lsp.hover_provider = provider;
        if self.lsp.hover_provider.is_none() {
            self.lsp.next_epoch();
            self.lsp.hover.clear();
            cx.notify();
        }
    }

    /// 设置当前文档 URI（诊断请求按 URI 取数）。
    pub fn set_document_uri(&mut self, uri: Option<Uri>, cx: &mut Context<Self>) {
        self.lsp.document_uri = uri;
        cx.notify();
    }

    /// 当前补全状态（列表/选中/可见，应用层按需读取）。
    pub fn completion_state(&self) -> &CompletionState {
        &self.lsp.completion
    }

    /// 补全弹窗状态实体（应用渲染 `CompletionPopup::new` 用）。
    pub fn completion_popup(&self) -> &Entity<CompletionPopupState> {
        &self.lsp.popup
    }

    /// 当前文档诊断（装饰集合的渲染源，应用层可喂 `DiagnosticMarkersState`）。
    pub fn diagnostics(&self) -> &[DiagnosticEntry] {
        &self.lsp.diagnostics
    }

    /// 当前悬停状态（应用层渲染浮层用）。
    pub fn hover_state(&self) -> &HoverState {
        &self.lsp.hover
    }

    /// 请求补全（防抖内置：150ms 内重复调用只算最后一次）。
    ///
    /// 应用在有 window 的位置调用（如按键处理/订阅回调经 `spawn_in` 转交；
    /// `InputEvent::Change` 订阅无 window，调不动——这是 provider trait 要
    /// window 的代价，文档注明）。
    pub fn request_completions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(provider) = self.lsp.completion_provider.clone() else {
            return;
        };
        let epoch = self.lsp.next_epoch();
        let (text, offset) = self
            .input
            .read_with(cx, |state, _| (state.text().clone(), state.cursor()));
        // provider 要 `&Rope`：`text()` 返回 `&Rope`，`clone` 只是 Arc 递增，便宜。
        let trigger = CompletionContext {
            trigger_kind: CompletionTriggerKind::INVOKED,
            trigger_character: None,
        };
        self.lsp._lsp_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(COMPLETION_DEBOUNCE).await;
            let task = this
                .update_in(cx, |this, window, cx| {
                    if this.lsp.epoch != epoch {
                        return None;
                    }
                    Some(provider.completions(&text, offset, trigger, window, cx))
                })
                .ok()
                .flatten();
            let Some(task) = task else { return };
            let response = task.await;
            let _ = this.update_in(cx, |this, _, cx| {
                if this.lsp.epoch != epoch {
                    return;
                }
                match response {
                    Ok(response) => this.apply_completion_response(response, cx),
                    Err(_) => this.dismiss_completion(cx),
                }
            });
        }));
    }

    /// 应用补全响应（空列表即收起）。
    fn apply_completion_response(&mut self, response: CompletionResponse, cx: &mut Context<Self>) {
        let max_items = self
            .lsp
            .completion_provider
            .as_ref()
            .map(|p| p.menu_options().max_visible_items)
            .unwrap_or(15);
        let items = CompletionState::from_response(response, max_items);
        self.lsp.completion.completions = items;
        self.lsp.completion.selected_index = 0;
        self.lsp.completion.visible = !self.lsp.completion.completions.is_empty();
        self.sync_completion_popup(cx);
        cx.notify();
    }

    /// 补全状态同步到弹窗状态（选中/可见/列表/选项全量同步）。
    fn sync_completion_popup(&self, cx: &mut App) {
        let popup = self.lsp.popup.clone();
        let completion = &self.lsp.completion;
        let _ = popup.update(cx, |popup, cx| {
            popup.update_from_state(completion);
            cx.notify();
        });
    }

    /// 确认补全（默认当前选中）。
    ///
    /// `insertTextFormat == Snippet` 的条目走 `expand_snippet`（M3 联动），
    /// 其余纯文本插入光标处。
    pub fn accept_completion(
        &mut self,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = index.unwrap_or(self.lsp.completion.selected_index);
        let Some(item) = self.lsp.completion.completions.get(index).cloned() else {
            return;
        };
        if item.lsp_item.insert_text_format == Some(InsertTextFormat::SNIPPET) {
            self.expand_snippet(item.insert_text.as_str(), window, cx);
            self.dismiss_completion(cx);
            return;
        }
        self.input.update(cx, |state, cx| {
            state.insert(item.insert_text.as_str(), window, cx);
        });
        self.dismiss_completion(cx);
    }

    /// 收起补全（清空列表 + 同步弹窗）。
    pub fn dismiss_completion(&mut self, cx: &mut Context<Self>) {
        self.lsp.completion.clear();
        self.sync_completion_popup(cx);
        cx.notify();
    }

    /// 请求诊断（防抖内置 500ms；无 URI 或无 provider 时直接返回）。
    pub fn request_diagnostics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(provider) = self.lsp.diagnostics_provider.clone() else {
            return;
        };
        let Some(uri) = self.lsp.document_uri.clone() else {
            return;
        };
        let epoch = self.lsp.next_epoch();
        self.lsp._lsp_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(DIAGNOSTICS_DEBOUNCE).await;
            let task = this
                .update_in(cx, |this, window, cx| {
                    if this.lsp.epoch != epoch {
                        return None;
                    }
                    Some(provider.diagnostics(&uri, window, cx))
                })
                .ok()
                .flatten();
            let Some(task) = task else { return };
            let response = task.await;
            let _ = this.update_in(cx, |this, _, cx| {
                if this.lsp.epoch != epoch {
                    return;
                }
                if let Ok(entries) = response {
                    this.lsp.diagnostics = entries;
                    this.refresh_diagnostic_decorations(cx);
                }
            });
        }));
    }

    /// 诊断刷到内部输入的独立装饰集合（下划线；借用内原地写，bracket 同款）。
    ///
    /// 空诊断时清空集合（保留句柄供复用）；集合按创建顺序分层，诊断层后于
    /// 标黄/括号匹配创建，见 [`TextDecorationCollection`] 层级规则。
    fn refresh_diagnostic_decorations(&mut self, cx: &mut Context<Self>) {
        let diagnostics = self.lsp.diagnostics.clone();
        let existing = self.lsp.diagnostics_collection.clone();
        let mut created = None;
        self.input.update(cx, |state, cx| {
            let text = state.text().clone();
            // LSP 行列按 UTF-8 字节列换算（与 Editor 状态行一致，CJK 以字节计）。
            let decorations: Vec<TextDecoration> = diagnostics
                .iter()
                .map(|entry| {
                    let range = PositionMapping::position_to_offset(&text, entry.range.start)
                        ..PositionMapping::position_to_offset(&text, entry.range.end);
                    TextDecoration::new(
                        range,
                        HighlightStyle {
                            underline: Some(UnderlineStyle {
                                thickness: px(1.),
                                color: Some(diagnostic_color(entry.severity)),
                                wavy: diagnostic_wavy(entry.severity),
                            }),
                            ..Default::default()
                        },
                    )
                })
                .collect();
            if let Some(collection) = existing {
                let decorations = normalize(&text, decorations);
                if collection.set_in_place(&mut state.core.decorations, decorations) {
                    cx.notify();
                }
            } else if !decorations.is_empty() {
                created = Some(state.create_decorations_collection(decorations, cx));
            }
        });
        if let Some(collection) = created {
            self.lsp.diagnostics_collection = Some(collection);
        }
    }

    /// 请求悬停（防抖内置 200ms；无 provider 时直接返回）。
    ///
    /// 浮层位置锚定不做（鼠标/光标像素坐标需布局命中测试，v1 边界）；
    /// 应用读 [`hover_state`](Self::hover_state) 自行渲染，见 `editor` 演示页。
    pub fn request_hover(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(provider) = self.lsp.hover_provider.clone() else {
            return;
        };
        let epoch = self.lsp.next_epoch();
        let text = self.input.read_with(cx, |state, _| state.text().clone());
        self.lsp._lsp_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(HOVER_DEBOUNCE).await;
            let task = this
                .update_in(cx, |this, window, cx| {
                    if this.lsp.epoch != epoch {
                        return None;
                    }
                    Some(provider.hover(&text, offset, window, cx))
                })
                .ok()
                .flatten();
            let Some(task) = task else { return };
            let response = task.await;
            let _ = this.update_in(cx, |this, _, cx| {
                if this.lsp.epoch != epoch {
                    return;
                }
                match response {
                    Ok(response) => {
                        this.lsp.hover.response = response;
                        this.lsp.hover.offset = Some(offset);
                        this.lsp.hover.visible = this.lsp.hover.response.is_some();
                        cx.notify();
                    }
                    Err(_) => this.lsp.hover.clear(),
                }
            });
        }));
    }

    /// 清空悬停（鼠标移出等场景应用层调用）。
    pub fn dismiss_hover(&mut self, cx: &mut Context<Self>) {
        self.lsp.hover.clear();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppContext as _;
    use crate::Render;

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

    /// 假补全 provider（固定返回 `println` / `print` 两个条目）。
    struct FakeCompletionProvider;

    impl CompletionProvider for FakeCompletionProvider {
        fn completions(
            &self,
            _text: &ropey::Rope,
            _offset: usize,
            _trigger: CompletionContext,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Task<anyhow::Result<CompletionResponse>> {
            Task::ready(Ok(CompletionResponse::Array(vec![
                lsp_types::CompletionItem {
                    label: "println".to_string(),
                    ..Default::default()
                },
                lsp_types::CompletionItem {
                    label: "print".to_string(),
                    ..Default::default()
                },
            ])))
        }
    }

    /// 假诊断 provider（首行 0..5 一个 error）。
    struct FakeDiagnosticsProvider;

    impl DiagnosticsProvider for FakeDiagnosticsProvider {
        fn diagnostics(
            &self,
            _uri: &Uri,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Task<anyhow::Result<Vec<DiagnosticEntry>>> {
            use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
            Task::ready(Ok(vec![DiagnosticEntry::from_diagnostic(Diagnostic {
                range: Range {
                    start: Position::new(0, 0),
                    end: Position::new(0, 5),
                },
                severity: Some(DiagnosticSeverity::ERROR),
                message: "fake error".to_string(),
                ..Default::default()
            })]))
        }
    }

    /// 假悬停 provider（固定返回一段文本）。
    struct FakeHoverProvider;

    impl HoverProvider for FakeHoverProvider {
        fn hover(
            &self,
            _text: &ropey::Rope,
            offset: usize,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Task<anyhow::Result<Option<crate::lsp::HoverResponse>>> {
            Task::ready(Ok(Some(crate::lsp::HoverResponse {
                range: offset..offset,
                contents: vec![crate::lsp::HoverContent::Text("fn main".to_string())],
            })))
        }
    }

    /// 泵：虚拟时钟快进 + 前后台执行器跑空（防抖 timer 全为虚拟时间）。
    fn pump(cx: &mut crate::TestAppContext) {
        cx.dispatcher.advance_clock(Duration::from_millis(2000));
        cx.run_until_parked();
        cx.background_executor.run_until_parked();
        cx.run_until_parked();
    }

    fn test_uri() -> Uri {
        "file:///test.rs".parse().unwrap()
    }

    /// 补全请求填入弹窗状态（防抖后可见 + 两条目 + 选项同步）。
    #[rgpui::test]
    fn completion_request_fills_popup(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_completion_provider(Some(Rc::new(FakeCompletionProvider)), cx);
                state.request_completions(window, cx);
            });
        });
        pump(cx);
        let visible = editor.read_with(cx, |state, _| state.completion_state().visible);
        assert!(visible);
        let labels: Vec<String> = editor.read_with(cx, |state, _| {
            state
                .completion_state()
                .completions
                .iter()
                .map(|c| c.label.clone())
                .collect()
        });
        assert_eq!(labels, vec!["println".to_string(), "print".to_string()]);
        // 弹窗状态同步（应用渲染 CompletionPopup 用）。
        let popup_visible =
            editor.read_with(cx, |state, cx| state.completion_popup().read(cx).visible);
        assert!(popup_visible);
    }

    /// 确认补全插入文本并收起（纯文本插入，M3 前无 snippet 展开）。
    #[rgpui::test]
    fn accept_completion_inserts_text(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn "));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_completion_provider(Some(Rc::new(FakeCompletionProvider)), cx);
                state.request_completions(window, cx);
            });
        });
        pump(cx);
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.accept_completion(None, window, cx);
            });
        });
        assert_eq!(
            editor.read_with(cx, |state, cx| state.text(cx)),
            "fn println"
        );
        assert!(!editor.read_with(cx, |state, _| state.completion_state().visible));
    }

    /// 诊断请求落库 + 下划线装饰集合创建（错误波浪线）。
    #[rgpui::test]
    fn diagnostics_request_renders_underline(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_diagnostics_provider(Some(Rc::new(FakeDiagnosticsProvider)), cx);
                state.set_document_uri(Some(test_uri()), cx);
                state.request_diagnostics(window, cx);
            });
        });
        pump(cx);
        let count = editor.read_with(cx, |state, _| state.diagnostics().len());
        assert_eq!(count, 1);
        assert_eq!(
            editor.read_with(cx, |state, _| state.diagnostics()[0].message.clone()),
            "fake error"
        );
        // 装饰集合已创建（下划线渲染源就绪）。
        assert!(editor.read_with(cx, |state, _| state.lsp.diagnostics_collection.is_some()));
    }

    /// 悬停请求填入状态（可见 + 内容）。
    #[rgpui::test]
    fn hover_request_fills_state(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_hover_provider(Some(Rc::new(FakeHoverProvider)), cx);
                state.request_hover(0, window, cx);
            });
        });
        pump(cx);
        let (visible, text) = editor.read_with(cx, |state, _| {
            let hover = state.hover_state();
            let text = hover
                .response
                .as_ref()
                .map(|r| format!("{:?}", r.contents))
                .unwrap_or_default();
            (hover.visible, text)
        });
        assert!(visible);
        assert!(text.contains("fn main"));
    }
}
