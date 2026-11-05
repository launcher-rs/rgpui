//! 向导式表单（问卷）。
//!
//! 把多步问答收进一条流程：顶部步骤条 + 当前步内容 + 底部导航（上一步 / 下一步 / 提交），
//! 并按步接收调用方的校验结果——未填全的步不放行前进，但仍允许后退。
//!
//! 状态与元素分离（同 `Carousel` 模式）：父组件
//! `cx.new(|_| QuestionnaireState::new().with_steps(...))` 持有进度与校验位，
//! `render` 里放 `Questionnaire::new(&state).child(第一步).child(第二步)`，
//! 每步一个内容元素、只渲染当前步。每步的控件（`Input` / `RadioGroup` / `Switch` 等）
//! 由调用方提供并持有——答价值不进本组件的模型，组件只管「第几步、能否前进、何时提交」。

use crate::components::{StepItem, Steps};
use crate::{prelude::FluentBuilder as _, *};

/// 向导的单个步骤。
#[derive(Clone)]
pub struct QuestionStep {
    /// 步骤标题（显示在步骤条上）。
    pub title: SharedString,
    /// 步骤说明（可选，显示在当前步内容上方）。
    pub description: Option<SharedString>,
}

impl QuestionStep {
    /// 创建步骤。
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
        }
    }

    /// 设置步骤说明。
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// 向导抛出的事件（应用经 `cx.subscribe` 接收）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuestionnaireEvent {
    /// 步骤切换（`from` → `to`，均为 0 起始下标）。
    StepChanged {
        /// 切换前的步骤下标。
        from: usize,
        /// 切换后的步骤下标。
        to: usize,
    },
    /// 提交（仅在末步且该步校验通过时发出）。
    Submit,
}

/// 向导状态实体。
pub struct QuestionnaireState {
    /// 步骤定义。
    steps: Vec<QuestionStep>,
    /// 当前步骤下标（0 起始）。
    current: usize,
    /// 每步的校验结果，与 `steps` 等长；缺省视为通过。
    valid: Vec<bool>,
    /// 是否已提交（提交后末步按钮不再可点）。
    submitted: bool,
}

impl QuestionnaireState {
    /// 创建空向导（无步骤）。
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            current: 0,
            valid: Vec::new(),
            submitted: false,
        }
    }

    /// 设置步骤列表（校验位全部初始化为通过）。
    pub fn with_steps(mut self, steps: impl IntoIterator<Item = QuestionStep>) -> Self {
        self.steps = steps.into_iter().collect();
        self.valid = vec![true; self.steps.len()];
        self.current = self.current.min(self.last_index());
        self
    }

    /// 追加一个步骤。
    pub fn push_step(&mut self, step: QuestionStep, cx: &mut Context<Self>) {
        self.steps.push(step);
        // 新步默认通过，避免「刚加的一步把流程卡死」。
        self.valid.push(true);
        cx.notify();
    }

    /// 步骤定义（只读）。
    pub fn steps(&self) -> &[QuestionStep] {
        &self.steps
    }

    /// 当前步骤下标。
    pub fn current(&self) -> usize {
        self.current
    }

    /// 步骤总数。
    pub fn total(&self) -> usize {
        self.steps.len()
    }

    /// 是否停在最后一步。
    pub fn is_last(&self) -> bool {
        self.steps.is_empty() || self.current + 1 >= self.steps.len()
    }

    /// 是否已提交。
    pub fn is_submitted(&self) -> bool {
        self.submitted
    }

    /// 第 `ix` 步是否通过校验（越界视为通过）。
    pub fn is_step_valid(&self, ix: usize) -> bool {
        self.valid.get(ix).copied().unwrap_or(true)
    }

    /// 设置第 `ix` 步的校验结果（结果变化时才重绘）。
    ///
    /// 调用方在自己的答值变化处（例如 `InputEvent::Change`）回写本步是否填全。
    pub fn set_step_valid(&mut self, ix: usize, valid: bool, cx: &mut Context<Self>) {
        if let Some(slot) = self.valid.get_mut(ix)
            && *slot != valid
        {
            *slot = valid;
            cx.notify();
        }
    }

    /// 当前步能否前进。
    pub fn can_advance(&self) -> bool {
        !self.is_last() && self.is_step_valid(self.current)
    }

    /// 下一步（当前步未通过校验或已在末步时不动）。
    pub fn next(&mut self, cx: &mut Context<Self>) {
        if self.can_advance() {
            self.set_current(self.current + 1, cx);
        }
    }

    /// 上一步（后退不受校验限制）。
    pub fn prev(&mut self, cx: &mut Context<Self>) {
        if self.current > 0 {
            self.set_current(self.current - 1, cx);
        }
    }

    /// 跳到指定步（越界钳制）：后退自由；前进要求途中每一步都通过校验。
    pub fn go_to(&mut self, index: usize, cx: &mut Context<Self>) {
        let target = index.min(self.last_index());
        if target == self.current {
            return;
        }
        // 只要求「跨过的步」合法：目标步自身可以尚未填完。
        let crossed_ok = (self.current..target).all(|ix| self.is_step_valid(ix));
        if target > self.current && !crossed_ok {
            return;
        }
        self.set_current(target, cx);
    }

    /// 提交：仅末步且该步通过校验时生效，置提交态并发出 [`QuestionnaireEvent::Submit`]。
    pub fn submit(&mut self, cx: &mut Context<Self>) {
        if self.submitted || !self.is_last() || !self.is_step_valid(self.current) {
            return;
        }
        self.submitted = true;
        cx.emit(QuestionnaireEvent::Submit);
        cx.notify();
    }

    /// 回到第一步并清除提交态（步骤内容与各步答值由调用方自行清空）。
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.submitted = false;
        self.set_current(0, cx);
    }

    /// 最后一步下标（无步骤时为 0）。
    fn last_index(&self) -> usize {
        self.steps.len().saturating_sub(1)
    }

    /// 写下标并发出 [`QuestionnaireEvent::StepChanged`]（同值不发）。
    fn set_current(&mut self, to: usize, cx: &mut Context<Self>) {
        let to = to.min(self.last_index());
        let from = self.current;
        if from == to {
            return;
        }
        self.current = to;
        // 从末步退回后允许重新提交。
        if to + 1 < self.steps.len() {
            self.submitted = false;
        }
        cx.emit(QuestionnaireEvent::StepChanged { from, to });
        cx.notify();
    }
}

impl Default for QuestionnaireState {
    fn default() -> Self {
        Self::new()
    }
}

impl EventEmitter<QuestionnaireEvent> for QuestionnaireState {}

/// 向导元素（步骤条 + 步骤计数 + 当前步内容 + 导航按钮）。
#[derive(IntoElement)]
pub struct Questionnaire {
    state: Entity<QuestionnaireState>,
    /// 每步的内容，与 `state.steps()` 一一对应。
    pages: Vec<AnyElement>,
    /// 是否显示顶部步骤条。
    show_steps: bool,
    /// 是否显示「第 n / N 步」计数。
    show_progress: bool,
    prev_label: SharedString,
    next_label: SharedString,
    submit_label: SharedString,
    style: StyleRefinement,
}

impl Questionnaire {
    /// 由状态实体创建。
    pub fn new(state: &Entity<QuestionnaireState>) -> Self {
        Self {
            state: state.clone(),
            pages: Vec::new(),
            show_steps: true,
            show_progress: true,
            prev_label: "上一步".into(),
            next_label: "下一步".into(),
            submit_label: "提交".into(),
            style: StyleRefinement::default(),
        }
    }

    /// 设置是否显示顶部步骤条，默认 `true`。
    pub fn show_steps(mut self, show: bool) -> Self {
        self.show_steps = show;
        self
    }

    /// 设置是否显示步骤计数，默认 `true`。
    pub fn show_progress(mut self, show: bool) -> Self {
        self.show_progress = show;
        self
    }

    /// 设置「上一步」按钮文案（本地化用）。
    pub fn prev_label(mut self, label: impl Into<SharedString>) -> Self {
        self.prev_label = label.into();
        self
    }

    /// 设置「下一步」按钮文案（本地化用）。
    pub fn next_label(mut self, label: impl Into<SharedString>) -> Self {
        self.next_label = label.into();
        self
    }

    /// 设置末步提交按钮文案（本地化用）。
    pub fn submit_label(mut self, label: impl Into<SharedString>) -> Self {
        self.submit_label = label.into();
        self
    }
}

impl ParentElement for Questionnaire {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.pages.extend(elements);
    }
}

impl Styled for Questionnaire {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Questionnaire {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let entity_id = self.state.entity_id();
        let state = self.state.clone();
        let user_style = self.style;
        let page_count = self.pages.len();

        let (raw_current, total, submitted, valid_flags, steps) =
            self.state.read_with(cx, |state, _| {
                (
                    state.current,
                    state.steps.len(),
                    state.submitted,
                    state.valid.clone(),
                    state.steps.clone(),
                )
            });

        // 内容数与步骤数不一致时以较短者为准，避免翻到空白页。
        let count = total.min(page_count);
        let current = if count == 0 {
            0
        } else {
            raw_current.min(count - 1)
        };
        let valid = valid_flags.get(current).copied().unwrap_or(true);
        let description = steps.get(current).and_then(|step| step.description.clone());
        let items: Vec<StepItem> = steps
            .iter()
            .map(|step| StepItem::new(step.title.clone()))
            .collect();

        let theme = cx.theme();
        let muted_foreground = theme.tokens.muted_foreground.color;
        let danger = theme.tokens.danger.color;

        let mut pages = self.pages.into_iter();
        let current_page = if count == 0 {
            None
        } else {
            _ = pages.by_ref().take(current).collect::<Vec<_>>();
            pages.next()
        };

        let mut root = div()
            .id(("questionnaire", entity_id))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .w_full()
            // 无障碍语义：整组作为分组节点，并把进度写进无障碍名。
            .role(accesskit::Role::Group)
            .aria_label(format!("第 {} 步，共 {} 步", current + 1, count));

        if self.show_steps && count > 0 {
            root = root.child(Steps::new(items, current));
        }

        if self.show_progress && count > 0 {
            root = root.child(
                div()
                    .id(("questionnaire-progress", entity_id))
                    .text_xs()
                    .text_color(muted_foreground)
                    .debug_selector(move || "questionnaire-progress".to_string())
                    .child(format!("第 {} / {} 步", current + 1, count)),
            );
        }

        if let Some(description) = description {
            root = root.child(
                div()
                    .id(("questionnaire-description", entity_id))
                    .text_sm()
                    .text_color(muted_foreground)
                    .child(description),
            );
        }

        if let Some(page) = current_page {
            root = root.child(
                div()
                    .id(("questionnaire-body", entity_id))
                    .flex_1()
                    .debug_selector(move || "questionnaire-body".to_string())
                    .child(page),
            );
        }

        if !valid && count > 0 {
            root = root.child(
                div()
                    .id(("questionnaire-invalid-hint", entity_id))
                    .text_xs()
                    .text_color(danger)
                    .debug_selector(move || "questionnaire-invalid-hint".to_string())
                    .child("请完成本步必填项后再继续"),
            );
        }

        // 底部导航：后退在首步禁用；末步把前进换成提交。
        let prev_state = state.clone();
        let is_last = current + 1 >= count;
        let right_enabled = if is_last { !submitted && valid } else { valid };
        let right_label = if is_last {
            self.submit_label
        } else {
            self.next_label
        };
        let right_selector = if is_last {
            "questionnaire-submit"
        } else {
            "questionnaire-next"
        };

        root = root.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .pt(px(4.0))
                .child(
                    div()
                        .id(("questionnaire-prev", entity_id))
                        .debug_selector(move || "questionnaire-prev".to_string())
                        .child(
                            Button::new(("questionnaire-prev-button", entity_id))
                                .outline()
                                .label(self.prev_label)
                                .disabled(current == 0)
                                .on_click(move |_, _, cx| {
                                    prev_state.update(cx, |state, cx| state.prev(cx));
                                }),
                        ),
                )
                .child(
                    div()
                        .id((right_selector, entity_id))
                        .debug_selector(move || right_selector.to_string())
                        .child(
                            Button::new((right_selector, entity_id))
                                .primary()
                                .label(right_label)
                                .disabled(!right_enabled)
                                .on_click(move |_, _, cx| {
                                    state.update(cx, |state, cx| {
                                        if is_last {
                                            state.submit(cx);
                                        } else {
                                            state.next(cx);
                                        }
                                    });
                                }),
                        ),
                ),
        );

        if submitted {
            root = root.child(
                div()
                    .id(("questionnaire-submitted", entity_id))
                    .text_xs()
                    .text_color(muted_foreground)
                    .debug_selector(move || "questionnaire-submitted".to_string())
                    .child("已提交"),
            );
        }

        root.map(|mut this| {
            this.style().refine(&user_style);
            this
        })
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{QuestionStep, Questionnaire, QuestionnaireEvent, QuestionnaireState};
    use crate::{
        App, AppContext as _, Context, Div, Entity, InteractiveElement, IntoElement, Modifiers,
        ParentElement as _, Render, Styled as _, TestAppContext, VisualTestContext, Window, div,
        px,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    /// 三步问卷的宿主视图：每步渲染成可定位的占位块，并记录收到的事件。
    struct WizardTest {
        /// 向导状态。
        state: Entity<QuestionnaireState>,
        /// 收到的事件序列。
        events: Rc<RefCell<Vec<QuestionnaireEvent>>>,
    }

    impl WizardTest {
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            let events = Rc::new(RefCell::new(Vec::new()));
            let sink = events.clone();
            let state = cx.new(|_| {
                QuestionnaireState::new().with_steps(
                    (0..3)
                        .map(|ix| {
                            QuestionStep::new(format!("第{ix}步")).description("请填写以下内容")
                        })
                        .collect::<Vec<_>>(),
                )
            });
            cx.subscribe(&state, move |_, _, event, _| {
                sink.borrow_mut().push(event.clone());
            })
            .detach();
            Self { state, events }
        }

        /// 回写第 `ix` 步的校验结果（模拟调用方按答值放行）。
        fn set_valid(&self, ix: usize, valid: bool, cx: &mut App) {
            self.state
                .update(cx, |state, cx| state.set_step_valid(ix, valid, cx));
        }

        /// 已收到的事件（按值取出，便于断言）。
        fn taken_events(&self) -> Vec<QuestionnaireEvent> {
            std::mem::take(&mut *self.events.borrow_mut())
        }
    }

    impl Render for WizardTest {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let page = |ix: usize| -> Div {
                div()
                    .h(px(20.))
                    .debug_selector(move || format!("page-{ix}"))
            };
            div().w(px(300.)).h(px(400.)).child(
                Questionnaire::new(&self.state)
                    .child(page(0))
                    .child(page(1))
                    .child(page(2)),
            )
        }
    }

    /// 建窗口并绘制首帧，返回可视测试上下文。
    fn setup(cx: &mut TestAppContext) -> (Entity<WizardTest>, &mut VisualTestContext) {
        cx.update(crate::theme::init);
        cx.add_window_view(WizardTest::new)
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    }

    /// 点击当前可见的主按钮（前进或提交）。
    fn click_primary(cx: &mut VisualTestContext) {
        let selector = if cx.debug_bounds("questionnaire-next").is_some() {
            "questionnaire-next"
        } else {
            "questionnaire-submit"
        };
        let bounds = cx.debug_bounds(selector).expect("主按钮边界");
        cx.simulate_click(bounds.center(), Modifiers::default());
        draw(cx);
    }

    /// 界面推进：逐步换页、末步换成提交、提交后出现「已提交」。
    #[rgpui::test]
    fn wizard_advances_and_submits(cx: &mut TestAppContext) {
        let (view, cx) = setup(cx);
        draw(cx);

        assert!(cx.debug_bounds("page-0").is_some());
        assert!(cx.debug_bounds("questionnaire-next").is_some());
        assert!(cx.debug_bounds("questionnaire-prev").is_some());

        click_primary(cx);
        assert!(cx.debug_bounds("page-1").is_some());
        assert!(cx.debug_bounds("page-0").is_none());
        assert!(
            cx.debug_bounds("questionnaire-progress").is_some(),
            "步骤计数默认显示"
        );

        click_primary(cx);
        assert!(cx.debug_bounds("page-2").is_some());
        assert!(
            cx.debug_bounds("questionnaire-submit").is_some(),
            "末步主按钮应换成提交"
        );

        click_primary(cx);
        assert!(cx.debug_bounds("questionnaire-submitted").is_some());
        assert!(
            view.read_with(cx, |view, _| view.taken_events())
                == vec![
                    QuestionnaireEvent::StepChanged { from: 0, to: 1 },
                    QuestionnaireEvent::StepChanged { from: 1, to: 2 },
                    QuestionnaireEvent::Submit,
                ]
        );
    }

    /// 校验门控：当前步未通过时出现提示且点「下一步」不翻页，放行后恢复。
    #[rgpui::test]
    fn invalid_step_blocks_next(cx: &mut TestAppContext) {
        let (view, cx) = setup(cx);
        draw(cx);

        cx.update(|_, cx| view.update(cx, |view, cx| view.set_valid(0, false, cx)));
        draw(cx);
        assert!(cx.debug_bounds("questionnaire-invalid-hint").is_some());

        click_primary(cx);
        assert!(cx.debug_bounds("page-0").is_some(), "未通过校验不应翻页");
        assert!(view.read_with(cx, |view, _| view.taken_events()).is_empty());

        cx.update(|_, cx| view.update(cx, |view, cx| view.set_valid(0, true, cx)));
        draw(cx);
        assert!(cx.debug_bounds("questionnaire-invalid-hint").is_none());
        click_primary(cx);
        assert!(cx.debug_bounds("page-1").is_some());
    }

    /// 后退不受校验限制；首步「上一步」禁用。
    #[rgpui::test]
    fn back_navigation_ignores_validation(cx: &mut TestAppContext) {
        let (view, cx) = setup(cx);
        draw(cx);

        // 首步点上一步：不翻页也不发事件。
        let bounds = cx.debug_bounds("questionnaire-prev").expect("上一步边界");
        cx.simulate_click(bounds.center(), Modifiers::default());
        draw(cx);
        assert!(cx.debug_bounds("page-0").is_some());

        click_primary(cx);
        assert!(cx.debug_bounds("page-1").is_some());

        // 第 1 步标记未通过后退回第 0 步仍应成功。
        cx.update(|_, cx| view.update(cx, |view, cx| view.set_valid(1, false, cx)));
        draw(cx);
        let bounds = cx.debug_bounds("questionnaire-prev").expect("上一步边界");
        cx.simulate_click(bounds.center(), Modifiers::default());
        draw(cx);
        assert!(cx.debug_bounds("page-0").is_some());
    }

    /// 纯状态口径：前进门槛、`go_to` 跳步、提交与重置、事件去重。
    #[rgpui::test]
    fn state_gates_navigation_and_events(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let state = cx.update(|cx| {
            cx.new(|_| {
                QuestionnaireState::new().with_steps(
                    (0..4)
                        .map(|ix| QuestionStep::new(format!("步骤{ix}")))
                        .collect::<Vec<_>>(),
                )
            })
        });
        let events: Rc<RefCell<Vec<QuestionnaireEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        cx.update(|cx| {
            cx.subscribe(&state, move |_, event, _| {
                sink.borrow_mut().push(event.clone());
            })
            .detach();

            state.update(cx, |state, cx| {
                // 第 2 步未通过：从第 0 步直接跳到第 3 步应被拦下。
                state.set_step_valid(2, false, cx);
                state.go_to(3, cx);
                assert_eq!(state.current(), 0, "途经未通过的步不允许跳步");
                // 前两步合法，逐步前进到第 2 步后被自己的校验位卡住。
                state.next(cx);
                assert_eq!(state.current(), 1);
                state.next(cx);
                assert_eq!(state.current(), 2);
                state.next(cx);
                assert_eq!(state.current(), 2, "当前步未通过校验不应前进");
                assert!(!state.can_advance());

                // 放行后一次跳到底，末步重复提交只发一次事件。
                state.set_step_valid(2, true, cx);
                state.go_to(3, cx);
                assert_eq!(state.current(), 3);
                assert!(state.is_last());
                state.submit(cx);
                state.submit(cx);
                assert!(state.is_submitted());

                // 退回非末步清除提交态，并可再次提交。
                state.go_to(1, cx);
                assert!(!state.is_submitted());
                state.go_to(3, cx);
                state.submit(cx);

                state.reset(cx);
                assert_eq!(state.current(), 0);
                assert!(!state.is_submitted());
                assert_eq!(state.total(), 4);
                assert_eq!(state.steps().len(), 4);
                assert!(state.is_step_valid(9), "越界的校验查询应视为通过");

                // 追加的步骤默认通过，当前步仍可前进。
                state.push_step(QuestionStep::new("步骤4"), cx);
                assert_eq!(state.total(), 5);
                assert!(state.can_advance());
                assert!(!state.is_last());
            });
        });
        assert_eq!(
            events.borrow().as_slice(),
            &[
                QuestionnaireEvent::StepChanged { from: 0, to: 1 },
                QuestionnaireEvent::StepChanged { from: 1, to: 2 },
                QuestionnaireEvent::StepChanged { from: 2, to: 3 },
                QuestionnaireEvent::Submit,
                QuestionnaireEvent::StepChanged { from: 3, to: 1 },
                QuestionnaireEvent::StepChanged { from: 1, to: 3 },
                QuestionnaireEvent::Submit,
                QuestionnaireEvent::StepChanged { from: 3, to: 0 },
            ]
        );
    }

    /// 空向导：没有步骤时不渲染内容区，也不报错。
    #[rgpui::test]
    fn empty_wizard_renders_without_pages(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let (_view, cx) = cx.add_window_view(|_window, cx| {
            let state = cx.new(|_| QuestionnaireState::new());
            EmptyWizard { state }
        });
        draw(cx);
        assert!(cx.debug_bounds("questionnaire-body").is_none());
        assert!(cx.debug_bounds("questionnaire-progress").is_none());
    }

    /// 无步骤宿主（只为空向导渲染测试存在）。
    struct EmptyWizard {
        state: Entity<QuestionnaireState>,
    }

    impl Render for EmptyWizard {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .w(px(200.))
                .h(px(200.))
                .child(Questionnaire::new(&self.state))
        }
    }
}
