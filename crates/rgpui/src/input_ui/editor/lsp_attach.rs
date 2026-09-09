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
    /// 输入时自动请求补全（默认关；开后文本变更触发，见 `maybe_auto_complete`）。
    auto_completion: bool,
    /// 自动补全的单词前缀最小长度（默认 2；触发字符不受此限制）。
    auto_min_prefix_len: usize,
    /// 确认补全后的写入会触发一次 `Change`，此标记将其消费掉，避免弹窗刚收起又弹出。
    auto_suppress_once: bool,
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
            auto_completion: false,
            auto_min_prefix_len: 2,
            auto_suppress_once: false,
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

/// 补全单词字符（光标前后扫描与确认替换共用；`.`/`:` 是触发字符，不算词内）。
fn is_completion_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// 光标前连续单词的起始字节偏移（确认替换与自动触发的前缀长度共用）。
fn word_start_before(text: &ropey::Rope, cursor: usize) -> usize {
    let mut start = 0;
    let mut off = 0;
    for ch in text.slice(..cursor).chars() {
        off += ch.len_utf8();
        if !is_completion_word_char(ch) {
            start = off;
        }
    }
    start
}

/// 光标处单词的字节范围（确认补全时整体替换，避免 `pr` + `print` 叠成 `prprint`）。
///
/// 后半词一并吃掉（`print|ln` 确认后不留 `ln` 尾巴，与 VSCode 默认一致）；
/// `obj.pr` 只换 `pr`（`.` 不算词内）。
fn word_range_at(text: &ropey::Rope, cursor: usize) -> std::ops::Range<usize> {
    let len = text.len();
    let cursor = cursor.min(len);
    // 前半：最后一个非词字符之后的位置。
    let start = word_start_before(text, cursor);
    // 后半：遇到非词字符即停。
    let mut end = cursor;
    for ch in text.slice(cursor..).chars() {
        if !is_completion_word_char(ch) {
            break;
        }
        end += ch.len_utf8();
    }
    start..end
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

    /// 设置输入时是否自动请求补全（默认关；开后文本变更触发，防抖沿用补全通道）。
    ///
    /// 关开关时若弹窗正开着会一并收起；打开仅影响后续输入，不立即请求一次。
    pub fn set_auto_completion_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.lsp.auto_completion = enabled;
        if !enabled {
            self.dismiss_completion(cx);
        } else {
            cx.notify();
        }
    }

    /// 输入时是否自动请求补全。
    pub fn auto_completion_enabled(&self) -> bool {
        self.lsp.auto_completion
    }

    /// 设置自动补全的单词前缀最小长度（默认 2；触发字符不受此限制）。
    ///
    /// 键入第 1 个字母不弹、凑够长度才弹；设为 1 即恢复“任何单词输入都弹”。
    pub fn set_auto_completion_min_prefix_len(&mut self, len: usize, cx: &mut Context<Self>) {
        self.lsp.auto_min_prefix_len = len;
        cx.notify();
    }

    /// 自动补全的单词前缀最小长度。
    pub fn auto_completion_min_prefix_len(&self) -> usize {
        self.lsp.auto_min_prefix_len
    }

    /// 文本变更后的自动补全钩子（`EditorState::new` 经 `subscribe_in` 接线，有 window）。
    ///
    /// 触发条件（需开关开 + 有 provider）：
    /// - 触发字符：光标前是 `.`/`:` 或 provider 的 `trigger_characters`，立即请求；
    /// - 单词字符：光标前连续单词长度达到最小前缀长度才请求（默认 2，单个字母不弹）。
    /// 空格/换行等落到 else 分支：弹窗开着就收起。
    /// 确认补全刚写入的一次变更会被 `auto_suppress_once` 吃掉，避免刚收起又弹出。
    pub(crate) fn maybe_auto_complete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(provider) = self.lsp.completion_provider.clone() else {
            return;
        };
        if !self.lsp.auto_completion {
            return;
        }
        if self.lsp.auto_suppress_once {
            self.lsp.auto_suppress_once = false;
            return;
        }
        let (last_char, prefix_len) = self.input.read_with(cx, |state, _| {
            // 光标是 UTF-8 字节偏移（库内 `slice` 均按字节，随 `auto_close` 同款写法）。
            let cursor = state.cursor().min(state.text().len());
            let text = state.text();
            let last = text.slice(..cursor).chars().last();
            let prefix_len = cursor - word_start_before(text, cursor);
            (last, prefix_len)
        });
        let is_trigger_char = last_char.is_some_and(|c| {
            c == '.'
                || c == ':'
                || provider
                    .trigger_characters()
                    .iter()
                    .any(|t| *t == c.to_string())
        });
        let long_enough_word = last_char.is_some_and(is_completion_word_char)
            && prefix_len >= self.lsp.auto_min_prefix_len;
        if is_trigger_char || long_enough_word {
            self.request_completions(window, cx);
        } else if self.lsp.completion.visible {
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

    /// 补全状态同步到弹窗状态（选中/可见/列表/选项全量同步 + 光标锚点）。
    ///
    /// 锚点取光标处渲染边界左下角（窗口坐标，供 `CompletionPopup` 经
    /// `deferred` + `anchored` 定位）；布局未就绪时保留上次位置，避免闪到左上角。
    fn sync_completion_popup(&self, cx: &mut App) {
        let popup = self.lsp.popup.clone();
        let anchor = self.input.read(cx).cursor();
        let anchor = self
            .input
            .read(cx)
            .range_to_bounds(&(anchor..anchor))
            .map(|bounds| bounds.bottom_left());
        let completion = &self.lsp.completion;
        let _ = popup.update(cx, |popup, cx| {
            popup.update_from_state(completion);
            if let Some(anchor) = anchor {
                popup.position = anchor;
            }
            cx.notify();
        });
    }

    /// 确认补全（默认当前选中）。
    ///
    /// `insertTextFormat == Snippet` 的条目走 `expand_snippet`（M3 联动），
    /// 其余把光标处单词整体替换为插入文本（`pr` 确认 `print` 得 `print` 而非 `prprint`）。
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
        // 确认写入会触发 `Change`，先标记抑制一次，免得弹窗刚收起又被自动补全唤起。
        self.lsp.auto_suppress_once = true;
        if item.lsp_item.insert_text_format == Some(InsertTextFormat::SNIPPET) {
            self.expand_snippet(item.insert_text.as_str(), window, cx);
            self.dismiss_completion(cx);
            return;
        }
        let insert_text = item.insert_text.clone();
        let word_range = self
            .input
            .read_with(cx, |state, _| word_range_at(state.text(), state.cursor()));
        self.input.update(cx, |state, cx| {
            state.set_selected_range(word_range, cx);
            state.replace(insert_text.as_str(), window, cx);
        });
        self.dismiss_completion(cx);
    }

    /// 收起补全（清空列表 + 同步弹窗）。
    pub fn dismiss_completion(&mut self, cx: &mut Context<Self>) {
        self.lsp.completion.clear();
        self.sync_completion_popup(cx);
        cx.notify();
    }

    /// 补全菜单是否处于可交互状态（可见且有选中项，键盘接管的判断依据）。
    ///
    /// 可见性蕴含非空（`apply_completion_response` 保证），再判选中是防越界吞键。
    pub fn completion_menu_active(&self) -> bool {
        self.lsp.completion.visible && self.lsp.completion.selected().is_some()
    }

    /// 补全选中下一项（弹窗隐藏时无操作，供键盘上下键调用）。
    pub fn select_next_completion(&mut self, cx: &mut Context<Self>) {
        if !self.lsp.completion.visible {
            return;
        }
        self.lsp.completion.select_next();
        self.sync_completion_popup(cx);
        cx.notify();
    }

    /// 补全选中上一项（弹窗隐藏时无操作，供键盘上下键调用）。
    pub fn select_previous_completion(&mut self, cx: &mut Context<Self>) {
        if !self.lsp.completion.visible {
            return;
        }
        self.lsp.completion.select_previous();
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

    /// 在内部输入上模拟一次键入（走真实 `Change` 事件，自动补全经订阅触发）。
    fn type_text(editor: &Entity<EditorState>, text: &str, cx: &mut crate::VisualTestContext) {
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                let input = state.input().clone();
                input.update(cx, |state, cx| {
                    crate::EntityInputHandler::replace_text_in_range(state, None, text, window, cx);
                });
            });
        });
    }

    fn completion_visible(editor: &Entity<EditorState>, cx: &mut crate::VisualTestContext) -> bool {
        editor.read_with(cx, |state, _| state.completion_state().visible)
    }

    fn selected_index(editor: &Entity<EditorState>, cx: &mut crate::VisualTestContext) -> usize {
        editor.read_with(cx, |state, _| state.completion_state().selected_index)
    }

    /// 开关默认关：键入不触发补全。
    #[rgpui::test]
    fn auto_completion_off_by_default(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn "));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        assert!(!editor.read_with(cx, |state, _| state.auto_completion_enabled()));
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.set_completion_provider(Some(Rc::new(FakeCompletionProvider)), cx);
            });
        });
        type_text(&editor, "p", cx);
        pump(cx);
        assert!(!completion_visible(&editor, cx));
    }

    /// 开关开时键入单词字符自动弹出，空格收起。
    #[rgpui::test]
    fn auto_completion_triggers_on_word_input(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn "));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.set_completion_provider(Some(Rc::new(FakeCompletionProvider)), cx);
                state.set_auto_completion_enabled(true, cx);
            });
        });
        assert!(editor.read_with(cx, |state, _| state.auto_completion_enabled()));
        // 单个字母不够最小前缀长度（默认 2）：不弹。
        type_text(&editor, "p", cx);
        pump(cx);
        assert!(!completion_visible(&editor, cx));
        // 凑够 `pr` 才弹。
        type_text(&editor, "r", cx);
        pump(cx);
        assert!(completion_visible(&editor, cx));
        // 空格不是单词字符：弹窗收起。
        type_text(&editor, " ", cx);
        pump(cx);
        assert!(!completion_visible(&editor, cx));
    }

    /// 触发字符不受最小前缀长度限制（`fn ` 后键入 `.` 即弹）。
    #[rgpui::test]
    fn auto_completion_trigger_char_ignores_threshold(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn "));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.set_completion_provider(Some(Rc::new(FakeCompletionProvider)), cx);
                state.set_auto_completion_enabled(true, cx);
            });
        });
        type_text(&editor, ".", cx);
        pump(cx);
        assert!(completion_visible(&editor, cx));
    }

    /// 最小前缀长度可配：设为 1 即恢复“任何单词输入都弹”。
    #[rgpui::test]
    fn auto_completion_min_prefix_len_configurable(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn "));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        assert_eq!(
            editor.read_with(cx, |state, _| state.auto_completion_min_prefix_len()),
            2
        );
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.set_completion_provider(Some(Rc::new(FakeCompletionProvider)), cx);
                state.set_auto_completion_enabled(true, cx);
                state.set_auto_completion_min_prefix_len(1, cx);
            });
        });
        type_text(&editor, "p", cx);
        pump(cx);
        assert!(completion_visible(&editor, cx));
    }

    /// 确认补全后的写入不立即重弹（抑制一次），之后键入恢复正常触发。
    #[rgpui::test]
    fn accept_completion_suppresses_auto_retrigger(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn "));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.set_completion_provider(Some(Rc::new(FakeCompletionProvider)), cx);
                state.set_auto_completion_enabled(true, cx);
            });
        });
        type_text(&editor, "pr", cx);
        pump(cx);
        assert!(completion_visible(&editor, cx));
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.accept_completion(None, window, cx);
            });
        });
        pump(cx);
        assert!(!completion_visible(&editor, cx));
        // 抑制标记已消费：继续键入重新触发。
        type_text(&editor, "x", cx);
        pump(cx);
        assert!(completion_visible(&editor, cx));
    }

    /// 关开关时正在展示的弹窗一并收起。
    #[rgpui::test]
    fn disabling_auto_completion_dismisses_popup(cx: &mut crate::TestAppContext) {
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
        assert!(completion_visible(&editor, cx));
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.set_auto_completion_enabled(false, cx);
            });
        });
        assert!(!completion_visible(&editor, cx));
    }

    /// 确认补全替换光标处单词前缀（`pr` 确认 `print` 得 `print` 而非 `prprint`）。
    #[rgpui::test]
    fn accept_completion_replaces_word_prefix(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn pr"));
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
        assert!(completion_visible(&editor, cx));
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                // 首项即 `println`。
                state.accept_completion(Some(0), window, cx);
            });
        });
        assert_eq!(
            editor.read_with(cx, |state, cx| state.text(cx)),
            "fn println"
        );
        assert!(!completion_visible(&editor, cx));
    }

    /// 确认补全连后半词一起吃掉（`pr|ln` 确认 `println` 不留 `ln` 尾巴）。
    #[rgpui::test]
    fn accept_completion_replaces_word_suffix(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "prln"));
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
        // 光标移到 `pr` 之后。
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.set_selected_range(2..2, cx);
            });
        });
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.accept_completion(Some(0), window, cx);
            });
        });
        assert_eq!(editor.read_with(cx, |state, cx| state.text(cx)), "println");
    }

    /// 补全选区上下移动（含首尾回绕；隐藏时无操作）。
    #[rgpui::test]
    fn completion_selection_moves_and_wraps(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn "));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        // 隐藏时移动无操作、不 panic。
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.select_next_completion(cx);
                state.select_previous_completion(cx);
            });
        });
        assert!(!completion_visible(&editor, cx));
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_completion_provider(Some(Rc::new(FakeCompletionProvider)), cx);
                state.request_completions(window, cx);
            });
        });
        pump(cx);
        assert!(editor.read_with(cx, |state, _| state.completion_menu_active()));
        assert_eq!(selected_index(&editor, cx), 0);
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.select_next_completion(cx);
            });
        });
        assert_eq!(selected_index(&editor, cx), 1);
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.select_next_completion(cx);
            });
        });
        // 两项回绕到 0。
        assert_eq!(selected_index(&editor, cx), 0);
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.select_previous_completion(cx);
            });
        });
        // 0 处上移回绕到末项。
        assert_eq!(selected_index(&editor, cx), 1);
    }
}
