//! 可搜索下拉框（单选/多选）。
//!
//! 输入框实时过滤 + 内联下拉列表（非浮层，免 `PopupMenu` 实体管理）。
//! 状态由实体持有，父组件经 `cx.new(|cx| ComboboxState::new(window, cx))` 创建。

use crate::{
    input_ui::{Input, InputEvent, InputState},
    prelude::*,
    *,
};
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
    /// 选中后待回焦（toggle 时无 Window，延后到 render 执行）。
    needs_focus: bool,
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
            InputEvent::Focus => {
                // 聚焦即展开（点输入框就有下拉，不必先打字）。
                this.open = true;
                cx.notify();
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
            needs_focus: false,
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
        // 点列表项会带走焦点，延后到 render 回焦输入框（否则 Backspace 无处可去）。
        self.needs_focus = true;
        cx.notify();
    }
}

impl Render for ComboboxState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let input = self.input.clone();
        // 延后的变更回调：render 有 Window 后真正触发。
        if self.pending_emit {
            self.pending_emit = false;
            if let Some(ref cb) = self.on_change.clone() {
                cb(&self.selected.clone(), window, cx);
            }
        }
        // 延后的回焦（选中/回车后焦点回到输入框，可继续打字/删除）。
        if self.needs_focus {
            self.needs_focus = false;
            window.focus(&input.focus_handle(cx), cx);
        }

        let theme = cx.theme();
        let border = theme.tokens.border;
        let popover = theme.tokens.popover;
        let accent = theme.tokens.accent.color;
        let muted_foreground = theme.tokens.muted_foreground.color;

        let panel = cx.entity();
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

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——combobox.rs 有 `use crate::*`，会把根导出的
    // `test` 过程宏引进作用域，遮蔽内置 `#[test]` 导致宏无限递归。
    use super::ComboboxState;
    use crate::input_ui::{Backspace, InputState};
    use crate::{AppContext as _, Context, Entity, Render, Window};

    /// 测试宿主视图。
    struct Probe {
        state: Entity<ComboboxState>,
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

    /// 建带三项的下拉框，返回（下拉实体，输入实体）。
    fn with_items(
        window: &mut Window,
        cx: &mut Context<Probe>,
    ) -> (Entity<ComboboxState>, Entity<InputState>) {
        let combo = cx.new(|cx| {
            ComboboxState::new(window, cx).items(vec![
                "apple".into(),
                "apricot".into(),
                "banana".into(),
            ])
        });
        let input = combo.read(cx).input().clone();
        (combo, input)
    }

    /// 打字过滤并展开（`ap` → apple/apricot）。
    #[rgpui::test]
    fn typing_filters_and_opens(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let (combo, _) = with_items(window, cx);
            Probe { state: combo }
        });
        let combo = probe.read_with(cx, |probe, _| probe.state.clone());
        let input = combo.read_with(cx, |state, _| state.input().clone());
        // 与真实键入同 funnel（OS 入口），验证订阅→过滤→展开链路。
        cx.update(|window, cx| {
            input.update(cx, |state, cx| {
                crate::EntityInputHandler::replace_text_in_range(state, None, "ap", window, cx);
            });
        });
        let (open, filtered) = combo.read_with(cx, |state, _| (state.open, state.filtered.clone()));
        assert!(open);
        assert_eq!(filtered, vec![0, 1]);
    }

    /// 退格删除并重过滤（`ap` → 删 → `a`，三项全回）。
    #[rgpui::test]
    fn backspace_deletes_and_refilters(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let (combo, _) = with_items(window, cx);
            Probe { state: combo }
        });
        let combo = probe.read_with(cx, |probe, _| probe.state.clone());
        let input = combo.read_with(cx, |state, _| state.input().clone());
        cx.update(|window, cx| {
            input.update(cx, |state, cx| {
                crate::EntityInputHandler::replace_text_in_range(state, None, "ap", window, cx);
                state.backspace(&Backspace, window, cx);
            });
        });
        let text = input.read_with(cx, |state, _| state.text().to_string());
        assert_eq!(text, "a");
        let filtered = combo.read_with(cx, |state, _| state.filtered.clone());
        assert_eq!(filtered, vec![0, 1, 2]);
    }

    /// 聚焦即展开（点输入框就有下拉，不必先打字）。
    ///
    /// 注：此处直接发 `Focus` 事件验证订阅链路；点击→聚焦→事件的框架派发
    /// 由渲染帧驱动，演示页手工验证。
    #[rgpui::test]
    fn focus_opens(cx: &mut crate::TestAppContext) {
        use crate::input_ui::InputEvent;
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let (combo, _) = with_items(window, cx);
            Probe { state: combo }
        });
        let combo = probe.read_with(cx, |probe, _| probe.state.clone());
        let input = combo.read_with(cx, |state, _| state.input().clone());
        assert!(!combo.read_with(cx, |state, _| state.open));
        cx.update(|_, cx| {
            input.update(cx, |_, cx| {
                cx.emit(InputEvent::Focus);
            });
        });
        assert!(combo.read_with(cx, |state, _| state.open));
    }
}
