//! Retained headless 基准：同一场景下增量 vs 全量的逐帧耗时（P2d）。
//!
//! 对标 `gpui-fast` 的 `retained_bench`：面板 × 标签仪表盘，按每帧变更面板数
//! 分档（0 / 1 / 6 / 全部），A 窗保留开启、B 窗保留关闭交替绘制，报告平均帧耗时。
//!
//! 只在 release 下跑（debug 数字无意义）：
//!
//! ```sh
//! cargo test -p rgpui --lib --release fast::tests::retained_bench -- --ignored --nocapture
//! ```
//!
//! 判定口径：同场景同窗口尺寸；预热后取均值；A/B 交替绘制压住机器温漂。
//! 变更面板一律 `notify`（真实应用契约；无 notify 自更新不在 oracle 覆盖内）。

use crate::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString,
    TestAppContext, Window, div, px, size, v_flex,
};
use std::{cell::RefCell, rc::Rc, time::Instant};

/// 基准面板数 × 每面板标签数（对标 gpui-fast 的 60×64）。
const PANELS: usize = 60;
/// 每面板标签数。
const LABELS: usize = 64;
/// 预热帧（字体加载、布局缓存就绪后才计时）。
const WARMUP_FRAMES: usize = 10;
/// 计时帧。
const MEASURED_FRAMES: usize = 60;

/// 基准面板：`tick` 变化即内容变化（调用方负责 `notify`）。
struct BenchPanel {
    id: usize,
    tick: usize,
}

impl Render for BenchPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().children((0..LABELS).map(|ix| {
            div().child(SharedString::from(format!(
                "p{}-l{ix} t{}",
                self.id, self.tick
            )))
        }))
    }
}

/// 仪表盘根视图（仅组装面板，无自身读取；变更靠面板 notify 向上传脏）。
struct BenchRoot {
    panels: Vec<Entity<BenchPanel>>,
}

impl Render for BenchRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().children(self.panels.iter().map(|panel| panel.clone()))
    }
}

/// 一档测量：每帧通知前 `changed` 个面板，交替绘制 A（增量）B（全量），
/// 返回双方均值毫秒与双方（复用累计，重建累计）。
fn measure_row(
    test_app: &mut TestAppContext,
    any_a: crate::AnyWindowHandle,
    any_b: crate::AnyWindowHandle,
    panels_a: &[Entity<BenchPanel>],
    panels_b: &[Entity<BenchPanel>],
    changed: usize,
) -> (f64, f64, (u64, u64), (u64, u64)) {
    // 预热（不计时）。
    for _ in 0..WARMUP_FRAMES {
        test_app.update(|cx| {
            notify_first(cx, panels_a, changed);
            notify_first(cx, panels_b, changed);
        });
        draw_window(test_app, any_a);
        draw_window(test_app, any_b);
    }
    // 计时（逐帧交替绘制顺序，消除先画/后画的系统性偏差）。
    let mut nanos_a = 0u128;
    let mut nanos_b = 0u128;
    for i in 0..MEASURED_FRAMES {
        test_app.update(|cx| {
            notify_first(cx, panels_a, changed);
            notify_first(cx, panels_b, changed);
        });
        if i % 2 == 0 {
            nanos_a += draw_window(test_app, any_a);
            nanos_b += draw_window(test_app, any_b);
        } else {
            nanos_b += draw_window(test_app, any_b);
            nanos_a += draw_window(test_app, any_a);
        }
    }
    let stats_a = test_app
        .update_window(any_a, |_, window, _| window.fast_stats.snapshot())
        .unwrap();
    let stats_b = test_app
        .update_window(any_b, |_, window, _| window.fast_stats.snapshot())
        .unwrap();
    (
        nanos_a as f64 / MEASURED_FRAMES as f64 / 1_000_000.0,
        nanos_b as f64 / MEASURED_FRAMES as f64 / 1_000_000.0,
        (stats_a.views_reused, stats_a.views_rebuilt),
        (stats_b.views_reused, stats_b.views_rebuilt),
    )
}

/// 通知前 `changed` 个面板（`tick` 推进 + `notify`）。
fn notify_first(cx: &mut App, panels: &[Entity<BenchPanel>], changed: usize) {
    for panel in panels.iter().take(changed) {
        panel.update(cx, |panel, cx| {
            panel.tick += 1;
            cx.notify();
        });
    }
}

/// 绘制一窗并返回耗时纳秒。
fn draw_window(test_app: &mut TestAppContext, window: crate::AnyWindowHandle) -> u128 {
    test_app
        .update_window(window, |_, window, cx| {
            let started = Instant::now();
            window.draw(cx).clear(cx);
            started.elapsed().as_nanos()
        })
        .unwrap()
}

/// 仪表盘基准主入口（`#[ignore]`，仅 release 手动跑）。
#[test]
#[ignore]
fn dashboard_retained_vs_from_scratch() {
    let mut test_app = TestAppContext::single();
    let stash_a: Rc<RefCell<Vec<Entity<BenchPanel>>>> = Rc::default();
    let stash_b: Rc<RefCell<Vec<Entity<BenchPanel>>>> = Rc::default();
    let window_size = size(px(1200.), px(800.));
    let win_a = test_app.open_window(window_size, {
        let stash = stash_a.clone();
        move |_, cx| {
            let panels = (0..PANELS)
                .map(|id| cx.new(|_| BenchPanel { id, tick: 0 }))
                .collect::<Vec<_>>();
            *stash.borrow_mut() = panels.clone();
            BenchRoot { panels }
        }
    });
    let win_b = test_app.open_window(window_size, {
        let stash = stash_b.clone();
        move |_, cx| {
            let panels = (0..PANELS)
                .map(|id| cx.new(|_| BenchPanel { id, tick: 0 }))
                .collect::<Vec<_>>();
            *stash.borrow_mut() = panels.clone();
            BenchRoot { panels }
        }
    });
    let any_a = win_a.into();
    let any_b = win_b.into();
    let (panels_a, panels_b) = (stash_a.borrow().clone(), stash_b.borrow().clone());

    // B 窗关闭保留：逐帧全量重建，作为对照基线。
    test_app
        .update_window(any_b, |_, window, _| {
            window.set_retention_override(Some(false));
        })
        .unwrap();

    println!("panels={PANELS} labels={LABELS} frames={MEASURED_FRAMES} (release, headless)");
    println!(
        "changed/frame | retained avg | from-scratch avg | delta | A(reused,rebuilt) | B(reused,rebuilt)"
    );
    for changed in [0, 1, 6, PANELS] {
        let (retained_ms, baseline_ms, stats_a, stats_b) =
            measure_row(&mut test_app, any_a, any_b, &panels_a, &panels_b, changed);
        let delta = (baseline_ms - retained_ms) / baseline_ms * 100.0;
        println!(
            "{changed:>13} | {retained_ms:>11.3}ms | {baseline_ms:>16.3}ms | {delta:+.1}% | {stats_a:?} | {stats_b:?}"
        );
    }
}
