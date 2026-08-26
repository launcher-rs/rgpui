//! 倒计时/正计时组件：天/时/分/秒展示，可选标签与分隔符。

use std::rc::Rc;
use std::time::Duration;
use web_time::SystemTime;

use crate::{prelude::FluentBuilder as _, *};

/// 倒计时尺寸。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum CountdownSize {
    /// 小号。
    Sm,
    /// 中号（默认）。
    #[default]
    Md,
    /// 大号。
    Lg,
}

impl CountdownSize {
    /// 数字字号。
    fn digit_size(&self) -> Pixels {
        match self {
            CountdownSize::Sm => px(20.0),
            CountdownSize::Md => px(32.0),
            CountdownSize::Lg => px(48.0),
        }
    }

    /// 标签字号。
    fn label_size(&self) -> Pixels {
        match self {
            CountdownSize::Sm => px(10.0),
            CountdownSize::Md => px(12.0),
            CountdownSize::Lg => px(14.0),
        }
    }

    /// 分隔符字号。
    fn separator_size(&self) -> Pixels {
        match self {
            CountdownSize::Sm => px(16.0),
            CountdownSize::Md => px(24.0),
            CountdownSize::Lg => px(36.0),
        }
    }

    /// 每个单位的内边距。
    fn unit_padding(&self) -> Pixels {
        match self {
            CountdownSize::Sm => px(8.0),
            CountdownSize::Md => px(12.0),
            CountdownSize::Lg => px(16.0),
        }
    }
}

/// 倒计时分隔符样式。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum CountdownSeparator {
    /// 冒号。
    #[default]
    Colon,
    /// 空格。
    Space,
    /// 短横线。
    Dash,
    /// 点。
    Dot,
    /// 无。
    None,
}

impl CountdownSeparator {
    /// 返回分隔符文本。
    fn as_str(&self) -> &'static str {
        match self {
            CountdownSeparator::Colon => ":",
            CountdownSeparator::Space => " ",
            CountdownSeparator::Dash => "-",
            CountdownSeparator::Dot => ".",
            CountdownSeparator::None => "",
        }
    }
}

/// 倒计时显示格式。
#[derive(Clone, Debug)]
pub struct CountdownFormat {
    /// 是否显示天。
    pub show_days: bool,
    /// 是否显示时。
    pub show_hours: bool,
    /// 是否显示分。
    pub show_minutes: bool,
    /// 是否显示秒。
    pub show_seconds: bool,
    /// 是否显示单位标签。
    pub show_labels: bool,
    /// 是否补零。
    pub pad_zeros: bool,
}

impl Default for CountdownFormat {
    fn default() -> Self {
        Self {
            show_days: true,
            show_hours: true,
            show_minutes: true,
            show_seconds: true,
            show_labels: true,
            pad_zeros: true,
        }
    }
}

impl CountdownFormat {
    /// 不显示天。
    pub fn no_days() -> Self {
        Self {
            show_days: false,
            ..Default::default()
        }
    }

    /// 仅显示时分秒、无标签。
    pub fn time_only() -> Self {
        Self {
            show_days: false,
            show_hours: true,
            show_minutes: true,
            show_seconds: true,
            show_labels: false,
            pad_zeros: true,
        }
    }

    /// 仅显示分秒、无标签。
    pub fn minimal() -> Self {
        Self {
            show_days: false,
            show_hours: false,
            show_minutes: true,
            show_seconds: true,
            show_labels: false,
            pad_zeros: true,
        }
    }
}

/// 时间单位分解。
#[derive(Clone, Debug)]
pub struct TimeUnits {
    /// 天。
    pub days: u64,
    /// 时。
    pub hours: u64,
    /// 分。
    pub minutes: u64,
    /// 秒。
    pub seconds: u64,
    /// 总秒数。
    pub total_seconds: i64,
}

impl TimeUnits {
    /// 由 `Duration` 分解为各单位。
    fn from_duration(duration: Duration) -> Self {
        let total_secs = duration.as_secs();
        let days = total_secs / 86400;
        let hours = (total_secs % 86400) / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;

        Self {
            days,
            hours,
            minutes,
            seconds,
            total_seconds: total_secs as i64,
        }
    }

    /// 全零单位。
    fn zero() -> Self {
        Self {
            days: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
            total_seconds: 0,
        }
    }
}

/// 倒计时状态：管理目标时间/正计时起点与每秒心跳。
pub struct CountdownState {
    target_time: Option<SystemTime>,
    start_time: Option<SystemTime>,
    count_up: bool,
    running: bool,
    completed: bool,
}

impl CountdownState {
    /// 创建状态。
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            target_time: None,
            start_time: None,
            count_up: false,
            running: false,
            completed: false,
        }
    }

    /// 设置倒计时目标时间。
    pub fn set_target(&mut self, target: SystemTime, cx: &mut Context<Self>) {
        self.target_time = Some(target);
        self.count_up = false;
        self.running = true;
        self.completed = false;
        self.schedule_tick(cx);
        cx.notify();
    }

    /// 设置倒计时时长。
    pub fn set_duration(&mut self, duration: Duration, cx: &mut Context<Self>) {
        let target = SystemTime::now() + duration;
        self.set_target(target, cx);
    }

    /// 设置正计时起点。
    pub fn set_count_up(&mut self, start: SystemTime, cx: &mut Context<Self>) {
        self.start_time = Some(start);
        self.target_time = None;
        self.count_up = true;
        self.running = true;
        self.completed = false;
        self.schedule_tick(cx);
        cx.notify();
    }

    /// 从当前时刻开始正计时。
    pub fn start_count_up(&mut self, cx: &mut Context<Self>) {
        self.set_count_up(SystemTime::now(), cx);
    }

    /// 停止计时。
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        self.running = false;
        cx.notify();
    }

    /// 恢复计时。
    pub fn resume(&mut self, cx: &mut Context<Self>) {
        if self.target_time.is_some() || self.start_time.is_some() {
            self.running = true;
            self.schedule_tick(cx);
            cx.notify();
        }
    }

    /// 重置状态。
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.target_time = None;
        self.start_time = None;
        self.running = false;
        self.completed = false;
        cx.notify();
    }

    /// 判断是否运行中。
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// 判断是否已完成（倒计时归零）。
    pub fn is_completed(&self) -> bool {
        self.completed
    }

    /// 计算当前时间单位。
    pub fn time_units(&self) -> TimeUnits {
        let now = SystemTime::now();

        if self.count_up {
            if let Some(start) = self.start_time {
                if let Ok(elapsed) = now.duration_since(start) {
                    return TimeUnits::from_duration(elapsed);
                }
            }
        } else if let Some(target) = self.target_time {
            if let Ok(remaining) = target.duration_since(now) {
                return TimeUnits::from_duration(remaining);
            }
        }

        TimeUnits::zero()
    }

    /// 安排每秒一次的计时心跳。
    fn schedule_tick(&self, cx: &mut Context<Self>) {
        if !self.running {
            return;
        }

        cx.spawn(async |this, cx| {
            cx.background_executor().timer(Duration::from_secs(1)).await;

            _ = this.update(cx, |state, cx| {
                if state.running {
                    if !state.count_up {
                        if let Some(target) = state.target_time {
                            if SystemTime::now() >= target {
                                state.completed = true;
                                state.running = false;
                            }
                        }
                    }

                    if state.running {
                        state.schedule_tick(cx);
                    }

                    cx.notify();
                }
            });
        })
        .detach();
    }
}

impl Render for CountdownState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// 倒计时显示组件。
#[derive(IntoElement)]
pub struct Countdown {
    id: ElementId,
    state: Entity<CountdownState>,
    size: CountdownSize,
    separator: CountdownSeparator,
    format: CountdownFormat,
    on_complete: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    style: StyleRefinement,
}

impl Countdown {
    /// 创建倒计时组件。
    pub fn new(id: impl Into<ElementId>, state: Entity<CountdownState>) -> Self {
        Self {
            id: id.into(),
            state,
            size: CountdownSize::Md,
            separator: CountdownSeparator::Colon,
            format: CountdownFormat::default(),
            on_complete: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置尺寸。
    pub fn size(mut self, size: CountdownSize) -> Self {
        self.size = size;
        self
    }

    /// 设置分隔符。
    pub fn separator(mut self, separator: CountdownSeparator) -> Self {
        self.separator = separator;
        self
    }

    /// 设置显示格式。
    pub fn format(mut self, format: CountdownFormat) -> Self {
        self.format = format;
        self
    }

    /// 是否显示天。
    pub fn show_days(mut self, show: bool) -> Self {
        self.format.show_days = show;
        self
    }

    /// 是否显示时。
    pub fn show_hours(mut self, show: bool) -> Self {
        self.format.show_hours = show;
        self
    }

    /// 是否显示分。
    pub fn show_minutes(mut self, show: bool) -> Self {
        self.format.show_minutes = show;
        self
    }

    /// 是否显示秒。
    pub fn show_seconds(mut self, show: bool) -> Self {
        self.format.show_seconds = show;
        self
    }

    /// 是否显示单位标签。
    pub fn show_labels(mut self, show: bool) -> Self {
        self.format.show_labels = show;
        self
    }

    /// 是否补零。
    pub fn pad_zeros(mut self, pad: bool) -> Self {
        self.format.pad_zeros = pad;
        self
    }

    /// 设置倒计时完成回调。
    pub fn on_complete(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_complete = Some(Rc::new(handler));
        self
    }

    /// 渲染单个时间单位。
    fn render_unit(&self, value: u64, label: &str, theme: &Theme) -> AnyElement {
        let digit_text = if self.format.pad_zeros {
            format!("{:02}", value)
        } else {
            format!("{}", value)
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(4.0))
            .px(self.size.unit_padding())
            .child(
                div()
                    .text_size(self.size.digit_size())
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.tokens.foreground)
                    .font_family(theme.mono_font_family.clone())
                    .child(digit_text),
            )
            .when(self.format.show_labels, |this| {
                this.child(
                    div()
                        .text_size(self.size.label_size())
                        .text_color(theme.tokens.muted_foreground)
                        .child(label.to_string()),
                )
            })
            .into_any_element()
    }

    /// 渲染分隔符。
    fn render_separator(&self, theme: &Theme) -> AnyElement {
        let sep = self.separator.as_str();
        if sep.is_empty() {
            return div().into_any_element();
        }

        div()
            .text_size(self.size.separator_size())
            .font_weight(FontWeight::BOLD)
            .text_color(theme.tokens.muted_foreground)
            .child(sep.to_string())
            .into_any_element()
    }
}

impl Styled for Countdown {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Countdown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let units = state.time_units();
        let completed = state.is_completed();
        let user_style = self.style.clone();

        if completed {
            if let Some(ref handler) = self.on_complete {
                handler(window, cx);
            }
        }

        let theme = cx.theme();

        let mut elements: Vec<AnyElement> = Vec::new();

        if self.format.show_days {
            elements.push(self.render_unit(units.days, "days", theme));
            if self.format.show_hours || self.format.show_minutes || self.format.show_seconds {
                elements.push(self.render_separator(theme));
            }
        }

        if self.format.show_hours {
            let hours = if self.format.show_days {
                units.hours
            } else {
                units.days * 24 + units.hours
            };
            elements.push(self.render_unit(hours, "hours", theme).into_any_element());
            if self.format.show_minutes || self.format.show_seconds {
                elements.push(self.render_separator(theme));
            }
        }

        if self.format.show_minutes {
            let minutes = if self.format.show_hours {
                units.minutes
            } else {
                (units.days * 24 + units.hours) * 60 + units.minutes
            };
            elements.push(self.render_unit(minutes, "min", theme).into_any_element());
            if self.format.show_seconds {
                elements.push(self.render_separator(theme));
            }
        }

        if self.format.show_seconds {
            let seconds = if self.format.show_minutes {
                units.seconds
            } else {
                units.total_seconds as u64
            };
            elements.push(self.render_unit(seconds, "sec", theme).into_any_element());
        }

        let mut root = div()
            .id(self.id)
            .flex()
            .items_center()
            .justify_center()
            .gap(px(4.0))
            .children(elements);
        root.style().refine(&user_style);
        root.into_any_element()
    }
}
