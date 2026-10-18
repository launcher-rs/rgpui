# Windows 平台实现（rgpui-windows）

**目录：** [概述](#概述) · [托盘](#托盘) · [鼠标穿透](#鼠标穿透) · [Mica 背景](#mica-背景) · [自启动](#自启动) · [焦点窗口](#焦点窗口) · [自定义消息](#自定义消息) · [DComp 与子窗口](#dcomp-与子窗口) · [最佳实践](#最佳实践)

## 概述

`crates/rgpui-windows/` 是 rgpui 的 Windows 后端，实现 `platform.rs` 中的
`Platform`/`PlatformWindow`。入口 `rgpui_windows.rs`，主要模块：
`platform.rs`（窗口/消息循环）、`events.rs`（窗口过程与 NCHITTEST）、`window.rs`（DComp/材质）、
`tray.rs`、`auto_launch.rs`、`focused_window.rs`、`global_hotkey.rs`、`notifications.rs`、
`clipboard.rs`、`directx_renderer.rs`、`dispatcher.rs`、`vsync.rs`、`direct_write.rs`、
`system_settings.rs`。

> 应用侧**不直接依赖**本 crate，全部经 `rgpui` 的 `App`/`Window`/`Platform` 接口。

## 托盘

`tray.rs`：`Icon` 结构（`:41`，封装 `NOTIFYICONDATAW` + `Shell_NotifyIconW`）：
`new(HWND)`（`:44`）、`set_icon(Option<&[u8]>, HWND)`（`:107`）、
`set_tooltip(&str, HWND)`（`:138`）、`show_context_menu(HWND)`（`:167`）。
回调消息 `WM_GPUI_TRAY_ICON`（`events.rs:49`），菜单命令走 `WM_COMMAND`。
应用层 API 见 `platform.md` 的「托盘」节。

## 鼠标穿透

`events.rs` 处理 `WM_NCHITTEST`（`:96`）→ `handle_hit_test_msg`（`:939`）：
`hit_test_window_control` 回调（`:944`），命中判定结果控制是否穿透。
应用层：`WindowOptions.mouse_passthrough` / `Window::set_mouse_passthrough(bool)`
（见 `platform.md`）。

## Mica 背景

`window.rs`（`:979`）用 DWM API 实现 Mica /
MicaAlt（`WindowBackgroundAppearance::MicaBackdrop`/`MicaAltBackdrop`，内核枚举
`platform.rs:2382`，`:2397`/`:2399`）：`DWMSBT_MAINWINDOW` → MicaBase、
`DWMSBT_TABBEDWINDOW` → MicaAlt。内核枚举含 `Opaque`/`Transparent`/`Blurred`。

## 自启动

`auto_launch.rs`：`set_auto_launch(app_id, enabled) -> Result<()>`（`:18`）、
`is_auto_launch_enabled(app_id) -> bool`（`:38`）。应用层 `App::set_auto_launch`/
`is_auto_launch_enabled`（`app.rs:2679`/`:2684`）。

## 焦点窗口

`focused_window.rs`（`:17` 起）用 `GetForegroundWindow` 查询，返回
`FocusedWindowInfo`（`platform.rs:219`）。应用层 `App::focused_window_info()`
（`app.rs:2689`）。

## 自定义消息

`events.rs` 定义 9 个 `WM_USER + N` 消息（全部 `pub(crate)`）：

| 常量 | 值 | 用途 |
|------|----|------|
| `WM_GPUI_CURSOR_STYLE_CHANGED` | +1 | 光标样式变更 |
| `WM_GPUI_CLOSE_ONE_WINDOW` | +2 | 关闭单个窗口 |
| `WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD` | +3 | 主线程任务派发 |
| `WM_GPUI_DOCK_MENU_ACTION` | +4 | Dock 菜单动作 |
| `WM_GPUI_FORCE_UPDATE_WINDOW` | +5 | 强制重绘 |
| `WM_GPUI_KEYBOARD_LAYOUT_CHANGED` | +6 | 键盘布局变更 |
| `WM_GPUI_GPU_DEVICE_LOST` | +7 | GPU 设备丢失（重建） |
| `WM_GPUI_KEYDOWN` | +8 | 键盘按下 |
| `WM_GPUI_TRAY_ICON` | +9 | 托盘图标事件 |

分派在 `platform.rs:1073-1079`（`WM_COMMAND` 走 `handle_tray_menu_command`）。

## DComp 与子窗口

- DComp 以 `CreateTargetForHwnd(hwnd, false)` 创建，子 HWND（如 WebView2）显示在
  DComp 内容之上——**改回 `true` 会遮挡 WebView**。
- 窗口过程把 `WM_NCHITTEST`、`DM_POINTERHITTEST`（`events.rs:151`，Direct
  Manipulation）等按需下发给回调。

## 最佳实践

- **Windows 专属能力都用内核抽象**（`Platform`/`App`/`Window`），不直接调后端。
- **托盘/穿透/Mica/自启动**：应用侧 API 见 `platform.md`。
- **跨平台改动本机 `cargo check` 不编译 macOS/Linux 分支**，靠 CI 矩阵验证。
- **子 HWND 覆盖**：保持 DComp 目标以 `CreateTargetForHwnd(hwnd, false)` 创建。
