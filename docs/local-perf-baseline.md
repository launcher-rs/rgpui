# 本机性能基线：1.8 vs 1.7（以老电脑为准）

## 目的与结论

本项目的目标是**普通电脑跑得更快**，因此性能验收一律以本机（一台约七八年前的 Windows 老电脑）为准，不采用任何特定硬件上的对标数字。

**结论（2026-11-21 本机实测）：1.8 在这台老电脑上不比 1.7 慢。**

| 场景 | v1.7.0 | 1.8（feat/1.8.0） | 差异 |
|---|---|---|---|
| 1000 行全表内容重绘（dynamic） | 6.9 FPS（首帧 2296 ms） | 6.9 FPS（首帧 2496 ms） | FPS 持平；首帧抖动范围内 |
| 1000 行内容不变、只重绘（static，测缓存） | 36.0 FPS（首帧 2601 ms） | 38.5 FPS（首帧 2252 ms） | FPS 约 +7%；首帧 1.8 更快 |

dynamic 与 static 各预热 1 次 + 正式 5 次取中位数，两版在同一台机器、同一窗口状态下跑完。首帧耗时单次波动较大（含冷启动、字体加载），仅作参考，不作为验收指标。

## 测量口径

- **工具**：`temp/perf-probe/src/bin/list.rs`（独立 workspace，用 `path` 依赖直接吃目标版本的 `crates/rgpui` 源码，不依赖发版）。`temp/` 被 `.gitignore` 忽略，是本地工具，不入库。
- **负载**：900×600 窗口，1000 行 × 14px 文本。
  - `--bench`（dynamic）：每帧 `tick+1` 让全部行的文本变化，迫使整表重新排版与绘制；`request_animation_frame()` 驱动，持续 3 秒后打印 `FPS = 帧数 / 墙钟`。
  - `--bench --static`（static）：文本不变，只重绘，测框架的文本/排版缓存。
- **为什么用 FPS 而不是 criterion**：criterion 基准（layout_line / scene_build / text_shaping / text_wrap）只覆盖 CPU 侧布局/shaping，不含 present；FPS 是**含窗口呈现的端到端口径**，才是老电脑用户能感知的指标。
- **置顶**：跑分脚本启动时把窗口设为前台置顶（`SetForegroundWindow` + `SetWindowPos(HWND_TOPMOST)`），否则窗口失焦会被系统降优先级，FPS 失真。
- **公平性**：两版用同一份探针源码，仅 `path` 依赖分别指向 `v1.7.0` 与 `feat/1.8.0` 的 worktree；release 配置一致（`opt-level=3` + LTO + `codegen-units=1` + `panic=abort` + `strip`）。

## 可复现步骤（本机）

1. 准备 1.7 对照 worktree（只需一次）：

   ```bash
   git worktree add /c/code/rgpui_workspace/rgpui-v170 v1.7.0
   cp -r temp/perf-probe /c/code/rgpui_workspace/rgpui-v170/temp/
   ```

2. 分别编译两版探针（release）：

   ```bash
   (cd temp/perf-probe && cargo build --release --bin rgpui-local-list)
   (cd /c/code/rgpui_workspace/rgpui-v170/temp/perf-probe && cargo build --release --bin rgpui-local-list)
   ```

   注意：本机磁盘长期接近满载，LTO 链接阶段需要约 1.2 GB 临时空间。**不要并行编译两版**（会写满磁盘导致 `os error 112`），串行编译；编译前后可用 `cargo clean` 或删除 `target/debug` 释放空间。

3. 跑分（每版各 1 次预热 + 5 次正式，取中位数）：

   ```powershell
   pwsh -NoProfile -File temp/perf-probe/compare.ps1 -Exe '<1.7 exe 路径>' -Runs 5
   pwsh -NoProfile -File temp/perf-probe/compare.ps1 -Exe '<1.8 exe 路径>' -Runs 5
   pwsh -NoProfile -File temp/perf-probe/compare.ps1 -Exe '<exe>' -Runs 5 -Static
   ```

## 局限与后续

- **绝对 FPS 低于历史 1.6 基线**：同一探针在 1.6 时期记录 dynamic 22.3 FPS，本次两版均为 6.9 FPS。两版一致说明差异来自环境（机器状态、后台进程）或 1.7 相对 1.6 的变化，而非 1.8 回退；本报告的结论只针对 1.8 vs 1.7。若需核对 1.6 → 1.7，可用同样方法加测 `v1.6.0` worktree。
- **GPU 时间未测**：本机没有可用的逐帧 GPU timestamp 方案；参考项目（gpui-fast / gpui-kit）也只测 CPU 时间与端到端 FPS，GPU 侧仅有利用率/显存，无耗时。既然验收口径是"老电脑体感帧率"，端到端 FPS 已足够；如需定位 GPU 热点再单独接 D3D11 timestamp query 或 wgpu timestamp-query。
- **已知 CPU 侧取舍仍在**：`text_wrap` 因 UAX #14 变慢 3.6×/2.4×、`Path` 描边 CPU 2–8×（见 `1.8-vs-1.7-upgrade.md`）。它们由 criterion 基准覆盖，不属于本次端到端 FPS 结论；批量软换行/长笔迹密集场景需单独评估。
- 磁盘紧张是本机的长期状态，跑分和编译前后注意清理构建产物。
