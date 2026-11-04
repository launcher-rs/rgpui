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

`pub trait Platform: 'static`（`platform.rs:302`），约 35+ 自有方法，按类别：

| 类别 | 方法（行号） |
|------|-------------|
| 执行器/文本 | `background_executor`(:304)、`foreground_executor`(:306)、`text_system`(:308) |
| 生命周期 | `run`(:311)、`quit`(:313)、`restart`(:315)、`activate`(:317)、`hide`(:319)、`hide_other_apps`/`unhide_other_apps`(:321/:323) |
| 显示 | `displays`(:326)、`primary_display`(:328)、`active_window`(:330)、`window_stack`(:332)、`is_screen_capture_supported`(:337)、`screen_capture_sources`(:342) |
| 外观 | `window_appearance`(:362)、`reduce_motion_enabled`(:368，系统「减少动态效果」)、`button_layout`(:373) |
| URL | `open_url`(:378)、`on_open_urls`(:380)、`register_url_scheme`(:382) |
| 文件 | `prompt_for_paths`(:385)、`prompt_for_new_path`(:390)、`can_select_mixed_files_and_dirs`(:396)、`reveal_path`(:398)、`open_with_system`(:400) |
| 菜单 | `set_menus`(:408)、`get_menus`(:410)、`set_dock_menu`(:415)、`perform_dock_menu_action`(:417)、`add_recent_document`(:419)、`update_jump_list`(:421)、`on_app_menu_action`(:429)、`on_will_open_app_menu`(:431)、`on_validate_app_menu_command`(:433) |
| 电源/热 | `thermal_state`(:436)、`on_thermal_state_change`(:438)、`on_system_power_event`(:555)、`on_system_wake`(:558)、`start_power_save_blocker`(:567)、`system_idle_time`(:575) |
| 系统信息 | `os_info`(:585)、`focused_window_info`(:537)、`network_status`(:580)、`compositor_name`(:441)、`app_path`(:445) |
| 剪贴板 | `read_from_clipboard`/`write_to_clipboard`(:462/:464)、`read_from_primary`/`write_to_primary`(:468/:471)、`read_from_find_pasteboard`/`write_to_find_pasteboard`(:475/:478)；富负载用 `ClipboardEntry::Format(ClipboardFormat)`（`new_html`/`new_rtf`/`new_with_format`，MIME 常量 `ClipboardFormat::HTML` 等，读取 `text`/`html`/`rtf`/`format`/`formats`） |
| 凭据 | `write_credentials`/`read_credentials`/`delete_credentials`(:481/:483/:485) |
| 键盘 | `keyboard_layout`(:488)、`keyboard_mapper`(:490)、`on_keyboard_layout_change`(:492) |
| 托盘 | `set_tray`(:495)、`set_tray_icon`(:497)、`set_tray_menu`(:499)、`set_tray_tooltip`(:501)、`set_tray_panel_mode`(:503)、`get_tray_icon_bounds`(:505)、`on_tray_icon_event`(:509)、`on_tray_menu_action`(:511) |
| 热键 | `register_global_hotkey`(:514)、`unregister_global_hotkey`(:518)、`on_global_hotkey`(:520) |
| 通知 | `show_notification`(:523) |
| 自启动 | `set_auto_launch`(:528)、`is_auto_launch_enabled`(:532) |
| 权限 | `check_permission`(:548)、`request_permission`(:552) |
| 光标 | `set_cursor_style`(:450)、`hide_cursor_until_mouse_moves`(:453)、`is_cursor_visible`(:456)、`should_auto_hide_scrollbars`(:459) |

类型：`SystemPowerEvent{Sleep,WakeUp}`(:178)、
`PowerSaveBlockerKind{PreventSleep,PreventDisplaySleep}`(:187)、`trait PowerSaveBlocker`(:198)、
`OsInfo{name,version}`(:202)、`PermissionStatus{NotDetermined,Granted,Denied,Unavailable}`(:211)、
`PermissionType{Accessibility,ScreenCapture,InputMonitoring}`(:226)、
`NetworkStatus{Disconnected,ConnectedBelowRequired,Connected}`(:245)、
`FocusedWindowInfo{app_name,window_title,bundle_id,pid}`(:256)、`WindowPosition`(:269)。

## PlatformWindow

`pub trait PlatformWindow: HasWindowHandle + HasDisplayHandle`（`platform.rs:1142`）。
核心：`bounds`(:1144)、`is_maximized`(:1146)、`resize`(:1152)、`scale_factor`(:1154)、
`appearance`(:1156)、`activate`(:1178)、`is_active`(:1180)、`set_title`(:1186)、
`minimize`(:1190)、`zoom`(:1192)、`toggle_fullscreen`(:1194)、`draw`(:1220)、
`on_input`(:1200)、`on_resize`(:1206)、`on_should_close`(:1210)、
`on_appearance_changed`(:1216)、`is_subpixel_rendering_supported`(:1226)、
`gpu_specs`(:1339)、`set_input_handler`/`take_input_handler`(:1166/:1168)。

**rgpui 自有 API：**
`get_raw_handle() -> HWND`（`:1306`，**仅 Windows**，后端 `open_window` 登记 HWND 用）、
`set_mouse_passthrough(bool)`（`:1391`）、`set_position(Point<Pixels>)`（`:1385`）、
`hide()`（`:1388`）、`set_titlebar_visible(bool)`（`:1397`）、
`set_input_region(Option<&[Bounds<Pixels>]>)`（`:1321`）、`request_attention()`（`:1382`）、
`set_exclusive_zone(Pixels)`（`:1372`）、`set_exclusive_edge`（`:1379`）、
`set_text_content_type`（`:1401`）、`supports_dom`/`dom_tree_update`/`on_dom_event`/`on_dom_scroll`（`:1233`-`:1259`，`dom-backend`）。

## App 包装方法

`App`（`app.rs`）包装了应用层可调用能力：

| 方法 | 位置 |
|------|------|
| `quit()` | `:1058` |
| `reduce_motion()` | `:1081` |
| `windows()` | `:1283` |
| `activate(bool)` | `:1357` |
| `hide()` | `:1362` |
| `open_url(&str)` | `:1512` |
| `register_url_scheme(&str) -> Task<Result<()>>` | `:1520` |
| `prompt_for_paths(...)` | `:1548` |
| `reveal_path(&Path)` | `:1589` |
| `open_with_system(&Path)` | `:1594` |
| `restart()` | `:1604` |
| `set_menus(...)` / `get_menus()` | `:2530` / `:2536` |
| `on_app_menu_action(FnMut(&dyn Action, &mut App))` | `:2546` |
| `set_dock_menu(Vec<MenuItem>)` | `:2583` |
| `add_recent_document(&Path)` | `:2596` |
| `update_jump_list(...)` | `:2602` |
| `show_notification(title, body) -> Result<()>` | `:2637` |
| `push_notification(title, message) -> Result<()>` | `:2644` |
| `check_permission(PermissionType) -> PermissionStatus` | `:2685` |
| `request_permission(PermissionType)` | `:2690` |
| `os_info() -> OsInfo` | `:2695` |
| `system_idle_time() -> Option<Duration>` | `:2700` |
| `network_status() -> NetworkStatus` | `:2705` |
| `on_system_power_event(...)` | `:2714` |
| `start_power_save_blocker(...)` | `:2730` |
| `set_auto_launch(app_id, enabled)` | `:2738` |
| `is_auto_launch_enabled(app_id)` | `:2743` |
| `focused_window_info() -> Option<FocusedWindowInfo>` | `:2748` |
| `register_global_hotkey(id, &Keystroke)` | `:2806` |
| `unregister_global_hotkey(id)` | `:2814` |
| `on_global_hotkey(FnMut(u32, &mut App))` | `:2819` |
| `set_keep_alive_without_windows(bool)` | `:2677` |

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

**App 方法：** `set_tray(Tray, Option<Vec<MenuItem>>)`（`app.rs:2611`）、
`set_tray_icon(Option<&[u8]>)`（`:2616`）、`set_tray_menu(Vec<TrayMenuItem>)`（`:2621`）、
`set_tray_tooltip(&str)`（`:2626`）、`set_tray_panel_mode(bool)`（`:2632`）、
`tray_icon_bounds() -> Option<Bounds<Pixels>>`（`:2649`）、
`on_tray_icon_event(FnMut(TrayIconEvent, &mut App))`（`:2654`）、
`on_tray_menu_action(FnMut(SharedString, &mut App))`（`:2664`）。
图标接受 PNG/ICO 字节。Windows 实现见 `windows.md`。

## 权限

统一走 `App::check_permission(PermissionType)` / `request_permission(kind)`，
`PermissionType` 仅三类：`Accessibility`/`ScreenCapture`/`InputMonitoring`。
**不要为单个类别加特例方法。**

## 单实例

`pub mod single_instance;`（`rgpui.rs:183`）+ `pub use single_instance::*`（`:615`）：
`AlreadyRunning`（`single_instance/mod.rs:24`）、`SingleInstance`（`:86`）：
`acquire(app_id) -> Result<Self, AlreadyRunning>`（`:129`）、
`on_activate(&App, FnMut(&mut App))`（`:297`）、`send_activate_to_existing(app_id)`（`:364`）。

`acquire` 成功即启动激活监听线程；`send_activate_to_existing` 在 Windows（命名事件）与
Unix（套接字）都可用，会触发已运行实例注册的回调。回调调度到**主线程**执行，可以直接
操作 `App`；注册之前到达的激活信号会缓存一次，注册后立即补发。

```rust
let instance = match SingleInstance::acquire(APP_ID) {
    Ok(instance) => instance,
    Err(_) => {
        let _ = send_activate_to_existing(APP_ID);
        std::process::exit(0);
    }
};

application().run(move |cx| {
    instance.on_activate(cx, |cx| {
        cx.show_all_windows();
        cx.activate(true);
    });
});
```

## 平台能力实现矩阵

这些能力在 `Platform` 上有默认实现（返回空值/`None`），各后端必须真正实现，
否则应用层拿到的是「看似成功」的空结果。1.6 审查后的实际实现：

| 能力 | Windows（`rgpui-windows`） | macOS（`rgpui-macos`） | Linux（`rgpui-linux`） |
|------|---------------------------|------------------------|------------------------|
| 全局热键 | `RegisterHotKey` + `WM_HOTKEY` | Carbon `RegisterEventHotKey`（`global_hotkey.rs`） | X11 `XGrabKey`；Wayland 注册时如实报错 |
| `os_info` | `RtlGetVersion`（按 build ≥ 22000 报 Windows 11） | `NSProcessInfo.operatingSystemVersion` | `/etc/os-release`，缺失时 `uname` |
| `system_idle_time` | `GetLastInputInfo` + `GetTickCount64`（处理 32 位回绕） | `CGEventSourceSecondsSinceLastEventType` | D-Bus：ScreenSaver → Mutter IdleMonitor |
| `network_status` | `InternetGetConnectedState` | `getifaddrs`（非回环、UP、有地址） | 门户 `NetworkMonitor`，回退 `/sys/class/net` |
| 电源事件 | `PowerRegisterSuspendResumeNotification` → 主线程消息窗口 | `NSWorkspace` WillSleep/DidWake 通知 | login1 `PrepareForSleep` 信号 |
| 电源抑制 | `SetThreadExecutionState(ES_CONTINUOUS|…)` | IOKit `IOPMAssertionCreateWithName` | logind `Inhibit`（fd 持有即生效） |
| `register_url_scheme` | `HKCU\Software\Classes\<scheme>` | `NSWorkspace.setDefaultApplicationAtURL`（macOS 12+） | `$XDG_DATA_HOME/applications` 桌面入口 + `xdg-mime default` |
| `focused_window_info` | `GetForegroundWindow` + 进程查询 | `NSWorkspace` + `AXUIElement`（需辅助功能授权） | X11 EWMH；**Wayland 原生会话返回 `None`**（无等价协议） |

两条易踩的边界：

- `focused_window_info()` 返回 `None` 表示「问不到」，不是「没有窗口」；Linux
  Wayland 会话必然走到这里，不要把它当成空数据渲染成 `"Unknown"`。
- `system_idle_time()` 返回 `None` 表示本轮探测失败；Windows 的 `GetLastInputInfo`
  成功但刚有输入时会给 `Some(0)`，两者语义不同。

## 最佳实践

- **加平台能力三件套**：`Platform` 方法 + `App` 包装 + 真实调用点。
- **权限只走 `check_permission`/`request_permission`**，不要新增特例方法。
- **托盘隐藏窗口**：`hide_window()`（移出任务栏）vs `minimize_window()`（保留图标）；
  恢复用 `activate_window()`。
- **`App::platform` 私有**：应用层只用包装方法。
- **`reduce_motion()` 会自动被动画遵循**（见 `animation.md`）。
