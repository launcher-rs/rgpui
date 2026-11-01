//! 场景构建基准：同样 1000 行，对比「逐行 `div`」与「`uniform_list`」。
//!
//! 只覆盖 CPU 侧（布局 + shaping + 绘制 + 场景构建）：bench 构建下
//! `cx.notify()` 的 effect flush 会同步 `Window::draw`，但不 `present`，
//! 因此不含 GPU 提交。

use criterion::{criterion_group, criterion_main};
use rgpui::prelude::*;
use rgpui::{
    AppContext, BenchAppContext, Context, IntoElement, Render, SharedString, Window, div, px,
    uniform_list, v_flex,
};
use std::fmt;
use std::rc::Rc;

/// 基准行数。
const LINE_COUNT: usize = 1000;

/// 行高（`uniform_list` 以首行高度测量全部行）。
const ROW_HEIGHT: f32 = 24.0;

/// 两种布局方式。
#[derive(Clone, Copy)]
enum SceneMode {
    /// 逐行 `div` 全量渲染。
    Divs,
    /// 等高虚拟列表，只渲染可视区。
    Uniform,
}

impl fmt::Display for SceneMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Divs => "divs",
            Self::Uniform => "uniform_list",
        })
    }
}

/// 生成唯一文本语料。
fn corpus() -> Rc<Vec<SharedString>> {
    Rc::new(
        (0..LINE_COUNT)
            .map(|index| {
                SharedString::from(format!(
                    "row {index:04}: the quick brown fox jumps over the lazy dog 0123456789"
                ))
            })
            .collect(),
    )
}

/// 基准根视图。
struct SceneView {
    mode: SceneMode,
    lines: Rc<Vec<SharedString>>,
}

impl SceneView {
    /// 创建基准根视图。
    fn new(mode: SceneMode, lines: Rc<Vec<SharedString>>) -> Self {
        Self { mode, lines }
    }
}

impl Render for SceneView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let rows = match self.mode {
            SceneMode::Divs => v_flex()
                .size_full()
                .children(
                    self.lines
                        .iter()
                        .map(|line| div().h(px(ROW_HEIGHT)).child(line.clone())),
                )
                .into_any_element(),
            SceneMode::Uniform => {
                let lines = self.lines.clone();
                uniform_list("scene-build", lines.len(), move |range, _window, _cx| {
                    range
                        .map(|index| div().h(px(ROW_HEIGHT)).child(lines[index].clone()))
                        .collect::<Vec<_>>()
                })
                .h_full()
                .into_any_element()
            }
        };

        v_flex().size_full().child(rows)
    }
}

/// 场景构建基准。
#[rgpui::bench(inputs = [SceneMode::Divs, SceneMode::Uniform], input_name = "layout")]
fn scene_build(mode: &SceneMode, cx: &mut BenchAppContext<'_, '_>) {
    let mode = *mode;
    let lines = corpus();
    let mut window = cx.add_empty_window();
    let view = window
        .update(|window, cx| window.replace_root(cx, |_, _| SceneView::new(mode, lines.clone())));
    window.run_until_idle();

    cx.bench_iter(|cx| {
        cx.with_window(view.entity_id(), |_window, cx| {
            view.update(cx, |_view, cx| cx.notify());
        })
        .expect("场景基准窗口应存在");
    });

    window.run_until_idle();
}

criterion_group!(benches, scene_build);
criterion_main!(benches);
