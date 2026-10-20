//! 公式栏 —— 名称框 + 取消/提交 + `fx` + 公式输入 + 展开。
//!
//! 对应电子表格编辑器的顶部「公式栏」：左侧名称框（单元格/命名区域），
//! 中间一组操作按钮（取消 / 提交 / 插入函数），右侧公式输入框。
//! 全部基于 [`InputState`]（完整文本编辑/输入法/选区能力），并以事件形式
//! 把「提交名称」「提交公式」「取消」「插入函数」抛给应用，本身不含单元格逻辑。
//!
//! ## 用法
//!
//! ```ignore
//! use rgpui::formula_bar::{FormulaBar, FormulaBarEvent, FormulaBarState};
//!
//! let bar = cx.new(|cx| FormulaBarState::new(window, cx));
//! cx.subscribe_in(&bar, window, |this, _, event: &FormulaBarEvent, window, cx| {
//!     match event {
//!         FormulaBarEvent::NameSubmit(name) => this.goto(name, cx),
//!         FormulaBarEvent::FormulaSubmit(text) => this.commit(text, cx),
//!         FormulaBarEvent::Cancel => this.cancel(),
//!         FormulaBarEvent::InsertFunction => this.open_insert_function(),
//!     }
//! })
//! .detach();
//!
//! // 视图树中：
//! FormulaBar::new(&bar)
//! ```

use crate::input_ui::{Input, InputEvent, InputState};
use crate::{
    ActiveTheme as _, App, AppContext as _, Button, Context, Entity, EventEmitter, IconName,
    InteractiveElement as _, IntoElement, ParentElement, RenderOnce, SharedString, Sizable as _,
    Styled, Subscription, Window, div, h_flex, px,
};

/// 公式栏抛出的事件（由 [`FormulaBarState`] 发出，应用经 `subscribe` 接收）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormulaBarEvent {
    /// 名称框按回车提交（值为去除首尾空白后的文本）。
    NameSubmit(SharedString),
    /// 公式输入按回车提交（或点击 ✓）。
    FormulaSubmit(SharedString),
    /// 取消编辑（按 `Esc` 或点击 ✕）。
    Cancel,
    /// 点击 `fx`（插入函数）。
    InsertFunction,
}

/// 公式栏状态：持有名称框与公式输入两个 [`InputState`] 实体，并转发其事件。
pub struct FormulaBarState {
    /// 名称框输入。
    name_input: Entity<InputState>,
    /// 公式输入。
    formula_input: Entity<InputState>,
    /// 是否展开（增高公式输入区）。
    expanded: bool,
    /// 名称框事件订阅（保活）。
    _name_subscription: Subscription,
    /// 公式输入事件订阅（保活）。
    _formula_subscription: Subscription,
}

impl FormulaBarState {
    /// 创建公式栏状态（两个输入框 + 回车/Cancel 事件转发）。
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("名称框")
                .submit_on_enter(true)
        });
        // 公式输入用多行控件：Enter 提交、Shift+Enter 换行，展开时增高显示长公式。
        let formula_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .auto_grow(1, 6)
                .submit_on_enter(true)
                .placeholder("输入公式或值")
        });

        let name_subscription = cx.subscribe_in(
            &name_input,
            window,
            |this, _, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    // 名称框提交时去掉首尾空白（`B3 ` 与 `B3` 等价）。
                    let value = this.name_input.read(cx).value().trim().to_string();
                    cx.emit(FormulaBarEvent::NameSubmit(value.into()));
                }
            },
        );
        let formula_subscription = cx.subscribe_in(
            &formula_input,
            window,
            |this, _, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    let value = this.formula_input.read(cx).value();
                    cx.emit(FormulaBarEvent::FormulaSubmit(value));
                }
            },
        );

        Self {
            name_input,
            formula_input,
            expanded: false,
            _name_subscription: name_subscription,
            _formula_subscription: formula_subscription,
        }
    }

    /// 名称框输入实体（供外部聚焦/读取）。
    pub fn name_input(&self) -> &Entity<InputState> {
        &self.name_input
    }

    /// 公式输入实体（供外部聚焦/读取）。
    pub fn formula_input(&self) -> &Entity<InputState> {
        &self.formula_input
    }

    /// 名称框文本。
    pub fn name(&self, cx: &App) -> SharedString {
        self.name_input.read(cx).value()
    }

    /// 公式文本。
    pub fn formula(&self, cx: &App) -> SharedString {
        self.formula_input.read(cx).value()
    }

    /// 设置名称框文本（不触发提交）。
    pub fn set_name(
        &mut self,
        value: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.name_input
            .update(cx, |input, cx| input.set_value(value, window, cx));
    }

    /// 设置公式文本（不触发提交）。
    pub fn set_formula(
        &mut self,
        value: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.formula_input
            .update(cx, |input, cx| input.set_value(value, window, cx));
    }

    /// 清空公式输入（常用于取消编辑后回退显示）。
    pub fn clear_formula(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_formula("", window, cx);
    }

    /// 是否展开。
    pub fn expanded(&self) -> bool {
        self.expanded
    }

    /// 设置展开态。
    pub fn set_expanded(&mut self, expanded: bool, cx: &mut Context<Self>) {
        if self.expanded != expanded {
            self.expanded = expanded;
            cx.notify();
        }
    }

    /// 切换展开态。
    pub fn toggle_expanded(&mut self, cx: &mut Context<Self>) {
        self.set_expanded(!self.expanded, cx);
    }
}

impl EventEmitter<FormulaBarEvent> for FormulaBarState {}

/// 公式栏元素。
#[derive(IntoElement)]
pub struct FormulaBar {
    state: Entity<FormulaBarState>,
    name_width: f32,
    show_name_box: bool,
    show_controls: bool,
    show_function: bool,
    show_expand: bool,
}

impl FormulaBar {
    /// 创建公式栏（`state` 建议由父视图 `observe`，展开态变化时重绘）。
    pub fn new(state: &Entity<FormulaBarState>) -> Self {
        Self {
            state: state.clone(),
            name_width: 120.0,
            show_name_box: true,
            show_controls: true,
            show_function: true,
            show_expand: true,
        }
    }

    /// 名称框宽度（像素，默认 120）。
    pub fn name_width(mut self, width: f32) -> Self {
        self.name_width = width;
        self
    }

    /// 是否显示名称框（默认显示）。
    pub fn show_name_box(mut self, show: bool) -> Self {
        self.show_name_box = show;
        self
    }

    /// 是否显示取消/提交按钮（默认显示）。
    pub fn show_controls(mut self, show: bool) -> Self {
        self.show_controls = show;
        self
    }

    /// 是否显示 `fx` 按钮（默认显示）。
    pub fn show_function(mut self, show: bool) -> Self {
        self.show_function = show;
        self
    }

    /// 是否显示展开开关（默认显示）。
    pub fn show_expand(mut self, show: bool) -> Self {
        self.show_expand = show;
        self
    }
}

impl RenderOnce for FormulaBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let entity_id = self.state.entity_id();
        let (name_input, formula_input, expanded) = {
            let state = self.state.read(cx);
            (
                state.name_input().clone(),
                state.formula_input().clone(),
                state.expanded(),
            )
        };
        let state = self.state.clone();

        let mut row = h_flex()
            .w_full()
            .items_start()
            .gap(px(4.0))
            .px(px(8.0))
            .py(px(4.0))
            .bg(theme.background)
            .border_b_1()
            .border_color(theme.border)
            .key_context("FormulaBar");

        if self.show_name_box {
            row = row.child(
                Input::new(&name_input)
                    .w(px(self.name_width))
                    .with_size(crate::ElementSize::Small),
            );
        }

        if self.show_controls {
            let cancel = {
                let state = state.clone();
                Button::new(("formula-bar-cancel", entity_id))
                    .compact()
                    .icon(IconName::Close)
                    .on_click(move |_event, _window, cx| {
                        state.update(cx, |_, cx| cx.emit(FormulaBarEvent::Cancel));
                    })
            };
            let commit = {
                let state = state.clone();
                Button::new(("formula-bar-commit", entity_id))
                    .compact()
                    .icon(IconName::Check)
                    .on_click(move |_event, _window, cx| {
                        state.update(cx, |state, cx| {
                            let value = state.formula(cx);
                            cx.emit(FormulaBarEvent::FormulaSubmit(value));
                        });
                    })
            };
            row = row.child(cancel).child(commit);
        }

        if self.show_function {
            let state = state.clone();
            let fx = Button::new(("formula-bar-fx", entity_id))
                .compact()
                .label("fx")
                .on_click(move |_event, _window, cx| {
                    state.update(cx, |_, cx| cx.emit(FormulaBarEvent::InsertFunction));
                });
            row = row.child(fx);
        }

        let mut formula = Input::new(&formula_input).with_size(crate::ElementSize::Small);
        if expanded {
            formula = formula.h(px(72.0));
        }
        row = row.child(div().flex_1().min_w(px(0.0)).child(formula));

        if self.show_expand {
            let state = state.clone();
            let icon = if expanded {
                IconName::ChevronUp
            } else {
                IconName::ChevronDown
            };
            let expand = Button::new(("formula-bar-expand", entity_id))
                .compact()
                .icon(icon)
                .on_click(move |_event, _window, cx| {
                    state.update(cx, |state, cx| state.toggle_expanded(cx));
                });
            row = row.child(expand);
        }

        // Esc 取消：输入框的 Esc 会向上冒泡，这里统一转成 Cancel 事件。
        row.on_key_down(move |event, _window, cx| {
            if event.keystroke.key.eq_ignore_ascii_case("escape") {
                state.update(cx, |_, cx| cx.emit(FormulaBarEvent::Cancel));
            }
        })
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{FormulaBar, FormulaBarEvent, FormulaBarState};
    use crate::AppContext as _;
    use crate::{Context, Entity, IntoElement, ParentElement as _, Render, Window, div};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// 渲染公式栏的测试宿主。
    struct Probe {
        state: Entity<FormulaBarState>,
    }

    impl Render for Probe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(FormulaBar::new(&self.state))
        }
    }

    /// 构建宿主视图，并把公式栏事件收集到返回的 `events` 中。
    fn setup(
        cx: &mut crate::TestAppContext,
    ) -> (
        Entity<Probe>,
        Rc<RefCell<Vec<FormulaBarEvent>>>,
        &mut crate::VisualTestContext,
    ) {
        let events: Rc<RefCell<Vec<FormulaBarEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(move |window, cx| {
            let state = cx.new(|cx| FormulaBarState::new(window, cx));
            cx.subscribe_in(
                &state,
                window,
                move |_this, _state, event: &FormulaBarEvent, _window, _cx| {
                    sink.borrow_mut().push(event.clone());
                },
            )
            .detach();
            Probe { state }
        });
        (probe, events, cx)
    }

    /// 聚焦名称框。
    fn focus_name(state: &Entity<FormulaBarState>, cx: &mut crate::VisualTestContext) {
        cx.update(|window, cx| {
            let input = state.read(cx).name_input().clone();
            input.update(cx, |input, cx| input.focus(window, cx));
        });
    }

    /// 聚焦公式输入。
    fn focus_formula(state: &Entity<FormulaBarState>, cx: &mut crate::VisualTestContext) {
        cx.update(|window, cx| {
            let input = state.read(cx).formula_input().clone();
            input.update(cx, |input, cx| input.focus(window, cx));
        });
    }

    /// 展开态默认关闭，可切换且幂等。
    #[rgpui::test]
    fn expanded_toggles(cx: &mut crate::TestAppContext) {
        let (probe, _events, mut cx) = setup(cx);
        let state = probe.read_with(cx, |probe, _| probe.state.clone());

        assert!(!state.read_with(cx, |state, _| state.expanded()));
        cx.update(|_, cx| state.update(cx, |state, cx| state.toggle_expanded(cx)));
        assert!(state.read_with(cx, |state, _| state.expanded()));
        cx.update(|_, cx| state.update(cx, |state, cx| state.toggle_expanded(cx)));
        assert!(!state.read_with(cx, |state, _| state.expanded()));
        // 设置同值不触发变化（保持原值）。
        cx.update(|_, cx| state.update(cx, |state, cx| state.set_expanded(false, cx)));
        assert!(!state.read_with(cx, |state, _| state.expanded()));
    }

    /// `set_formula` / `formula` / `clear_formula` 读写一致。
    #[rgpui::test]
    fn formula_read_write(cx: &mut crate::TestAppContext) {
        let (probe, _events, mut cx) = setup(cx);
        let state = probe.read_with(cx, |probe, _| probe.state.clone());

        cx.update(|window, cx| {
            state.update(cx, |state, cx| state.set_formula("=A1+B1", window, cx));
        });
        assert_eq!(
            state.read_with(cx, |state, cx| state.formula(cx)).as_ref(),
            "=A1+B1"
        );

        cx.update(|window, cx| {
            state.update(cx, |state, cx| state.clear_formula(window, cx));
        });
        assert_eq!(
            state.read_with(cx, |state, cx| state.formula(cx)).as_ref(),
            ""
        );
    }

    /// 名称框回车抛出 `NameSubmit`（去首尾空白）。
    #[rgpui::test]
    fn enter_submits_name(cx: &mut crate::TestAppContext) {
        let (probe, events, mut cx) = setup(cx);
        let state = probe.read_with(cx, |probe, _| probe.state.clone());

        focus_name(&state, &mut cx);
        cx.simulate_input("  B3  ");
        cx.simulate_keystrokes("enter");
        assert_eq!(
            events.borrow().as_slice(),
            [FormulaBarEvent::NameSubmit("B3".into())]
        );
        // 输入框内保留用户原始文本，仅事件值去空白。
        assert_eq!(
            state.read_with(cx, |state, cx| state.name(cx)).as_ref(),
            "  B3  "
        );
    }

    /// 公式输入回车抛出 `FormulaSubmit`。
    #[rgpui::test]
    fn enter_submits_formula(cx: &mut crate::TestAppContext) {
        let (probe, events, mut cx) = setup(cx);
        let state = probe.read_with(cx, |probe, _| probe.state.clone());

        focus_formula(&state, &mut cx);
        cx.simulate_input("=SUM(A1:A3)");
        cx.simulate_keystrokes("enter");
        // `simulate_keystrokes("enter")` 会附带 `key_char = "\n"`（测试注入），
        // 多行输入会一并插入，故断言「事件值 == 提交时的输入值」即可验证转发。
        let value = state.read_with(cx, |state, cx| state.formula(cx));
        assert!(value.starts_with("=SUM(A1:A3)"), "实际值：{value:?}");
        assert_eq!(
            events.borrow().as_slice(),
            [FormulaBarEvent::FormulaSubmit(value)]
        );
    }

    /// `Esc` 在输入框内冒泡到公式栏，抛出 `Cancel`。
    #[rgpui::test]
    fn escape_cancels(cx: &mut crate::TestAppContext) {
        let (probe, events, mut cx) = setup(cx);
        let state = probe.read_with(cx, |probe, _| probe.state.clone());

        focus_formula(&state, &mut cx);
        cx.simulate_input("=A1");
        cx.simulate_keystrokes("escape");
        assert_eq!(events.borrow().as_slice(), [FormulaBarEvent::Cancel]);
    }
}
