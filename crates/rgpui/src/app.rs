//! 应用核心 —— 提供 App 上下文、实体生命周期管理及事件调度。

use crate::scheduler::Instant;
use std::{
    any::{TypeId, type_name},
    cell::{BorrowMutError, Cell, Ref, RefCell, RefMut},
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{Arc, atomic::Ordering::SeqCst},
    time::Duration,
};

use anyhow::{Context as _, Result, anyhow};
use derive_more::{Deref, DerefMut};
use futures::{
    Future, FutureExt,
    channel::oneshot,
    future::{LocalBoxFuture, Shared},
};
use itertools::Itertools;
use parking_lot::RwLock;
use slotmap::SlotMap;

use crate::collections::{FxHashMap, FxHashSet, HashMap, TypeIdHashMap, TypeIdHashSet, VecDeque};
use crate::debug_panic;
use crate::http_client::{HttpClient, Url};
use crate::rgpui_util::ResultExt;
pub use async_context::*;
#[cfg(feature = "bench")]
pub use bench_context::{BenchAppContext, BenchReport, BenchWindowContext, bench_platform};
pub use context::*;
pub use entity_map::*;
#[cfg(any(test, feature = "test-support"))]
pub use headless_app_context::*;
use smallvec::SmallVec;
#[cfg(any(test, feature = "test-support"))]
pub use test_app::*;
#[cfg(any(test, feature = "test-support"))]
pub use test_context::*;
#[cfg(all(target_os = "macos", any(test, feature = "test-support")))]
pub use visual_test_context::*;

#[cfg(any(feature = "inspector", debug_assertions))]
use crate::InspectorElementRegistry;
use crate::{
    Action, ActionBuildError, ActionRegistry, Any, AnyView, AnyWindowHandle, AppContext, Arena,
    ArenaBox, Asset, AssetSource, BackgroundExecutor, Bounds, ClipboardItem, CursorStyle,
    DispatchPhase, DisplayId, EventEmitter, FocusHandle, FocusMap, ForegroundExecutor, Global,
    KeyBinding, KeyContext, Keymap, Keystroke, LayoutId, Menu, MenuItem, OwnedMenu,
    PathPromptOptions, Pixels, Platform, PlatformDisplay, PlatformKeyboardLayout,
    PlatformKeyboardMapper, Point, Priority, PromptBuilder, PromptButton, PromptHandle,
    PromptLevel, Render, RenderImage, RenderablePromptHandle, Reservation, ScreenCaptureSource,
    SharedString, SubscriberSet, Subscription, SvgRenderer, Task, TextRenderingMode, TextSystem,
    ThermalState, Tray, TrayIconEvent, TrayMenuItem, Window, WindowAppearance, WindowButtonLayout,
    WindowHandle, WindowId, WindowInvalidator,
    colors::{Colors, GlobalColors},
    hash, init_app_menus,
    root::Root,
};

mod async_context;
#[cfg(feature = "bench")]
mod bench_context;
mod context;
mod entity_map;
#[cfg(any(test, feature = "test-support"))]
mod headless_app_context;
#[cfg(any(test, feature = "test-support"))]
mod test_app;
#[cfg(any(test, feature = "test-support"))]
mod test_context;
#[cfg(all(target_os = "macos", any(test, feature = "test-support")))]
mod visual_test_context;

/// 应用完全退出前，[Context::on_app_quit] 返回的 future 可运行的最大时长。
pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(200);

/// [`RefCell<App>`] 的临时封装，用于调试双重借用问题。
/// 稳定后强烈建议移除。
#[doc(hidden)]
pub struct AppCell {
    app: RefCell<App>,
}

impl AppCell {
    #[doc(hidden)]
    #[track_caller]
    pub fn borrow(&self) -> AppRef<'_> {
        if option_env!("TRACK_THREAD_BORROWS").is_some() {
            let thread_id = std::thread::current().id();
            eprintln!("borrowed {thread_id:?}");
        }
        AppRef(self.app.borrow())
    }

    #[doc(hidden)]
    #[track_caller]
    pub fn borrow_mut(&self) -> AppRefMut<'_> {
        if option_env!("TRACK_THREAD_BORROWS").is_some() {
            let thread_id = std::thread::current().id();
            eprintln!("borrowed {thread_id:?}");
        }
        AppRefMut(self.app.borrow_mut())
    }

    #[doc(hidden)]
    #[track_caller]
    pub fn try_borrow_mut(&self) -> Result<AppRefMut<'_>, BorrowMutError> {
        if option_env!("TRACK_THREAD_BORROWS").is_some() {
            let thread_id = std::thread::current().id();
            eprintln!("borrowed {thread_id:?}");
        }
        Ok(AppRefMut(self.app.try_borrow_mut()?))
    }
}

#[doc(hidden)]
#[derive(Deref, DerefMut)]
pub struct AppRef<'a>(Ref<'a, App>);

impl Drop for AppRef<'_> {
    fn drop(&mut self) {
        if option_env!("TRACK_THREAD_BORROWS").is_some() {
            let thread_id = std::thread::current().id();
            eprintln!("dropped borrow from {thread_id:?}");
        }
    }
}

#[doc(hidden)]
#[derive(Deref, DerefMut)]
pub struct AppRefMut<'a>(RefMut<'a, App>);

impl Drop for AppRefMut<'_> {
    fn drop(&mut self) {
        if option_env!("TRACK_THREAD_BORROWS").is_some() {
            let thread_id = std::thread::current().id();
            eprintln!("dropped {thread_id:?}");
        }
    }
}

/// 对 RGPUI 应用的引用，通常在应用的 `main` 函数中构建。
/// 除初始配置和启动阶段外，你不会频繁与此类型交互。
pub struct Application(Rc<AppCell>);

/// 通过 [`Application::run_embedded`] 启动的应用的强引用句柄。
///
/// 丢弃此句柄将释放应用，因此嵌入方必须持有它直到应用结束运行。
/// 持有期间，它就是嵌入方在外部运行循环每次交还控制权时重新进入 RGPUI 的入口。
pub struct ApplicationHandle {
    app: Rc<AppCell>,
}

impl ApplicationHandle {
    /// 使用应用上下文调用 `f`。不可在已在 update 内部的代码中重入调用；
    /// 应用状态是 `RefCell`，双重借用会 panic。
    pub fn update<R>(&self, f: impl FnOnce(&mut App) -> R) -> R {
        let cx = &mut *self.app.borrow_mut();
        f(cx)
    }

    /// 用于跨 await 点使用的 [`AsyncApp`]。它弱引用应用；保持应用存活仍是此句柄的职责。
    pub fn to_async(&self) -> AsyncApp {
        self.update(|cx| cx.to_async())
    }
}

/// 表示尚未完全启动的应用。配置完成后，
/// 使用 `App::run` 启动应用。
impl Application {
    /// 使用调用方提供的平台实现构建应用。
    pub fn with_platform(platform: Rc<dyn Platform>) -> Self {
        Self(App::new_app(
            platform,
            Arc::new(()),
            Arc::new(NullHttpClient),
        ))
    }

    /// 强制禁用无障碍（AccessKit）集成来构建应用。
    ///
    /// 在此模式下，无障碍 API（如 [`div().role()`][crate::StatefulInteractiveElement::role]）
    /// 会静默地不执行任何操作。
    ///
    /// 参见[无障碍指南](crate::_accessibility)了解被禁用功能的概述。
    pub fn new_inaccessible(platform: Rc<dyn Platform>) -> Self {
        let this = Self::with_platform(platform);
        this.0.borrow_mut().accessibility_force_disabled = true;
        this
    }

    /// 设置应用的资源来源。
    pub fn with_assets(self, asset_source: impl AssetSource) -> Self {
        let mut context_lock = self.0.borrow_mut();
        let asset_source = Arc::new(asset_source);
        context_lock.asset_source = asset_source.clone();
        context_lock.svg_renderer = SvgRenderer::new(asset_source);
        drop(context_lock);
        self
    }

    /// 设置应用的 HTTP 客户端。
    pub fn with_http_client(self, http_client: Arc<dyn HttpClient>) -> Self {
        let mut context_lock = self.0.borrow_mut();
        context_lock.http_client = http_client;
        drop(context_lock);
        self
    }

    /// 配置应用自动退出的时机。
    /// 默认使用 [`QuitMode::Default`]。
    pub fn with_quit_mode(self, mode: QuitMode) -> Self {
        self.0.borrow_mut().quit_mode = mode;
        self
    }

    /// 启动应用。提供的回调将在应用完全启动后被调用。
    pub fn run<F>(self, on_finish_launching: F)
    where
        F: 'static + FnOnce(&mut App),
    {
        let this = self.0.clone();
        let platform = self.0.borrow().platform.clone();
        platform.run(Box::new(move || {
            let cx = &mut *this.borrow_mut();
            on_finish_launching(cx);
        }));
    }

    /// 为自行驱动运行循环的嵌入方启动应用。
    ///
    /// 在普通平台上，`Platform::run` 会在应用生命周期内阻塞，应用状态由
    /// [`Application::run`] 的栈帧保持存活。嵌入平台（即运行循环归属他方，
    /// 例如编译为 Wasm guest 的 RGPUI，或托管在外部原生应用中的 RGPUI 视图）
    /// 实现 `Platform::run` 时会调用启动回调后立即返回。本方法支持这种模式：
    /// 它返回一个 [`ApplicationHandle`] 来保持应用存活，并允许嵌入方在外部运行
    /// 循环交还控制权时重新进入应用。
    pub fn run_embedded<F>(self, on_finish_launching: F) -> ApplicationHandle
    where
        F: 'static + FnOnce(&mut App),
    {
        let this = self.0.clone();
        let platform = self.0.borrow().platform.clone();
        platform.run(Box::new(move || {
            let cx = &mut *this.borrow_mut();
            on_finish_launching(cx);
        }));
        ApplicationHandle { app: self.0 }
    }

    /// 注册一个处理器，当平台指示应用打开一个或多个 URL 时被调用。
    pub fn on_open_urls<F>(&self, mut callback: F) -> &Self
    where
        F: 'static + FnMut(Vec<String>),
    {
        self.0.borrow().platform.on_open_urls(Box::new(callback));
        self
    }

    /// 当已运行的应用再次被启动时调用处理器。
    /// 在 macOS 上，双击应用图标或通过 Dock 启动应用时会触发此回调。
    pub fn on_reopen<F>(&self, mut callback: F) -> &Self
    where
        F: 'static + FnMut(&mut App),
    {
        let this = Rc::downgrade(&self.0);
        self.0.borrow_mut().platform.on_reopen(Box::new(move || {
            if let Some(app) = this.upgrade() {
                callback(&mut app.borrow_mut());
            }
        }));
        self
    }

    /// 当系统从休眠中唤醒时调用处理器。
    pub fn on_system_wake<F>(&self, mut callback: F) -> &Self
    where
        F: 'static + FnMut(&mut App),
    {
        let this = Rc::downgrade(&self.0);
        self.0
            .borrow_mut()
            .platform
            .on_system_wake(Box::new(move || {
                if let Some(app) = this.upgrade() {
                    callback(&mut app.borrow_mut());
                }
            }));
        self
    }

    /// 返回与此应用关联的 [`BackgroundExecutor`] 句柄，可用于在后台生成 future。
    pub fn background_executor(&self) -> BackgroundExecutor {
        self.0.borrow().background_executor.clone()
    }

    /// 返回与此应用关联的 [`ForegroundExecutor`] 句柄，可用于在前台生成 future。
    pub fn foreground_executor(&self) -> ForegroundExecutor {
        self.0.borrow().foreground_executor.clone()
    }

    /// 返回与此应用关联的 [`TextSystem`] 引用。
    pub fn text_system(&self) -> Arc<TextSystem> {
        self.0.borrow().text_system.clone()
    }

    /// 返回应用 bundle 中指定名称的可执行文件的文件 URL
    pub fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf> {
        self.0.borrow().path_for_auxiliary_executable(name)
    }
}

type Handler = Box<dyn FnMut(&mut App) -> bool + 'static>;
type Listener = Box<dyn FnMut(&dyn Any, &mut App) -> bool + 'static>;
pub(crate) type KeystrokeObserver =
    Box<dyn FnMut(&KeystrokeEvent, &mut Window, &mut App) -> bool + 'static>;
type QuitHandler = Box<dyn FnOnce(&mut App) -> LocalBoxFuture<'static, ()> + 'static>;
type WindowClosedHandler = Box<dyn FnMut(&mut App, WindowId)>;
type ReleaseListener = Box<dyn FnOnce(&mut dyn Any, &mut App) + 'static>;
type NewEntityListener = Box<dyn FnMut(AnyEntity, &mut Option<&mut Window>, &mut App) + 'static>;

/// 定义应用自动退出的时机。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuitMode {
    /// macOS 上使用 [`QuitMode::Explicit`]，其他平台使用 [`QuitMode::LastWindowClosed`]。
    #[default]
    Default,
    /// 最后一个窗口关闭时自动退出。
    LastWindowClosed,
    /// 仅在通过 [`App::quit`] 请求时退出。
    Explicit,
}

/// 控制 RGPUI 在响应键盘输入时何时隐藏鼠标光标。
///
/// 鼠标移动时的恢复由平台层处理；此枚举仅描述
/// *触发*隐藏的策略。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum CursorHideMode {
    /// 从不自动隐藏光标。
    Never,
    /// 在产生字符的按键（打字）时隐藏。
    OnTyping,
    /// 在产生字符的按键时隐藏，*以及*当按键绑定
    /// 解析为消耗该按键的动作时也隐藏。
    #[default]
    OnTypingAndAction,
}

#[doc(hidden)]
#[derive(Clone, PartialEq, Eq)]
pub struct SystemWindowTab {
    pub id: WindowId,
    pub title: SharedString,
    pub handle: AnyWindowHandle,
    pub last_active_at: Instant,
}

impl SystemWindowTab {
    /// 创建窗口标签页的新实例。
    pub fn new(title: SharedString, handle: AnyWindowHandle) -> Self {
        Self {
            id: handle.id,
            title,
            handle,
            last_active_at: Instant::now(),
        }
    }
}

/// 管理窗口标签页的控制器。
#[derive(Default)]
pub struct SystemWindowTabController {
    visible: Option<bool>,
    tab_groups: FxHashMap<usize, Vec<SystemWindowTab>>,
}

impl Global for SystemWindowTabController {}

impl SystemWindowTabController {
    /// 创建窗口标签页控制器的新实例。
    pub fn new() -> Self {
        Self {
            visible: None,
            tab_groups: FxHashMap::default(),
        }
    }

    /// 初始化全局窗口标签页控制器。
    pub fn init(cx: &mut App) {
        cx.set_global(SystemWindowTabController::new());
    }

    /// 获取所有标签页分组。
    pub fn tab_groups(&self) -> &FxHashMap<usize, Vec<SystemWindowTab>> {
        &self.tab_groups
    }

    /// 获取下一个标签页分组的窗口句柄。
    pub fn get_next_tab_group_window(cx: &mut App, id: WindowId) -> Option<&AnyWindowHandle> {
        let controller = cx.global::<SystemWindowTabController>();
        let current_group = controller
            .tab_groups
            .iter()
            .find_map(|(group, tabs)| tabs.iter().find(|tab| tab.id == id).map(|_| group));

        let current_group = current_group?;
        // 按 group_id 排序确保稳定的循环顺序
        let mut group_ids: Vec<_> = controller.tab_groups.keys().collect();
        group_ids.sort();
        let idx = group_ids.iter().position(|g| *g == current_group)?;
        let next_idx = (idx + 1) % group_ids.len();

        controller
            .tab_groups
            .get(group_ids[next_idx])
            .and_then(|tabs| {
                tabs.iter()
                    .max_by_key(|tab| tab.last_active_at)
                    .or_else(|| tabs.first())
                    .map(|tab| &tab.handle)
            })
    }

    /// 获取上一个标签页分组的窗口句柄。
    pub fn get_prev_tab_group_window(cx: &mut App, id: WindowId) -> Option<&AnyWindowHandle> {
        let controller = cx.global::<SystemWindowTabController>();
        let current_group = controller
            .tab_groups
            .iter()
            .find_map(|(group, tabs)| tabs.iter().find(|tab| tab.id == id).map(|_| group));

        let current_group = current_group?;
        // 按 group_id 排序确保稳定的循环顺序
        let mut group_ids: Vec<_> = controller.tab_groups.keys().collect();
        group_ids.sort();
        let idx = group_ids.iter().position(|g| *g == current_group)?;
        let prev_idx = if idx == 0 {
            group_ids.len() - 1
        } else {
            idx - 1
        };

        controller
            .tab_groups
            .get(group_ids[prev_idx])
            .and_then(|tabs| {
                tabs.iter()
                    .max_by_key(|tab| tab.last_active_at)
                    .or_else(|| tabs.first())
                    .map(|tab| &tab.handle)
            })
    }

    /// 获取同一窗口中的所有标签页。
    pub fn tabs(&self, id: WindowId) -> Option<&Vec<SystemWindowTab>> {
        self.tab_groups
            .values()
            .find(|tabs| tabs.iter().any(|tab| tab.id == id))
    }

    /// 初始化系统窗口标签页控制器的可见性。
    pub fn init_visible(cx: &mut App, visible: bool) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        if controller.visible.is_none() {
            controller.visible = Some(visible);
        }
    }

    /// 获取系统窗口标签页控制器的可见性。
    pub fn is_visible(&self) -> bool {
        self.visible.unwrap_or(false)
    }

    /// 设置系统窗口标签页控制器的可见性。
    pub fn set_visible(cx: &mut App, visible: bool) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        controller.visible = Some(visible);
    }

    /// 更新窗口的最后活跃时间。
    pub fn update_last_active(cx: &mut App, id: WindowId) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        for windows in controller.tab_groups.values_mut() {
            for tab in windows.iter_mut() {
                if tab.id == id {
                    tab.last_active_at = Instant::now();
                }
            }
        }
    }

    /// 更新标签页在其分组中的位置。
    pub fn update_tab_position(cx: &mut App, id: WindowId, ix: usize) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        for (_, windows) in controller.tab_groups.iter_mut() {
            if let Some(current_pos) = windows.iter().position(|tab| tab.id == id) {
                if ix < windows.len() && current_pos != ix {
                    let window_tab = windows.remove(current_pos);
                    windows.insert(ix, window_tab);
                }
                break;
            }
        }
    }

    /// 更新标签页的标题。
    pub fn update_tab_title(cx: &mut App, id: WindowId, title: SharedString) {
        let controller = cx.global::<SystemWindowTabController>();
        let tab = controller
            .tab_groups
            .values()
            .flat_map(|windows| windows.iter())
            .find(|tab| tab.id == id);

        if tab.map_or(true, |t| t.title == title) {
            return;
        }

        let mut controller = cx.global_mut::<SystemWindowTabController>();
        for windows in controller.tab_groups.values_mut() {
            for tab in windows.iter_mut() {
                if tab.id == id {
                    tab.title = title;
                    return;
                }
            }
        }
    }

    /// 将标签页插入标签页分组。
    pub fn add_tab(cx: &mut App, id: WindowId, tabs: Vec<SystemWindowTab>) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        let Some(tab) = tabs.iter().find(|tab| tab.id == id).cloned() else {
            return;
        };

        let mut expected_tab_ids: Vec<_> = tabs
            .iter()
            .filter(|tab| tab.id != id)
            .map(|tab| tab.id)
            .sorted()
            .collect();

        let mut tab_group_id = None;
        for (group_id, group_tabs) in &controller.tab_groups {
            let tab_ids: Vec<_> = group_tabs.iter().map(|tab| tab.id).sorted().collect();
            if tab_ids == expected_tab_ids {
                tab_group_id = Some(*group_id);
                break;
            }
        }

        if let Some(tab_group_id) = tab_group_id {
            if let Some(tabs) = controller.tab_groups.get_mut(&tab_group_id) {
                tabs.push(tab);
            }
        } else {
            let new_group_id = controller.tab_groups.len();
            controller.tab_groups.insert(new_group_id, tabs);
        }
    }

    /// 从标签页分组中移除标签页。
    pub fn remove_tab(cx: &mut App, id: WindowId) -> Option<SystemWindowTab> {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        let mut removed_tab = None;

        controller.tab_groups.retain(|_, tabs| {
            if let Some(pos) = tabs.iter().position(|tab| tab.id == id) {
                removed_tab = Some(tabs.remove(pos));
            }
            !tabs.is_empty()
        });

        removed_tab
    }

    /// 将标签页移动到新的标签页分组。
    pub fn move_tab_to_new_window(cx: &mut App, id: WindowId) {
        let mut removed_tab = Self::remove_tab(cx, id);
        let mut controller = cx.global_mut::<SystemWindowTabController>();

        if let Some(tab) = removed_tab {
            let new_group_id = controller.tab_groups.keys().max().map_or(0, |k| k + 1);
            controller.tab_groups.insert(new_group_id, vec![tab]);
        }
    }

    /// 将所有标签页分组合并为一个分组。
    pub fn merge_all_windows(cx: &mut App, id: WindowId) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        let Some(initial_tabs) = controller.tabs(id) else {
            return;
        };

        let initial_tabs_len = initial_tabs.len();
        let mut all_tabs = initial_tabs.clone();

        for (_, mut tabs) in controller.tab_groups.drain() {
            tabs.retain(|tab| !all_tabs[..initial_tabs_len].contains(tab));
            all_tabs.extend(tabs);
        }

        controller.tab_groups.insert(0, all_tabs);
    }

    /// 在标签页分组中向后方向选择下一个标签页。
    pub fn select_next_tab(cx: &mut App, id: WindowId) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        let Some(tabs) = controller.tabs(id) else {
            return;
        };

        let current_index = tabs.iter().position(|tab| tab.id == id).unwrap();
        let next_index = (current_index + 1) % tabs.len();

        let _ = &tabs[next_index].handle.update(cx, |_, window, _| {
            window.activate_window();
        });
    }

    /// 在标签页分组中向前方向选择上一个标签页。
    pub fn select_previous_tab(cx: &mut App, id: WindowId) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        let Some(tabs) = controller.tabs(id) else {
            return;
        };

        let current_index = tabs.iter().position(|tab| tab.id == id).unwrap();
        let previous_index = if current_index == 0 {
            tabs.len() - 1
        } else {
            current_index - 1
        };

        let _ = &tabs[previous_index].handle.update(cx, |_, window, _| {
            window.activate_window();
        });
    }
}

pub(crate) enum GpuiMode {
    #[cfg(any(test, feature = "test-support"))]
    Test {
        skip_drawing: bool,
    },
    Production,
}

impl GpuiMode {
    #[cfg(any(test, feature = "test-support"))]
    pub fn test() -> Self {
        GpuiMode::Test {
            skip_drawing: false,
        }
    }

    #[inline]
    pub(crate) fn skip_drawing(&self) -> bool {
        match self {
            #[cfg(any(test, feature = "test-support"))]
            GpuiMode::Test { skip_drawing } => *skip_drawing,
            GpuiMode::Production => false,
        }
    }
}

/// 包含整个应用的状态，作为引用传递给各种回调。
/// 其他 [Context] 通过解引用转换为此类型。
/// 需要 `App` 的引用来访问 [Entity] 的状态。
pub struct App {
    pub(crate) this: Weak<AppCell>,
    pub(crate) platform: Rc<dyn Platform>,
    text_system: Arc<TextSystem>,

    pub(crate) actions: Rc<ActionRegistry>,
    pub(crate) active_drag: Option<AnyDrag>,
    pub(crate) background_executor: BackgroundExecutor,
    pub(crate) foreground_executor: ForegroundExecutor,
    pub(crate) entities: EntityMap,
    pub(crate) new_entity_observers: SubscriberSet<TypeId, NewEntityListener>,
    pub(crate) windows: SlotMap<WindowId, Option<Box<Window>>>,
    pub(crate) window_handles: FxHashMap<WindowId, AnyWindowHandle>,
    pub(crate) focus_handles: Arc<FocusMap>,
    pub(crate) keymap: Rc<RefCell<Keymap>>,
    pub(crate) keyboard_layout: Box<dyn PlatformKeyboardLayout>,
    pub(crate) keyboard_mapper: Rc<dyn PlatformKeyboardMapper>,
    pub(crate) global_action_listeners:
        TypeIdHashMap<Vec<Rc<dyn Fn(&dyn Any, DispatchPhase, &mut Self)>>>,
    pending_effects: VecDeque<Effect>,

    pub(crate) observers: SubscriberSet<EntityId, Handler>,
    pub(crate) event_listeners: SubscriberSet<EntityId, (TypeId, Listener)>,
    pub(crate) keystroke_observers: SubscriberSet<(), KeystrokeObserver>,
    pub(crate) keystroke_interceptors: SubscriberSet<(), KeystrokeObserver>,
    pub(crate) keyboard_layout_observers: SubscriberSet<(), Handler>,
    pub(crate) thermal_state_observers: SubscriberSet<(), Handler>,
    pub(crate) release_listeners: SubscriberSet<EntityId, ReleaseListener>,
    pub(crate) global_observers: SubscriberSet<TypeId, Handler>,
    pub(crate) quit_observers: SubscriberSet<(), QuitHandler>,
    pub(crate) restart_observers: SubscriberSet<(), Handler>,
    pub(crate) window_closed_observers: SubscriberSet<(), WindowClosedHandler>,

    /// 每个 App 的元素 arena。在不同 App 实例间隔离元素分配
    /// （对多个 App 并发运行的测试很重要）。
    pub(crate) element_arena: RefCell<Arena>,
    /// 每个 App 的事件 arena。
    pub(crate) event_arena: Arena,

    // Drop globals last. We need to ensure all tasks owned by entities and
    // callbacks are marked cancelled at this point as this will also shutdown
    // the tokio runtime. As any task attempting to spawn a blocking tokio task,
    // might panic.
    pub(crate) globals_by_type: TypeIdHashMap<Box<dyn Any>>,

    // assets
    pub(crate) loading_assets: FxHashMap<(TypeId, u64), Box<dyn Any>>,
    asset_source: Arc<dyn AssetSource>,
    pub(crate) svg_renderer: SvgRenderer,
    http_client: Arc<dyn HttpClient>,

    // below is plain data, the drop order is insignificant here
    pub(crate) pending_notifications: FxHashSet<EntityId>,
    pub(crate) pending_global_notifications: TypeIdHashSet,
    pub(crate) restart_path: Option<PathBuf>,
    pub(crate) layout_id_buffer: Vec<LayoutId>, // We recycle this memory across layout requests.
    pub(crate) propagate_event: bool,
    pub(crate) prompt_builder: Option<PromptBuilder>,
    pub(crate) window_invalidators_by_entity:
        FxHashMap<EntityId, FxHashMap<WindowId, WindowInvalidator>>,
    pub(crate) tracked_entities: FxHashMap<WindowId, FxHashSet<EntityId>>,
    pub(crate) current_window_by_entity: FxHashMap<EntityId, WindowId>,
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(crate) inspector_renderer: Option<crate::InspectorRenderer>,
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(crate) inspector_element_registry: InspectorElementRegistry,
    #[cfg(any(test, feature = "test-support", debug_assertions))]
    pub(crate) name: Option<&'static str>,
    pub(crate) text_rendering_mode: Rc<Cell<TextRenderingMode>>,

    pub(crate) window_update_stack: Vec<WindowId>,
    pub(crate) mode: GpuiMode,
    pub(crate) cursor_hide_mode: CursorHideMode,
    pub(crate) reduce_motion: bool,
    /// 共享时钟的原点，用于相位锁定同步的重复动画。
    pub(crate) synced_animation_epoch: Instant,
    /// 应用是否由 [`Application::new_inaccessible`] 创建。
    /// 设置此标志时不会调用任何 accesskit API。
    pub(crate) accessibility_force_disabled: bool,
    flushing_effects: bool,
    pending_updates: usize,
    quit_mode: QuitMode,
    quitting: bool,

    // We need to ensure the leak detector drops last, after all tasks, callbacks and things have been dropped.
    // Otherwise it may report false positives.
    #[cfg(any(test, feature = "leak-detection"))]
    _ref_counts: Arc<RwLock<EntityRefCounts>>,
}

impl App {
    #[allow(clippy::new_ret_no_self)]
    pub(crate) fn new_app(
        platform: Rc<dyn Platform>,
        asset_source: Arc<dyn AssetSource>,
        http_client: Arc<dyn HttpClient>,
    ) -> Rc<AppCell> {
        let background_executor = platform.background_executor();
        let foreground_executor = platform.foreground_executor();
        assert!(
            background_executor.is_main_thread(),
            "must construct App on main thread"
        );

        let text_system = Arc::new(TextSystem::new(platform.text_system()));
        let entities = EntityMap::new();
        let keyboard_layout = platform.keyboard_layout();
        let keyboard_mapper = platform.keyboard_mapper();
        let synced_animation_epoch = background_executor.now();

        #[cfg(any(test, feature = "leak-detection"))]
        let _ref_counts = entities.ref_counts_drop_handle();

        let app = Rc::new_cyclic(|this| AppCell {
            app: RefCell::new(App {
                this: this.clone(),
                platform: platform.clone(),
                text_system,
                text_rendering_mode: Rc::new(Cell::new(TextRenderingMode::default())),
                mode: GpuiMode::Production,
                actions: Rc::new(ActionRegistry::default()),
                flushing_effects: false,
                pending_updates: 0,
                active_drag: None,
                background_executor,
                foreground_executor,
                svg_renderer: SvgRenderer::new(asset_source.clone()),
                loading_assets: Default::default(),
                asset_source,
                http_client,
                globals_by_type: Default::default(),
                entities,
                new_entity_observers: SubscriberSet::new(),
                windows: SlotMap::with_key(),
                window_update_stack: Vec::new(),
                window_handles: FxHashMap::default(),
                focus_handles: Arc::new(RwLock::new(SlotMap::with_key())),
                keymap: Rc::new(RefCell::new(Keymap::default())),
                keyboard_layout,
                keyboard_mapper,
                global_action_listeners: Default::default(),
                pending_effects: VecDeque::new(),
                pending_notifications: FxHashSet::default(),
                pending_global_notifications: Default::default(),
                observers: SubscriberSet::new(),
                tracked_entities: FxHashMap::default(),
                window_invalidators_by_entity: FxHashMap::default(),
                current_window_by_entity: FxHashMap::default(),
                event_listeners: SubscriberSet::new(),
                release_listeners: SubscriberSet::new(),
                keystroke_observers: SubscriberSet::new(),
                keystroke_interceptors: SubscriberSet::new(),
                keyboard_layout_observers: SubscriberSet::new(),
                thermal_state_observers: SubscriberSet::new(),
                global_observers: SubscriberSet::new(),
                quit_observers: SubscriberSet::new(),
                restart_observers: SubscriberSet::new(),
                restart_path: None,
                window_closed_observers: SubscriberSet::new(),
                layout_id_buffer: Default::default(),
                propagate_event: true,
                prompt_builder: Some(PromptBuilder::Default),
                #[cfg(any(feature = "inspector", debug_assertions))]
                inspector_renderer: None,
                #[cfg(any(feature = "inspector", debug_assertions))]
                inspector_element_registry: InspectorElementRegistry::default(),
                quit_mode: QuitMode::default(),
                quitting: false,
                cursor_hide_mode: CursorHideMode::default(),
                reduce_motion: false,
                synced_animation_epoch,
                accessibility_force_disabled: false,

                #[cfg(any(test, feature = "test-support", debug_assertions))]
                name: None,
                element_arena: RefCell::new(Arena::new(1024 * 1024)),
                event_arena: Arena::new(1024 * 1024),

                #[cfg(any(test, feature = "leak-detection"))]
                _ref_counts,
            }),
        });

        init_app_menus(platform.as_ref(), &app.borrow());
        SystemWindowTabController::init(&mut app.borrow_mut());

        platform.on_keyboard_layout_change(Box::new({
            let app = Rc::downgrade(&app);
            move || {
                if let Some(app) = app.upgrade() {
                    let cx = &mut app.borrow_mut();
                    cx.keyboard_layout = cx.platform.keyboard_layout();
                    cx.keyboard_mapper = cx.platform.keyboard_mapper();
                    cx.keyboard_layout_observers
                        .clone()
                        .retain(&(), move |callback| (callback)(cx));
                }
            }
        }));

        platform.on_thermal_state_change(Box::new({
            let app = Rc::downgrade(&app);
            move || {
                if let Some(app) = app.upgrade() {
                    let cx = &mut app.borrow_mut();
                    cx.thermal_state_observers
                        .clone()
                        .retain(&(), move |callback| (callback)(cx));
                }
            }
        }));

        platform.on_quit(Box::new({
            let cx = Rc::downgrade(&app);
            move || {
                if let Some(cx) = cx.upgrade() {
                    cx.borrow_mut().shutdown();
                }
            }
        }));

        app
    }

    #[doc(hidden)]
    pub fn ref_counts_drop_handle(&self) -> impl Sized + use<> {
        self.entities.ref_counts_drop_handle()
    }

    /// 捕获当前所有拥有存活句柄的实体的快照。
    ///
    /// 返回的 [`LeakDetectorSnapshot`] 稍后可传递给
    /// [`assert_no_new_leaks`](Self::assert_no_new_leaks) 以验证快照之后
    /// 创建的实体是否仍然存活。
    #[cfg(any(test, feature = "leak-detection"))]
    pub fn leak_detector_snapshot(&self) -> LeakDetectorSnapshot {
        self.entities.leak_detector_snapshot()
    }

    /// 断言在 `snapshot` 之后创建的实体没有仍然存活的句柄。
    ///
    /// 快照时已被跟踪的实体会被忽略，即使它们仍有句柄。
    /// 只有 *新* 实体（其 `EntityId` 不在快照中）才被视为泄漏。
    ///
    /// # panic
    ///
    /// 如果存在新的实体句柄则 panic。panic 消息会列出每个
    /// 泄漏实体的类型名称，当设置了 `LEAK_BACKTRACE` 时
    /// 还会包含分配位置的回溯信息。
    #[cfg(any(test, feature = "leak-detection"))]
    pub fn assert_no_new_leaks(&self, snapshot: &LeakDetectorSnapshot) {
        self.entities.assert_no_new_leaks(snapshot)
    }

    /// 优雅地退出应用。通过 [`Context::on_app_quit`] 注册的处理器将获得
    /// `SHUTDOWN_TIMEOUT` 的时间来完成，然后才会退出。
    pub fn shutdown(&mut self) {
        let mut futures = Vec::new();

        for observer in self.quit_observers.remove(&()) {
            futures.push(observer(self));
        }

        self.windows.clear();
        self.window_handles.clear();
        self.flush_effects();
        self.quitting = true;

        let futures = futures::future::join_all(futures);
        if self
            .foreground_executor
            .block_with_timeout(SHUTDOWN_TIMEOUT, futures)
            .is_err()
        {
            log::error!("timed out waiting on app_will_quit");
        }

        self.quitting = false;
    }

    /// 获取当前键盘布局的 ID。
    pub fn keyboard_layout(&self) -> &dyn PlatformKeyboardLayout {
        self.keyboard_layout.as_ref()
    }

    /// 获取当前键盘映射器。
    pub fn keyboard_mapper(&self) -> &Rc<dyn PlatformKeyboardMapper> {
        &self.keyboard_mapper
    }

    /// 当当前键盘布局发生变化时调用处理器
    pub fn on_keyboard_layout_change<F>(&self, mut callback: F) -> Subscription
    where
        F: 'static + FnMut(&mut App),
    {
        let (subscription, activate) = self.keyboard_layout_observers.insert(
            (),
            Box::new(move |cx| {
                callback(cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// 通过平台的标准例程优雅退出应用。
    pub fn quit(&self) {
        self.platform.quit();
    }

    /// 返回当前响应键盘输入时隐藏光标的策略。
    pub fn cursor_hide_mode(&self) -> CursorHideMode {
        self.cursor_hide_mode
    }

    /// 设置 RGPUI 在响应键盘输入时隐藏光标的策略。
    pub fn set_cursor_hide_mode(&mut self, mode: CursorHideMode) {
        self.cursor_hide_mode = mode;
    }

    /// 根据平台判断光标当前是否可见。当键盘输入隐藏了光标且
    /// 用户尚未移动鼠标恢复时，此方法返回 `false`。
    ///
    /// 参见 [`App::set_cursor_hide_mode`]。
    pub fn is_cursor_visible(&self) -> bool {
        self.platform.is_cursor_visible()
    }

    /// 返回非必要动画（如加载旋转器）是否应以静态状态渲染而非动画播放。
    pub fn reduce_motion(&self) -> bool {
        self.reduce_motion
    }

    /// 设置非必要动画（如加载旋转器）是否应以静态状态渲染而非动画播放。
    pub fn set_reduce_motion(&mut self, reduce_motion: bool) {
        if self.reduce_motion != reduce_motion {
            self.reduce_motion = reduce_motion;
            self.refresh_windows();
        }
    }

    /// 调度应用中所有窗口重绘。可在更新周期内多次调用，
    /// 仍只会产生一次重绘。
    pub fn refresh_windows(&mut self) {
        self.pending_effects.push_back(Effect::RefreshWindows);
    }

    pub(crate) fn update<R>(&mut self, update: impl FnOnce(&mut Self) -> R) -> R {
        self.start_update();
        let result = update(self);
        self.finish_update();
        result
    }

    pub(crate) fn start_update(&mut self) {
        self.pending_updates += 1;
    }

    pub(crate) fn finish_update(&mut self) {
        if !self.flushing_effects && self.pending_updates == 1 {
            self.flushing_effects = true;
            self.flush_effects();
            self.flushing_effects = false;
        }
        self.pending_updates -= 1;
    }

    /// 安排一个回调，当给定实体在其对应上下文中调用 `notify` 时被调用。
    pub fn observe<W>(
        &mut self,
        entity: &Entity<W>,
        mut on_notify: impl FnMut(Entity<W>, &mut App) + 'static,
    ) -> Subscription
    where
        W: 'static,
    {
        self.observe_internal(entity, move |e, cx| {
            on_notify(e, cx);
            true
        })
    }

    pub(crate) fn detect_accessed_entities<R>(
        &mut self,
        callback: impl FnOnce(&mut App) -> R,
    ) -> (R, FxHashSet<EntityId>) {
        let accessed_entities_start = self.entities.accessed_entities.get_mut().clone();
        let result = callback(self);
        let entities_accessed_in_callback = self
            .entities
            .accessed_entities
            .get_mut()
            .difference(&accessed_entities_start)
            .copied()
            .collect::<FxHashSet<EntityId>>();
        (result, entities_accessed_in_callback)
    }

    pub(crate) fn record_entities_accessed(
        &mut self,
        window_handle: AnyWindowHandle,
        invalidator: WindowInvalidator,
        entities: &FxHashSet<EntityId>,
    ) {
        let mut tracked_entities =
            std::mem::take(self.tracked_entities.entry(window_handle.id).or_default());
        for entity in tracked_entities.iter() {
            self.window_invalidators_by_entity
                .entry(*entity)
                .and_modify(|windows| {
                    windows.remove(&window_handle.id);
                });
        }
        for entity in entities.iter() {
            self.window_invalidators_by_entity
                .entry(*entity)
                .or_default()
                .insert(window_handle.id, invalidator.clone());
            self.current_window_by_entity
                .insert(*entity, window_handle.id);
        }
        tracked_entities.clear();
        tracked_entities.extend(entities.iter().copied());
        self.tracked_entities
            .insert(window_handle.id, tracked_entities);
    }

    pub(crate) fn new_observer(&mut self, key: EntityId, value: Handler) -> Subscription {
        let (subscription, activate) = self.observers.insert(key, value);
        self.defer(move |_| activate());
        subscription
    }

    pub(crate) fn observe_internal<W>(
        &mut self,
        entity: &Entity<W>,
        mut on_notify: impl FnMut(Entity<W>, &mut App) -> bool + 'static,
    ) -> Subscription
    where
        W: 'static,
    {
        let entity_id = entity.entity_id();
        let handle = entity.downgrade();
        self.new_observer(
            entity_id,
            Box::new(move |cx| {
                if let Some(entity) = handle.upgrade() {
                    on_notify(entity, cx)
                } else {
                    false
                }
            }),
        )
    }

    /// 安排一个回调，当给定实体发出给定类型的事件时被调用。
    /// 回调会收到发出实体的句柄和发出事件的引用。
    pub fn subscribe<T, Event>(
        &mut self,
        entity: &Entity<T>,
        mut on_event: impl FnMut(Entity<T>, &Event, &mut App) + 'static,
    ) -> Subscription
    where
        T: 'static + EventEmitter<Event>,
        Event: 'static,
    {
        self.subscribe_internal(entity, move |entity, event, cx| {
            on_event(entity, event, cx);
            true
        })
    }

    pub(crate) fn new_subscription(
        &mut self,
        key: EntityId,
        value: (TypeId, Listener),
    ) -> Subscription {
        let (subscription, activate) = self.event_listeners.insert(key, value);
        self.defer(move |_| activate());
        subscription
    }
    pub(crate) fn subscribe_internal<T, Evt>(
        &mut self,
        entity: &Entity<T>,
        mut on_event: impl FnMut(Entity<T>, &Evt, &mut App) -> bool + 'static,
    ) -> Subscription
    where
        T: 'static + EventEmitter<Evt>,
        Evt: 'static,
    {
        let entity_id = entity.entity_id();
        let handle = entity.downgrade();
        self.new_subscription(
            entity_id,
            (
                TypeId::of::<Evt>(),
                Box::new(move |event, cx| {
                    let event: &Evt = event.downcast_ref().expect("invalid event type");
                    if let Some(entity) = handle.upgrade() {
                        on_event(entity, event, cx)
                    } else {
                        false
                    }
                }),
            ),
        )
    }

    /// 返回应用中所有打开窗口的句柄。
    /// 每个句柄可以向下转型为该窗口根视图的类型化句柄。
    /// 要查找给定类型的所有窗口，可以使用 filter。
    pub fn windows(&self) -> Vec<AnyWindowHandle> {
        self.windows
            .keys()
            .flat_map(|window_id| self.window_handles.get(&window_id).copied())
            .collect()
    }

    /// 返回按屏幕上出现顺序排列的窗口句柄，从前到后。
    ///
    /// 返回列表中的第一个窗口是应用的活动/最顶层窗口。
    ///
    /// 如果平台尚未实现此方法，返回 None。
    pub fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        self.platform.window_stack()
    }

    /// 返回当前在平台级别获得焦点的窗口的句柄（如果存在）。
    pub fn active_window(&self) -> Option<AnyWindowHandle> {
        self.platform.active_window()
    }

    /// 使用给定选项和给定函数返回的根视图打开一个新窗口。
    /// 该函数使用 `Window` 调用，可用于与窗口特定功能交互。
    pub fn open_window<V: 'static + Render>(
        &mut self,
        options: crate::WindowOptions,
        build_root_view: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
    ) -> anyhow::Result<WindowHandle<V>> {
        self.update(|cx| {
            let id = cx.windows.insert(None);
            let handle = WindowHandle::new(id);
            match Window::new(handle.into(), options, cx) {
                Ok(mut window) => {
                    // 确保存在全局主题：Root 渲染需要主题；若未显式初始化则使用默认主题。
                    if !cx.has_global::<crate::theme::Theme>() {
                        cx.set_global(crate::theme::Theme::default());
                    }
                    cx.window_update_stack.push(id);
                    let root_view: AnyView = build_root_view(&mut window, cx).into();
                    cx.window_update_stack.pop();
                    // 自动将用户视图包装进 Root，以提供 tooltip/dialog 等全局覆盖层支持。
                    // 若用户已显式传入 Root（例如对话框测试），则不重复包装。
                    let root_view = if root_view.clone().downcast::<Root>().is_ok() {
                        root_view
                    } else {
                        cx.new(|cx| Root::new(root_view, cx)).into()
                    };
                    window.root.replace(root_view);
                    window.defer(cx, |window: &mut Window, cx| window.appearance_changed(cx));

                    // allow a window to draw at least once before returning
                    // this didn't cause any issues on non windows platforms as it seems we always won the race to on_request_frame
                    // on windows we quite frequently lose the race and return a window that has never rendered, which leads to a crash
                    // where DispatchTree::root_node_id asserts on empty nodes
                    let clear = window.draw(cx);
                    clear.clear(cx);

                    cx.window_handles.insert(id, window.handle);
                    cx.windows.get_mut(id).unwrap().replace(Box::new(window));
                    Ok(handle)
                }
                Err(e) => {
                    cx.windows.remove(id);
                    Err(e)
                }
            }
        })
    }

    /// 指示平台通过将应用带到前台来激活应用。
    pub fn activate(&self, ignoring_other_apps: bool) {
        self.platform.activate(ignoring_other_apps);
    }

    /// 在平台级别隐藏应用。
    pub fn hide(&self) {
        self.platform.hide();
    }

    /// 在平台级别隐藏其他应用。
    pub fn hide_other_apps(&self) {
        self.platform.hide_other_apps();
    }

    /// 在平台级别取消隐藏其他应用。
    pub fn unhide_other_apps(&self) {
        self.platform.unhide_other_apps();
    }

    /// 返回当前活动显示器的列表。
    pub fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        self.platform.displays()
    }

    /// 返回将用于新窗口的主显示器。
    pub fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.platform.primary_display()
    }

    /// 返回 `screen_capture_sources` 是否可能工作。
    pub fn is_screen_capture_supported(&self) -> bool {
        self.platform.is_screen_capture_supported()
    }

    /// 返回可用屏幕捕获源的列表。
    pub fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
        self.platform.screen_capture_sources()
    }

    /// 返回具有给定 ID 的显示器（如果存在）。
    pub fn find_display(&self, id: DisplayId) -> Option<Rc<dyn PlatformDisplay>> {
        self.displays()
            .iter()
            .find(|display| display.id() == id)
            .cloned()
    }

    /// 返回系统当前的热状态。
    pub fn thermal_state(&self) -> ThermalState {
        self.platform.thermal_state()
    }

    /// 当热状态发生变化时调用处理器
    pub fn on_thermal_state_change<F>(&self, mut callback: F) -> Subscription
    where
        F: 'static + FnMut(&mut App),
    {
        let (subscription, activate) = self.thermal_state_observers.insert(
            (),
            Box::new(move |cx| {
                callback(cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// 返回应用窗口的外观。
    pub fn window_appearance(&self) -> WindowAppearance {
        self.platform.window_appearance()
    }

    /// 返回受支持时的窗口按钮布局配置。
    pub fn button_layout(&self) -> Option<WindowButtonLayout> {
        self.platform.button_layout()
    }

    /// 从平台剪贴板读取数据。
    pub fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.platform.read_from_clipboard()
    }

    /// 设置应用的文本渲染模式。
    pub fn set_text_rendering_mode(&mut self, mode: TextRenderingMode) {
        self.text_rendering_mode.set(mode);
    }

    /// 返回应用当前的文本渲染模式。
    pub fn text_rendering_mode(&self) -> TextRenderingMode {
        self.text_rendering_mode.get()
    }

    /// 向平台剪贴板写入数据。
    pub fn write_to_clipboard(&self, item: ClipboardItem) {
        self.platform.write_to_clipboard(item)
    }

    /// 从主选择缓冲区读取数据。
    /// 仅在 Linux 上可用。
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    pub fn read_from_primary(&self) -> Option<ClipboardItem> {
        self.platform.read_from_primary()
    }

    /// 向主选择缓冲区写入数据。
    /// 仅在 Linux 上可用。
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    pub fn write_to_primary(&self, item: ClipboardItem) {
        self.platform.write_to_primary(item)
    }

    /// 从 macOS 的"查找"粘贴板读取数据。
    ///
    /// 用于在应用之间共享当前搜索字符串。
    ///
    /// https://developer.apple.com/documentation/appkit/nspasteboard/name-swift.struct/find
    #[cfg(target_os = "macos")]
    pub fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        self.platform.read_from_find_pasteboard()
    }

    /// 向 macOS 的"查找"粘贴板写入数据。
    ///
    /// 用于在应用之间共享当前搜索字符串。
    ///
    /// https://developer.apple.com/documentation/appkit/nspasteboard/name-swift.struct/find
    #[cfg(target_os = "macos")]
    pub fn write_to_find_pasteboard(&self, item: ClipboardItem) {
        self.platform.write_to_find_pasteboard(item)
    }

    /// 向平台密钥链写入凭据。
    pub fn write_credentials(
        &self,
        url: &str,
        username: &str,
        password: &[u8],
    ) -> Task<Result<()>> {
        self.platform.write_credentials(url, username, password)
    }

    /// 从平台密钥链读取凭据。
    pub fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        self.platform.read_credentials(url)
    }

    /// 从平台密钥链删除凭据。
    pub fn delete_credentials(&self, url: &str) -> Task<Result<()>> {
        self.platform.delete_credentials(url)
    }

    /// 指示平台默认浏览器打开给定的 URL。
    pub fn open_url(&self, url: &str) {
        self.platform.open_url(url);
    }

    /// 注册给定的 URL scheme（例如 `rgpui` 用于 `rgpui://` URL）以由当前应用打开。
    ///
    /// 在某些平台（例如 macOS）上，你可以在应用分发时注册 URL scheme，
    /// 但此方法允许你在运行时注册 scheme。
    pub fn register_url_scheme(&self, scheme: &str) -> Task<Result<()>> {
        self.platform.register_url_scheme(scheme)
    }

    /// 返回当前应用 bundle 的完整路径名。
    ///
    /// 如果应用不是从 bundle 运行的，则返回错误。
    pub fn app_path(&self) -> Result<PathBuf> {
        self.platform.app_path()
    }

    /// 在 Linux 上，返回正在使用的合成器名称。
    ///
    /// 在其他平台上返回空字符串。
    pub fn compositor_name(&self) -> &'static str {
        self.platform.compositor_name()
    }

    /// 返回应用 bundle 中指定名称的可执行文件的文件 URL
    pub fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf> {
        self.platform.path_for_auxiliary_executable(name)
    }

    /// 显示用于选择路径的平台模态框。
    ///
    /// 当选择一个或多个路径时，它们将通过返回的 oneshot 通道异步中继。
    /// 如果取消，则中继 `None`。
    /// 在 Linux 上，如果无法打开文件选择器，可能返回错误。
    pub fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        self.platform.prompt_for_paths(options)
    }

    /// 显示用于选择新路径的平台模态框，文件可以保存到该路径。
    ///
    /// 提供的目录将用于设置初始位置。
    /// 当选择路径时，它将通过返回的 oneshot 通道异步中继。
    /// 如果取消，则中继 `None`。
    /// 在 Linux 上，如果无法打开文件选择器，可能返回错误。
    pub fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        self.platform.prompt_for_new_path(directory, suggested_name)
    }

    /// 在平台级别显示指定路径，例如在 macOS 的 Finder 中。
    pub fn reveal_path(&self, path: &Path) {
        self.platform.reveal_path(path)
    }

    /// 使用系统默认应用程序打开指定路径。
    pub fn open_with_system(&self, path: &Path) {
        self.platform.open_with_system(path)
    }

    /// 返回用户是否在平台级别配置了滚动条自动隐藏。
    pub fn should_auto_hide_scrollbars(&self) -> bool {
        self.platform.should_auto_hide_scrollbars()
    }

    /// 重启应用。
    pub fn restart(&mut self) {
        self.restart_observers
            .clone()
            .retain(&(), |observer| observer(self));
        self.platform.restart(self.restart_path.take())
    }

    /// 设置重启应用时使用的路径。
    pub fn set_restart_path(&mut self, path: PathBuf) {
        self.restart_path = Some(path);
    }

    /// 返回应用的 HTTP 客户端。
    pub fn http_client(&self) -> Arc<dyn HttpClient> {
        self.http_client.clone()
    }

    /// 设置应用的 HTTP 客户端。
    pub fn set_http_client(&mut self, new_client: Arc<dyn HttpClient>) {
        self.http_client = new_client;
    }

    /// 配置应用自动退出的时机。
    /// 默认使用 [`QuitMode::Default`]。
    pub fn set_quit_mode(&mut self, mode: QuitMode) {
        self.quit_mode = mode;
    }

    /// 返回应用使用的 SVG 渲染器。
    pub fn svg_renderer(&self) -> SvgRenderer {
        self.svg_renderer.clone()
    }

    pub(crate) fn push_effect(&mut self, effect: Effect) {
        match &effect {
            Effect::Notify { emitter } => {
                if !self.pending_notifications.insert(*emitter) {
                    return;
                }
            }
            Effect::NotifyGlobalObservers { global_type } => {
                if !self.pending_global_notifications.insert(*global_type) {
                    return;
                }
            }
            _ => {}
        };

        self.pending_effects.push_back(effect);
    }

    /// 在 [`App::update`] 结束时调用，以完成所有副作用，
    /// 例如通知观察者、发出事件等。副作用本身可以产生副作用，
    /// 因此我们持续循环直到所有副作用被处理。
    fn flush_effects(&mut self) {
        loop {
            self.release_dropped_entities();
            self.release_dropped_focus_handles();
            if let Some(effect) = self.pending_effects.pop_front() {
                match effect {
                    Effect::Notify { emitter } => {
                        self.apply_notify_effect(emitter);
                    }

                    Effect::Emit {
                        emitter,
                        event_type,
                        event,
                    } => self.apply_emit_effect(emitter, event_type, &*event),

                    Effect::RefreshWindows => {
                        self.apply_refresh_effect();
                    }

                    Effect::NotifyGlobalObservers { global_type } => {
                        self.apply_notify_global_observers_effect(global_type);
                    }

                    Effect::Defer { callback } => {
                        self.apply_defer_effect(callback);
                    }
                    Effect::EntityCreated {
                        entity,
                        tid,
                        window,
                    } => {
                        self.apply_entity_created_effect(entity, tid, window);
                    }
                }
            } else {
                #[cfg(any(test, feature = "test-support", feature = "bench"))]
                for window in self
                    .windows
                    .values()
                    .filter_map(|window| {
                        let window = window.as_deref()?;
                        window.invalidator.is_dirty().then_some(window.handle)
                    })
                    .collect::<Vec<_>>()
                {
                    self.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
                        .unwrap();
                }

                if self.pending_effects.is_empty() {
                    self.event_arena.clear();
                    break;
                }
            }
        }
    }

    /// 在 `flush_effects` 期间重复调用，以释放引用计数已变为零的实体。
    /// 我们在丢弃每个实体之前调用所有释放观察者。
    fn release_dropped_entities(&mut self) {
        loop {
            let dropped = self.entities.take_dropped();
            if dropped.is_empty() {
                break;
            }

            for (entity_id, mut entity) in dropped {
                self.observers.remove(&entity_id);
                self.event_listeners.remove(&entity_id);
                self.window_invalidators_by_entity.remove(&entity_id);
                self.current_window_by_entity.remove(&entity_id);
                for release_callback in self.release_listeners.remove(&entity_id) {
                    release_callback(entity.as_mut(), self);
                }
            }
        }
    }

    /// 在 `flush_effects` 期间重复调用，以处理被丢弃的焦点句柄。
    fn release_dropped_focus_handles(&mut self) {
        self.focus_handles
            .clone()
            .write()
            .retain(|handle_id, focus| {
                if focus.ref_count.load(SeqCst) == 0 {
                    for window_handle in self.windows() {
                        window_handle
                            .update(self, |_, window, _| {
                                if window.focus == Some(handle_id) {
                                    window.blur();
                                }
                            })
                            .unwrap();
                    }
                    false
                } else {
                    true
                }
            });
    }

    fn apply_notify_effect(&mut self, emitter: EntityId) {
        self.pending_notifications.remove(&emitter);

        self.observers
            .clone()
            .retain(&emitter, |handler| handler(self));
    }

    fn apply_emit_effect(&mut self, emitter: EntityId, event_type: TypeId, event: &dyn Any) {
        self.event_listeners
            .clone()
            .retain(&emitter, |(stored_type, handler)| {
                if *stored_type == event_type {
                    handler(event, self)
                } else {
                    true
                }
            });
    }

    fn apply_refresh_effect(&mut self) {
        for window in self.windows.values_mut() {
            if let Some(window) = window.as_deref_mut() {
                window.refreshing = true;
                window.invalidator.set_dirty(true);
            }
        }
    }

    fn apply_notify_global_observers_effect(&mut self, type_id: TypeId) {
        self.pending_global_notifications.remove(&type_id);
        self.global_observers
            .clone()
            .retain(&type_id, |observer| observer(self));
    }

    fn apply_defer_effect(&mut self, callback: Box<dyn FnOnce(&mut Self) + 'static>) {
        callback(self);
    }

    fn apply_entity_created_effect(
        &mut self,
        entity: AnyEntity,
        tid: TypeId,
        window: Option<WindowId>,
    ) {
        // Seed the entity's current window from its creation context so
        // `with_window` resolves correctly before the entity has ever been
        // rendered.
        if let Some(id) = window {
            self.current_window_by_entity.insert(entity.entity_id(), id);
        }

        self.new_entity_observers.clone().retain(&tid, |observer| {
            if let Some(id) = window {
                self.update_window_id(id, {
                    let entity = entity.clone();
                    |_, window, cx| (observer)(entity, &mut Some(window), cx)
                })
                .expect("All windows should be off the stack when flushing effects");
            } else {
                (observer)(entity.clone(), &mut None, self)
            }
            true
        });
    }

    /// 对实体的*当前*窗口执行 `f`——即最近引用该实体的渲染窗口，
    /// 如果尚未渲染则为其创建窗口。如果实体没有当前窗口、
    /// 该窗口已关闭或已在更新栈上，则返回 `None`。
    pub fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R> {
        let window_id = *self.current_window_by_entity.get(&entity_id)?;
        self.update_window_id(window_id, |_, window, cx| f(window, cx))
            .ok()
    }

    fn ensure_window(&mut self, entity_id: EntityId, window: WindowId) {
        self.current_window_by_entity
            .entry(entity_id)
            .or_insert(window);
    }

    pub(crate) fn update_window_id<T, F>(&mut self, id: WindowId, update: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        self.update(|cx| {
            let mut window = cx.windows.get_mut(id)?.take()?;

            let root_view = window.root.clone().unwrap();

            cx.window_update_stack.push(window.handle.id);
            let result = update(root_view, &mut window, cx);
            fn trail(id: WindowId, window: Box<Window>, cx: &mut App) -> Option<()> {
                cx.window_update_stack.pop();

                if window.removed {
                    cx.window_handles.remove(&id);
                    cx.windows.remove(id);
                    if let Some(tracked) = cx.tracked_entities.remove(&id) {
                        for entity_id in tracked {
                            if let Some(windows) =
                                cx.window_invalidators_by_entity.get_mut(&entity_id)
                            {
                                windows.remove(&id);
                            }
                            if cx.current_window_by_entity.get(&entity_id) == Some(&id) {
                                cx.current_window_by_entity.remove(&entity_id);
                            }
                        }
                    }

                    cx.window_closed_observers.clone().retain(&(), |callback| {
                        callback(cx, id);
                        true
                    });

                    let quit_on_empty = match cx.quit_mode {
                        QuitMode::Explicit => false,
                        QuitMode::LastWindowClosed => true,
                        QuitMode::Default => cfg!(not(target_os = "macos")),
                    };

                    if quit_on_empty && cx.windows.is_empty() {
                        cx.quit();
                    }
                } else {
                    cx.windows.get_mut(id)?.replace(window);
                }
                Some(())
            }
            trail(id, window, cx)?;

            Some(result)
        })
        .context("window not found")
    }

    /// 创建一个 `AsyncApp`，可以克隆且具有静态生命周期，
    /// 因此可以跨 `await` 点持有。
    pub fn to_async(&self) -> AsyncApp {
        AsyncApp {
            app: self.this.clone(),
            background_executor: self.background_executor.clone(),
            foreground_executor: self.foreground_executor.clone(),
        }
    }

    /// 获取执行器的引用，可用于生成 future。
    pub fn background_executor(&self) -> &BackgroundExecutor {
        &self.background_executor
    }

    /// 获取执行器的引用，可用于生成 future。
    pub fn foreground_executor(&self) -> &ForegroundExecutor {
        if self.quitting {
            panic!("Can't spawn on main thread after on_app_quit")
        };
        &self.foreground_executor
    }

    /// 在主线程上生成给定函数返回的 future。闭包将使用 [AsyncApp] 调用，
    /// 允许跨 await 点访问应用状态。
    #[track_caller]
    pub fn spawn<AsyncFn, R>(&self, f: AsyncFn) -> Task<R>
    where
        AsyncFn: AsyncFnOnce(&mut AsyncApp) -> R + 'static,
        R: 'static,
    {
        if self.quitting {
            debug_panic!("Can't spawn on main thread after on_app_quit")
        };

        let mut cx = self.to_async();

        self.foreground_executor
            .spawn(async move { f(&mut cx).await }.boxed_local())
    }

    /// 在主线程上以给定优先级生成给定函数返回的 future。
    /// 闭包将使用 [AsyncApp] 调用，允许跨 await 点访问应用状态。
    pub fn spawn_with_priority<AsyncFn, R>(&self, priority: Priority, f: AsyncFn) -> Task<R>
    where
        AsyncFn: AsyncFnOnce(&mut AsyncApp) -> R + 'static,
        R: 'static,
    {
        if self.quitting {
            debug_panic!("Can't spawn on main thread after on_app_quit")
        };

        let mut cx = self.to_async();

        self.foreground_executor
            .spawn_with_priority(priority, async move { f(&mut cx).await }.boxed_local())
    }

    /// 安排给定函数在当前副作用周期结束时运行，允许当前在栈上的实体
    /// 返回到应用。
    pub fn defer(&mut self, f: impl FnOnce(&mut App) + 'static) {
        self.push_effect(Effect::Defer {
            callback: Box::new(f),
        });
    }

    /// 应用资源来源的访问器，在构造 `App` 时提供。
    pub fn asset_source(&self) -> &Arc<dyn AssetSource> {
        &self.asset_source
    }

    /// 文本系统的访问器。
    pub fn text_system(&self) -> &Arc<TextSystem> {
        &self.text_system
    }

    /// 检查是否已分配给定类型的全局变量。
    pub fn has_global<G: Global>(&self) -> bool {
        self.globals_by_type.contains_key(&TypeId::of::<G>())
    }

    /// 访问给定类型的全局变量。如果未分配该类型的全局变量则 panic。
    #[track_caller]
    pub fn global<G: Global>(&self) -> &G {
        self.globals_by_type
            .get(&TypeId::of::<G>())
            .map(|any_state| any_state.downcast_ref::<G>().unwrap())
            .unwrap_or_else(|| panic!("no state of type {} exists", type_name::<G>()))
    }

    /// 如果已分配值，则访问给定类型的全局变量。
    pub fn try_global<G: Global>(&self) -> Option<&G> {
        self.globals_by_type
            .get(&TypeId::of::<G>())
            .map(|any_state| any_state.downcast_ref::<G>().unwrap())
    }

    /// 可变访问给定类型的全局变量。如果未分配该类型的全局变量则 panic。
    #[track_caller]
    pub fn global_mut<G: Global>(&mut self) -> &mut G {
        let global_type = TypeId::of::<G>();
        self.push_effect(Effect::NotifyGlobalObservers { global_type });
        self.globals_by_type
            .get_mut(&global_type)
            .and_then(|any_state| any_state.downcast_mut::<G>())
            .unwrap_or_else(|| panic!("no state of type {} exists", type_name::<G>()))
    }

    /// 可变访问给定类型的全局变量。如果尚未分配该类型的全局变量，则分配默认值。
    pub fn default_global<G: Global + Default>(&mut self) -> &mut G {
        let global_type = TypeId::of::<G>();
        self.push_effect(Effect::NotifyGlobalObservers { global_type });
        self.globals_by_type
            .entry(global_type)
            .or_insert_with(|| Box::<G>::default())
            .downcast_mut::<G>()
            .unwrap()
    }

    /// 设置给定类型全局变量的值。
    pub fn set_global<G: Global>(&mut self, global: G) {
        let global_type = TypeId::of::<G>();
        self.push_effect(Effect::NotifyGlobalObservers { global_type });
        self.globals_by_type.insert(global_type, Box::new(global));
    }

    /// 清除所有存储的全局变量。不通知全局观察者。
    #[cfg(any(test, feature = "test-support"))]
    pub fn clear_globals(&mut self) {
        self.globals_by_type.drain();
    }

    /// 从应用上下文中移除给定类型的全局变量。不通知全局观察者。
    pub fn remove_global<G: Global>(&mut self) -> G {
        let global_type = TypeId::of::<G>();
        self.push_effect(Effect::NotifyGlobalObservers { global_type });
        *self
            .globals_by_type
            .remove(&global_type)
            .unwrap_or_else(|| panic!("no global added for {}", type_name::<G>()))
            .downcast()
            .unwrap()
    }

    /// 注册一个回调，当给定类型的全局变量被更新时调用。
    pub fn observe_global<G: Global>(
        &mut self,
        mut f: impl FnMut(&mut Self) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.global_observers.insert(
            TypeId::of::<G>(),
            Box::new(move |cx| {
                f(cx);
                true
            }),
        );
        self.defer(move |_| activate());
        subscription
    }

    /// 将给定类型的全局变量移动到栈上。
    #[track_caller]
    pub(crate) fn lease_global<G: Global>(&mut self) -> GlobalLease<G> {
        GlobalLease::new(
            self.globals_by_type
                .remove(&TypeId::of::<G>())
                .with_context(|| format!("no global registered of type {}", type_name::<G>()))
                .unwrap(),
        )
    }

    /// 将全局变量移动到栈后恢复该类型的全局变量。
    pub(crate) fn end_global_lease<G: Global>(&mut self, lease: GlobalLease<G>) {
        let global_type = TypeId::of::<G>();

        self.push_effect(Effect::NotifyGlobalObservers { global_type });
        self.globals_by_type.insert(global_type, lease.global);
    }

    pub(crate) fn new_entity_observer(
        &self,
        key: TypeId,
        value: NewEntityListener,
    ) -> Subscription {
        let (subscription, activate) = self.new_entity_observers.insert(key, value);
        activate();
        subscription
    }

    /// 安排在创建指定类型的视图时调用给定函数。
    /// 该函数将接收视图的可变引用和适当的上下文。
    pub fn observe_new<T: 'static>(
        &self,
        on_new: impl 'static + Fn(&mut T, Option<&mut Window>, &mut Context<T>),
    ) -> Subscription {
        self.new_entity_observer(
            TypeId::of::<T>(),
            Box::new(
                move |any_entity: AnyEntity, window: &mut Option<&mut Window>, cx: &mut App| {
                    any_entity
                        .downcast::<T>()
                        .unwrap()
                        .update(cx, |entity_state, cx| {
                            on_new(entity_state, window.as_deref_mut(), cx)
                        })
                },
            ),
        )
    }

    /// 观察实体的释放。回调在实体没有更多强引用后但在丢弃前调用。
    pub fn observe_release<T>(
        &self,
        handle: &Entity<T>,
        on_release: impl FnOnce(&mut T, &mut App) + 'static,
    ) -> Subscription
    where
        T: 'static,
    {
        let (subscription, activate) = self.release_listeners.insert(
            handle.entity_id(),
            Box::new(move |entity, cx| {
                let entity = entity.downcast_mut().expect("invalid entity type");
                on_release(entity, cx)
            }),
        );
        activate();
        subscription
    }

    /// 观察实体的释放。回调在实体没有更多强引用后但在丢弃前调用。
    pub fn observe_release_in<T>(
        &self,
        handle: &Entity<T>,
        window: &Window,
        on_release: impl FnOnce(&mut T, &mut Window, &mut App) + 'static,
    ) -> Subscription
    where
        T: 'static,
    {
        let window_handle = window.handle;
        self.observe_release(handle, move |entity, cx| {
            let _ = window_handle.update(cx, |_, window, cx| on_release(entity, window, cx));
        })
    }

    /// 注册一个回调，当应用在任何窗口中收到按键时调用。
    /// 注意，此回调在所有其他动作和事件机制解析后触发，
    /// 如果事件的传播被停止，则不会调用此 API。
    pub fn observe_keystrokes(
        &mut self,
        mut f: impl FnMut(&KeystrokeEvent, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        fn inner(
            keystroke_observers: &SubscriberSet<(), KeystrokeObserver>,
            handler: KeystrokeObserver,
        ) -> Subscription {
            let (subscription, activate) = keystroke_observers.insert((), handler);
            activate();
            subscription
        }

        inner(
            &self.keystroke_observers,
            Box::new(move |event, window, cx| {
                f(event, window, cx);
                true
            }),
        )
    }

    /// 注册一个回调，当应用在任何窗口中收到按键时调用。
    /// 注意，此回调在所有其他动作和事件机制解析*之前*触发，
    /// 与 [`App::observe_keystrokes`] 在之后触发不同。
    /// 这意味着拦截器中的 `cx.stop_propagation` 调用将阻止动作分发。
    pub fn intercept_keystrokes(
        &mut self,
        mut f: impl FnMut(&KeystrokeEvent, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        fn inner(
            keystroke_interceptors: &SubscriberSet<(), KeystrokeObserver>,
            handler: KeystrokeObserver,
        ) -> Subscription {
            let (subscription, activate) = keystroke_interceptors.insert((), handler);
            activate();
            subscription
        }

        inner(
            &self.keystroke_interceptors,
            Box::new(move |event, window, cx| {
                f(event, window, cx);
                true
            }),
        )
    }

    /// 注册键绑定。
    pub fn bind_keys(&mut self, bindings: impl IntoIterator<Item = KeyBinding>) {
        self.keymap.borrow_mut().add_bindings(bindings);
        self.pending_effects.push_back(Effect::RefreshWindows);
    }

    /// 清除应用中所有键绑定。
    pub fn clear_key_bindings(&mut self) {
        self.keymap.borrow_mut().clear();
        self.pending_effects.push_back(Effect::RefreshWindows);
    }

    /// 获取应用中所有键绑定。
    pub fn key_bindings(&self) -> Rc<RefCell<Keymap>> {
        self.keymap.clone()
    }

    /// 注册通过键盘调用动作的全局处理程序。这些处理程序在动作的
    /// 冒泡阶段结束时运行，因此仅在没有其他处理程序或它们调用了
    /// `cx.propagate()` 时才会被调用。
    pub fn on_action<A: Action>(
        &mut self,
        listener: impl Fn(&A, &mut Self) + 'static,
    ) -> &mut Self {
        self.global_action_listeners
            .entry(TypeId::of::<A>())
            .or_default()
            .push(Rc::new(move |action, phase, cx| {
                if phase == DispatchPhase::Bubble {
                    let action = action.downcast_ref().unwrap();
                    listener(action, cx)
                }
            }));
        self
    }

    /// 事件处理程序默认传播事件。调用此方法可停止向 z-index 较低（鼠标）
    /// 或树中较高（键盘）的事件处理程序分发。这与 [`Self::propagate`] 相反。
    /// 也可以在副作用刷新前调用此方法来取消 [`Self::propagate`] 调用。
    pub fn stop_propagation(&mut self) {
        self.propagate_event = false;
    }

    /// 动作处理程序在动作分发的冒泡阶段默认停止传播，
    /// 不向元素树中较高的动作处理程序分发。这与
    /// [`Self::stop_propagation`] 相反。也可以在副作用刷新前
    /// 调用此方法来取消 [`Self::stop_propagation`] 调用。
    pub fn propagate(&mut self) {
        self.propagate_event = true;
    }

    /// 从一些任意数据构建动作，通常是键映射条目。
    pub fn build_action(
        &self,
        name: &str,
        data: Option<serde_json::Value>,
    ) -> std::result::Result<Box<dyn Action>, ActionBuildError> {
        self.actions.build_action(name, data)
    }

    /// 获取所有已注册的动作名称。注意，注册仅允许动态构建动作，
    /// 与在元素树中绑定动作无关。
    pub fn all_action_names(&self) -> &[&'static str] {
        self.actions.all_action_names()
    }

    /// 返回在当前焦点元素上调用给定动作的键绑定，不检查上下文。
    /// 绑定按添加顺序返回。显示时，最后一个绑定应优先。
    pub fn all_bindings_for_input(&self, input: &[Keystroke]) -> Vec<KeyBinding> {
        RefCell::borrow(&self.keymap).all_bindings_for_input(input)
    }

    /// 获取所有已注册的非内部动作及其 schema。
    pub fn action_schemas(
        &self,
        generator: &mut schemars::SchemaGenerator,
    ) -> Vec<(&'static str, Option<schemars::Schema>)> {
        self.actions.action_schemas(generator)
    }

    /// 按名称获取特定动作的 schema。
    /// 如果未找到动作则返回 `None`。
    /// 如果动作存在但没有 schema 则返回 `Some(None)`。
    /// 如果动作存在且有 schema 则返回 `Some(Some(schema))`。
    pub fn action_schema_by_name(
        &self,
        name: &str,
        generator: &mut schemars::SchemaGenerator,
    ) -> Option<Option<schemars::Schema>> {
        self.actions.action_schema_by_name(name, generator)
    }

    /// 获取从已弃用动作名称到规范名称的映射。
    pub fn deprecated_actions_to_preferred_actions(&self) -> &HashMap<&'static str, &'static str> {
        self.actions.deprecated_aliases()
    }

    /// 获取从动作名称到弃用消息的映射。
    pub fn action_deprecation_messages(&self) -> &HashMap<&'static str, &'static str> {
        self.actions.deprecation_messages()
    }

    /// 获取从动作名称到文档的映射。
    pub fn action_documentation(&self) -> &HashMap<&'static str, &'static str> {
        self.actions.documentation()
    }

    /// 注册一个回调，当应用即将退出时调用。
    /// 此时无法取消退出事件。
    pub fn on_app_quit<Fut>(
        &self,
        mut on_quit: impl FnMut(&mut App) -> Fut + 'static,
    ) -> Subscription
    where
        Fut: 'static + Future<Output = ()>,
    {
        let (subscription, activate) = self.quit_observers.insert(
            (),
            Box::new(move |cx| {
                let future = on_quit(cx);
                future.boxed_local()
            }),
        );
        activate();
        subscription
    }

    /// 注册一个回调，当应用即将重启时调用。
    ///
    /// 这些回调在任何 `on_app_quit` 回调之前调用。
    pub fn on_app_restart(&self, mut on_restart: impl 'static + FnMut(&mut App)) -> Subscription {
        let (subscription, activate) = self.restart_observers.insert(
            (),
            Box::new(move |cx| {
                on_restart(cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// 注册一个回调，当窗口关闭时调用。
    /// 在调用此回调时，窗口不再可访问。
    pub fn on_window_closed(
        &self,
        mut on_closed: impl FnMut(&mut App, WindowId) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.window_closed_observers.insert((), Box::new(on_closed));
        activate();
        subscription
    }

    pub(crate) fn clear_pending_keystrokes(&mut self) {
        for window in self.windows() {
            window
                .update(self, |_, window, cx| {
                    if window.pending_input_keystrokes().is_some() {
                        window.clear_pending_keystrokes();
                        window.pending_input_changed(cx);
                    }
                })
                .ok();
        }
    }

    /// 检查给定动作是否在当前上下文中被绑定，由应用的当前焦点、
    /// 元素树中的绑定和任何全局动作监听器定义。
    pub fn is_action_available(&mut self, action: &dyn Action) -> bool {
        let mut action_available = false;
        if let Some(window) = self.active_window()
            && let Ok(window_action_available) =
                window.update(self, |_, window, cx| window.is_action_available(action, cx))
        {
            action_available = window_action_available;
        }

        action_available
            || self
                .global_action_listeners
                .contains_key(&action.as_any().type_id())
    }

    /// 设置此应用的菜单栏。这将替换任何现有的菜单栏。
    pub fn set_menus(&self, menus: impl IntoIterator<Item = Menu>) {
        let menus: Vec<Menu> = menus.into_iter().collect();
        self.platform.set_menus(menus, &self.keymap.borrow());
    }

    /// 获取此应用的菜单栏。
    pub fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        self.platform.get_menus()
    }

    /// 设置 Dock 中应用图标的右键菜单
    pub fn set_dock_menu(&self, menus: Vec<MenuItem>) {
        self.platform.set_dock_menu(menus, &self.keymap.borrow())
    }

    /// 执行与给定 Dock 菜单项关联的动作，目前仅在 Windows 上使用。
    pub fn perform_dock_menu_action(&self, action: usize) {
        self.platform.perform_dock_menu_action(action);
    }

    /// 将给定路径添加到应用最近路径列表的底部。
    /// 该列表通常显示在 Dock 中应用图标的上下文菜单中，
    /// 允许通过该上下文菜单打开最近的文件。
    /// 如果路径已在列表中，它将被移动到列表底部。
    pub fn add_recent_document(&self, path: &Path) {
        self.platform.add_recent_document(path);
    }

    /// 使用更新的最近路径列表更新跳转列表，目前仅在 Windows 上使用。
    /// 注意，这也会在 Windows 上设置 Dock 菜单。
    pub fn update_jump_list(
        &self,
        menus: Vec<MenuItem>,
        entries: Vec<SmallVec<[PathBuf; 2]>>,
    ) -> Task<Vec<SmallVec<[PathBuf; 2]>>> {
        self.platform.update_jump_list(menus, entries)
    }

    /// 设置系统托盘图标和菜单（旧 API，向后兼容）
    pub fn set_tray(&self, tray: Tray, menus: Option<Vec<MenuItem>>) {
        self.platform.set_tray(tray, menus, &self.keymap.borrow())
    }

    /// 设置系统托盘图标
    pub fn set_tray_icon(&self, icon: Option<&[u8]>) {
        self.platform.set_tray_icon(icon);
    }

    /// 设置系统托盘菜单项
    pub fn set_tray_menu(&self, menu: Vec<TrayMenuItem>) {
        self.platform.set_tray_menu(menu);
    }

    /// 设置系统托盘工具提示
    pub fn set_tray_tooltip(&self, tooltip: &str) {
        self.platform.set_tray_tooltip(tooltip);
    }

    /// 启用或禁用托盘面板模式
    /// 启用时，点击托盘图标会触发 `TrayIconEvent::LeftClick` 而不是显示菜单
    pub fn set_tray_panel_mode(&self, enabled: bool) {
        self.platform.set_tray_panel_mode(enabled);
    }

    /// 在操作系统中显示通知
    pub fn show_notification(&self, title: &str, body: &str) -> Result<()> {
        self.platform.show_notification(title, body)
    }

    /// 推送通知（便捷方法，触发操作系统级通知）。
    ///
    /// 等价于 `show_notification`，提供更具语义化的命名。
    pub fn push_notification(&self, title: &str, message: &str) -> Result<()> {
        self.show_notification(title, message)
    }

    /// 获取托盘图标的屏幕边界坐标，用于在其下方定位窗口
    pub fn tray_icon_bounds(&self) -> Option<Bounds<Pixels>> {
        self.platform.get_tray_icon_bounds()
    }

    /// 注册系统托盘图标事件的回调函数
    pub fn on_tray_icon_event(&self, mut callback: impl FnMut(TrayIconEvent, &mut App) + 'static) {
        let this = self.this.clone();
        self.platform.on_tray_icon_event(Box::new(move |event| {
            if let Some(app) = this.upgrade() {
                callback(event, &mut app.borrow_mut());
            }
        }));
    }

    /// 注册托盘菜单项点击事件的回调函数
    pub fn on_tray_menu_action(&self, mut callback: impl FnMut(SharedString, &mut App) + 'static) {
        let this = self.this.clone();
        self.platform.on_tray_menu_action(Box::new(move |id| {
            if let Some(app) = this.upgrade() {
                callback(id, &mut app.borrow_mut());
            }
        }));
    }

    /// 设置应用程序是否应在没有窗口时保持运行
    pub fn set_keep_alive_without_windows(&self, keep_alive: bool) {
        self.platform.set_keep_alive_without_windows(keep_alive);
    }

    /// 最小化到托盘 —— 隐藏所有窗口（从任务栏移除）。
    ///
    /// 常用于点击关闭按钮时将应用最小化到系统托盘而非退出。
    pub fn minimize_to_tray(&mut self) {
        let windows: Vec<AnyWindowHandle> = self.windows();
        for window in windows {
            self.update_window(window, |_view, window, _cx| {
                window.hide_window();
            })
            .ok();
        }
    }

    /// 显示所有窗口（将所有窗口带到前台）。
    pub fn show_all_windows(&mut self) {
        let windows: Vec<AnyWindowHandle> = self.windows();
        for window in windows {
            self.update_window(window, |_view, window, _cx| {
                window.activate_window();
            })
            .ok();
        }
    }

    /// 隐藏所有窗口（从任务栏和屏幕移除）。
    pub fn hide_all_windows(&mut self) {
        let windows: Vec<AnyWindowHandle> = self.windows();
        for window in windows {
            self.update_window(window, |_view, window, _cx| {
                window.hide_window();
            })
            .ok();
        }
    }

    /// 最小化所有窗口。
    pub fn minimize_all_windows(&mut self) {
        let windows: Vec<AnyWindowHandle> = self.windows();
        for window in windows {
            self.update_window(window, |_view, window, _cx| {
                window.minimize_window();
            })
            .ok();
        }
    }

    /// 注册全局快捷键
    ///
    /// # 参数
    /// * `id` - 快捷键的唯一标识符
    /// * `keystroke` - 快捷键组合（如 "cmd-shift-k"）
    ///
    /// # 返回
    /// 成功时返回 `Ok(())`，失败时返回错误
    pub fn register_global_hotkey(&self, id: u32, keystroke: &Keystroke) -> Result<()> {
        self.platform.register_global_hotkey(id, keystroke)
    }

    /// 注销全局快捷键
    ///
    /// # 参数
    /// * `id` - 要注销的快捷键 ID
    pub fn unregister_global_hotkey(&self, id: u32) {
        self.platform.unregister_global_hotkey(id);
    }

    /// 注册全局快捷键事件的回调函数
    pub fn on_global_hotkey(&self, mut callback: impl FnMut(u32, &mut App) + 'static) {
        let this = self.this.clone();
        self.platform.on_global_hotkey(Box::new(move |id| {
            if let Some(app) = this.upgrade() {
                callback(id, &mut app.borrow_mut());
            }
        }));
    }

    /// 将动作分发到当前活动窗口或全局动作处理程序
    /// 参见 [`crate::Action`] 了解动作如何工作的更多信息
    pub fn dispatch_action(&mut self, action: &dyn Action) {
        if let Some(active_window) = self.active_window() {
            active_window
                .update(self, |_, window, cx| {
                    window.dispatch_action(action.boxed_clone(), cx)
                })
                .log_err();
        } else {
            self.dispatch_global_action(action);
        }
    }

    fn dispatch_global_action(&mut self, action: &dyn Action) {
        self.propagate_event = true;

        if let Some(mut global_listeners) = self
            .global_action_listeners
            .remove(&action.as_any().type_id())
        {
            for listener in &global_listeners {
                listener(action.as_any(), DispatchPhase::Capture, self);
                if !self.propagate_event {
                    break;
                }
            }

            global_listeners.extend(
                self.global_action_listeners
                    .remove(&action.as_any().type_id())
                    .unwrap_or_default(),
            );

            self.global_action_listeners
                .insert(action.as_any().type_id(), global_listeners);
        }

        if self.propagate_event
            && let Some(mut global_listeners) = self
                .global_action_listeners
                .remove(&action.as_any().type_id())
        {
            for listener in global_listeners.iter().rev() {
                listener(action.as_any(), DispatchPhase::Bubble, self);
                if !self.propagate_event {
                    break;
                }
            }

            global_listeners.extend(
                self.global_action_listeners
                    .remove(&action.as_any().type_id())
                    .unwrap_or_default(),
            );

            self.global_action_listeners
                .insert(action.as_any().type_id(), global_listeners);
        }
    }

    /// 当前是否有正在拖动的内容？
    pub fn has_active_drag(&self) -> bool {
        self.active_drag.is_some()
    }

    /// 获取当前活动拖动操作的光标样式。
    pub fn active_drag_cursor_style(&self) -> Option<CursorStyle> {
        self.active_drag.as_ref().and_then(|drag| drag.cursor_style)
    }

    /// 停止活动拖动并清除任何相关副作用。
    pub fn stop_active_drag(&mut self, window: &mut Window) -> bool {
        if self.active_drag.is_some() {
            self.active_drag = None;
            window.refresh();
            true
        } else {
            false
        }
    }

    /// 获取活动拖动的值（如果有的话）（用于接收文件拖放）。
    pub fn take_active_drag_value(&mut self) -> Option<Arc<dyn std::any::Any>> {
        self.active_drag.take().map(|drag| drag.value)
    }

    /// 设置当前活动拖动操作的光标样式。
    pub fn set_active_drag_cursor_style(
        &mut self,
        cursor_style: CursorStyle,
        window: &mut Window,
    ) -> bool {
        if let Some(ref mut drag) = self.active_drag {
            drag.cursor_style = Some(cursor_style);
            window.refresh();
            true
        } else {
            false
        }
    }

    /// 设置 RGPUI 的提示渲染器。这将用此自定义实现替换默认或平台特定的提示。
    pub fn set_prompt_builder(
        &mut self,
        renderer: impl Fn(
            PromptLevel,
            &str,
            Option<&str>,
            &[PromptButton],
            PromptHandle,
            &mut Window,
            &mut App,
        ) -> RenderablePromptHandle
        + 'static,
    ) {
        self.prompt_builder = Some(PromptBuilder::Custom(Box::new(renderer)));
    }

    /// 将提示构建器重置为默认实现。
    pub fn reset_prompt_builder(&mut self) {
        self.prompt_builder = Some(PromptBuilder::Default);
    }

    /// 从 RGPUI 缓存中移除资源
    pub fn remove_asset<A: Asset>(&mut self, source: &A::Source) {
        let asset_id = (TypeId::of::<A>(), hash(source));
        self.loading_assets.remove(&asset_id);
    }

    /// 异步加载资源，如果资源尚未完成加载则返回 None。
    ///
    /// 注意，多次调用此方法每次只会产生一次 `Asset::load` 调用，
    /// 且该调用的结果将被缓存。
    pub fn fetch_asset<A: Asset>(&mut self, source: &A::Source) -> (Shared<Task<A::Output>>, bool) {
        let asset_id = (TypeId::of::<A>(), hash(source));
        let mut is_first = false;
        let task = self
            .loading_assets
            .remove(&asset_id)
            .map(|boxed_task| *boxed_task.downcast::<Shared<Task<A::Output>>>().unwrap())
            .unwrap_or_else(|| {
                is_first = true;
                let future = A::load(source.clone(), self);

                self.background_executor().spawn(future).shared()
            });

        self.loading_assets.insert(asset_id, Box::new(task.clone()));

        (task, is_first)
    }

    /// 获取一个新的 [`FocusHandle`]，允许你跟踪和操作
    /// 此窗口中渲染的元素的键盘焦点。
    #[track_caller]
    pub fn focus_handle(&self) -> FocusHandle {
        FocusHandle::new(&self.focus_handles)
    }

    /// 告诉 RGPUI 实体已更改，应通知其观察者。
    pub fn notify(&mut self, entity_id: EntityId) {
        let window_invalidators = mem::take(
            self.window_invalidators_by_entity
                .entry(entity_id)
                .or_default(),
        );

        // `window_invalidators_by_entity` is monotonic, so an entry alone
        // doesn't mean the window is currently rendering the entity. Filter
        // through `tracked_entities` to keep invalidation tight to windows
        // that actually display this entity right now.
        let live_invalidators: SmallVec<[WindowInvalidator; 2]> = window_invalidators
            .iter()
            .filter(|(window_id, _)| {
                self.tracked_entities
                    .get(window_id)
                    .is_some_and(|set| set.contains(&entity_id))
            })
            .map(|(_, invalidator)| invalidator.clone())
            .collect();

        if live_invalidators.is_empty() {
            if self.pending_notifications.insert(entity_id) {
                self.pending_effects
                    .push_back(Effect::Notify { emitter: entity_id });
            }
        } else {
            for invalidator in &live_invalidators {
                invalidator.invalidate_view(entity_id, self);
            }
        }

        self.window_invalidators_by_entity
            .insert(entity_id, window_invalidators);
    }

    /// 返回此 [`App`] 的名称。
    #[cfg(any(test, feature = "test-support", debug_assertions))]
    pub fn get_name(&self) -> Option<&'static str> {
        self.name
    }

    /// 如果平台文件选择器支持选择文件和目录的混合，则返回 `true`。
    pub fn can_select_mixed_files_and_dirs(&self) -> bool {
        self.platform.can_select_mixed_files_and_dirs()
    }

    /// 从所有窗口的精灵图集中移除图像。
    ///
    /// 如果当前窗口正在更新，它将从 `App.windows` 中移除，你可以使用 `current_window` 指定当前窗口。
    /// 如果图像不在精灵图集中，此操作无效。
    pub fn drop_image(&mut self, image: Arc<RenderImage>, current_window: Option<&mut Window>) {
        // remove the texture from all other windows
        for window in self.windows.values_mut().flatten() {
            _ = window.drop_image(image.clone());
        }

        // remove the texture from the current window
        if let Some(window) = current_window {
            _ = window.drop_image(image);
        }
    }

    /// 设置检查器的渲染器。
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub fn set_inspector_renderer(&mut self, f: crate::InspectorRenderer) {
        self.inspector_renderer = Some(f);
    }

    /// 注册特定于检查器状态的渲染器。
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub fn register_inspector_element<T: 'static, R: crate::IntoElement>(
        &mut self,
        f: impl 'static + Fn(crate::InspectorElementId, &T, &mut Window, &mut App) -> R,
    ) {
        self.inspector_element_registry.register(f);
    }

    /// 初始化应用的 rgpui 默认颜色。
    ///
    /// 这些颜色可以通过 `cx.default_colors()` 访问。
    pub fn init_colors(&mut self) {
        self.set_global(GlobalColors(Arc::new(Colors::default())));
    }
}

impl AppContext for App {
    /// 构建由应用拥有的实体。
    ///
    /// 给定函数将使用 [`Context`] 调用，必须返回表示实体的对象。
    /// 将返回 [`Entity`] 句柄，可用于在上下文中访问实体。
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T> {
        self.update(|cx| {
            let slot = cx.entities.reserve();
            let handle = slot.clone();
            let entity = build_entity(&mut Context::new_context(cx, slot.downgrade()));

            cx.push_effect(Effect::EntityCreated {
                entity: handle.into_any(),
                tid: TypeId::of::<T>(),
                window: cx.window_update_stack.last().cloned(),
            });

            cx.entities.insert(slot, entity)
        })
    }

    fn reserve_entity<T: 'static>(&mut self) -> Reservation<T> {
        Reservation(self.entities.reserve())
    }

    fn insert_entity<T: 'static>(
        &mut self,
        reservation: Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T> {
        self.update(|cx| {
            let slot = reservation.0;
            let entity = build_entity(&mut Context::new_context(cx, slot.downgrade()));
            cx.entities.insert(slot, entity)
        })
    }

    /// 更新给定句柄引用的实体。函数接收实体的可变引用和实体的 `Context`。
    fn update_entity<T: 'static, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        self.update(|cx| {
            let mut entity = cx.entities.lease(handle);
            let result = update(
                &mut entity,
                &mut Context::new_context(cx, handle.downgrade()),
            );
            cx.entities.end_lease(entity);
            result
        })
    }

    fn as_mut<'a, T>(&'a mut self, handle: &Entity<T>) -> GpuiBorrow<'a, T>
    where
        T: 'static,
    {
        GpuiBorrow::new(handle.clone(), self)
    }

    fn read_entity<T, R>(&self, handle: &Entity<T>, read: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static,
    {
        let entity = self.entities.read(handle);
        read(entity, self)
    }

    fn update_window<T, F>(&mut self, handle: AnyWindowHandle, update: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        self.update_window_id(handle.id, update)
    }

    fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R> {
        App::with_window(self, entity_id, f)
    }

    fn read_window<T, R>(
        &self,
        window: &WindowHandle<T>,
        read: impl FnOnce(Entity<T>, &App) -> R,
    ) -> Result<R>
    where
        T: 'static,
    {
        let window = self
            .windows
            .get(window.id)
            .context("window not found")?
            .as_deref()
            .expect("attempted to read a window that is already on the stack");

        let root_view = window.root.clone().unwrap();
        let view = Root::root_view_downcast::<T>(root_view, self)
            .map_err(|_| anyhow!("root view's type has changed"))?;

        Ok(read(view, self))
    }

    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static,
    {
        self.background_executor.spawn(future)
    }

    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> R
    where
        G: Global,
    {
        let mut g = self.global::<G>();
        callback(g, self)
    }
}

/// 这些副作用在每个应用更新周期结束时处理。
pub(crate) enum Effect {
    Notify {
        emitter: EntityId,
    },
    Emit {
        emitter: EntityId,
        event_type: TypeId,
        event: ArenaBox<dyn Any>,
    },
    RefreshWindows,
    NotifyGlobalObservers {
        global_type: TypeId,
    },
    Defer {
        callback: Box<dyn FnOnce(&mut App) + 'static>,
    },
    EntityCreated {
        entity: AnyEntity,
        tid: TypeId,
        window: Option<WindowId>,
    },
}

impl std::fmt::Debug for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Effect::Notify { emitter } => write!(f, "Notify({})", emitter),
            Effect::Emit { emitter, .. } => write!(f, "Emit({:?})", emitter),
            Effect::RefreshWindows => write!(f, "RefreshWindows"),
            Effect::NotifyGlobalObservers { global_type } => {
                write!(f, "NotifyGlobalObservers({:?})", global_type)
            }
            Effect::Defer { .. } => write!(f, "Defer(..)"),
            Effect::EntityCreated { entity, .. } => write!(f, "EntityCreated({:?})", entity),
        }
    }
}

/// 在 `update_global` 期间包装全局变量值，当值已移动到栈上时。
pub(crate) struct GlobalLease<G: Global> {
    global: Box<dyn Any>,
    global_type: PhantomData<G>,
}

impl<G: Global> GlobalLease<G> {
    fn new(global: Box<dyn Any>) -> Self {
        GlobalLease {
            global,
            global_type: PhantomData,
        }
    }
}

impl<G: Global> Deref for GlobalLease<G> {
    type Target = G;

    fn deref(&self) -> &Self::Target {
        self.global.downcast_ref().unwrap()
    }
}

impl<G: Global> DerefMut for GlobalLease<G> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.global.downcast_mut().unwrap()
    }
}

/// 包含与活动拖动操作关联的状态，通过在窗口中拖动元素
/// 或从底层平台拖入应用来启动。
pub struct AnyDrag {
    /// 用于渲染此拖动的视图
    pub view: AnyView,

    /// 被拖动项的值，将被拖放
    pub value: Arc<dyn Any>,

    /// 用于在发起拖动的原始元素的同一位置渲染被拖动项
    pub cursor_offset: Point<Pixels>,

    /// 拖动时使用的光标样式
    pub cursor_style: Option<CursorStyle>,
}

/// 包含与工具提示关联的状态。仅当在自定义元素上实现工具提示行为时才需要此结构体。
/// 否则，请使用 [Div::tooltip](crate::Interactivity::tooltip)。
#[derive(Clone)]
pub struct AnyTooltip {
    /// 用于显示工具提示的视图
    pub view: AnyView,

    /// 工具提示展开时鼠标的绝对位置。
    pub mouse_position: Point<Pixels>,

    /// 根据工具提示的边界检查工具提示是否仍应可见，并相应地更新其状态。
    /// 这需要在悬停元素的鼠标移动处理程序之上，以处理元素未被绘制的情况
    /// （例如通过使用 `visible_on_hover`）。
    pub check_visible_and_update: Rc<dyn Fn(Bounds<Pixels>, &mut Window, &mut App) -> bool>,
}

/// 按键事件，以及可能关联的动作
#[derive(Debug)]
pub struct KeystrokeEvent {
    /// 发生的按键
    pub keystroke: Keystroke,

    /// 为按键解析出的动作（如果有）
    pub action: Option<Box<dyn Action>>,

    /// 事件发生时的上下文栈
    pub context_stack: Vec<KeyContext>,
}

struct NullHttpClient;

impl HttpClient for NullHttpClient {
    fn send(
        &self,
        _req: crate::http_client::Request<crate::http_client::AsyncBody>,
    ) -> futures::future::BoxFuture<
        'static,
        anyhow::Result<crate::http_client::Response<crate::http_client::AsyncBody>>,
    > {
        async move {
            anyhow::bail!("No HttpClient available");
        }
        .boxed()
    }

    fn user_agent(&self) -> Option<&crate::http_client::http::HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }
}

/// 对 RGPUI 拥有的实体的可变引用
pub struct GpuiBorrow<'a, T> {
    inner: Option<Lease<T>>,
    app: &'a mut App,
}

impl<'a, T: 'static> GpuiBorrow<'a, T> {
    fn new(inner: Entity<T>, app: &'a mut App) -> Self {
        app.start_update();
        let lease = app.entities.lease(&inner);
        Self {
            inner: Some(lease),
            app,
        }
    }
}

impl<'a, T: 'static> std::borrow::Borrow<T> for GpuiBorrow<'a, T> {
    fn borrow(&self) -> &T {
        self.inner.as_ref().unwrap().borrow()
    }
}

impl<'a, T: 'static> std::borrow::BorrowMut<T> for GpuiBorrow<'a, T> {
    fn borrow_mut(&mut self) -> &mut T {
        self.inner.as_mut().unwrap().borrow_mut()
    }
}

impl<'a, T: 'static> std::ops::Deref for GpuiBorrow<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().unwrap()
    }
}

impl<'a, T: 'static> std::ops::DerefMut for GpuiBorrow<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.inner.as_mut().unwrap()
    }
}

impl<'a, T> Drop for GpuiBorrow<'a, T> {
    fn drop(&mut self) {
        let lease = self.inner.take().unwrap();
        self.app.notify(lease.id);
        self.app.entities.end_lease(lease);
        self.app.finish_update();
    }
}

#[cfg(test)]
mod test {
    use std::{cell::RefCell, rc::Rc};

    use crate::{AppContext, TestAppContext};

    #[test]
    fn test_gpui_borrow() {
        let cx = TestAppContext::single();
        let observation_count = Rc::new(RefCell::new(0));

        let state = cx.update(|cx| {
            let state = cx.new(|_| false);
            cx.observe(&state, {
                let observation_count = observation_count.clone();
                move |_, _| {
                    let mut count = observation_count.borrow_mut();
                    *count += 1;
                }
            })
            .detach();

            state
        });

        cx.update(|cx| {
            // Calling this like this so that we don't clobber the borrow_mut above
            *std::borrow::BorrowMut::borrow_mut(&mut state.as_mut(cx)) = true;
        });

        cx.update(|cx| {
            state.write(cx, false);
        });

        assert_eq!(*observation_count.borrow(), 2);
    }
}
