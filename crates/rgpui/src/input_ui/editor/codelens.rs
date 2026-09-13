//! CodeLens 行上透镜（O4，`editor` feature 门控）：引用数/测试运行等行级信息。
//!
//! 薄封装：provider trait（默认空实现，不断编译）→ `EditorState` 请求（epoch
//! 防抖，M2 同款）→ 存外壳小字段 → `CodeLensOverlay` 浮层绘制（deferred +
//! anchored，补全弹窗同款定位）。
//!
//! v1 约束（文档注明即契约）：只做单行文本透镜（多行折叠/参数透镜不做）；
//! 透镜行高固定、不挤占编辑区滚动（overlay 层，sticky 同款）；无 provider 零开销
//! （不请求、不绘制、不存数）；透镜锚在目标行首上方（`BottomLeft` 锚点），
//! 与上一行重叠时被覆盖，无避让（v1 约束）；点击回调由应用层经 overlay 注入。

use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use ropey::{LineType, Rope};

use crate::{
    ActiveTheme as _, Anchor, App, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement, Styled, Task, Window, anchored, deferred, div, px,
};

use super::state::EditorState;

/// 透镜请求防抖（编辑后连续触发只算最后一次，inlay 同款 300ms）。
const CODELENS_DEBOUNCE: Duration = Duration::from_millis(300);

/// 单屏最多绘制的透镜数（防抖之外的第二道保险，大文档 provider 应按可见范围取数）。
const MAX_VISIBLE_LENSES: usize = 20;

/// 行透镜（provider 输出：行号 + 标题；偏移由外壳存时换算）。
#[derive(Debug, Clone)]
pub struct CodeLens {
    /// 缓冲行号（0 起）。
    pub line: u32,
    /// 透镜标题（单行文本，如 `2 引用`、`▶ 运行`）。
    pub title: crate::SharedString,
}

/// 已落位透镜（渲染源：行号 + 行首字节偏移 + 标题）。
#[derive(Debug, Clone)]
pub struct ResolvedCodeLens {
    /// 缓冲行号（0 起）。
    pub line: u32,
    /// 行首 UTF-8 字节偏移（点击跳转/锚点定位用）。
    pub offset: usize,
    /// 透镜标题。
    pub title: crate::SharedString,
}

/// 行透镜 provider。
///
/// 实现此 trait 即可为编辑器提供 CodeLens；应用层通常经 LSP
/// （`textDocument/codeLens`）实现，手动计算亦可。
pub trait CodeLensProvider {
    /// 请求透镜（可见范围外可剪枝，大文档约束）。
    ///
    /// # 参数
    /// * `text` - 当前文档快照
    /// * `visible` - 可见字节范围（首绘前无布局时为全文，inlay 同款）
    /// * `window` - 窗口引用
    /// * `cx` - 应用上下文
    fn codelenses(
        &self,
        text: &Rope,
        visible: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Vec<CodeLens>>> {
        let _ = (text, visible, window, cx);
        Task::ready(Ok(Vec::new()))
    }
}

/// 透镜接入状态（`EditorState` 内嵌小字段）。
pub(super) struct CodelensState {
    /// provider（应用注入，传输自理；`None` 即关闭）。
    provider: Option<Rc<dyn CodeLensProvider>>,
    /// 已落位透镜（渲染源）。
    lenses: Vec<ResolvedCodeLens>,
    /// 请求 epoch（防抖作废旧请求，M2 同款）。
    epoch: u64,
    /// 在途请求任务（持有防取消）。
    _task: Option<Task<()>>,
}

impl CodelensState {
    pub(super) fn new() -> Self {
        Self {
            provider: None,
            lenses: Vec::new(),
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
    /// 设置透镜 provider（传 `None` 断开并清空已存透镜）。
    pub fn set_codelens_provider(
        &mut self,
        provider: Option<Rc<dyn CodeLensProvider>>,
        cx: &mut Context<Self>,
    ) {
        self.codelens.provider = provider;
        if self.codelens.provider.is_none() {
            self.codelens.next_epoch();
            self.codelens.lenses.clear();
            cx.notify();
        }
    }

    /// 当前已落位透镜（渲染源；按行号排序）。
    pub fn codelenses(&self) -> &[ResolvedCodeLens] {
        &self.codelens.lenses
    }

    /// 请求透镜（防抖内置 300ms；无 provider 时直接返回）。
    ///
    /// 文本变更管线自动调用（`subscribe_in` 回调，有 provider 即刷新）；
    /// 应用层也可手动调用（如切换分支后）。
    pub fn request_codelenses(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(provider) = self.codelens.provider.clone() else {
            return;
        };
        let (text, visible) = self.input.read_with(cx, |state, _| {
            let visible = state
                .last_layout
                .as_ref()
                .map(|layout| layout.visible_range_offset.clone())
                .unwrap_or(0..state.text().len());
            (state.text().clone(), visible)
        });
        let epoch = self.codelens.next_epoch();
        self.codelens._task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(CODELENS_DEBOUNCE).await;
            let task = this
                .update_in(cx, |this, window, cx| {
                    if this.codelens.epoch != epoch {
                        return None;
                    }
                    Some(provider.codelenses(&text, visible, window, cx))
                })
                .ok()
                .flatten();
            let Some(task) = task else { return };
            let response = task.await;
            let _ = this.update_in(cx, |this, _, cx| {
                if this.codelens.epoch != epoch {
                    return;
                }
                if let Ok(lenses) = response {
                    let current = this.input.read_with(cx, |state, _| state.text().clone());
                    this.codelens.lenses = Self::resolve_lenses(&current, lenses);
                    cx.notify();
                }
            });
        }));
    }

    /// 透镜落位：行号 → 行首字节偏移（越界行钳制到末行），按行号排序。
    fn resolve_lenses(text: &Rope, lenses: Vec<CodeLens>) -> Vec<ResolvedCodeLens> {
        let last_row = text.len_lines(LineType::LF).saturating_sub(1);
        let mut resolved: Vec<ResolvedCodeLens> = lenses
            .into_iter()
            .map(|lens| {
                let row = (lens.line as usize).min(last_row);
                let offset = text.line_to_byte_idx(row, LineType::LF).min(text.len());
                ResolvedCodeLens {
                    line: row as u32,
                    offset,
                    title: lens.title,
                }
            })
            .collect();
        resolved.sort_by_key(|lens| (lens.line, lens.offset));
        resolved
    }
}

/// 行透镜浮层（读 `EditorState` 渲染；点击行走 `on_lens` 回写应用层）。
///
/// 锚点按当前布局实时换算（滚动跟随；不可见行无布局自动隐藏）；`BottomLeft`
/// 锚点使标题落在目标行首上方一行。
#[derive(IntoElement)]
pub struct CodeLensOverlay {
    editor: Entity<EditorState>,
    /// 行点击回调（参数为透镜下标；应用层如跳光标到 `offset`）。
    on_lens: Option<Rc<dyn Fn(usize, &mut Window, &mut App)>>,
}

impl CodeLensOverlay {
    /// 由编辑器状态创建。
    pub fn new(editor: &Entity<EditorState>) -> Self {
        Self {
            editor: editor.clone(),
            on_lens: None,
        }
    }

    /// 设置行点击回调（不设置则行不可点，仅展示）。
    pub fn on_lens(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_lens = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for CodeLensOverlay {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (lenses, anchors) = self.editor.read_with(cx, |state, cx| {
            let lenses: Vec<ResolvedCodeLens> = state
                .codelenses()
                .iter()
                .take(MAX_VISIBLE_LENSES)
                .cloned()
                .collect();
            let anchors: Vec<Option<crate::Point<crate::Pixels>>> = lenses
                .iter()
                .map(|lens| {
                    state
                        .input()
                        .read(cx)
                        .range_to_bounds(&(lens.offset..lens.offset))
                        // 行首上方一行（透镜字高 + 间隙）。
                        .map(|bounds| bounds.origin - crate::point(px(0.), px(18.)))
                })
                .collect();
            (lenses, anchors)
        });
        let on_lens = self.on_lens.clone();
        div().children(lenses.into_iter().zip(anchors).enumerate().filter_map(
            move |(ix, (lens, anchor))| {
                let anchor = anchor?;
                let on_lens = on_lens.clone();
                let row = div()
                    .id(("codelens", ix))
                    .px_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .max_w(px(400.))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .hover(|this| this.text_color(cx.theme().foreground))
                    .child(lens.title);
                let row = if let Some(on_lens) = on_lens {
                    row.cursor_pointer().on_click(move |_, window, cx| {
                        on_lens(ix, window, cx);
                    })
                } else {
                    row
                };
                Some(
                    deferred(
                        anchored()
                            .position(anchor)
                            .anchor(Anchor::BottomLeft)
                            .snap_to_window_with_margin(px(8.))
                            .child(row),
                    )
                    .with_priority(1)
                    .into_any_element(),
                )
            },
        ))
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

    /// 假透镜 provider（固定两条：0 行运行 + 5 行引用）。
    struct FakeCodelensProvider;

    impl CodeLensProvider for FakeCodelensProvider {
        fn codelenses(
            &self,
            _text: &Rope,
            _visible: Range<usize>,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Task<anyhow::Result<Vec<CodeLens>>> {
            Task::ready(Ok(vec![
                CodeLens {
                    line: 0,
                    title: "▶ 运行".into(),
                },
                CodeLens {
                    line: 5,
                    title: "3 引用".into(),
                },
            ]))
        }
    }

    /// 泵：虚拟时钟快进 + 执行器跑空（防抖 timer 全为虚拟时间）。
    fn pump(cx: &mut crate::TestAppContext) {
        cx.dispatcher.advance_clock(Duration::from_millis(2000));
        cx.run_until_parked();
        cx.background_executor.run_until_parked();
        cx.run_until_parked();
    }

    /// 请求落位（行号钳制 + 行首偏移 + 排序）。
    #[rgpui::test]
    fn codelens_request_resolves(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\nline2\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        assert!(editor.read_with(cx, |state, _| state.codelenses().is_empty()));
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_codelens_provider(Some(Rc::new(FakeCodelensProvider)), cx);
                state.request_codelenses(window, cx);
            });
        });
        pump(cx);
        let lenses = editor.read_with(cx, |state, _| state.codelenses().to_vec());
        assert_eq!(lenses.len(), 2);
        assert_eq!(lenses[0].line, 0);
        assert_eq!(lenses[0].offset, 0);
        assert_eq!(lenses[0].title.to_string(), "▶ 运行");
        // 第 5 行越界（全文 3 行，末行空行）→ 钳制到末行。
        assert_eq!(lenses[1].line, 2);
    }

    /// 断开 provider 清空透镜。
    #[rgpui::test]
    fn codelens_disconnect_clears(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_codelens_provider(Some(Rc::new(FakeCodelensProvider)), cx);
                state.request_codelenses(window, cx);
            });
        });
        pump(cx);
        assert!(!editor.read_with(cx, |state, _| state.codelenses().is_empty()));
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.set_codelens_provider(None, cx);
            });
        });
        assert!(editor.read_with(cx, |state, _| state.codelenses().is_empty()));
    }
}
