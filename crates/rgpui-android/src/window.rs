//! Android 窗口：包一层系统给的 `ANativeWindow`，自带 wgpu 渲染器。
//!
//! 生命周期（系统随时可销毁重建 surface，M2 语义与参考实现一致）：
//! * `INIT_WINDOW` —— 建 wgpu surface 与渲染器（已有则只换 surface，保图集）；
//! * `TERM_WINDOW` —— `unconfigure_surface` 停 present，渲染器与图集留着；
//! * `WINDOW_RESIZED` —— `update_drawable_size`。
//!
//! 同一 `ANativeWindow` 重建回来时走 `destroy` + `recover` 双步
//! （Vulkan 一窗一 surface，直接 `replace_surface` 会撞 `IN_USE_KHR`）。
//!
//! 输入由 `bridge` 的 NDK 队列喂进来（`handle_touch` / `handle_key_event`），
//! 经 [`AndroidPlatformWindow`] 转成 [`rgpui::PlatformInput`] 交核心。

use parking_lot::Mutex;
use raw_window_handle::{HandleError, HasDisplayHandle, HasWindowHandle};
use rgpui::{
    AtlasKey, AtlasTextureId, AtlasTextureKind, AtlasTile, Bounds, Capslock, DevicePixels,
    DispatchEventResult, GpuSpecs, Modifiers, Pixels, PlatformAtlas, PlatformDisplay,
    PlatformInput, PlatformInputHandler, PlatformWindow, PromptButton, PromptLevel,
    RequestFrameOptions, Scene, Size, TileId, TouchEvent, TouchId, TouchPhase, WindowAppearance,
    WindowBackgroundAppearance, WindowBounds, WindowControlArea, point, px, size,
};
/// 真机渲染三件套（主机桩用不到，Android 才编译）。
#[cfg(target_os = "android")]
use rgpui_wgpu::{GpuContext, WgpuRenderer, WgpuSurfaceConfig};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use super::fling_guard::FlingGuard;
use super::keyboard::{AKEY_EVENT_ACTION_DOWN, AKEY_EVENT_ACTION_UP, android_key_to_keystroke};
use super::{AndroidKeyEvent, TouchPoint};

// ── 安全区 ───────────────────────────────────────────────────────────────────

/// 安全区内边距（物理像素；刘海/状态栏/导航栏占掉的区域）。
#[derive(Debug, Clone, Copy, Default)]
pub struct SafeAreaInsets {
    /// 顶部（状态栏/刘海）。
    pub top: f32,
    /// 底部（导航栏/手势区）。
    pub bottom: f32,
    /// 左侧（横屏刘海）。
    pub left: f32,
    /// 右侧（横屏刘海）。
    pub right: f32,
}

impl SafeAreaInsets {
    /// 物理像素转逻辑像素。
    pub fn to_logical(&self, scale_factor: f32) -> SafeAreaInsets {
        SafeAreaInsets {
            top: self.top / scale_factor,
            bottom: self.bottom / scale_factor,
            left: self.left / scale_factor,
            right: self.right / scale_factor,
        }
    }
}

// ── 回调类型 ─────────────────────────────────────────────────────────────────

/// 每 vsync 取帧回调。
pub type RequestFrameCallback = Box<dyn FnMut() + Send + 'static>;
/// 触摸点回调。
pub type TouchCallback = Box<dyn FnMut(TouchPoint) + Send + 'static>;
/// 按键事件回调；返回值表示事件是否被应用消费（用于放行系统默认处理）。
pub type KeyCallback = Box<dyn FnMut(AndroidKeyEvent) -> bool + Send + 'static>;
/// IME 组合回调（`InputConnection` 操作进队后，主循环消化经此交核心）。
pub type ImeCallback = Box<dyn FnMut(rgpui::ImeEvent) + Send + 'static>;
/// 尺寸变化回调（设备像素尺寸 + 缩放；仅真机存槽）。
pub type ResizeCallback = Box<dyn FnMut(Size<DevicePixels>, f32) + Send + 'static>;
/// 深浅色变化回调（参数为本地外观，`AndroidPlatformWindow` 再映射核心外观）。
pub type AppearanceCallback = Box<dyn FnMut(LocalAppearance) + Send + 'static>;
/// 前后台回调。
pub type ActiveStatusCallback = Box<dyn FnMut(bool) + Send + 'static>;

// ── 窗口外观（本地） ─────────────────────────────────────────────────────────

/// 窗口深浅色（含高对比，两档映射到核心外观）。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum LocalAppearance {
    /// 浅色。
    #[default]
    Light,
    /// 深色。
    Dark,
}

// ── 窗口状态 ─────────────────────────────────────────────────────────────────

/// 锁内可变状态（GPU 相关只在主线程碰，见 `Send` 实现上的 SAFETY）。
struct WindowState {
    /// 原生窗口（`TERM_WINDOW` 后为空，`INIT_WINDOW` 回填）。
    #[cfg(target_os = "android")]
    native_window: Option<ndk::native_window::NativeWindow>,
    /// 共享 GPU 上下文（实例 + 适配器 + 设备 + 队列，建一次多窗复用）。
    #[cfg(target_os = "android")]
    gpu_context: GpuContext,
    /// wgpu 渲染器（surface 不可用时为空）。
    #[cfg(target_os = "android")]
    renderer: Option<WgpuRenderer>,
    /// 缓存几何（物理像素）。
    width: i32,
    /// 缓存几何（物理像素）。
    height: i32,
    /// 缩放（设备像素 / 逻辑像素）。
    scale_factor: f32,
    /// 安全区（物理像素，`content_rect` 回填）。
    safe_area_insets: SafeAreaInsets,
    /// 当前外观。
    appearance: LocalAppearance,
    /// 是否前台。
    is_active: bool,
    /// 是否透明 surface（真机建 surface 用；主机桩无渲染，不存）。
    #[cfg(target_os = "android")]
    transparent: bool,
    /// 渲染器 surface 所绑的 `ANativeWindow` 地址（防同窗重建撞 `IN_USE_KHR`）。
    #[cfg(target_os = "android")]
    surface_window_addr: usize,
    /// 各回调（平台只存，`bridge` 循环与核心触发）。
    request_frame_callback: Option<RequestFrameCallback>,
    touch_callback: Option<TouchCallback>,
    key_callback: Option<KeyCallback>,
    /// IME 组合回调（`bridge` 主循环消化队列经此交核心）。
    ime_callback: Option<ImeCallback>,
    /// 尺寸回调（真机 `WINDOW_RESIZED`/重建时触发；主机单 surface 无来源）。
    #[cfg(target_os = "android")]
    resize_callback: Option<ResizeCallback>,
    /// 外观回调。
    appearance_callback: Option<AppearanceCallback>,
    /// 前后台回调。
    active_status_callback: Option<ActiveStatusCallback>,
}

// SAFETY：`WindowState` 只在持锁时访问；`GpuContext`（`Rc`）的 GPU 活
// 全在 Android 主线程，`Rc` 永不逃到他线程；`Send` 只为满足外层
// `Arc<AndroidWindow>` 的界。
#[cfg(target_os = "android")]
unsafe impl Send for WindowState {}

/// Android 窗口（通常一应用一窗，折叠屏可多窗）。
pub struct AndroidWindow {
    /// 锁内状态。
    state: Arc<Mutex<WindowState>>,
    /// 稳定数字标识（首个原生窗口指针派生）。
    id: u64,
    /// 前台标记（原子量，生命周期处理器不拿锁也能置）。
    active: Arc<std::sync::atomic::AtomicBool>,
    /// 新 surface 首帧强制全刷（换 swapchain 后 GPUI 脏区为空会黑屏一帧）。
    force_render_once: Arc<std::sync::atomic::AtomicBool>,
}

// SAFETY：状态有锁保护（`Send`/`Sync` 只为 `Arc` 共享）。
unsafe impl Send for AndroidWindow {}
unsafe impl Sync for AndroidWindow {}

impl AndroidWindow {
    /// 桩窗口（主机单测 + surface 未到时的占位）。
    pub fn headless(width: f32, height: f32, scale_factor: f32) -> Arc<Self> {
        let state = Arc::new(Mutex::new(WindowState {
            #[cfg(target_os = "android")]
            native_window: None,
            #[cfg(target_os = "android")]
            gpu_context: Rc::new(RefCell::new(None)),
            #[cfg(target_os = "android")]
            renderer: None,
            width: width as i32,
            height: height as i32,
            scale_factor,
            safe_area_insets: SafeAreaInsets::default(),
            appearance: LocalAppearance::Light,
            is_active: false,
            #[cfg(target_os = "android")]
            transparent: false,
            #[cfg(target_os = "android")]
            surface_window_addr: 0,
            request_frame_callback: None,
            touch_callback: None,
            key_callback: None,
            ime_callback: None,
            #[cfg(target_os = "android")]
            resize_callback: None,
            appearance_callback: None,
            active_status_callback: None,
        }));
        Arc::new(Self {
            state,
            id: ((width as u64) << 32) | (height as u64),
            active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            force_render_once: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        })
    }

    /// 稳定数字标识。
    pub fn id(&self) -> u64 {
        self.id
    }

    /// 缩放因子。
    pub fn scale_factor(&self) -> f32 {
        self.state.lock().scale_factor
    }

    /// 物理尺寸（设备像素）。
    pub fn physical_size(&self) -> Size<DevicePixels> {
        let state = self.state.lock();
        size(DevicePixels(state.width), DevicePixels(state.height))
    }

    /// 安全区（物理像素）。
    pub fn safe_area_insets(&self) -> SafeAreaInsets {
        self.state.lock().safe_area_insets
    }

    /// 安全区（逻辑像素）。
    pub fn safe_area_insets_logical(&self) -> SafeAreaInsets {
        let state = self.state.lock();
        state.safe_area_insets.to_logical(state.scale_factor)
    }

    /// 由系统 `content_rect`（物理像素）推安全区。
    pub fn update_safe_area_from_content_rect(
        &self,
        content_left: i32,
        content_top: i32,
        content_right: i32,
        content_bottom: i32,
    ) {
        let mut state = self.state.lock();
        state.safe_area_insets = SafeAreaInsets {
            top: content_top as f32,
            bottom: (state.height - content_bottom).max(0) as f32,
            left: content_left as f32,
            right: (state.width - content_right).max(0) as f32,
        };
    }

    /// 是否前台。
    pub fn is_active(&self) -> bool {
        self.active.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 置前后台（回调在锁外触发，防与 GPUI 回调死锁）。
    pub fn set_active(&self, active: bool) {
        use std::sync::atomic::Ordering;
        if self.active.swap(active, Ordering::Relaxed) == active {
            return;
        }
        let callback = {
            let mut state = self.state.lock();
            state.is_active = active;
            state.active_status_callback.take()
        };
        if let Some(mut callback) = callback {
            callback(active);
            let mut state = self.state.lock();
            if state.active_status_callback.is_none() {
                state.active_status_callback = Some(callback);
            }
        }
        if active {
            // 前台回来重投 vsync（后台前投的回调可能永不触发）。
            #[cfg(target_os = "android")]
            super::frame_source::resume();
        }
    }

    /// 置外观（变才调回调）。
    pub fn set_appearance(&self, appearance: LocalAppearance) {
        let callback = {
            let mut state = self.state.lock();
            if state.appearance == appearance {
                return;
            }
            state.appearance = appearance;
            state.appearance_callback.take()
        };
        if let Some(mut callback) = callback {
            callback(appearance);
            let mut state = self.state.lock();
            if state.appearance_callback.is_none() {
                state.appearance_callback = Some(callback);
            }
        }
    }

    /// 当前外观。
    pub fn appearance(&self) -> LocalAppearance {
        self.state.lock().appearance
    }

    /// 触发尺寸回调（存量尺寸重发一遍，供 surface 重建后对齐布局；仅真机）。
    #[cfg(target_os = "android")]
    fn notify_resize(&self) {
        let (width, height, scale) = {
            let state = self.state.lock();
            (state.width, state.height, state.scale_factor)
        };
        fire_resize(self, width, height, scale);
    }

    /// 注册每帧回调。
    pub fn on_request_frame(&self, callback: RequestFrameCallback) {
        self.state.lock().request_frame_callback = Some(callback);
    }

    /// 注册触摸回调。
    pub fn on_touch(&self, callback: TouchCallback) {
        self.state.lock().touch_callback = Some(callback);
    }

    /// 注册按键回调。
    pub fn on_key_event(&self, callback: KeyCallback) {
        self.state.lock().key_callback = Some(callback);
    }

    /// 注册 IME 组合回调。
    pub fn on_ime(&self, callback: ImeCallback) {
        self.state.lock().ime_callback = Some(callback);
    }

    /// 注册尺寸回调（真机存槽；主机单 surface 无尺寸来源，直接丢弃）。
    pub fn on_resize(&self, callback: ResizeCallback) {
        #[cfg(target_os = "android")]
        {
            self.state.lock().resize_callback = Some(callback);
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = callback;
        }
    }

    /// 注册外观回调。
    pub fn on_appearance_changed(&self, callback: AppearanceCallback) {
        self.state.lock().appearance_callback = Some(callback);
    }

    /// 注册前后台回调。
    pub fn on_active_status_change(&self, callback: ActiveStatusCallback) {
        self.state.lock().active_status_callback = Some(callback);
    }

    /// 跑一帧回调（回调取出锁外跑，防 `draw` 回锁死锁）。
    pub fn request_frame(&self) {
        let callback = { self.state.lock().request_frame_callback.take() };
        if let Some(mut callback) = callback {
            log::info!("request_frame：触发核心帧回调");
            callback();
            let mut state = self.state.lock();
            if state.request_frame_callback.is_none() {
                state.request_frame_callback = Some(callback);
            }
        }
    }

    /// 投递触摸点（无回调即丢并记日志，方便首帧前的问题定位）。
    pub fn handle_touch(&self, point: TouchPoint) {
        let callback = { self.state.lock().touch_callback.take() };
        if let Some(mut callback) = callback {
            callback(point);
            let mut state = self.state.lock();
            if state.touch_callback.is_none() {
                state.touch_callback = Some(callback);
            }
        } else {
            log::warn!("触摸被丢弃（帧回调未注册）：id={}", point.id);
        }
    }

    /// 投递按键事件；返回是否被应用消费。
    pub fn handle_key_event(&self, event: AndroidKeyEvent) -> bool {
        let callback = { self.state.lock().key_callback.take() };
        if let Some(mut callback) = callback {
            let consumed = callback(event);
            let mut state = self.state.lock();
            if state.key_callback.is_none() {
                state.key_callback = Some(callback);
            }
            consumed
        } else {
            false
        }
    }

    /// 投递 IME 组合事件（无回调即丢并记日志）。
    pub fn handle_ime(&self, event: rgpui::ImeEvent) {
        let callback = { self.state.lock().ime_callback.take() };
        if let Some(mut callback) = callback {
            callback(event);
            let mut state = self.state.lock();
            if state.ime_callback.is_none() {
                state.ime_callback = Some(callback);
            }
        } else {
            log::warn!("IME 事件被丢弃（输入回调未注册）");
        }
    }
}

/// 在锁外跑尺寸回调（回调内调 `bounds/scale_factor` 要拿锁；仅真机）。
#[cfg(target_os = "android")]
fn fire_resize(window: &AndroidWindow, width: i32, height: i32, scale: f32) {
    let callback = { window.state.lock().resize_callback.take() };
    if let Some(mut callback) = callback {
        callback(size(DevicePixels(width), DevicePixels(height)), scale);
        let mut state = window.state.lock();
        if state.resize_callback.is_none() {
            state.resize_callback = Some(callback);
        }
    }
}

// ── 真机 surface 生命周期（仅 Android） ───────────────────────────────────────

#[cfg(target_os = "android")]
mod native {
    use super::*;
    use ndk::native_window::NativeWindow;

    /// 轻量 owned 窗口句柄（`WgpuRenderer::new` 要 `Clone + Debug + Send + Sync`）。
    #[derive(Debug, Clone, Copy)]
    struct RawAndroidWindow {
        /// `ANativeWindow*` 裸指针。
        raw: *mut std::ffi::c_void,
    }

    unsafe impl Send for RawAndroidWindow {}
    unsafe impl Sync for RawAndroidWindow {}

    impl HasWindowHandle for RawAndroidWindow {
        fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> {
            use raw_window_handle::AndroidNdkWindowHandle;
            use std::ptr::NonNull;
            let ptr = NonNull::new(self.raw).ok_or(HandleError::Unavailable)?;
            let handle = AndroidNdkWindowHandle::new(ptr);
            Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
        }
    }

    impl HasDisplayHandle for RawAndroidWindow {
        fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, HandleError> {
            use raw_window_handle::{AndroidDisplayHandle, RawDisplayHandle};
            Ok(unsafe {
                raw_window_handle::DisplayHandle::borrow_raw(RawDisplayHandle::Android(
                    AndroidDisplayHandle::new(),
                ))
            })
        }
    }

    /// 由 `NativeWindow` 包 raw 句柄。
    fn raw_window(native_window: &NativeWindow) -> RawAndroidWindow {
        let ptr = native_window
            .window_handle()
            .expect("NativeWindow 无句柄")
            .as_raw();
        let raw = match ptr {
            raw_window_handle::RawWindowHandle::AndroidNdk(handle) => {
                handle.a_native_window.as_ptr()
            }
            _ => panic!("只要 AndroidNdk 窗口句柄"),
        };
        RawAndroidWindow { raw }
    }

    /// 设备 API 级别（`dlsym` 查 `android_get_device_api_level`；
    /// 该符号 API 29 才有，低版本系统直接回 `minSdk` 基线 26，
    /// 绝不能静态链——否则 API 28 及以下设备 `dlopen` 即炸）。
    fn device_api_level() -> i32 {
        let name = c"android_get_device_api_level";
        let symbol = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
        if symbol.is_null() {
            return 26;
        }
        type GetLevel = unsafe extern "C" fn() -> std::os::raw::c_int;
        let get_level: GetLevel = unsafe { std::mem::transmute(symbol) };
        unsafe { get_level() }
    }

    /// 按需请 120Hz（全 `dlsym` 动态查符号，低版本系统不 `dlopen` 炸）。
    fn request_high_frame_rate(window: &NativeWindow) {
        if device_api_level() < 30 {
            return;
        }
        let name = c"ANativeWindow_setFrameRateWithChangeStrategy";
        let symbol = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
        if symbol.is_null() {
            return;
        }
        type SetRate = unsafe extern "C" fn(*mut ndk_sys::ANativeWindow, f32, i8, i8) -> i32;
        let set_rate: SetRate = unsafe { std::mem::transmute(symbol) };
        let status = unsafe { set_rate(window.ptr().as_ptr(), 120.0, 0, 0) };
        if status == 0 {
            log::info!("已请 120Hz 刷新率");
        }
    }

    impl AndroidWindow {
        /// 由原生窗口建窗并初始化渲染器。
        pub fn new(
            native_window: NativeWindow,
            gpu_context: GpuContext,
            scale_factor: f32,
            transparent: bool,
        ) -> anyhow::Result<Arc<Self>> {
            request_high_frame_rate(&native_window);
            let width = native_window.width();
            let height = native_window.height();
            log::info!("AndroidWindow 建窗：{width}×{height} scale={scale_factor:.1}");
            let renderer = Self::create_renderer(
                &native_window,
                Rc::clone(&gpu_context),
                width,
                height,
                transparent,
            )?;
            let id = native_window.ptr().as_ptr() as u64;
            let addr = native_window.ptr().as_ptr() as usize;
            let state = Arc::new(Mutex::new(WindowState {
                native_window: Some(native_window),
                gpu_context,
                renderer: Some(renderer),
                width,
                height,
                scale_factor,
                safe_area_insets: SafeAreaInsets::default(),
                appearance: LocalAppearance::Light,
                is_active: true,
                transparent,
                surface_window_addr: addr,
                request_frame_callback: None,
                touch_callback: None,
                key_callback: None,
                ime_callback: None,
                resize_callback: None,
                appearance_callback: None,
                active_status_callback: None,
            }));
            Ok(Arc::new(Self {
                state,
                id,
                active: Arc::new(std::sync::atomic::AtomicBool::new(true)),
                force_render_once: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            }))
        }

        /// 建渲染器。
        fn create_renderer(
            native_window: &NativeWindow,
            gpu_context: GpuContext,
            width: i32,
            height: i32,
            transparent: bool,
        ) -> anyhow::Result<WgpuRenderer> {
            let raw = raw_window(native_window);
            let config = WgpuSurfaceConfig {
                size: rgpui::size(DevicePixels(width), DevicePixels(height)),
                transparent,
                preferred_present_mode: Some(rgpui_wgpu::wgpu::PresentMode::Mailbox),
            };
            WgpuRenderer::new(gpu_context, &raw, config, None)
        }

        /// `INIT_WINDOW`：新 surface 到（后台回来/重建），复用渲染器只换面。
        pub fn init_window(
            &self,
            native_window: NativeWindow,
            gpu_context: GpuContext,
        ) -> anyhow::Result<()> {
            request_high_frame_rate(&native_window);
            let width = native_window.width();
            let height = native_window.height();
            let mut state = self.state.lock();
            let transparent = state.transparent;
            let incoming = native_window.ptr().as_ptr() as usize;
            if state.renderer.is_some() && state.surface_window_addr == incoming {
                // 同一活 `ANativeWindow`：Vulkan 不许一窗两面，先 destroy 再 recover，
                // 图集 `Arc` 留着（tile 清掉下帧重光栅，首帧强制全刷兜底）。
                log::info!("同窗重建 surface：{width}×{height}");
                let raw = raw_window(&native_window);
                let renderer = state
                    .renderer
                    .as_mut()
                    .unwrap_or_else(|| panic!("渲染器应在锁内"));
                renderer.destroy();
                renderer.recover(&raw)?;
                renderer
                    .update_drawable_size(rgpui::size(DevicePixels(width), DevicePixels(height)));
            } else if state.renderer.is_some() {
                let raw = raw_window(&native_window);
                let config = WgpuSurfaceConfig {
                    size: rgpui::size(DevicePixels(width), DevicePixels(height)),
                    transparent,
                    preferred_present_mode: Some(rgpui_wgpu::wgpu::PresentMode::Mailbox),
                };
                let instance = state
                    .gpu_context
                    .borrow()
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("换面时 GPU 上下文丢了"))?
                    .instance
                    .clone();
                state
                    .renderer
                    .as_mut()
                    .expect("渲染器应在锁内")
                    .replace_surface(&raw, config, &instance)?;
                state.surface_window_addr = incoming;
                log::info!("surface 已换：{width}×{height}");
            } else {
                let context = if state.gpu_context.borrow().is_some() {
                    Rc::clone(&state.gpu_context)
                } else {
                    state.gpu_context = Rc::clone(&gpu_context);
                    gpu_context
                };
                let renderer =
                    Self::create_renderer(&native_window, context, width, height, transparent)?;
                state.renderer = Some(renderer);
                state.surface_window_addr = incoming;
                log::info!("渲染器新建：{width}×{height}");
            }
            let size_changed = state.width != width || state.height != height;
            state.native_window = Some(native_window);
            state.width = width;
            state.height = height;
            state.is_active = true;
            self.active
                .store(true, std::sync::atomic::Ordering::Relaxed);
            // 空 swapchain 首帧强制全刷，否则 GPUI 脏区为空黑一帧。
            self.force_render_once
                .store(true, std::sync::atomic::Ordering::Relaxed);
            drop(state);
            super::super::frame_source::resume();
            // Activity 重建可能连尺寸一起换（旋转），直接重发尺寸对齐布局。
            if size_changed {
                self.notify_resize();
            }
            Ok(())
        }

        /// `TERM_WINDOW`：surface 销毁，只 `unconfigure` 不丢渲染器（保图集）。
        pub fn term_window(&self) {
            let mut state = self.state.lock();
            if let Some(renderer) = state.renderer.as_mut() {
                renderer.unconfigure_surface();
                log::info!("surface 已卸（渲染器与图集保留）");
            }
            state.native_window = None;
            state.is_active = false;
            self.active
                .store(false, std::sync::atomic::Ordering::Relaxed);
            // 不触发 close 回调：逻辑窗与 GPUI 回调都留着，回来 `init_window` 无缝续上。
        }

        /// `WINDOW_RESIZED`：尺寸变才调渲染器与回调。
        pub fn handle_resize(&self) {
            let (width, height, scale) = {
                let mut state = self.state.lock();
                let window = match state.native_window.as_ref() {
                    Some(window) => window,
                    None => return,
                };
                let (width, height) = (window.width(), window.height());
                if width == state.width && height == state.height {
                    return;
                }
                log::info!(
                    "窗口尺寸：{}×{} → {width}×{height}",
                    state.width,
                    state.height
                );
                state.width = width;
                state.height = height;
                // `update_drawable_size` 内部 poll 设备，渲染器取出锁外跑。
                if let Some(mut renderer) = state.renderer.take() {
                    renderer.update_drawable_size(rgpui::size(
                        DevicePixels(width),
                        DevicePixels(height),
                    ));
                    state.renderer = Some(renderer);
                }
                // 尺寸真变了必须补一帧：否则合成器继续拉伸旧缓冲（旋转实证）。
                #[cfg(target_os = "android")]
                {
                    super::super::frame_source::schedule_frame();
                }
                (width, height, state.scale_factor)
            };
            fire_resize(self, width, height, scale);
        }

        /// 有 surface 才真。
        pub fn has_surface(&self) -> bool {
            self.state.lock().renderer.is_some()
        }

        /// 当前记录的 surface 尺寸（设备像素，看门狗对账用）。
        pub fn surface_size(&self) -> (i32, i32) {
            let state = self.state.lock();
            (state.width, state.height)
        }

        /// 当前记录的 surface 对象地址（看门狗判断换面用）。
        pub fn surface_addr(&self) -> usize {
            self.state.lock().surface_window_addr
        }

        /// 渲染器精灵图集（surface 卸掉时为空）。
        pub fn sprite_atlas(&self) -> Option<Arc<dyn PlatformAtlas>> {
            let state = self.state.lock();
            state
                .renderer
                .as_ref()
                .map(|renderer| renderer.sprite_atlas().clone() as Arc<dyn PlatformAtlas>)
        }

        /// GPU 信息（surface 卸掉时为空）。
        pub fn gpu_specs(&self) -> Option<GpuSpecs> {
            let state = self.state.lock();
            state.renderer.as_ref().map(|renderer| renderer.gpu_specs())
        }

        /// 双源混合（亚像素抗锯齿）支持。
        pub fn supports_subpixel_aa(&self) -> bool {
            self.state
                .lock()
                .renderer
                .as_ref()
                .map(|renderer| renderer.supports_dual_source_blending())
                .unwrap_or(false)
        }

        /// 画场景（空场景跳过：上游渲染器清屏后再 present 会闪黑一帧）。
        pub fn draw(&self, scene: &Scene) {
            if scene_is_empty(scene) {
                log::info!("draw：空场景跳过");
                return;
            }
            log::info!(
                "draw：quads={} shadows={} sprites={}+{}+{} surfaces={}",
                scene.quads.len(),
                scene.shadows.len(),
                scene.monochrome_sprites.len(),
                scene.subpixel_sprites.len(),
                scene.polychrome_sprites.len(),
                scene.surfaces.len(),
            );
            // 渲染器取出锁外画：`get_current_texture` 可能阻塞 GPU，
            // 抱锁画会卡死布局/事件循环对状态的访问。
            let mut renderer = {
                let mut state = self.state.lock();
                match state.renderer.take() {
                    Some(renderer) => renderer,
                    None => return,
                }
            };
            let presented = renderer.draw(scene);
            log::info!("draw：present 结果={presented}");
            self.state.lock().renderer = Some(renderer);
        }
    }

    impl Drop for AndroidWindow {
        /// 先毁渲染器再放原生窗口。
        fn drop(&mut self) {
            let mut state = self.state.lock();
            if let Some(mut renderer) = state.renderer.take() {
                renderer.destroy();
            }
            state.native_window = None;
        }
    }

    /// 场景是否无可画图元（文本/图片走 sprite 桶，不能只看 quads）。
    fn scene_is_empty(scene: &Scene) -> bool {
        scene.shadows.is_empty()
            && scene.quads.is_empty()
            && scene.paths.is_empty()
            && scene.underlines.is_empty()
            && scene.monochrome_sprites.is_empty()
            && scene.subpixel_sprites.is_empty()
            && scene.polychrome_sprites.is_empty()
            && scene.surfaces.is_empty()
    }
}

// ── 主机桩绘制 ────────────────────────────────────────────────────────────────

/// 主机 `draw` 空实现（无 GPU）。
#[cfg(not(target_os = "android"))]
impl AndroidWindow {
    /// 是否有 surface（主机恒假）。
    pub fn has_surface(&self) -> bool {
        false
    }

    /// 精灵图集（主机恒空，走兜底图集）。
    pub fn sprite_atlas(&self) -> Option<Arc<dyn PlatformAtlas>> {
        None
    }

    /// GPU 信息（主机恒空）。
    pub fn gpu_specs(&self) -> Option<GpuSpecs> {
        None
    }

    /// 亚像素支持（主机恒假）。
    pub fn supports_subpixel_aa(&self) -> bool {
        false
    }

    /// 画场景（主机空实现）。
    pub fn draw(&self, _scene: &Scene) {}
}

// ── 兜底图集 ──────────────────────────────────────────────────────────────────

/// 渲染器不可用时的空图集（只分配 tile 不上屏，首帧前布局不断）。
struct FallbackAtlas {
    /// 状态锁。
    state: Mutex<FallbackAtlasState>,
}

/// 兜底图集状态。
struct FallbackAtlasState {
    /// 下一序号。
    next_id: u32,
    /// 已分配 tile。
    tiles: HashMap<AtlasKey, AtlasTile>,
}

impl FallbackAtlas {
    /// 新兜底图集。
    fn new() -> Self {
        Self {
            state: Mutex::new(FallbackAtlasState {
                next_id: 1,
                tiles: HashMap::new(),
            }),
        }
    }
}

impl PlatformAtlas for FallbackAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<Option<AtlasTile>> {
        let mut state = self.state.lock();
        if let Some(tile) = state.tiles.get(key) {
            return Ok(Some(*tile));
        }
        let Some((size, _)) = build()? else {
            return Ok(None);
        };
        let id = state.next_id;
        state.next_id += 1;
        let tile = AtlasTile {
            texture_id: AtlasTextureId {
                index: 0,
                kind: AtlasTextureKind::Monochrome,
            },
            tile_id: TileId(id),
            padding: 0,
            bounds: Bounds {
                origin: point(DevicePixels(0), DevicePixels(0)),
                size,
            },
        };
        state.tiles.insert(key.clone(), tile);
        Ok(Some(tile))
    }

    fn remove(&self, key: &AtlasKey) {
        self.state.lock().tiles.remove(key);
    }
}

// ── 窗口列表 ──────────────────────────────────────────────────────────────────

/// 存活窗口表（直板机一窗，折叠屏可多窗）。
#[derive(Default)]
pub struct WindowList {
    /// 窗口。
    windows: Vec<Arc<AndroidWindow>>,
}

impl WindowList {
    /// 入表。
    pub fn push(&mut self, window: Arc<AndroidWindow>) {
        self.windows.push(window);
    }

    /// 按 id 移除并返回。
    pub fn remove(&mut self, id: u64) -> Option<Arc<AndroidWindow>> {
        self.windows
            .iter()
            .position(|window| window.id() == id)
            .map(|index| self.windows.remove(index))
    }

    /// 取主窗（首个）。
    pub fn primary(&self) -> Option<&Arc<AndroidWindow>> {
        self.windows.first()
    }

    /// 窗口数。
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    /// 是否空。
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }
}

// ── PlatformWindow 实现 ───────────────────────────────────────────────────────

/// `Platform::open_window` 交核心的包装（把 GPUI 回调桥到 `AndroidWindow`）。
pub struct AndroidPlatformWindow {
    /// 底层窗口。
    window: Arc<AndroidWindow>,
    /// 所属显示器。
    display: Option<Rc<dyn PlatformDisplay>>,
    /// 输入处理器（IME 组合用，M2 只存，M3 接组合串）。
    input_handler: Rc<RefCell<Option<PlatformInputHandler>>>,
    /// 转发给核心的输入回调（`Send` 化后存 `Arc` 供触摸/按键闭包共享）。
    input_callback: Arc<Mutex<Box<dyn FnMut(PlatformInput) -> DispatchEventResult + Send>>>,
    /// 标题。
    title: String,
}

impl AndroidPlatformWindow {
    /// 包已有窗口与显示器。
    pub fn new(window: Arc<AndroidWindow>, display: Option<Rc<dyn PlatformDisplay>>) -> Self {
        Self {
            window,
            display,
            input_handler: Rc::new(RefCell::new(None)),
            input_callback: Arc::new(Mutex::new(Box::new(|_| DispatchEventResult::default()))),
            title: String::new(),
        }
    }

    /// 取底层窗口。
    pub fn inner(&self) -> &Arc<AndroidWindow> {
        &self.window
    }
}

impl HasWindowHandle for AndroidPlatformWindow {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

impl HasDisplayHandle for AndroidPlatformWindow {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, HandleError> {
        Ok(unsafe {
            raw_window_handle::DisplayHandle::borrow_raw(
                raw_window_handle::RawDisplayHandle::Android(
                    raw_window_handle::AndroidDisplayHandle::new(),
                ),
            )
        })
    }
}

impl PlatformWindow for AndroidPlatformWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        let physical = self.window.physical_size();
        let scale = self.window.scale_factor();
        Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(
                px(physical.width.0 as f32 / scale),
                px(physical.height.0 as f32 / scale),
            ),
        }
    }

    fn is_maximized(&self) -> bool {
        true
    }

    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Fullscreen(self.bounds())
    }

    fn content_size(&self) -> Size<Pixels> {
        self.bounds().size
    }

    fn resize(&mut self, _size: Size<Pixels>) {
        // 尺寸系统说了算，应用无权调。
    }

    fn scale_factor(&self) -> f32 {
        self.window.scale_factor()
    }

    fn appearance(&self) -> WindowAppearance {
        match self.window.appearance() {
            LocalAppearance::Dark => WindowAppearance::Dark,
            LocalAppearance::Light => WindowAppearance::Light,
        }
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.display.clone()
    }

    fn mouse_position(&self) -> rgpui::Point<Pixels> {
        // 纯触摸设备，无鼠标位置。
        point(px(0.0), px(0.0))
    }

    fn modifiers(&self) -> Modifiers {
        Modifiers::default()
    }

    fn capslock(&self) -> Capslock {
        Capslock::default()
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        *self.input_handler.borrow_mut() = Some(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.input_handler.borrow_mut().take()
    }

    fn prompt(
        &self,
        _level: PromptLevel,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[PromptButton],
    ) -> Option<futures::channel::oneshot::Receiver<usize>> {
        // 原生弹窗要 JNI 起 AlertDialog，M3 再接；此处返回 None 表不支持。
        None
    }

    fn activate(&self) {
        self.window.set_active(true);
    }

    fn is_active(&self) -> bool {
        self.window.is_active()
    }

    fn is_hovered(&self) -> bool {
        // 无悬停概念，前台即视为 hover。
        self.window.is_active()
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }

    fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    fn set_background_appearance(&self, _background_appearance: WindowBackgroundAppearance) {}

    fn minimize(&self) {}

    fn zoom(&self) {}

    fn toggle_fullscreen(&self) {}

    fn is_fullscreen(&self) -> bool {
        true
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        // 核心回调包一层：键盘文本脏 / 新 surface 时强制全刷，
        // 整包转 Send 存槽（回调恒在主线程触发）。
        let force_render_once = Arc::clone(&self.window.force_render_once);
        let mut callback = callback;
        let wrapped: Box<dyn FnMut()> = Box::new(move || {
            let text_dirty =
                super::TEXT_INPUT_DIRTY.swap(false, std::sync::atomic::Ordering::AcqRel);
            let surface_new = force_render_once.swap(false, std::sync::atomic::Ordering::AcqRel);
            callback(RequestFrameOptions {
                require_presentation: false,
                force_render: text_dirty || surface_new,
            });
        });
        // SAFETY：恒主线程触发，转 `Send` 只为存进窗口回调槽。
        let send_callback: Box<dyn FnMut() + Send> = unsafe { std::mem::transmute(wrapped) };
        let send_callback = Mutex::new(send_callback);
        self.window.on_request_frame(Box::new(move || {
            send_callback.lock()();
        }));
    }

    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        // SAFETY：同上，输入回调恒在主线程触发。
        let send_callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult + Send> =
            unsafe { std::mem::transmute(callback) };
        *self.input_callback.lock() = send_callback;
        let shared = Arc::clone(&self.input_callback);

        // 触摸扇出：Android 指针 id 会复用，转单调 `TouchId`；
        // 多指 DOWN/UP 只派受影响的那根；过守卫防惯性轴锁定。
        {
            let shared = Arc::clone(&shared);
            let window = Arc::downgrade(&self.window);
            // 单调 id 发号器（Android 指针 id 会复用；`Cell` 避 move 语义坑）。
            let next_touch_id = std::cell::Cell::new(0u64);
            let mut live_touches = HashMap::new();
            let mut fling_guard = FlingGuard::new();
            self.window.on_touch(Box::new(move |touch: TouchPoint| {
                let alloc_id = || {
                    let id = TouchId(next_touch_id.get());
                    next_touch_id.set(next_touch_id.get().checked_add(1).expect("触摸 id 耗尽"));
                    id
                };
                let phase = match touch.action {
                    0 => TouchPhase::Started,
                    1 => TouchPhase::Ended,
                    2 => TouchPhase::Moved,
                    3 => TouchPhase::Cancelled,
                    _ => return,
                };
                let id = if phase == TouchPhase::Started {
                    let id = alloc_id();
                    live_touches.insert(touch.id, id);
                    id
                } else {
                    match live_touches.get(&touch.id).copied() {
                        Some(id) => id,
                        None => return,
                    }
                };
                let Some(window) = window.upgrade() else {
                    return;
                };
                let scale = window.scale_factor();
                let event = TouchEvent {
                    id,
                    phase,
                    position: point(px(touch.x / scale), px(touch.y / scale)),
                    force: None,
                };
                let mut guard = shared.lock();
                fling_guard.relay(event, alloc_id, |event| {
                    guard(PlatformInput::Touch(event));
                });
                if matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                    live_touches.remove(&touch.id);
                }
            }));
        }

        // 按键：DEL/方向/Home/End 走文本哨兵，其余 unicode 明文先过全局回调，
        // 再按 keystroke 发 KeyDown/KeyUp。
        {
            let shared = Arc::clone(&shared);
            // 返回键 DOWN 的消费结果，配对进 UP，避免 DOWN 已被消费而 UP 触发系统退出。
            let mut back_down_consumed = false;
            self.window
                .on_key_event(Box::new(move |key_event: AndroidKeyEvent| {
                    if key_event.action == AKEY_EVENT_ACTION_DOWN {
                        match key_event.key_code {
                            67 => {
                                super::dispatch_text_input("\x08");
                            }
                            21 => {
                                super::dispatch_text_input("\x1b[D");
                            }
                            22 => {
                                super::dispatch_text_input("\x1b[C");
                            }
                            122 => {
                                super::dispatch_text_input("\x1b[H");
                            }
                            123 => {
                                super::dispatch_text_input("\x1b[F");
                            }
                            _ => {
                                if key_event.unicode_char != 0 {
                                    if let Some(symbol) = char::from_u32(key_event.unicode_char) {
                                        super::dispatch_text_input(&symbol.to_string());
                                    }
                                }
                            }
                        }
                    }
                    let keystroke = match android_key_to_keystroke(
                        key_event.key_code,
                        key_event.meta_state,
                        key_event.unicode_char,
                    ) {
                        Some(keystroke) => keystroke,
                        None => return false,
                    };
                    let event = if key_event.action == AKEY_EVENT_ACTION_DOWN {
                        PlatformInput::KeyDown(rgpui::KeyDownEvent {
                            keystroke,
                            is_held: false,
                            prefer_character_input: key_event.unicode_char != 0,
                        })
                    } else if key_event.action == AKEY_EVENT_ACTION_UP {
                        PlatformInput::KeyUp(rgpui::KeyUpEvent { keystroke })
                    } else {
                        return false;
                    };
                    let result = shared.lock()(event);
                    // 应用是否消费了该按键（阻止系统默认处理，如 Android 返回键退出）。
                    let consumed = result.default_prevented || !result.propagate;
                    if key_event.key_code == super::keyboard::AKEYCODE_BACK {
                        if key_event.action == AKEY_EVENT_ACTION_DOWN {
                            back_down_consumed = consumed;
                            consumed
                        } else {
                            let paired = back_down_consumed || consumed;
                            back_down_consumed = false;
                            paired
                        }
                    } else {
                        consumed
                    }
                }));
        }

        // IME 组合：队列消化经此进核心分发（`Window::dispatch_ime_event`）。
        {
            let shared = Arc::clone(&shared);
            self.window.on_ime(Box::new(move |event: rgpui::ImeEvent| {
                let _ = shared.lock()(PlatformInput::Ime(event));
            }));
        }
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        // SAFETY：同上，恒主线程触发。
        let send_callback: Box<dyn FnMut(bool) + Send> = unsafe { std::mem::transmute(callback) };
        let send_callback = Mutex::new(send_callback);
        self.window.on_active_status_change(Box::new(move |active| {
            send_callback.lock()(active);
        }));
    }

    fn on_hover_status_change(&self, _callback: Box<dyn FnMut(bool)>) {
        // 触摸设备无悬停。
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        #[cfg(target_os = "android")]
        {
            // SAFETY：恒主线程触发，转 Send 存槽。
            let send_callback: Box<dyn FnMut(Size<Pixels>, f32) + Send> =
                unsafe { std::mem::transmute(callback) };
            let send_callback = Arc::new(Mutex::new(send_callback));
            self.window.on_resize(Box::new(move |device_size, scale| {
                log::info!(
                    "resize 回调：设备 {}×{} scale={scale} → 逻辑 {}×{}",
                    device_size.width.0,
                    device_size.height.0,
                    device_size.width.0 as f32 / scale,
                    device_size.height.0 as f32 / scale,
                );
                send_callback.lock()(
                    size(
                        px(device_size.width.0 as f32 / scale),
                        px(device_size.height.0 as f32 / scale),
                    ),
                    scale,
                );
            }));
        }
        #[cfg(not(target_os = "android"))]
        {
            // 主机单 surface 无尺寸来源。
            let _ = callback;
        }
    }

    fn on_moved(&self, _callback: Box<dyn FnMut()>) {}

    fn on_should_close(&self, _callback: Box<dyn FnMut() -> bool>) {
        // 生命周期系统说了算。
    }

    fn on_hit_test_window_control(&self, _callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
    }

    fn on_close(&self, _callback: Box<dyn FnOnce()>) {
        // Android 无窗口关闭事件（surface 丢失 ≠ 关闭，逻辑窗保留续跑）。
    }

    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        // SAFETY：同上，恒主线程触发。
        let send_callback: Box<dyn FnMut() + Send> = unsafe { std::mem::transmute(callback) };
        let send_callback = Mutex::new(send_callback);
        self.window
            .on_appearance_changed(Box::new(move |_appearance| {
                send_callback.lock()();
            }));
    }

    fn draw(&self, scene: &Scene) {
        self.window.draw(scene);
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.window
            .sprite_atlas()
            .unwrap_or_else(|| Arc::new(FallbackAtlas::new()))
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        self.window.supports_subpixel_aa()
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.window.gpu_specs()
    }

    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        // 经 JNI 告诉输入法候选框位置（`CursorAnchorInfo`，API 21+）。
        // NativeActivity 下无自定义 Activity 时调用失败即记日志，不管。
        #[cfg(target_os = "android")]
        {
            use super::bridge::{activity, with_env};
            use jni::objects::JValue;
            let x = f32::from(bounds.origin.x);
            let y = f32::from(bounds.origin.y);
            let height = f32::from(bounds.size.height);
            let _ = with_env(|env| {
                let activity = activity(env)?;
                let service = env
                    .new_string("input_method")
                    .map_err(|error| error.to_string())?;
                let manager = env
                    .call_method(
                        &activity,
                        jni::jni_str!("getSystemService"),
                        jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/Object;"),
                        &[JValue::Object(&service)],
                    )
                    .and_then(|value| value.l())
                    .map_err(|error| {
                        env.exception_clear();
                        error.to_string()
                    })?;
                if manager.is_null() {
                    return Err("输入法服务为空".to_string());
                }
                let builder = env
                    .new_object(
                        jni::jni_str!("android/view/inputmethod/CursorAnchorInfo$Builder"),
                        jni::jni_sig!("()V"),
                        &[],
                    )
                    .map_err(|error| {
                        env.exception_clear();
                        error.to_string()
                    })?;
                let _ = env.call_method(
                    &builder,
                    jni::jni_str!("setInsertionMarkerLocation"),
                    jni::jni_sig!("(FFFFI)Landroid/view/inputmethod/CursorAnchorInfo$Builder;"),
                    &[
                        JValue::Float(x),
                        JValue::Float(y),
                        JValue::Float(y + height * 0.8),
                        JValue::Float(y + height),
                        JValue::Int(0),
                    ],
                );
                env.exception_clear();
                let anchor = env
                    .call_method(
                        &builder,
                        jni::jni_str!("build"),
                        jni::jni_sig!("()Landroid/view/inputmethod/CursorAnchorInfo;"),
                        &[],
                    )
                    .and_then(|value| value.l())
                    .map_err(|error| {
                        env.exception_clear();
                        error.to_string()
                    })?;
                if anchor.is_null() {
                    return Err("CursorAnchorInfo 构建失败".to_string());
                }
                let window = env
                    .call_method(
                        &activity,
                        jni::jni_str!("getWindow"),
                        jni::jni_sig!("()Landroid/view/Window;"),
                        &[],
                    )
                    .and_then(|value| value.l())
                    .map_err(|error| {
                        env.exception_clear();
                        error.to_string()
                    })?;
                if window.is_null() {
                    return Err("窗口为空".to_string());
                }
                let decor = env
                    .call_method(
                        &window,
                        jni::jni_str!("getDecorView"),
                        jni::jni_sig!("()Landroid/view/View;"),
                        &[],
                    )
                    .and_then(|value| value.l())
                    .map_err(|error| {
                        env.exception_clear();
                        error.to_string()
                    })?;
                if decor.is_null() {
                    return Err("decor view 为空".to_string());
                }
                let _ = env.call_method(
                    &manager,
                    jni::jni_str!("updateCursorAnchorInfo"),
                    jni::jni_sig!(
                        "(Landroid/view/View;Landroid/view/inputmethod/CursorAnchorInfo;)V"
                    ),
                    &[JValue::Object(&decor), JValue::Object(&anchor)],
                );
                env.exception_clear();
                Ok(())
            });
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = bounds;
        }
    }

    fn get_title(&self) -> String {
        self.title.clone()
    }

    fn inner_window_bounds(&self) -> WindowBounds {
        self.window_bounds()
    }

    /// 空 HWND（仅 Windows 主机编译桩时满足 trait）。
    #[cfg(target_os = "windows")]
    fn get_raw_handle(&self) -> windows::Win32::Foundation::HWND {
        windows::Win32::Foundation::HWND(std::ptr::null_mut())
    }
}
