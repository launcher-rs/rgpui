//! iOS 平台实现预留（1.4.0 M1 结构桩，真机实现见 M4）。
//!
//! 保证 `--target aarch64-apple-ios` 可 check：窗口/显示器/文本系统均为桩实现。
//! iOS 特有依赖一律 `cfg(target_os = "ios")` 门控。

use rgpui::{
    Action, AnyWindowHandle, BackgroundExecutor, BatteryStatus, ClipboardItem, CursorStyle,
    ForegroundExecutor, Keymap, Menu, MenuItem, NoopTextSystem, PathPromptOptions, Platform,
    PlatformDisplay, PlatformKeyboardLayout, PlatformKeyboardMapper, PlatformTextSystem,
    PlatformWindow, Task, ThermalState, WindowAppearance, WindowParams,
};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

mod dispatcher;
mod display;
mod window;

use dispatcher::IosDispatcher;
use display::IosDisplay;
use window::{IosPlatformWindow, IosWindow};

/// iOS 平台实现（M1 桩）。
pub struct IosPlatform {
    /// 前后台任务分发器。
    dispatcher: Arc<IosDispatcher>,
    /// 文本系统（M1 空实现）。
    text_system: Arc<NoopTextSystem>,
    /// 显示器桩。
    displays: Vec<IosDisplay>,
    /// 是否头模式。
    headless: bool,
}

impl IosPlatform {
    /// 构造 iOS 平台实例。
    ///
    /// /// `headless` 为 true 时不触碰任何原生窗口，专供主机单测与文档构建。
    pub fn new(headless: bool) -> Self {
        log::info!("IosPlatform::new(headless={headless})");
        Self {
            dispatcher: Arc::new(IosDispatcher::new()),
            text_system: Arc::new(NoopTextSystem::new()),
            displays: vec![IosDisplay::headless(844, 390)],
            headless,
        }
    }

    /// 是否头模式。
    pub fn is_headless(&self) -> bool {
        self.headless
    }
}

impl Platform for IosPlatform {
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
        on_finish_launching();
    }

    fn quit(&self) {
        log::info!("IosPlatform::quit");
    }

    fn restart(&self, _binary_path: Option<PathBuf>) {
        log::warn!("IosPlatform::restart 在 iOS 上不支持");
    }

    fn activate(&self, _ignoring_other_apps: bool) {}

    fn hide(&self) {}

    fn hide_other_apps(&self) {}

    fn unhide_other_apps(&self) {}

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        self.displays
            .iter()
            .cloned()
            .map(|display| Rc::new(display) as Rc<dyn PlatformDisplay>)
            .collect()
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.displays
            .first()
            .cloned()
            .map(|display| Rc::new(display) as Rc<dyn PlatformDisplay>)
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        None
    }

    fn open_window(
        &self,
        _handle: AnyWindowHandle,
        _options: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>> {
        let window = IosWindow::headless(844.0, 390.0, 3.0);
        let display = self.primary_display();
        Ok(Box::new(IosPlatformWindow::new(window, display)))
    }

    fn window_appearance(&self) -> WindowAppearance {
        WindowAppearance::Light
    }

    fn open_url(&self, url: &str) {
        log::info!("IosPlatform::open_url({url}) 尚未实现");
    }

    fn on_open_urls(&self, _callback: Box<dyn FnMut(Vec<String>)>) {}

    fn register_url_scheme(&self, _url: &str) -> Task<anyhow::Result<()>> {
        Task::ready(Ok(()))
    }

    fn prompt_for_paths(
        &self,
        _options: PathPromptOptions,
    ) -> futures::channel::oneshot::Receiver<anyhow::Result<Option<Vec<PathBuf>>>> {
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

    fn on_quit(&self, _callback: Box<dyn FnMut()>) {}

    fn on_reopen(&self, _callback: Box<dyn FnMut()>) {}

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
        Ok(PathBuf::new())
    }

    fn path_for_auxiliary_executable(&self, _name: &str) -> anyhow::Result<PathBuf> {
        anyhow::bail!("iOS 上不支持辅助可执行文件")
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
        None
    }

    fn write_to_clipboard(&self, _item: ClipboardItem) {}

    /// 触发振动（M1 桩：M4 随真实现接 `AudioToolbox`）。
    fn vibrate(&self, _duration_ms: u64) {}

    /// 读取电池状态（M1 桩：恒为未知）。
    fn battery_status(&self) -> BatteryStatus {
        BatteryStatus::unknown()
    }

    /// 从 Linux 主选择区读取（M1 桩：iOS 无主选择区，宿主机 check 用）。
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem> {
        None
    }

    /// 写入 Linux 主选择区（M1 桩：无操作）。
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, _item: ClipboardItem) {}

    /// 从 macOS 查找粘贴板读取（M1 桩：宿主机 check 用）。
    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        None
    }

    /// 写入 macOS 查找粘贴板（M1 桩：无操作）。
    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, _item: ClipboardItem) {}

    fn write_credentials(
        &self,
        _url: &str,
        _username: &str,
        _password: &[u8],
    ) -> Task<anyhow::Result<()>> {
        Task::ready(Err(anyhow::anyhow!("iOS 凭据库在 M4 前不支持")))
    }

    fn read_credentials(&self, _url: &str) -> Task<anyhow::Result<Option<(String, Vec<u8>)>>> {
        Task::ready(Ok(None))
    }

    fn delete_credentials(&self, _url: &str) -> Task<anyhow::Result<()>> {
        Task::ready(Ok(()))
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(IosKeyboardLayout::new("en-US"))
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(rgpui::DummyKeyboardMapper)
    }

    fn on_keyboard_layout_change(&self, _callback: Box<dyn FnMut()>) {}
}

/// iOS 键盘布局（M1 固定 `en-US`）。
struct IosKeyboardLayout {
    /// 布局标识。
    id: String,
}

impl IosKeyboardLayout {
    /// 构造指定标识的键盘布局。
    fn new(id: &str) -> Self {
        Self { id: id.to_string() }
    }
}

impl PlatformKeyboardLayout for IosKeyboardLayout {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.id
    }
}

/// 返回 iOS 平台实现（与 `rgpui_linux::current_platform` 同签名）。
///
/// /// `headless` 为 true 时不触碰任何原生窗口，专供主机单测与文档构建。
pub fn current_platform(headless: bool) -> Rc<dyn Platform> {
    Rc::new(IosPlatform::new(headless))
}
