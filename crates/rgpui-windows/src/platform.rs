use std::{
    cell::{Cell, RefCell},
    ffi::OsStr,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use ::rgpui::util::ResultExt;
use anyhow::{Context as _, Result, anyhow};
use futures::channel::oneshot::{self, Receiver};
use itertools::Itertools;
use parking_lot::RwLock;
use smallvec::SmallVec;
use windows::{
    UI::ViewManagement::UISettings,
    Win32::{
        Foundation::*,
        Graphics::{Direct3D11::ID3D11Device, Gdi::*},
        Security::Credentials::*,
        System::{Com::*, LibraryLoader::*, Ole::*, SystemInformation::*},
        UI::{Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
    },
    core::*,
};

use crate::*;
use rgpui::*;

/// Windows 平台实现，负责管理整个应用程序的生命周期
///
/// 该结构体实现了 GPUI 的 `Platform` trait，提供了以下功能：
/// - 窗口创建和管理
/// - 事件循环处理
/// - 剪贴板操作
/// - 系统对话框（文件打开/保存）
/// - 显示器枚举
/// - 键盘和鼠标输入处理
/// - GPU 设备丢失恢复
pub struct WindowsPlatform {
    inner: Rc<WindowsPlatformInner>,
    raw_window_handles: Arc<RwLock<SmallVec<[SafeHwnd; 4]>>>,
    // 以下成员在应用整个生命周期中不会改变
    headless: bool,
    icon: HICON,
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,
    text_system: Arc<dyn PlatformTextSystem>,
    direct_write_text_system: Option<Arc<DirectWriteTextSystem>>,
    drop_target_helper: Option<IDropTargetHelper>,
    /// 标记用于指示 `VSyncProvider` 线程使 DirectX 设备失效
    /// 因为调整它们的大小失败了，导致我们至少丢失了渲染目标
    invalidate_devices: Arc<AtomicBool>,
    handle: HWND,
    disable_direct_composition: bool,
}

struct WindowsPlatformInner {
    state: WindowsPlatformState,
    raw_window_handles: std::sync::Weak<RwLock<SmallVec<[SafeHwnd; 4]>>>,
    // 以下成员在应用整个生命周期中不会改变
    validation_number: usize,
    main_receiver: PriorityQueueReceiver<RunnableVariant>,
    dispatcher: Arc<WindowsDispatcher>,
}

pub(crate) struct WindowsPlatformState {
    callbacks: PlatformCallbacks,
    menus: RefCell<Vec<OwnedMenu>>,
    jump_list: RefCell<JumpList>,
    tray: RefCell<Option<WindowsTray>>,
    tray_menu_actions: RefCell<std::collections::HashMap<muda::MenuId, Box<dyn Action>>>,
    // 新增：托盘事件回调
    tray_icon_event_callback: RefCell<Option<Box<dyn FnMut(TrayIconEvent)>>>,
    tray_menu_action_callback: RefCell<Option<Box<dyn FnMut(SharedString)>>>,
    // 注意：标准光标句柄不需要关闭
    pub(crate) current_cursor: Cell<Option<HCURSOR>>,
    /// 与每个窗口共享，以便 `WM_SETCURSOR` 可以直接读取
    pub(crate) cursor_visible: Arc<AtomicBool>,
    directx_devices: RefCell<Option<DirectXDevices>>,
    // 新增：全局快捷键管理器
    global_hotkey: RefCell<WindowsGlobalHotkey>,
    // 新增：全局快捷键回调
    global_hotkey_callback: RefCell<Option<Box<dyn FnMut(u32)>>>,
    // 新增：无窗口保持运行标志
    keep_alive_without_windows: AtomicBool,
}

#[derive(Default)]
struct PlatformCallbacks {
    open_urls: Cell<Option<Box<dyn FnMut(Vec<String>)>>>,
    quit: Cell<Option<Box<dyn FnMut()>>>,
    reopen: Cell<Option<Box<dyn FnMut()>>>,
    app_menu_action: Cell<Option<Box<dyn FnMut(&dyn Action)>>>,
    will_open_app_menu: Cell<Option<Box<dyn FnMut()>>>,
    validate_app_menu_command: Cell<Option<Box<dyn FnMut(&dyn Action) -> bool>>>,
    keyboard_layout_change: Cell<Option<Box<dyn FnMut()>>>,
}

impl WindowsPlatformState {
    fn new(directx_devices: Option<DirectXDevices>) -> Self {
        let callbacks = PlatformCallbacks::default();
        let jump_list = JumpList::new();
        let current_cursor = load_cursor(CursorStyle::Arrow);

        Self {
            callbacks,
            jump_list: RefCell::new(jump_list),
            current_cursor: Cell::new(current_cursor),
            cursor_visible: Arc::new(AtomicBool::new(true)),
            directx_devices: RefCell::new(directx_devices),
            menus: RefCell::new(Vec::new()),
            tray: RefCell::new(None),
            tray_menu_actions: RefCell::new(std::collections::HashMap::new()),
            tray_icon_event_callback: RefCell::new(None),
            tray_menu_action_callback: RefCell::new(None),
            global_hotkey: RefCell::new(WindowsGlobalHotkey::new()),
            global_hotkey_callback: RefCell::new(None),
            keep_alive_without_windows: AtomicBool::new(false),
        }
    }
}

impl WindowsPlatform {
    /// 创建新的 Windows 平台实例
    ///
    /// # 参数
    /// * `headless` - 如果为 true，则不初始化 DirectX 设备和文本系统，适用于无头模式
    ///
    /// # 返回
    /// 返回初始化成功的 `WindowsPlatform` 实例，或初始化失败的错误
    pub fn new(headless: bool) -> Result<Self> {
        unsafe {
            OleInitialize(None).context("unable to initialize Windows OLE")?;
        }
        let (directx_devices, text_system, direct_write_text_system) = if !headless {
            let devices = DirectXDevices::new().context("Creating DirectX devices")?;
            let dw_text_system = Arc::new(
                DirectWriteTextSystem::new(&devices)
                    .context("Error creating DirectWriteTextSystem")?,
            );
            (
                Some(devices),
                dw_text_system.clone() as Arc<dyn PlatformTextSystem>,
                Some(dw_text_system),
            )
        } else {
            (
                None,
                Arc::new(rgpui::NoopTextSystem::new()) as Arc<dyn PlatformTextSystem>,
                None,
            )
        };

        let (main_sender, main_receiver) = PriorityQueueReceiver::new();
        let validation_number = if usize::BITS == 64 {
            rand::random::<u64>() as usize
        } else {
            rand::random::<u32>() as usize
        };
        let raw_window_handles = Arc::new(RwLock::new(SmallVec::new()));

        register_platform_window_class();
        let mut context = PlatformWindowCreateContext {
            inner: None,
            raw_window_handles: Arc::downgrade(&raw_window_handles),
            validation_number,
            main_sender: Some(main_sender),
            main_receiver: Some(main_receiver),
            directx_devices,
            dispatcher: None,
        };
        let result = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PLATFORM_WINDOW_CLASS_NAME,
                None,
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                Some(&raw const context as *const _),
            )
        };
        let inner = context
            .inner
            .take()
            .context("CreateWindowExW did not run correctly")??;
        let dispatcher = context
            .dispatcher
            .take()
            .context("CreateWindowExW did not run correctly")?;
        let handle = result?;

        let disable_direct_composition = std::env::var(DISABLE_DIRECT_COMPOSITION)
            .is_ok_and(|value| value == "true" || value == "1");
        let background_executor = BackgroundExecutor::new(dispatcher.clone());
        let foreground_executor = ForegroundExecutor::new(dispatcher);

        run_muda_menu_event_listener(&foreground_executor, &inner);

        let drop_target_helper: Option<IDropTargetHelper> = if !headless {
            Some(unsafe {
                CoCreateInstance(&CLSID_DragDropHelper, None, CLSCTX_INPROC_SERVER)
                    .context("Error creating drop target helper.")?
            })
        } else {
            None
        };
        let icon = if !headless {
            load_icon().unwrap_or_default()
        } else {
            HICON::default()
        };

        Ok(Self {
            inner,
            handle,
            raw_window_handles,
            headless,
            icon,
            background_executor,
            foreground_executor,
            text_system,
            direct_write_text_system,
            disable_direct_composition,
            drop_target_helper,
            invalidate_devices: Arc::new(AtomicBool::new(false)),
        })
    }

    /// 根据窗口句柄查找对应的窗口内部对象
    ///
    /// # 参数
    /// * `hwnd` - Windows 窗口句柄
    ///
    /// # 返回
    /// 如果找到对应的窗口，返回其内部对象的引用计数指针
    pub(crate) fn window_from_hwnd(&self, hwnd: HWND) -> Option<Rc<WindowsWindowInner>> {
        self.raw_window_handles
            .read()
            .iter()
            .find(|entry| entry.as_raw() == hwnd)
            .and_then(|hwnd| window_from_hwnd(hwnd.as_raw()))
    }

    /// 向所有窗口发送消息
    ///
    /// # 参数
    /// * `message` - 要发送的 Windows 消息 ID
    /// * `wparam` - 消息的 wParam 参数
    /// * `lparam` - 消息的 lParam 参数
    #[inline]
    fn post_message(&self, message: u32, wparam: WPARAM, lparam: LPARAM) {
        self.raw_window_handles
            .read()
            .iter()
            .for_each(|handle| unsafe {
                PostMessageW(Some(handle.as_raw()), message, wparam, lparam).log_err();
            });
    }

    /// 收集平台级别共享的资源配置，用于传递给新创建的窗口
    ///
    /// # 返回
    /// 返回窗口创建信息结构体
    fn generate_creation_info(&self) -> WindowCreationInfo {
        WindowCreationInfo {
            icon: self.icon,
            executor: self.foreground_executor.clone(),
            current_cursor: self.inner.state.current_cursor.get(),
            cursor_visible: self.inner.state.cursor_visible.clone(),
            drop_target_helper: self.drop_target_helper.clone().unwrap(),
            validation_number: self.inner.validation_number,
            main_receiver: self.inner.main_receiver.clone(),
            platform_window_handle: self.handle,
            disable_direct_composition: self.disable_direct_composition,
            directx_devices: self.inner.state.directx_devices.borrow().clone().unwrap(),
            invalidate_devices: self.invalidate_devices.clone(),
        }
    }

    fn set_dock_menus(&self, menus: Vec<MenuItem>) {
        let mut actions = Vec::new();
        menus.into_iter().for_each(|menu| {
            if let Some(dock_menu) = DockMenuItem::new(menu).log_err() {
                actions.push(dock_menu);
            }
        });
        self.inner.state.jump_list.borrow_mut().dock_menus = actions;
        let borrow = self.inner.state.jump_list.borrow();
        let dock_menus = borrow
            .dock_menus
            .iter()
            .map(|menu| (menu.name.clone(), menu.description.clone()))
            .collect::<Vec<_>>();
        let recent_workspaces = borrow.recent_workspaces.clone();
        self.background_executor
            .spawn(async move {
                update_jump_list(&recent_workspaces, &dock_menus).log_err();
            })
            .detach();
    }

    fn update_jump_list(
        &self,
        menus: Vec<MenuItem>,
        entries: Vec<SmallVec<[PathBuf; 2]>>,
    ) -> Task<Vec<SmallVec<[PathBuf; 2]>>> {
        let mut actions = Vec::new();
        menus.into_iter().for_each(|menu| {
            if let Some(dock_menu) = DockMenuItem::new(menu).log_err() {
                actions.push(dock_menu);
            }
        });
        let mut jump_list = self.inner.state.jump_list.borrow_mut();
        jump_list.dock_menus = actions;
        jump_list.recent_workspaces = entries.into();
        let dock_menus = jump_list
            .dock_menus
            .iter()
            .map(|menu| (menu.name.clone(), menu.description.clone()))
            .collect::<Vec<_>>();
        let recent_workspaces = jump_list.recent_workspaces.clone();
        self.background_executor.spawn(async move {
            update_jump_list(&recent_workspaces, &dock_menus)
                .log_err()
                .unwrap_or_default()
        })
    }

    /// 查找当前处于活动状态的窗口句柄
    fn find_current_active_window(&self) -> Option<HWND> {
        let active_window_hwnd = unsafe { GetActiveWindow() };
        if active_window_hwnd.is_invalid() {
            return None;
        }
        self.raw_window_handles
            .read()
            .iter()
            .find(|hwnd| hwnd.as_raw() == active_window_hwnd)
            .map(|hwnd| hwnd.as_raw())
    }

    /// 启动 VSync 同步线程
    ///
    /// 该线程负责：
    /// - 等待垂直同步信号
    /// - 检测 GPU 设备丢失状态
    /// - 触发所有窗口的重绘
    fn begin_vsync_thread(&self) {
        let Some(directx_devices) = self.inner.state.directx_devices.borrow().clone() else {
            return;
        };
        let Some(direct_write_text_system) = &self.direct_write_text_system else {
            return;
        };
        let mut directx_device = directx_devices;
        let platform_window: SafeHwnd = self.handle.into();
        let validation_number = self.inner.validation_number;
        let all_windows = Arc::downgrade(&self.raw_window_handles);
        let text_system = Arc::downgrade(direct_write_text_system);
        let invalidate_devices = self.invalidate_devices.clone();

        std::thread::Builder::new()
            .name("VSyncProvider".to_owned())
            .spawn(move || {
                let vsync_provider = VSyncProvider::new();
                loop {
                    vsync_provider.wait_for_vsync();
                    if check_device_lost(&directx_device.device)
                        || invalidate_devices.fetch_and(false, Ordering::Acquire)
                    {
                        if let Err(err) = handle_gpu_device_lost(
                            &mut directx_device,
                            platform_window.as_raw(),
                            validation_number,
                            &all_windows,
                            &text_system,
                        ) {
                            panic!("Device lost: {err}");
                        }
                    }
                    let Some(all_windows) = all_windows.upgrade() else {
                        break;
                    };
                    for hwnd in all_windows.read().iter() {
                        unsafe {
                            let _ = RedrawWindow(Some(hwnd.as_raw()), None, None, RDW_INVALIDATE);
                        }
                    }
                }
            })
            .unwrap();
    }
}

fn translate_accelerator(msg: &MSG) -> Option<()> {
    if msg.message != WM_KEYDOWN && msg.message != WM_SYSKEYDOWN {
        return None;
    }

    let result = unsafe {
        SendMessageW(
            msg.hwnd,
            WM_GPUI_KEYDOWN,
            Some(msg.wParam),
            Some(msg.lParam),
        )
    };
    (result.0 == 0).then_some(())
}

impl Platform for WindowsPlatform {
    fn background_executor(&self) -> BackgroundExecutor {
        self.background_executor.clone()
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        self.foreground_executor.clone()
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text_system.clone()
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(
            WindowsKeyboardLayout::new()
                .log_err()
                .unwrap_or(WindowsKeyboardLayout::unknown()),
        )
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(WindowsKeyboardMapper::new())
    }

    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
        self.inner
            .state
            .callbacks
            .keyboard_layout_change
            .set(Some(callback));
    }

    fn on_thermal_state_change(&self, _callback: Box<dyn FnMut()>) {}

    fn thermal_state(&self) -> ThermalState {
        ThermalState::Nominal
    }

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        on_finish_launching();
        if !self.headless {
            self.begin_vsync_thread();
        }

        let mut msg = MSG::default();
        unsafe {
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                if translate_accelerator(&msg).is_none() {
                    _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

        self.inner
            .with_callback(|callbacks| &callbacks.quit, |callback| callback());

        // 绕过 CRT 退出逻辑，该逻辑在调用 ExitProcess 之前运行 atexit 处理程序。
        // aws-lc 注册了一个 atexit 处理程序，该处理程序故意获取锁而不释放它。
        // aws-lc 还有 thread_local 对象，在其析构函数中获取此锁。
        // thread_locals 的析构函数在加载器锁下运行，因此存在竞争条件
        // 如果在 atexit 处理程序运行后有线程退出，TLS 析构函数将
        // 在持有加载器锁时无限期地阻塞此锁。由于 ExitProcess 也需要
        // 加载器锁，进程拆卸将死锁。
        unsafe {
            windows::Win32::System::Threading::ExitProcess(0);
        }
    }

    fn quit(&self) {
        self.foreground_executor()
            .spawn(async { unsafe { PostQuitMessage(0) } })
            .detach();
    }

    fn restart(&self, binary_path: Option<PathBuf>) {
        let pid = std::process::id();
        let Some(app_path) = binary_path.or(self.app_path().log_err()) else {
            return;
        };
        let script = format!(
            r#"
            $pidToWaitFor = {}
            $exePath = "{}"

            while ($true) {{
                $process = Get-Process -Id $pidToWaitFor -ErrorAction SilentlyContinue
                if (-not $process) {{
                    Start-Process -FilePath $exePath
                    break
                }}
                Start-Sleep -Seconds 0.1
            }}
            "#,
            pid,
            app_path.display(),
        );

        // 延迟生成到前台执行器，以便在当前 `AppCell` 借用释放后运行。
        // 在 Windows 上，`Command::spawn()` 可以泵送 Win32 消息循环（通过 `CreateProcessW`），
        // 这会重新进入消息处理，可能导致另一个可变借用 `AppCell` 导致双重借用 panic
        self.foreground_executor
            .spawn(async move {
                #[allow(
                    clippy::disallowed_methods,
                    reason = "We are restarting ourselves, using std command thus is fine"
                )]
                let restart_process = ::rgpui::util::command::new_std_command(
                    ::rgpui::util::shell::get_windows_system_shell(),
                )
                .arg("-command")
                .arg(script)
                .spawn();

                match restart_process {
                    Ok(_) => unsafe { PostQuitMessage(0) },
                    Err(e) => log::error!("failed to spawn restart script: {:?}", e),
                }
            })
            .detach();
    }

    fn activate(&self, _ignoring_other_apps: bool) {}

    fn hide(&self) {}

    // todo(windows)
    fn hide_other_apps(&self) {
        unimplemented!()
    }

    // todo(windows)
    fn unhide_other_apps(&self) {
        unimplemented!()
    }

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        WindowsDisplay::displays()
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        WindowsDisplay::primary_monitor().map(|display| Rc::new(display) as Rc<dyn PlatformDisplay>)
    }

    #[cfg(feature = "screen-capture")]
    fn is_screen_capture_supported(&self) -> bool {
        true
    }

    #[cfg(feature = "screen-capture")]
    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
        rgpui::scap_screen_capture::scap_screen_sources(&self.foreground_executor)
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        let active_window_hwnd = unsafe { GetActiveWindow() };
        self.window_from_hwnd(active_window_hwnd)
            .map(|inner| inner.handle)
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        let window = WindowsWindow::new(handle, options, self.generate_creation_info())?;
        let handle = window.get_raw_handle();
        self.raw_window_handles.write().push(handle.into());

        Ok(Box::new(window))
    }

    fn window_appearance(&self) -> WindowAppearance {
        system_appearance().log_err().unwrap_or_default()
    }

    fn open_url(&self, url: &str) {
        if url.is_empty() {
            return;
        }
        let url_string = url.to_string();
        self.background_executor()
            .spawn(async move {
                open_target(&url_string)
                    .with_context(|| format!("Opening url: {}", url_string))
                    .log_err();
            })
            .detach();
    }

    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        self.inner.state.callbacks.open_urls.set(Some(callback));
    }

    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> Receiver<Result<Option<Vec<PathBuf>>>> {
        let (tx, rx) = oneshot::channel();
        let window = self.find_current_active_window();
        self.foreground_executor()
            .spawn(async move {
                let _ = tx.send(file_open_dialog(options, window));
            })
            .detach();

        rx
    }

    fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> Receiver<Result<Option<PathBuf>>> {
        let directory = directory.to_owned();
        let suggested_name = suggested_name.map(|s| s.to_owned());
        let (tx, rx) = oneshot::channel();
        let window = self.find_current_active_window();
        self.foreground_executor()
            .spawn(async move {
                let _ = tx.send(file_save_dialog(directory, suggested_name, window));
            })
            .detach();

        rx
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        // FOS_PICKFOLDERS 标志在"仅文件"和"仅文件夹"之间切换
        false
    }

    fn reveal_path(&self, path: &Path) {
        if path.as_os_str().is_empty() {
            return;
        }
        let path = path.to_path_buf();
        self.background_executor()
            .spawn(async move {
                open_target_in_explorer(&path)
                    .with_context(|| format!("Revealing path {} in explorer", path.display()))
                    .log_err();
            })
            .detach();
    }

    fn open_with_system(&self, path: &Path) {
        if path.as_os_str().is_empty() {
            return;
        }
        let path = path.to_path_buf();
        self.background_executor()
            .spawn(async move {
                open_target(&path)
                    .with_context(|| format!("Opening {} with system", path.display()))
                    .log_err();
            })
            .detach();
    }

    fn on_quit(&self, callback: Box<dyn FnMut()>) {
        self.inner.state.callbacks.quit.set(Some(callback));
    }

    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        self.inner.state.callbacks.reopen.set(Some(callback));
    }

    fn set_menus(&self, menus: Vec<Menu>, _keymap: &Keymap) {
        *self.inner.state.menus.borrow_mut() = menus.into_iter().map(|menu| menu.owned()).collect();
    }

    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        Some(self.inner.state.menus.borrow().clone())
    }

    fn set_dock_menu(&self, menus: Vec<MenuItem>, _keymap: &Keymap) {
        self.set_dock_menus(menus);
    }

    /// 设置系统托盘图标和菜单（旧 API，向后兼容）
    fn set_tray(&self, tray: Tray, menus: Option<Vec<MenuItem>>, _keymap: &Keymap) {
        // 旧 API 兼容：将 MenuItem 转换为 TrayMenuItem
        let tray_menu_items = menus
            .as_ref()
            .map(|menus| convert_menu_items_to_tray(menus))
            .unwrap_or_default();

        let mut windows_tray = self.inner.state.tray.borrow_mut();
        if let Some(windows_tray) = windows_tray.as_mut() {
            windows_tray.menu_items = tray_menu_items;
            if let Some(icon_data) = &tray.icon_data {
                windows_tray.set_icon(Some(&icon_data.data), self.handle);
            }
            if let Some(tooltip) = &tray.tooltip {
                windows_tray.set_tooltip(tooltip, self.handle);
            }
        } else {
            let mut new_tray = WindowsTray::new(self.handle);
            new_tray.menu_items = tray_menu_items;
            if let Some(icon_data) = &tray.icon_data {
                new_tray.set_icon(Some(&icon_data.data), self.handle);
            }
            if let Some(tooltip) = &tray.tooltip {
                new_tray.set_tooltip(tooltip, self.handle);
            }
            windows_tray.replace(new_tray);
        }
    }

    /// 设置系统托盘图标
    fn set_tray_icon(&self, icon: Option<&[u8]>) {
        let mut windows_tray = self.inner.state.tray.borrow_mut();
        if windows_tray.is_none() {
            windows_tray.replace(WindowsTray::new(self.handle));
        }
        if let Some(windows_tray) = windows_tray.as_mut() {
            windows_tray.set_icon(icon, self.handle);
        }
    }

    /// 设置系统托盘菜单项
    fn set_tray_menu(&self, menu: Vec<TrayMenuItem>) {
        let mut windows_tray = self.inner.state.tray.borrow_mut();
        if windows_tray.is_none() {
            windows_tray.replace(WindowsTray::new(self.handle));
        }
        if let Some(windows_tray) = windows_tray.as_mut() {
            windows_tray.menu_items = menu;
        }
    }

    /// 设置系统托盘工具提示文本
    fn set_tray_tooltip(&self, tooltip: &str) {
        let mut windows_tray = self.inner.state.tray.borrow_mut();
        if windows_tray.is_none() {
            windows_tray.replace(WindowsTray::new(self.handle));
        }
        if let Some(windows_tray) = windows_tray.as_mut() {
            windows_tray.set_tooltip(tooltip, self.handle);
        }
    }

    /// 启用或禁用托盘面板模式（Windows 不支持）
    fn set_tray_panel_mode(&self, _enabled: bool) {
        // Windows 不支持面板模式，此方法为空
    }

    /// 获取托盘图标的屏幕边界坐标
    fn get_tray_icon_bounds(&self) -> Option<Bounds<Pixels>> {
        // Windows 托盘图标位置由系统管理，难以获取精确位置
        // 返回 None 表示不可用
        None
    }

    /// 注册托盘图标事件回调
    fn on_tray_icon_event(&self, callback: Box<dyn FnMut(TrayIconEvent)>) {
        *self.inner.state.tray_icon_event_callback.borrow_mut() = Some(callback);
    }

    /// 注册托盘菜单项点击事件回调
    fn on_tray_menu_action(&self, callback: Box<dyn FnMut(SharedString)>) {
        *self.inner.state.tray_menu_action_callback.borrow_mut() = Some(callback);
    }

    fn set_keep_alive_without_windows(&self, keep_alive: bool) {
        self.inner
            .state
            .keep_alive_without_windows
            .store(keep_alive, Ordering::Release);
    }

    /// 注册全局快捷键
    fn register_global_hotkey(&self, id: u32, keystroke: &Keystroke) -> Result<()> {
        let mut hotkey = self.inner.state.global_hotkey.borrow_mut();
        hotkey.register(self.handle, id as i32, keystroke)
    }

    /// 注销全局快捷键
    fn unregister_global_hotkey(&self, id: u32) {
        let mut hotkey = self.inner.state.global_hotkey.borrow_mut();
        hotkey.unregister(id as i32);
    }

    /// 注册全局快捷键事件回调
    fn on_global_hotkey(&self, callback: Box<dyn FnMut(u32)>) {
        *self.inner.state.global_hotkey_callback.borrow_mut() = Some(callback);
    }

    /// 显示系统原生通知
    fn show_notification(&self, title: &str, body: &str) -> Result<()> {
        show_balloon_notification(self.handle, title, body)
    }

    /// 设置开机自启动
    fn set_auto_launch(&self, app_id: &str, enabled: bool) -> Result<()> {
        crate::set_auto_launch(app_id, enabled)
    }

    /// 检查开机自启动是否已启用
    fn is_auto_launch_enabled(&self, app_id: &str) -> bool {
        crate::is_auto_launch_enabled(app_id)
    }

    /// 获取当前系统聚焦窗口信息
    fn focused_window_info(&self) -> Option<FocusedWindowInfo> {
        crate::get_focused_window_info()
    }

    /// 获取辅助功能权限状态（Windows 默认授予）
    fn accessibility_status(&self) -> PermissionStatus {
        PermissionStatus::Granted
    }

    /// 请求辅助功能权限（Windows 无需请求）
    fn request_accessibility_permission(&self) {}

    /// 获取麦克风权限状态（Windows 默认授予）
    fn microphone_status(&self) -> PermissionStatus {
        PermissionStatus::Granted
    }

    /// 请求麦克风权限（Windows 无需请求）
    fn request_microphone_permission(&self, callback: Box<dyn FnOnce(bool)>) {
        callback(true);
    }

    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>) {
        self.inner
            .state
            .callbacks
            .app_menu_action
            .set(Some(callback));
    }

    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>) {
        self.inner
            .state
            .callbacks
            .will_open_app_menu
            .set(Some(callback));
    }

    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        self.inner
            .state
            .callbacks
            .validate_app_menu_command
            .set(Some(callback));
    }

    fn app_path(&self) -> Result<PathBuf> {
        Ok(std::env::current_exe()?)
    }

    // todo(windows)
    fn path_for_auxiliary_executable(&self, _name: &str) -> Result<PathBuf> {
        anyhow::bail!("not yet implemented");
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        let hcursor = load_cursor(style);
        if self.inner.state.current_cursor.get().map(|c| c.0) != hcursor.map(|c| c.0) {
            self.post_message(
                WM_GPUI_CURSOR_STYLE_CHANGED,
                WPARAM(0),
                LPARAM(hcursor.map_or(0, |c| c.0 as isize)),
            );
            self.inner.state.current_cursor.set(hcursor);
        }
    }

    fn hide_cursor_until_mouse_moves(&self) {
        if !self
            .inner
            .state
            .cursor_visible
            .swap(false, Ordering::Relaxed)
        {
            return;
        }

        for handle in self.raw_window_handles.read().iter() {
            let Some(window) = window_from_hwnd(handle.as_raw()) else {
                continue;
            };
            if window.state.hovered.get() {
                unsafe { SetCursor(None) };
                break;
            }
        }
    }

    fn is_cursor_visible(&self) -> bool {
        self.inner.state.cursor_visible.load(Ordering::Relaxed)
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        should_auto_hide_scrollbars().log_err().unwrap_or(false)
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        write_to_clipboard(item);
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        read_from_clipboard()
    }

    fn write_credentials(&self, url: &str, username: &str, password: &[u8]) -> Task<Result<()>> {
        let password = password.to_vec();
        let mut username = username.encode_utf16().chain(Some(0)).collect_vec();
        let mut target_name = windows_credentials_target_name(url)
            .encode_utf16()
            .chain(Some(0))
            .collect_vec();
        self.foreground_executor().spawn(async move {
            let credentials = CREDENTIALW {
                LastWritten: unsafe { GetSystemTimeAsFileTime() },
                Flags: CRED_FLAGS(0),
                Type: CRED_TYPE_GENERIC,
                TargetName: PWSTR::from_raw(target_name.as_mut_ptr()),
                CredentialBlobSize: password.len() as u32,
                CredentialBlob: password.as_ptr() as *mut _,
                Persist: CRED_PERSIST_LOCAL_MACHINE,
                UserName: PWSTR::from_raw(username.as_mut_ptr()),
                ..CREDENTIALW::default()
            };
            unsafe {
                CredWriteW(&credentials, 0).map_err(|err| {
                    anyhow!(
                        "Failed to write credentials to Windows Credential Manager: {}",
                        err,
                    )
                })?;
            }
            Ok(())
        })
    }

    fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        let target_name = windows_credentials_target_name(url)
            .encode_utf16()
            .chain(Some(0))
            .collect_vec();
        self.foreground_executor().spawn(async move {
            let mut credentials: *mut CREDENTIALW = std::ptr::null_mut();
            let result = unsafe {
                CredReadW(
                    PCWSTR::from_raw(target_name.as_ptr()),
                    CRED_TYPE_GENERIC,
                    None,
                    &mut credentials,
                )
            };

            if let Err(err) = result {
                // ERROR_NOT_FOUND means the credential doesn't exist.
                // Return Ok(None) to match macOS and Linux behavior.
                if err.code() == ERROR_NOT_FOUND.to_hresult() {
                    return Ok(None);
                }
                return Err(err.into());
            }

            if credentials.is_null() {
                Ok(None)
            } else {
                let username: String = unsafe { (*credentials).UserName.to_string()? };
                let credential_blob = unsafe {
                    std::slice::from_raw_parts(
                        (*credentials).CredentialBlob,
                        (*credentials).CredentialBlobSize as usize,
                    )
                };
                let password = credential_blob.to_vec();
                unsafe { CredFree(credentials as *const _ as _) };
                Ok(Some((username, password)))
            }
        })
    }

    fn delete_credentials(&self, url: &str) -> Task<Result<()>> {
        let target_name = windows_credentials_target_name(url)
            .encode_utf16()
            .chain(Some(0))
            .collect_vec();
        self.foreground_executor().spawn(async move {
            unsafe {
                CredDeleteW(
                    PCWSTR::from_raw(target_name.as_ptr()),
                    CRED_TYPE_GENERIC,
                    None,
                )?
            };
            Ok(())
        })
    }

    fn register_url_scheme(&self, _: &str) -> Task<anyhow::Result<()>> {
        Task::ready(Err(anyhow!("register_url_scheme unimplemented")))
    }

    fn perform_dock_menu_action(&self, action: usize) {
        unsafe {
            PostMessageW(
                Some(self.handle),
                WM_GPUI_DOCK_MENU_ACTION,
                WPARAM(self.inner.validation_number),
                LPARAM(action as isize),
            )
            .log_err();
        }
    }

    fn update_jump_list(
        &self,
        menus: Vec<MenuItem>,
        entries: Vec<SmallVec<[PathBuf; 2]>>,
    ) -> Task<Vec<SmallVec<[PathBuf; 2]>>> {
        self.update_jump_list(menus, entries)
    }
}

impl WindowsPlatformInner {
    fn new(context: &mut PlatformWindowCreateContext) -> Result<Rc<Self>> {
        let state = WindowsPlatformState::new(context.directx_devices.take());
        Ok(Rc::new(Self {
            state,
            raw_window_handles: context.raw_window_handles.clone(),
            dispatcher: context
                .dispatcher
                .as_ref()
                .context("missing dispatcher")?
                .clone(),
            validation_number: context.validation_number,
            main_receiver: context
                .main_receiver
                .take()
                .context("missing main receiver")?,
        }))
    }

    /// 调用 `project` 投影到对应的回调字段，从回调中移除它，
    /// 用回调调用 `f`，然后将回调放回
    fn with_callback<T>(
        &self,
        project: impl Fn(&PlatformCallbacks) -> &Cell<Option<T>>,
        f: impl FnOnce(&mut T),
    ) {
        let callback = project(&self.state.callbacks).take();
        if let Some(mut callback) = callback {
            f(&mut callback);
            project(&self.state.callbacks).set(Some(callback));
        }
    }

    fn handle_msg(
        self: &Rc<Self>,
        handle: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let handled = match msg {
            WM_GPUI_CLOSE_ONE_WINDOW
            | WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD
            | WM_GPUI_DOCK_MENU_ACTION
            | WM_GPUI_KEYBOARD_LAYOUT_CHANGED
            | WM_GPUI_GPU_DEVICE_LOST => self.handle_gpui_events(msg, wparam, lparam),
            WM_GPUI_TRAY_ICON => self.handle_tray_icon_event(handle, lparam),
            WM_COMMAND => self.handle_tray_menu_command(wparam),
            WM_HOTKEY => self.handle_global_hotkey(wparam),
            _ => None,
        };
        if let Some(result) = handled {
            LRESULT(result)
        } else {
            unsafe { DefWindowProcW(handle, msg, wparam, lparam) }
        }
    }

    fn handle_gpui_events(&self, message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        if wparam.0 != self.validation_number {
            log::error!("Wrong validation number while processing message: {message}");
            return None;
        }
        match message {
            WM_GPUI_CLOSE_ONE_WINDOW => {
                self.close_one_window(HWND(lparam.0 as _));
                Some(0)
            }
            WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD => self.run_foreground_task(),
            WM_GPUI_DOCK_MENU_ACTION => self.handle_dock_action_event(lparam.0 as _),
            WM_GPUI_KEYBOARD_LAYOUT_CHANGED => self.handle_keyboard_layout_change(),
            WM_GPUI_GPU_DEVICE_LOST => self.handle_device_lost(lparam),
            _ => unreachable!(),
        }
    }

    fn close_one_window(&self, target_window: HWND) -> bool {
        let Some(all_windows) = self.raw_window_handles.upgrade() else {
            log::error!("Failed to upgrade raw window handles");
            return false;
        };
        let mut lock = all_windows.write();
        let index = lock
            .iter()
            .position(|handle| handle.as_raw() == target_window)
            .unwrap();
        lock.remove(index);

        lock.is_empty()
    }

    #[inline]
    fn run_foreground_task(&self) -> Option<isize> {
        const MAIN_TASK_TIMEOUT: u128 = 10;

        let start = std::time::Instant::now();
        'tasks: loop {
            'timeout_loop: loop {
                if start.elapsed().as_millis() >= MAIN_TASK_TIMEOUT {
                    log::debug!("foreground task timeout reached");
                    // 我们花费了预算在 gpui 任务上，我们可能有很多工作排队，所以先清理系统事件以保持响应
                    // 然后退出前台工作，允许我们在返回前台任务工作之前先处理其他 gpui 事件
                    // 如果不这样做，我们可能例如无法处理窗口退出事件
                    let mut msg = MSG::default();
                    let process_message = |msg: &_| {
                        if translate_accelerator(msg).is_none() {
                            _ = unsafe { TranslateMessage(msg) };
                            unsafe { DispatchMessageW(msg) };
                        }
                    };
                    let peek_msg = |msg: &mut _, msg_kind| unsafe {
                        PeekMessageW(msg, None, 0, 0, PM_REMOVE | msg_kind).as_bool()
                    };
                    // We need to process a paint message here as otherwise we will re-enter `run_foreground_task` before painting if we have work remaining.
                    // The reason for this is that windows prefers custom application message processing over system messages.
                    if peek_msg(&mut msg, PM_QS_PAINT) {
                        process_message(&msg);
                    }
                    while peek_msg(&mut msg, PM_QS_INPUT) {
                        process_message(&msg);
                    }
                    // Allow the main loop to process other gpui events before going back into `run_foreground_task`
                    unsafe {
                        if let Err(_) = PostMessageW(
                            Some(self.dispatcher.platform_window_handle.as_raw()),
                            WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD,
                            WPARAM(self.validation_number),
                            LPARAM(0),
                        ) {
                            self.dispatcher.wake_posted.store(false, Ordering::Release);
                        };
                    }
                    break 'tasks;
                }
                let mut main_receiver = self.main_receiver.clone();
                match main_receiver.try_pop() {
                    Ok(Some(runnable)) => WindowsDispatcher::execute_runnable(runnable),
                    _ => break 'timeout_loop,
                }
            }

            // Someone could enqueue a Runnable here. The flag is still true, so they will not PostMessage.
            // We need to check for those Runnables after we clear the flag.
            self.dispatcher.wake_posted.store(false, Ordering::Release);
            let mut main_receiver = self.main_receiver.clone();
            match main_receiver.try_pop() {
                Ok(Some(runnable)) => {
                    self.dispatcher.wake_posted.store(true, Ordering::Release);

                    WindowsDispatcher::execute_runnable(runnable);
                }
                _ => break 'tasks,
            }
        }

        Some(0)
    }

    fn handle_dock_action_event(&self, action_idx: usize) -> Option<isize> {
        let Some(action) = self
            .state
            .jump_list
            .borrow()
            .dock_menus
            .get(action_idx)
            .map(|dock_menu| dock_menu.action.boxed_clone())
        else {
            log::error!("Dock menu for index {action_idx} not found");
            return Some(1);
        };
        self.with_callback(
            |callbacks| &callbacks.app_menu_action,
            |callback| callback(&*action),
        );
        Some(0)
    }

    fn handle_keyboard_layout_change(&self) -> Option<isize> {
        self.with_callback(
            |callbacks| &callbacks.keyboard_layout_change,
            |callback| callback(),
        );
        Some(0)
    }

    fn handle_device_lost(&self, lparam: LPARAM) -> Option<isize> {
        let directx_devices = lparam.0 as *const DirectXDevices;
        let directx_devices = unsafe { &*directx_devices };
        self.state.directx_devices.borrow_mut().take();
        *self.state.directx_devices.borrow_mut() = Some(directx_devices.clone());

        Some(0)
    }

    /// 处理托盘图标消息
    /// 根据 lParam 中的消息类型触发相应事件
    /// 参数:
    ///   handle - 窗口句柄
    ///   lparam - 包含托盘消息类型的 LPARAM
    /// 返回: 消息处理结果
    fn handle_tray_icon_event(&self, handle: HWND, lparam: LPARAM) -> Option<isize> {
        let msg = (lparam.0 & 0xFFFF) as u32;
        let event = match msg {
            WM_LBUTTONUP => Some(TrayIconEvent::LeftClick),
            WM_RBUTTONUP => Some(TrayIconEvent::RightClick),
            WM_LBUTTONDBLCLK => Some(TrayIconEvent::DoubleClick),
            _ => None,
        };
        if let Some(event) = event {
            // 右键点击时显示上下文菜单
            if event == TrayIconEvent::RightClick {
                let mut tray = self.state.tray.borrow_mut();
                if let Some(ref mut t) = *tray {
                    t.show_context_menu(handle);
                }
            }
            // 触发托盘事件回调
            if let Some(callback) = self.state.tray_icon_event_callback.borrow_mut().as_mut() {
                callback(event);
            }
        }
        Some(0)
    }

    /// 处理托盘菜单命令
    /// 当用户点击菜单项时，通过命令 ID 查找对应的菜单项 ID 并触发回调
    /// 参数:
    ///   wparam - 包含命令 ID 的 WPARAM
    /// 返回: 消息处理结果
    fn handle_tray_menu_command(&self, wparam: WPARAM) -> Option<isize> {
        let cmd_id = (wparam.0 & 0xFFFF) as u32;
        let item_id = {
            let tray = self.state.tray.borrow();
            tray.as_ref()
                .and_then(|tray| tray.command_id_map.get(&cmd_id).cloned())
        };
        let Some(item_id) = item_id else {
            return None;
        };
        if let Some(callback) = self.state.tray_menu_action_callback.borrow_mut().as_mut() {
            callback(item_id);
        }
        Some(0)
    }

    fn handle_tray_menu_action_event(&self, menu_id: muda::MenuId) -> Option<isize> {
        let Some(action) = self
            .state
            .tray_menu_actions
            .borrow()
            .get(&menu_id)
            .map(|action| action.boxed_clone())
        else {
            log::error!("Tray menu_id: {:?} not found", menu_id);
            return Some(1);
        };
        self.with_callback(
            |callbacks| &callbacks.app_menu_action,
            |callback| callback(&*action),
        );

        Some(0)
    }

    /// 处理全局快捷键消息
    /// 当用户按下注册的全局快捷键时触发
    /// 参数:
    ///   wparam - 包含快捷键 ID 的 WPARAM
    /// 返回: 消息处理结果
    fn handle_global_hotkey(&self, wparam: WPARAM) -> Option<isize> {
        let hotkey_id = wparam.0 as u32;
        if let Some(callback) = self.state.global_hotkey_callback.borrow_mut().as_mut() {
            callback(hotkey_id);
        }
        Some(0)
    }
}

impl Drop for WindowsPlatform {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.handle)
                .context("Destroying platform window")
                .log_err();
            OleUninitialize();
        }
    }
}

/// 窗口创建时需要的配置信息
///
/// 包含创建新窗口所需的所有共享资源和配置
pub(crate) struct WindowCreationInfo {
    pub(crate) icon: HICON,
    pub(crate) executor: ForegroundExecutor,
    pub(crate) current_cursor: Option<HCURSOR>,
    pub(crate) cursor_visible: Arc<AtomicBool>,
    pub(crate) drop_target_helper: IDropTargetHelper,
    pub(crate) validation_number: usize,
    pub(crate) main_receiver: PriorityQueueReceiver<RunnableVariant>,
    pub(crate) platform_window_handle: HWND,
    pub(crate) disable_direct_composition: bool,
    pub(crate) directx_devices: DirectXDevices,
    /// Flag to instruct the `VSyncProvider` thread to invalidate the directx devices
    /// as resizing them has failed, causing us to have lost at least the render target.
    pub(crate) invalidate_devices: Arc<AtomicBool>,
}

struct PlatformWindowCreateContext {
    inner: Option<Result<Rc<WindowsPlatformInner>>>,
    raw_window_handles: std::sync::Weak<RwLock<SmallVec<[SafeHwnd; 4]>>>,
    validation_number: usize,
    main_sender: Option<PriorityQueueSender<RunnableVariant>>,
    main_receiver: Option<PriorityQueueReceiver<RunnableVariant>>,
    directx_devices: Option<DirectXDevices>,
    dispatcher: Option<Arc<WindowsDispatcher>>,
}

fn open_target(target: impl AsRef<OsStr>) -> Result<()> {
    let target = target.as_ref();
    let ret = unsafe {
        ShellExecuteW(
            None,
            windows::core::w!("open"),
            &HSTRING::from(target),
            None,
            None,
            SW_SHOWDEFAULT,
        )
    };
    if ret.0 as isize <= 32 {
        Err(anyhow::anyhow!(
            "Unable to open target: {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

fn open_target_in_explorer(target: &Path) -> Result<()> {
    let dir = target.parent().context("No parent folder found")?;
    let desktop = unsafe { SHGetDesktopFolder()? };

    let mut dir_item = std::ptr::null_mut();
    unsafe {
        desktop.ParseDisplayName(
            HWND::default(),
            None,
            &HSTRING::from(dir),
            None,
            &mut dir_item,
            std::ptr::null_mut(),
        )?;
    }

    let mut file_item = std::ptr::null_mut();
    unsafe {
        desktop.ParseDisplayName(
            HWND::default(),
            None,
            &HSTRING::from(target),
            None,
            &mut file_item,
            std::ptr::null_mut(),
        )?;
    }

    let highlight = [file_item as *const _];
    unsafe { SHOpenFolderAndSelectItems(dir_item as _, Some(&highlight), 0) }.or_else(|err| {
        if err.code().0 == ERROR_FILE_NOT_FOUND.0 as i32 {
            // On some systems, the above call mysteriously fails with "file not
            // found" even though the file is there.  In these cases, ShellExecute()
            // seems to work as a fallback (although it won't select the file).
            open_target(dir).context("Opening target parent folder")
        } else {
            Err(anyhow::anyhow!("Can not open target path: {}", err))
        }
    })
}

fn file_open_dialog(
    options: PathPromptOptions,
    window: Option<HWND>,
) -> Result<Option<Vec<PathBuf>>> {
    let folder_dialog: IFileOpenDialog =
        unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL)? };

    let mut dialog_options = FOS_FILEMUSTEXIST;
    if options.multiple {
        dialog_options |= FOS_ALLOWMULTISELECT;
    }
    if options.directories {
        dialog_options |= FOS_PICKFOLDERS;
    }

    unsafe {
        folder_dialog.SetOptions(dialog_options)?;

        if let Some(prompt) = options.prompt {
            let prompt: &str = &prompt;
            folder_dialog.SetOkButtonLabel(&HSTRING::from(prompt))?;
        }

        if folder_dialog.Show(window).is_err() {
            // User cancelled
            return Ok(None);
        }
    }

    let results = unsafe { folder_dialog.GetResults()? };
    let file_count = unsafe { results.GetCount()? };
    if file_count == 0 {
        return Ok(None);
    }

    let mut paths = Vec::with_capacity(file_count as usize);
    for i in 0..file_count {
        let item = unsafe { results.GetItemAt(i)? };
        let path = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH)?.to_string()? };
        paths.push(PathBuf::from(path));
    }

    Ok(Some(paths))
}

fn file_save_dialog(
    directory: PathBuf,
    suggested_name: Option<String>,
    window: Option<HWND>,
) -> Result<Option<PathBuf>> {
    let dialog: IFileSaveDialog = unsafe { CoCreateInstance(&FileSaveDialog, None, CLSCTX_ALL)? };
    if !directory.to_string_lossy().is_empty()
        && let Some(full_path) = directory
            .canonicalize()
            .context("failed to canonicalize directory")
            .log_err()
    {
        let full_path = dunce::simplified(&full_path);
        let full_path_string = full_path.display().to_string();
        let path_item: IShellItem =
            unsafe { SHCreateItemFromParsingName(&HSTRING::from(full_path_string), None)? };
        unsafe {
            dialog
                .SetFolder(&path_item)
                .context("failed to set dialog folder")
                .log_err()
        };
    }

    if let Some(suggested_name) = suggested_name {
        unsafe {
            dialog
                .SetFileName(&HSTRING::from(suggested_name))
                .context("failed to set file name")
                .log_err()
        };
    }

    unsafe {
        dialog.SetFileTypes(&[Common::COMDLG_FILTERSPEC {
            pszName: windows::core::w!("All files"),
            pszSpec: windows::core::w!("*.*"),
        }])?;
        if dialog.Show(window).is_err() {
            // User cancelled
            return Ok(None);
        }
    }
    let shell_item = unsafe { dialog.GetResult()? };
    let file_path_string = unsafe {
        let pwstr = shell_item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let string = pwstr.to_string()?;
        CoTaskMemFree(Some(pwstr.0 as _));
        string
    };
    Ok(Some(PathBuf::from(file_path_string)))
}

fn load_icon() -> Result<HICON> {
    let module = unsafe { GetModuleHandleW(None).context("unable to get module handle")? };
    let handle = unsafe {
        LoadImageW(
            Some(module.into()),
            windows::core::PCWSTR(1 as _),
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTSIZE | LR_SHARED,
        )
        .context("unable to load icon file")?
    };
    Ok(HICON(handle.0))
}

#[inline]
fn should_auto_hide_scrollbars() -> Result<bool> {
    let ui_settings = UISettings::new()?;
    Ok(ui_settings.AutoHideScrollBars()?)
}

fn check_device_lost(device: &ID3D11Device) -> bool {
    let device_state = unsafe { device.GetDeviceRemovedReason() };
    match device_state {
        Ok(_) => false,
        Err(err) => {
            log::error!("DirectX device lost detected: {:?}", err);
            true
        }
    }
}

fn handle_gpu_device_lost(
    directx_devices: &mut DirectXDevices,
    platform_window: HWND,
    validation_number: usize,
    all_windows: &std::sync::Weak<RwLock<SmallVec<[SafeHwnd; 4]>>>,
    text_system: &std::sync::Weak<DirectWriteTextSystem>,
) -> Result<()> {
    // Here we wait a bit to ensure the system has time to recover from the device lost state.
    // If we don't wait, the final drawing result will be blank.
    std::thread::sleep(std::time::Duration::from_millis(350));

    *directx_devices = try_to_recover_from_device_lost(|| {
        DirectXDevices::new().context("Failed to recreate new DirectX devices after device lost")
    })?;
    log::info!("DirectX devices successfully recreated.");

    let lparam = LPARAM(directx_devices as *const _ as _);
    unsafe {
        SendMessageW(
            platform_window,
            WM_GPUI_GPU_DEVICE_LOST,
            Some(WPARAM(validation_number)),
            Some(lparam),
        );
    }

    if let Some(text_system) = text_system.upgrade() {
        text_system.handle_gpu_lost(&directx_devices)?;
    }
    if let Some(all_windows) = all_windows.upgrade() {
        for window in all_windows.read().iter() {
            unsafe {
                SendMessageW(
                    window.as_raw(),
                    WM_GPUI_GPU_DEVICE_LOST,
                    Some(WPARAM(validation_number)),
                    Some(lparam),
                );
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
        for window in all_windows.read().iter() {
            unsafe {
                SendMessageW(
                    window.as_raw(),
                    WM_GPUI_FORCE_UPDATE_WINDOW,
                    Some(WPARAM(validation_number)),
                    None,
                );
            }
        }
    }
    Ok(())
}

const PLATFORM_WINDOW_CLASS_NAME: PCWSTR = w!("Zed::PlatformWindow");

fn register_platform_window_class() {
    let wc = WNDCLASSW {
        lpfnWndProc: Some(window_procedure),
        lpszClassName: PCWSTR(PLATFORM_WINDOW_CLASS_NAME.as_ptr()),
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc) };
}

unsafe extern "system" fn window_procedure(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCCREATE {
        let params = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        let creation_context = params.lpCreateParams as *mut PlatformWindowCreateContext;
        let creation_context = unsafe { &mut *creation_context };

        let Some(main_sender) = creation_context.main_sender.take() else {
            creation_context.inner = Some(Err(anyhow!("missing main sender")));
            return LRESULT(0);
        };
        creation_context.dispatcher = Some(Arc::new(WindowsDispatcher::new(
            main_sender,
            hwnd,
            creation_context.validation_number,
        )));

        return match WindowsPlatformInner::new(creation_context) {
            Ok(inner) => {
                let weak = Box::new(Rc::downgrade(&inner));
                unsafe { set_window_long(hwnd, GWLP_USERDATA, Box::into_raw(weak) as isize) };
                creation_context.inner = Some(Ok(inner));
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            Err(error) => {
                creation_context.inner = Some(Err(error));
                LRESULT(0)
            }
        };
    }

    let ptr = unsafe { get_window_long(hwnd, GWLP_USERDATA) } as *mut Weak<WindowsPlatformInner>;
    if ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let inner = unsafe { &*ptr };
    let result = if let Some(inner) = inner.upgrade() {
        if cfg!(debug_assertions) {
            let inner = std::panic::AssertUnwindSafe(inner);
            match std::panic::catch_unwind(|| { inner }.handle_msg(hwnd, msg, wparam, lparam)) {
                Ok(result) => result,
                Err(_) => std::process::abort(),
            }
        } else {
            inner.handle_msg(hwnd, msg, wparam, lparam)
        }
    } else {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    };

    if msg == WM_NCDESTROY {
        unsafe { set_window_long(hwnd, GWLP_USERDATA, 0) };
        unsafe { drop(Box::from_raw(ptr)) };
    }

    result
}

/// Runs a menu event listener in a separate thread.
/// To coordinate [`muda::MenuEvent::receiver()`] with the main thread, use a channel.
fn run_muda_menu_event_listener(
    foreground_executor: &ForegroundExecutor,
    inner: &Rc<WindowsPlatformInner>,
) {
    let (tx, rx) = smol::channel::unbounded::<muda::MenuEvent>();
    foreground_executor
        .spawn({
            let inner = inner.clone();
            async move {
                while let Ok(event) = rx.recv().await {
                    _ = inner.handle_tray_menu_action_event(event.id);
                }
            }
        })
        .detach();

    std::thread::Builder::new()
        .name("MenuEventReceiver".to_string())
        .spawn(move || {
            while let Ok(event) = muda::MenuEvent::receiver().recv() {
                _ = tx.send_blocking(event);
            }
        })
        .unwrap();
}

/// Builds a [`muda::Menu`] from a `Vec<MenuItem>`.
#[allow(dead_code)]
fn build_muda_menu(
    items: &Vec<MenuItem>,
    action_map: &mut std::collections::HashMap<muda::MenuId, Box<dyn Action>>,
) -> muda::Menu {
    use muda::{CheckMenuItem, MenuItemKind, PredefinedMenuItem, Submenu};

    let menu = muda::Menu::new();
    for item in items {
        match item {
            MenuItem::Separator => {
                let _ = menu.append(&PredefinedMenuItem::separator());
            }
            MenuItem::Action {
                name,
                checked,
                action,
                ..
            } => {
                if *checked {
                    let muda_item = CheckMenuItem::new(name, true, *checked, None);
                    action_map.insert(muda_item.id().clone(), action.boxed_clone());
                    let _ = menu.append(&muda_item);
                } else {
                    let muda_item = muda::MenuItem::new(name, true, None);
                    action_map.insert(muda_item.id().clone(), action.boxed_clone());
                    let _ = menu.append(&muda_item);
                }
            }
            MenuItem::Submenu(_submenu) => {
                let child_menu = build_muda_menu(&_submenu.items, action_map);
                let submenu = Submenu::new(&_submenu.name, true);
                for item in child_menu.items() {
                    match item {
                        MenuItemKind::Check(item) => {
                            let _ = submenu.append(&item);
                        }
                        MenuItemKind::Icon(item) => {
                            let _ = submenu.append(&item);
                        }
                        MenuItemKind::Predefined(item) => {
                            let _ = submenu.append(&item);
                        }
                        MenuItemKind::Submenu(item) => {
                            let _ = submenu.append(&item);
                        }
                        MenuItemKind::MenuItem(item) => {
                            let _ = submenu.append(&item);
                        }
                    }
                }
                let _ = menu.append(&submenu);
            }
            _ => {}
        };
    }
    menu
}

/// 将 MenuItem 转换为 TrayMenuItem（旧 API 兼容）
fn convert_menu_items_to_tray(items: &[MenuItem]) -> Vec<TrayMenuItem> {
    items
        .iter()
        .filter_map(|item| match item {
            MenuItem::Separator => Some(TrayMenuItem::Separator),
            MenuItem::Action { name, .. } => Some(TrayMenuItem::Action {
                label: name.clone(),
                id: name.clone(),
            }),
            MenuItem::Submenu(menu) => Some(TrayMenuItem::Submenu {
                label: menu.name.clone(),
                items: convert_menu_items_to_tray(&menu.items),
            }),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::{read_from_clipboard, write_to_clipboard};
    use rgpui::ClipboardItem;

    #[test]
    fn test_clipboard() {
        let item = ClipboardItem::new_string("你好，我是张小白".to_string());
        write_to_clipboard(item.clone());
        assert_eq!(read_from_clipboard(), Some(item));

        let item = ClipboardItem::new_string("12345".to_string());
        write_to_clipboard(item.clone());
        assert_eq!(read_from_clipboard(), Some(item));

        let item = ClipboardItem::new_string_with_json_metadata("abcdef".to_string(), vec![3, 4]);
        write_to_clipboard(item.clone());
        assert_eq!(read_from_clipboard(), Some(item));
    }
}
