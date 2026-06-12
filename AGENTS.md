# AGENTS.md - rgpui 开发指南

## 项目概述

rgpui 是基于 Zed 的 GPU 加速 UI 框架 `gpui` 的跨平台移植。采用 Rust workspace 架构，核心库 `gpui` 通过 `Platform` trait 抽象各平台实现。

## 架构与包边界

```
crates/
├── gpui/           # 核心 UI 框架，平台无关逻辑
├── gpui_platform/  # 平台选择入口，根据 cfg 选择具体平台 crate
├── gpui_windows/   # Windows 平台实现（windows-rs 绑定）
├── gpui_macos/     # macOS 平台实现
├── gpui_linux/     # Linux 平台实现
├── gpui_web/       # Web/WASM 平台实现
├── gpui_wgpu/      # wgpu 渲染后端
├── gpui_macros/    # 过程宏
└── gpui_tokio/     # Tokio 异步运行时集成
```

- `gpui/src/platform.rs` 定义了 `Platform` trait 和 `PlatformWindow` trait，所有平台必须实现
- 示例代码使用 `gpui_platform::application()` 获取平台应用入口
- 平台特有代码放在对应 `gpui_<platform>/` crate 中

## 开发命令

```bash
# 检查整个 workspace（推荐日常使用，比 build 快）
cargo check --workspace

# 构建单个示例
cargo build --example tray

# 运行示例
cargo run --example tray

# 运行所有测试
cargo test --workspace

# 运行单个包的测试
cargo test -p gpui

# Clippy 检查
cargo clippy --workspace
```

## Clippy 规则

workspace 级别拒绝 `dbg_macro` 和 `todo`。`style` lint 规则设为 `allow`（因为 Zed 上游跑 clippy 很慢），但以下规则为 `deny`：
- `declare_interior_mutable_const`
- `redundant_clone`
- `disallowed_methods`

## 托盘（System Tray）实现

托盘功能在 `gpui` + `gpui_windows` 中实现，关键文件：

| 文件 | 说明 |
|------|------|
| `gpui/src/tray.rs` | `TrayMenuItem`、`TrayIconEvent` 等公开类型 |
| `gpui/src/app.rs` | `set_tray_icon`、`set_tray_menu`、`on_tray_menu_action` 等 App 方法 |
| `gpui_windows/src/tray.rs` | `WindowsTray` 结构体，`Shell_NotifyIconW` 集成 |
| `gpui_windows/src/platform.rs` | 消息循环处理 `WM_GPUI_TRAY_ICON` 和 `WM_COMMAND` |

### 托盘图标格式

- `set_tray_icon()` 接受 PNG/ICO 格式的字节数据
- `create_hicon_from_bytes()` 先用 `LookupIconIdFromDirectoryEx` 尝试 ICO 解析，失败后用 `image` crate 解码 PNG
- 示例使用 `include_bytes!("image/app-icon.png")` 嵌入图标

### 窗口隐藏/恢复

- 关闭按钮调用 `window.hide_window()` → `ShowWindowAsync(hwnd, SW_HIDE)` 完全隐藏窗口（从任务栏移除）
- 托盘菜单"显示窗口"调用 `window.activate_window()` → 检测 `!IsWindowVisible` 后调用 `SW_SHOWNORMAL` 恢复
- `minimize_window()` 和 `hide_window()` 的区别：最小化保留任务栏图标，隐藏则不保留

## 窗口生命周期

```
PlatformWindow trait 关键方法:
- minimize() → SW_MINIMIZE
- hide() → SW_HIDE (从任务栏移除)
- activate() → SW_RESTORE / SW_SHOWNORMAL + SetForegroundWindow
```

## 提交与推送规范
- **推送前必须检查**：执行 `cargo check --workspace` 和 `cargo check --workspace --examples`，确保没有任何错误和警告
- **推送前格式化代码**: 执行 `cargo fmt` 格式化代码（注意 `rgpui-linux/src/linux/platform.rs` 有 Rust 2024 edition 解析问题，`cargo fmt` 会报错，需跳过该文件）
- **禁止使用 `#[allow(dead_code)]`**：未使用的代码应当删除或重构，不得使用属性压制警告

## 跨平台检查

### 问题背景

Windows 下 `cargo check` 只编译 `#[cfg(target_os = "windows")]` 和通用代码，`#[cfg(target_os = "macos")]` 和 `#[cfg(target_os = "linux")]` 中的代码不会被编译。合并上游 PR 后容易把 Linux/macOS 代码弄坏。

### 本地跨目标检查

安装目标：
```bash
rustup target add x86_64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin
```

跨平台检查（只检查 Rust 语法和类型，不需要真机）：
```bash
# 检查 Linux 代码
cargo check --target x86_64-unknown-linux-gnu

# 检查 macOS Intel 代码
cargo check --target x86_64-apple-darwin

# 检查 macOS ARM 代码
cargo check --target aarch64-apple-darwin
```

> **注意**：由于平台绑定的原生依赖（windows-rs、objc2、tree-sitter 的 C 库、psm/stacker），跨目标检查在 Windows 上几乎不可行——缺少对应平台的 C 编译器（如 `x86_64-linux-gnu-gcc`）。真正的跨平台验证依赖 CI（GitHub Actions 矩阵构建）。

### Feature 组合检查

使用 `cargo-hack` 验证所有 feature 组合：
```bash
cargo hack check --each-feature --workspace --ignore-private
```

### GitHub Actions CI

项目配置了 CI（`.github/workflows/ci.yml`），在 push/PR 时自动执行：

| 任务 | 平台 | 内容 |
|------|------|------|
| `check` | windows-latest / ubuntu-latest / macos-latest | `cargo check --workspace` + `cargo test --workspace` + `cargo clippy` |
| `feature-check` | ubuntu-latest | `cargo hack check --each-feature` |
| `lint` | ubuntu-latest | `cargo fmt --all --check` |

CI 通过矩阵策略在三个平台分别运行，确保跨平台兼容性。

## 已知问题

### `screen-capture` / `scap` feature

原依赖 `zed-scap`（Zed 维护的 fork）因基于 `windows-capture` 1.3.x API 而无法兼容 1.5.x。已切换到 crates.io 上官方 `scap = "0.0.8"`，该版本依赖兼容的 `windows-capture` 版本，编译通过。

### 跨目标检查在 Windows 上不可行

`psm`/`stacker` 等 crate 依赖 C 编译器，Windows 上缺少对应平台（如 `x86_64-linux-gnu-gcc`、`cc` for macOS）的交叉编译工具链。真正的跨平台验证依赖 GitHub Actions 矩阵构建。

## 上游 PR 合并

需要合并上游 PR 时，先读取 `.opencode/merge-upstream-workflow.md` 并按说明执行。

- PR 状态追踪: `UPSTREAM-PRS.json`
- 上游仓库规则: `.opencode/upstream-rules.json`

## 代码规范

- **所有函数必须添加中文注释**：公开 API 和内部函数均需使用简体中文说明功能、参数和返回值
- 注释风格遵循 Rust 文档规范（`///` 用于公开 API，`//` 用于内部逻辑）
- 避免使用英文注释，保持项目语言统一