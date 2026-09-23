//! Android 平台实现（1.4.0 移动端，Android 优先）。
//!
//! M2 在 `android-activity` 单路径上打通真机：`bridge`（入口 + 事件循环）、
//! `window`（`ANativeWindow` + wgpu 三态）、输入（触摸扇出 + 按键）、
//! 字体（系统字体 + CBDT Emoji）、显示（安全区 + 深浅色）。
//! 详见 `docs/1.4.0/1.4.0-dev-plan.md` 的 M2 节。
//! Android 特有依赖一律 `cfg(target_os = "android")` 门控，主机可测。

pub mod dispatcher;
pub mod display;
pub mod fling_guard;
pub mod gestures;
#[cfg(target_os = "android")]
pub mod ime;
pub mod keyboard;
pub mod platform;
pub mod platform_view;
pub mod window;

/// 帧节拍（`frame_source` 的平台无关内核；仅真机编译，单测随真机 target 编译检查）。
#[cfg(target_os = "android")]
pub mod frame_pacer;
/// vsync 帧源（仅真机；主机无循环线程，`window.rs` 相关调用已 `cfg` 掉）。
#[cfg(target_os = "android")]
pub mod frame_source;

#[cfg(target_os = "android")]
pub mod bridge;

pub use dispatcher::AndroidDispatcher;
pub use display::AndroidDisplay;
pub use platform::{AndroidPlatform, SharedPlatform};
pub use window::{AndroidPlatformWindow, AndroidWindow, SafeAreaInsets};

use std::rc::Rc;

/// 返回 Android 平台实现（与 `rgpui_linux::current_platform` 同签名）。
///
/// /// `headless` 为 true 时不触碰任何原生窗口，专供主机单测与文档构建。
pub fn current_platform(headless: bool) -> Rc<dyn rgpui::Platform> {
    Rc::new(AndroidPlatform::new(headless))
}

// ── GPU 后端选择 ─────────────────────────────────────────────────────────────

/// Android 的 GPU 后端偏好（Vulkan 优先，GLES 兜底）。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum AndroidBackend {
    /// Vulkan（强烈偏好）。
    #[default]
    Vulkan,
    /// OpenGL ES（无 Vulkan 1.1 的设备回退）。
    Gles,
}

impl std::fmt::Display for AndroidBackend {
    /// 返回后端展示名。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Vulkan => write!(f, "Vulkan"),
            Self::Gles => write!(f, "OpenGL ES"),
        }
    }
}

// ── 输入事件类型 ─────────────────────────────────────────────────────────────

/// 精简键事件（NDK `KeyEvent` 中 GPUI 需要的子集）。
#[derive(Clone, Debug)]
pub struct AndroidKeyEvent {
    /// Android 键码（`KEYCODE_*`）。
    pub key_code: i32,
    /// `ACTION_DOWN = 0` / `ACTION_UP = 1`。
    pub action: i32,
    /// 修饰键位图（`META_SHIFT_ON` / `META_CTRL_ON` …）。
    pub meta_state: i32,
    /// 产生的 Unicode 字符（无则为 0）。
    pub unicode_char: u32,
}

/// NDK 输入队列送来的单个触摸点。
#[derive(Clone, Debug)]
pub struct TouchPoint {
    /// 触摸点标识。
    pub id: i32,
    /// 物理像素横坐标（相对 surface）。
    pub x: f32,
    /// 物理像素纵坐标（相对 surface）。
    pub y: f32,
    /// 单指掩码后的动作（`AMOTION_EVENT_ACTION_*`）。
    pub action: u32,
}

// ── 软键盘输入回调 ───────────────────────────────────────────────────────────

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

/// 软键盘文本回调类型。
pub type TextInputCallbackFn = Box<dyn FnMut(&str)>;

/// 收到键盘文本、待重渲染的全局标记。
///
/// 键盘文本绕过 GPUI 的 invalidator，平台帧回调见此标记即强制渲染一帧，
/// 由 `request_frame` 包装把文本刷进 UI。
pub static TEXT_INPUT_DIRTY: AtomicBool = AtomicBool::new(false);

/// 置脏并唤醒帧源（真机叫醒循环；主机无循环，只记标记供下帧消费）。
pub(crate) fn mark_text_input_dirty() {
    TEXT_INPUT_DIRTY.store(true, Ordering::Release);
    #[cfg(target_os = "android")]
    frame_source::wake();
}

thread_local! {
    /// 当前聚焦输入框注册的软键盘文本回调（同一时刻最多一个）。
    static TEXT_INPUT_CALLBACK: RefCell<Option<TextInputCallbackFn>> = RefCell::new(None);
}

/// 注册软键盘文本回调（`None` 为注销，通常由获焦的输入框调用）。
pub fn set_text_input_callback(callback: Option<TextInputCallbackFn>) {
    TEXT_INPUT_CALLBACK.with(|slot| {
        *slot.borrow_mut() = callback;
    });
}

/// 把键盘文本分发给已注册回调（平台层在收到键盘文本时调用）。
///
/// 有回调处理即返回 true，并置 [`TEXT_INPUT_DIRTY`] 强制下一帧渲染。
pub fn dispatch_text_input(text: &str) -> bool {
    TEXT_INPUT_CALLBACK.with(|slot| {
        if let Some(callback) = slot.borrow_mut().as_mut() {
            callback(text);
            mark_text_input_dirty();
            true
        } else {
            false
        }
    })
}

// ── 软键盘类型与高度 ─────────────────────────────────────────────────────────

/// 软键盘类型（映射 iOS `UIKeyboardType` / Android `InputType`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyboardType {
    /// 标准文本键盘。
    #[default]
    Default,
    /// 邮箱键盘（含 @ 与 .）。
    EmailAddress,
    /// 电话拨号盘。
    Phone,
    /// 纯数字键盘。
    NumberPad,
    /// URL 键盘。
    URL,
    /// 小数键盘（数字 + 小数点）。
    Decimal,
}

/// 弹出软键盘（默认类型；非移动端为空操作）。
pub fn show_keyboard() {
    show_keyboard_with_type(KeyboardType::Default);
}

/// 按指定类型弹出软键盘（非移动端为空操作）。
pub fn show_keyboard_with_type(keyboard_type: KeyboardType) {
    #[cfg(target_os = "android")]
    {
        // 类型存档供 `EditorInfo` 取；真机重弹键盘时 `onCreateInputConnection` 生效。
        ime::set_input_type(keyboard_type);
        bridge::show_keyboard_android(keyboard_type);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = keyboard_type;
    }
}

/// 收起软键盘（非移动端为空操作）。
pub fn hide_keyboard() {
    #[cfg(target_os = "android")]
    {
        bridge::hide_keyboard_android();
    }
    #[cfg(not(target_os = "android"))]
    {}
}

use std::sync::atomic::AtomicU32;

/// 当前软键盘高度（逻辑点，`f32` 位模式存原子量；0 表收起）。
pub static KEYBOARD_HEIGHT_BITS: AtomicU32 = AtomicU32::new(0);
/// 读取当前软键盘高度（逻辑点，收起时 0）。
pub fn keyboard_height() -> f32 {
    f32::from_bits(KEYBOARD_HEIGHT_BITS.load(Ordering::Relaxed))
}

/// 更新软键盘高度（平台层在键盘显隐时调用，变化超 0.5pt 才置脏）。
pub fn set_keyboard_height(height: f32) {
    let prev = f32::from_bits(KEYBOARD_HEIGHT_BITS.load(Ordering::Relaxed));
    if (prev - height).abs() > 0.5 {
        KEYBOARD_HEIGHT_BITS.store(height.to_bits(), Ordering::Release);
        mark_text_input_dirty();
    }
}

// ── 系统 chrome（状态栏/导航栏） ──────────────────────────────────────────────

/// 状态栏内容（文字/图标）深浅。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusBarContentStyle {
    /// 白字（深背景用）。
    Light,
    /// 黑字（浅背景用）。
    #[default]
    Dark,
}

/// 系统 chrome 样式（颜色为 `0xRRGGBB`，不带 alpha；`None` 表不动）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemChromeStyle {
    /// 状态栏底色。
    pub status_bar_color: Option<u32>,
    /// 状态栏内容深浅。
    pub status_bar_style: StatusBarContentStyle,
    /// 导航栏底色。
    pub navigation_bar_color: Option<u32>,
}

impl Default for SystemChromeStyle {
    /// 默认样式（只深色文字，其余不动）。
    fn default() -> Self {
        Self {
            status_bar_color: None,
            status_bar_style: StatusBarContentStyle::Dark,
            navigation_bar_color: None,
        }
    }
}

/// 应用系统 chrome 样式（状态栏/导航栏底色与文字深浅；主机空操作）。
pub fn set_system_chrome(style: &SystemChromeStyle) {
    #[cfg(target_os = "android")]
    {
        bridge::set_system_chrome(style);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = style;
    }
}
