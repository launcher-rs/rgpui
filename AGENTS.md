# AGENTS.md - rgpui 开发指南

## 项目概述

rgpui 是一个**独立演进**的 GPU 加速跨平台 UI 框架。它历史上合并了 Zed 的 `gpui`（渲染/布局内核）与 `gpui-component`（组件库），但**现已与上游正式切分**：上游（Zed）是不同团队维护且处于重大重构中，我们不再跟随其步伐，所有 crate 均属本项目自有，按自身目标独立演进。

采用 Rust workspace 架构，核心库 `rgpui` 通过 `Platform` trait 抽象各平台实现。

## AGENTS.md — 项目约定（本机 Windows + PowerShell 7）

本机（Windows + PowerShell 7）下的 opencode / agent 约定，避免中文输出乱码与命令副作用。

### 中文乱码约定

本机 pwsh 的 `[Console]::OutputEncoding` 默认是系统代码页（936 / GB2312），而
opencode 等工具读取 pwsh 输出时按 UTF-8 解码。因此任何**含中文的输出**命令
（`git log`、`git diff`、`rg`、`cargo` 警告、`Get-Content`、`Get-ChildItem`、
`Write-Output` 等）都可能出现乱码，影响对结果的判断。

#### 执行含中文输出的命令前，先设置 UTF-8（四行都设）

```powershell
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::InputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$PSStyle.OutputRendering = 'PlainText'
```

- `[Console]::OutputEncoding` / `InputEncoding`：控制台层编码。
- `$OutputEncoding`：管道 / 外部命令的输出编码。
- `$PSStyle.OutputRendering = 'PlainText'`：pwsh 7 默认会往管道输出 ANSI 样式
  （彩色），在重定向 / 捕获时干扰编码与解析，先关掉再执行。

#### 读取中文文件建议加 -Encoding UTF8

```powershell
Get-Content -Path <file> -Encoding UTF8
```

#### 可选项：写入 pwsh profile 让所有会话自动生效

把上述四行写入 `$PROFILE`（`C:\Users\deego\Documents\PowerShell\Microsoft.PowerShell_profile.ps1`），
之后每个 pwsh 会话都默认 UTF-8，不再需要逐个命令前手动设置。

#### 规避建议

- 识别文本结果（文件名、crate 名、报错位置）尽量用专用工具（Read / Glob / Grep），
  它们按 UTF-8 读取，不受控制台代码页影响。
- 需要程序化解析命令输出时，优先让命令输出英文（如 `git -c core.quotepath=false diff`），
  或用 `--no-ansi` 去掉样式。

## 架构与包边界

```
rgpui（核心：框架 + 基础组件 + 扩展组件/动画/手势/滚动物理，charts/effects/qr-code/tokio feature 门控）
  ├── rgpui-markdown（独立专业库，已落地；rgpui-editor 预留，另行论证）
  └── rgpui-term / rgpui-3d（专业集成库）

crates/
├── rgpui/                   # 核心 UI 框架，平台无关逻辑
├── rgpui-3d/                # 3D 渲染支持
├── rgpui-character/         # 字符/文本处理
├── rgpui-linux/             # Linux 平台实现
├── rgpui-macos/             # macOS 平台实现
├── rgpui-markdown/          # Markdown 渲染独立库（pulldown-cmark 0.13）
├── rgpui-macros/            # 过程宏
├── rgpui-platform/          # 平台选择入口，根据 cfg 选择具体平台 crate
├── rgpui-term/              # 终端组件
├── rgpui-web/               # Web/WASM 平台实现
├── rgpui-wgpu/              # wgpu 渲染后端
└── rgpui-windows/           # Windows 平台实现（windows-rs 绑定）

examples/                    # 独立 workspace（56 个示例 crate）
├── Cargo.toml               # workspace 配置，path deps 指向 crates/
├── hello_world/             # 简单示例
├── tray/                    # 托盘示例
├── desktop_pet/             # 桌面宠物示例
├── view_example/            # 多文件示例
└── ...                      # 其余 52 个示例
```

- Tokio 异步运行时集成已并入 `rgpui` 核心（feature `tokio` 门控，`rgpui::tokio` 模块），不再作为独立 crate 存在

- `rgpui/src/platform.rs` 定义了 `Platform` trait 和 `PlatformWindow` trait，所有平台必须实现
- 示例代码使用 `rgpui_platform::application()` 获取平台应用入口
- 平台特有代码放在对应 `rgpui-<platform>/` crate 中

## 开发命令

```bash
# 检查整个 workspace（推荐日常使用，比 build 快）
cargo check --workspace

# 检查 examples workspace
cargo check --workspace
# 在 examples/ 目录下执行，或指定 manifest
cargo check --workspace --manifest-path examples/Cargo.toml

# 构建单个示例（examples workspace）
cargo build -p tray --manifest-path examples/Cargo.toml

# 运行示例（examples workspace）
cargo run -p tray --manifest-path examples/Cargo.toml

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
- **推送前必须检查**：执行 `cargo check --workspace` 和 `cargo check --workspace --manifest-path examples/Cargo.toml`，确保没有任何错误和警告
- **推送前格式化代码**: 执行 `cargo fmt` 格式化代码（注意 `rgpui-linux/src/linux/platform.rs` 有 Rust 2024 edition 解析问题，`cargo fmt` 会报错，需跳过该文件）
- **禁止使用 `#[allow(dead_code)]`**：未使用的代码应当删除或重构，不得使用属性压制警告

## PR 工作流

### 分支策略

- **main 分支**：保护分支，只接受 PR 合并，**禁止直接推送**（包括 opencode agent）
- **功能分支**：从 main 创建，命名规范 `feat/xxx`、`fix/xxx`、`refactor/xxx`、`chore/xxx`
- **PR 合并方式**：Squash and merge（保持 main 历史整洁）

### ⚠️ Agent 行为约束

**opencode agent 必须严格遵守以下规则：**

1. **任何改动都必须通过 PR 流程**，即使是小修改、依赖升级、文档更新
2. **禁止直接 `git push` 到 main**，必须先创建功能分支，再通过 PR 合并
3. **PR 创建后等待 CI 全绿再合并**，不得跳过 CI 检查
4. **单次 PR 只做一件事**：依赖升级、功能开发、bug 修复应分开提交

```
正确流程：
1. git checkout -b chore/upgrade-deps
2. 开发、提交
3. git push -u origin chore/upgrade-deps
4. gh pr create --title "chore(deps): upgrade dependencies" --body "..."
5. 等待 CI 全绿
6. gh pr merge --squash

错误流程：
git commit -m "upgrade deps" && git push  ← 禁止！
```

### PR 流程

1. 从 main 创建功能分支
2. 在功能分支上开发、提交
3. 推送功能分支并创建 PR
4. CI 自动运行（fmt、clippy、check、test，三平台矩阵）
5. CI 全部通过后，Squash 合并到 main
6. 合并后删除功能分支

### PR 要求

- **标题格式**：`feat: xxx`、`fix: xxx`、`refactor: xxx`、`chore: xxx`（Conventional Commits）
- **CI 必须全绿**：fmt、clippy、check（主 workspace + examples workspace）、test 均通过
- **代码审查**：至少一人批准（个人项目可自批）
- **关联 Issue**：如有相关 Issue，在 PR 描述中引用
- **PR 描述**：说明改动内容、原因、影响范围

### PR 模板

项目已配置 PR 模板（`.github/pull_request_template.md`），创建 PR 时自动填充。

### 违规处理

如果已经直接推送到 main（如依赖升级 `cfc84e1`），采取以下措施：
1. **不回滚**：直接推送的代码如果编译通过且测试通过，保留不回滚
2. **后续改动走 PR**：从下一次提交开始严格执行 PR 流程
3. **记录在案**：在 PR 描述中说明历史违规情况，保持透明

## 跨平台检查

### 问题背景

Windows 下 `cargo check` 只编译 `#[cfg(target_os = "windows")]` 和通用代码，`#[cfg(target_os = "macos")]` 和 `#[cfg(target_os = "linux")]` 中的代码不会被编译，跨平台代码改动容易把 Linux/macOS 代码弄坏。

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
| `check (examples)` | windows-latest / ubuntu-latest / macos-latest | `cargo check --workspace`（examples workspace） |
| `feature-check` | ubuntu-latest | `cargo hack check --each-feature`（排除已知故障的 `scap`/`screen-capture` feature） |
| `lint` | ubuntu-latest | `cargo fmt --all --check` |

CI 通过矩阵策略在三个平台分别运行，确保跨平台兼容性。

## 已知问题

### `screen-capture` / `scap` feature 编译失败

`cargo hack check --each-feature` 中 `rgpui` 的 `screen-capture` feature 因其依赖的 `zed-scap` 与 `windows-capture` 1.5.0 API 不兼容（函数参数数量变化），暂时无法通过编译。CI 中已通过 `--exclude-features scap,screen-capture` 跳过此检查。等待上游 `zed-scap` 更新后可移除排除。

### 跨目标检查在 Windows 上不可行

`psm`/`stacker` 等 crate 依赖 C 编译器，Windows 上缺少对应平台（如 `x86_64-linux-gnu-gcc`、`cc` for macOS）的交叉编译工具链。真正的跨平台验证依赖 GitHub Actions 矩阵构建。

## rgpui 独有的功能

rgpui 在上游 gpui 基础上增加了大量独有功能，是项目的差异化价值所在，任何重构不得移除这些功能。

### 组件库（rgpui 核心自带）

rgpui 核心自带完整基础组件库。已并入的子系统：

| 子系统 | 目标模块 | 关键公开类型 |
|--------|----------|--------------|
| 滚动 | `elements/scroll/` | `Scrollable`、`Scrollbar`、`ScrollHandle` |
| 基础元素 | `elements/` | `Button`、`Checkbox`、`Radio`、`Switch`、`Slider`、`Spinner`、`Skeleton`、`Badge`、`Tag`、`Separator`、`Kbd`、`Tooltip`、`Icon` |
| 表单 | `form/` | `Form`、`Field`、`FieldBuilder`、`v_form`/`h_form`/`field` |
| 输入 | `input_ui/` | `Input`、`MaskedInput`、`NumberInput`、`PasswordInput`、`TextArea` |
| 菜单 | `menu/` | `PopupMenu`、`ContextMenu`、`DropdownMenu`、`Menu`、`MenuBar`、`MenuItem`、`HoverCard`、`Notification`、`Toast` |
| 对话框 | `dialog/` | `Dialog`、`AlertDialog`、`DialogHeader/Content/Footer/Title/Description`、`FocusTrapElement` |
| 列表 | `list/` | `List`、`VirtualList`、`ListDelegate`、`ListState` |
| 表格 | `table/` | `Table`、`DataTable`、`Column`、`TableState`、`TableHeader/Body/Footer/Row/Head/Cell/Caption` |
| 标签页 | `tabs/` | `Tab`、`TabBar`、`TabVariant`、`Accordion`、`AccordionItem`、`Collapsible` |
| 标题栏 | `title_bar/` | `TitleBar`、`WindowBorder`、`window_paddings()` |
| 扩展组件 | `components/`（原 rgpui-ui 并入） | 动画 13 组件、`TagInput`/`OtpInput`/`HotkeyInput`/`InlineEdit`、`SplitPane`/`Resizable`/`DragDrop`/`SortableList`、`Spotlight`/`CommandPalette`/`AppMenuBar`、`ImageViewer`/`Sparkline`/`SvgRenderer`、`Waveform` 等；`charts`/`effects`/`qr-code` feature 门控 |
| 动画/手势/物理 | `animation/`、`mouse_gestures.rs`、`scroll_physics.rs` | `Spring`、`AnimationPreset`、`GestureDetector`、`ScrollPhysics` |

配套扩展 trait 已并入 `prelude`：`ActiveTheme`、`ElementExt`、`InteractiveElementExt`（`on_double_click`）、`Selectable`、`Sizable`、`StyledExt`、`FluentBuilder`。

### 关键字段/类型（platform.rs）

| 项目 | 位置 | 说明 |
|------|------|------|
| `mouse_passthrough: bool` | `WindowOptions` + `WindowParams` | 鼠标事件穿透（桌面宠物覆盖层） |
| `WindowKind::Overlay` | 窗口类型枚举 | 覆盖层窗口（始终置顶、无边框） |
| `MicaBackdrop` / `MicaAltBackdrop` | `WindowBackgroundAppearance` | Windows 11 Mica 材质 |
| `Tray` / `TrayMenuItem` / `TrayIconEvent` / `TrayIconData` | `tray.rs` 公开类型 | 托盘系统 |
| `SystemPowerEvent` / `PowerSaveBlockerKind` / `OsInfo` / `PermissionType` / `NetworkStatus` / `MediaKeyEvent` / `BiometricStatus` / `AttentionType` / `FocusedWindowInfo` / `WindowPosition` | 平台相关类型 | 系统集成 API |
| `single_instance` | `single_instance.rs` 模块 | 单实例进程管理 |

### 自定义 PlatformWindow 方法

以下 `PlatformWindow` trait 方法是 rgpui 自有 API：

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

### 完整性检查清单

任何重构/提交前，应检查以下内容保持完整：

1. `cargo check --workspace` 通过
2. `cargo check --workspace --manifest-path examples/Cargo.toml` 通过
3. `mouse_passthrough` 字段存在于 `WindowOptions` 和 `WindowParams`（`platform.rs`）
4. `WindowKind::Overlay` 变体存在
5. `PlatformWindow` trait 的所有自定义方法都在
6. `Platform` trait 的所有自定义方法都在
7. 托盘示例 `cargo run -p tray --manifest-path examples/Cargo.toml` 可编译
8. 桌面宠物示例 `cargo run -p desktop_pet --manifest-path examples/Cargo.toml` / `desktop_pet_3d` 可编译
9. `MicaBackdrop` / `MicaAltBackdrop` 枚举变体存在
10. 所有中文注释未被删除
11. `tray.rs` / `single_instance.rs` 模块未被删除
12. rgpui-3d MSAA 方法存在
13. 组件库核心模块存在：`form/`、`input_ui/`、`menu/`、`dialog/`、`list/`、`table/`、`tabs/`、`title_bar/`、`elements/scroll/`
14. `prelude` 暴露组件扩展 trait：`ActiveTheme`、`ElementExt`、`InteractiveElementExt`、`Selectable`、`Sizable`、`StyledExt`、`FluentBuilder`
15. `crates/rgpui-adabraka-ui`、`crates/rgpui-yororen-ui`、`crates/rgpui-component-workspce` 已删除，workspace 不再依赖旧 UI 库
16. `crates/rgpui-markdown` 存在，`pulldown-cmark = "0.13"` 为工作区依赖
17. 核心 `rgpui` 存在 `charts` / `effects` / `qr-code` 三个 feature（`components/charts/` 及特效、二维码组件门控），`cargo check -p rgpui --features charts,effects,qr-code` 通过
18. `rgpui-tokio` 已并入核心（feature `tokio` 门控，`rgpui::tokio` 模块），`crates/rgpui-tokio` 已删除，workspace 不再依赖该 crate；`cargo check -p rgpui --features tokio` 通过
19. `rgpui-editor` 保持预留不构建，论证留档于 `docs/ui-crate-plan.md` §6.5（核心 `input_ui` 已含输入编辑器，仅缺 tree-sitter 高亮/折叠/LSP）
20. `rgpui-ui` 迁移已全部完成并**已并入核心**（2026-08-19，执行记录见 `docs/ui-crate-plan.md` §9）：动画 13 组件、特效（aurora/confetti/particle_emitter，门控于 `effects`）、显示（qr_code/sparkline/svg_renderer/image_viewer，qr_code 门控于 `qr-code`，code_block/rich_text 放弃）、高级输入（tag_input/otp_input/hotkey_input/inline_edit）、布局（split_pane/resizable/drag_drop/sortable_list 等）、通知/命令（spotlight/app_menu/command_palette 等）、工具（mouse_gestures/scroll_physics）均已迁入；`animated_progress` 已迁；yororen keybinding 不迁移、carousel/tilt_card/magnetic_button 放弃（理由见 §9）；`crates/rgpui-ui` 已删除，workspace 不再依赖该 crate
21. Web DOM 后端存在：核心 `rgpui` 有 `dom-backend` feature（`src/dom.rs`，含 `DomTree`/`DomNodeKey`/`DomTreeBuilder`/`set_dom_layer_enabled`）、`crates/rgpui-dom` 存在（`reconcile`/`DomPatch`/`DomBackend`/`WebDomBackend`/`to_html`）、`rgpui-web` 已接入（`supports_dom`/`dom_tree_update`）；`cargo check -p rgpui --features dom-backend` 与 `cargo test -p rgpui-dom` 通过；用法见 `docs/web-dom-backend-usage.md`
22. `crates/rgpui/examples/` 已删除，所有示例迁移至 `examples/` 独立 workspace（56 个示例 crate）
23. `rgpui` 的 `[[example]]` 条目已全部删除，`rgpui-platform` dev-dependency 已移除（消除发布循环依赖）
24. `cargo publish -p rgpui --dry-run` 通过（不再需要 `--no-verify`）
25. v1.1.0-alpha 功能存在：`push_notification`（App 方法）、`init_all`（`init.rs` 模块）、`minimize_to_tray`（App 方法）、`show_all_windows` / `hide_all_windows` / `minimize_all_windows`（App 方法）、`SearchPanel`（`components/search_panel.rs`）
26. WebView 组件已并入核心 `rgpui`（feature `webview` 门控，`rgpui::webview` 模块）：`WebView` + `WebViewHandle` + `load_url`/`load_html`/`forward`/`reload`/`eval_script`；`crates/rgpui-webview` 已删除，workspace 不再依赖该 crate；`cargo check -p rgpui --features webview` 通过
27. v1.1.0-beta 功能存在：`lsp` feature 门控（`LspClient` trait + 补全/悬停/定义/诊断/语义高亮子系统）、`highlight` 模块（`Highlighter` trait）、`StatusBar` 组件（`components/status_bar.rs`）
28. v1.1.0-rc 功能存在：`file_watcher` 模块（`FileWatcher`/`FileEvent`）、`config_store` 模块（`ConfigStore`）、`chat_ui` 模块（`ChatView`/`Message`/`MessageGroup`）、`fps_hud` 模块（`FpsHud`/`FpsHudState`）
29. v1.1.0 功能存在：`tabs/tab_drag.rs`（`TabDragDrop`/`TabDragState`）、`i18n` 模块（`I18nManager`/`I18nText`）、`theme_watcher` 模块（`ThemeWatcher`/`ThemeManager`）、`block_render` 模块（`BlockRenderer`/`BlockType`）、`virtual_scroll` 模块（`VirtualScroll`/`VirtualScrollList`）、`source_map` 模块（`SourceMap`/`SourceLocation`）、`markdown_ext` 模块（`MarkdownPlugin`/`PluginManager`）

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
# rgpui-web 示例
cd crates/rgpui-web/examples/hello_web && trunk serve
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

### Web DOM 后端（文本选择/复制）

默认 Web 平台走纯 canvas 渲染（文本不可选中）。需要浏览器原生文本能力时，启用
`rgpui` 的 `dom-backend` feature 并在打开窗口前调用 `rgpui::set_dom_layer_enabled(true)`，
即可叠加一层绝对定位的 DOM 覆盖层（`rgpui-dom` 增量对账，div/文本已 DOM 化）。
运行时开关默认关闭；用法与定制方式见 `docs/web-dom-backend-usage.md`。
`hello_web` 与 `rgpui_story` 两个示例已在 wasm 下开启 DOM 层。

## 代码规范

- **所有函数必须添加中文注释**：公开 API 和内部函数均需使用简体中文说明功能、参数和返回值
- 注释风格遵循 Rust 文档规范（`///` 用于公开 API，`//` 用于内部逻辑）
- 避免使用英文注释，保持项目语言统一
