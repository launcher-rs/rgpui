# AGENTS.md - rgpui 开发指南

## 项目概述

rgpui 是一个**独立演进**的 GPU 加速跨平台 UI 框架（历史合并 Zed 的 `gpui` +
`gpui-component`，现已与上游切分，不再跟随）。Rust workspace 架构，核心库
`rgpui` 通过 `Platform` trait 抽象各平台实现。

## 本机约定（Windows + PowerShell 7）

opencode 等工具按 UTF-8 解码 pwsh 输出，而本机控制台默认 GB2312 —— 任何含中文
输出的命令都先执行这四行：

```powershell
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::InputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$PSStyle.OutputRendering = 'PlainText'
```

- 文本结果优先用专用工具（Read / Glob / Grep，不受代码页影响）。
- 程序化解析命令输出时尽量用英文输出或 `--no-ansi`。
- 读中文文件加 `-Encoding UTF8`。

## 架构与包边界

```
rgpui（核心：框架 + 基础组件 + 扩展组件/动画/手势/滚动物理，charts/effects/qr-code/tokio/webview feature 门控）
  ├── rgpui-markdown（独立 Markdown 库）└── rgpui-term / rgpui-3d（专业集成库）

crates/                 # 12 个库 crate（rgpui / rgpui-{3d,character,linux,macos,macros,markdown,platform,term,web,wgpu,windows}）
examples/               # 45 个示例 crate（同根 workspace；showcase 用多 binary，如 image_showcase --bin image）
```

- `rgpui/src/platform.rs` 定义 `Platform` / `PlatformWindow` trait；示例经
  `rgpui_platform::application()` 启动；平台特有代码在对应 `rgpui-<platform>/`。
- Tokio 已并入核心（`rgpui::tokio`，feature 门控），WebView 同理（`rgpui::webview`）。

## 开发命令

```bash
cargo check --workspace            # 日常检查（比 build 快）
cargo run -p <pkg>                 # 运行示例；showcase 加 --bin，如 -p image_showcase --bin image
cargo test --workspace             # 所有测试
cargo test -p rgpui                # 单包测试
cargo clippy -p <pkg> --all-targets -- -D warnings   # 提交前对改动包执行
cargo fmt --all                    # 提交前格式化
```

## Clippy 规则

workspace 级 `deny`：`dbg_macro`、`todo`、`declare_interior_mutable_const`、
`redundant_clone`、`disallowed_methods`；`style` 为 `allow`。

## 托盘与窗口隐藏

- 类型：`TrayMenuItem` / `TrayIconEvent`（`rgpui::tray`）；App 方法：
  `set_tray_icon/menu/tooltip`、`on_tray_menu_action`、`on_tray_icon_event`。
- Windows 实现：`rgpui-windows/src/tray.rs`（`Shell_NotifyIconW`）+
  `platform.rs`（`WM_GPUI_TRAY_ICON` / `WM_COMMAND`）。
- `hide_window()`（任务栏移除）vs `minimize_window()`（保留任务栏图标）；
  托盘恢复用 `activate_window()`。图标接受 PNG/ICO 字节。

## 提交与 PR 规范

- 提交前：`cargo check --workspace` + `cargo fmt --all` + 相关包 clippy，无错误警告。
- **禁止 `#[allow(dead_code)]`**，无用代码删除或重构。
- 开发阶段废弃代码直接删除，不留 `#[deprecated]` 兼容（不为未发布的 API 做兼容）。
- main 受保护，**合入 main 的改动走 PR**（分支命名 `feat|fix|refactor|chore/xxx`，
  Squash 合并，标题 Conventional Commits）。
  版本开发分支（如 `feat/1.2.0`）上直接提交推送，**不为合入版本分支开中间 PR**；
  版本分支完成后再开一个 PR 合入 main。
- `gh pr create / merge --squash`；CI 全绿再合并；等 CI 时不要反复 push 刷运行。
- CI（`.github/workflows/ci.yml`）：三平台矩阵，跑 fmt（逐库）+ clippy
 （`--workspace --lib --bins -D warnings`）+ check + test；同分支 push/PR 共用
  concurrency 组，后触发自动取消先触发的。

## 跨平台注意

- Windows 的 `cargo check` 不编译 macOS/Linux `cfg` 代码，跨平台改动靠 CI 矩阵验证。
- `cargo hack check --each-feature` 可用；`scap` / `screen-capture` feature
  已知编译失败（`zed-scap` 与 `windows-capture` API 不兼容），不要启用。

## rgpui 独有功能（重构不得移除）

### 组件库索引

| 子系统 | 模块 | 关键类型 |
|--------|------|----------|
| 滚动 | `elements/scroll/` | `Scrollable`、`Scrollbar`、`ScrollHandle` |
| 基础元素 | `elements/` | `Button`、`Checkbox`、`Radio`、`Switch`、`Slider`、`Spinner`、`Skeleton`、`Badge`、`Tag`、`Separator`、`Kbd`、`Tooltip`、`Icon`、`Avatar`、`Alert`、`Breadcrumb`、`Card`、`Typography`、`Toggle` |
| 表单 | `form/` | `Form`、`Field`、`FieldBuilder` |
| 输入 | `input_ui/` | `Input`、`MaskedInput`、`NumberInput`、`PasswordInput`、`TextArea`；`InputState::reveal_offset/reveal_range`、`TextDecorationCollection`、`Debouncer`（见 `docs/1.2.0/1.2.0-dev-plan.md` §E） |
| 菜单 | `menu/` | `PopupMenu`、`ContextMenu`、`DropdownMenu`、`Popconfirm`、`MenuBar`、`HoverCard`、`Notification`、`Toast` |
| 对话框 | `dialog/` | `Dialog`、`AlertDialog`、`DialogHeader/Content/Footer`、`FocusTrapElement` |
| 列表/表格/标签页 | `list/`、`table/`、`tabs/` | `List`、`VirtualList`、`DataTable`、`TabBar`、`Accordion` |
| 标题栏/扩展 | `title_bar/`、`components/` | `TitleBar`、动画 13 组件、`SplitPane`、`CommandPalette`、`ImageViewer`、`SearchPanelState`、`Sidebar`/`SidebarSection`、`Select`、`Combobox`、`DatePicker`、`ColorPicker`、`Pagination`、`Steps`、`Timeline`、`Rate`、`Upload`、`Carousel`、`MermaidDiagram`、`DockArea` 等（`charts`/`effects`/`qr-code` feature 门控） |
| 动画/手势/物理 | `animation/`、`mouse_gestures.rs`、`scroll_physics.rs` | `Spring`、`GestureDetector`、`ScrollPhysics` |
| 对话（AI-chat） | `chat/` | `Message`/`MessageGroup`/`ChatView`、`Prompts`、`Suggestion`、`ThoughtChain`、`Attachments`/`FileCard`、`Sources`、`Actions`、`Sender`、`Bubble`、`MessageScroller`、`Marker` |

`prelude` 含：`ActiveTheme`、`ElementExt`、`InteractiveElementExt`、`Selectable`、
`Sizable`、`StyledExt`、`FluentBuilder`。

### 平台 trait 自有 API（platform.rs）

`PlatformWindow`：`hide`、`set_mouse_passthrough`、`set_position`、
`window_extended_style` / `set_window_extended_style`、`set_titlebar_visible`、
`set_input_region`、`request_attention`、`get_raw_handle`。
`Platform` 约 35+ 自有方法：托盘 8 件套、全局热键、通知、电源、辅助功能/网络/
媒体键/系统信息/生物识别/Dock/上下文菜单/原生弹窗/无窗口保活，另
`WindowOptions.mouse_passthrough`、`WindowKind::Overlay`、Mica 材质、
`tray.rs` / `single_instance.rs`。

### rgpui-windows 特有

托盘（`tray.rs`）、NCHITTEST 鼠标穿透（`events.rs`）、Mica（`window.rs`）、
自启动（`auto_launch.rs`）、焦点窗口查询、9 个 `WM_USER` 自定义消息。
DComp 以 `CreateTargetForHwnd(hwnd, false)` 创建，子 HWND（如 WebView2）显示在
DComp 内容之上——改回 `true` 会遮挡 WebView。

### 完整性检查清单（重构/提交前）

1. `cargo check --workspace` 通过
2. `mouse_passthrough`（`WindowOptions`/`WindowParams`）、`WindowKind::Overlay`、
   Mica 变体、`tray.rs` / `single_instance.rs` 都在
3. `Platform` / `PlatformWindow` 自有方法都在（对照上表）
4. `tray`、`desktop_pet`（`_3d`）示例可编译
5. 中文注释未被删除；无 `#[allow(dead_code)]`
6. 组件子系统目录都在（`form input_ui menu dialog list table tabs title_bar elements/scroll`）
7. `crates/rgpui-ui`、`rgpui-tokio` 等旧 crate 未复活；`rgpui-markdown` 在，
   `pulldown-cmark = "0.13"`
8. feature 组合按需通过：`charts,effects,qr-code` / `tokio` / `webview` /
   `dom-backend`（+ `cargo test -p rgpui-dom`）
9. `cargo publish -p rgpui --dry-run --registry crates-io` 通过
10. 发布要求无 dev 循环依赖（测试放消费方 `tests/`，见 1.1.1 教训）

## Web/WASM 开发

前置：nightly 工具链 + `wasm32-unknown-unknown` + `rust-src` + `trunk`。
Web 示例是独立子 crate（含 `index.html`、`trunk.toml`、`.cargo/config.toml`、
`rust-toolchain.toml`），入口模式：`#![cfg_attr(target_family = "wasm", no_main)]` +
`#[wasm_bindgen(start)]` 中先调 `rgpui_platform::web_init()`。

限制：剪贴板 None、文件对话框/托盘/原生菜单不可用、tree-sitter 不可用、图标 CDN 下载。
原生文本能力：`dom-backend` feature + `set_dom_layer_enabled(true)`（见
`docs/web-dom-backend-usage.md`，`hello_web` 与 `rgpui_story` 已开启）。

## 代码规范

- 所有函数中文注释（`///` 公开 API，`//` 内部逻辑），不写英文注释。
