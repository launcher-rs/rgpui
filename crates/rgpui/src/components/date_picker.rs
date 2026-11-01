//! 日期选择器。
//!
//! 月历弹窗面板：年月导航 + 日期格 + 选择回调。状态由实体持有，
//! 父组件经 `cx.new(|cx| DatePickerState::new(cx))` 创建；弹窗打开后调用
//! [`DatePickerState::focus`] 即可纯键盘操作（方向键 / Home / End /
//! PageUp / PageDown / Enter / Space / Esc）。

use crate::{prelude::*, *};
use chrono::{Datelike, NaiveDate};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// 内部不变量：值为 `None` 说明前提被破坏（日历网格年月日必构成合法日期等），必须立刻暴露。
///
/// 统一走这里而不是散落的 `unwrap`：语义与原先一致（同样 panic），但把「这是不变量
/// 断言」的意图写进调用点，模块级 `deny` 仍能拦住随手写的 unwrap。
#[inline]
#[allow(clippy::panic)] // 不变量断言：前提被破坏必须立刻暴露
fn invariant<T>(value: Option<T>, message: &str) -> T {
    match value {
        Some(value) => value,
        None => panic!("{message}"),
    }
}

/// DatePicker 实例计数器（元素 ID 唯一，避免同页多实例冲突）。
static DATE_PICKER_ID: AtomicU64 = AtomicU64::new(0);

/// 日期选择器状态实体。
pub struct DatePickerState {
    /// 实例序号（元素 ID 前缀）。
    instance: u64,
    /// 当前展示的年月（day 位忽略）。
    viewing: NaiveDate,
    /// 已选日期。
    selected: Option<NaiveDate>,
    /// 键盘活动日：描边标记当前位置，`Enter` / `Space` 后转为已选日。
    active_day: NaiveDate,
    /// 面板焦点句柄（由元素上的 `track_focus` 跟踪）。
    focus_handle: FocusHandle,
    /// 选择回调。
    on_change: Option<Arc<dyn Fn(NaiveDate, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 关闭回调（`Esc` 时触发；弹窗由父组件持有，负责隐藏自身）。
    on_close: Option<Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static>>,
}

impl DatePickerState {
    /// 创建日期选择器（默认当月，可先 `select` 预填）。
    pub fn new(cx: &mut Context<Self>) -> Self {
        let today = chrono::Local::now().date_naive();
        Self {
            instance: DATE_PICKER_ID.fetch_add(1, Ordering::Relaxed),
            viewing: today,
            selected: None,
            active_day: today,
            focus_handle: cx.focus_handle(),
            on_change: None,
            on_close: None,
        }
    }

    /// 预填选中日期（同时切到该月并把活动日移到该天）。
    pub fn select(mut self, date: NaiveDate) -> Self {
        self.viewing = date;
        self.active_day = date;
        self.selected = Some(date);
        self
    }

    /// 设置选择回调。
    pub fn on_change<F>(mut self, f: F) -> Self
    where
        F: Fn(NaiveDate, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_change = Some(Arc::new(f));
        self
    }

    /// 设置关闭回调（`Esc` 触发）。
    pub fn on_close<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_close = Some(Arc::new(f));
        self
    }

    /// 已选日期。
    pub fn selected(&self) -> Option<NaiveDate> {
        self.selected
    }

    /// 当前键盘活动日。
    pub fn active_day(&self) -> NaiveDate {
        self.active_day
    }

    /// 聚焦日历面板（父组件打开弹窗后调用）。
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
    }

    /// 切到上/下个月（`delta` 为月份偏移，可为负），活动日钳制到目标月份。
    fn shift_month(&mut self, delta: i32, cx: &mut Context<Self>) {
        let months = self.viewing.year() * 12 + self.viewing.month() as i32 - 1 + delta;
        let year = months.div_euclid(12);
        let month = months.rem_euclid(12) as u32 + 1;
        let Some(first) = NaiveDate::from_ymd_opt(year, month, 1) else {
            return;
        };
        self.viewing = first;
        let day = self.active_day.day().min(days_in_month(year, month));
        // 目标月份已确定，钳制后的年月日必构成合法日期
        self.active_day = invariant(
            NaiveDate::from_ymd_opt(year, month, day),
            "日期选择器：钳制后的年月日必构成合法日期",
        );
        cx.notify();
    }

    /// 活动日平移 `days` 天（跨月时展示月份跟随）。
    fn move_active(&mut self, days: i64, cx: &mut Context<Self>) {
        let Some(target) = self
            .active_day
            .checked_add_signed(chrono::Duration::days(days))
        else {
            return;
        };
        self.set_active(target, cx);
    }

    /// 活动日移到本周一 / 周日。
    fn move_active_to_week_edge(&mut self, end: bool, cx: &mut Context<Self>) {
        let offset = self.active_day.weekday().num_days_from_monday() as i64;
        self.move_active(if end { 6 - offset } else { -offset }, cx);
    }

    /// 设置活动日并同步展示月份。
    fn set_active(&mut self, date: NaiveDate, cx: &mut Context<Self>) {
        self.active_day = date;
        self.viewing = date;
        cx.notify();
    }

    /// 选中某天（同时把它设为活动日与展示月份）。
    fn pick(&mut self, date: NaiveDate, window: &mut Window, cx: &mut Context<Self>) {
        self.active_day = date;
        self.viewing = date;
        self.selected = Some(date);
        self.focus(window, cx);
        if let Some(cb) = self.on_change.clone() {
            cb(date, window, cx);
        }
        cx.notify();
    }

    /// 键盘：方向键 ±1 / ±7 天、`Home` / `End` 周首周末、`PageUp` / `PageDown`
    /// 换月、`Enter` / `Space` 选中、`Esc` 关闭。
    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.to_ascii_lowercase();
        match key.as_str() {
            "left" => self.move_active(-1, cx),
            "right" => self.move_active(1, cx),
            "up" => self.move_active(-7, cx),
            "down" => self.move_active(7, cx),
            "home" => self.move_active_to_week_edge(false, cx),
            "end" => self.move_active_to_week_edge(true, cx),
            "pageup" => self.shift_month(-1, cx),
            "pagedown" => self.shift_month(1, cx),
            "enter" | "space" => {
                let date = self.active_day;
                self.pick(date, window, cx);
            }
            "escape" => {
                if let Some(cb) = self.on_close.clone() {
                    cb(window, cx);
                }
                cx.notify();
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Focusable for DatePickerState {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// 当月天数。
fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(ny, nm, 1)
        .and_then(|first_next| first_next.pred_opt())
        .map(|last| last.day())
        .unwrap_or(28)
}

impl Render for DatePickerState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let border = theme.tokens.border;
        let popover = theme.tokens.popover;
        let accent = theme.tokens.accent.color;
        let muted_foreground = theme.tokens.muted_foreground.color;

        let year = self.viewing.year();
        let month = self.viewing.month();
        // 周一起始的空白格数。
        let lead = NaiveDate::from_ymd_opt(year, month, 1)
            .map(|d| d.weekday().num_days_from_monday() as usize)
            .unwrap_or(0);
        let total = days_in_month(year, month) as usize;
        let selected = self.selected;
        let active_day = self.active_day;
        let panel = cx.entity();
        let instance = self.instance;
        let focus_handle = self.focus_handle.clone();

        let mut grid = div().flex().flex_col().gap(px(2.0));
        // 星期头
        grid = grid.child(div().flex().flex_row().children(
            ["一", "二", "三", "四", "五", "六", "日"].iter().map(|d| {
                div()
                    .w(px(28.0))
                    .text_center()
                    .text_xs()
                    .text_color(muted_foreground)
                    .child(*d)
            }),
        ));
        // 日期格（按周分行，共 42 格）
        for week in 0..6 {
            let mut row = div().flex().flex_row().gap(px(2.0));
            for col in 0..7 {
                let cell = week * 7 + col;
                if cell < lead || cell >= lead + total {
                    row = row.child(div().w(px(28.0)).h(px(28.0)));
                    continue;
                }
                let day = (cell - lead + 1) as u32;
                let panel = panel.clone();
                // 日历网格由合法年月 + 格号推导，必构成合法日期
                let date = invariant(
                    NaiveDate::from_ymd_opt(year, month, day),
                    "日期选择器：网格年月日必构成合法日期",
                );
                let is_selected = selected == Some(date);
                let is_active = active_day == date;
                let label = format!("{day}");
                row = row.child(
                    div()
                        .id(SharedString::from(format!("dp-{instance}-{day}")))
                        .role(accesskit::Role::GridCell)
                        .aria_label(format!("{year}年{month}月{day}日"))
                        .aria_selected(is_selected)
                        .when(is_active, |this| this.aria_active_descendant())
                        .w(px(28.0))
                        .h(px(28.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_sm()
                        .cursor_pointer()
                        .text_sm()
                        .border_1()
                        // 活动日描边、选中日填充，两者可同时出现。
                        .border_color(if is_active {
                            accent
                        } else {
                            transparent_black()
                        })
                        .text_color(if is_selected {
                            accent
                        } else {
                            muted_foreground
                        })
                        .when(is_selected, |this| this.bg(accent.opacity(0.15)))
                        .when(!is_selected, |this| {
                            this.hover(|this| this.bg(accent.opacity(0.08)))
                        })
                        .child(label)
                        .debug_selector(move || format!("dp-day-{day}"))
                        .on_click(move |_, window, cx| {
                            panel.update(cx, |this, cx| this.pick(date, window, cx));
                        }),
                );
            }
            grid = grid.child(row);
        }

        div()
            .id(("dp-panel", instance))
            // 无障碍语义：日历根节点；键盘操作由面板统一处理。
            .role(accesskit::Role::Grid)
            .key_context("DatePicker")
            .track_focus(&focus_handle)
            .on_key_down(
                window.listener_for(&panel, |state, event: &KeyDownEvent, window, cx| {
                    state.on_key_down(event, window, cx);
                }),
            )
            .flex()
            .flex_col()
            .w(px(224.0))
            .bg(popover)
            .border_1()
            .border_color(border)
            .rounded_md()
            .p(px(8.0))
            .gap(px(6.0))
            // 月份导航行
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child({
                        let panel = panel.clone();
                        Button::new(format!("dp-{instance}-prev"))
                            .ghost()
                            .small()
                            .icon(IconName::ChevronLeft)
                            .on_click(move |_, window, cx| {
                                panel.update(cx, |this, cx| {
                                    this.focus(window, cx);
                                    this.shift_month(-1, cx);
                                });
                            })
                    })
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted_foreground)
                            .child(format!("{year} 年 {month} 月")),
                    )
                    .child({
                        let panel = panel.clone();
                        Button::new(format!("dp-{instance}-next"))
                            .ghost()
                            .small()
                            .icon(IconName::ChevronRight)
                            .on_click(move |_, window, cx| {
                                panel.update(cx, |this, cx| {
                                    this.focus(window, cx);
                                    this.shift_month(1, cx);
                                });
                            })
                    }),
            )
            .child(grid)
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::DatePickerState;
    use crate::{
        AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, TestAppContext,
        Window, div,
    };
    use chrono::NaiveDate;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering as AtomicOrdering},
    };

    /// 渲染日期选择器的测试宿主。
    struct Probe {
        date: Entity<DatePickerState>,
    }

    impl Render for Probe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(self.date.clone())
        }
    }

    /// 构建宿主视图，并把选择 / 关闭回调收集到返回的计数器中。
    fn setup(
        cx: &mut TestAppContext,
        initial: NaiveDate,
    ) -> (
        Entity<DatePickerState>,
        Arc<Mutex<Vec<NaiveDate>>>,
        Arc<AtomicUsize>,
        &mut crate::VisualTestContext,
    ) {
        cx.update(crate::theme::init);
        let changes: Arc<Mutex<Vec<NaiveDate>>> = Arc::default();
        let closes = Arc::new(AtomicUsize::new(0));
        let change_sink = changes.clone();
        let close_sink = closes.clone();
        let (probe, cx) = cx.add_window_view(move |_window, cx| {
            let date = cx.new(|cx| {
                DatePickerState::new(cx)
                    .select(initial)
                    .on_change(move |date, _, _| {
                        change_sink.lock().unwrap().push(date);
                    })
                    .on_close(move |_, _| {
                        close_sink.fetch_add(1, AtomicOrdering::Relaxed);
                    })
            });
            Probe { date }
        });
        let state = probe.read_with(cx, |probe, _| probe.date.clone());
        (state, changes, closes, cx)
    }

    /// 聚焦日历面板（模拟弹窗打开后的初始聚焦）。
    fn focus(state: &Entity<DatePickerState>, cx: &mut crate::VisualTestContext) {
        cx.update(|window, cx| {
            state.update(cx, |state, cx| state.focus(window, cx));
        });
    }

    /// 读取当前活动日。
    fn active_day(state: &Entity<DatePickerState>, cx: &mut crate::VisualTestContext) -> NaiveDate {
        state.read_with(cx, |state, _| state.active_day())
    }

    /// 合法日期（测试数据固定，不合法即测试前提错误）。
    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("测试日期必合法")
    }

    /// 方向键 ±1 / ±7 天与 Home / End 周首周末。
    #[rgpui::test]
    fn keyboard_moves_active_day(cx: &mut TestAppContext) {
        let (state, _changes, _closes, mut cx) = setup(cx, date(2024, 5, 15));
        focus(&state, &mut cx);

        cx.simulate_keystrokes("right");
        assert_eq!(active_day(&state, &mut cx), date(2024, 5, 16));
        cx.simulate_keystrokes("down");
        assert_eq!(active_day(&state, &mut cx), date(2024, 5, 23));
        cx.simulate_keystrokes("left up");
        assert_eq!(active_day(&state, &mut cx), date(2024, 5, 15));
        // 2024-05-15 是周三：Home 到周一 13 号，End 到周日 19 号。
        cx.simulate_keystrokes("home");
        assert_eq!(active_day(&state, &mut cx), date(2024, 5, 13));
        cx.simulate_keystrokes("end");
        assert_eq!(active_day(&state, &mut cx), date(2024, 5, 19));
    }

    /// 方向键跨月时展示月份跟随。
    #[rgpui::test]
    fn keyboard_moves_across_month(cx: &mut TestAppContext) {
        let (state, _changes, _closes, mut cx) = setup(cx, date(2024, 5, 31));
        focus(&state, &mut cx);

        cx.simulate_keystrokes("right");
        assert_eq!(active_day(&state, &mut cx), date(2024, 6, 1));
        cx.simulate_keystrokes("left");
        assert_eq!(active_day(&state, &mut cx), date(2024, 5, 31));
    }

    /// PageUp / PageDown 换月，日期钳制到目标月份的最后一天。
    #[rgpui::test]
    fn page_keys_switch_month(cx: &mut TestAppContext) {
        let (state, _changes, _closes, mut cx) = setup(cx, date(2024, 5, 15));
        focus(&state, &mut cx);

        cx.simulate_keystrokes("pagedown");
        assert_eq!(active_day(&state, &mut cx), date(2024, 6, 15));
        cx.simulate_keystrokes("pageup pageup");
        assert_eq!(active_day(&state, &mut cx), date(2024, 4, 15));
    }

    /// 闰年 1 月 31 日换到 2 月时钳制为 29 日。
    #[rgpui::test]
    fn page_key_clamps_day(cx: &mut TestAppContext) {
        let (state, _changes, _closes, mut cx) = setup(cx, date(2024, 1, 31));
        focus(&state, &mut cx);

        cx.simulate_keystrokes("pagedown");
        assert_eq!(active_day(&state, &mut cx), date(2024, 2, 29));
    }

    /// Enter / Space 选中活动日并触发选择回调。
    #[rgpui::test]
    fn enter_selects_active_day(cx: &mut TestAppContext) {
        let (state, changes, _closes, mut cx) = setup(cx, date(2024, 5, 15));
        focus(&state, &mut cx);

        cx.simulate_keystrokes("right space");
        assert_eq!(
            state.read_with(cx, |state, _| state.selected()),
            Some(date(2024, 5, 16))
        );
        assert_eq!(*changes.lock().unwrap(), vec![date(2024, 5, 16)]);

        cx.simulate_keystrokes("down enter");
        assert_eq!(
            state.read_with(cx, |state, _| state.selected()),
            Some(date(2024, 5, 23))
        );
        assert_eq!(
            *changes.lock().unwrap(),
            vec![date(2024, 5, 16), date(2024, 5, 23)]
        );
    }

    /// Esc 触发关闭回调。
    #[rgpui::test]
    fn escape_closes(cx: &mut TestAppContext) {
        let (state, _changes, closes, mut cx) = setup(cx, date(2024, 5, 15));
        focus(&state, &mut cx);

        cx.simulate_keystrokes("escape");
        assert_eq!(closes.load(AtomicOrdering::Relaxed), 1);
    }

    /// 鼠标点击日期格仍选中该天并触发回调。
    #[rgpui::test]
    fn mouse_click_selects_day(cx: &mut TestAppContext) {
        let (state, changes, _closes, mut cx) = setup(cx, date(2024, 5, 15));
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });

        let bounds = cx.debug_bounds("dp-day-16").expect("日期格边界");
        cx.simulate_click(bounds.center(), crate::Modifiers::default());
        assert_eq!(
            state.read_with(cx, |state, _| state.selected()),
            Some(date(2024, 5, 16))
        );
        assert_eq!(
            state.read_with(cx, |state, _| state.active_day()),
            date(2024, 5, 16)
        );
        assert_eq!(*changes.lock().unwrap(), vec![date(2024, 5, 16)]);
    }

    /// 无障碍树：根节点为 Grid，日期格为带选中状态的 GridCell，
    /// 活动日通过 `active_descendant_focus` 报告。
    #[rgpui::test]
    fn a11y_tree_reports_grid_cell_selection(cx: &mut TestAppContext) {
        let (state, _changes, _closes, mut cx) = setup(cx, date(2024, 5, 15));
        focus(&state, &mut cx);
        cx.simulate_keystrokes("right enter");

        cx.update(|window, cx| {
            window.force_a11y_active_for_test();
            let _ = window.draw(cx);
        });
        let json = cx
            .update(|window, _| window.debug_a11y_tree_json())
            .expect("无障碍树未捕获");
        let tree: serde_json::Value = serde_json::from_str(&json).expect("树 JSON 必可解析");
        let nodes = tree["nodes"].as_object().expect("nodes 必为对象");

        let (selected_key, selected) = nodes
            .iter()
            .find(|(_, node)| node["aria"]["label"].as_str() == Some("2024年5月16日"))
            .expect("选中日期格必在树中");
        assert_eq!(selected["aria"]["role"], "GridCell");
        assert_eq!(selected["aria"]["selected"], true);
        assert_eq!(
            tree["active_descendant_focus"].as_str(),
            Some(selected_key.as_str())
        );
        assert!(
            nodes.values().any(|node| node["aria"]["role"] == "Grid"),
            "日历根节点必为 Grid"
        );
    }
}
