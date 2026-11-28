//! 编辑器内联输入框：把静态渲染与编辑态所需的度量收拢到一处。
//!
//! Word / Markdown / Excel 这类“文档内可编辑文本”都有一个共同难题：静态态用
//! `StyledText` 等元素按段落字号/行高排版，点击进入编辑态后换成 [`Input`]，
//! 若输入框的度量与静态态不一致，文字就会跳动、下沉或变色。过去各编辑器各自
//! 调用 `set_font_size_override` / `set_line_height_override` / padding 等一组 API，
//! 漏掉任何一项都会产生可见抖动（例如 Markdown 曾忘记设字号，点击后文字缩回默认）。
//!
//! 本模块提供 [`EditorInput`] 与 [`EditorMetrics`]：度量只描述一次，
//! [`EditorMetrics::apply_to`] 一次性写入 `InputState`，[`EditorInput`] 渲染时用
//! **同一份**度量设置 widget。两者共用同一个数据源，从根本上杜绝“两边不一致”。

use crate::{
    App, AppContext, Entity, Hsla, IntoElement, Pixels, RenderOnce, Styled, StyledExt as _, Window,
    px,
};

use super::{Input, InputState};

/// 编辑器内联输入框的一组度量。
///
/// 所有字段都是“与静态渲染对齐后的目标值”。调用方只需构造一次，
/// 同时交给 [`EditorMetrics::apply_to`]（写状态）和 [`EditorInput::new`]（渲染）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorMetrics {
    /// 文本字号（px）。编辑态文字按此字号绘制，须与静态段落一致。
    pub font_size: Pixels,
    /// 行高。传 `Some` 时用固定像素值（与静态段落行高一致）；
    /// `None` 表示按字号与窗口默认行高推导。
    pub line_height: Option<Pixels>,
    /// 光标宽度。`None` 用平台默认值。
    pub cursor_width: Option<Pixels>,
    /// 折行宽度。`Some` 时同时预置 display_map 折行与 auto-grow 行数，
    /// 避免切换目标后首帧用旧行数布局而跳动。
    pub wrap_width: Option<Pixels>,
    /// 折行宽度右侧预留。外部排版器已按相同宽度折行时应设为 0。
    pub wrap_inset: Pixels,
    /// 水平内边距（widget 的 `px`）。
    pub padding_x: Pixels,
    /// 垂直内边距（widget 的 `py`）。内联编辑器通常需要 0 才能与静态文字对齐。
    pub padding_y: Pixels,
    /// 行数覆盖。外部排版器已算好行数时传入，绕过 auto-grow 的帧延迟。
    pub row_count: Option<usize>,
}

impl Default for EditorMetrics {
    fn default() -> Self {
        Self {
            font_size: px(14.0),
            line_height: None,
            cursor_width: None,
            wrap_width: None,
            wrap_inset: px(10.0),
            padding_x: px(0.0),
            padding_y: px(0.0),
            row_count: None,
        }
    }
}

impl EditorMetrics {
    /// 一次性把度量写入 `InputState`。
    ///
    /// 带去重：若状态里记录的度量与本次相同则直接返回，不触发 `cx.notify()`，
    /// 因此可以安全地在 render 中调用而不会形成重绘循环。
    /// 度量变化时立即同步折行与 auto-grow 行数，保证下一帧布局与静态态一致。
    pub fn apply_to<C: AppContext>(
        &self,
        input: &Entity<InputState>,
        window: &mut Window,
        cx: &mut C,
    ) {
        input.update(cx, |state, cx| {
            if state.editor_metrics == Some(*self) {
                return;
            }
            state.editor_metrics = Some(*self);
            state.set_font_size_override(window, Some(self.font_size), cx);
            state.set_line_height_override(self.line_height, cx);
            state.set_cursor_width_override(self.cursor_width, cx);
            state.set_wrap_width_inset(self.wrap_inset, cx);
            state.set_row_count_override(self.row_count, cx);
            state.set_wrap_width(self.wrap_width, cx);
        });
    }
}

/// 绑定到 [`InputState`] 的编辑器内联输入框。
///
/// 渲染时用 [`EditorMetrics`] 设置 widget 样式（字号、行高、内边距），
/// 并在渲染前把同一份度量同步到状态。这样“写状态”和“画 widget”永远一致。
#[derive(IntoElement)]
pub struct EditorInput {
    input: Entity<InputState>,
    metrics: EditorMetrics,
    show_context_menu: Option<bool>,
    style: crate::StyleRefinement,
}

impl EditorInput {
    /// 创建编辑器内联输入框。
    pub fn new(input: &Entity<InputState>, metrics: EditorMetrics) -> Self {
        Self {
            input: input.clone(),
            metrics,
            show_context_menu: None,
            style: crate::StyleRefinement::default(),
        }
    }

    /// 取底层输入框状态（供编辑器聚焦、设置文本等操作）。
    pub fn state(&self) -> &Entity<InputState> {
        &self.input
    }

    /// 是否显示输入框自带的右键菜单。
    pub fn show_context_menu(mut self, show: bool) -> Self {
        self.show_context_menu = Some(show);
        self
    }

    /// 设置文本颜色。
    pub fn text_color(mut self, color: impl Into<Hsla>) -> Self {
        self.style.text.color = Some(color.into());
        self
    }
}

impl Styled for EditorInput {
    fn style(&mut self) -> &mut crate::StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for EditorInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // 度量同步到状态；相同值会被去重，不会触发额外重绘。
        self.metrics.apply_to(&self.input, window, cx);

        let mut input = Input::new(&self.input)
            .appearance(false)
            .bordered(false)
            .focus_bordered(false)
            .p(px(0.0))
            .px(self.metrics.padding_x)
            .py(self.metrics.padding_y)
            .text_size(self.metrics.font_size);
        if let Some(show) = self.show_context_menu {
            input = input.show_context_menu(show);
        }
        if let Some(line_height) = self.metrics.line_height {
            input = input.line_height(line_height);
        }
        input.refine_style(&self.style).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_ui::InputState;
    use crate::input_ui::init;
    use crate::theme;
    use crate::{Context, Render};

    struct Probe {
        _state: Entity<InputState>,
    }

    impl Render for Probe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            crate::div()
        }
    }

    #[crate::test]
    fn editor_metrics_syncs_all_overrides_once(cx: &mut crate::TestAppContext) {
        cx.update(init);
        cx.update(theme::init);
        let metrics = EditorMetrics {
            font_size: px(22.0),
            line_height: Some(px(30.0)),
            cursor_width: Some(px(1.0)),
            wrap_width: None,
            wrap_inset: px(0.0),
            padding_x: px(2.0),
            padding_y: px(0.0),
            row_count: Some(3),
        };
        cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            metrics.apply_to(&state, window, cx);
            state.update(cx, |state, _cx| {
                assert_eq!(state.editor_metrics, Some(metrics));
                assert_eq!(state.font_size_override, Some(px(22.0)));
                assert_eq!(state.line_height_override, Some(px(30.0)));
                assert_eq!(state.cursor_width_override, Some(px(1.0)));
                assert_eq!(state.wrap_width_inset, px(0.0));
                assert_eq!(state.row_count_override, Some(3));
            });
            Probe { _state: state }
        });
    }
}
