# 平台抽象（Platform）

**目录：** [概述](#概述) · [启动应用](#启动应用) · [Platform trait](#platform-trait) · [PlatformWindow](#platformwindow) · [App 包装方法](#app-包装方法) · [窗口方法](#窗口方法) · [托盘](#托盘) · [权限](#权限) · [单实例](#单实例) · [最佳实践](#最佳实践)

## 概述

`Platform` / `PlatformWindow` trait 定义在 `crates/rgpui/src/platform.rs`，各平台后端
（`rgpui-windows`/`rgpui-linux`/`rgpui-macos`/`rgpui-web`）实现。应用经
`rgpui_platform::application()` 启动。**`App::platform` 是私有字段**——应用层只能通过
`App` 的包装方法访问平台能力。

**关键约定（AGENTS.md）：** 给 `Platform` 加方法时**必须同时加 `App` 包装 + 一个真实
调用点**（示例或核心逻辑），否则应用层一行都调不到。

## 启动应用

`rgpui_platform::application() -> rgpui::Application`（`rgpui-platform/src/rgpui_platform.rs`）。
`Application::with_platform`（`app.rs:179`）。**`Application` 没有 `new()`**。

```rust
fn main() {
    rgpui_platform::application().run(|cx: &mut App| {
        cx.open_window(Default::default(), |window, cx| {
            cx.new(|cx| MyView::new(window, cx))
        }).unwrap();
    });
}
```

Web 端先调 `rgpui_platform::web_init()`（`#![cfg_attr(target_family = "wasm", no_main)]` +
`#[wasm_bindgen(start)]`）。

## Platform trait

`pub trait Platform: 'static`（`platform.rs:265`），约 35+ 自有方法，按类别：

| 类别 | 方法（行号） |
|------|-------------|
| 执行器/文本 | `background_executor`(:267)、`foreground_executor`(:269)、`text_system`(:271) |
| 生命周期 | `run`(:274)、`quit`(:276)、`restart`(:278)、`activate`(:280)、`hide`(:282)、`hide_other_apps`/`unhide_other_apps`(:284/:286) |
| 显示 | `displays`(:289)、`primary_display`(:291)、`active_window`(:293)、`window_stack`(:295)、`screen_capture_sources`(:305)、`is_screen_capture_supported`(:300) |
| 外观 | `window_appearance`(:325)、`reduce_motion_enabled`(:331，系统「减少动态效果」)、`button_layout`(:336) |
| URL | `open_url`(:341)、`on_open_urls`(:343)、`register_url_scheme`(:345) |
| 文件 | `prompt_for_paths`(:348)、`prompt_for_new_path`(:353)、`reveal_path`(:361)、`open_with_system`(:363) |
| 菜单 | `set_menus`(:371)、`get_menus`(:373)、`set_dock_menu`(:378)、`perform_dock_menu_action`(:380)、`add_recent_document`(:382)、`update_jump_list`(:384)、`on_app_menu_action`(:392)、`on_will_open_app_menu`(:394)、`on_validate_app_menu_command`(:396) |
| 电源/热 | `thermal_state`(:399)、`on_thermal_state_change`(:401)、`on_system_power_event`(:518)、`on_system_wake`(:521)、`start_power_save_blocker`(:530)、`system_idle_time`(:538) |
| 系统信息 | `os_info`(:557)、`focused_window_info`(:500)、`network_status`(:543)、`compositor_name`(:404)、`app_path`(:408) |
| 剪贴板 | `read_from_clipboard`/`write_to_clipboard`(:425/:427)、`read_from_primary`/`write_to_primary`(:431/:434)、`read_from_find_pasteboard`/`write_to_find_pasteboard`(:438/:441) |
| 凭据 | `write_credentials`/`read_credentials`/`delete_credentials`(:444/:446/:448) |
| 键盘 | `keyboard_layout`(:451)、`keyboard_mapper`(:453)、`on_keyboard_layout_change`(:455) |
| 托盘 | `set_tray`(:458)、`set_tray_icon`(:460)、`set_tray_menu`(:462)、`set_tray_tooltip`(:464)、`set_tray_panel_mode`(:466)、`get_tray_icon_bounds`(:468)、`on_tray_icon_event`(:472)、`on_tray_menu_action`(:474) |
| 热键 | `register_global_hotkey`(:477)、`unregister_global_hotkey`(:481)、`on_global_hotkey`(:483) |
| 通知 | `show_notification`(:486) |
| 自启动 | `set_auto_launch`(:491)、`is_auto_launch_enabled`(:495) |
| 权限 | `check_permission`(:511)、`request_permission`(:515) |
| 光标 | `set_cursor_style`(:413)、`hide_cursor_until_mouse_moves`(:416)、`is_cursor_visible`(:419)、`should_auto_hide_scrollbars`(:422) |
| 上下文菜单 | `show_context_menu`(:548) |

类型：`SystemPowerEvent{Sleep,WakeUp}`(:141)、
`PowerSaveBlockerKind{PreventSleep,PreventDisplaySleep}`(:150)、`trait PowerSaveBlocker`(:161)、
`OsInfo{name,version}`(:165)、`PermissionStatus{NotDetermined,Granted,Denied,Unavailable}`(:174)、
`PermissionType{Accessibility,ScreenCapture,InputMonitoring}`(:189)、
`NetworkStatus{Disconnected,ConnectedBelowRequired,Connected}`(:208)、
`FocusedWindowInfo{app_name,window_title,bundle_id,pid}`(:219)、`WindowPosition`(:232)。

## PlatformWindow

`pub trait PlatformWindow: HasWindowHandle + HasDisplayHandle`（`platform.rs:1114`）。
核心：`bounds`(:1116)、`is_maximized`(:1118)、`resize`(:1124)、`scale_factor`(:1126)、
`appearance`(:1128)、`activate`(:1150)、`is_active`(:1152)、`set_title`(:1158)、
`minimize`(:1162)、`zoom`(:1164)、`toggle_fullscreen`(:1166)、`draw`(:1192)、
`on_input`(:1172)、`on_resize`(:1178)、`on_should_close`(:1182)、
`on_appearance_changed`(:1188)、`is_subpixel_rendering_supported`(:1198)、
`gpu_specs`(:1311)、`set_input_handler`/`take_input_handler`(:1138/:1140)。

**rgpui 自有 API：**
`get_raw_handle() -> HWND`（`:1278`，**仅 Windows**，后端 `open_window` 登记 HWND 用）、
`set_mouse_passthrough(bool)`（`:1363`）、`set_position(Point<Pixels>)`（`:1357`）、
`hide()`（`:1360`）、`set_titlebar_visible(bool)`（`:1369`）、
`set_input_region(Option<&[Bounds<Pixels>]>)`（`:1293`）、`request_attention()`（`:1354`）、
`set_exclusive_zone(Pixels)`（`:1344`）、`set_exclusive_edge`（`:1351`）、
`set_text_content_type`（`:1373`）、`supports_dom`/`dom_tree_update`/`on_dom_event`/`on_dom_scroll`（`:1205`-`:1231`，`dom-backend`）。

## App 包装方法

`App`（`app.rs`）包装了应用层可调用能力：

| 方法 | 位置 |
|------|------|
| `quit()` | `:1015` |
| `reduce_motion()` | `:1038` |
| `windows()` | `:1240` |
| `activate(bool)` | `:1313` |
| `hide()` | `:1318` |
| `open_url(&str)` | `:1468` |
| `register_url_scheme(&str) -> Task<Result<()>>` | `:1476` |
| `prompt_for_paths(...)` | `:1504` |
| `reveal_path(&Path)` | `:1545` |
| `open_with_system(&Path)` | `:1550` |
| `restart()` | `:1560` |
| `set_menus(...)` / `get_menus()` | `:2471` / `:2477` |
| `on_app_menu_action(FnMut(&dyn Action, &mut App))` | `:2487` |
| `set_dock_menu(Vec<MenuItem>)` | `:2524` |
| `add_recent_document(&Path)` | `:2537` |
| `update_jump_list(...)` | `:2543` |
| `show_notification(title, body) -> Result<()>` | `:2578` |
| `push_notification(title, message) -> Result<()>` | `:2585` |
| `check_permission(PermissionType) -> PermissionStatus` | `:2626` |
| `request_permission(PermissionType)` | `:2631` |
| `os_info() -> OsInfo` | `:2636` |
| `system_idle_time() -> Option<Duration>` | `:2641` |
| `network_status() -> NetworkStatus` | `:2646` |
| `on_system_power_event(...)` | `:2655` |
| `start_power_save_blocker(...)` | `:2671` |
| `set_auto_launch(app_id, enabled)` | `:2679` |
| `is_auto_launch_enabled(app_id)` | `:2684` |
| `focused_window_info() -> Option<FocusedWindowInfo>` | `:2689` |
| `register_global_hotkey(id, &Keystroke)` | `:2747` |
| `unregister_global_hotkey(id)` | `:2755` |
| `on_global_hotkey(FnMut(u32, &mut App))` | `:2760` |
| `set_keep_alive_without_windows(bool)` | `:2618` |

## 窗口方法

`Window`（`window.rs`）：`activate_window()`（`:5989`）、`request_attention()`（`:5994`）、
`minimize_window()`（`:5999`）、`hide_window()`（`:6004`）、
`set_mouse_passthrough(bool)`（`:6009`）、`set_position(Point<Pixels>)`（`:6014`）、
`set_titlebar_visible(bool)`（`:2311`）、`set_input_region`（`:2343`）、
`is_window_active()`（`:2678`）、`on_window_should_close(...)`（`:6241`）。

## 托盘

类型经 `pub use system::tray::*`（`rgpui.rs:543`）在根导出：
`TrayIconEvent{LeftClick,RightClick,DoubleClick}`（`system/tray.rs:9`）、
`TrayMenuItem::{Action{label,id},Separator,Submenu{label,items},Toggle{label,checked,id}}`（`:20`）、
`Tray{tooltip,icon,icon_data,menu_builder,visible}`（`:50`）。

**App 方法：** `set_tray(Tray, Option<Vec<MenuItem>>)`（`app.rs:2552`）、
`set_tray_icon(Option<&[u8]>)`（`:2557`）、`set_tray_menu(Vec<TrayMenuItem>)`（`:2562`）、
`set_tray_tooltip(&str)`（`:2567`）、`set_tray_panel_mode(bool)`（`:2573`）、
`tray_icon_bounds() -> Option<Bounds<Pixels>>`（`:2590`）、
`on_tray_icon_event(FnMut(TrayIconEvent, &mut App))`（`:2595`）、
`on_tray_menu_action(FnMut(SharedString, &mut App))`（`:2605`）。
图标接受 PNG/ICO 字节。Windows 实现见 `windows.md`。

## 权限

统一走 `App::check_permission(PermissionType)` / `request_permission(kind)`，
`PermissionType` 仅三类：`Accessibility`/`ScreenCapture`/`InputMonitoring`。
**不要为单个类别加特例方法。**

## 单实例

`pub mod single_instance;`（`rgpui.rs:166`）+ `pub use single_instance::*`（`:512`）：
`AlreadyRunning`（`single_instance/mod.rs:10`）、`SingleInstance`（`:21`）：
`acquire(app_id) -> Result<Self, AlreadyRunning>`（`:45`）、`on_activate(Fn())`（`:124`）、
`send_activate_to_existing(app_id)`（`:169`）。

## 最佳实践

- **加平台能力三件套**：`Platform` 方法 + `App` 包装 + 真实调用点。
- **权限只走 `check_permission`/`request_permission`**，不要新增特例方法。
- **托盘隐藏窗口**：`hide_window()`（移出任务栏）vs `minimize_window()`（保留图标）；
  恢复用 `activate_window()`。
- **`App::platform` 私有**：应用层只用包装方法。
- **`reduce_motion()` 会自动被动画遵循**（见 `animation.md`）。
