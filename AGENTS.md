# AGENTS.md - rgpui 开发指南

## 项目概述

rgpui 是基于 Zed 的 GPU 加速 UI 框架 `gpui` 的跨平台移植。采用 Rust workspace 架构，核心库 `rgpui` 通过 `Platform` trait 抽象各平台实现。

## 架构与包边界

```
crates/
├── rgpui/                   # 核心 UI 框架，平台无关逻辑
├── rgpui-3d/                # 3D 渲染支持
├── rgpui-adabraka-ui/       # Adabraka UI 组件库
├── rgpui-character/         # 字符/文本处理
├── rgpui-linux/             # Linux 平台实现
├── rgpui-macos/             # macOS 平台实现
├── rgpui-macros/            # 过程宏
├── rgpui-platform/          # 平台选择入口，根据 cfg 选择具体平台 crate
├── rgpui-term/              # 终端组件
├── rgpui-tokio/             # Tokio 异步运行时集成
├── rgpui-web/               # Web/WASM 平台实现
├── rgpui-wgpu/              # wgpu 渲染后端
├── rgpui-windows/           # Windows 平台实现（windows-rs 绑定）
├── rgpui-yororen-ui/        # Yororen UI 组件库
└── rgpui-component-workspce/  # 组件子工作区（独立 Cargo workspace）
    ├── rgpui-component/          # 通用 UI 组件框架
    ├── rgpui-component-assets/   # 组件资源文件（图标、图片等）
    ├── rgpui-component-macros/   # 组件过程宏
    ├── rgpui-component-story/    # 组件 Storybook（原生）
    ├── rgpui-component-story-web/ # 组件 Storybook（WASM/Web）
    ├── rgpui-webview/            # WebView 组件
    └── themes/                   # 22 套 JSON 颜色主题
```

- `rgpui/src/platform.rs` 定义了 `Platform` trait 和 `PlatformWindow` trait，所有平台必须实现
- 示例代码使用 `rgpui_platform::application()` 获取平台应用入口
- 平台特有代码放在对应 `rgpui-<platform>/` crate 中

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
cargo test -p rgpui

# Clippy 检查
cargo clippy --workspace
```

## Clippy 规则

workspace 级别拒绝 `dbg_macro` 和 `todo`。`style` lint 规则设为 `allow`（因为 Zed 上游跑 clippy 很慢），但以下规则为 `deny`：
- `declare_interior_mutable_const`
- `redundant_clone`
- `disallowed_methods`

## 托盘（System Tray）实现

托盘功能在 `rgpui` + `rgpui-windows` 中实现，关键文件：

| 文件 | 说明 |
|------|------|
| `rgpui/src/tray.rs` | `TrayMenuItem`、`TrayIconEvent` 等公开类型 |
| `rgpui/src/app.rs` | `set_tray_icon`、`set_tray_menu`、`on_tray_menu_action` 等 App 方法 |
| `rgpui-windows/src/tray.rs` | `WindowsTray` 结构体，`Shell_NotifyIconW` 集成 |
| `rgpui-windows/src/platform.rs` | 消息循环处理 `WM_GPUI_TRAY_ICON` 和 `WM_COMMAND` |

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
| `feature-check` | ubuntu-latest | `cargo hack check --each-feature`（排除已知故障的 `scap`/`screen-capture` feature） |
| `lint` | ubuntu-latest | `cargo fmt --all --check` |

CI 通过矩阵策略在三个平台分别运行，确保跨平台兼容性。

## 已知问题

### `screen-capture` / `scap` feature 编译失败

`cargo hack check --each-feature` 中 `rgpui` 的 `screen-capture` feature 因其依赖的 `zed-scap` 与 `windows-capture` 1.5.0 API 不兼容（函数参数数量变化），暂时无法通过编译。CI 中已通过 `--exclude-features scap,screen-capture` 跳过此检查。等待上游 `zed-scap` 更新后可移除排除。

### 跨目标检查在 Windows 上不可行

`psm`/`stacker` 等 crate 依赖 C 编译器，Windows 上缺少对应平台（如 `x86_64-linux-gnu-gcc`、`cc` for macOS）的交叉编译工具链。真正的跨平台验证依赖 GitHub Actions 矩阵构建。

## 上游 PR 合并

需要合并上游 PR 时，先读取 `.opencode/merge-upstream-workflow.md` 并按说明执行。

- PR 状态追踪: `UPSTREAM-PRS.json`
- 上游仓库规则: `.opencode/upstream-rules.json`

### 上游独立 crate → rgpui 模块映射

上游 zed 将多个 crate 拆为独立工作区 crate（`collections`、`sum_tree` 等），而 rgpui 将它们合并为 `rgpui` crate 内部的模块。合并 PR 时，`scripts/merge-upstream-pr.ps1` 通过 `content_mappings` 自动替换 `use` 路径。以下是关键映射：

| 上游 `use <crate>::<item>` | rgpui 中的位置 | 说明 |
|---|---|---|
| `use collections::FxHashMap;` | `use crate::collections::FxHashMap;` | 根级 `pub mod collections;`，文件在 `rgpui/src/collections.rs` |
| `use sum_tree::SumTree;` | `use crate::sum_tree::SumTree;` | 根级 `pub mod sum_tree;`，文件在 `rgpui/src/sum_tree.rs` |
| `use scheduler::Scheduler;` | `use crate::scheduler::Scheduler;` | 根级 `pub mod scheduler;`，文件在 `rgpui/src/scheduler.rs` |
| `use refineable::Refineable;` | `use crate::refineable::Refineable;` | 根级 `pub mod refineable;`，目录在 `rgpui/src/refineable/` |
| `use http_client::HttpClient;` | `use crate::http_client::HttpClient;` | 根级 `pub mod http_client;`，目录在 `rgpui/src/http_client/` |
| `use gpui_util::ResultExt;` | `use rgpui::ResultExt;` | `gpui_util` → `crate::rgpui_util`（私有模块），关键项通过 `pub use rgpui_util::...` 重导出到 `rgpui::` |
| `use gpui_util::defer;` | `use rgpui::defer;` | 同上 |
| `use gpui_macros::*;` | `use rgpui_macros::*;` | 独立 proc-macro crate，路径不变 |

**关于 `gpui_util` 的重要说明**：内容映射 `"gpui_util": "rgpui_util"` 在 `rgpui` crate 内部代码中正确，但在**平台 crate**（`rgpui-windows`、`rgpui-linux`、`rgpui-macos`）中，`rgpui_util` 不是一个可访问的 crate 或路径。平台 crate 应使用 `use rgpui::ResultExt` 等通过 `rgpui` crate 重导出的名称。合并后需手动修正此导入。

**关于 `Cargo.toml`**：上游 `Cargo.toml` 中包含 `collections.workspace = true`、`sum_tree.workspace = true` 等依赖声明，这些在 rgpui 中不存在对应工作区 crate。合并 PR 时**跳过修改 Cargo.toml**，仅在代码中手动适配依赖项的引用方式。

## rgpui 独有的功能（合并上游 PR 时需保护）

rgpui 在上游 gpui 基础上增加了大量独有功能。合并上游 PR 后必须检查这些功能是否被破坏。

### 关键字段/类型（platform.rs）

| 项目 | 位置 | 说明 | 合并风险 |
|------|------|------|----------|
| `mouse_passthrough: bool` | `WindowOptions` + `WindowParams` | 鼠标事件穿透（桌面宠物覆盖层） | 上游 PR 可能移除/重命名此字段 |
| `WindowKind::Overlay` | 窗口类型枚举 | 覆盖层窗口（始终置顶、无边框） | 上游 PR 可能删除此变体 |
| `MicaBackdrop` / `MicaAltBackdrop` | `WindowBackgroundAppearance` | Windows 11 Mica 材质 | 上游无此变体 |
| `Tray` / `TrayMenuItem` / `TrayIconEvent` / `TrayIconData` | `tray.rs` 公开类型 | 托盘系统 | 上游无此功能 |
| `SystemPowerEvent` / `PowerSaveBlockerKind` / `OsInfo` / `PermissionType` / `NetworkStatus` / `MediaKeyEvent` / `BiometricStatus` / `AttentionType` / `FocusedWindowInfo` / `WindowPosition` | 平台相关类型 | 系统集成 API | 上游不存�?/在 |
| `single_instance` | `single_instance.rs` 模块 | 单实例进程管理 | 上游无此功能 |

### 自定义 PlatformWindow 方法

以下 `PlatformWindow` trait 方法是 rgpui 独有的，合并 PR 时不能删除：

```rust
fn hide(&self) {}                                              // 隐藏窗口（从任务栏移除）
fn set_mouse_passthrough(&self, _passthrough: bool) {}         // 鼠标穿透（Windows）
fn set_position(&mut self, _position: Point<Pixels>) {}        // 设置窗口位置
fn window_extended_style(&self) -> u32 { 0 }                   // Windows 扩展样式
fn set_window_extended_style(&self, _style: u32) {}            // 设置扩展样式
fn set_titlebar_visible(&self, _visible: bool) {}              // 显示/隐藏标题栏
fn set_input_region(&self, _region: Option<&[Bounds<Pixels>]>) {} // 输入区域（Wayland + Windows passthrough）
fn request_attention(&self) {}                                 // 请求用户注意
fn get_raw_handle(&self) -> HWND                               // 获取原始 HWND
```

### 自定义 Platform 方法

以下 `Platform` trait 方法是 rgpui 独有的（约 35+ 方法），主要涉及：

| 功能分类 | 关键方法 | 文件 |
|----------|----------|------|
| 托盘系统 | `set_tray`, `set_tray_icon`, `set_tray_menu`, `set_tray_tooltip`, `set_tray_panel_mode`, `get_tray_icon_bounds`, `on_tray_icon_event`, `on_tray_menu_action` | `app.rs`, `platform.rs` |
| 全局热键 | `register_global_hotkey`, `unregister_global_hotkey`, `on_global_hotkey` | `app.rs`, `platform.rs` |
| 通知 | `show_notification` | `app.rs` |
| 电源管理 | `on_system_power_event`, `start_power_save_blocker`, `stop_power_save_blocker`, `system_idle_time` | `platform.rs` |
| 辅助功能 | `accessibility_status`, `request_accessibility_permission`, `microphone_status`, `request_microphone_permission` | `platform.rs` |
| 网络 | `network_status`, `on_network_status_change` | `platform.rs` |
| 媒体键 | `on_media_key_event` | `platform.rs` |
| 系统信息 | `os_info` | `platform.rs` |
| 生物识别 | `biometric_status`, `authenticate_biometric` | `platform.rs` |
| Dock | `set_dock_badge`, `request_user_attention`, `cancel_user_attention` | `platform.rs` |
| 上下文菜单 | `show_context_menu` | `platform.rs` |
| 原生弹窗 | `show_dialog` | `platform.rs` |
| 无窗口保活 | `set_keep_alive_without_windows` | `app.rs` |

### rgpui-windows 特有功能

| 功能 | 文件 | 说明 |
|------|------|------|
| 托盘系统 | `tray.rs`, `platform.rs` | `Shell_NotifyIconW` + `WM_GPUI_TRAY_ICON` |
| NCHITTEST 鼠标穿透 | `events.rs:928-1027` | `HTTRANSPARENT` 逻辑，Ctrl 键覆盖 |
| Mica 材质 | `window.rs:981-989` | Windows 11 `DwmSetWindowCompositionAttribute` |
| 自动启动 | `auto_launch.rs` | 注册表自启动 |
| 焦点窗口查询 | `focused_window.rs` | `GetForegroundWindow` + `GetWindowText` |
| 自定义窗口消息 | `events.rs:32-49` | 9 个 `WM_USER` 自定义消息 |
| 扩展样式管理 | `window.rs:637-670, 1174-1278` | `sync_mouse_passthrough_style`, `set_titlebar_visible` |

### rgpui-3d MSAA 抗锯齿

`rgpui-3d/src/context.rs` 中的 MSAA 支持是 rgpui 独有的：

- `msaa_sample_count` 字段 + `resolve_texture`
- `set_msaa_sample_count(u32)`, `set_msaa_enabled(bool)`
- 离线渲染路径（无 swapchain，MSAA 通过 resolve texture 读回 CPU）

### 保护清单（合并 PR 后检查）

合并上游 PR 后，必须检查以下内容未被破坏：

1. `cargo check --workspace` 通过
2. `mouse_passthrough` 字段仍存在于 `WindowOptions` 和 `WindowParams`（`platform.rs`）
3. `WindowKind::Overlay` 变体存在
4. `PlatformWindow` trait 的所有自定义方法都在
5. `Platform` trait 的所有自定义方法都在
6. 托盘示例 `cargo run --example tray` 可编译
7. 桌面宠物示例 `cargo run -p desktop_pet` / `desktop_pet_3d` 可编译
8. `MicaBackdrop` / `MicaAltBackdrop` 枚举变体存在
9. 所有中文注释未被删除
10. `tray.rs` / `single_instance.rs` 模块未被删除
11. rgpui-3d MSAA 方法存在

## Web/WASM 开发

项目支持编译为 WebAssembly 在浏览器中运行。Web 平台实现位于 `rgpui-web/` crate。

### 前置条件

```bash
rustup toolchain install nightly
rustup target add wasm32-unknown-unknown --toolchain nightly
rustup component add rust-src --toolchain nightly
cargo install trunk
```

### 运行 Web 示例

```bash
# rgpui-component Web 示例（位于子工作区中）
cd crates/rgpui-component-workspce/rgpui-component/examples/hello_world_web && trunk serve
cd crates/rgpui-component-workspce/rgpui-component/examples/components_web && trunk serve

# rgpui-web 示例
cd crates/rgpui-web/examples/hello_web && trunk serve
cd crates/rgpui-web/examples/hello_world_web && trunk serve
cd crates/rgpui-web/examples/components_web && trunk serve
```

### Web 示例目录结构

每个 Web 示例是独立的子 crate，包含以下必要文件：

| 文件 | 说明 |
|------|------|
| `Cargo.toml` | 包配置，使用 `[workspace]` 避免继承问题 |
| `main.rs` | 应用代码，包含 `#[cfg(target_family = "wasm")]` 入口 |
| `index.html` | HTML 外壳，通过 `data-trunk` 链接 Rust 二进制 |
| `trunk.toml` | Trunk 配置，设置 COOP/COEP 头（SharedArrayBuffer 所需） |
| `.cargo/config.toml` | WASM 编译 flags（atomics、shared-memory 等） |
| `rust-toolchain.toml` | 指定 nightly 工具链和 wasm32 目标 |

### Web 入口函数模式

```rust
#![cfg_attr(target_family = "wasm", no_main)]

fn run_example() {
    rgpui_platform::application().run(|cx| {
        // 应用逻辑
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() { run_example(); }

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();  // 必须在应用逻辑前调用
    run_example();
}
```

### Web 平台限制

- 剪贴板 API 未实现（返回 None）
- 文件对话框不可用（浏览器安全限制）
- 系统托盘、原生菜单不支持
- Tree-sitter 语法高亮不可用（WASM 中无法编译 C 依赖）
- 图标从 CDN 运行时下载（需要网络连接）

详细文档见 `crates/rgpui-component-workspce/rgpui-component/examples/WEB.md`。

## 代码规范

- **所有函数必须添加中文注释**：公开 API 和内部函数均需使用简体中文说明功能、参数和返回值
- 注释风格遵循 Rust 文档规范（`///` 用于公开 API，`//` 用于内部逻辑）
- 避免使用英文注释，保持项目语言统一
