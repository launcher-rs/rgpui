//! Inlay hints（M4，`editor` feature 门控）：行内虚拟文本（类型/参数提示）。
//!
//! 薄封装：provider trait（默认空实现，不断编译）→ `EditorState` 请求（epoch
//! 防抖，M2 同款）→ 存内部 `InputState` 字段 → paint 阶段 overlay 绘制。
//!
//! 三不（文档注明即契约）：不占布局（绘制在文本流之外，不影响换行/滚动尺寸）、
//! 不进 `Rope`（纯渲染层）、不参与选中复制（命中测试与选区管线碰不到它）。
//! 大文档只渲染可视行（provider 按可见范围取数 + paint 按可见行过滤）。
//! 无 provider 或开关关闭时零开销（不请求、不绘制、不存数）。

use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use ropey::Rope;

use crate::{App, Context, Task, Window};

use super::state::EditorState;

/// inlay 请求防抖（滚动/输入后连续触发只算最后一次）。
const INLAY_DEBOUNCE: Duration = Duration::from_millis(300);

/// 行内提示（虚拟文本：字节偏移 + 显示文本）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHint {
    /// 提示锚点（UTF-8 字节偏移；绘制在该偏移的 x 处同行）。
    pub offset: usize,
    /// 显示文本（单行；含换行只取首行，文档注明）。
    pub text: String,
}

/// 行内提示 provider。
///
/// 实现此 trait 即可为编辑器提供 inlay hints；应用层通常经 LSP
///（`textDocument/inlayHint`）实现，手动计算亦可。
pub trait InlayProvider {
    /// 请求可见范围内的提示。
    ///
    /// # 参数
    /// * `text` - 当前文档快照
    /// * `visible` - 可见字节范围（大文档只算此范围，文档注明）
    /// * `window` - 窗口引用
    /// * `cx` - 应用上下文
    fn inlay_hints(
        &self,
        text: &Rope,
        visible: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Vec<InlayHint>>> {
        let _ = (text, visible, window, cx);
        Task::ready(Ok(Vec::new()))
    }
}

/// inlay 接入状态（`EditorState` 内嵌小字段）。
pub(super) struct InlayState {
    /// provider（应用注入，传输自理）。
    provider: Option<Rc<dyn InlayProvider>>,
    /// 请求 epoch（防抖作废旧请求，M2 同款）。
    epoch: u64,
    /// 在途请求任务（持有防取消）。
    _task: Option<Task<()>>,
}

impl InlayState {
    pub(super) fn new() -> Self {
        Self {
            provider: None,
            epoch: 0,
            _task: None,
        }
    }

    fn next_epoch(&mut self) -> u64 {
        self.epoch = self.epoch.wrapping_add(1);
        self.epoch
    }
}

impl EditorState {
    /// 设置 inlay provider（传 `None` 断开并清空已存提示）。
    pub fn set_inlay_provider(
        &mut self,
        provider: Option<Rc<dyn InlayProvider>>,
        cx: &mut Context<Self>,
    ) {
        self.inlay.provider = provider;
        if self.inlay.provider.is_none() {
            self.inlay.next_epoch();
            self.clear_inlay_hints(cx);
        }
    }

    /// 设置 inlay 总开关（默认关，性能姿态保守；透传内部输入）。
    ///
    /// 关闭时清空已存提示（零渲染 + 零内存）。
    pub fn set_inlay_hints_enabled(&self, enabled: bool, cx: &mut App) {
        let _ = self.input.update(cx, |state, cx| {
            state.inlay_hints_enabled = enabled;
            if !enabled {
                state.inlay_hints.clear();
            }
            cx.notify();
        });
    }

    /// 当前是否开启 inlay。
    pub fn inlay_hints_enabled(&self, cx: &App) -> bool {
        self.input
            .read_with(cx, |state, _| state.inlay_hints_enabled)
    }

    /// 当前已存提示（渲染源；应用层按需读取）。
    pub fn inlay_hint_list(&self, cx: &App) -> Vec<InlayHint> {
        self.input
            .read_with(cx, |state, _| state.inlay_hints.clone())
    }

    /// 请求 inlay（防抖内置 300ms；关闭/无 provider/布局未就绪时直接返回）。
    ///
    /// 可见范围取内部输入上次布局（`visible_range_offset`）；首绘前无布局时
    /// 按全文请求（首屏 hints 不缺席，文档注明）。
    pub fn request_inlay_hints(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(provider) = self.inlay.provider.clone() else {
            return;
        };
        let (text, visible, enabled) = self.input.read_with(cx, |state, _| {
            let visible = state
                .last_layout
                .as_ref()
                .map(|layout| layout.visible_range_offset.clone())
                .unwrap_or(0..state.text().len());
            (state.text().clone(), visible, state.inlay_hints_enabled)
        });
        if !enabled {
            return;
        }
        let epoch = self.inlay.next_epoch();
        self.inlay._task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(INLAY_DEBOUNCE).await;
            let task = this
                .update_in(cx, |this, window, cx| {
                    if this.inlay.epoch != epoch {
                        return None;
                    }
                    Some(provider.inlay_hints(&text, visible, window, cx))
                })
                .ok()
                .flatten();
            let Some(task) = task else { return };
            let response = task.await;
            let _ = this.update_in(cx, |this, _, cx| {
                if this.inlay.epoch != epoch {
                    return;
                }
                if let Ok(hints) = response {
                    this.input.update(cx, |state, cx| {
                        state.inlay_hints = hints;
                        cx.notify();
                    });
                }
            });
        }));
    }

    /// 清空已存提示（内部用；断开 provider/关闭开关时调用）。
    fn clear_inlay_hints(&mut self, cx: &mut Context<Self>) {
        self.input.update(cx, |state, cx| {
            state.inlay_hints.clear();
            cx.notify();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppContext as _;
    use crate::{Entity, Render};

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

    /// 假 provider（固定返回两个提示）。
    struct FakeInlayProvider;

    impl InlayProvider for FakeInlayProvider {
        fn inlay_hints(
            &self,
            _text: &Rope,
            visible: Range<usize>,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Task<anyhow::Result<Vec<InlayHint>>> {
            assert!(!visible.is_empty());
            Task::ready(Ok(vec![
                InlayHint {
                    offset: 2,
                    text: ": i32".to_string(),
                },
                InlayHint {
                    offset: 5,
                    text: "-> ()".to_string(),
                },
            ]))
        }
    }

    /// 泵：虚拟时钟快进 + 前后台执行器跑空（防抖 timer 全为虚拟时间）。
    fn pump(cx: &mut crate::TestAppContext) {
        cx.dispatcher.advance_clock(Duration::from_millis(2000));
        cx.run_until_parked();
        cx.background_executor.run_until_parked();
        cx.run_until_parked();
    }

    /// 请求落库（开开关 + 可见范围非空 + 条目保留）。
    #[rgpui::test]
    fn inlay_request_stores_hints(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        assert!(!editor.read_with(cx, |state, cx| state.inlay_hints_enabled(cx)));
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_inlay_provider(Some(Rc::new(FakeInlayProvider)), cx);
                state.set_inlay_hints_enabled(true, cx);
                state.request_inlay_hints(window, cx);
            });
        });
        pump(cx);
        let hints = editor.read_with(cx, |state, cx| state.inlay_hint_list(cx));
        assert_eq!(hints.len(), 2);
        assert_eq!(hints[0].offset, 2);
        assert_eq!(hints[0].text, ": i32");
    }

    /// 关闭时不请求（零开销）+ 关闭清空已存。
    #[rgpui::test]
    fn disabled_skips_request_and_clears(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        // 默认关闭：请求直接返回。
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_inlay_provider(Some(Rc::new(FakeInlayProvider)), cx);
                state.request_inlay_hints(window, cx);
            });
        });
        pump(cx);
        assert!(
            editor
                .read_with(cx, |state, cx| state.inlay_hint_list(cx))
                .is_empty()
        );
        // 打开后落库，再关闭即清空。
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_inlay_hints_enabled(true, cx);
                state.request_inlay_hints(window, cx);
            });
        });
        pump(cx);
        assert_eq!(
            editor
                .read_with(cx, |state, cx| state.inlay_hint_list(cx))
                .len(),
            2
        );
        editor.update(cx, |state, cx| {
            state.set_inlay_hints_enabled(false, cx);
        });
        assert!(
            editor
                .read_with(cx, |state, cx| state.inlay_hint_list(cx))
                .is_empty()
        );
    }

    /// 断开 provider 即清空。
    #[rgpui::test]
    fn disconnect_clears_hints(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_inlay_provider(Some(Rc::new(FakeInlayProvider)), cx);
                state.set_inlay_hints_enabled(true, cx);
                state.request_inlay_hints(window, cx);
            });
        });
        pump(cx);
        assert_eq!(
            editor
                .read_with(cx, |state, cx| state.inlay_hint_list(cx))
                .len(),
            2
        );
        editor.update(cx, |state, cx| {
            state.set_inlay_provider(None, cx);
        });
        assert!(
            editor
                .read_with(cx, |state, cx| state.inlay_hint_list(cx))
                .is_empty()
        );
    }
}
