mod app_menu;
mod keyboard;
mod keystroke;

#[cfg(all(target_os = "linux", feature = "wayland"))]
#[expect(missing_docs)]
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

use crate::scheduler::Instant;
pub use crate::scheduler::RunnableMeta;
use crate::{
    Action, AnyWindowHandle, App, AsyncWindowContext, BackgroundExecutor, Bounds,
    DEFAULT_WINDOW_SIZE, DevicePixels, DispatchEventResult, Font, FontId, FontMetrics, FontRun,
    ForegroundExecutor, GlyphId, GpuSpecs, Hsla, ImageSource, Keymap, LineLayout, Pixels,
    PlatformInput, Point, Priority, RenderGlyphParams, RenderImage, RenderImageParams,
    RenderSvgParams, Scene, ShapedGlyph, ShapedRun, SharedString, Size, SvgRenderer,
    SystemWindowTab, Task, ThreadTaskTimings, Tray, TrayIconEvent, TrayMenuItem, Window,
    WindowControlArea, hash, point, px, size,
};
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

#[cfg(all(target_os = "macos", any(test, feature = "test-support")))]
pub use visual_test::VisualTestPlatform;

// TODO(jk): return an enum instead of a string
/// 猜测当前 Linux 桌面环境使用的显示服务器类型
/// 不会实际尝试连接显示服务器
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

/// 平台抽象层 trait，定义了与操作系统交互所需的所有接口
/// 各平台（Windows、macOS、Linux、Web）需实现此 trait 以提供平台特定功能
#[expect(missing_docs)]
pub trait Platform: 'static {
    fn background_executor(&self) -> BackgroundExecutor;
    fn foreground_executor(&self) -> ForegroundExecutor;
    fn text_system(&self) -> Arc<dyn PlatformTextSystem>;

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>);
    fn quit(&self);
    fn restart(&self, binary_path: Option<PathBuf>);
    fn activate(&self, ignoring_other_apps: bool);
    fn hide(&self);
    fn hide_other_apps(&self);
    fn unhide_other_apps(&self);

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>>;
    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>>;
    fn active_window(&self) -> Option<AnyWindowHandle>;
    fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        None
    }

    fn is_screen_capture_supported(&self) -> bool {
        false
    }

    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<anyhow::Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
        let (sources_tx, sources_rx) = oneshot::channel();
        sources_tx
            .send(Err(anyhow::anyhow!(
                "gpui was compiled without the screen-capture feature"
            )))
            .ok();
        sources_rx
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>>;

    /// 返回应用窗口的外观
    fn window_appearance(&self) -> WindowAppearance;

    /// 返回窗口按钮布局配置（部分平台支持）
    fn button_layout(&self) -> Option<WindowButtonLayout> {
        None
    }

    fn open_url(&self, url: &str);
    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>);
    fn register_url_scheme(&self, url: &str) -> Task<Result<()>>;

    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>>;
    fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>>;
    fn can_select_mixed_files_and_dirs(&self) -> bool;
    fn reveal_path(&self, path: &Path);
    fn open_with_system(&self, path: &Path);

    fn on_quit(&self, callback: Box<dyn FnMut()>);
    fn on_reopen(&self, callback: Box<dyn FnMut()>);

    fn set_menus(&self, menus: Vec<Menu>, keymap: &Keymap);
    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        None
    }

    fn set_dock_menu(&self, menu: Vec<MenuItem>, keymap: &Keymap);
    fn perform_dock_menu_action(&self, _action: usize) {}
    fn add_recent_document(&self, _path: &Path) {}
    fn update_jump_list(
        &self,
        _menus: Vec<MenuItem>,
        _entries: Vec<SmallVec<[PathBuf; 2]>>,
    ) -> Task<Vec<SmallVec<[PathBuf; 2]>>> {
        Task::ready(Vec::new())
    }

    // 系统托盘相关方法
    /// 设置系统托盘图标
    fn set_tray_icon(&self, _icon: Option<&[u8]>) {}
    /// 设置系统托盘菜单项
    fn set_tray_menu(&self, _menu: Vec<TrayMenuItem>) {}
    /// 设置系统托盘工具提示文本
    fn set_tray_tooltip(&self, _tooltip: &str) {}
    /// 启用或禁用托盘面板模式
    fn set_tray_panel_mode(&self, _enabled: bool) {}
    /// 获取托盘图标的屏幕边界坐标
    fn get_tray_icon_bounds(&self) -> Option<Bounds<Pixels>> {
        None
    }
    /// 注册托盘图标事件回调
    fn on_tray_icon_event(&self, _callback: Box<dyn FnMut(TrayIconEvent)>) {}
    /// 注册托盘菜单项点击事件回调
    fn on_tray_menu_action(&self, _callback: Box<dyn FnMut(SharedString)>) {}

    // 保留旧的 set_tray 方法以向后兼容
    fn set_tray(&self, _tray: Tray, _menu: Option<Vec<MenuItem>>, _keymap: &Keymap) {}

    fn set_keep_alive_without_windows(&self, _keep_alive: bool) {}

    /// 注册全局快捷键
    fn register_global_hotkey(&self, _id: u32, _keystroke: &Keystroke) -> Result<()> {
        Err(anyhow::anyhow!(
            "Global hotkeys not supported on this platform"
        ))
    }

    /// 注销全局快捷键
    fn unregister_global_hotkey(&self, _id: u32) {}

    /// 注册全局快捷键事件回调
    fn on_global_hotkey(&self, _callback: Box<dyn FnMut(u32)>) {}

    /// 显示系统原生通知
    fn show_notification(&self, _title: &str, _body: &str) -> Result<()> {
        Err(anyhow::anyhow!(
            "Notifications not supported on this platform"
        ))
    }

    /// 设置开机自启动
    fn set_auto_launch(&self, _app_id: &str, _enabled: bool) -> Result<()> {
        Err(anyhow::anyhow!(
            "Auto launch not supported on this platform"
        ))
    }

    /// 检查开机自启动是否已启用
    fn is_auto_launch_enabled(&self, _app_id: &str) -> bool {
        false
    }

    /// 获取当前聚焦窗口信息
    fn focused_window_info(&self) -> Option<FocusedWindowInfo> {
        None
    }

    /// 获取辅助功能权限状态
    fn accessibility_status(&self) -> PermissionStatus {
        PermissionStatus::Granted
    }

    /// 请求辅助功能权限
    fn request_accessibility_permission(&self) {}

    /// 获取麦克风权限状态
    fn microphone_status(&self) -> PermissionStatus {
        PermissionStatus::Granted
    }

    /// 请求麦克风权限
    fn request_microphone_permission(&self, callback: Box<dyn FnOnce(bool)>) {
        callback(true);
    }

    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>);
    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>);
    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>);

    fn thermal_state(&self) -> ThermalState;
    fn on_thermal_state_change(&self, callback: Box<dyn FnMut()>);

    fn compositor_name(&self) -> &'static str {
        ""
    }
    fn app_path(&self) -> Result<PathBuf>;
    fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf>;

    fn set_cursor_style(&self, style: CursorStyle);

    /// Hides the mouse cursor until the user moves the mouse over one of
    /// this application's windows.
    fn hide_cursor_until_mouse_moves(&self);

    /// Returns whether the mouse cursor is currently visible.
    fn is_cursor_visible(&self) -> bool;

    fn should_auto_hide_scrollbars(&self) -> bool;

    fn read_from_clipboard(&self) -> Option<ClipboardItem>;
    fn write_to_clipboard(&self, item: ClipboardItem);

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem>;
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, item: ClipboardItem);

    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem>;
    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, item: ClipboardItem);

    fn write_credentials(&self, url: &str, username: &str, password: &[u8]) -> Task<Result<()>>;
    fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>>;
    fn delete_credentials(&self, url: &str) -> Task<Result<()>>;

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout>;
    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper>;
    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>);

    /// 注册系统电源事件回调
    fn on_system_power_event(&self, _callback: Box<dyn FnMut(SystemPowerEvent)>) {}
    /// 启动电源阻止器，阻止系统进入省电模式
    fn start_power_save_blocker(&self, _kind: PowerSaveBlockerKind) -> Option<u32> {
        None
    }
    /// 停止电源阻止器
    fn stop_power_save_blocker(&self, _id: u32) {}
    /// 获取系统空闲时间
    fn system_idle_time(&self) -> Option<Duration> {
        None
    }
    /// 获取当前网络状态
    fn network_status(&self) -> NetworkStatus {
        NetworkStatus::Online
    }
    /// 注册网络状态变更回调
    fn on_network_status_change(&self, _callback: Box<dyn FnMut(NetworkStatus)>) {}
    /// 注册媒体键事件回调
    fn on_media_key_event(&self, _callback: Box<dyn FnMut(MediaKeyEvent)>) {}
    /// 请求用户注意力（如弹跳 Dock 图标）
    fn request_user_attention(&self, _attention_type: AttentionType) {}
    /// 取消用户注意力请求
    fn cancel_user_attention(&self) {}
    /// 设置 Dock 徽章（如 macOS 上的未读计数）
    fn set_dock_badge(&self, _label: Option<&str>) {}
    /// 在指定位置显示上下文菜单
    fn show_context_menu(
        &self,
        _position: Point<Pixels>,
        _items: Vec<TrayMenuItem>,
        _callback: Box<dyn FnMut(SharedString)>,
    ) {
    }
    /// 显示原生对话框
    fn show_dialog(&self, _options: DialogOptions) -> oneshot::Receiver<usize> {
        let (tx, rx) = oneshot::channel();
        tx.send(0).ok();
        rx
    }
    /// 获取操作系统信息
    fn os_info(&self) -> OsInfo {
        OsInfo {
            name: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            version: String::new().into(),
            locale: String::new().into(),
            hostname: String::new().into(),
        }
    }
    /// 获取生物识别认证状态
    fn biometric_status(&self) -> BiometricStatus {
        BiometricStatus::Unavailable
    }
    /// 发起生物识别认证
    fn authenticate_biometric(&self, _reason: &str, callback: Box<dyn FnOnce(bool) + Send>) {
        callback(false);
    }
}

/// 平台显示器句柄 trait，表示物理显示器或笔记本屏幕
pub trait PlatformDisplay: Debug {
    /// 获取显示器 ID
    fn id(&self) -> DisplayId;

    /// 返回可在系统重启后持久使用的稳定标识符
    fn uuid(&self) -> Result<Uuid>;

    /// 获取显示器边界
    fn bounds(&self) -> Bounds<Pixels>;

    /// 获取显示器可见边界，排除任务栏/停靠栏区域
    /// 这是窗口可以放置而不会被遮挡的可用区域
    /// 如果未重写则默认为完整显示器边界
    fn visible_bounds(&self) -> Bounds<Pixels> {
        self.bounds()
    }

    /// 获取在此显示器上放置窗口的默认边界
    fn default_bounds(&self) -> Bounds<Pixels> {
        let bounds = self.bounds();
        let center = bounds.center();
        let clipped_window_size = DEFAULT_WINDOW_SIZE.min(&bounds.size);

        let offset = clipped_window_size / 2.0;
        let origin = point(center.x - offset.width, center.y - offset.height);
        Bounds::new(origin, clipped_window_size)
    }
}

/// 系统热状态，表示 CPU/GPU 的热限制情况
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
    /// 屏幕的不透明标识符
    pub id: u64,
    /// 源的人类可读标签
    pub label: Option<SharedString>,
    /// 此源是否为主显示器
    pub is_main: Option<bool>,
    /// 源的视频分辨率
    pub resolution: Size<DevicePixels>,
}

/// 可捕获的屏幕视频内容源
pub trait ScreenCaptureSource {
    /// 返回此源的元数据
    fn metadata(&self) -> Result<SourceMetadata>;

    /// 开始从此源捕获视频，对每一帧调用给定的回调
    fn stream(
        &self,
        foreground_executor: &ForegroundExecutor,
        frame_callback: Box<dyn Fn(ScreenCaptureFrame) + Send>,
    ) -> oneshot::Receiver<Result<Box<dyn ScreenCaptureStream>>>;
}

/// 从屏幕捕获的视频流
pub trait ScreenCaptureStream {
    /// 返回此源的元数据
    fn metadata(&self) -> Result<SourceMetadata>;
}

/// 从屏幕捕获的视频帧
pub struct ScreenCaptureFrame(pub PlatformScreenCaptureFrame);

/// 硬件显示器的不透明标识符
#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub struct DisplayId(pub(crate) u64);

impl DisplayId {
    /// 从原始平台显示器标识符创建新的 `DisplayId`
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

/// 窗口可调整大小的边缘位置
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

/// 窗口装饰类型，决定使用服务端还是客户端装饰
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default)]
pub enum WindowDecorations {
    #[default]
    /// 服务端装饰（由窗口管理器绘制）
    Server,
    /// 客户端装饰（由应用自行绘制）
    Client,
}

/// 描述窗口当前的装饰配置
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

/// 平台支持的窗口控制按钮
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct WindowControls {
    /// 是否支持全屏
    pub fullscreen: bool,
    /// 是否支持最大化
    pub maximize: bool,
    /// 是否支持最小化
    pub minimize: bool,
    /// 是否支持窗口菜单
    pub window_menu: bool,
}

impl Default for WindowControls {
    fn default() -> Self {
        // 默认假设支持所有控制按钮
        Self {
            fullscreen: true,
            maximize: true,
            minimize: true,
            window_menu: true,
        }
    }
}

/// 标题栏中的窗口控制按钮类型
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
    /// 返回渲染此按钮时使用的稳定元素 ID
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

/// 标题栏每侧最多可放置的按钮数量
pub const MAX_BUTTONS_PER_SIDE: usize = 3;

/// 描述标题栏每侧出现哪些控制按钮
///
/// 在 Linux 上，此信息从桌面环境的配置中读取
/// （例如 GNOME 的 `gtk-decoration-layout` gsetting）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowButtonLayout {
    /// 标题栏左侧的按钮
    pub left: [Option<WindowButton>; MAX_BUTTONS_PER_SIDE],
    /// 标题栏右侧的按钮
    pub right: [Option<WindowButton>; MAX_BUTTONS_PER_SIDE],
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
impl WindowButtonLayout {
    /// 返回 Linux 标题栏的内置回退按钮布局
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

    /// 解析 GNOME 风格的 `button-layout` 字符串（例如 `"close,minimize:maximize"`）
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

    /// 将布局格式转换回 GNOME 风格的 `button-layout` 字符串
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

/// 描述窗口哪些边缘当前被平铺
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default)]
pub struct Tiling {
    /// 上边缘是否被平铺
    pub top: bool,
    /// 左边缘是否被平铺
    pub left: bool,
    /// 右边缘是否被平铺
    pub right: bool,
    /// 下边缘是否被平铺
    pub bottom: bool,
}

impl Tiling {
    /// 初始化所有边缘都被平铺的 [`Tiling`]
    pub fn tiled() -> Self {
        Self {
            top: true,
            left: true,
            right: true,
            bottom: true,
        }
    }

    /// 是否有任何边缘被平铺
    pub fn is_tiled(&self) -> bool {
        self.top || self.left || self.right || self.bottom
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
#[expect(missing_docs)]
pub struct RequestFrameOptions {
    /// 是否需要呈现
    pub require_presentation: bool,
    /// 为 true 时强制刷新所有渲染状态
    pub force_render: bool,
}

/// 平台窗口 trait，定义了窗口操作的核心接口
/// 各平台窗口实现需实现此 trait
#[expect(missing_docs)]
pub trait PlatformWindow: HasWindowHandle + HasDisplayHandle {
    fn bounds(&self) -> Bounds<Pixels>;
    fn is_maximized(&self) -> bool;
    fn window_bounds(&self) -> WindowBounds;
    fn content_size(&self) -> Size<Pixels>;
    fn resize(&mut self, size: Size<Pixels>);
    /// 设置窗口位置（保持大小不变）
    fn set_position(&mut self, position: Point<Pixels>);
    fn scale_factor(&self) -> f32;
    fn appearance(&self) -> WindowAppearance;
    fn display(&self) -> Option<Rc<dyn PlatformDisplay>>;
    fn mouse_position(&self) -> Point<Pixels>;
    fn modifiers(&self) -> Modifiers;
    fn capslock(&self) -> Capslock;
    fn set_input_handler(&mut self, input_handler: PlatformInputHandler);
    fn take_input_handler(&mut self) -> Option<PlatformInputHandler>;
    fn prompt(
        &self,
        level: PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>>;
    fn activate(&self);
    fn is_active(&self) -> bool;
    fn is_hovered(&self) -> bool;
    fn background_appearance(&self) -> WindowBackgroundAppearance;
    fn set_title(&mut self, title: &str);
    fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance);
    fn minimize(&self);
    fn hide(&self);
    fn zoom(&self);
    fn toggle_fullscreen(&self);
    fn is_fullscreen(&self) -> bool;
    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>);
    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>);
    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>);
    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>);
    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>);
    fn on_moved(&self, callback: Box<dyn FnMut()>);
    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>);
    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>);
    fn on_close(&self, callback: Box<dyn FnOnce()>);
    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>);
    fn on_button_layout_changed(&self, _callback: Box<dyn FnMut()>) {}
    fn draw(&self, scene: &Scene);
    fn completed_frame(&self) {}
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas>;
    fn is_subpixel_rendering_supported(&self) -> bool;

    // macOS specific methods
    fn get_title(&self) -> String {
        String::new()
    }
    fn tabbed_windows(&self) -> Option<Vec<SystemWindowTab>> {
        None
    }
    fn tab_bar_visible(&self) -> bool {
        false
    }
    fn set_edited(&mut self, _edited: bool) {}
    fn set_document_path(&self, _path: Option<&std::path::Path>) {}
    fn show_character_palette(&self) {}
    fn titlebar_double_click(&self) {}
    fn on_move_tab_to_new_window(&self, _callback: Box<dyn FnMut()>) {}
    fn on_merge_all_windows(&self, _callback: Box<dyn FnMut()>) {}
    fn on_select_previous_tab(&self, _callback: Box<dyn FnMut()>) {}
    fn on_select_next_tab(&self, _callback: Box<dyn FnMut()>) {}
    fn on_toggle_tab_bar(&self, _callback: Box<dyn FnMut()>) {}
    fn merge_all_windows(&self) {}
    fn move_tab_to_new_window(&self) {}
    fn toggle_window_tab_overview(&self) {}
    fn set_tabbing_identifier(&self, _identifier: Option<String>) {}

    #[cfg(target_os = "windows")]
    fn get_raw_handle(&self) -> windows::Win32::Foundation::HWND;

    // Linux specific methods
    fn inner_window_bounds(&self) -> WindowBounds {
        self.window_bounds()
    }
    fn request_decorations(&self, _decorations: WindowDecorations) {}
    fn show_window_menu(&self, _position: Point<Pixels>) {}
    fn start_window_move(&self) {}
    fn start_window_resize(&self, _edge: ResizeEdge) {}
    fn window_decorations(&self) -> Decorations {
        Decorations::Server
    }
    fn set_app_id(&mut self, _app_id: &str) {}
    fn map_window(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    fn window_controls(&self) -> WindowControls {
        WindowControls::default()
    }
    fn set_client_inset(&self, _inset: Pixels) {}
    fn gpu_specs(&self) -> Option<GpuSpecs>;

    fn update_ime_position(&self, _bounds: Bounds<Pixels>);

    fn play_system_bell(&self) {}

    /// 显示窗口（与 hide 相对）
    fn show(&self) {}
    /// 检查窗口当前是否可见
    fn is_visible(&self) -> bool {
        true
    }
    /// 设置任务栏/程序坞进度条状态
    fn set_progress_bar(&self, _state: ProgressBarState) {}

    /// 设置窗口是否允许鼠标事件穿透到后面的窗口
    fn set_mouse_passthrough(&self, _passthrough: bool) {}

    /// 设置标题栏和边框是否可见
    fn set_titlebar_visible(&self, _visible: bool) {}

    #[cfg(any(test, feature = "test-support"))]
    fn as_test(&mut self) -> Option<&mut TestWindow> {
        None
    }

    /// 将场景渲染到纹理并返回 RGBA 图像的像素数据
    /// 不会将帧呈现到屏幕 - 用于视觉测试，在不显示窗口的情况下捕获渲染内容
    #[cfg(any(test, feature = "test-support"))]
    fn render_to_image(&self, _scene: &Scene) -> Result<RgbaImage> {
        anyhow::bail!("render_to_image not implemented for this platform")
    }
}

/// 无头窗口的渲染器，可生成真实的渲染输出
#[cfg(any(test, feature = "test-support"))]
pub trait PlatformHeadlessRenderer {
    /// 渲染场景并返回 RGBA 图像
    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> Result<RgbaImage>;

    /// 返回此渲染器使用的精灵图集
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas>;
}

/// 带元数据的可运行任务类型别名
#[doc(hidden)]
pub type RunnableVariant = Runnable<RunnableMeta>;

#[doc(hidden)]
pub type TimerResolutionGuard = crate::rgpui_util::Deferred<Box<dyn FnOnce() + Send>>;

/// 平台分发器 trait，负责任务调度和执行
/// 此类型公开是为了让测试宏可以生成和使用它，但不属于公共 API
#[doc(hidden)]
pub trait PlatformDispatcher: Send + Sync {
    fn get_all_timings(&self) -> Vec<ThreadTaskTimings>;
    fn get_current_thread_timings(&self) -> ThreadTaskTimings;
    fn is_main_thread(&self) -> bool;
    fn dispatch(&self, runnable: RunnableVariant, priority: Priority);
    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority);
    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant);

    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>);

    fn now(&self) -> Instant {
        Instant::now()
    }

    fn increase_timer_resolution(&self) -> TimerResolutionGuard {
        crate::defer(Box::new(|| {}))
    }

    #[cfg(any(test, feature = "test-support"))]
    fn as_test(&self) -> Option<&TestDispatcher> {
        None
    }
}

/// 平台文本系统 trait，负责字体加载、字形渲染和文本布局
#[expect(missing_docs)]
pub trait PlatformTextSystem: Send + Sync {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()>;
    /// 获取所有可用字体名称
    fn all_font_names(&self) -> Vec<String>;
    /// 获取字体描述符对应的字体 ID
    fn font_id(&self, descriptor: &Font) -> Result<FontId>;
    /// 获取字体度量信息
    fn font_metrics(&self, font_id: FontId) -> FontMetrics;
    /// 获取字形的排版边界
    fn typographic_bounds(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Bounds<f32>>;
    /// 获取字形的前进宽度
    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>>;
    /// 获取字符对应的字形 ID
    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId>;
    /// 获取字形的栅格边界
    fn glyph_raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>>;
    /// 栅格化字形
    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)>;
    /// 使用给定的字体运行信息布局一行文本
    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout;
    /// 返回给定字体和大小的推荐文本渲染模式
    fn recommended_rendering_mode(&self, _font_id: FontId, _font_size: Pixels)
    -> TextRenderingMode;
    /// 返回使用给定颜色绘制字形时应使用的膨胀级别
    fn glyph_dilation_for_color(&self, _color: Hsla) -> u8 {
        0
    }
}

#[expect(missing_docs)]
pub struct NoopTextSystem;

#[expect(missing_docs)]
impl NoopTextSystem {
    #[allow(dead_code)]
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

// 改编自 https://github.com/microsoft/terminal/blob/1283c0f5b99a2961673249fa77c6b986efb5086c/src/renderer/atlas/dwrite.cpp
// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
/// 计算子像素文本渲染的伽马校正比率
#[allow(dead_code)]
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

#[derive(PartialEq, Eq, Hash, Clone)]
#[expect(missing_docs)]
pub enum AtlasKey {
    Glyph(RenderGlyphParams),
    Svg(RenderSvgParams),
    Image(RenderImageParams),
}

impl AtlasKey {
    #[cfg_attr(
        all(
            any(target_os = "linux", target_os = "freebsd"),
            not(any(feature = "x11", feature = "wayland"))
        ),
        allow(dead_code)
    )]
    /// 返回此图集键对应的纹理类型
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

/// 平台图集 trait，用于管理纹理图集的插入和移除
#[expect(missing_docs)]
pub trait PlatformAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>>;
    fn remove(&self, key: &AtlasKey);
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

    #[allow(dead_code)]
    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut T> {
        self.textures.iter_mut().flatten()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
#[expect(missing_docs)]
pub struct AtlasTile {
    /// 此图块所属的纹理
    pub texture_id: AtlasTextureId,
    /// 图块在其纹理中的唯一 ID
    pub tile_id: TileId,
    /// 图块内容周围的填充像素
    pub padding: u32,
    /// 图块在纹理中的边界
    pub bounds: Bounds<DevicePixels>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
#[expect(missing_docs)]
pub struct AtlasTextureId {
    // 使用 u32 而非 usize 以兼容 Metal 着色语言
    /// 此纹理在图集中的索引
    pub index: u32,
    /// 此纹理存储的内容类型
    pub kind: AtlasTextureKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
#[expect(missing_docs)]
pub enum AtlasTextureKind {
    Monochrome = 0,
    Polychrome = 1,
    Subpixel = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
#[expect(missing_docs)]
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

#[expect(missing_docs)]
pub struct PlatformInputHandler {
    cx: AsyncWindowContext,
    handler: Box<dyn InputHandler>,
}

#[expect(missing_docs)]
#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
impl PlatformInputHandler {
    pub fn new(cx: AsyncWindowContext, handler: Box<dyn InputHandler>) -> Self {
        Self { cx, handler }
    }

    pub fn selected_text_range(&mut self, ignore_disabled_input: bool) -> Option<UTF16Selection> {
        self.cx
            .update(|window, cx| {
                self.handler
                    .selected_text_range(ignore_disabled_input, window, cx)
            })
            .ok()
            .flatten()
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn marked_text_range(&mut self) -> Option<Range<usize>> {
        self.cx
            .update(|window, cx| self.handler.marked_text_range(window, cx))
            .ok()
            .flatten()
    }

    #[cfg_attr(
        any(target_os = "linux", target_os = "freebsd", target_os = "windows"),
        allow(dead_code)
    )]
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

    pub fn replace_text_in_range(&mut self, replacement_range: Option<Range<usize>>, text: &str) {
        self.cx
            .update(|window, cx| {
                self.handler
                    .replace_text_in_range(replacement_range, text, window, cx);
            })
            .ok();
    }

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

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn unmark_text(&mut self) {
        self.cx
            .update(|window, cx| self.handler.unmark_text(window, cx))
            .ok();
    }

    pub fn bounds_for_range(&mut self, range_utf16: Range<usize>) -> Option<Bounds<Pixels>> {
        self.cx
            .update(|window, cx| self.handler.bounds_for_range(range_utf16, window, cx))
            .ok()
            .flatten()
    }

    #[allow(dead_code)]
    pub fn apple_press_and_hold_enabled(&mut self) -> bool {
        self.handler.apple_press_and_hold_enabled()
    }

    pub fn dispatch_input(&mut self, input: &str, window: &mut Window, cx: &mut App) {
        self.handler.replace_text_in_range(None, input, window, cx);
    }

    pub fn selected_bounds(&mut self, window: &mut Window, cx: &mut App) -> Option<Bounds<Pixels>> {
        let selection = self.handler.selected_text_range(true, window, cx)?;
        self.handler.bounds_for_range(
            if selection.reversed {
                selection.range.start..selection.range.start
            } else {
                selection.range.end..selection.range.end
            },
            window,
            cx,
        )
    }

    #[allow(unused)]
    pub fn character_index_for_point(&mut self, point: Point<Pixels>) -> Option<usize> {
        self.cx
            .update(|window, cx| self.handler.character_index_for_point(point, window, cx))
            .ok()
            .flatten()
    }

    #[allow(dead_code)]
    pub fn accepts_text_input(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.handler.accepts_text_input(window, cx)
    }

    #[allow(dead_code)]
    pub fn query_accepts_text_input(&mut self) -> bool {
        self.cx
            .update(|window, cx| self.handler.accepts_text_input(window, cx))
            .unwrap_or(true)
    }

    #[allow(dead_code)]
    pub fn query_prefers_ime_for_printable_keys(&mut self) -> bool {
        self.cx
            .update(|window, cx| self.handler.prefers_ime_for_printable_keys(window, cx))
            .unwrap_or(false)
    }
}

/// 文本缓冲区中表示选区的结构，使用 UTF16 字符
/// 与普通的 Range 不同，因为选区的头部可能在尾部之前
#[derive(Debug)]
pub struct UTF16Selection {
    /// 此选区对应的文档范围（UTF-16 字符）
    pub range: Range<usize>,
    /// 选区头部是否在范围的起始位置（true），还是末尾位置（false）
    pub reversed: bool,
}

/// Zed 处理平台 IME 系统文本输入的接口
/// 当前为 NSTextInputClient API 的 1:1 映射：
///
/// <https://developer.apple.com/documentation/appkit/nstextinputclient>
pub trait InputHandler: 'static {
    /// 获取用户当前选中的文本范围（如有）
    /// 对应 [selectedRange()](https://developer.apple.com/documentation/appkit/nstextinputclient/1438242-selectedrange)
    ///
    /// 返回值为 UTF-16 字符范围，从 0 到文档长度
    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<UTF16Selection>;

    /// 获取当前标记文本的范围（如有）
    /// 对应 [markedRange()](https://developer.apple.com/documentation/appkit/nstextinputclient/1438250-markedrange)
    ///
    /// 返回值为 UTF-16 字符范围，从 0 到文档长度
    fn marked_text_range(&mut self, window: &mut Window, cx: &mut App) -> Option<Range<usize>>;

    /// 获取给定文档范围的 UTF-16 文本
    /// 对应 [attributedSubstring(forProposedRange: actualRange:)](https://developer.apple.com/documentation/appkit/nstextinputclient/1438238-attributedsubstring)
    ///
    /// range_utf16 为 UTF-16 字符范围
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<String>;

    /// 用给定文本替换指定文档范围的内容
    /// 对应 [insertText(_:replacementRange:)](https://developer.apple.com/documentation/appkit/nstextinputclient/1438258-inserttext)
    ///
    /// replacement_range 为 UTF-16 字符范围
    fn replace_text_in_range(
        &mut self,
        replacement_range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    );

    /// 用给定文本替换指定文档范围的内容，并将文本标记为 IME '组合' 状态
    /// 对应 [setMarkedText(_:selectedRange:replacementRange:)](https://developer.apple.com/documentation/appkit/nstextinputclient/1438246-setmarkedtext)
    ///
    /// range_utf16 为 UTF-16 字符范围
    /// new_selected_range 为 UTF-16 字符范围
    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    );

    /// 从文档中移除 IME '组合' 状态
    /// 对应 [unmarkText()](https://developer.apple.com/documentation/appkit/nstextinputclient/1438239-unmarktext)
    fn unmark_text(&mut self, window: &mut Window, cx: &mut App);

    /// 获取给定文档范围在屏幕坐标系中的边界
    /// 对应 [firstRect(forCharacterRange:actualRange:)](https://developer.apple.com/documentation/appkit/nstextinputclient/1438240-firstrect)
    ///
    /// 用于定位 IME 候选窗口
    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Bounds<Pixels>>;

    /// 获取给定点对应的字符偏移（UTF16 字符）
    ///
    /// 对应 [characterIndexForPoint:](https://developer.apple.com/documentation/appkit/nstextinputclient/characterindex(for:))
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<usize>;

    /// 允许输入上下文选择接收原始按键重复事件而非发送到平台
    /// TODO: 理想情况下我们应该能够在 NSUserDefaults 中设置 ApplePressAndHoldEnabled
    /// （iTerm 就是这样做的），但目前不起作用
    #[allow(dead_code)]
    fn apple_press_and_hold_enabled(&mut self) -> bool {
        true
    }

    /// 返回此处理器是否接受文本输入
    fn accepts_text_input(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        true
    }

    /// 返回在非 ASCII 输入源（如日文、韩文、中文 IME）激活时，
    /// 可打印按键是否应在按键绑定匹配之前路由到 IME。
    /// 这防止了像 `jj` 这样的多击按键绑定拦截 IME 应该组合的按键。
    ///
    /// 默认为 `false`。编辑器根据是否期望字符输入来覆盖此值
    /// （例如 Vim 插入模式返回 `true`，普通模式返回 `false`）。
    /// 终端保持默认 `false` 以便原始按键能到达终端进程。
    fn prefers_ime_for_printable_keys(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        false
    }
}

/// 创建新窗口时可配置的选项
#[derive(Debug)]
pub struct WindowOptions {
    /// 指定窗口在屏幕坐标系中的状态和边界
    /// - `None`: 继承当前边界
    /// - `Some(WindowBounds)`: 使用指定的状态和恢复大小打开窗口
    pub window_bounds: Option<WindowBounds>,

    /// 窗口标题栏配置
    pub titlebar: Option<TitlebarOptions>,

    /// 创建时是否获取焦点
    pub focus: bool,

    /// 创建时是否显示
    pub show: bool,

    /// 窗口类型
    pub kind: WindowKind,

    /// 用户是否可移动窗口
    pub is_movable: bool,

    /// 用户是否可调整窗口大小
    pub is_resizable: bool,

    /// 用户是否可最小化窗口
    pub is_minimizable: bool,

    /// 创建窗口的显示器，为 None 时使用主显示器
    pub display_id: Option<DisplayId>,

    /// 窗口背景外观
    pub window_background: WindowBackgroundAppearance,

    /// 窗口应用标识符，桌面环境可用于分组应用
    pub app_id: Option<String>,

    /// 窗口最小尺寸
    pub window_min_size: Option<Size<Pixels>>,

    /// 使用客户端还是服务端装饰，仅 Wayland 有效
    /// 注意：此设置可能被忽略
    pub window_decorations: Option<WindowDecorations>,

    /// 窗口图标（仅 X11）
    pub icon: Option<Arc<image::RgbaImage>>,

    /// 标签组名称，允许在 macOS 10.12+ 上将窗口作为原生标签打开
    /// 具有相同标签标识符的窗口会被分组
    pub tabbing_identifier: Option<String>,

    /// 是否允许鼠标事件穿透到后面的窗口
    pub mouse_passthrough: bool,
}

/// 创建新窗口时传递给平台的参数
#[derive(Debug)]
#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
#[allow(missing_docs)]
pub struct WindowParams {
    pub bounds: Bounds<Pixels>,

    /// The titlebar configuration of the window
    #[cfg_attr(feature = "wayland", allow(dead_code))]
    pub titlebar: Option<TitlebarOptions>,

    /// The kind of window to create
    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub kind: WindowKind,

    /// Whether the window should be movable by the user
    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub is_movable: bool,

    /// Whether the window should be resizable by the user
    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub is_resizable: bool,

    /// Whether the window should be minimized by the user
    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub is_minimizable: bool,

    #[cfg_attr(
        any(target_os = "linux", target_os = "freebsd", target_os = "windows"),
        allow(dead_code)
    )]
    pub focus: bool,

    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub show: bool,

    /// 窗口图标（仅 X11）
    #[cfg_attr(feature = "wayland", allow(dead_code))]
    pub icon: Option<Arc<image::RgbaImage>>,

    #[cfg_attr(feature = "wayland", allow(dead_code))]
    pub display_id: Option<DisplayId>,

    pub window_min_size: Option<Size<Pixels>>,
    #[cfg(target_os = "macos")]
    pub tabbing_identifier: Option<String>,
    #[cfg_attr(
        any(target_os = "linux", target_os = "freebsd", target_os = "macos"),
        allow(dead_code)
    )]
    pub window_decorations: WindowDecorations,

    /// 是否允许鼠标事件穿透到后面的窗口
    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub mouse_passthrough: bool,
}

/// 表示窗口的打开状态
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum WindowBounds {
    /// 窗口以给定边界打开
    Windowed(Bounds<Pixels>),
    /// 窗口以最大化状态打开，边界为恢复大小
    Maximized(Bounds<Pixels>),
    /// 窗口以全屏状态打开，边界为恢复大小
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

    /// 创建在屏幕上居中的窗口边界
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
            mouse_passthrough: false,
        }
    }
}

/// 窗口标题栏可配置的选项
#[derive(Debug, Default)]
pub struct TitlebarOptions {
    /// 窗口的初始标题
    pub title: Option<SharedString>,

    /// 是否隐藏系统默认标题栏以使用自定义绘制的标题栏（仅 macOS 和 Windows）
    /// Linux 请参考 [`WindowOptions::window_decorations`]
    pub appears_transparent: bool,

    /// macOS 红绿灯按钮的位置
    pub traffic_light_position: Option<Point<Pixels>>,
}

/// 要创建的窗口类型
#[derive(Clone, Debug, PartialEq)]
pub enum WindowKind {
    /// 普通应用窗口
    Normal,

    /// 始终在其他窗口上方的窗口，通常用于警告或弹出窗口
    /// 请谨慎使用！
    PopUp,

    /// 浮动在父窗口上方的窗口
    Floating,

    /// Wayland LayerShell 窗口，用于绘制叠加层或背景
    /// 适用于坞、通知或壁纸等应用
    #[cfg(all(target_os = "linux", feature = "wayland"))]
    LayerShell(layer_shell::LayerShellOptions),

    /// 浮动在父窗口上方并阻止与其交互的窗口
    /// 直到模态窗口关闭
    Dialog,

    /// 覆盖窗口，用于全局覆盖层、屏幕标注或透明 UI
    /// 特性：始终置顶、无装饰
    /// 鼠标穿透通过 [`WindowOptions::mouse_passthrough`] 控制
    /// 透明度通过 [`WindowOptions::window_background`] 控制
    Overlay,
}

/// 窗口外观，由操作系统定义
///
/// 在 macOS 上，这对应命名的 [`NSAppearance`](https://developer.apple.com/documentation/appkit/nsappearance) 值
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum WindowAppearance {
    /// 浅色外观
    ///
    /// 在 macOS 上对应 `aqua` 外观
    #[default]
    Light,

    /// 浅色鲜艳外观
    ///
    /// 在 macOS 上对应 `NSAppearanceNameVibrantLight` 外观
    VibrantLight,

    /// 深色外观
    ///
    /// 在 macOS 上对应 `darkAqua` 外观
    Dark,

    /// 深色鲜艳外观
    ///
    /// 在 macOS 上对应 `NSAppearanceNameVibrantDark` 外观
    VibrantDark,
}

/// 窗口背景的外观，当没有内容或内容透明时使用
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum WindowBackgroundAppearance {
    /// 不透明
    ///
    /// 这告知窗口管理器不需要绘制此窗口后面的内容
    ///
    /// 实际颜色取决于系统，主题应定义完全不透明的背景色
    #[default]
    Opaque,
    /// 纯 Alpha 透明
    Transparent,
    /// 透明，但窗口后面的内容会被模糊
    ///
    /// 并非所有平台都支持
    Blurred,
    /// 云母背景材质，仅 Windows 11 支持
    MicaBackdrop,
    /// 云母 Alt 背景材质，仅 Windows 11 支持
    MicaAltBackdrop,
}

/// 绘制字形时使用的文本渲染模式
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum TextRenderingMode {
    /// 使用平台默认的文本渲染模式
    #[default]
    PlatformDefault,
    /// 使用子像素（ClearType 风格）文本渲染
    Subpixel,
    /// 使用灰度文本渲染
    Grayscale,
}

/// 文件对话框提示的可配置选项
#[derive(Clone, Debug)]
pub struct PathPromptOptions {
    /// 是否允许选择文件
    pub files: bool,
    /// 是否允许选择目录
    pub directories: bool,
    /// 是否允许多选
    pub multiple: bool,
    /// 选择路径时显示给用户的提示文字
    pub prompt: Option<SharedString>,
}

/// 提示框的样式级别
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PromptLevel {
    /// 通知用户某些信息
    Info,

    /// 警告用户存在潜在问题
    Warning,

    /// 发生了严重问题
    Critical,
}

/// 提示框按钮
#[derive(Clone, Debug, PartialEq)]
pub enum PromptButton {
    /// 确定按钮
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

    /// 创建确定按钮
    pub fn ok(label: impl Into<SharedString>) -> Self {
        PromptButton::Ok(label.into())
    }

    /// 创建取消按钮
    pub fn cancel(label: impl Into<SharedString>) -> Self {
        PromptButton::Cancel(label.into())
    }

    /// 返回此按钮是否为取消按钮
    #[allow(dead_code)]
    pub fn is_cancel(&self) -> bool {
        matches!(self, PromptButton::Cancel(_))
    }

    /// 返回按钮标签
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
            "ok" => PromptButton::Ok("Ok".into()),
            "cancel" => PromptButton::Cancel("Cancel".into()),
            _ => PromptButton::Other(SharedString::from(value.to_owned())),
        }
    }
}

/// 鼠标指针样式
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum CursorStyle {
    /// 默认箭头指针
    #[default]
    Arrow,

    /// 文本输入光标
    /// 对应 CSS cursor 值 `text`
    IBeam,

    /// 十字准星光标
    /// 对应 CSS cursor 值 `crosshair`
    Crosshair,

    /// 闭合手形光标
    /// 对应 CSS cursor 值 `grabbing`
    ClosedHand,

    /// 张开手形光标
    /// 对应 CSS cursor 值 `grab`
    OpenHand,

    /// 指向手形光标
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

    /// 左上-右下调整大小光标
    /// 对应 CSS cursor 值 `nesw-resize`
    ResizeUpLeftDownRight,

    /// 右上-左下调整大小光标
    /// 对应 CSS cursor 值 `nwse-resize`
    ResizeUpRightDownLeft,

    /// 表示项目/列可水平调整大小的光标
    /// 对应 CSS cursor 值 `col-resize`
    ResizeColumn,

    /// 表示项目/行可垂直调整大小的光标
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

/// 应复制到剪贴板的项目
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardItem {
    /// 此剪贴板项目中的条目
    pub entries: Vec<ClipboardEntry>,
}

/// 剪贴板条目类型，可以是文本、图片或外部文件路径
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardEntry {
    /// 文本条目
    String(ClipboardString),
    /// 图片条目
    Image(Image),
    /// 外部文件路径条目
    ExternalPaths(crate::ExternalPaths),
}

impl ClipboardItem {
    /// 创建不带元数据的文本剪贴板项目
    pub fn new_string(text: String) -> Self {
        Self {
            entries: vec![ClipboardEntry::String(ClipboardString::new(text))],
        }
    }

    /// 创建带元数据的文本剪贴板项目
    pub fn new_string_with_metadata(text: String, metadata: String) -> Self {
        Self {
            entries: vec![ClipboardEntry::String(ClipboardString {
                text,
                metadata: Some(metadata),
            })],
        }
    }

    /// 创建带 JSON 元数据的文本剪贴板项目
    pub fn new_string_with_json_metadata<T: Serialize>(text: String, metadata: T) -> Self {
        Self {
            entries: vec![ClipboardEntry::String(
                ClipboardString::new(text).with_json_metadata(metadata),
            )],
        }
    }

    /// 创建不带元数据的图片剪贴板项目
    pub fn new_image(image: &Image) -> Self {
        Self {
            entries: vec![ClipboardEntry::Image(image.clone())],
        }
    }

    /// 拼接项目中所有文本条目
    /// 如果没有文本条目则返回 None
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

    /// 如果项目是单个文本条目，返回其元数据
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
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

/// One of the editor's supported image formats (e.g. PNG, JPEG) - used when dealing with images in the clipboard
#[derive(Clone, Copy, Debug, Eq, PartialEq, EnumIter, Hash)]
pub enum ImageFormat {
    // Sorted from most to least likely to be pasted into an editor,
    // which matters when we iterate through them trying to see if
    // clipboard content matches them.
    /// .png
    Png,
    /// .jpeg or .jpg
    Jpeg,
    /// .webp
    Webp,
    /// .gif
    Gif,
    /// .svg
    Svg,
    /// .bmp
    Bmp,
    /// .tif or .tiff
    Tiff,
    /// .ico
    Ico,
    /// Netpbm image formats (.pbm, .ppm, .pgm).
    Pnm,
}

impl ImageFormat {
    /// Returns the mime type for the ImageFormat
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

    /// Returns the ImageFormat for the given mime type, including known aliases.
    pub fn from_mime_type(mime_type: &str) -> Option<Self> {
        use strum::IntoEnumIterator;
        Self::iter()
            .find(|format| format.mime_type() == mime_type)
            .or_else(|| Self::from_mime_type_alias(mime_type))
    }

    /// Non-canonical mime types that some producers use in the wild.
    /// Unlike `mime_type()` which returns the single canonical form,
    /// these are legacy or shortened variants we still need to recognize.
    fn from_mime_type_alias(mime_type: &str) -> Option<Self> {
        match mime_type {
            "image/jpg" => Some(Self::Jpeg),
            "image/tif" => Some(Self::Tiff),
            _ => None,
        }
    }
}

/// An image, with a format and certain bytes
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// The image format the bytes represent (e.g. PNG)
    pub format: ImageFormat,
    /// The raw image bytes
    pub bytes: Vec<u8>,
    /// The unique ID for the image
    pub id: u64,
}

impl Hash for Image {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.id);
    }
}

impl Image {
    /// An empty image containing no data
    pub fn empty() -> Self {
        Self::from_bytes(ImageFormat::Png, Vec::new())
    }

    /// Create an image from a format and bytes
    pub fn from_bytes(format: ImageFormat, bytes: Vec<u8>) -> Self {
        Self {
            id: hash(&bytes),
            format,
            bytes,
        }
    }

    /// Get this image's ID
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Use the GPUI `use_asset` API to make this image renderable
    pub fn use_render_image(
        self: Arc<Self>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Arc<RenderImage>> {
        ImageSource::Image(self)
            .use_data(None, window, cx)
            .and_then(|result| result.ok())
    }

    /// Use the GPUI `get_asset` API to make this image renderable
    pub fn get_render_image(
        self: Arc<Self>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Arc<RenderImage>> {
        ImageSource::Image(self)
            .get_data(None, window, cx)
            .and_then(|result| result.ok())
    }

    /// Use the GPUI `remove_asset` API to drop this image, if possible.
    pub fn remove_asset(self: Arc<Self>, cx: &mut App) {
        ImageSource::Image(self).remove_asset(cx);
    }

    /// Convert the clipboard image to an `ImageData` object.
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

    /// Get the format of the clipboard image
    pub fn format(&self) -> ImageFormat {
        self.format
    }

    /// Get the raw bytes of the clipboard image
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

/// A clipboard item that should be copied to the clipboard
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardString {
    /// The text content.
    pub text: String,
    /// Optional metadata associated with this clipboard string.
    pub metadata: Option<String>,
}

impl ClipboardString {
    /// Create a new clipboard string with the given text
    pub fn new(text: String) -> Self {
        Self {
            text,
            metadata: None,
        }
    }

    /// Return a new clipboard item with the metadata replaced by the given metadata,
    /// after serializing it as JSON.
    pub fn with_json_metadata<T: Serialize>(mut self, metadata: T) -> Self {
        self.metadata = Some(serde_json::to_string(&metadata).unwrap());
        self
    }

    /// Get the text of the clipboard string
    pub fn text(&self) -> &String {
        &self.text
    }

    /// Get the owned text of the clipboard string
    pub fn into_text(self) -> String {
        self.text
    }

    /// Get the metadata of the clipboard string, formatted as JSON
    pub fn metadata_json<T>(&self) -> Option<T>
    where
        T: for<'a> Deserialize<'a>,
    {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
    }

    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    /// Compute a hash of the given text for clipboard change detection.
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

/// 语义化窗口位置，用于相对于屏幕定位窗口
#[derive(Debug, Clone, PartialEq)]
pub enum WindowPosition {
    /// 在主显示器居中显示窗口
    Center,
    /// 在指定显示器上居中
    CenterOnDisplay(DisplayId),
    /// 在托盘图标区域上方居中显示
    TrayCenter(Bounds<Pixels>),
    /// 定位在右上角
    TopRight {
        /// 距离屏幕边缘的边距
        margin: Pixels,
    },
    /// 定位在右下角
    BottomRight {
        /// 距离屏幕边缘的边距
        margin: Pixels,
    },
    /// 定位在左上角
    TopLeft {
        /// 距离屏幕边缘的边距
        margin: Pixels,
    },
    /// 定位在左下角
    BottomLeft {
        /// 距离屏幕边缘的边距
        margin: Pixels,
    },
}

/// 当前系统聚焦窗口的信息
#[derive(Debug, Clone)]
pub struct FocusedWindowInfo {
    /// 应用程序名称
    pub app_name: String,
    /// 窗口标题
    pub window_title: String,
    /// macOS 专属：应用 Bundle ID
    pub bundle_id: Option<String>,
    /// 进程 ID
    pub pid: Option<u32>,
}

/// 权限类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionType {
    /// 辅助功能权限
    Accessibility,
    /// 屏幕录制权限
    ScreenCapture,
    /// 输入监控权限
    InputMonitoring,
}

/// 权限状态枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionStatus {
    /// 权限已授予
    Granted,
    /// 权限被拒绝
    Denied,
    /// 权限尚未确定（用户未做出选择）
    NotDetermined,
}

/// 全局快捷键事件
#[derive(Debug, Clone)]
pub struct GlobalHotKeyEvent {
    /// 快捷键的唯一标识符
    pub id: u32,
}

/// 系统电源状态变更事件
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemPowerEvent {
    /// 系统即将休眠
    Suspend,
    /// 系统已从休眠中恢复
    Resume,
    /// 屏幕已锁定
    LockScreen,
    /// 屏幕已解锁
    UnlockScreen,
    /// 系统正在关机
    Shutdown,
}

/// 电源阻止器类型，用于阻止系统进入省电模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerSaveBlockerKind {
    /// 阻止应用被挂起
    PreventAppSuspension,
    /// 阻止显示器进入睡眠
    PreventDisplaySleep,
}

/// 当前网络连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkStatus {
    /// 系统有网络连接
    Online,
    /// 系统无网络连接
    Offline,
}

/// 媒体键事件，来自硬件媒体键或系统媒体控件
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKeyEvent {
    /// 播放媒体
    Play,
    /// 暂停媒体
    Pause,
    /// 切换播放/暂停
    PlayPause,
    /// 停止媒体播放
    Stop,
    /// 跳转到下一曲目
    NextTrack,
    /// 跳转到上一曲目
    PreviousTrack,
}

/// 请求用户注意力的类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttentionType {
    /// 信息性请求（例如弹跳 Dock 图标一次）
    Informational,
    /// 关键请求（例如持续弹跳 Dock 图标）
    Critical,
}

/// 任务栏/程序坞进度条状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProgressBarState {
    /// 不显示进度条
    None,
    /// 显示不确定进度条
    Indeterminate,
    /// 普通进度条，值为 0.0 到 1.0
    Normal(f64),
    /// 错误进度条，值为 0.0 到 1.0
    Error(f64),
    /// 暂停进度条，值为 0.0 到 1.0
    Paused(f64),
}

/// 原生对话框类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    /// 信息对话框
    Info,
    /// 警告对话框
    Warning,
    /// 错误对话框
    Error,
}

/// 原生对话框的选项
#[derive(Debug, Clone)]
pub struct DialogOptions {
    /// 对话框类型
    pub kind: DialogKind,
    /// 对话框标题
    pub title: SharedString,
    /// 对话框主消息
    pub message: SharedString,
    /// 消息下方的可选详细信息
    pub detail: Option<SharedString>,
    /// 对话框按钮标签
    pub buttons: Vec<SharedString>,
}

/// 操作系统信息
#[derive(Debug, Clone)]
pub struct OsInfo {
    /// 操作系统名称（如 "linux"）
    pub name: SharedString,
    /// 操作系统版本
    pub version: SharedString,
    /// CPU 架构（如 "x86_64"）
    pub arch: SharedString,
    /// 系统区域设置（如 "en-US"）
    pub locale: SharedString,
    /// 系统主机名
    pub hostname: SharedString,
}

/// 可用的生物识别认证类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiometricKind {
    /// macOS Touch ID
    TouchId,
    /// Windows Hello
    WindowsHello,
    /// 通用指纹识别器
    Fingerprint,
}

/// 生物识别认证的可用性状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiometricStatus {
    /// 生物识别认证可用，指定了具体类型
    Available(BiometricKind),
    /// 生物识别认证不可用
    Unavailable,
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
    use std::collections::HashSet;

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
