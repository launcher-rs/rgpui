//! Android 平台顶层：执行器、显示器、窗口表、剪贴板最小集。
//!
//! 真机 `Platform::run` 阻塞跑 `bridge` 事件循环，`finish_launching` 延后到
//! 首个 `INIT_WINDOW` 之后（`Application` 全程活在调用栈，GPUI 弱引用不断）。
//! M2 单 `android-activity` 路径；字体走系统目录 + CBDT Emoji 兜底。

use rgpui::{
    Action, AnyWindowHandle, BackgroundExecutor, ClipboardItem, CursorStyle, ForegroundExecutor,
    Keymap, Menu, MenuItem, PathPromptOptions, Platform, PlatformDisplay, PlatformKeyboardLayout,
    PlatformKeyboardMapper, PlatformTextSystem, PlatformWindow, Task, ThermalState,
    WindowAppearance, WindowParams,
};
use rgpui_wgpu::CosmicTextSystem;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use super::AndroidBackend;
use super::dispatcher::AndroidDispatcher;
use super::display::{AndroidDisplay, DisplayList};
use super::keyboard::{AndroidKeyboardLayout, AndroidKeyboardMapper};
use super::window::{AndroidPlatformWindow, AndroidWindow, WindowList};

/// Android 平台实现。
pub struct AndroidPlatform {
    /// 前后台分发器。
    dispatcher: Arc<AndroidDispatcher>,
    /// 文本系统（真机 CosmicText + 系统字体，主机空实现）。
    text_system: Arc<CosmicTextSystem>,
    /// GPU 上下文（多窗共享，首窗初始化）。
    gpu_context: rgpui_wgpu::GpuContext,
    /// 存活窗口。
    windows: parking_lot::Mutex<WindowList>,
    /// 显示器。
    displays: parking_lot::Mutex<DisplayList>,
    /// 进程内剪贴板（M3 经 JNI 接 `ClipboardManager`）。
    clipboard: parking_lot::Mutex<Option<String>>,
    /// `run` 的启动回调（首窗就绪后调一次）。
    finish_launching: parking_lot::Mutex<Option<Box<dyn FnOnce() + Send>>>,
    /// 首窗就绪回调（应用挂视图用，调一次）。
    on_init_window: parking_lot::Mutex<Option<Box<dyn FnOnce(Arc<AndroidWindow>) + Send>>>,
    /// 退出回调。
    quit_callback: parking_lot::Mutex<Option<Box<dyn FnMut() -> bool + Send>>>,
    /// 重开回调。
    reopen_callback: parking_lot::Mutex<Option<Box<dyn FnMut() + Send>>>,
    /// 链接打开回调。
    open_urls_callback: parking_lot::Mutex<Option<Box<dyn FnMut(Vec<String>) + Send>>>,
    /// 键盘布局变化回调。
    keyboard_layout_callback: parking_lot::Mutex<Option<Box<dyn FnMut() + Send>>>,
    /// 前台标记。
    is_active: std::sync::atomic::AtomicBool,
    /// 是否头模式。
    headless: bool,
    /// 退出标记（主循环轮询）。
    should_quit: std::sync::atomic::AtomicBool,
    /// GPU 后端偏好。
    preferred_backend: parking_lot::Mutex<AndroidBackend>,
}

// SAFETY：`gpu_context`（`Rc`）的 GPU 活全在主线程，永不逃逸；
// `Send` 只为 `Arc<AndroidPlatform>` 进全局 `RwLock`。
unsafe impl Send for AndroidPlatform {}
unsafe impl Sync for AndroidPlatform {}

impl AndroidPlatform {
    /// 建平台（Android 下须在主线程：分发器取其 looper）。
    pub fn new(headless: bool) -> Self {
        log::info!("AndroidPlatform 建：headless={headless}");
        let text_system = Arc::new(CosmicTextSystem::new_without_system_fonts("Roboto"));
        let this = Self {
            dispatcher: AndroidDispatcher::new(),
            text_system: Arc::clone(&text_system),
            gpu_context: Rc::new(RefCell::new(None)),
            windows: parking_lot::Mutex::new(WindowList::default()),
            displays: parking_lot::Mutex::new(DisplayList::single(AndroidDisplay::headless(
                1080, 1920,
            ))),
            clipboard: parking_lot::Mutex::new(None),
            finish_launching: parking_lot::Mutex::new(None),
            on_init_window: parking_lot::Mutex::new(None),
            quit_callback: parking_lot::Mutex::new(None),
            reopen_callback: parking_lot::Mutex::new(None),
            open_urls_callback: parking_lot::Mutex::new(None),
            keyboard_layout_callback: parking_lot::Mutex::new(None),
            is_active: std::sync::atomic::AtomicBool::new(false),
            headless,
            should_quit: std::sync::atomic::AtomicBool::new(false),
            preferred_backend: parking_lot::Mutex::new(AndroidBackend::Vulkan),
        };
        #[cfg(target_os = "android")]
        this.load_system_fonts();
        this
    }

    /// 是否头模式。
    pub fn is_headless(&self) -> bool {
        self.headless
    }

    /// 是否前台。
    pub fn is_active(&self) -> bool {
        self.is_active.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 进前台（生命周期 Resume 调）。
    pub fn did_become_active(&self) {
        self.is_active
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// 进后台（生命周期 Pause 调）。
    pub fn did_enter_background(&self) {
        self.is_active
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// 要求退出（主循环下轮收）。
    pub fn quit_platform(&self) {
        let mut slot = self.quit_callback.lock();
        let can_quit = slot.as_mut().map_or(true, |callback| callback());
        if can_quit {
            self.should_quit
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// 是否要求退出。
    pub fn should_quit(&self) -> bool {
        self.should_quit.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 注册首窗就绪回调（`INIT_WINDOW` 后调一次，应用在此挂视图）。
    pub fn set_on_init_window(&self, callback: impl FnOnce(Arc<AndroidWindow>) + Send + 'static) {
        *self.on_init_window.lock() = Some(Box::new(callback));
    }

    /// 取走首窗就绪回调。
    pub fn take_on_init_window_callback(
        &self,
    ) -> Option<Box<dyn FnOnce(Arc<AndroidWindow>) + Send>> {
        self.on_init_window.lock().take()
    }

    /// 取走启动回调。
    pub fn take_finish_launching_callback(&self) -> Option<Box<dyn FnOnce() + Send>> {
        self.finish_launching.lock().take()
    }

    /// 跑一轮延后任务。
    pub fn tick(&self) {
        self.dispatcher.tick();
    }

    /// 下个延后任务到期（主循环休眠上限）。
    pub fn next_delayed_due(&self) -> Option<std::time::Instant> {
        self.dispatcher.next_delayed_due()
    }

    /// 排空主线程任务（返回跑了几个；大于零调用方可补一帧需求）。
    pub fn flush_main_thread_tasks(&self) -> usize {
        self.dispatcher.flush_main_thread_tasks()
    }

    /// 取主窗。
    pub fn primary_window(&self) -> Option<Arc<AndroidWindow>> {
        self.windows.lock().primary().cloned()
    }

    /// 存活窗口数。
    pub fn window_count(&self) -> usize {
        self.windows.lock().len()
    }

    /// 取共享 GPU 上下文。
    pub fn gpu_context(&self) -> rgpui_wgpu::GpuContext {
        Rc::clone(&self.gpu_context)
    }

    /// 取主显示器。
    pub fn primary_display(&self) -> Option<AndroidDisplay> {
        self.displays.lock().primary().cloned()
    }

    /// 由新原生窗口开窗（`INIT_WINDOW` 首窗走这里）。
    #[cfg(target_os = "android")]
    pub fn open_window(
        &self,
        native_window: ndk::native_window::NativeWindow,
        scale_factor: f32,
        transparent: bool,
    ) -> anyhow::Result<Arc<AndroidWindow>> {
        let window = AndroidWindow::new(
            native_window,
            Rc::clone(&self.gpu_context),
            scale_factor,
            transparent,
        )?;
        self.windows.lock().push(Arc::clone(&window));
        Ok(window)
    }

    /// 首窗显示器由新原生窗口回填（`INIT_WINDOW` 时调）。
    #[cfg(target_os = "android")]
    pub fn update_primary_display(
        &self,
        native_window: &ndk::native_window::NativeWindow,
        asset_manager: &ndk::asset::AssetManager,
    ) {
        let display = AndroidDisplay::from_activity(native_window, asset_manager);
        *self.displays.lock() = DisplayList::single(display);
    }

    /// 键盘布局变化通知。
    pub fn notify_keyboard_layout_change(&self) {
        if let Some(callback) = self.keyboard_layout_callback.lock().as_mut() {
            callback();
        }
    }

    /// 投递重开事件。
    pub fn deliver_reopen(&self) {
        if let Some(callback) = self.reopen_callback.lock().as_mut() {
            callback();
        }
    }

    /// 投递链接打开事件。
    pub fn deliver_open_urls(&self, urls: Vec<String>) {
        if let Some(callback) = self.open_urls_callback.lock().as_mut() {
            callback(urls);
        }
    }

    /// GPU 后端偏好。
    pub fn preferred_backend(&self) -> AndroidBackend {
        *self.preferred_backend.lock()
    }

    /// 改 GPU 后端偏好。
    pub fn set_preferred_backend(&self, backend: AndroidBackend) {
        *self.preferred_backend.lock() = backend;
    }

    /// 装系统字体 + Emoji（仅 Android：读 `/system/fonts` 与 APK assets）。
    #[cfg(target_os = "android")]
    fn load_system_fonts(&self) {
        const FONT_PATHS: &[&str] = &[
            "/system/fonts/Roboto-Regular.ttf",
            "/system/fonts/Roboto-Bold.ttf",
            "/system/fonts/Roboto-Italic.ttf",
            "/system/fonts/Roboto-BoldItalic.ttf",
            "/system/fonts/Roboto-Medium.ttf",
            "/system/fonts/Roboto-Light.ttf",
            "/system/fonts/DroidSans.ttf",
            "/system/fonts/DroidSans-Bold.ttf",
            "/system/fonts/DroidSansMono.ttf",
            "/system/fonts/NotoSans-Regular.ttf",
            "/system/fonts/NotoSans-Bold.ttf",
            "/system/fonts/NotoSansCJK-Regular.ttc",
            "/system/fonts/NotoSerif-Regular.ttf",
            "/system/fonts/NotoSerif-Bold.ttf",
            // 旗帜 Emoji（CBDT 位图，swash 可渲）。
            "/system/fonts/NotoColorEmojiFlags.ttf",
        ];
        let mut data: Vec<std::borrow::Cow<'static, [u8]>> = Vec::new();
        for path in FONT_PATHS {
            match std::fs::read(path) {
                Ok(bytes) => {
                    log::info!("系统字体：{path}（{} 字节）", bytes.len());
                    data.push(std::borrow::Cow::Owned(bytes));
                }
                Err(error) => log::debug!("跳过系统字体 {path}：{error}"),
            }
        }
        // Emoji：系统 NotoColorEmoji 在 API 33+ 是 swash 渲不动的 COLR v1，
        // 有 CBDT 表才用系统版，否则读 APK assets 里带的位图版。
        let mut emoji_ready = false;
        if let Ok(system_emoji) = std::fs::read("/system/fonts/NotoColorEmoji.ttf") {
            if has_cbdt_tables(&system_emoji) {
                log::info!("系统 Emoji 含 CBDT，直接用");
                data.push(std::borrow::Cow::Owned(system_emoji));
                emoji_ready = true;
            } else {
                log::info!("系统 Emoji 是 COLR v1，找 APK 内兜底");
            }
        }
        if !emoji_ready {
            if let Some(app) = super::bridge::android_app() {
                let manager = app.asset_manager();
                let path =
                    std::ffi::CString::new("fonts/NotoColorEmoji.ttf").expect("资源路径转 C 串");
                match manager.open(&path) {
                    Some(mut asset) => match asset.buffer() {
                        Ok(buffer) => {
                            log::info!("APK 内 Emoji（{} 字节）", buffer.len());
                            data.push(std::borrow::Cow::Owned(buffer.to_vec()));
                            emoji_ready = true;
                        }
                        Err(error) => {
                            log::warn!("APK Emoji 读缓冲失败：{error}")
                        }
                    },
                    None => log::debug!("APK 内无 fonts/NotoColorEmoji.ttf"),
                }
            }
        }
        if !emoji_ready {
            log::warn!("无可用 Emoji 字体，表情可能显示豆腐块");
        }
        if data.is_empty() {
            log::warn!("/system/fonts 下无可用字体");
        } else if let Err(error) = self.text_system.add_fonts(data) {
            log::warn!("系统字体装载失败：{error:#}");
        }
    }
}

/// 字体文件是否含 CBDT（位图 Emoji）表（COLR v1 的 swash 渲不动）。
#[cfg(target_os = "android")]
fn has_cbdt_tables(data: &[u8]) -> bool {
    if data.len() < 12 {
        return false;
    }
    let table_count = u16::from_be_bytes([data[4], data[5]]) as usize;
    if data.len() < 12 + table_count * 16 {
        return false;
    }
    (0..table_count).any(|index| {
        let offset = 12 + index * 16;
        &data[offset..offset + 4] == b"CBDT"
    })
}

/// `Arc<AndroidPlatform>` 的 `Rc` 兼容包装（交 `Application::with_platform`）。
///
/// [`Platform`] 是 `Rc` 语义（单线程），全局平台是 `Arc`（跨线程共享）；
/// 本包装自己实现 [`Platform`]，逐项转发给内部 `Arc`（`Rc::new` 包一层即可，
/// 状态仍在全局单例里，窗口表/分发器不分叉）。
pub struct SharedPlatform {
    /// 全局平台。
    inner: Arc<AndroidPlatform>,
}

impl SharedPlatform {
    /// 包全局平台。
    pub fn new(platform: Arc<AndroidPlatform>) -> Self {
        Self { inner: platform }
    }

    /// 包成 `Rc<dyn Platform>`（`Application::with_platform` 要这个）。
    pub fn into_rc(self) -> Rc<dyn Platform> {
        Rc::new(self)
    }
}

impl Platform for SharedPlatform {
    fn background_executor(&self) -> BackgroundExecutor {
        self.inner.background_executor()
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        self.inner.foreground_executor()
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.inner.text_system()
    }

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        self.inner.run(on_finish_launching)
    }

    fn quit(&self) {
        self.inner.quit()
    }

    fn restart(&self, binary_path: Option<PathBuf>) {
        self.inner.restart(binary_path)
    }

    fn activate(&self, ignoring_other_apps: bool) {
        self.inner.activate(ignoring_other_apps)
    }

    fn hide(&self) {
        self.inner.hide()
    }

    fn hide_other_apps(&self) {
        self.inner.hide_other_apps()
    }

    fn unhide_other_apps(&self) {
        self.inner.unhide_other_apps()
    }

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        self.inner.displays()
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.inner
            .primary_display()
            .map(|display| Rc::new(display) as Rc<dyn PlatformDisplay>)
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        self.inner.active_window()
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>> {
        // 注意：同名 inherent 方法（原生窗口开窗）只在真机用，
        // 这里显式走 trait 方法包已有主窗。
        <AndroidPlatform as Platform>::open_window(&self.inner, handle, options)
    }

    fn window_appearance(&self) -> WindowAppearance {
        self.inner.window_appearance()
    }

    fn open_url(&self, url: &str) {
        self.inner.open_url(url)
    }

    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        self.inner.on_open_urls(callback)
    }

    fn register_url_scheme(&self, url: &str) -> Task<anyhow::Result<()>> {
        self.inner.register_url_scheme(url)
    }

    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> futures::channel::oneshot::Receiver<anyhow::Result<Option<Vec<PathBuf>>>> {
        self.inner.prompt_for_paths(options)
    }

    fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> futures::channel::oneshot::Receiver<anyhow::Result<Option<PathBuf>>> {
        self.inner.prompt_for_new_path(directory, suggested_name)
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        self.inner.can_select_mixed_files_and_dirs()
    }

    fn reveal_path(&self, path: &Path) {
        self.inner.reveal_path(path)
    }

    fn open_with_system(&self, path: &Path) {
        self.inner.open_with_system(path)
    }

    fn on_quit(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_quit(callback)
    }

    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_reopen(callback)
    }

    fn set_menus(&self, menus: Vec<Menu>, keymap: &Keymap) {
        self.inner.set_menus(menus, keymap)
    }

    fn set_dock_menu(&self, menu: Vec<MenuItem>, keymap: &Keymap) {
        self.inner.set_dock_menu(menu, keymap)
    }

    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>) {
        self.inner.on_app_menu_action(callback)
    }

    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_will_open_app_menu(callback)
    }

    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        self.inner.on_validate_app_menu_command(callback)
    }

    fn thermal_state(&self) -> ThermalState {
        self.inner.thermal_state()
    }

    fn on_thermal_state_change(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_thermal_state_change(callback)
    }

    fn app_path(&self) -> anyhow::Result<PathBuf> {
        self.inner.app_path()
    }

    fn path_for_auxiliary_executable(&self, name: &str) -> anyhow::Result<PathBuf> {
        self.inner.path_for_auxiliary_executable(name)
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        self.inner.set_cursor_style(style)
    }

    fn hide_cursor_until_mouse_moves(&self) {
        self.inner.hide_cursor_until_mouse_moves()
    }

    fn is_cursor_visible(&self) -> bool {
        self.inner.is_cursor_visible()
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        self.inner.should_auto_hide_scrollbars()
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.inner.read_from_clipboard()
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        self.inner.write_to_clipboard(item)
    }

    /// 从 Linux 主选择区读取（宿主机 check 用，透传内部实现）。
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem> {
        self.inner.read_from_primary()
    }

    /// 写入 Linux 主选择区（宿主机 check 用，透传内部实现）。
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, item: ClipboardItem) {
        self.inner.write_to_primary(item)
    }

    /// 从 macOS 查找粘贴板读取（宿主机 check 用，透传内部实现）。
    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        self.inner.read_from_find_pasteboard()
    }

    /// 写入 macOS 查找粘贴板（宿主机 check 用，透传内部实现）。
    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, item: ClipboardItem) {
        self.inner.write_to_find_pasteboard(item)
    }

    fn write_credentials(
        &self,
        url: &str,
        username: &str,
        password: &[u8],
    ) -> Task<anyhow::Result<()>> {
        self.inner.write_credentials(url, username, password)
    }

    fn read_credentials(&self, url: &str) -> Task<anyhow::Result<Option<(String, Vec<u8>)>>> {
        self.inner.read_credentials(url)
    }

    fn delete_credentials(&self, url: &str) -> Task<anyhow::Result<()>> {
        self.inner.delete_credentials(url)
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        self.inner.keyboard_layout()
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        self.inner.keyboard_mapper()
    }

    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_keyboard_layout_change(callback)
    }
}

impl Platform for AndroidPlatform {
    fn background_executor(&self) -> BackgroundExecutor {
        BackgroundExecutor::new(self.dispatcher.clone())
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        ForegroundExecutor::new(self.dispatcher.clone())
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text_system.clone()
    }

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        // trait 给的回调非 Send；Android 下恒主线程调，转 Send 安全（实践）。
        let send_callback: Box<dyn FnOnce() + Send> =
            unsafe { std::mem::transmute(on_finish_launching) };
        #[cfg(target_os = "android")]
        {
            // 阻塞跑事件循环：回调延后到首窗就绪（`bridge` 内调），
            // `Application` 全程活在调用栈，GPUI 弱引用不断。
            *self.finish_launching.lock() = Some(send_callback);
            if let Some(app) = super::bridge::android_app() {
                super::bridge::run_event_loop(&app);
                return;
            }
            // 无 AndroidApp（单测）：直接调回调即返。
            if let Some(callback) = self.take_finish_launching_callback() {
                callback();
            }
        }
        #[cfg(not(target_os = "android"))]
        {
            send_callback();
        }
    }

    fn quit(&self) {
        self.quit_platform();
    }

    fn restart(&self, _binary_path: Option<PathBuf>) {
        log::warn!("Android 不支持 restart");
    }

    fn activate(&self, _ignoring_other_apps: bool) {
        self.did_become_active();
    }

    fn hide(&self) {}

    fn hide_other_apps(&self) {}

    fn unhide_other_apps(&self) {}

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        self.displays
            .lock()
            .all()
            .iter()
            .cloned()
            .map(|display| Rc::new(display) as Rc<dyn PlatformDisplay>)
            .collect()
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.primary_display()
            .map(|display| Rc::new(display) as Rc<dyn PlatformDisplay>)
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        // 单窗语义，活动窗由应用上下文跟踪，此处空。
        None
    }

    fn open_window(
        &self,
        _handle: AnyWindowHandle,
        _options: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>> {
        // 原生窗口系统建（`INIT_WINDOW`），应用层开窗即包已有主窗。
        let window = self
            .primary_window()
            .ok_or_else(|| anyhow::anyhow!("原生窗口未就绪（on_init_window 回调之后再开窗）"))?;
        let display = self
            .primary_display()
            .map(|display| Rc::new(display) as Rc<dyn PlatformDisplay>);
        Ok(Box::new(AndroidPlatformWindow::new(window, display)))
    }

    fn window_appearance(&self) -> WindowAppearance {
        #[cfg(target_os = "android")]
        {
            super::bridge::system_window_appearance()
        }
        #[cfg(not(target_os = "android"))]
        {
            WindowAppearance::Dark
        }
    }

    fn open_url(&self, url: &str) {
        #[cfg(target_os = "android")]
        {
            super::bridge::open_url_android(url);
        }
        #[cfg(not(target_os = "android"))]
        {
            log::info!("open_url({url})：主机桩");
        }
    }

    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        // SAFETY：恒主线程，转 Send 存槽。
        *self.open_urls_callback.lock() = Some(unsafe {
            std::mem::transmute::<Box<dyn FnMut(Vec<String>)>, Box<dyn FnMut(Vec<String>) + Send>>(
                callback,
            )
        });
    }

    fn register_url_scheme(&self, _url: &str) -> Task<anyhow::Result<()>> {
        Task::ready(Ok(()))
    }

    fn prompt_for_paths(
        &self,
        _options: PathPromptOptions,
    ) -> futures::channel::oneshot::Receiver<anyhow::Result<Option<Vec<PathBuf>>>> {
        // 要自定义 Activity（`ACTION_OPEN_DOCUMENT`），M3 再接；M2 空选择。
        let (tx, rx) = futures::channel::oneshot::channel();
        let _ = tx.send(Ok(None));
        rx
    }

    fn prompt_for_new_path(
        &self,
        _directory: &Path,
        _suggested_name: Option<&str>,
    ) -> futures::channel::oneshot::Receiver<anyhow::Result<Option<PathBuf>>> {
        let (tx, rx) = futures::channel::oneshot::channel();
        let _ = tx.send(Ok(None));
        rx
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false
    }

    fn reveal_path(&self, _path: &Path) {}

    fn open_with_system(&self, _path: &Path) {}

    fn on_quit(&self, callback: Box<dyn FnMut()>) {
        let mut callback = callback;
        let wrapper: Box<dyn FnMut() -> bool> = Box::new(move || {
            callback();
            true
        });
        // SAFETY：恒主线程，转 Send 存槽。
        *self.quit_callback.lock() = Some(unsafe {
            std::mem::transmute::<Box<dyn FnMut() -> bool>, Box<dyn FnMut() -> bool + Send>>(
                wrapper,
            )
        });
    }

    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        // SAFETY：恒主线程，转 Send 存槽。
        *self.reopen_callback.lock() = Some(unsafe {
            std::mem::transmute::<Box<dyn FnMut()>, Box<dyn FnMut() + Send>>(callback)
        });
    }

    fn set_menus(&self, _menus: Vec<Menu>, _keymap: &Keymap) {}

    fn set_dock_menu(&self, _menu: Vec<MenuItem>, _keymap: &Keymap) {}

    fn on_app_menu_action(&self, _callback: Box<dyn FnMut(&dyn Action)>) {}

    fn on_will_open_app_menu(&self, _callback: Box<dyn FnMut()>) {}

    fn on_validate_app_menu_command(&self, _callback: Box<dyn FnMut(&dyn Action) -> bool>) {}

    fn thermal_state(&self) -> ThermalState {
        ThermalState::Nominal
    }

    fn on_thermal_state_change(&self, _callback: Box<dyn FnMut()>) {}

    fn app_path(&self) -> anyhow::Result<PathBuf> {
        Ok(std::fs::read_link("/proc/self/exe").unwrap_or_default())
    }

    fn path_for_auxiliary_executable(&self, _name: &str) -> anyhow::Result<PathBuf> {
        anyhow::bail!("Android 不支持辅助可执行文件")
    }

    fn set_cursor_style(&self, _style: CursorStyle) {}

    fn hide_cursor_until_mouse_moves(&self) {}

    fn is_cursor_visible(&self) -> bool {
        false
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        true
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        #[cfg(target_os = "android")]
        {
            // 真机读系统剪贴板；失败回退进程内缓存。
            super::bridge::read_clipboard_android()
                .map(ClipboardItem::new_string)
                .or_else(|| self.clipboard.lock().clone().map(ClipboardItem::new_string))
        }
        #[cfg(not(target_os = "android"))]
        {
            self.clipboard.lock().clone().map(ClipboardItem::new_string)
        }
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        let text = item.text().unwrap_or_default();
        #[cfg(target_os = "android")]
        {
            *self.clipboard.lock() = Some(text.clone());
            super::bridge::write_clipboard_android(&text);
        }
        #[cfg(not(target_os = "android"))]
        {
            *self.clipboard.lock() = Some(text);
        }
    }

    /// 从 Linux 主选择区读取（宿主机 check 用桩：无主选择区）。
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem> {
        None
    }

    /// 写入 Linux 主选择区（宿主机 check 用桩：无操作）。
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, _item: ClipboardItem) {}

    /// 从 macOS 查找粘贴板读取（宿主机 check 用桩：无查找粘贴板）。
    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        None
    }

    /// 写入 macOS 查找粘贴板（宿主机 check 用桩：无操作）。
    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, _item: ClipboardItem) {}

    fn write_credentials(
        &self,
        _url: &str,
        _username: &str,
        _password: &[u8],
    ) -> Task<anyhow::Result<()>> {
        Task::ready(Err(anyhow::anyhow!("Android 凭据库 M3 再接")))
    }

    fn read_credentials(&self, _url: &str) -> Task<anyhow::Result<Option<(String, Vec<u8>)>>> {
        Task::ready(Ok(None))
    }

    fn delete_credentials(&self, _url: &str) -> Task<anyhow::Result<()>> {
        Task::ready(Ok(()))
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        #[cfg(target_os = "android")]
        {
            Box::new(AndroidKeyboardLayout::new(
                &super::bridge::keyboard_layout_id(),
            ))
        }
        #[cfg(not(target_os = "android"))]
        {
            Box::new(AndroidKeyboardLayout::new("en-US"))
        }
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(AndroidKeyboardMapper)
    }

    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
        // SAFETY：恒主线程，转 Send 存槽。
        *self.keyboard_layout_callback.lock() = Some(unsafe {
            std::mem::transmute::<Box<dyn FnMut()>, Box<dyn FnMut() + Send>>(callback)
        });
    }
}
