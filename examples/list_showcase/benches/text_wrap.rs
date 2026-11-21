//! 软换行基准：拉丁与 CJK 语料各 10000 行，走 `LineWrapper::wrap_line`
//! （与 editor `DisplayMap` 相同的换行路径，含东亚禁则否决）。
//!
//! 对应借鉴批次 C2 的「先测量后动手」：基线数据用于评估 gpui-kit
//! `514f243d`（大文档/非 ASCII 软换行加速）思路的落地收益。
//! 每次迭代复用同一个 `LineWrapperHandle`（字符宽度缓存跨行复用，
//! 与真实 `DisplayMap` 用法一致）；`collect` 计入消费侧 Vec 分配。

use criterion::{criterion_group, criterion_main};
use rgpui::{LineFragment, font, px};
use std::hint::black_box;

/// 基准行数。
const LINE_COUNT: usize = 10_000;

/// 拉丁语料：约 90 字符/行的代码行（唯一，避免命中缓存偏差）。
fn latin_corpus() -> Vec<String> {
    (0..LINE_COUNT)
        .map(|index| {
            format!(
                "[{index:04}] fn handle_event(&mut self, event: &Event) -> Result<()> {{ self.count += {index}; }}"
            )
        })
        .collect()
}

/// CJK 语料：约 45 字/行混排（触发逐字候选断点 + 禁则否决）。
fn cjk_corpus() -> Vec<String> {
    (0..LINE_COUNT)
        .map(|index| {
            format!("第{index:04}行：文本系统对中西文混排逐字换行，句号「。」与开引号（「）都要遵守禁则。")
        })
        .collect()
}

/// 拉丁软换行基准。
#[rgpui::bench]
fn wrap_latin(cx: &mut rgpui::BenchAppContext<'_, '_>) {
    let text_system = cx.read(|cx| cx.text_system().clone());
    let lines = latin_corpus();
    cx.bench_iter(|_| {
        let mut wrapper = text_system.line_wrapper(font(".SystemUIFont"), px(14.0));
        for line in &lines {
            let wrapped = wrapper
                .wrap_line(
                    std::slice::from_ref(&LineFragment::text(line.as_str())),
                    px(600.0),
                )
                .collect::<Vec<_>>();
            black_box(wrapped);
        }
    });
}

/// CJK 软换行基准（禁则路径）。
#[rgpui::bench]
fn wrap_cjk(cx: &mut rgpui::BenchAppContext<'_, '_>) {
    let text_system = cx.read(|cx| cx.text_system().clone());
    let lines = cjk_corpus();
    cx.bench_iter(|_| {
        let mut wrapper = text_system.line_wrapper(font(".SystemUIFont"), px(14.0));
        for line in &lines {
            let wrapped = wrapper
                .wrap_line(
                    std::slice::from_ref(&LineFragment::text(line.as_str())),
                    px(600.0),
                )
                .collect::<Vec<_>>();
            black_box(wrapped);
        }
    });
}

criterion_group!(benches, wrap_latin, wrap_cjk);
criterion_main!(benches);
