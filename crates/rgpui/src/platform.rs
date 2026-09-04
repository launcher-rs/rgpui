//! 平台抽象层：定义 `Platform` trait 和 `PlatformWindow` trait，供各平台 crate 实现。

mod app_menu;
mod keyboard;
mod keystroke;

/// 用于配置父窗口锚定弹出窗口的类型，如下拉菜单、弹出菜单和工具提示。
pub mod popup;

#[cfg(all(
    any(test, feature = "test-support"),
    any(target_os = "windows", target_os = "linux", target_family = "wasm")
))]
mod threaded_dispatcher;

/// Wayland Layer Shell 支持 — 允许窗口作为覆盖层、面板或桌面背景渲染。
#[cfg(all(target_os = "linux", feature = "wayland"))]
pub mod layer_shell;

#[cfg(any(test, feature = "test-support"))]
mod test;

#[cfg(all(target_os = "macos", any(test, feature = "test-support")))]
mod visual_test;

#[cfg(all(
    feature = "screen-capture",
    any(target_os = "windows", target_os = "linux", target_os = "freebsd",)
))]
pub mod scap_screen_capture;

#[cfg(all(
    any(target_os = "windows", target_os = "linux"),
    feature = "screen-capture"
))]
pub(crate) type PlatformScreenCaptureFrame = scap::frame::Frame;
#[cfg(not(feature = "screen-capture"))]
pub(crate) type PlatformScreenCaptureFrame = ();
#[cfg(all(target_os = "macos", feature = "screen-capture"))]
pub(crate) type PlatformScreenCaptureFrame = core_video::image_buffer::CVImageBuffer;

use crate::rgpui_util;
use crate::scheduler::Instant;
pub use crate::scheduler::RunnableMeta;
use crate::{
    Action, AnyWindowHandle, App, AsyncWindowContext, BackgroundExecutor, Bounds,
    DEFAULT_WINDOW_SIZE, DevicePixels, DispatchEventResult, Font, FontId, FontMetrics, FontRun,
    ForegroundExecutor, GlyphId, GpuSpecs, Hsla, ImageSource, Keymap, LineLayout, Pixels,
    PlatformInput, Point, Priority, RenderGlyphParams, RenderImage, RenderImageParams,
    RenderSvgParams, Scene, ShapedGlyph, ShapedRun, SharedString, Size, SvgRenderer,
    SystemWindowTab, Task, Window, WindowControlArea, hash, point, px, size,
};
use crate::{Tray, TrayIconEvent, TrayMenuItem};
use anyhow::Result;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
use anyhow::bail;
use async_task::Runnable;
use futures::channel::oneshot;
#[cfg(any(test, feature = "test-support"))]
use image::RgbaImage;
use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder as _, Frame};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use schemars::JsonSchema;
use seahash::SeaHasher;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::ops;
use std::time::Duration;
use std::{
    fmt::{self, Debug},
    ops::Range,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};
use strum::EnumIter;
use uuid::Uuid;

pub use app_menu::*;
pub use keyboard::*;
pub use keystroke::*;

#[cfg(any(test, feature = "test-support"))]
pub(crate) use test::*;

#[cfg(any(test, feature = "test-support"))]
pub use test::{TestDispatcher, TestScreenCaptureSource, TestScreenCaptureStream};

#[cfg(all(
    any(test, feature = "test-support"),
    any(target_os = "windows", target_os = "linux", target_family = "wasm")
))]
pub use threaded_dispatcher::ThreadedDispatcher;

#[cfg(all(target_os = "macos", any(test, feature = "test-support")))]
pub use visual_test::VisualTestPlatform;

// TODO(jk): return an enum instead of a string
/// 返回当前使用的合成器名称（猜测），
/// 不会尝试连接到指定的合成器。
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[inline]
pub fn guess_compositor() -> &'static str {
    if std::env::var_os("ZED_HEADLESS").is_some() {
        return "Headless";
    }

    #[cfg(feature = "wayland")]
    let wayland_display = std::env::var_os("WAYLAND_DISPLAY");
    #[cfg(not(feature = "wayland"))]
    let wayland_display: Option<std::ffi::OsString> = None;

    #[cfg(feature = "x11")]
    let x11_display = std::env::var_os("DISPLAY");
    #[cfg(not(feature = "x11"))]
    let x11_display: Option<std::ffi::OsString> = None;

    let use_wayland = wayland_display.is_some_and(|display| !display.is_empty());
    let use_x11 = x11_display.is_some_and(|display| !display.is_empty());

    if use_wayland {
        "Wayland"
    } else if use_x11 {
        "X11"
    } else {
        "Headless"
    }
}

// ============================================================================
// 缺失的系统类型定义
// ============================================================================

/// 系统电源事件
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemPowerEvent {
    /// 系统即将进入睡眠
    Sleep,
    /// 系统已从睡眠唤醒
    WakeUp,
}

/// 电源阻止器类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerSaveBlockerKind {
    /// 阻止系统休眠
    PreventSleep,
    /// 阻止屏幕关闭
    PreventDisplaySleep,
}

/// 操作系统信息
#[derive(Debug, Clone)]
pub struct OsInfo {
    /// 操作系统名称
    pub name: String,
    /// 操作系统版本
    pub version: String,
}

/// 权限状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionStatus {
    /// 未确定
    NotDetermined,
    /// 已授权
    Granted,
    /// 已拒绝
    Denied,
    /// 不可用
    Unavailable,
}

/// 权限类型（用于描述应用在系统中申请的权限类别）
///
/// 通常用于 macOS / Windows 等系统能力访问控制，例如辅助功能、屏幕录制、输入监控等。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionType {
    /// 辅助功能权限（Accessibility）
    ///
    /// 用于允许应用模拟用户操作、读取 UI 元素、控制系统界面等能力。
    Accessibility,

    /// 屏幕录制/屏幕捕获权限（Screen Capture）
    ///
    /// 用于获取屏幕内容，例如截图、录屏或远程桌面功能。
    ScreenCapture,

    /// 输入监控权限（Input Monitoring）
    ///
    /// 用于监听键盘和鼠标输入事件（如全局快捷键、输入记录等）。
    InputMonitoring,
}

/// 网络状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkStatus {
    /// 无法连接网络
    Disconnected,
    /// 已连接但不满足服务要求
    ConnectedBelowRequired,
    /// 已连接且满足服务要求
    Connected,
}

/// 媒体键事件
#[derive(Debug, Clone)]
pub struct MediaKeyEvent {
    /// 键码
    pub key_code: u16,
}

/// 生物识别状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiometricStatus {
    /// 不可用
    Unavailable,
    /// 已解锁
    Unlocked,
    /// 已锁定
    Locked,
}

/// 用户注意力请求类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttentionType {
    /// 请求非关键性注意（如弹跳 Dock 图标一次）
    Informational,
    /// 请求关键性注意（如弹跳 Dock 图标直到被激活）
    Critical,
}

/// 对话框选项
#[derive(Debug, Clone)]
pub struct DialogOptions {
    /// 对话框类型
    pub dialog_type: DialogType,
    /// 对话框标题
    pub title: String,
    /// 对话框消息
    pub message: String,
    /// 确认按钮文本
    pub confirm_label: Option<String>,
    /// 取消按钮文本
    pub cancel_label: Option<String>,
}

/// 对话框类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogType {
    /// 信息提示
    Info,
    /// 警告
    Warning,
    /// 错误
    Error,
}

/// 聚焦窗口信息
#[derive(Debug, Clone)]
pub struct FocusedWindowInfo {
    /// 窗口所属应用名称
    pub app_name: String,
    /// 窗口标题
    pub window_title: String,
    /// Bundle ID（macOS 特有）
    pub bundle_id: Option<String>,
    /// 进程 ID
    pub pid: Option<u32>,
}

/// 语义化窗口位置，用于计算窗口的屏幕位置
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowPosition {
    /// 在主显示区域居中
    Center,
    /// 在指定显示区域居中
    CenterOnDisplay(DisplayId),
    /// 在托盘图标上方居中
    TrayCenter(Bounds<Pixels>),
    /// 屏幕右上角（带边距）
    TopRight {
        /// 与屏幕边缘的距离
        margin: Pixels,
    },
    /// 屏幕右下角（带边距）
    BottomRight {
        /// 与屏幕边缘的距离
        margin: Pixels,
    },
    /// 屏幕左上角（带边距）
    TopLeft {
        /// 与屏幕边缘的距离
        margin: Pixels,
    },
    /// 屏幕左下角（带边距）
    BottomLeft {
        /// 与屏幕边缘的距离
        margin: Pixels,
    },
}

/// 跨平台应用抽象层，由各平台 crate（rgpui-windows、rgpui-macos、rgpui-linux、rgpui-web）实现。
///
/// 提供应用生命周期、窗口管理、系统集成（托盘、快捷键、通知、电源等）的统一接口。
/// 应用通过 [`rgpui_platform::application()`] 获取实现此 trait 的实例。
pub trait Platform: 'static {
    /// 返回后台线程执行器，用于调度异步任务。
    fn background_executor(&self) -> BackgroundExecutor;
    /// 返回主线程执行器，用于调度需要在 UI 线程运行的任务。
    fn foreground_executor(&self) -> ForegroundExecutor;
    /// 返回文本渲染系统实例，负责字体加载、文本布局和渲染。
    fn text_system(&self) -> Arc<dyn PlatformTextSystem>;

    /// 启动应用主循环，`on_finish_launching` 在启动完成后回调。
    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>);
    /// 退出应用进程。
    fn quit(&self);
    /// 重启应用，可选指定新的二进制路径。
    fn restart(&self, binary_path: Option<PathBuf>);
    /// 激活应用（将窗口置于前台），`ignoring_other_apps` 在 macOS 下是否忽略其他应用。
    fn activate(&self, ignoring_other_apps: bool);
    /// 隐藏应用（macOS 下隐藏所有窗口，其他平台最小化）。
    fn hide(&self);
    /// 隐藏当前应用以外的所有其他应用的窗口。
    fn hide_other_apps(&self);
    /// 取消隐藏所有被 `hide_other_apps` 隐藏的应用。
    fn unhide_other_apps(&self);

    /// 返回所有可用显示器的列表。
    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>>;
    /// 返回主显示器（包含任务栏/菜单栏的显示器）。
    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>>;
    /// 返回当前获得焦点的窗口句柄。
    fn active_window(&self) -> Option<AnyWindowHandle>;
    /// 返回窗口栈（Z-order），从最顶层到最底层。
    fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        None
    }

    /// 当前平台是否支持屏幕捕获功能。
    fn is_screen_capture_supported(&self) -> bool {
        false
    }

    /// 获取可用的屏幕捕获源列表（屏幕/窗口），通过 oneshot channel 异步返回。
    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<anyhow::Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
        let (sources_tx, sources_rx) = oneshot::channel();
        sources_tx
            .send(Err(anyhow::anyhow!(
                "rgpui was compiled without the screen-capture feature"
            )))
            .ok();
        sources_rx
    }

    /// 根据窗口参数创建平台原生窗口，返回 `PlatformWindow` 实例。
    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>>;

    /// 返回应用窗口的外观模式（亮色/暗色）。
    fn window_appearance(&self) -> WindowAppearance;

    /// 返回窗口按钮布局配置（如 macOS 红绿灯位置、Windows 按钮顺序）。
    fn button_layout(&self) -> Option<WindowButtonLayout> {
        None
    }

    /// 在系统默认浏览器中打开 URL。
    fn open_url(&self, url: &str);
    /// 注册 URL scheme 回调，当应用通过自定义 URL scheme 打开时触发。
    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>);
    /// 注册自定义 URL scheme（如 `myapp://`），使系统将该 scheme 的 URL 分发到本应用。
    fn register_url_scheme(&self, url: &str) -> Task<Result<()>>;

    /// 打开文件选择对话框，返回用户选择的文件路径列表。
    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>>;
    /// 打开文件保存对话框，返回用户指定的保存路径。
    fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>>;
    /// 文件选择对话框是否支持同时选择文件和目录。
    fn can_select_mixed_files_and_dirs(&self) -> bool;
    /// 在系统文件管理器中显示（reveal）指定路径。
    fn reveal_path(&self, path: &Path);
    /// 使用系统默认应用打开指定路径。
    fn open_with_system(&self, path: &Path);

    /// 注册应用退出时的回调。
    fn on_quit(&self, callback: Box<dyn FnMut()>);
    /// 注册应用从后台恢复（macOS Dock 图标点击）时的回调。
    fn on_reopen(&self, callback: Box<dyn FnMut()>);

    /// 设置应用菜单栏（macOS 为全局菜单栏，其他平台为窗口菜单）。
    fn set_menus(&self, menus: Vec<Menu>, keymap: &Keymap);
    /// 获取当前应用菜单的副本。
    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        None
    }

    /// 设置 macOS Dock 栏右键菜单。
    fn set_dock_menu(&self, menu: Vec<MenuItem>, keymap: &Keymap);
    /// 执行 Dock 菜单中的操作。
    fn perform_dock_menu_action(&self, _action: usize) {}
    /// 将路径添加到最近打开文档列表。
    fn add_recent_document(&self, _path: &Path) {}
    /// 更新 Windows 跳转列表（任务栏右键菜单中的最近文档）。
    fn update_jump_list(
        &self,
        _menus: Vec<MenuItem>,
        _entries: Vec<SmallVec<[PathBuf; 2]>>,
    ) -> Task<Vec<SmallVec<[PathBuf; 2]>>> {
        Task::ready(Vec::new())
    }
    /// 注册应用菜单操作回调，当用户点击菜单项时触发。
    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>);
    /// 注册菜单即将打开时的回调（可用于动态更新菜单项状态）。
    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>);
    /// 注册菜单命令验证回调，返回 `false` 可禁用菜单项。
    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>);

    /// 返回系统热状态（正常、警告、临界）。
    fn thermal_state(&self) -> ThermalState;
    /// 注册系统热状态变化回调。
    fn on_thermal_state_change(&self, callback: Box<dyn FnMut()>);

    /// 返回合成器名称（如 "dwm"、"mutter"），用于诊断。
    fn compositor_name(&self) -> &'static str {
        ""
    }
    /// 返回应用自身可执行文件的路径。
    fn app_path(&self) -> Result<PathBuf>;
    /// 返回辅助可执行文件的路径（如子进程、插件）。
    fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf>;

    /// 设置鼠标光标样式（箭头、手型、文本光标等）。
    fn set_cursor_style(&self, style: CursorStyle);

    /// 隐藏鼠标光标，直到用户移动鼠标时自动恢复显示。
    fn hide_cursor_until_mouse_moves(&self);

    /// 返回鼠标光标当前是否可见。
    fn is_cursor_visible(&self) -> bool;

    /// 是否自动隐藏滚动条（鼠标靠近时才显示）。
    fn should_auto_hide_scrollbars(&self) -> bool;

    /// 从系统剪贴板读取内容。
    fn read_from_clipboard(&self) -> Option<ClipboardItem>;
    /// 写入内容到系统剪贴板。
    fn write_to_clipboard(&self, item: ClipboardItem);

    /// 从 Linux/X11 主选择区（Primary Selection）读取内容。
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem>;
    /// 写入内容到 Linux/X11 主选择区。
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, item: ClipboardItem);

    /// 从 macOS 查找粘贴板（Find Pasteboard）读取内容。
    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem>;
    /// 写入内容到 macOS 查找粘贴板。
    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, item: ClipboardItem);

    /// 将凭据（URL、用户名、密码）写入系统密钥链。
    fn write_credentials(&self, url: &str, username: &str, password: &[u8]) -> Task<Result<()>>;
    /// 从系统密钥链读取指定 URL 的凭据。
    fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>>;
    /// 从系统密钥链删除指定 URL 的凭据。
    fn delete_credentials(&self, url: &str) -> Task<Result<()>>;

    /// 返回当前键盘布局信息。
    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout>;
    /// 返回键盘映射器，用于将原始按键事件转换为字符输入。
    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper>;
    /// 注册键盘布局变化回调（用户切换输入法/布局时触发）。
    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>);

    /// 设置系统托盘图标、菜单和按键绑定。
    fn set_tray(&self, _tray: Tray, _menus: Option<Vec<MenuItem>>, _keymap: &Keymap) {}
    /// 更新系统托盘图标，`None` 表示移除图标。
    fn set_tray_icon(&self, _icon: Option<&[u8]>) {}
    /// 更新系统托盘右键菜单。
    fn set_tray_menu(&self, _menu: Vec<TrayMenuItem>) {}
    /// 设置鼠标悬停在托盘图标上时显示的工具提示文本。
    fn set_tray_tooltip(&self, _tooltip: &str) {}
    /// 设置托盘面板模式（Windows 下影响图标的显示行为）。
    fn set_tray_panel_mode(&self, _enabled: bool) {}
    /// 返回系统托盘图标在屏幕上的边界矩形。
    fn get_tray_icon_bounds(&self) -> Option<Bounds<Pixels>> {
        None
    }
    /// 注册托盘图标事件回调（单击、双击、右键等）。
    fn on_tray_icon_event(&self, _callback: Box<dyn FnMut(TrayIconEvent)>) {}
    /// 注册托盘菜单项操作回调。
    fn on_tray_menu_action(&self, _callback: Box<dyn FnMut(SharedString)>) {}

    /// 设置是否在所有窗口关闭后保持应用运行（仅显示托盘图标）。
    fn set_keep_alive_without_windows(&self, _keep_alive: bool) {}

    /// 注册全局系统快捷键，`id` 用于标识快捷键，`keystroke` 定义按键组合。
    fn register_global_hotkey(&self, _id: u32, _keystroke: &Keystroke) -> Result<()> {
        Ok(())
    }
    /// 取消注册全局系统快捷键。
    fn unregister_global_hotkey(&self, _id: u32) {}
    /// 注册全局快捷键触发回调，`id` 对应注册时的标识。
    fn on_global_hotkey(&self, _callback: Box<dyn FnMut(u32)>) {}

    /// 显示系统通知，返回 `Ok(())` 表示通知已发送。
    fn show_notification(&self, _title: &str, _body: &str) -> Result<()> {
        Ok(())
    }

    /// 设置开机自启动，`app_id` 为应用唯一标识。
    fn set_auto_launch(&self, _app_id: &str, _enabled: bool) -> Result<()> {
        Ok(())
    }
    /// 查询开机自启动是否已启用。
    fn is_auto_launch_enabled(&self, _app_id: &str) -> bool {
        false
    }

    /// 返回当前系统中获得焦点的窗口信息（标题、进程名等）。
    fn focused_window_info(&self) -> Option<FocusedWindowInfo> {
        None
    }

    /// 返回辅助功能（Accessibility）权限状态。
    fn accessibility_status(&self) -> PermissionStatus {
        PermissionStatus::Unavailable
    }
    /// 请求辅助功能权限（macOS 需要用户授权）。
    fn request_accessibility_permission(&self) {}

    /// 返回麦克风权限状态。
    fn microphone_status(&self) -> PermissionStatus {
        PermissionStatus::Unavailable
    }
    /// 请求麦克风权限，`callback` 收到授权结果。
    fn request_microphone_permission(&self, _callback: Box<dyn FnOnce(bool)>) {}

    /// 注册系统电源事件回调（电池状态变化、电源插拔等）。
    fn on_system_power_event(&self, _callback: Box<dyn FnMut(SystemPowerEvent)>) {}

    /// 注册系统唤醒时的回调函数。
    fn on_system_wake(&self, _callback: Box<dyn FnMut()>) {}

    /// 启动电源节省阻止器（阻止系统进入睡眠），返回阻止器 ID。
    fn start_power_save_blocker(&self, _kind: PowerSaveBlockerKind) -> Option<u32> {
        None
    }
    /// 停止指定的电源节省阻止器。
    fn stop_power_save_blocker(&self, _id: u32) {}

    /// 返回系统空闲时间（自上次用户输入以来的时长）。
    fn system_idle_time(&self) -> Option<Duration> {
        None
    }

    /// 返回当前网络连接状态。
    fn network_status(&self) -> NetworkStatus {
        NetworkStatus::Connected
    }
    /// 注册网络状态变化回调（在线/离线/连接变化）。
    fn on_network_status_change(&self, _callback: Box<dyn FnMut(NetworkStatus)>) {}

    /// 注册媒体键事件回调（播放/暂停/音量等）。
    fn on_media_key_event(&self, _callback: Box<dyn FnMut(MediaKeyEvent)>) {}

    /// 请求用户注意力（macOS Dock 图标弹跳、Windows 任务栏闪烁）。
    fn request_user_attention(&self, _attention_type: AttentionType) {}
    /// 取消用户注意力请求。
    fn cancel_user_attention(&self) {}

    /// 设置 macOS Dock 标签徽章文本（如未读消息数）。
    fn set_dock_badge(&self, _label: Option<&str>) {}

    /// 在指定位置显示右键上下文菜单。
    fn show_context_menu(
        &self,
        _position: Point<Pixels>,
        _items: Vec<TrayMenuItem>,
        _callback: Box<dyn FnMut(SharedString)>,
    ) {
    }

    /// 显示系统原生对话框（如确认、警告等），返回用户选择的按钮索引。
    fn show_dialog(&self, _options: DialogOptions) -> oneshot::Receiver<usize> {
        let (tx, rx) = oneshot::channel();
        let _ = tx.send(0);
        rx
    }

    /// 返回操作系统信息（名称、版本号）。
    fn os_info(&self) -> OsInfo {
        OsInfo {
            name: String::new(),
            version: String::new(),
        }
    }

    /// 返回生物识别（指纹/面容 ID）硬件状态。
    fn biometric_status(&self) -> BiometricStatus {
        BiometricStatus::Unavailable
    }
    /// 触发生物识别认证，`reason` 为提示文本，`callback` 收到认证结果。
    fn authenticate_biometric(&self, _reason: &str, _callback: Box<dyn FnOnce(bool)>) {}
}

/// 平台显示器句柄，代表一个物理显示器或笔记本屏幕。
pub trait PlatformDisplay: Debug {
    /// 获取显示器 ID。
    fn id(&self) -> DisplayId;

    /// 返回显示器的持久化唯一标识符，可在系统重启后继续使用。
    fn uuid(&self) -> Result<Uuid>;

    /// 获取显示器的边界区域（包含任务栏/Dock 区域）。
    fn bounds(&self) -> Bounds<Pixels>;

    /// 获取显示器的可见边界区域（排除任务栏/Dock 区域）。
    /// 这是可放置窗口且不会被遮挡的可用区域。
    /// 未覆盖时默认返回完整显示器边界。
    fn visible_bounds(&self) -> Bounds<Pixels> {
        self.bounds()
    }

    /// 获取显示器的默认窗口放置区域。
    fn default_bounds(&self) -> Bounds<Pixels> {
        let bounds = self.bounds();
        let center = bounds.center();
        let clipped_window_size = DEFAULT_WINDOW_SIZE.min(&bounds.size);

        let offset = clipped_window_size / 2.0;
        let origin = point(center.x - offset.width, center.y - offset.height);
        Bounds::new(origin, clipped_window_size)
    }
}

/// 系统热状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalState {
    /// 系统无热限制
    Nominal,
    /// 系统轻微受限，应减少非必要工作
    Fair,
    /// 系统中度受限，应减少 CPU/GPU 密集型工作
    Serious,
    /// 系统严重受限，应最小化所有资源使用
    Critical,
}

/// 屏幕捕获源的元数据
#[derive(Clone)]
pub struct SourceMetadata {
    /// 屏幕的不透明标识符。
    pub id: u64,
    /// 人类可读的源标签。
    pub label: Option<SharedString>,
    /// 该源是否为主显示器。
    pub is_main: Option<bool>,
    /// 该源的视频分辨率。
    pub resolution: Size<DevicePixels>,
}

/// 可被捕获的屏幕视频内容源。
pub trait ScreenCaptureSource {
    /// 返回该源的元数据。
    fn metadata(&self) -> Result<SourceMetadata>;

    /// 开始从该源捕获视频，每帧调用给定的回调函数。
    fn stream(
        &self,
        foreground_executor: &ForegroundExecutor,
        frame_callback: Box<dyn Fn(ScreenCaptureFrame) + Send>,
    ) -> oneshot::Receiver<Result<Box<dyn ScreenCaptureStream>>>;
}

/// 从屏幕捕获的视频流。
pub trait ScreenCaptureStream {
    /// 返回该源的元数据。
    fn metadata(&self) -> Result<SourceMetadata>;
}

/// 从屏幕捕获的视频帧。
pub struct ScreenCaptureFrame(pub PlatformScreenCaptureFrame);

#[cfg(all(
    any(target_os = "windows", target_os = "linux"),
    feature = "screen-capture"
))]
impl ScreenCaptureFrame {
    /// 获取帧宽度（像素）
    pub fn width(&self) -> u32 {
        match &self.0 {
            scap::frame::Frame::YUVFrame(f) => f.width as u32,
            scap::frame::Frame::RGB(f) => f.width as u32,
            scap::frame::Frame::RGBx(f) => f.width as u32,
            scap::frame::Frame::XBGR(f) => f.width as u32,
            scap::frame::Frame::BGRx(f) => f.width as u32,
            scap::frame::Frame::BGR0(f) => f.width as u32,
            scap::frame::Frame::BGRA(f) => f.width as u32,
        }
    }

    /// 获取帧高度（像素）
    pub fn height(&self) -> u32 {
        match &self.0 {
            scap::frame::Frame::YUVFrame(f) => f.height as u32,
            scap::frame::Frame::RGB(f) => f.height as u32,
            scap::frame::Frame::RGBx(f) => f.height as u32,
            scap::frame::Frame::XBGR(f) => f.height as u32,
            scap::frame::Frame::BGRx(f) => f.height as u32,
            scap::frame::Frame::BGR0(f) => f.height as u32,
            scap::frame::Frame::BGRA(f) => f.height as u32,
        }
    }

    /// 将帧转换为 RGBA 图像
    ///
    /// 支持所有 scap 输出格式：BGRA、BGR0、BGRx、XBGR、RGB、RGBx、YUV(NV12)。
    /// YUV 格式使用 BT.601 标准进行色彩空间转换。
    pub fn to_rgba(&self) -> Option<image::RgbaImage> {
        match &self.0 {
            scap::frame::Frame::BGRA(f) => {
                let w = f.width as u32;
                let h = f.height as u32;
                let mut rgba = Vec::with_capacity((w * h * 4) as usize);
                for chunk in f.data.chunks_exact(4) {
                    rgba.push(chunk[2]); // R
                    rgba.push(chunk[1]); // G
                    rgba.push(chunk[0]); // B
                    rgba.push(chunk[3]); // A
                }
                image::RgbaImage::from_raw(w, h, rgba)
            }
            scap::frame::Frame::BGR0(f) => {
                let w = f.width as u32;
                let h = f.height as u32;
                let mut rgba = Vec::with_capacity((w * h * 4) as usize);
                for chunk in f.data.chunks_exact(4) {
                    rgba.push(chunk[2]); // R
                    rgba.push(chunk[1]); // G
                    rgba.push(chunk[0]); // B
                    rgba.push(255); // A (不透明)
                }
                image::RgbaImage::from_raw(w, h, rgba)
            }
            scap::frame::Frame::BGRx(f) => {
                let w = f.width as u32;
                let h = f.height as u32;
                let mut rgba = Vec::with_capacity((w * h * 4) as usize);
                for chunk in f.data.chunks_exact(4) {
                    rgba.push(chunk[2]); // R
                    rgba.push(chunk[1]); // G
                    rgba.push(chunk[0]); // B
                    rgba.push(255); // A (不透明)
                }
                image::RgbaImage::from_raw(w, h, rgba)
            }
            scap::frame::Frame::XBGR(f) => {
                let w = f.width as u32;
                let h = f.height as u32;
                let mut rgba = Vec::with_capacity((w * h * 4) as usize);
                for chunk in f.data.chunks_exact(4) {
                    rgba.push(chunk[3]); // R
                    rgba.push(chunk[2]); // G
                    rgba.push(chunk[1]); // B
                    rgba.push(255); // A (不透明)
                }
                image::RgbaImage::from_raw(w, h, rgba)
            }
            scap::frame::Frame::RGB(f) => {
                let w = f.width as u32;
                let h = f.height as u32;
                let mut rgba = Vec::with_capacity((w * h * 4) as usize);
                for chunk in f.data.chunks_exact(3) {
                    rgba.push(chunk[0]); // R
                    rgba.push(chunk[1]); // G
                    rgba.push(chunk[2]); // B
                    rgba.push(255); // A (不透明)
                }
                image::RgbaImage::from_raw(w, h, rgba)
            }
            scap::frame::Frame::RGBx(f) => {
                let w = f.width as u32;
                let h = f.height as u32;
                let mut rgba = Vec::with_capacity((w * h * 4) as usize);
                for chunk in f.data.chunks_exact(4) {
                    rgba.push(chunk[0]); // R
                    rgba.push(chunk[1]); // G
                    rgba.push(chunk[2]); // B
                    rgba.push(255); // A (不透明)
                }
                image::RgbaImage::from_raw(w, h, rgba)
            }
            scap::frame::Frame::YUVFrame(f) => {
                let w = f.width as u32;
                let h = f.height as u32;
                let mut rgba = Vec::with_capacity((w * h * 4) as usize);

                // NV12 格式：Y 平面 + 交错 UV 平面
                let y_plane = &f.luminance_bytes;
                let uv_plane = &f.chrominance_bytes;
                let y_stride = f.luminance_stride as usize;
                let uv_stride = f.chrominance_stride as usize;

                for row in 0..h as usize {
                    for col in 0..w as usize {
                        // 读取 Y 值（考虑步长）
                        let y_idx = row * y_stride + col;
                        let y = if y_idx < y_plane.len() {
                            y_plane[y_idx] as i32
                        } else {
                            0
                        };

                        // 读取 U、V 值（UV 交错，每两个像素共享）
                        let uv_row = row / 2;
                        let uv_col = (col / 2) * 2;
                        let uv_idx = uv_row * uv_stride + uv_col;
                        let u = if uv_idx < uv_plane.len() {
                            uv_plane[uv_idx] as i32
                        } else {
                            128
                        };
                        let v = if uv_idx + 1 < uv_plane.len() {
                            uv_plane[uv_idx + 1] as i32
                        } else {
                            128
                        };

                        // BT.601 YUV → RGB 转换
                        let c = 298 * (y - 16);
                        let r = ((c + 409 * (v - 128) + 128) >> 8).clamp(0, 255) as u8;
                        let g = ((c - 100 * (u - 128) - 208 * (v - 128) + 128) >> 8).clamp(0, 255)
                            as u8;
                        let b = ((c + 516 * (u - 128) + 128) >> 8).clamp(0, 255) as u8;

                        rgba.push(r);
                        rgba.push(g);
                        rgba.push(b);
                        rgba.push(255); // A (不透明)
                    }
                }
                image::RgbaImage::from_raw(w, h, rgba)
            }
        }
    }
}

#[cfg(all(target_os = "macos", feature = "screen-capture"))]
impl ScreenCaptureFrame {
    /// 获取帧宽度（像素）
    pub fn width(&self) -> u32 {
        0
    }

    /// 获取帧高度（像素）
    pub fn height(&self) -> u32 {
        0
    }

    /// 将帧转换为 RGBA 图像
    pub fn to_rgba(&self) -> Option<image::RgbaImage> {
        None
    }
}

/// 硬件显示器的不透明标识符
#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub struct DisplayId(pub(crate) u64);

impl DisplayId {
    /// 从原始平台显示器标识符创建新的 `DisplayId`。
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

impl From<u64> for DisplayId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl From<DisplayId> for u64 {
    fn from(id: DisplayId) -> Self {
        id.0
    }
}

impl Debug for DisplayId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DisplayId({})", self.0)
    }
}

/// 窗口调整大小的边缘方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    /// 上边缘
    Top,
    /// 右上角
    TopRight,
    /// 右边缘
    Right,
    /// 右下角
    BottomRight,
    /// 下边缘
    Bottom,
    /// 左下角
    BottomLeft,
    /// 左边缘
    Left,
    /// 左上角
    TopLeft,
}

/// 描述窗口外观类型的枚举
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default)]
pub enum WindowDecorations {
    #[default]
    /// 服务端装饰
    Server,
    /// 客户端装饰
    Client,
}

/// 描述窗口当前装饰配置的类型
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default)]
pub enum Decorations {
    /// 窗口配置为使用服务端装饰
    #[default]
    Server,
    /// 窗口配置为使用客户端装饰
    Client {
        /// 边缘平铺状态
        tiling: Tiling,
    },
}

/// 平台支持的窗口控件
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct WindowControls {
    /// 该平台是否支持全屏
    pub fullscreen: bool,
    /// 该平台是否支持最大化
    pub maximize: bool,
    /// 该平台是否支持最小化
    pub minimize: bool,
    /// 该平台是否支持窗口菜单
    pub window_menu: bool,
}

impl Default for WindowControls {
    fn default() -> Self {
        // 默认假设所有功能都可用，除非另有说明
        Self {
            fullscreen: true,
            maximize: true,
            minimize: true,
            window_menu: true,
        }
    }
}

/// [`WindowButtonLayout`] 中使用的窗口控制按钮类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowButton {
    /// 最小化按钮
    Minimize,
    /// 最大化按钮
    Maximize,
    /// 关闭按钮
    Close,
}

impl WindowButton {
    /// 返回该按钮渲染时使用的稳定元素 ID。
    pub fn id(&self) -> &'static str {
        match self {
            WindowButton::Minimize => "minimize",
            WindowButton::Maximize => "maximize",
            WindowButton::Close => "close",
        }
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn index(&self) -> usize {
        match self {
            WindowButton::Minimize => 0,
            WindowButton::Maximize => 1,
            WindowButton::Close => 2,
        }
    }
}

/// 标题栏每侧最大的 [`WindowButton`] 数量。
pub const MAX_BUTTONS_PER_SIDE: usize = 3;

/// 描述标题栏每侧出现的 [`WindowButton`]。
///
/// 在 Linux 上，此配置从桌面环境的配置中读取
/// （例如 GNOME 的 `gtk-decoration-layout` gsetting），通过 [`WindowButtonLayout::parse`] 解析。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowButtonLayout {
    /// 标题栏左侧的按钮。
    pub left: [Option<WindowButton>; MAX_BUTTONS_PER_SIDE],
    /// 标题栏右侧的按钮。
    pub right: [Option<WindowButton>; MAX_BUTTONS_PER_SIDE],
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
impl WindowButtonLayout {
    /// 返回 Zed 内置的 Linux 标题栏回退按钮布局。
    pub fn linux_default() -> Self {
        Self {
            left: [None; MAX_BUTTONS_PER_SIDE],
            right: [
                Some(WindowButton::Minimize),
                Some(WindowButton::Maximize),
                Some(WindowButton::Close),
            ],
        }
    }

    /// 解析 GNOME 风格的 `button-layout` 字符串（如 `"close,minimize:maximize"`）。
    pub fn parse(layout_string: &str) -> Result<Self> {
        fn parse_side(
            s: &str,
            seen_buttons: &mut [bool; MAX_BUTTONS_PER_SIDE],
            unrecognized: &mut Vec<String>,
        ) -> [Option<WindowButton>; MAX_BUTTONS_PER_SIDE] {
            let mut result = [None; MAX_BUTTONS_PER_SIDE];
            let mut i = 0;
            for name in s.split(',') {
                let trimmed = name.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let button = match trimmed {
                    "minimize" => Some(WindowButton::Minimize),
                    "maximize" => Some(WindowButton::Maximize),
                    "close" => Some(WindowButton::Close),
                    other => {
                        unrecognized.push(other.to_string());
                        None
                    }
                };
                if let Some(button) = button {
                    if seen_buttons[button.index()] {
                        continue;
                    }
                    if let Some(slot) = result.get_mut(i) {
                        *slot = Some(button);
                        seen_buttons[button.index()] = true;
                        i += 1;
                    }
                }
            }
            result
        }

        let (left_str, right_str) = layout_string.split_once(':').unwrap_or(("", layout_string));
        let mut unrecognized = Vec::new();
        let mut seen_buttons = [false; MAX_BUTTONS_PER_SIDE];
        let layout = Self {
            left: parse_side(left_str, &mut seen_buttons, &mut unrecognized),
            right: parse_side(right_str, &mut seen_buttons, &mut unrecognized),
        };

        if !unrecognized.is_empty()
            && layout.left.iter().all(Option::is_none)
            && layout.right.iter().all(Option::is_none)
        {
            bail!(
                "button layout string {:?} contains no valid buttons (unrecognized: {})",
                layout_string,
                unrecognized.join(", ")
            );
        }

        Ok(layout)
    }

    /// 将布局格式化为 GNOME 风格的 `button-layout` 字符串。
    #[cfg(test)]
    pub fn format(&self) -> String {
        fn format_side(buttons: &[Option<WindowButton>; MAX_BUTTONS_PER_SIDE]) -> String {
            buttons
                .iter()
                .flatten()
                .map(|button| match button {
                    WindowButton::Minimize => "minimize",
                    WindowButton::Maximize => "maximize",
                    WindowButton::Close => "close",
                })
                .collect::<Vec<_>>()
                .join(",")
        }

        format!("{}:{}", format_side(&self.left), format_side(&self.right))
    }
}

/// 描述窗口各边当前的平铺状态
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default)]
pub struct Tiling {
    /// 上边缘是否平铺
    pub top: bool,
    /// 左边缘是否平铺
    pub left: bool,
    /// 右边缘是否平铺
    pub right: bool,
    /// 下边缘是否平铺
    pub bottom: bool,
}

impl Tiling {
    /// 创建一个所有边都平铺的 [`Tiling`] 实例。
    pub fn tiled() -> Self {
        Self {
            top: true,
            left: true,
            right: true,
            bottom: true,
        }
    }

    /// 是否有任何边缘处于平铺状态
    pub fn is_tiled(&self) -> bool {
        self.top || self.left || self.right || self.bottom
    }
}

/// 辅助功能适配器的回调函数。
pub struct A11yCallbacks {
    /// 适配器被激活时调用（屏幕阅读器连接）。
    pub activation: Box<dyn Fn() -> Option<accesskit::TreeUpdate> + Send + 'static>,
    /// 屏幕阅读器请求操作时调用。
    pub action: Box<dyn Fn(accesskit::ActionRequest) + Send + 'static>,
    /// 适配器被停用时调用（屏幕阅读器断开连接）。
    pub deactivation: Box<dyn Fn() + Send + 'static>,
}

/// 帧请求选项，控制平台何时请求重绘。
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct RequestFrameOptions {
    /// 是否需要呈现帧（提交到屏幕）。
    pub require_presentation: bool,
    /// 为 `true` 时强制刷新所有渲染状态。
    pub force_render: bool,
}

/// 平台原生窗口抽象，由各平台 crate 实现，提供窗口管理、输入、渲染等能力。
///
/// 通过 [`Platform::open_window`] 创建实例，通过 [`PlatformWindow`] trait 与窗口交互。
pub trait PlatformWindow: HasWindowHandle + HasDisplayHandle {
    /// 返回窗口在屏幕上的边界矩形（包含标题栏和边框）。
    fn bounds(&self) -> Bounds<Pixels>;
    /// 窗口是否处于最大化状态。
    fn is_maximized(&self) -> bool;
    /// 返回窗口当前边界状态（窗口化/最大化/全屏）。
    fn window_bounds(&self) -> WindowBounds;
    /// 返回窗口内容区域的尺寸（不含标题栏和边框）。
    fn content_size(&self) -> Size<Pixels>;
    /// 调整窗口内容区域的尺寸。
    fn resize(&mut self, size: Size<Pixels>);
    /// 返回窗口的显示缩放因子（如 1.0、1.5、2.0）。
    fn scale_factor(&self) -> f32;
    /// 返回窗口当前的外观模式（亮色/暗色）。
    fn appearance(&self) -> WindowAppearance;
    /// 返回窗口所在的显示器。
    fn display(&self) -> Option<Rc<dyn PlatformDisplay>>;
    /// 返回鼠标光标在窗口内的位置。
    fn mouse_position(&self) -> Point<Pixels>;
    /// 返回当前修饰键状态（Ctrl/Shift/Alt/Command）。
    fn modifiers(&self) -> Modifiers;
    /// 返回 Caps Lock 锁定状态。
    fn capslock(&self) -> Capslock;
    /// 设置输入处理器，用于处理 IME（输入法）组合文本。
    fn set_input_handler(&mut self, input_handler: PlatformInputHandler);
    /// 取出并返回当前输入处理器（take 语义）。
    fn take_input_handler(&mut self) -> Option<PlatformInputHandler>;
    /// 显示模态提示对话框，返回用户选择的按钮索引。
    fn prompt(
        &self,
        level: PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>>;
    /// 将窗口置于前台并获得焦点。
    fn activate(&self);
    /// 窗口是否当前处于活动（获得焦点）状态。
    fn is_active(&self) -> bool;
    /// 鼠标光标是否悬停在窗口上方。
    fn is_hovered(&self) -> bool;
    /// 返回窗口背景外观（透明/不透明/毛玻璃）。
    fn background_appearance(&self) -> WindowBackgroundAppearance;
    /// 设置窗口标题文本。
    fn set_title(&mut self, title: &str);
    /// 设置窗口背景外观模式。
    fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance);
    /// 最小化窗口。
    fn minimize(&self);
    /// 最大化窗口（macOS 下为缩放/zoom）。
    fn zoom(&self);
    /// 切换全屏状态。
    fn toggle_fullscreen(&self);
    /// 窗口是否处于全屏状态。
    fn is_fullscreen(&self) -> bool;
    /// 注册帧请求回调，平台在需要重绘时调用。
    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>);
    /// 注册输入事件回调，处理键盘、鼠标、触摸等事件。
    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>);
    /// 注册窗口活动状态变化回调（获得/失去焦点时触发）。
    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>);
    /// 注册鼠标悬停状态变化回调（进入/离开窗口时触发）。
    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>);
    /// 注册窗口尺寸变化回调，参数为新尺寸和缩放因子。
    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>);
    /// 注册窗口位置变化回调。
    fn on_moved(&self, callback: Box<dyn FnMut()>);
    /// 注册窗口关闭请求回调，返回 `false` 可阻止关闭。
    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>);
    /// 注册窗口控件区域命中测试回调（用于自定义标题栏拖拽区域）。
    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>);
    /// 注册窗口关闭回调，窗口关闭时调用。
    fn on_close(&self, callback: Box<dyn FnOnce()>);
    /// 注册窗口外观变化回调（系统切换亮色/暗色主题时触发）。
    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>);
    /// 注册窗口按钮布局变化回调（macOS 全屏按钮位置变化等）。
    fn on_button_layout_changed(&self, _callback: Box<dyn FnMut()>) {}
    /// 将渲染场景提交到窗口进行绘制。
    fn draw(&self, scene: &Scene);
    /// 通知平台当前帧已完成渲染。
    fn completed_frame(&self) {}
    /// 返回精灵图集（用于图标、表情符号等位图渲染）。
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas>;
    /// 是否支持亚像素渲染（ClearType 文本渲染）。
    fn is_subpixel_rendering_supported(&self) -> bool;

    /// 该平台窗口是否支持 Web DOM 后端。
    ///
    /// 返回 `true` 时，核心会在每帧构建 DOM 树并通过 [`Self::dom_tree_update`] 交付，
    /// 桌面平台默认 `false` 以保持零开销。
    #[cfg(feature = "dom-backend")]
    fn supports_dom(&self) -> bool {
        false
    }

    /// 交付当前帧的 DOM 树（每帧一次，仅当 `supports_dom()` 为真）。
    ///
    /// DOM 层渲染在 canvas 之上的覆盖层中（v1 接受双重绘制），
    /// 平台侧负责增量对账（见 `rgpui-dom` crate 的 reconcile）。
    #[cfg(feature = "dom-backend")]
    fn dom_tree_update(&self, _tree: &crate::dom::DomTree) {}

    /// 注册 Web DOM 事件委托回调。
    ///
    /// 点击 DOM 覆盖层上的元素时，平台按 `data-gpui-id` 反查 DOM key 链并回调，
    /// 由核心按 key 链直接命中 hitbox（绕过坐标 hit-test）。桌面平台默认空实现。
    #[cfg(feature = "dom-backend")]
    fn on_dom_event(
        &self,
        _callback: Box<dyn FnMut(Vec<crate::DomNodeKey>, PlatformInput) -> DispatchEventResult>,
    ) {
    }

    /// 由 DOM 后端在可滚动容器发生浏览器原生滚动（`scroll` 事件）后回调，
    /// 参数为事件链与滚动视口的 `scrollLeft`/`scrollTop`（向下/向右为正）。
    /// 默认空实现：仅 Web DOM 后端使用。
    #[cfg(feature = "dom-backend")]
    fn on_dom_scroll(&self, _callback: Box<dyn FnMut(Vec<crate::DomNodeKey>, f64, f64)>) {}

    // macOS specific methods
    /// 返回窗口标题文本。
    fn get_title(&self) -> String {
        String::new()
    }
    /// 返回当前窗口的标签页组（macOS 标签页功能）。
    fn tabbed_windows(&self) -> Option<Vec<SystemWindowTab>> {
        None
    }
    /// 标签页栏是否可见。
    fn tab_bar_visible(&self) -> bool {
        false
    }
    /// 标记文档是否已编辑（macOS 窗口标题栏红点标记）。
    fn set_edited(&mut self, _edited: bool) {}
    /// 设置文档路径（macOS 标题栏显示文件名）。
    fn set_document_path(&self, _path: Option<&std::path::Path>) {}
    /// 设置 macOS 红绿灯按钮位置。
    #[cfg(target_os = "macos")]
    fn set_traffic_light_position(&self, _position: Point<Pixels>) {}
    /// 显示系统字符面板（emoji、特殊符号）。
    fn show_character_palette(&self) {}
    /// 处理标题栏双击事件（可自定义行为，如最大化/缩放）。
    fn titlebar_double_click(&self, _is_resizable: bool, _is_minimizable: bool) {}
    /// 注册"将标签页移至新窗口"回调。
    fn on_move_tab_to_new_window(&self, _callback: Box<dyn FnMut()>) {}
    /// 注册"合并所有窗口"回调。
    fn on_merge_all_windows(&self, _callback: Box<dyn FnMut()>) {}
    /// 注册"切换到上一个标签页"回调。
    fn on_select_previous_tab(&self, _callback: Box<dyn FnMut()>) {}
    /// 注册"切换到下一个标签页"回调。
    fn on_select_next_tab(&self, _callback: Box<dyn FnMut()>) {}
    /// 注册"切换标签页栏可见性"回调。
    fn on_toggle_tab_bar(&self, _callback: Box<dyn FnMut()>) {}
    /// 合并所有窗口为一个窗口的标签页。
    fn merge_all_windows(&self) {}
    /// 将当前标签页移至新窗口。
    fn move_tab_to_new_window(&self) {}
    /// 切换标签页总览视图（macOS Exposé 风格）。
    fn toggle_window_tab_overview(&self) {}
    /// 设置窗口的 tabbing identifier（控制标签页分组）。
    fn set_tabbing_identifier(&self, _identifier: Option<String>) {}

    /// 返回窗口的原始 HWND 句柄（仅 Windows）。
    #[cfg(target_os = "windows")]
    fn get_raw_handle(&self) -> windows::Win32::Foundation::HWND;

    /// 返回窗口的内部边界（Linux 下包含 CSD 装饰区域）。
    fn inner_window_bounds(&self) -> WindowBounds {
        self.window_bounds()
    }
    /// 请求设置窗口装饰模式（客户端装饰/服务端装饰）。
    fn request_decorations(&self, _decorations: WindowDecorations) {}
    /// 在指定位置显示窗口系统菜单（Linux 右键标题栏）。
    fn show_window_menu(&self, _position: Point<Pixels>) {}
    /// 启动窗口拖拽移动（Linux CSD 模式下从自定义标题栏触发）。
    fn start_window_move(&self) {}
    /// 启动窗口边缘调整大小（Linux CSD 模式下从自定义边框触发）。
    fn start_window_resize(&self, _edge: ResizeEdge) {}
    /// 设置窗口输入区域（指定哪些区域接收鼠标事件，其余区域穿透）。
    fn set_input_region(&self, _region: Option<&[Bounds<Pixels>]>) {}
    /// 返回当前窗口装饰类型（客户端/服务端/无装饰）。
    fn window_decorations(&self) -> Decorations {
        Decorations::Server
    }
    /// 设置 Wayland app_id（用于窗口标识和桌面集成）。
    fn set_app_id(&mut self, _app_id: &str) {}
    /// 映射窗口（X11 下将窗口显示到屏幕）。
    fn map_window(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    /// 返回窗口控件信息（最小化/最大化/关闭按钮位置）。
    fn window_controls(&self) -> WindowControls {
        WindowControls::default()
    }
    /// 设置客户端区域的内边距（Wayland layer-shell 排除区域）。
    fn set_client_inset(&self, _inset: Pixels) {}
    /// 返回 GPU 硬件信息（设备名称、显存等）。
    fn gpu_specs(&self) -> Option<GpuSpecs>;

    /// 更新输入法（IME）候选框的位置。
    fn update_ime_position(&self, _bounds: Bounds<Pixels>);

    /// 播放系统提示音。
    fn play_system_bell(&self) {}

    /// 初始化辅助功能适配器，注册辅助功能回调。
    fn a11y_init(&self, _callbacks: A11yCallbacks) {}

    /// 向辅助功能适配器提供无障碍树更新数据（accesskit）。
    fn a11y_tree_update(&self, _tree_update: accesskit::TreeUpdate) {}

    /// 通知辅助功能适配器窗口边界已更新。
    fn a11y_update_window_bounds(&self) {}

    /// 使用指定场景渲染到 RGBA 图像纹理（仅测试用途）。
    #[cfg(any(test, feature = "test-support"))]
    fn as_test(&mut self) -> Option<&mut TestWindow> {
        None
    }

    /// 将给定场景渲染到纹理并返回 RGBA 像素数据（仅测试用途）。
    /// 不会将帧呈现到屏幕，用于视觉测试。
    #[cfg(any(test, feature = "test-support"))]
    fn render_to_image(&self, _scene: &Scene) -> Result<RgbaImage> {
        anyhow::bail!("render_to_image not implemented for this platform")
    }

    /// 设置 Wayland layer-shell 独占区域大小（像素）。
    fn set_exclusive_zone(&self, _zone: Pixels) {}
    /// 设置 Wayland layer-shell 独占边缘（顶部/底部/左侧/右侧）。
    #[cfg(all(target_os = "linux", feature = "wayland"))]
    fn set_exclusive_edge(&self, _edge: layer_shell::Anchor) {}

    /// 请求用户注意力（任务栏闪烁/弹跳，提示用户查看窗口）。
    fn request_attention(&self) {}

    /// 设置窗口在屏幕上的位置。
    fn set_position(&mut self, _position: Point<Pixels>) {}

    /// 隐藏窗口（从任务栏移除，托盘模式下使用）。
    fn hide(&self) {}

    /// 设置鼠标事件是否穿透窗口（桌面宠物/覆盖层场景）。
    fn set_mouse_passthrough(&self, _passthrough: bool) {}

    /// 返回 Windows 窗口扩展样式（WS_EX_* 标志位）。
    fn window_extended_style(&self) -> u32 {
        0
    }
    /// 设置 Windows 窗口扩展样式。
    fn set_window_extended_style(&self, _style: u32) {}

    /// 设置标题栏是否可见（控制自定义标题栏/原生标题栏切换）。
    fn set_titlebar_visible(&self, _visible: bool) {}

    /// 设置输入框的语义内容类型（如 `password`、`email`），
    /// 供系统输入法/自动填充识别。macOS 通过 `NSTextContent` 实现，其他平台为空操作。
    fn set_text_content_type(&self, _content_type: Option<&'static str>) {}
}

/// 无头窗口渲染器，可生成真实渲染输出。
#[cfg(any(test, feature = "test-support"))]
pub trait PlatformHeadlessRenderer {
    /// 渲染场景并作为 RGBA 图像返回结果
    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> Result<RgbaImage>;

    /// 渲染场景到离屏目标，不读取结果
    ///
    /// 这是绘制到真实窗口的无头等效操作：它执行与绘制到真实窗口相同的 CPU 端场景编码和 GPU 提交，但不阻塞 GPU 完成或复制像素回来
    fn render_scene(&mut self, scene: &Scene, size: Size<DevicePixels>) -> Result<()>;

    /// 返回此渲染器使用的精灵图集
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas>;
}

/// 带元数据的可运行任务类型别名。
/// 之前是单变体枚举，现在简化为直接类型别名。
#[doc(hidden)]
pub type RunnableVariant = Runnable<RunnableMeta>;

#[doc(hidden)]
pub type TimerResolutionGuard = rgpui_util::Deferred<Box<dyn FnOnce() + Send>>;

#[doc(hidden)]
pub enum TasksIncluded {
    OnlyCompleted,
    CompletedAndRunning,
}

/// 此类型公开是为了测试宏可以生成和使用它，但不应视为公共 API 的一部分。
#[doc(hidden)]
pub trait PlatformDispatcher: Send + Sync {
    fn is_main_thread(&self) -> bool;
    fn dispatch(&self, runnable: RunnableVariant, priority: Priority);
    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority);
    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant);

    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>);

    fn now(&self) -> Instant {
        Instant::now()
    }

    fn increase_timer_resolution(&self) -> TimerResolutionGuard {
        rgpui_util::defer(Box::new(|| {}))
    }

    #[cfg(any(test, feature = "test-support"))]
    fn as_test(&self) -> Option<&TestDispatcher> {
        None
    }

    // 此 cfg 必须与 `threaded_dispatcher` 模块的匹配，该模块在编译时实现此方法
    #[cfg(all(
        any(test, feature = "test-support"),
        any(target_os = "windows", target_os = "linux", target_family = "wasm")
    ))]
    fn as_threaded(&self) -> Option<&ThreadedDispatcher> {
        None
    }
}

/// 平台文本系统抽象 — 提供字体加载、字形光栅化、文本排版等能力。各平台需实现此 trait。
pub trait PlatformTextSystem: Send + Sync {
    /// 加载指定的字体数据（TTF/OTF 字节流）。
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()>;
    /// 获取所有可用的字体名称。
    fn all_font_names(&self) -> Vec<String>;
    /// 根据字体描述符获取字体 ID。
    fn font_id(&self, descriptor: &Font) -> Result<FontId>;
    /// 获取字体的度量信息。
    fn font_metrics(&self, font_id: FontId) -> FontMetrics;
    /// 获取字形的排版边界。
    fn typographic_bounds(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Bounds<f32>>;
    /// 获取字形的前进宽度。
    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>>;
    /// 获取字符对应的字形 ID。
    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId>;
    /// 获取字形的光栅化边界。
    fn glyph_raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>>;
    /// 光栅化字形。
    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)>;
    /// 使用给定的字体运行（Font Run）排版一行文本。
    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout;
    /// 返回给定字体和大小的推荐文本渲染模式。
    fn recommended_rendering_mode(&self, _font_id: FontId, _font_size: Pixels)
    -> TextRenderingMode;
    /// 返回以给定颜色绘制字形时使用的膨胀级别。
    fn glyph_dilation_for_color(&self, _color: Hsla) -> u8 {
        0
    }
}

/// 空操作文本系统实现，所有方法返回默认值。用于测试或无文本系统需求的平台。
pub struct NoopTextSystem;

impl NoopTextSystem {
    /// 创建一个新的空操作文本系统实例。
    pub fn new() -> Self {
        Self
    }
}

impl PlatformTextSystem for NoopTextSystem {
    fn add_fonts(&self, _fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        Ok(())
    }

    fn all_font_names(&self) -> Vec<String> {
        Vec::new()
    }

    fn font_id(&self, _descriptor: &Font) -> Result<FontId> {
        Ok(FontId(1))
    }

    fn font_metrics(&self, _font_id: FontId) -> FontMetrics {
        FontMetrics {
            units_per_em: 1000,
            ascent: 1025.0,
            descent: -275.0,
            line_gap: 0.0,
            underline_position: -95.0,
            underline_thickness: 60.0,
            cap_height: 698.0,
            x_height: 516.0,
            bounding_box: Bounds {
                origin: Point {
                    x: -260.0,
                    y: -245.0,
                },
                size: Size {
                    width: 1501.0,
                    height: 1364.0,
                },
            },
        }
    }

    fn typographic_bounds(&self, _font_id: FontId, _glyph_id: GlyphId) -> Result<Bounds<f32>> {
        Ok(Bounds {
            origin: Point { x: 54.0, y: 0.0 },
            size: size(392.0, 528.0),
        })
    }

    fn advance(&self, _font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>> {
        Ok(size(600.0 * glyph_id.0 as f32, 0.0))
    }

    fn glyph_for_char(&self, _font_id: FontId, ch: char) -> Option<GlyphId> {
        Some(GlyphId(ch.len_utf16() as u32))
    }

    fn glyph_raster_bounds(&self, _params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        Ok(Default::default())
    }

    fn rasterize_glyph(
        &self,
        _params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        Ok((raster_bounds.size, Vec::new()))
    }

    fn layout_line(&self, text: &str, font_size: Pixels, _runs: &[FontRun]) -> LineLayout {
        let mut position = px(0.);
        let metrics = self.font_metrics(FontId(0));
        let em_width = font_size
            * self
                .advance(FontId(0), self.glyph_for_char(FontId(0), 'm').unwrap())
                .unwrap()
                .width
            / metrics.units_per_em as f32;
        let mut glyphs = Vec::new();
        for (ix, c) in text.char_indices() {
            if let Some(glyph) = self.glyph_for_char(FontId(0), c) {
                glyphs.push(ShapedGlyph {
                    id: glyph,
                    position: point(position, px(0.)),
                    index: ix,
                    is_emoji: glyph.0 == 2,
                });
                if glyph.0 == 2 {
                    position += em_width * 2.0;
                } else {
                    position += em_width;
                }
            } else {
                position += em_width
            }
        }
        let mut runs = Vec::default();
        if !glyphs.is_empty() {
            runs.push(ShapedRun {
                font_id: FontId(0),
                glyphs,
            });
        } else {
            position = px(0.);
        }

        LineLayout {
            font_size,
            width: position,
            ascent: font_size * (metrics.ascent / metrics.units_per_em as f32),
            descent: font_size * (metrics.descent / metrics.units_per_em as f32),
            runs,
            len: text.len(),
        }
    }

    fn recommended_rendering_mode(
        &self,
        _font_id: FontId,
        _font_size: Pixels,
    ) -> TextRenderingMode {
        TextRenderingMode::Grayscale
    }
}

// Adapted from https://github.com/microsoft/terminal/blob/1283c0f5b99a2961673249fa77c6b986efb5086c/src/renderer/atlas/dwrite.cpp
// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
/// 计算亚像素文本渲染的伽马校正比率。
pub fn get_gamma_correction_ratios(gamma: f32) -> [f32; 4] {
    const GAMMA_INCORRECT_TARGET_RATIOS: [[f32; 4]; 13] = [
        [0.0000 / 4.0, 0.0000 / 4.0, 0.0000 / 4.0, 0.0000 / 4.0], // gamma = 1.0
        [0.0166 / 4.0, -0.0807 / 4.0, 0.2227 / 4.0, -0.0751 / 4.0], // gamma = 1.1
        [0.0350 / 4.0, -0.1760 / 4.0, 0.4325 / 4.0, -0.1370 / 4.0], // gamma = 1.2
        [0.0543 / 4.0, -0.2821 / 4.0, 0.6302 / 4.0, -0.1876 / 4.0], // gamma = 1.3
        [0.0739 / 4.0, -0.3963 / 4.0, 0.8167 / 4.0, -0.2287 / 4.0], // gamma = 1.4
        [0.0933 / 4.0, -0.5161 / 4.0, 0.9926 / 4.0, -0.2616 / 4.0], // gamma = 1.5
        [0.1121 / 4.0, -0.6395 / 4.0, 1.1588 / 4.0, -0.2877 / 4.0], // gamma = 1.6
        [0.1300 / 4.0, -0.7649 / 4.0, 1.3159 / 4.0, -0.3080 / 4.0], // gamma = 1.7
        [0.1469 / 4.0, -0.8911 / 4.0, 1.4644 / 4.0, -0.3234 / 4.0], // gamma = 1.8
        [0.1627 / 4.0, -1.0170 / 4.0, 1.6051 / 4.0, -0.3347 / 4.0], // gamma = 1.9
        [0.1773 / 4.0, -1.1420 / 4.0, 1.7385 / 4.0, -0.3426 / 4.0], // gamma = 2.0
        [0.1908 / 4.0, -1.2652 / 4.0, 1.8650 / 4.0, -0.3476 / 4.0], // gamma = 2.1
        [0.2031 / 4.0, -1.3864 / 4.0, 1.9851 / 4.0, -0.3501 / 4.0], // gamma = 2.2
    ];

    const NORM13: f32 = ((0x10000 as f64) / (255.0 * 255.0) * 4.0) as f32;
    const NORM24: f32 = ((0x100 as f64) / (255.0) * 4.0) as f32;

    let index = ((gamma * 10.0).round() as usize).clamp(10, 22) - 10;
    let ratios = GAMMA_INCORRECT_TARGET_RATIOS[index];

    [
        ratios[0] * NORM13,
        ratios[1] * NORM24,
        ratios[2] * NORM13,
        ratios[3] * NORM24,
    ]
}

/// 精灵图集的缓存键，标识一种可渲染的图元（字形、SVG 或图像）。
#[derive(PartialEq, Eq, Hash, Clone)]
pub enum AtlasKey {
    /// 字形图元
    Glyph(RenderGlyphParams),
    /// SVG 矢量图元
    Svg(RenderSvgParams),
    /// 位图图像图元
    Image(RenderImageParams),
}

impl AtlasKey {
    /// 返回该图集键的纹理类型。
    pub fn texture_kind(&self) -> AtlasTextureKind {
        match self {
            AtlasKey::Glyph(params) => {
                if params.is_emoji {
                    AtlasTextureKind::Polychrome
                } else if params.subpixel_rendering {
                    AtlasTextureKind::Subpixel
                } else {
                    AtlasTextureKind::Monochrome
                }
            }
            AtlasKey::Svg(_) => AtlasTextureKind::Monochrome,
            AtlasKey::Image(_) => AtlasTextureKind::Polychrome,
        }
    }
}

impl From<RenderGlyphParams> for AtlasKey {
    fn from(params: RenderGlyphParams) -> Self {
        Self::Glyph(params)
    }
}

impl From<RenderSvgParams> for AtlasKey {
    fn from(params: RenderSvgParams) -> Self {
        Self::Svg(params)
    }
}

impl From<RenderImageParams> for AtlasKey {
    fn from(params: RenderImageParams) -> Self {
        Self::Image(params)
    }
}

/// 平台精灵图集抽象 — 管理 GPU 纹理中的图元缓存（字形、SVG、图像）。
pub trait PlatformAtlas {
    /// 根据键获取图集瓦片，若不存在则通过 build 闭包创建并插入。
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>>;
    /// 从图集中移除指定键对应的瓦片。
    fn remove(&self, key: &AtlasKey);
    #[cfg(any(test, feature = "test-support"))]
    fn contains(&self, _key: &AtlasKey) -> bool {
        false
    }
}

#[doc(hidden)]
pub struct AtlasTextureList<T> {
    pub textures: Vec<Option<T>>,
    pub free_list: Vec<usize>,
}

impl<T> Default for AtlasTextureList<T> {
    fn default() -> Self {
        Self {
            textures: Vec::default(),
            free_list: Vec::default(),
        }
    }
}

impl<T> ops::Index<usize> for AtlasTextureList<T> {
    type Output = Option<T>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.textures[index]
    }
}

impl<T> AtlasTextureList<T> {
    #[allow(unused)]
    pub fn drain(&mut self) -> std::vec::Drain<'_, Option<T>> {
        self.free_list.clear();
        self.textures.drain(..)
    }

    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut T> {
        self.textures.iter_mut().flatten()
    }
}

/// 精灵图集中的一块瓦片，描述其在纹理中的位置和边距。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct AtlasTile {
    /// 该瓦片所属的纹理。
    pub texture_id: AtlasTextureId,
    /// 该瓦片在其纹理内的唯一 ID。
    pub tile_id: TileId,
    /// 瓦片内容周围的像素边距。
    pub padding: u32,
    /// 该瓦片在纹理中的边界区域。
    pub bounds: Bounds<DevicePixels>,
}

/// 图集纹理的唯一标识符，包含索引和内容类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct AtlasTextureId {
    // 使用 u32 而非 usize 以兼容 Metal Shader Language
    /// 该纹理在图集中的索引。
    pub index: u32,
    /// 该纹理中存储的内容类型。
    pub kind: AtlasTextureKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
/// 图集纹理的内容类型，决定颜色格式和渲染路径。
pub enum AtlasTextureKind {
    /// 单色（灰度字形）
    Monochrome,
    /// 多色（彩色图像、Emoji）
    Polychrome,
    /// 亚像素渲染（LCD 抗锯齿字形）
    Subpixel,
}

/// 图集瓦片的唯一标识符，封装 etagere 分配器的序列化 ID。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct TileId(pub u32);

impl From<etagere::AllocId> for TileId {
    fn from(id: etagere::AllocId) -> Self {
        Self(id.serialize())
    }
}

impl From<TileId> for etagere::AllocId {
    fn from(id: TileId) -> Self {
        Self::deserialize(id.0)
    }
}

/// 平台输入处理器，封装异步窗口上下文和文本输入回调，处理选区、标记文本等 IME 操作。
pub struct PlatformInputHandler {
    cx: AsyncWindowContext,
    handler: Box<dyn InputHandler>,
}

impl PlatformInputHandler {
    /// 创建新的输入处理器。
    pub fn new(cx: AsyncWindowContext, handler: Box<dyn InputHandler>) -> Self {
        Self { cx, handler }
    }

    /// 获取当前选中的文本范围（UTF-16 偏移）。
    pub fn selected_text_range(&mut self, ignore_disabled_input: bool) -> Option<UTF16Selection> {
        self.cx
            .update(|window, cx| {
                self.handler
                    .selected_text_range(ignore_disabled_input, window, cx)
            })
            .ok()
            .flatten()
    }

    /// 获取当前标记（未确认）文本的范围。
    pub fn marked_text_range(&mut self) -> Option<Range<usize>> {
        self.cx
            .update(|window, cx| self.handler.marked_text_range(window, cx))
            .ok()
            .flatten()
    }

    /// 获取指定 UTF-16 范围内的文本内容。
    pub fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
    ) -> Option<String> {
        self.cx
            .update(|window, cx| {
                self.handler
                    .text_for_range(range_utf16, adjusted, window, cx)
            })
            .ok()
            .flatten()
    }

    /// 替换指定范围内的文本。
    pub fn replace_text_in_range(&mut self, replacement_range: Option<Range<usize>>, text: &str) {
        self.cx
            .update(|window, cx| {
                self.handler
                    .replace_text_in_range(replacement_range, text, window, cx);
            })
            .ok();
    }

    /// 替换指定范围内的文本并设置标记（IME 组合文本）。
    pub fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
    ) {
        self.cx
            .update(|window, cx| {
                self.handler.replace_and_mark_text_in_range(
                    range_utf16,
                    new_text,
                    new_selected_range,
                    window,
                    cx,
                )
            })
            .ok();
    }

    /// 清除标记文本（确认输入）。
    pub fn unmark_text(&mut self) {
        self.cx
            .update(|window, cx| self.handler.unmark_text(window, cx))
            .ok();
    }

    /// 获取指定 UTF-16 范围在屏幕上的边界矩形。
    pub fn bounds_for_range(&mut self, range_utf16: Range<usize>) -> Option<Bounds<Pixels>> {
        self.cx
            .update(|window, cx| self.handler.bounds_for_range(range_utf16, window, cx))
            .ok()
            .flatten()
    }

    /// macOS: 是否启用长按弹出字符面板功能。
    pub fn apple_press_and_hold_enabled(&mut self) -> bool {
        self.handler.apple_press_and_hold_enabled()
    }

    /// 直接分发文本输入（绕过 IME 组合流程）。
    pub fn dispatch_input(&mut self, input: &str, window: &mut Window, cx: &mut App) {
        self.handler.replace_text_in_range(None, input, window, cx);
    }

    /// 计算 IME 候选框的屏幕位置（基于标记文本范围和选区位置）。
    pub fn compute_ime_candidate_bounds(
        marked_range: Option<Range<usize>>,
        selection: &UTF16Selection,
        mut bounds_for_range: impl FnMut(Range<usize>) -> Option<Bounds<Pixels>>,
    ) -> Option<Bounds<Pixels>> {
        if let Some(marked_range) = marked_range {
            // Default to the start of the marked (composing) range.
            let mut line_start = marked_range.start;

            // Walk backward from the caret looking for a line break. A change in
            // the Y coordinate means we crossed into the previous visual line, so
            // the line start is one position after the break point.
            let caret = selection.range.end;
            if let Some(caret_bounds) = bounds_for_range(caret..caret) {
                for i in (marked_range.start..caret).rev() {
                    if let Some(b) = bounds_for_range(i..i) {
                        if (b.origin.y - caret_bounds.origin.y).abs() > px(0.1) {
                            line_start = i + 1;
                            break;
                        }
                    }
                }
            }
            bounds_for_range(line_start..line_start)
        } else {
            // No active composition  — use the selection endpoint.
            let offset = if selection.reversed {
                selection.range.start
            } else {
                selection.range.end
            };
            bounds_for_range(offset..offset)
        }
    }

    /// 获取当前选中文本的边界框。
    pub fn selected_bounds(&mut self, window: &mut Window, cx: &mut App) -> Option<Bounds<Pixels>> {
        let marked_range = self.handler.marked_text_range(window, cx);
        let selection = self.handler.selected_text_range(true, window, cx)?;
        Self::compute_ime_candidate_bounds(marked_range, &selection, |range| {
            self.handler.bounds_for_range(range, window, cx)
        })
    }

    /// 获取 IME 候选区域的边界框。
    pub fn ime_candidate_bounds(&mut self) -> Option<Bounds<Pixels>> {
        let marked_range = self.marked_text_range();
        let selection = self.selected_text_range(true)?;
        Self::compute_ime_candidate_bounds(marked_range, &selection, |range| {
            self.bounds_for_range(range)
        })
    }

    /// 根据屏幕坐标返回最近的字符索引。
    #[allow(unused)]
    pub fn character_index_for_point(&mut self, point: Point<Pixels>) -> Option<usize> {
        self.cx
            .update(|window, cx| self.handler.character_index_for_point(point, window, cx))
            .ok()
            .flatten()
    }

    /// 查询当前是否接受文本输入。
    pub fn accepts_text_input(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.handler.accepts_text_input(window, cx)
    }

    /// 查询当前是否接受文本输入（异步版本）。
    pub fn query_accepts_text_input(&mut self) -> bool {
        self.cx
            .update(|window, cx| self.handler.accepts_text_input(window, cx))
            .unwrap_or(true)
    }

    /// 参见 [`InputHandler::prefers_ime_for_printable_keys`]。
    ///
    /// 这不是对处理器简单的委托：当多按键绑定处于待处理状态时，无论处理器的偏好如何，
    /// 该函数都会返回 `false`，因为下一个可打印按键可能完成一个前缀已绕过 IME 的绑定。
    pub fn query_prefers_ime_for_printable_keys(&mut self) -> bool {
        self.cx
            .update(|window, cx| {
                // 下一个可打印按键可能完成一个前缀已绕过 IME 的按键组合。
                !window.has_pending_keystrokes()
                    && self.handler.prefers_ime_for_printable_keys(window, cx)
            })
            .unwrap_or(false)
    }
}

/// 表示文本缓冲区中的选区，以 UTF16 字符为单位。
/// 与 Range 不同，选区的头部可能在尾部之前。
#[derive(Debug)]
pub struct UTF16Selection {
    /// 该选区对应的文档中文本的范围（以 UTF16 字符为单位）。
    pub range: Range<usize>,
    /// 选区的头部是否在范围的起始位置（true）或结束位置（false）。
    pub reversed: bool,
}

/// Zed 的平台 IME 系统文本输入处理接口。
/// 目前是 NSTextInputClient API 的 1:1 映射：
///
/// <https://developer.apple.com/documentation/appkit/nstextinputclient>
pub trait InputHandler: 'static {
    /// 获取用户当前选中文本的范围（如果有）。
    /// 对应 [selectedRange()](https://developer.apple.com/documentation/appkit/nstextinputclient/1438242-selectedrange)
    ///
    /// 返回值以 UTF-16 字符为单位，范围从 0 到文档长度。
    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<UTF16Selection>;

    /// 获取当前标记（未确认）文本的范围（如果有）。
    /// 对应 [markedRange()](https://developer.apple.com/documentation/appkit/nstextinputclient/1438250-markedrange)
    ///
    /// 返回值以 UTF-16 字符为单位，范围从 0 到文档长度。
    fn marked_text_range(&mut self, window: &mut Window, cx: &mut App) -> Option<Range<usize>>;

    /// 获取给定文档范围内的文本（以 UTF-16 字符为单位）。
    /// 对应 [attributedSubstring(forProposedRange: actualRange:)](https://developer.apple.com/documentation/appkit/nstextinputclient/1438238-attributedsubstring)
    ///
    /// range_utf16 以 UTF-16 字符为单位。
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<String>;

    /// 用给定文本替换文档中指定范围的文本。
    /// 对应 [insertText(_:replacementRange:)](https://developer.apple.com/documentation/appkit/nstextinputclient/1438258-inserttext)
    ///
    /// replacement_range 以 UTF-16 字符为单位。
    fn replace_text_in_range(
        &mut self,
        replacement_range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    );

    /// 用给定文本替换文档中指定范围的文本，
    /// 并将给定文本标记为 IME「组合」状态的一部分。
    /// 对应 [setMarkedText(_:selectedRange:replacementRange:)](https://developer.apple.com/documentation/appkit/nstextinputclient/1438246-setmarkedtext)
    ///
    /// range_utf16 以 UTF-16 字符为单位。
    /// new_selected_range 以 UTF-16 字符为单位。
    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    );

    /// 移除文档中的 IME「组合」状态。
    /// 对应 [unmarkText()](https://developer.apple.com/documentation/appkit/nstextinputclient/1438239-unmarktext)
    fn unmark_text(&mut self, window: &mut Window, cx: &mut App);

    /// 获取给定文档范围在屏幕坐标中的边界区域。
    /// 对应 [firstRect(forCharacterRange:actualRange:)](https://developer.apple.com/documentation/appkit/nstextinputclient/1438240-firstrect)
    ///
    /// 用于定位 IME 候选窗口。
    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Bounds<Pixels>>;

    /// 获取给定点在 UTF16 字符中的字符偏移量。
    ///
    /// 对应 [characterIndexForPoint:](https://developer.apple.com/documentation/appkit/nstextinputclient/characterindex(for:))
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<usize>;

    /// 允许输入上下文选择接收原始按键重复，而非将其发送到平台。
    /// TODO: 理想情况下应能通过 NSUserDefaults 设置 ApplePressAndHoldEnabled
    /// （iTerm 就是这样做的），但目前似乎不生效。
    fn apple_press_and_hold_enabled(&mut self) -> bool {
        true
    }

    /// 返回此处理器是否接受要插入的文本输入。
    fn accepts_text_input(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        true
    }

    /// 返回在非 ASCII 输入源（如日语、韩语、中文 IME）激活时，
    /// 可打印按键是否应在按键绑定匹配之前先路由到 IME。
    /// 这防止了 `jj` 等多击按键绑定拦截 IME 应该组合的按键。
    ///
    /// 默认为 `false`。编辑器根据是否期望字符输入来覆盖此值
    /// （例如 Vim 插入模式返回 `true`，正常模式返回 `false`）。
    /// 终端保持默认的 `false`，以便原始按键到达终端进程。
    fn prefers_ime_for_printable_keys(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        false
    }

    /// 设置输入中的选中文本范围。
    fn set_selected_text_range(
        &mut self,
        _range_utf16: Range<usize>,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    /// 获取元素在屏幕坐标中的边界区域。
    fn element_bounds(&mut self, _window: &mut Window, _cx: &mut App) -> Option<Bounds<Pixels>> {
        None
    }

    /// 获取文本的长度（以 UTF-16 字符为单位）。
    fn text_length_utf16(&mut self, _window: &mut Window, _cx: &mut App) -> Option<usize> {
        None
    }
}

/// 创建窗口时可配置的变量
#[derive(Debug)]
pub struct WindowOptions {
    /// 指定窗口在屏幕坐标中的状态和边界。
    /// - `None`：继承边界。
    /// - `Some(WindowBounds)`：以对应的状态和恢复尺寸打开窗口。
    pub window_bounds: Option<WindowBounds>,

    /// 窗口标题栏配置
    pub titlebar: Option<TitlebarOptions>,

    /// 窗口创建时是否获取焦点
    pub focus: bool,

    /// 窗口创建时是否显示
    pub show: bool,

    /// 要创建的窗口类型
    pub kind: WindowKind,

    /// 窗口是否可被用户拖拽移动
    pub is_movable: bool,

    /// 窗口是否可被用户调整大小
    pub is_resizable: bool,

    /// 窗口是否可被用户最小化
    pub is_minimizable: bool,

    /// 在哪个显示器上创建窗口，若为 None，
    /// 则在主显示器上创建
    pub display_id: Option<DisplayId>,

    /// 窗口背景外观。
    pub window_background: WindowBackgroundAppearance,

    /// 窗口的应用标识符，桌面环境可用于将应用分组。
    pub app_id: Option<String>,

    /// 窗口最小尺寸
    pub window_min_size: Option<Size<Pixels>>,

    /// 使用客户端还是服务端装饰。仅 Wayland。
    /// 注意此设置可能被忽略。
    pub window_decorations: Option<WindowDecorations>,

    /// 图标图片（仅 X11）
    pub icon: Option<Arc<image::RgbaImage>>,

    /// 标签页组名称，允许在 macOS 10.12+ 上以原生标签页方式打开窗口。具有相同 tabbing identifier 的窗口将被分组在一起。
    pub tabbing_identifier: Option<String>,

    /// macOS 专用：应用是否自行处理标题栏拖拽。当使用自定义标题栏时设置为 true，
    /// 使 AppKit 不拦截标题栏点击，由应用通过 `Window::start_window_move` 自行处理。
    pub app_owns_titlebar_drag: bool,

    /// Windows/Linux：是否启用鼠标事件穿透（点击穿透到后面的窗口）。
    /// 用于桌面宠物、覆盖层等需要让鼠标点击穿透到下层窗口的场景。
    pub mouse_passthrough: bool,
}

/// 创建窗口时的配置参数。
#[derive(Debug)]
pub struct WindowParams {
    /// 窗口初始位置和尺寸。
    pub bounds: Bounds<Pixels>,

    /// 标题栏配置。
    pub titlebar: Option<TitlebarOptions>,

    /// 窗口类型（普通窗口、覆盖层等）。
    pub kind: WindowKind,

    /// 窗口是否可被用户拖拽移动。
    pub is_movable: bool,

    /// 窗口是否可被用户调整大小。
    pub is_resizable: bool,

    /// 窗口是否可被用户最小化。
    pub is_minimizable: bool,

    /// 窗口打开后是否自动获取焦点。
    pub focus: bool,

    /// 窗口打开后是否立即显示。
    pub show: bool,

    /// 窗口图标（仅 X11 有效）。
    pub icon: Option<Arc<image::RgbaImage>>,

    /// 指定显示在哪个显示器上（None 为默认）。
    pub display_id: Option<DisplayId>,

    /// 应用标识符（主要用于 Wayland）。
    pub app_id: Option<String>,

    /// 窗口最小尺寸限制。
    pub window_min_size: Option<Size<Pixels>>,
    /// macOS 标签页分组标识符，相同标识符的窗口可合并为标签页。
    #[cfg(target_os = "macos")]
    pub tabbing_identifier: Option<String>,

    /// macOS only: 应用是否自行处理标题栏拖拽。
    /// 当使用自定义标题栏时设置为 true（macOS 专用，其他平台无效果）。
    pub app_owns_titlebar_drag: bool,

    /// Windows/Linux: 是否启用鼠标事件穿透（点击穿透到后面的窗口）。
    /// 覆盖层窗口需要此选项让鼠标事件穿透到底层窗口。
    pub mouse_passthrough: bool,
}

/// 表示窗口打开时应处于的状态
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum WindowBounds {
    /// 表示窗口应以窗口化状态打开，使用给定的边界。
    Windowed(Bounds<Pixels>),
    /// 表示窗口应以最大化状态打开。
    /// 此处提供的边界表示窗口的恢复尺寸。
    Maximized(Bounds<Pixels>),
    /// 表示窗口应以全屏模式打开。
    /// 此处提供的边界表示窗口的恢复尺寸。
    Fullscreen(Bounds<Pixels>),
}

impl Default for WindowBounds {
    fn default() -> Self {
        WindowBounds::Windowed(Bounds::default())
    }
}

impl WindowBounds {
    /// 获取内部边界
    pub fn get_bounds(&self) -> Bounds<Pixels> {
        match self {
            WindowBounds::Windowed(bounds) => *bounds,
            WindowBounds::Maximized(bounds) => *bounds,
            WindowBounds::Fullscreen(bounds) => *bounds,
        }
    }

    /// 创建一个新的窗口边界，使窗口在屏幕上居中。
    pub fn centered(size: Size<Pixels>, cx: &App) -> Self {
        WindowBounds::Windowed(Bounds::centered(None, size, cx))
    }
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            window_bounds: None,
            titlebar: Some(TitlebarOptions {
                title: Default::default(),
                appears_transparent: Default::default(),
                traffic_light_position: Default::default(),
            }),
            focus: true,
            show: true,
            kind: WindowKind::Normal,
            is_movable: true,
            is_resizable: true,
            is_minimizable: true,
            display_id: None,
            window_background: WindowBackgroundAppearance::default(),
            icon: None,
            app_id: None,
            window_min_size: None,
            window_decorations: None,
            tabbing_identifier: None,
            app_owns_titlebar_drag: false,
            mouse_passthrough: false,
        }
    }
}

/// 窗口标题栏可配置的选项
#[derive(Debug, Default)]
pub struct TitlebarOptions {
    /// 窗口的初始标题
    pub title: Option<SharedString>,

    /// 是否隐藏默认系统标题栏以使用自定义绘制的标题栏？（仅 macOS 和 Windows）
    /// Linux 上请参见 [`WindowOptions::window_decorations`]
    pub appears_transparent: bool,

    /// macOS 红绿灯按钮的位置
    pub traffic_light_position: Option<Point<Pixels>>,
}

/// 要创建的窗口类型
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowKind {
    /// 普通应用窗口
    Normal,

    /// 出现在所有其他窗口上方的窗口，通常用于警告或弹出窗口。
    /// 应谨慎使用！
    PopUp,

    /// 父窗口锚定的原生弹出窗口，用于菜单、组合框、上下文菜单和工具提示。
    /// 与 [`WindowKind::PopUp`] 不同，它相对于父窗口定位。
    ///
    /// 弹出窗口的大小来自 [`WindowOptions::window_bounds`]，其原点被忽略。
    /// 参见 [`popup::PopupOptions`] 了解放置选项。没有原生实现的平台
    /// 会以 [`popup::PopupNotSupportedError`] 拒绝。
    AnchoredPopup(popup::PopupOptions),

    /// 出现在父窗口上方的浮动窗口
    Floating,

    /// Wayland LayerShell 窗口，用于为应用绘制覆盖层或背景，
    /// 如 Dock、通知或壁纸。
    #[cfg(all(target_os = "linux", feature = "wayland"))]
    LayerShell(layer_shell::LayerShellOptions),

    /// 出现在父窗口上方的模态窗口，阻止与父窗口的交互，
    /// 直到模态窗口关闭
    Dialog,

    /// 覆盖层窗口：始终置顶、无边框、支持透明度
    Overlay,
}

/// 窗口的外观，由操作系统定义。
///
/// 在 macOS 上，这对应于命名的 [`NSAppearance`](https://developer.apple.com/documentation/appkit/nsappearance)
/// 值。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum WindowAppearance {
    /// 亮色外观。
    ///
    /// 在 macOS 上，这对应于 `aqua` 外观。
    #[default]
    Light,

    /// 带有鲜艳颜色的亮色外观。
    ///
    /// 在 macOS 上，这对应于 `NSAppearanceNameVibrantLight` 外观。
    VibrantLight,

    /// 暗色外观。
    ///
    /// 在 macOS 上，这对应于 `darkAqua` 外观。
    Dark,

    /// 带有鲜艳颜色的暗色外观。
    ///
    /// 在 macOS 上，这对应于 `NSAppearanceNameVibrantDark` 外观。
    VibrantDark,
}

/// 窗口本身的背景外观，在没有内容或内容透明时显示。
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum WindowBackgroundAppearance {
    /// 不透明。
    ///
    /// 告诉窗口管理器此窗口背后的内容不需要绘制。
    ///
    /// 实际颜色取决于系统，主题应定义完全不透明的背景色。
    #[default]
    Opaque,
    /// 纯 Alpha 透明。
    Transparent,
    /// 透明，但窗口背后的内容会被模糊。
    ///
    /// 并非总是支持。
    Blurred,
    /// Mica 背景材质，Windows 11 支持。
    MicaBackdrop,
    /// Mica Alt 背景材质，Windows 11 支持。
    MicaAltBackdrop,
}

/// 绘制字形时使用的文本渲染模式。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum TextRenderingMode {
    /// 使用平台默认的文本渲染模式。
    #[default]
    PlatformDefault,
    /// 使用亚像素（ClearType 风格）文本渲染。
    Subpixel,
    /// 使用灰度文本渲染。
    Grayscale,
}

/// 文件对话框提示可配置的选项
#[derive(Clone, Debug)]
pub struct PathPromptOptions {
    /// 提示是否允许选择文件？
    pub files: bool,
    /// 提示是否允许选择目录？
    pub directories: bool,
    /// 提示是否允许多选文件？
    pub multiple: bool,
    /// 选择路径时显示给用户的提示文本
    pub prompt: Option<SharedString>,
}

/// 提示样式类型
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PromptLevel {
    /// 通知用户的提示
    Info,

    /// 警告用户潜在问题的提示
    Warning,

    /// 发生严重问题时的提示
    Critical,
}

/// 提示对话框按钮
#[derive(Clone, Debug, PartialEq)]
pub enum PromptButton {
    /// 确认按钮
    Ok(SharedString),
    /// 取消按钮
    Cancel(SharedString),
    /// 其他按钮
    Other(SharedString),
}

impl PromptButton {
    /// 创建带标签的按钮
    pub fn new(label: impl Into<SharedString>) -> Self {
        PromptButton::Other(label.into())
    }

    /// 创建确认按钮
    pub fn ok(label: impl Into<SharedString>) -> Self {
        PromptButton::Ok(label.into())
    }

    /// 创建取消按钮
    pub fn cancel(label: impl Into<SharedString>) -> Self {
        PromptButton::Cancel(label.into())
    }

    /// 返回此按钮是否为取消按钮。
    pub fn is_cancel(&self) -> bool {
        matches!(self, PromptButton::Cancel(_))
    }

    /// 返回按钮的标签文本
    pub fn label(&self) -> &SharedString {
        match self {
            PromptButton::Ok(label) => label,
            PromptButton::Cancel(label) => label,
            PromptButton::Other(label) => label,
        }
    }
}

impl From<&str> for PromptButton {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "ok" => PromptButton::Ok("OK".into()),
            "cancel" => PromptButton::Cancel("Cancel".into()),
            _ => PromptButton::Other(SharedString::from(value.to_owned())),
        }
    }
}

/// 光标（指针）样式
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum CursorStyle {
    /// 默认光标
    #[default]
    Arrow,

    /// 文本输入光标
    /// 对应 CSS cursor 值 `text`
    IBeam,

    /// 十字光标
    /// 对应 CSS cursor 值 `crosshair`
    Crosshair,

    /// 闭合手型光标
    /// 对应 CSS cursor 值 `grabbing`
    ClosedHand,

    /// 张开手型光标
    /// 对应 CSS cursor 值 `grab`
    OpenHand,

    /// 指向手型光标
    /// 对应 CSS cursor 值 `pointer`
    PointingHand,

    /// 向左调整大小光标
    /// 对应 CSS cursor 值 `w-resize`
    ResizeLeft,

    /// 向右调整大小光标
    /// 对应 CSS cursor 值 `e-resize`
    ResizeRight,

    /// 左右调整大小光标
    /// 对应 CSS cursor 值 `ew-resize`
    ResizeLeftRight,

    /// 向上调整大小光标
    /// 对应 CSS cursor 值 `n-resize`
    ResizeUp,

    /// 向下调整大小光标
    /// 对应 CSS cursor 值 `s-resize`
    ResizeDown,

    /// 上下调整大小光标
    /// 对应 CSS cursor 值 `ns-resize`
    ResizeUpDown,

    /// 向左上和右下调整大小光标
    /// 对应 CSS cursor 值 `nesw-resize`
    ResizeUpLeftDownRight,

    /// 向右上和左下调整大小光标
    /// 对应 CSS cursor 值 `nwse-resize`
    ResizeUpRightDownLeft,

    /// 表示可以水平调整大小的光标
    /// 对应 CSS cursor 值 `col-resize`
    ResizeColumn,

    /// 表示可以垂直调整大小的光标
    /// 对应 CSS cursor 值 `row-resize`
    ResizeRow,

    /// 垂直布局的文本输入光标
    /// 对应 CSS cursor 值 `vertical-text`
    IBeamCursorForVerticalLayout,

    /// 表示操作不允许的光标
    /// 对应 CSS cursor 值 `not-allowed`
    OperationNotAllowed,

    /// 表示操作将产生链接的光标
    /// 对应 CSS cursor 值 `alias`
    DragLink,

    /// 表示操作将产生副本的光标
    /// 对应 CSS cursor 值 `copy`
    DragCopy,

    /// 表示操作将产生上下文菜单的光标
    /// 对应 CSS cursor 值 `context-menu`
    ContextualMenu,
}

/// 应复制到剪贴板的剪贴板项目
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardItem {
    /// 此剪贴板项目的条目。
    pub entries: Vec<ClipboardEntry>,
}

/// 剪贴板字符串或剪贴板图像
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardEntry {
    /// 字符串条目
    String(ClipboardString),
    /// 图像条目
    Image(Image),
    /// 文件条目
    ExternalPaths(crate::ExternalPaths),
}

impl ClipboardItem {
    /// 创建一个不带关联元数据的新 ClipboardItem::String
    pub fn new_string(text: String) -> Self {
        Self {
            entries: vec![ClipboardEntry::String(ClipboardString::new(text))],
        }
    }

    /// 创建一个带有关联元数据的新 ClipboardItem::String
    pub fn new_string_with_metadata(text: String, metadata: String) -> Self {
        Self {
            entries: vec![ClipboardEntry::String(ClipboardString {
                text,
                metadata: Some(metadata),
            })],
        }
    }

    /// 创建一个带有关联元数据（JSON 序列化）的新 ClipboardItem::String
    pub fn new_string_with_json_metadata<T: Serialize>(text: String, metadata: T) -> Self {
        Self {
            entries: vec![ClipboardEntry::String(
                ClipboardString::new(text).with_json_metadata(metadata),
            )],
        }
    }

    /// 创建一个不带关联元数据的新 ClipboardItem::Image
    pub fn new_image(image: &Image) -> Self {
        Self {
            entries: vec![ClipboardEntry::Image(image.clone())],
        }
    }

    /// 连接项目中所有 ClipboardString 条目的文本。
    /// 如果没有 ClipboardString 条目则返回 None。
    pub fn text(&self) -> Option<String> {
        let mut answer = String::new();

        for entry in self.entries.iter() {
            if let ClipboardEntry::String(ClipboardString { text, metadata: _ }) = entry {
                answer.push_str(text);
            }
        }

        if answer.is_empty() {
            for entry in self.entries.iter() {
                if let ClipboardEntry::ExternalPaths(paths) = entry {
                    for path in &paths.0 {
                        use std::fmt::Write as _;
                        _ = write!(answer, "{}", path.display());
                    }
                }
            }
        }

        if !answer.is_empty() {
            Some(answer)
        } else {
            None
        }
    }

    /// 如果此项目是单个 ClipboardEntry::String，返回其元数据。
    pub fn metadata(&self) -> Option<&String> {
        match self.entries().first() {
            Some(ClipboardEntry::String(clipboard_string)) if self.entries.len() == 1 => {
                clipboard_string.metadata.as_ref()
            }
            _ => None,
        }
    }

    /// 获取项目的条目
    pub fn entries(&self) -> &[ClipboardEntry] {
        &self.entries
    }

    /// 获取项目条目的所有权版本
    pub fn into_entries(self) -> impl Iterator<Item = ClipboardEntry> {
        self.entries.into_iter()
    }
}

impl From<ClipboardString> for ClipboardEntry {
    fn from(value: ClipboardString) -> Self {
        Self::String(value)
    }
}

impl From<String> for ClipboardEntry {
    fn from(value: String) -> Self {
        Self::from(ClipboardString::from(value))
    }
}

impl From<Image> for ClipboardEntry {
    fn from(value: Image) -> Self {
        Self::Image(value)
    }
}

impl From<ClipboardEntry> for ClipboardItem {
    fn from(value: ClipboardEntry) -> Self {
        Self {
            entries: vec![value],
        }
    }
}

impl From<String> for ClipboardItem {
    fn from(value: String) -> Self {
        Self::from(ClipboardEntry::from(value))
    }
}

impl From<Image> for ClipboardItem {
    fn from(value: Image) -> Self {
        Self::from(ClipboardEntry::from(value))
    }
}

/// 编辑器支持的图像格式之一（如 PNG、JPEG）- 用于处理剪贴板中的图像
#[derive(Clone, Copy, Debug, Eq, PartialEq, EnumIter, Hash)]
pub enum ImageFormat {
    // 按粘贴到编辑器的可能性从高到低排序，
    // 在遍历检查剪贴板内容是否匹配时这很重要。
    /// .png
    Png,
    /// .jpeg 或 .jpg
    Jpeg,
    /// .webp
    Webp,
    /// .gif
    Gif,
    /// .svg
    Svg,
    /// .bmp
    Bmp,
    /// .tif 或 .tiff
    Tiff,
    /// .ico
    Ico,
    /// Netpbm 图像格式（.pbm、.ppm、.pgm）。
    Pnm,
}

impl ImageFormat {
    /// 返回 ImageFormat 的 MIME 类型
    pub const fn mime_type(self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Webp => "image/webp",
            ImageFormat::Gif => "image/gif",
            ImageFormat::Svg => "image/svg+xml",
            ImageFormat::Bmp => "image/bmp",
            ImageFormat::Tiff => "image/tiff",
            ImageFormat::Ico => "image/ico",
            ImageFormat::Pnm => "image/x-portable-anymap",
        }
    }

    /// 根据 MIME 类型返回对应的 ImageFormat，包括已知别名。
    pub fn from_mime_type(mime_type: &str) -> Option<Self> {
        use strum::IntoEnumIterator;
        Self::iter()
            .find(|format| format.mime_type() == mime_type)
            .or_else(|| Self::from_mime_type_alias(mime_type))
    }

    /// 非规范的 MIME 类型，一些生产者在实际使用中使用。
    /// 不同于返回单一规范形式的 `mime_type()`，
    /// 这些是我们仍需识别的遗留或缩写变体。
    fn from_mime_type_alias(mime_type: &str) -> Option<Self> {
        match mime_type {
            "image/jpg" => Some(Self::Jpeg),
            "image/tif" => Some(Self::Tiff),
            _ => None,
        }
    }
}

/// 图像，包含格式和字节数据
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// 字节数据表示的图像格式（如 PNG）
    pub format: ImageFormat,
    /// 原始图像字节
    pub bytes: Vec<u8>,
    /// 图像的唯一 ID
    pub id: u64,
}

impl Hash for Image {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.id);
    }
}

impl Image {
    /// 一个不包含数据的空图像
    pub fn empty() -> Self {
        Self::from_bytes(ImageFormat::Png, Vec::new())
    }

    /// 从格式和字节数据创建图像
    pub fn from_bytes(format: ImageFormat, bytes: Vec<u8>) -> Self {
        Self {
            id: hash(&bytes),
            format,
            bytes,
        }
    }

    /// 获取图像的 ID
    pub fn id(&self) -> u64 {
        self.id
    }

    /// 使用 RGPUI `use_asset` API 使此图像可渲染
    pub fn use_render_image(
        self: Arc<Self>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Arc<RenderImage>> {
        ImageSource::Image(self)
            .use_data(None, window, cx)
            .and_then(|result| result.ok())
    }

    /// 使用 RGPUI `get_asset` API 使此图像可渲染
    pub fn get_render_image(
        self: Arc<Self>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Arc<RenderImage>> {
        ImageSource::Image(self)
            .get_data(None, window, cx)
            .and_then(|result| result.ok())
    }

    /// 使用 RGPUI `remove_asset` API 移除此图像（如果可能）。
    pub fn remove_asset(self: Arc<Self>, cx: &mut App) {
        ImageSource::Image(self).remove_asset(cx);
    }

    /// 将剪贴板图像转换为 `ImageData` 对象。
    pub fn to_image_data(&self, svg_renderer: SvgRenderer) -> Result<Arc<RenderImage>> {
        fn frames_for_image(
            bytes: &[u8],
            format: image::ImageFormat,
        ) -> Result<SmallVec<[Frame; 1]>> {
            let mut data = image::load_from_memory_with_format(bytes, format)?.into_rgba8();

            // Convert from RGBA to BGRA.
            for pixel in data.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }

            Ok(SmallVec::from_elem(Frame::new(data), 1))
        }

        let frames = match self.format {
            ImageFormat::Gif => {
                let decoder = GifDecoder::new(Cursor::new(&self.bytes))?;
                let mut frames = SmallVec::new();

                for frame in decoder.into_frames() {
                    match frame {
                        Ok(mut frame) => {
                            // Convert from RGBA to BGRA.
                            for pixel in frame.buffer_mut().chunks_exact_mut(4) {
                                pixel.swap(0, 2);
                            }
                            frames.push(frame);
                        }
                        Err(err) => {
                            log::debug!("Skipping GIF frame due to decode error: {err}");
                        }
                    }
                }

                if frames.is_empty() {
                    anyhow::bail!("GIF could not be decoded: all frames failed");
                }

                frames
            }
            ImageFormat::Png => frames_for_image(&self.bytes, image::ImageFormat::Png)?,
            ImageFormat::Jpeg => frames_for_image(&self.bytes, image::ImageFormat::Jpeg)?,
            ImageFormat::Webp => frames_for_image(&self.bytes, image::ImageFormat::WebP)?,
            ImageFormat::Bmp => frames_for_image(&self.bytes, image::ImageFormat::Bmp)?,
            ImageFormat::Tiff => frames_for_image(&self.bytes, image::ImageFormat::Tiff)?,
            ImageFormat::Ico => frames_for_image(&self.bytes, image::ImageFormat::Ico)?,
            ImageFormat::Svg => {
                return svg_renderer
                    .render_single_frame(&self.bytes, 1.0)
                    .map_err(Into::into);
            }
            ImageFormat::Pnm => frames_for_image(&self.bytes, image::ImageFormat::Pnm)?,
        };

        Ok(Arc::new(RenderImage::new(frames)))
    }

    /// 获取剪贴板图像的格式
    pub fn format(&self) -> ImageFormat {
        self.format
    }

    /// 获取剪贴板图像的原始字节
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

/// 应复制到剪贴板的剪贴板字符串项目
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardString {
    /// 文本内容。
    pub text: String,
    /// 关联的可选元数据。
    pub metadata: Option<String>,
}

impl ClipboardString {
    /// 创建一个新的剪贴板字符串
    pub fn new(text: String) -> Self {
        Self {
            text,
            metadata: None,
        }
    }

    /// 返回一个新的剪贴板项目，其元数据通过 JSON 序列化后替换为给定值。
    pub fn with_json_metadata<T: Serialize>(mut self, metadata: T) -> Self {
        self.metadata = Some(serde_json::to_string(&metadata).unwrap());
        self
    }

    /// 获取剪贴板字符串的文本
    pub fn text(&self) -> &String {
        &self.text
    }

    /// 获取剪贴板字符串的所有权文本
    pub fn into_text(self) -> String {
        self.text
    }

    /// 获取剪贴板字符串的元数据（JSON 格式）
    pub fn metadata_json<T>(&self) -> Option<T>
    where
        T: for<'a> Deserialize<'a>,
    {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
    }

    /// 计算给定文本的哈希值，用于剪贴板变化检测。
    pub fn text_hash(text: &str) -> u64 {
        let mut hasher = SeaHasher::new();
        text.hash(&mut hasher);
        hasher.finish()
    }
}

impl From<String> for ClipboardString {
    fn from(value: String) -> Self {
        Self {
            text: value,
            metadata: None,
        }
    }
}

#[cfg(test)]
mod image_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_svg_image_to_image_data_converts_to_bgra() {
        let image = Image::from_bytes(
            ImageFormat::Svg,
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">
<rect width="1" height="1" fill="#38BDF8"/>
</svg>"##
                .to_vec(),
        );

        let render_image = image.to_image_data(SvgRenderer::new(Arc::new(()))).unwrap();
        let bytes = render_image.as_bytes(0).unwrap();

        for pixel in bytes.chunks_exact(4) {
            assert_eq!(pixel, &[0xF8, 0xBD, 0x38, 0xFF]);
        }
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "freebsd")))]
mod tests {
    use super::*;
    use rgpui::collections::HashSet;

    #[test]
    fn test_window_button_layout_parse_standard() {
        let layout = WindowButtonLayout::parse("close,minimize:maximize").unwrap();
        assert_eq!(
            layout.left,
            [
                Some(WindowButton::Close),
                Some(WindowButton::Minimize),
                None
            ]
        );
        assert_eq!(layout.right, [Some(WindowButton::Maximize), None, None]);
    }

    #[test]
    fn test_window_button_layout_parse_right_only() {
        let layout = WindowButtonLayout::parse("minimize,maximize,close").unwrap();
        assert_eq!(layout.left, [None, None, None]);
        assert_eq!(
            layout.right,
            [
                Some(WindowButton::Minimize),
                Some(WindowButton::Maximize),
                Some(WindowButton::Close)
            ]
        );
    }

    #[test]
    fn test_window_button_layout_parse_left_only() {
        let layout = WindowButtonLayout::parse("close,minimize,maximize:").unwrap();
        assert_eq!(
            layout.left,
            [
                Some(WindowButton::Close),
                Some(WindowButton::Minimize),
                Some(WindowButton::Maximize)
            ]
        );
        assert_eq!(layout.right, [None, None, None]);
    }

    #[test]
    fn test_window_button_layout_parse_with_whitespace() {
        let layout = WindowButtonLayout::parse(" close , minimize : maximize ").unwrap();
        assert_eq!(
            layout.left,
            [
                Some(WindowButton::Close),
                Some(WindowButton::Minimize),
                None
            ]
        );
        assert_eq!(layout.right, [Some(WindowButton::Maximize), None, None]);
    }

    #[test]
    fn test_window_button_layout_parse_empty() {
        let layout = WindowButtonLayout::parse("").unwrap();
        assert_eq!(layout.left, [None, None, None]);
        assert_eq!(layout.right, [None, None, None]);
    }

    #[test]
    fn test_window_button_layout_parse_intentionally_empty() {
        let layout = WindowButtonLayout::parse(":").unwrap();
        assert_eq!(layout.left, [None, None, None]);
        assert_eq!(layout.right, [None, None, None]);
    }

    #[test]
    fn test_window_button_layout_parse_invalid_buttons() {
        let layout = WindowButtonLayout::parse("close,invalid,minimize:maximize,foo").unwrap();
        assert_eq!(
            layout.left,
            [
                Some(WindowButton::Close),
                Some(WindowButton::Minimize),
                None
            ]
        );
        assert_eq!(layout.right, [Some(WindowButton::Maximize), None, None]);
    }

    #[test]
    fn test_window_button_layout_parse_deduplicates_same_side_buttons() {
        let layout = WindowButtonLayout::parse("close,close,minimize").unwrap();
        assert_eq!(
            layout.right,
            [
                Some(WindowButton::Close),
                Some(WindowButton::Minimize),
                None
            ]
        );
        assert_eq!(layout.format(), ":close,minimize");
    }

    #[test]
    fn test_window_button_layout_parse_deduplicates_buttons_across_sides() {
        let layout = WindowButtonLayout::parse("close:maximize,close,minimize").unwrap();
        assert_eq!(layout.left, [Some(WindowButton::Close), None, None]);
        assert_eq!(
            layout.right,
            [
                Some(WindowButton::Maximize),
                Some(WindowButton::Minimize),
                None
            ]
        );

        let button_ids: Vec<_> = layout
            .left
            .iter()
            .chain(layout.right.iter())
            .flatten()
            .map(WindowButton::id)
            .collect();
        let unique_button_ids = button_ids.iter().copied().collect::<HashSet<_>>();
        assert_eq!(unique_button_ids.len(), button_ids.len());
        assert_eq!(layout.format(), "close:maximize,minimize");
    }

    #[test]
    fn test_window_button_layout_parse_gnome_style() {
        let layout = WindowButtonLayout::parse("close").unwrap();
        assert_eq!(layout.left, [None, None, None]);
        assert_eq!(layout.right, [Some(WindowButton::Close), None, None]);
    }

    #[test]
    fn test_window_button_layout_parse_elementary_style() {
        let layout = WindowButtonLayout::parse("close:maximize").unwrap();
        assert_eq!(layout.left, [Some(WindowButton::Close), None, None]);
        assert_eq!(layout.right, [Some(WindowButton::Maximize), None, None]);
    }

    #[test]
    fn test_window_button_layout_round_trip() {
        let cases = [
            "close:minimize,maximize",
            "minimize,maximize,close:",
            ":close",
            "close:",
            "close:maximize",
            ":",
        ];

        for case in cases {
            let layout = WindowButtonLayout::parse(case).unwrap();
            assert_eq!(layout.format(), case, "Round-trip failed for: {}", case);
        }
    }

    #[test]
    fn test_window_button_layout_linux_default() {
        let layout = WindowButtonLayout::linux_default();
        assert_eq!(layout.left, [None, None, None]);
        assert_eq!(
            layout.right,
            [
                Some(WindowButton::Minimize),
                Some(WindowButton::Maximize),
                Some(WindowButton::Close)
            ]
        );

        let round_tripped = WindowButtonLayout::parse(&layout.format()).unwrap();
        assert_eq!(round_tripped, layout);
    }

    #[test]
    fn test_window_button_layout_parse_all_invalid() {
        assert!(WindowButtonLayout::parse("asdfghjkl").is_err());
    }
}
