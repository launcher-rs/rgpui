//! 文本 shaping 基准：1000 行、每行约 80 字符的唯一 ASCII 文本。
//!
//! 每次迭代都新建一个 `WindowTextSystem`，让 `layout_line` 真正走平台 shaping；
//! 它背后的 `TextSystem`（字体表）仍共享，字体解析不会变成测量噪声。
//! Criterion 输出的是 1000 行的总耗时，除以 1000 即得单行 µs。

use criterion::{criterion_group, criterion_main};
use rgpui::{TextRun, WindowTextSystem, black, font, px};
use std::hint::black_box;

/// 基准行数。
const LINE_COUNT: usize = 1000;

/// 生成唯一文本语料（约 90 字符/行）。
fn corpus() -> Vec<String> {
    (0..LINE_COUNT)
        .map(|index| {
            format!(
                "[{index:04}] fn handle_event(&mut self, event: &Event) -> Result<()> {{ self.count += {index}; }}"
            )
        })
        .collect()
}

/// 单行文本 shaping 基准。
#[rgpui::bench]
fn text_shaping(cx: &mut rgpui::BenchAppContext<'_, '_>) {
    let text_system = cx.read(|cx| cx.text_system().clone());
    let font = font(".SystemUIFont");
    let lines = corpus();
    let runs: Vec<TextRun> = lines
        .iter()
        .map(|line| TextRun {
            len: line.len(),
            font: font.clone(),
            color: black(),
            ..TextRun::default()
        })
        .collect();

    cx.bench_iter(|_| {
        let text_system = WindowTextSystem::new(text_system.clone());
        for (line, run) in lines.iter().zip(&runs) {
            let layout = text_system.layout_line(line, px(14.0), std::slice::from_ref(run), None);
            black_box(layout);
        }
    });
}

criterion_group!(benches, text_shaping);
criterion_main!(benches);
