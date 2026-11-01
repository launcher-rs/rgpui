# FPS HUD（性能浮层）

**目录：** [概述](#概述) · [FpsHudState](#fpshudstate) · [FpsHud](#fpshud) · [最佳实践](#最佳实践)

## 概述

`crates/rgpui/src/fps_hud.rs`（`pub mod fps_hud;` `rgpui.rs:281`），prelude 重导出
`FpsHud`/`FpsHudState`（`prelude.rs:64`）。用于显示 FPS/CPU/内存/绘制耗时的调试浮层。
示例：`cargo run -p showcase --bin fps_hud`。

`DRAW` 行默认读取 [`Window::frame_stats`] 的 `avg_ms`（CPU 侧 draw 段：布局 +
场景构建 + 文本绘制，不含 GPU 提交与桌面合成），也可经
[`FpsHudState::update_draw_ms`] 用自己的计时口径覆盖。

## FpsHudState

`FpsHudState`（`fps_hud.rs:28`）：

| 方法 | 行号 | 说明 |
|------|------|------|
| `new()` | `:57` | |
| `update_fps(fps: f64)` | `:72` | 直接设置 FPS |
| `record_frame(now: Instant)` | `:81` | 记录一帧（自动统计） |
| `reset_sampling()` | `:101` | 重置采样 |
| `update_cpu(usage: f64)` | `:106` | |
| `update_memory(mb: f64)` | `:115` | |
| `update_draw_ms(avg_ms: f64)` | `:122` | 覆盖窗口统计的 DRAW 值；置 0 回退 |
| `toggle()` | `:127` | 显示/隐藏 |
| `average_fps() -> f64` | `:136` | |

## FpsHud

`FpsHud`（`:142`）：`new(Entity<FpsHudState>)`（`:156`）、`position(Point<Pixels>)`（`:167`）、
`show_cpu(bool)`（`:173`）、`show_memory(bool)`（`:179`）、`continuous(bool)`（`:188`）。

```rust
let hud = cx.new(|_| FpsHudState::new());
// 渲染：FpsHud::new(hud).position(px(8.).into())
```

## 最佳实践

- **调试期用 `record_frame` 自动统计**，或外部算好 FPS 用 `update_fps`。
- **`continuous(true)`** 用于持续刷新；否则按需 `toggle()`。
- **排查性能回归先看 `DRAW`**：它来自 `Window::frame_stats().avg_ms`；需要程序化
  采集时直接读 `window.frame_stats()`，或设 `ZED_MEASUREMENTS=1 RUST_LOG=info`
  输出 `[fast]` 单行快照。DRAW 是 CPU 绘制耗时，不等同于端到端帧时间。
