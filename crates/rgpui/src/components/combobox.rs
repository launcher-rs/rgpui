//! 可搜索下拉框（单选/多选）。
//!
//! 输入框实时过滤 + 内联下拉列表（非浮层，免 `PopupMenu` 实体管理）。
//! 状态由实体持有，父组件经 `cx.new(|cx| ComboboxState::new(window, cx))` 创建。

use crate::{input_ui::{Input, InputEvent, InputState}, prelude::*, *};
use std::sync::Arc;

/// 可搜索下拉框状态实体。
pub struct ComboboxState {
    /// 输入框。
    input: Entity<InputState>,
    /// 全部候选项。
    items: Vec<SharedString>,
    /// 过滤后的候选下标。
    filtered: Vec<usize>,
    /// 是否多选。
    multiple: bool,
    /// 已选下标。
    selected: Vec<usize>,
    /// 下拉列表是否展开。
    open: bool,
    /// 变更回调（已选下标列表）。
    on_change: Option<Arc<dyn Fn(&[usize], &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 有待触发的变更回调（toggle 时无 Window，延后到 render 触发）。
    pending_emit: bool,
}

impl ComboboxState {
    /// 创建空可搜索下拉框（`Context<ComboboxState>` 内调用，父组件经 `cx.new` 间接调用）。
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("搜索…"));

        cx.subscribe(&input, |this, _input, event, cx| match event {
            InputEvent::Change => {
                this.refilter(cx);
            }
            InputEvent::PressEnter { .. } => {
                // 回车选中首个过滤项。
                if let Some(&ix) = this.filtered.first() {
                    this.toggle_select(ix, cx);
                }
            }
            _ => {}
        })
        .detach();

        Self {
            input,
            items: Vec::new(),
            filtered: Vec::new(),
            multiple: false,
            selected: Vec::new(),
            open: false,
            on_change: None,
            pending_emit: false,
        }
    }

    /// 设置候选项（重置过滤与选中）。
    pub fn items(mut self, items: Vec<SharedString>) -> Self {
        self.filtered = (0..items.len()).collect();
        self.selected.clear();
        self.items = items;
        self
    }

    /// 设置是否多选（默认单选）。
    pub fn multiple(mut self, multiple: bool) -> Self {
        self.multiple = multiple;
        self
    }

    /// 设置变更回调。
    pub fn on_change<F>(mut self, f: F) -> Self
    where
        F: Fn(&[usize], &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_change = Some(Arc::new(f));
        self
    }

    /// 输入框实体（供父组件聚焦等）。
    pub fn input(&self) -> &Entity<InputState> {
        &self.input
    }

    /// 已选下标。
    pub fn selected(&self) -> &[usize] {
        &self.selected
    }

    /// 按当前输入文本重过滤。
    fn refilter(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).text().to_string().to_lowercase();
        self.filtered = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.to_lowercase().contains(&query))
            .map(|(ix, _)| ix)
            .collect();
        self.open = true;
        cx.notify();
    }

    /// 切换选中（单选时替换并收起，多选时增删）。
    fn toggle_select(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.multiple {
            if let Some(pos) = self.selected.iter().position(|&i| i == ix) {
                self.selected.remove(pos);
            } else {
                self.selected.push(ix);
            }
        } else {
            self.selected = vec![ix];
            self.open = false;
        }
        let selected = self.selected.clone();
        if let Some(ref cb) = self.on_change.clone() {
            // 回调需要 Window，延后到 render 触发。
            let _ = (cb, selected);
            self.pending_emit = true;
        }
        cx.notify();
    }
}

impl Render for ComboboxState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 延后的变更回调：render 有 Window 后真正触发。
        if self.pending_emit {
            self.pending_emit = false;
            if let Some(ref cb) = self.on_change.clone() {
                cb(&self.selected.clone(), window, cx);
            }
        }

        let theme = cx.theme();
        let border = theme.tokens.border;
        let popover = theme.tokens.popover;
        let accent = theme.tokens.accent.color;
        let muted_foreground = theme.tokens.muted_foreground.color;

        let panel = cx.entity();
        let input = self.input.clone();
        let open = self.open && !self.filtered.is_empty();
        let selected = self.selected.clone();
        let items = self.items.clone();

        div()
            .flex()
            .flex_col()
            .w(px(240.0))
            .child(Input::new(&input).w_full())
            .when(open, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .mt(px(4.0))
                        .max_h(px(200.0))
                        .overflow_y_scrollbar()
                        .bg(popover)
                        .border_1()
                        .border_color(border)
                        .rounded_md()
                        .p(px(4.0))
                        .gap(px(2.0))
                        .children(self.filtered.iter().map(|&ix| {
                            let panel = panel.clone();
                            let label = items.get(ix).cloned().unwrap_or_default();
                            let checked = selected.contains(&ix);
                            div()
                                .id(ix)
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .px(px(8.0))
                                .py(px(6.0))
                                .rounded_sm()
                                .cursor_pointer()
                                .when(checked, |this| this.bg(accent.opacity(0.15)))
                                .when(!checked, |this| {
                                    this.hover(|this| this.bg(accent.opacity(0.08)))
                                })
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(muted_foreground)
                                        .child(if checked { "✓ " } else { "" })
                                        .child(label),
                                )
                                .on_click(move |_, _, cx| {
                                    panel.update(cx, |this, cx| this.toggle_select(ix, cx));
                                })
                        })),
                )
            })
    }
}
