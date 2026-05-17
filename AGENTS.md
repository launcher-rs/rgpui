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

## 注意事项

- `gpui_platform` 是示例代码的入口 crate，通过 `cfg(target_os)` 选择平台实现
- 所有中文注释使用简体中文
- 编辑文件后务必运行 `cargo check --workspace` 确认无警告
- 新增 `PlatformWindow` trait 方法时，需同步更新 `gpui/src/platform.rs`、`gpui/src/platform/test/window.rs`、`gpui/src/platform/visual_test.rs` 和各平台实现
