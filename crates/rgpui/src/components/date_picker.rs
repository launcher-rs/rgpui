//! 日期选择器。
//!
//! 月历弹窗面板：年月导航 + 日期格 + 选择回调。状态由实体持有，
//! 父组件经 `cx.new(|_| DatePickerState::new())` 创建。

use crate::{prelude::*, *};
use chrono::{Datelike, NaiveDate};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

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
    /// 选择回调。
    on_change: Option<Arc<dyn Fn(NaiveDate, &mut Window, &mut App) + Send + Sync + 'static>>,
}

impl DatePickerState {
    /// 创建日期选择器（默认当月，可先 `select` 预填）。
    pub fn new() -> Self {
        let today = chrono::Local::now().date_naive();
        Self {
            instance: DATE_PICKER_ID.fetch_add(1, Ordering::Relaxed),
            viewing: today,
            selected: None,
            on_change: None,
        }
    }

    /// 预填选中日期（同时切到该月）。
    pub fn select(mut self, date: NaiveDate) -> Self {
        self.viewing = date;
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

    /// 已选日期。
    pub fn selected(&self) -> Option<NaiveDate> {
        self.selected
    }

    /// 切到上/下个月（`delta` 为月份偏移，可为负）。
    fn shift_month(&mut self, delta: i32, cx: &mut Context<Self>) {
        let months = self.viewing.year() * 12 + self.viewing.month() as i32 - 1 + delta;
        let year = months.div_euclid(12);
        let month = months.rem_euclid(12) as u32 + 1;
        if let Some(first) = NaiveDate::from_ymd_opt(year, month, 1) {
            self.viewing = first;
            cx.notify();
        }
    }

    /// 选中某天。
    fn pick(&mut self, date: NaiveDate, cx: &mut Context<Self>) {
        self.selected = Some(date);
        cx.notify();
    }
}

impl Default for DatePickerState {
    fn default() -> Self {
        Self::new()
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let on_change = self.on_change.clone();
        let panel = cx.entity();
        let instance = self.instance;

        let mut grid = div().flex().flex_col().gap(px(2.0));
        // 星期头
        grid = grid.child(
            div()
                .flex()
                .flex_row()
                .children(["一", "二", "三", "四", "五", "六", "日"].iter().map(|d| {
                    div()
                        .w(px(28.0))
                        .text_center()
                        .text_xs()
                        .text_color(muted_foreground)
                        .child(*d)
                })),
        );
        // 日期格（按周分行，共 42 格）
        for week in 0..6 {
            let panel = panel.clone();
            let mut row = div().flex().flex_row().gap(px(2.0));
            for col in 0..7 {
                let cell = week * 7 + col;
                if cell < lead || cell >= lead + total {
                    row = row.child(div().w(px(28.0)).h(px(28.0)));
                    continue;
                }
                let day = (cell - lead + 1) as u32;
                let panel = panel.clone();
                let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
                let is_selected = selected == Some(date);
                let label = format!("{day}");
                let on_change = on_change.clone();
                row = row.child(
                    div()
                        .id(SharedString::from(format!("dp-{instance}-{day}")))
                        .w(px(28.0))
                        .h(px(28.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_sm()
                        .cursor_pointer()
                        .text_sm()
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
                        .on_click(move |_, window, cx| {
                            let on_change = on_change.clone();
                            panel.update(cx, |this, cx| {
                                this.pick(date, cx);
                                if let Some(ref cb) = on_change {
                                    cb(date, window, cx);
                                }
                            });
                        }),
                );
            }
            grid = grid.child(row);
        }

        div()
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
                        Button::new("dp-prev")
                            .ghost()
                            .small()
                            .icon(IconName::ChevronLeft)
                            .on_click(move |_, _, cx| {
                                panel.update(cx, |this, cx| this.shift_month(-1, cx));
                            })
                    })
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted_foreground)
                            .child(format!("{year} 年 {month} 月")),
                    )
                    .child({
                        Button::new("dp-next")
                            .ghost()
                            .small()
                            .icon(IconName::ChevronRight)
                            .on_click(move |_, _, cx| {
                                panel.update(cx, |this, cx| this.shift_month(1, cx));
                            })
                    }),
            )
            .child(grid)
    }
}
