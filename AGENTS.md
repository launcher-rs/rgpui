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
- **推送前必须检查**：执行 `cargo check --workspace --examples` 确保没有任何错误和警告
- - **推送前格式化代码**: 执行 `cargo fmt` 格式化代码
- **禁止使用 `#[allow(dead_code)]`**：未使用的代码应当删除或重构，不得使用属性压制警告

## Zed PR 合并规范

### 自动合并（推荐）

使用 `scripts/merge-upstream-pr.ps1` 脚本自动完成上游 PR 的获取、映射和应用。

```powershell
# 合并指定 PR
.\scripts\merge-upstream-pr.ps1 -PR 58291

# 合并所有待处理 PR（UPSTREAM-PRS.json 中 status = "pending" 的）
.\scripts\merge-upstream-pr.ps1 -All

# 仅分析，不写入文件
.\scripts\merge-upstream-pr.ps1 -PR 58291 -DryRun
```

脚本自动执行：
1. 克隆/拉取 https://github.com/zed-industries/zed.git 到 `temp/zed-upstream/`
2. 获取 PR 的变更文件列表
3. 按映射规则替换路径和内容中的 `gpui_*` → `rgpui_*`
4. 写入本地仓库
5. 运行 `cargo check -p rgpui` 验证
6. 更新 `UPSTREAM-PRS.json` 中的 PR 状态

### 包名映射规则

Zed 上游仓库的 PR 合并到 rgpui 时，所有包名/路径需要添加 `r` 前缀映射：

| Zed 原 crate | rgpui 对应 crate |
|--------------|------------------|
| `gpui` | `rgpui` |
| `gpui_platform` | `rgpui_platform` |
| `gpui_windows` | `rgpui_windows` |
| `gpui_macos` | `rgpui_macos` |
| `gpui_linux` | `rgpui_linux` |
| `gpui_web` | `rgpui_web` |
| `gpui_wgpu` | `rgpui_wgpu` |
| `gpui_macros` | `rgpui_macros` |
| `gpui_tokio` | `rgpui_tokio` |

### PR 追踪配置

`UPSTREAM-PRS.json` 管理所有上游 PR 的状态和映射规则：

```json
{
  "upstream": {
    "url": "https://github.com/zed-industries/zed.git",
    "branch": "main"
  },
  "prs": [
    {
      "number": 57835,
      "title": "Log worst hanging tasks and actions",
      "status": "merged",
      "merged_at": "2025-06-04"
    }
  ]
}
```

状态值：`pending`、`merged`、`check-failed`、`analyzed`

### 手动合并步骤

当自动脚本不适用时（如需要手动调整）：

1. 将上游 PR 中所有 `gpui` 开头的 crate 引用替换为 `rgpui` 开头
2. 检查 `Cargo.toml` 中的依赖声明是否指向正确的 rgpui crate 路径
3. 检查 `use` 语句和路径引用是否已更新
4. 运行 `cargo check --workspace` 验证无编译错误

## 代码规范

- **所有函数必须添加中文注释**：公开 API 和内部函数均需使用简体中文说明功能、参数和返回值
- 注释风格遵循 Rust 文档规范（`///` 用于公开 API，`//` 用于内部逻辑）
- 避免使用英文注释，保持项目语言统一