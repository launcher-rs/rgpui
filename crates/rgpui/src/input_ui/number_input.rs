use std::rc::Rc;

use crate::prelude::FluentBuilder as _;
use crate::{
    actions, AnyElement, App, Context, Corners, Disableable, ElementSize, Entity, EventEmitter,
    FocusHandle, Focusable, IconName, InteractiveElement as _, IntoElement, KeyBinding, ParentElement,
    RenderOnce, Role, SharedString, Sizable, StatefulInteractiveElement as _, StyleRefinement,
    Styled, TextAlign, Window, h_flex, px, ActiveTheme as _,
};
use crate::{
    Button, ButtonCustomVariant, ButtonVariants as _, StyledExt as _,
};

use super::{Input, InputState, MaskPattern, input_style};

actions!(number_input, [Increment, Decrement]);

const CONTEXT: &str = "NumberInput";
/// 初始化数字输入的键盘绑定（上下方向键步进）。
pub fn init(cx: &mut App) {
    cx.bind_keys(vec![
        KeyBinding::new("up", Increment, Some(CONTEXT)),
        KeyBinding::new("down", Decrement, Some(CONTEXT)),
    ]);
}

/// 带增减按钮的数字输入组件。
#[derive(IntoElement)]
pub struct NumberInput {
    state: Entity<InputState>,
    placeholder: SharedString,
    size: ElementSize,
    prefix: Option<AnyElement>,
    suffix: Option<AnyElement>,
    appearance: bool,
    disabled: bool,
    style: StyleRefinement,
}

impl NumberInput {
    /// 创建一个绑定到 [`InputState`] 的 [`NumberInput`] 元素。
    pub fn new(state: &Entity<InputState>) -> Self {
        Self {
            state: state.clone(),
            size: ElementSize::default(),
            placeholder: SharedString::default(),
            prefix: None,
            suffix: None,
            appearance: true,
            disabled: false,
            style: StyleRefinement::default(),
        }
    }

    /// 设置数字输入的占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置数字输入的前缀元素。
    pub fn prefix(mut self, prefix: impl IntoElement) -> Self {
        self.prefix = Some(prefix.into_any_element());
        self
    }

    /// 设置数字输入的后缀元素。
    pub fn suffix(mut self, suffix: impl IntoElement) -> Self {
        self.suffix = Some(suffix.into_any_element());
        self
    }

    /// 设置数字输入的外观，为 false 时不显示边框和背景。
    pub fn appearance(mut self, appearance: bool) -> Self {
        self.appearance = appearance;
        self
    }

    fn on_increment(state: &Entity<InputState>, window: &mut Window, cx: &mut App) {
        state.update(cx, |state, cx| {
            state.focus(window, cx);
            state.on_action_increment(&Increment, window, cx);
        })
    }

    fn on_decrement(state: &Entity<InputState>, window: &mut Window, cx: &mut App) {
        state.update(cx, |state, cx| {
            state.focus(window, cx);
            state.on_action_decrement(&Decrement, window, cx);
        })
    }
}

impl Disableable for NumberInput {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl InputState {
    fn on_action_increment(&mut self, _: &Increment, window: &mut Window, cx: &mut Context<Self>) {
        self.on_number_input_step(StepAction::Increment, window, cx);
    }

    fn on_action_decrement(&mut self, _: &Decrement, window: &mut Window, cx: &mut Context<Self>) {
        self.on_number_input_step(StepAction::Decrement, window, cx);
    }

    fn on_number_input_step(
        &mut self,
        action: StepAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }

        // 默认情况下 NumberInput 以步长 1 内部步进数值。
        // 若想放弃内部步进并改为发出 `NumberInputEvent::Step`（由调用方更新数值），
        // 可调用 `state.set_step(None, window, cx)`。
        if let Some(step) = self.number_step.clone() {
            let value = self.unmask_value();
            let current = value.trim().parse::<f64>().unwrap_or(0.);
            let step = step.value(current, action, cx);
            if let Some(new_value) =
                step_value(&value, action, step, self.number_min, self.number_max)
            {
                // 步进后的值必须通过 `pattern`/`validate` 校验，
                // 否则回退为发出事件交由调用方处理。
                if self.is_valid_input(&new_value, cx) {
                    let range = self.range_to_utf16(&(0..self.text.len()));
                    self.replace_text_in_range_silent(Some(range), &new_value, window, cx);
                    return;
                }
            } else {
                // 步进无法朝该方向移动数值（例如对低于最小值的值执行减操作），不做任何事。
                return;
            }
        }

        cx.emit(NumberInputEvent::Step(action));
    }
}

/// [`NumberInput`] 增减步进策略。
///
/// 另见 [`InputState::step`] 和 [`InputState::step_by`]。
#[derive(Clone)]
pub enum NumberStep {
    /// 固定的步进值。
    Fixed(f64),
    /// 根据当前值和方向计算步进值。
    ByValue(Rc<dyn Fn(f64, StepAction, &mut Context<InputState>) -> f64>),
}

impl NumberStep {
    /// 创建根据当前值和方向在步进时计算步进值的策略。
    ///
    /// 当前值是步进前的值；空或非法值视为 0。[`StepAction`] 指示是增还是减，
    /// 可用于步长在区间边界处随方向不同的场景。
    ///
    /// 闭包会收到一个 [`Context<InputState>`] 以便在计算步进时读取或更新其他实体，
    /// 但不得重新进入所属的 [`InputState`]（步进期间它被可变借用）。
    pub fn by_value(
        f: impl Fn(f64, StepAction, &mut Context<InputState>) -> f64 + 'static,
    ) -> Self {
        Self::ByValue(Rc::new(f))
    }

    /// 返回给定当前值和方向的步进值。
    pub(super) fn value(
        &self,
        current: f64,
        action: StepAction,
        cx: &mut Context<InputState>,
    ) -> f64 {
        match self {
            Self::Fixed(step) => *step,
            Self::ByValue(f) => f(current, action, cx),
        }
    }
}

impl From<f64> for NumberStep {
    fn from(step: f64) -> Self {
        Self::Fixed(step)
    }
}

/// 将 `value` 按 `step` 步进并把结果钳制到 `min`/`max` 区间。
///
/// 若步进无法朝给定方向移动数值（例如已处于边界）则返回 `None`。
///
/// 结果保留当前值和步进值的最大小数位数，避免浮点精度问题，如 `0.1 + 0.2 -> 0.3`。
fn step_value(
    value: &str,
    action: StepAction,
    step: f64,
    min: Option<f64>,
    max: Option<f64>,
) -> Option<String> {
    fn fraction_digits(value: &str) -> usize {
        value.split('.').nth(1).map_or(0, |frac| frac.len())
    }

    let current = value.trim().parse::<f64>().ok();
    let mut new_value = match action {
        StepAction::Increment => current.unwrap_or(0.) + step,
        StepAction::Decrement => current.unwrap_or(0.) - step,
    };
    let mut digits = fraction_digits(value).max(fraction_digits(&step.to_string()));
    if let Some(min) = min {
        if new_value < min {
            new_value = min;
            digits = digits.max(fraction_digits(&min.to_string()));
        }
    }
    if let Some(max) = max {
        if new_value > max {
            new_value = max;
            digits = digits.max(fraction_digits(&max.to_string()));
        }
    }

    // Web 行为：步进必须朝按压方向移动数值，
    // 因此对低于最小值的值执行减操作不做任何事而不是向上钳制。
    // 空值或非法值总是步进到区间内。
    if let Some(current) = current {
        let moved = match action {
            StepAction::Increment => new_value > current,
            StepAction::Decrement => new_value < current,
        };
        if !moved {
            return None;
        }
    }

    Some(format!("{:.*}", digits, new_value))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StepAction {
    Decrement,
    Increment,
}
pub enum NumberInputEvent {
    Step(StepAction),
}
impl EventEmitter<NumberInputEvent> for InputState {}

impl Focusable for NumberInput {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.focus_handle(cx)
    }
}

impl Sizable for NumberInput {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl Styled for NumberInput {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for NumberInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // 默认使用 `MaskPattern::Number` 把输入限制为合法数字
        // （可选前导符号、数字和单个小数点），并规范化全角数字字符，如 `12。5` -> `12.5`。
        //
        // 仅在用户没有显式设置 `mask_pattern` 时生效，
        // 以便通过 `set_mask_pattern(MaskPattern::None)` 选择退出。
        if !self.state.read(cx).mask_pattern_set {
            self.state.update(cx, |state, _| {
                state.mask_pattern = MaskPattern::Number {
                    separator: None,
                    fraction: None,
                };
            });
        }

        let numeric_value = self.state.read(cx).value().parse::<f64>().ok();
        let focused = self.state.read(cx).focus_handle.is_focused(window) && !self.disabled;
        let (bg, _) = input_style(self.disabled, cx);
        let border_color = if self.disabled {
            cx.theme().input.opacity(0.5)
        } else {
            cx.theme().input
        };
        // 像幽灵按钮一样透明，但在悬停时按主题着色。
        let button_variant = ButtonCustomVariant::new(cx)
            .foreground(cx.theme().secondary_foreground)
            .hover(cx.theme().input.opacity(0.4))
            .active(cx.theme().input.opacity(0.6));
        // 按钮位于 1px 边框内部，因此它们的圆角比边框小一个像素，
        // 否则会盖过边框的内侧曲线。
        let button_radius = if self.appearance {
            (cx.theme().radius - px(1.)).max(px(0.))
        } else {
            cx.theme().radius
        };

        h_flex()
            .id(("number-input", self.state.entity_id()))
            .role(Role::SpinButton)
            .when_some(numeric_value, |this, v| this.aria_numeric_value(v))
            .key_context(CONTEXT)
            .on_action(window.listener_for(&self.state, InputState::on_action_increment))
            .on_action(window.listener_for(&self.state, InputState::on_action_decrement))
            .flex_1()
            .rounded(cx.theme().radius)
            // 按钮是幽灵样式，因此整个控件的边框在这里绘制，而不是由各部分绘制。
            .when(self.appearance, |this| {
                this.bg(bg)
                    .border_1()
                    .border_color(border_color)
                    .when(focused, |this| this.focused_border(cx))
            })
            .refine_style(&self.style)
            .when(self.disabled, |this| this.opacity(0.5))
            .child(
                Button::new("minus")
                    .custom(button_variant)
                    .rounded(button_radius)
                    .with_size(self.size)
                    .icon(IconName::Minus)
                    .compact()
                    .tab_stop(false)
                    .disabled(self.disabled)
                    // 只保留外侧圆角，以贴合边框。
                    .border_corners(Corners {
                        top_left: true,
                        top_right: false,
                        bottom_right: false,
                        bottom_left: true,
                    })
                    .on_click({
                        let state = self.state.clone();
                        move |_, window, cx| {
                            Self::on_decrement(&state, window, cx);
                        }
                    }),
            )
            .child(
                Input::new(&self.state)
                    .appearance(false)
                    .with_size(self.size)
                    .disabled(self.disabled)
                    .gap_0()
                    .rounded_none()
                    .text_align(TextAlign::Center)
                    .when_some(self.prefix, |this, prefix| this.prefix(prefix))
                    .when_some(self.suffix, |this, suffix| this.suffix(suffix)),
            )
            .child(
                Button::new("plus")
                    .custom(button_variant)
                    .rounded(button_radius)
                    .with_size(self.size)
                    .icon(IconName::Plus)
                    .compact()
                    .tab_stop(false)
                    .disabled(self.disabled)
                    .border_corners(Corners {
                        top_left: false,
                        top_right: true,
                        bottom_right: true,
                        bottom_left: false,
                    })
                    .on_click({
                        let state = self.state.clone();
                        move |_, window, cx| {
                            Self::on_increment(&state, window, cx);
                        }
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{StepAction, step_value};

    // `test_number_step` 位于 `state::tests`，因为 `NumberStep::value`
    // 现在需要 `Context<InputState>` 来调用 `by_value` 闭包。

    #[test]
    fn test_step_value() {
        fn some(value: &str) -> Option<String> {
            Some(value.to_string())
        }

        // 从空值步进
        assert_eq!(
            step_value("", StepAction::Increment, 1., None, None),
            some("1")
        );
        assert_eq!(
            step_value("", StepAction::Decrement, 1., None, None),
            some("-1")
        );
        // 非法的中间值视为 0
        assert_eq!(
            step_value("-", StepAction::Increment, 1., None, None),
            some("1")
        );
        assert_eq!(
            step_value("1", StepAction::Increment, 1., None, None),
            some("2")
        );
        assert_eq!(
            step_value("-2", StepAction::Increment, 1., None, None),
            some("-1")
        );

        // 避免浮点精度问题，如 0.1 + 0.2 != 0.30000000000000004
        assert_eq!(
            step_value("0.1", StepAction::Increment, 0.2, None, None),
            some("0.3")
        );
        assert_eq!(
            step_value("0.3", StepAction::Decrement, 0.1, None, None),
            some("0.2")
        );
        // 保留当前值的小数位数
        assert_eq!(
            step_value("1.25", StepAction::Increment, 1., None, None),
            some("2.25")
        );

        // 从空值步进总是进入区间
        assert_eq!(
            step_value("", StepAction::Increment, 1., Some(10.), None),
            some("10")
        );
        assert_eq!(
            step_value("", StepAction::Decrement, 1., Some(10.), None),
            some("10")
        );
        // 钳制到 min/max
        assert_eq!(
            step_value("99.5", StepAction::Increment, 1., None, Some(100.)),
            some("100.0")
        );
        assert_eq!(
            step_value("1000", StepAction::Decrement, 1., None, Some(100.)),
            some("100")
        );
        // 保留钳制边界的小数位数
        assert_eq!(
            step_value("1", StepAction::Decrement, 1., Some(0.25), None),
            some("0.25")
        );

        // 步进必须朝按压方向移动数值：边界处无操作
        assert_eq!(
            step_value("10", StepAction::Decrement, 1., Some(10.), None),
            None
        );
        assert_eq!(
            step_value("100", StepAction::Increment, 1., None, Some(100.)),
            None
        );
        // 对低于最小值执行减操作（或对高于最大值执行加操作）不做任何事，
        // 而不是朝相反方向移动数值
        assert_eq!(
            step_value("5", StepAction::Decrement, 1., Some(10.), None),
            None
        );
        assert_eq!(
            step_value("1000", StepAction::Increment, 1., None, Some(100.)),
            None
        );
    }
}