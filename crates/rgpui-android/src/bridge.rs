//! Android 桥：`android-activity` 入口、主循环、JNI 小件。
//!
//! 线程模型：`android-activity` 起一根 native 线程调 `android_main`，
//! GPUI 的 draw/event 全跑这根线程；`AndroidApp` 句柄 `Send + Sync` 可共享。
//! 事件循环阻塞在 looper 上：生命周期命令、输入、主任务、vsync 回调
//! 各走各的 fd，谁到谁叫醒；延后任务按 `next_delayed_due` 定超时。
//!
//! M2 走 `android-activity` 单路径（`NativeActivity`，零 Java 代码）。
//! 自定义 Activity（`InputConnection` 组合串）记 M3，见开发文档。
//!
//! 本模块仅 Android 编译（`#[cfg(target_os = "android")]` 门控）。

#![allow(unsafe_code)]

use android_activity::{AndroidApp, MainEvent, PollEvent};
use jni::JavaVM;
use jni::objects::{JObject, JString, JValue};
use parking_lot::RwLock;
use rgpui::{Platform, WindowAppearance};
use std::{
    ffi::c_void,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use super::frame_source;
use super::platform::{AndroidPlatform, SharedPlatform};
use super::window::{AndroidWindow, LocalAppearance};
use super::{AndroidKeyEvent, TouchPoint};

// ── 延迟生命周期标记 ─────────────────────────────────────────────────────────
// `poll_events` 回调里只记标记、不干活：Java 侧回调在 condvar 上等 native
// 线程处理完命令，此时去拿窗口锁（渲染线程可能正抱着画）必死锁。
// 主循环每轮在 `poll_events` 返回后逐个消化。

/// surface 销毁待处理。
static TERM_WINDOW_PENDING: AtomicBool = AtomicBool::new(false);
/// surface 新建待处理。
static INIT_WINDOW_PENDING: AtomicBool = AtomicBool::new(false);
/// 尺寸变化待处理。
static WINDOW_RESIZED_PENDING: AtomicBool = AtomicBool::new(false);
/// 配置变化待处理（键盘布局/深浅色）。
static CONFIG_CHANGED_PENDING: AtomicBool = AtomicBool::new(false);
/// 进后台待处理。
static PAUSE_PENDING: AtomicBool = AtomicBool::new(false);
/// 回前台待处理。
static RESUME_PENDING: AtomicBool = AtomicBool::new(false);

/// 首帧已渲染（Java 闪屏/宿主可据此撤启动页）。
pub static NATIVE_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// `finish_launching` 是否已调（surface 重建后重跑一次初始化回调）。
static INIT_WINDOW_DONE: AtomicBool = AtomicBool::new(false);

// ── 全局状态 ─────────────────────────────────────────────────────────────────

/// `android_main` 存的 `AndroidApp`（每次 Activity 重建都更新，只读用）。
static ANDROID_APP: RwLock<Option<AndroidApp>> = RwLock::new(None);

/// 进程级 `AndroidPlatform`（每次 Activity 重建都换新，旧实例释放时注销 looper）。
static PLATFORM: RwLock<Option<Arc<AndroidPlatform>>> = RwLock::new(None);

/// JVM 包装（`JavaVM::from_raw` 包一次，多线程复用）。
static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();

/// 取已存的 `AndroidApp`（初始化前为空）。
pub fn android_app() -> Option<AndroidApp> {
    ANDROID_APP.read().clone()
}

/// 取全局平台（初始化前为空；返回克隆，调用方持有期间旧平台不释放）。
pub fn platform() -> Option<Arc<AndroidPlatform>> {
    locked_platform()
}

/// 取当前全局平台（内部用，读锁下克隆）。
fn locked_platform() -> Option<Arc<AndroidPlatform>> {
    PLATFORM.read().clone()
}

/// 取 `Application::with_platform` 要的共享平台包装。
pub fn shared_platform() -> Option<SharedPlatform> {
    locked_platform().map(|platform| SharedPlatform::new(platform))
}

/// JVM 裸指针（`platform.rs` 的 JNI 调用用）。
pub fn java_vm() -> *mut c_void {
    ANDROID_APP
        .read()
        .as_ref()
        .map(|app| app.vm_as_ptr())
        .unwrap_or(std::ptr::null_mut())
}

/// Activity 的 JNI 全局引用（`android-activity` 保证进程期有效）。
pub fn activity_as_ptr() -> *mut c_void {
    ANDROID_APP
        .read()
        .as_ref()
        .map(|app| app.activity_as_ptr())
        .unwrap_or(std::ptr::null_mut())
}

// ── JNI 小件（`jni 0.22` 安全包装） ───────────────────────────────────────────

/// 取/建静态 `JavaVM` 包装。
fn java_vm_safe() -> Result<&'static JavaVM, String> {
    if let Some(vm) = JAVA_VM.get() {
        return Ok(vm);
    }
    let ptr = java_vm();
    if ptr.is_null() {
        return Err("JavaVM 不可用".into());
    }
    Ok(JAVA_VM.get_or_init(|| unsafe { JavaVM::from_raw(ptr as *mut jni::sys::JavaVM) }))
}

/// 在当前线程附着 JVM 跑闭包（未附着则自动 detach）。
pub fn with_env<T>(task: impl FnOnce(&mut jni::Env) -> Result<T, String>) -> Result<T, String> {
    let vm = java_vm_safe()?;
    let mut result: Option<Result<T, String>> = None;
    vm.attach_current_thread(|env: &mut jni::Env| -> Result<(), jni::errors::Error> {
        result = Some(task(env));
        Ok(())
    })
    .map_err(|error: jni::errors::Error| error.to_string())?;
    result.unwrap_or_else(|| Err("JNI 闭包无返回".into()))
}

/// 取 Activity 的 [`JObject`]（`jni 0.22` 的 `from_raw` 绑局部帧生命周期）。
pub fn activity<'local>(env: &jni::Env<'local>) -> Result<JObject<'local>, String> {
    let ptr = activity_as_ptr();
    if ptr.is_null() {
        return Err("Activity 不可用".into());
    }
    Ok(unsafe { JObject::from_raw(env, ptr as jni::sys::jobject) })
}

/// Java `String` 转 Rust `String`（空/错给空串）。
pub fn get_string(env: &mut jni::Env<'_>, obj: &JObject<'_>) -> String {
    if obj.is_null() {
        return String::new();
    }
    let raw = unsafe { JString::from_raw(env, obj.as_raw()) };
    raw.to_string()
}

/// 经 JNI 取按键的 unicode（`KeyEvent.getUnicodeChar`，失败给 0）。
pub fn unicode_char_for_key_event(key_code: i32, action: i32, meta_state: i32) -> u32 {
    with_env(|env| {
        let key_event = match env.new_object(
            jni::jni_str!("android/view/KeyEvent"),
            jni::jni_sig!("(II)V"),
            &[JValue::Int(action), JValue::Int(key_code)],
        ) {
            Ok(object) => object,
            Err(_) => {
                env.exception_clear();
                return Ok(0);
            }
        };
        match env.call_method(
            &key_event,
            jni::jni_str!("getUnicodeChar"),
            jni::jni_sig!("(I)I"),
            &[JValue::Int(meta_state)],
        ) {
            Ok(value) => {
                let symbol = value.i().unwrap_or(0);
                Ok(if symbol > 0 { symbol as u32 } else { 0 })
            }
            Err(_) => {
                env.exception_clear();
                Ok(0)
            }
        }
    })
    .unwrap_or(0)
}

/// 经 NDK `Configuration` 查深浅色（`uiMode_night == Yes` 即深色）。
pub fn is_dark_mode() -> bool {
    let Some(app) = android_app() else {
        return false;
    };
    let config = ndk::configuration::Configuration::from_asset_manager(&app.asset_manager());
    config.ui_mode_night() == ndk::configuration::UiModeNight::Yes
}

// ── 软键盘（NativeActivity 兼容路径） ─────────────────────────────────────────
// 自定义 `GpuiInputActivity` 的 `InputConnection` 组合串记 M3；
// M2 用系统输入法开关 + 按键事件明文（见 `window.rs` 的键盘分支）。

/// 弹出软键盘（`show_soft_input`，无窗口时记日志不管）。
///
/// `keyboard_type` 在 `NativeActivity` 下无处可设（系统输入法自己定），
/// 记 M3 随自定义 Activity 接 `InputType`；M2 先保证能弹。
pub fn show_keyboard_android(keyboard_type: super::KeyboardType) {
    let _ = keyboard_type;
    match android_app() {
        Some(app) => app.show_soft_input(true),
        None => log::warn!("软键盘弹出失败：无 AndroidApp"),
    }
}

/// 收起软键盘。
pub fn hide_keyboard_android() {
    match android_app() {
        Some(app) => app.hide_soft_input(true),
        None => log::warn!("软键盘收起失败：无 AndroidApp"),
    }
}

// ── 初始化 ───────────────────────────────────────────────────────────────────

/// panic 进 `logcat`（`android_main` 第一行调）。
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let payload = if let Some(text) = info.payload().downcast_ref::<&str>() {
            (*text).to_string()
        } else if let Some(text) = info.payload().downcast_ref::<String>() {
            text.clone()
        } else {
            "Box<dyn Any>".to_string()
        };
        match info.location() {
            Some(location) => log::error!(
                "PANIC {}:{}:{}：{}",
                location.file(),
                location.line(),
                location.column(),
                payload
            ),
            None => log::error!("PANIC：{payload}"),
        }
    }));
}

/// 存 `AndroidApp` 并建全局平台（每次 `android_main` 都调：返回键退出后
/// 进程还在，同进程重进会起新 native 线程，必须换新平台——旧平台绑定的
/// 线程/looper 已随旧 Activity 销毁，复用会在 `App` 主线程断言处 panic）。
pub fn init_platform(app: &AndroidApp) -> Arc<AndroidPlatform> {
    *ANDROID_APP.write() = Some(app.clone());
    log::info!("init_platform：AndroidApp 已存");
    let platform = Arc::new(AndroidPlatform::new(false));
    log::info!("init_platform：AndroidPlatform 已建");
    let old = PLATFORM.write().replace(platform.clone());
    if old.is_some() {
        log::info!("init_platform：Activity 重建，旧平台已替换（Drop 注销 looper）");
    }
    platform
}

// ── 主循环 ───────────────────────────────────────────────────────────────────

/// 事件循环（`Platform::run` 内阻塞到 `quit()` 或 Activity 销毁）。
pub fn run_event_loop(app: &AndroidApp) {
    log::info!("run_event_loop：进主循环");
    INIT_WINDOW_DONE.store(false, Ordering::Relaxed);
    super::frame_source::install();
    let mut app_is_active = false;

    loop {
        if let Some(platform) = locked_platform() {
            if platform.should_quit() {
                log::info!("run_event_loop：平台要求退出");
                break;
            }
            platform.tick();
        }

        // 阻塞等事：生命周期命令、输入、主任务、vsync 回调；
        // 超时只兜延后任务与无 choreographer 的挂钟。
        let timeout = frame_source::poll_timeout(
            locked_platform().and_then(|platform| platform.next_delayed_due()),
            INIT_WINDOW_DONE.load(Ordering::Relaxed) && app_is_active,
        );
        app.poll_events(Some(timeout), |event| match event {
            PollEvent::Main(main_event) => handle_main_event(main_event),
            PollEvent::Wake | PollEvent::Timeout | _ => {}
        });

        // 延迟生命周期消化（`poll_events` 返回后，无锁竞争）。
        drain_pending_lifecycle(app);
        if let Some(platform) = locked_platform() {
            if let Some(window) = platform.primary_window() {
                let is_active = window.is_active();
                if is_active != app_is_active {
                    log::info!("前后台：{app_is_active} → {is_active}");
                    app_is_active = is_active;
                }
            }
        }

        process_input_events(app);
        invoke_deferred_init_callbacks();

        // 渲染：可画且节拍器欠帧才 tick GPUI 一次。
        if let Some(platform) = locked_platform() {
            if INIT_WINDOW_DONE.load(Ordering::Relaxed) && app_is_active {
                let ran = platform.flush_main_thread_tasks();
                if ran > 0 {
                    // 主任务跑了（定时器/动画下一拍多半在里面）：补一帧需求。
                    frame_source::schedule_frame();
                }
                if crate::TEXT_INPUT_DIRTY.load(Ordering::Acquire) {
                    frame_source::schedule_frame();
                }
                if frame_source::take_frame() {
                    log::info!("tick：欠帧取出，跑一帧");
                    if let Some(window) = platform.primary_window() {
                        window.request_frame();
                    }
                }
            }
        }
    }
    log::info!("run_event_loop：出主循环");
}

/// 消化延迟生命周期标记（`InitWindow` 建窗/`TermWindow` 卸面等）。
fn drain_pending_lifecycle(app: &AndroidApp) {
    // 每次处理后排空一次 Java 侧新到的命令（防 UI 线程 condvar 久等 ANR）。
    let drain = |app: &AndroidApp| {
        app.poll_events(Some(Duration::ZERO), |event| {
            if let PollEvent::Main(main_event) = event {
                handle_main_event(main_event);
            }
        });
    };

    if TERM_WINDOW_PENDING.swap(false, Ordering::Relaxed) {
        log::info!("lifecycle：TerminateWindow");
        INIT_WINDOW_DONE.store(false, Ordering::Relaxed);
        if let Some(platform) = locked_platform() {
            if let Some(window) = platform.primary_window() {
                window.term_window();
            }
        }
        drain(app);
    }

    if INIT_WINDOW_PENDING.swap(false, Ordering::Relaxed) {
        log::info!("lifecycle：InitWindow");
        if let Some(platform) = locked_platform() {
            if let Some(native_window) = app.native_window() {
                let (width, height) = (native_window.width(), native_window.height());
                log::info!("InitWindow：{width}×{height}");
                platform.update_primary_display(&native_window, &app.asset_manager());
                let scale = platform
                    .primary_display()
                    .map(|display| display.scale_factor())
                    .unwrap_or(2.0);
                if let Some(existing) = platform.primary_window() {
                    let gpu = platform.gpu_context();
                    match existing.init_window(native_window, gpu) {
                        Ok(()) => log::info!("旧窗 surface 已重挂"),
                        Err(error) => log::error!("surface 重挂失败：{error:#}"),
                    }
                    existing.handle_resize();
                    let rect = app.content_rect();
                    existing.update_safe_area_from_content_rect(
                        rect.left,
                        rect.top,
                        rect.right,
                        rect.bottom,
                    );
                    apply_system_appearance(&existing);
                    INIT_WINDOW_DONE.store(true, Ordering::Relaxed);
                } else {
                    match platform.open_window(native_window, scale, false) {
                        Ok(window) => {
                            log::info!("首窗已开 scale={scale:.1}");
                            let rect = app.content_rect();
                            window.update_safe_area_from_content_rect(
                                rect.left,
                                rect.top,
                                rect.right,
                                rect.bottom,
                            );
                            apply_system_appearance(&window);
                        }
                        Err(error) => log::error!("开窗失败：{error:#}"),
                    }
                }
            }
        }
        drain(app);
    }

    if WINDOW_RESIZED_PENDING.swap(false, Ordering::Relaxed) {
        if let Some(platform) = locked_platform() {
            if let Some(window) = platform.primary_window() {
                window.handle_resize();
                let rect = app.content_rect();
                window.update_safe_area_from_content_rect(
                    rect.left,
                    rect.top,
                    rect.right,
                    rect.bottom,
                );
            }
        }
    }

    if CONFIG_CHANGED_PENDING.swap(false, Ordering::Relaxed) {
        if let Some(platform) = locked_platform() {
            platform.notify_keyboard_layout_change();
            if let Some(window) = platform.primary_window() {
                apply_system_appearance(&window);
            }
        }
    }

    if PAUSE_PENDING.swap(false, Ordering::Relaxed) {
        log::info!("lifecycle：Pause");
        if let Some(platform) = locked_platform() {
            platform.did_enter_background();
            if let Some(window) = platform.primary_window() {
                window.set_active(false);
            }
        }
    }

    if RESUME_PENDING.swap(false, Ordering::Relaxed) {
        log::info!("lifecycle：Resume");
        if let Some(platform) = locked_platform() {
            platform.did_become_active();
            if let Some(window) = platform.primary_window() {
                window.set_active(true);
            }
        }
    }
}

/// 按系统深浅色刷窗口外观。
fn apply_system_appearance(window: &AndroidWindow) {
    window.set_appearance(if is_dark_mode() {
        LocalAppearance::Dark
    } else {
        LocalAppearance::Light
    });
}

/// 首窗就绪后调延后回调（`finish_launching` 与 `on_init_window` 只跑一次）。
fn invoke_deferred_init_callbacks() {
    if INIT_WINDOW_DONE.load(Ordering::Relaxed) {
        return;
    }
    let Some(platform) = locked_platform() else {
        return;
    };
    if platform.primary_window().is_none() {
        return;
    }
    if let Some(finish) = platform.take_finish_launching_callback() {
        log::info!("调 finish_launching 回调");
        finish();
    }
    if let Some(on_window) = platform.take_on_init_window_callback() {
        let window = platform.primary_window().expect("主窗应在");
        log::info!("调 on_init_window 回调");
        on_window(window);
    }
    INIT_WINDOW_DONE.store(true, Ordering::Relaxed);
    NATIVE_INITIALIZED.store(true, Ordering::Release);
    log::info!("NATIVE_INITIALIZED = true");
    platform.flush_main_thread_tasks();
    if let Some(window) = platform.primary_window() {
        window.request_frame();
    }
}

/// 单个主事件只记标记（见模块头死锁说明）。
fn handle_main_event(event: MainEvent<'_>) {
    match event {
        MainEvent::InitWindow { .. } => {
            INIT_WINDOW_PENDING.store(true, Ordering::Relaxed);
        }
        MainEvent::TerminateWindow { .. } => {
            TERM_WINDOW_PENDING.store(true, Ordering::Relaxed);
        }
        MainEvent::WindowResized { .. } => {
            WINDOW_RESIZED_PENDING.store(true, Ordering::Relaxed);
        }
        MainEvent::GainedFocus | MainEvent::Resume { .. } => {
            RESUME_PENDING.store(true, Ordering::Relaxed);
        }
        MainEvent::LostFocus | MainEvent::Pause => {
            PAUSE_PENDING.store(true, Ordering::Relaxed);
        }
        MainEvent::ConfigChanged { .. } => {
            CONFIG_CHANGED_PENDING.store(true, Ordering::Relaxed);
        }
        MainEvent::InsetsChanged { .. } | MainEvent::ContentRectChanged { .. } => {
            WINDOW_RESIZED_PENDING.store(true, Ordering::Relaxed);
        }
        MainEvent::Start | MainEvent::Stop | MainEvent::SaveState { .. } => {
            log::debug!("lifecycle：次要事件");
        }
        MainEvent::LowMemory => {
            log::warn!("系统内存吃紧，考虑放缓存");
        }
        MainEvent::Destroy => {
            log::info!("lifecycle：Destroy");
            if let Some(platform) = locked_platform() {
                platform.quit();
            }
        }
        _ => {}
    }
}

// ── 输入处理 ─────────────────────────────────────────────────────────────────

/// 触摸动作常量（NDK `AMOTION_EVENT_ACTION_*`）。
const ACTION_DOWN: u32 = 0;
/// 触摸动作常量。
const ACTION_UP: u32 = 1;
/// 触摸动作常量。
const ACTION_MOVE: u32 = 2;
/// 触摸动作常量。
const ACTION_CANCEL: u32 = 3;

/// 耗尽输入队列喂窗口（每批输入补一帧需求，触摸采样才跟手）。
fn process_input_events(app: &AndroidApp) {
    let Some(platform) = locked_platform() else {
        return;
    };
    let Some(window) = platform.primary_window() else {
        return;
    };
    let events = match app.input_events_iter() {
        Ok(iterator) => iterator,
        Err(error) => {
            log::error!("输入迭代器拿不到：{error:?}");
            return;
        }
    };
    let mut iterator = events;
    let mut saw_input = false;
    loop {
        let more = iterator.next(|event| {
            saw_input = true;
            use android_activity::input::{InputEvent, KeyAction, MotionAction};
            match event {
                InputEvent::MotionEvent(motion) => {
                    let action = motion.action();
                    let count = motion.pointer_count();
                    for index in 0..count {
                        let pointer = motion.pointer_at_index(index);
                        let touch_action = match action {
                            MotionAction::Down => ACTION_DOWN,
                            MotionAction::PointerDown => {
                                if index != motion.pointer_index() {
                                    continue;
                                }
                                ACTION_DOWN
                            }
                            MotionAction::Up => ACTION_UP,
                            MotionAction::PointerUp => {
                                if index != motion.pointer_index() {
                                    continue;
                                }
                                ACTION_UP
                            }
                            MotionAction::Move => ACTION_MOVE,
                            MotionAction::Cancel => ACTION_CANCEL,
                            _ => continue,
                        };
                        window.handle_touch(TouchPoint {
                            id: pointer.pointer_id(),
                            x: pointer.x(),
                            y: pointer.y(),
                            action: touch_action,
                        });
                    }
                    android_activity::InputStatus::Handled
                }
                InputEvent::KeyEvent(key_event) => {
                    let action = match key_event.action() {
                        KeyAction::Down => 0,
                        KeyAction::Up => 1,
                        _ => return android_activity::InputStatus::Unhandled,
                    };
                    let code: u32 = key_event.key_code().into();
                    let meta: u32 = key_event.meta_state().0;
                    let unicode = unicode_char_for_key_event(code as i32, action, meta as i32);
                    let consumed = window.handle_key_event(AndroidKeyEvent {
                        key_code: code as i32,
                        action,
                        meta_state: meta as i32,
                        unicode_char: unicode,
                    });
                    // 返回键未被应用消费时放行系统默认处理（退出到桌面）；
                    // 其余按键恒视为已消费，保持既有文本输入行为。
                    if code as i32 == super::keyboard::AKEYCODE_BACK && !consumed {
                        android_activity::InputStatus::Unhandled
                    } else {
                        android_activity::InputStatus::Handled
                    }
                }
                _ => android_activity::InputStatus::Unhandled,
            }
        });
        if !more {
            break;
        }
    }
    if saw_input {
        frame_source::schedule_frame();
    }
}

/// 查键盘布局标识（`InputMethodManager` 当前子类型 locale，失败回 `en-US`）。
pub fn keyboard_layout_id() -> String {
    with_env(|env| {
        let activity_obj = activity(env)?;
        let service = env
            .new_string("input_method")
            .map_err(|error| error.to_string())?;
        let manager = env
            .call_method(
                &activity_obj,
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
            return Ok(None);
        }
        let subtype = env
            .call_method(
                &manager,
                jni::jni_str!("getCurrentInputMethodSubtype"),
                jni::jni_sig!("()Landroid/view/inputmethod/InputMethodSubtype;"),
                &[],
            )
            .and_then(|value| value.l())
            .map_err(|error| {
                env.exception_clear();
                error.to_string()
            })?;
        if subtype.is_null() {
            return Ok(None);
        }
        let locale = env
            .call_method(
                &subtype,
                jni::jni_str!("getLocale"),
                jni::jni_sig!("()Ljava/lang/String;"),
                &[],
            )
            .and_then(|value| value.l())
            .map_err(|error| {
                env.exception_clear();
                error.to_string()
            })?;
        let id = get_string(env, &locale).replace('_', "-");
        Ok(if id.is_empty() { None } else { Some(id) })
    })
    .ok()
    .flatten()
    .unwrap_or_else(|| "en-US".to_string())
}

/// 查窗口外观（深浅色，供 `Platform::window_appearance`）。
pub fn system_window_appearance() -> WindowAppearance {
    if is_dark_mode() {
        WindowAppearance::Dark
    } else {
        WindowAppearance::Light
    }
}

// ── 系统 chrome（状态栏/导航栏） ──────────────────────────────────────────────

/// 上次应用的样式（同值跳过 JNI：View 操作抢 UI 线程锁，高频调会卡）。
static LAST_CHROME_STYLE: std::sync::Mutex<
    Option<(Option<u32>, Option<u32>, super::StatusBarContentStyle)>,
> = std::sync::Mutex::new(None);

/// 应用系统 chrome 样式（状态栏/导航栏底色 + 状态栏文字深浅）。
pub fn set_system_chrome(style: &super::SystemChromeStyle) {
    let key = (
        style.status_bar_color,
        style.navigation_bar_color,
        style.status_bar_style,
    );
    {
        let mut last = LAST_CHROME_STYLE.lock().unwrap();
        if *last == Some(key) {
            return;
        }
        *last = Some(key);
    }
    let result = with_env(|env| {
        let activity_obj = activity(env)?;
        let window = env
            .call_method(
                &activity_obj,
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
            return Err("getWindow 为空".into());
        }
        // 状态栏底色（补 alpha FF）。
        if let Some(color) = style.status_bar_color {
            let argb = (0xFF00_0000u32 | color) as i32;
            let _ = env.call_method(
                &window,
                jni::jni_str!("setStatusBarColor"),
                jni::jni_sig!("(I)V"),
                &[JValue::Int(argb)],
            );
            env.exception_clear();
        }
        // 导航栏底色。
        if let Some(color) = style.navigation_bar_color {
            let argb = (0xFF00_0000u32 | color) as i32;
            let _ = env.call_method(
                &window,
                jni::jni_str!("setNavigationBarColor"),
                jni::jni_sig!("(I)V"),
                &[JValue::Int(argb)],
            );
            env.exception_clear();
        }
        // 状态栏文字深浅：API 30+ 走 WindowInsetsController，否则 decorView 标志位。
        let controller = env.call_method(
            &window,
            jni::jni_str!("getInsetsController"),
            jni::jni_sig!("()Landroid/view/WindowInsetsController;"),
            &[],
        );
        match controller {
            Ok(value) => {
                if let Ok(ctl) = value.l() {
                    if !ctl.is_null() {
                        const MASK: i32 = 0x0000_0008;
                        let appearance = match style.status_bar_style {
                            super::StatusBarContentStyle::Dark => 0x0000_0008,
                            super::StatusBarContentStyle::Light => 0,
                        };
                        let _ = env.call_method(
                            &ctl,
                            jni::jni_str!("setSystemBarsAppearance"),
                            jni::jni_sig!("(II)V"),
                            &[JValue::Int(appearance), JValue::Int(MASK)],
                        );
                        env.exception_clear();
                    }
                }
            }
            Err(_) => {
                env.exception_clear();
                if let Ok(decor) = env
                    .call_method(
                        &window,
                        jni::jni_str!("getDecorView"),
                        jni::jni_sig!("()Landroid/view/View;"),
                        &[],
                    )
                    .and_then(|value| value.l())
                {
                    if !decor.is_null() {
                        if let Ok(current) = env
                            .call_method(
                                &decor,
                                jni::jni_str!("getSystemUiVisibility"),
                                jni::jni_sig!("()I"),
                                &[],
                            )
                            .and_then(|value| value.i())
                        {
                            const LIGHT_FLAG: i32 = 0x0000_2000;
                            let next = match style.status_bar_style {
                                super::StatusBarContentStyle::Dark => current | LIGHT_FLAG,
                                super::StatusBarContentStyle::Light => current & !LIGHT_FLAG,
                            };
                            let _ = env.call_method(
                                &decor,
                                jni::jni_str!("setSystemUiVisibility"),
                                jni::jni_sig!("(I)V"),
                                &[JValue::Int(next)],
                            );
                            env.exception_clear();
                        }
                    }
                }
            }
        }
        Ok(())
    });
    if let Err(error) = result {
        log::warn!("set_system_chrome：{error}");
    }
}

// ── 剪贴板（`ClipboardManager`） ──────────────────────────────────────────────

/// 取 `ClipboardManager` 服务对象。
fn clipboard_manager<'local>(env: &mut jni::Env<'local>) -> Result<JObject<'local>, String> {
    let activity_obj = activity(env)?;
    let service = env
        .new_string("clipboard")
        .map_err(|error| error.to_string())?;
    let manager = env
        .call_method(
            &activity_obj,
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
        return Err("剪贴板服务为空".into());
    }
    Ok(manager)
}

/// 读系统剪贴板文本（空/错回空）。
pub fn read_clipboard_android() -> Option<String> {
    with_env(|env| {
        let manager = clipboard_manager(env)?;
        let clip = env
            .call_method(
                &manager,
                jni::jni_str!("getPrimaryClip"),
                jni::jni_sig!("()Landroid/content/ClipData;"),
                &[],
            )
            .and_then(|value| value.l())
            .map_err(|error| {
                env.exception_clear();
                error.to_string()
            })?;
        if clip.is_null() {
            return Ok(None);
        }
        let item = env
            .call_method(
                &clip,
                jni::jni_str!("getItemAt"),
                jni::jni_sig!("(I)Landroid/content/ClipData$Item;"),
                &[JValue::Int(0)],
            )
            .and_then(|value| value.l())
            .map_err(|error| {
                env.exception_clear();
                error.to_string()
            })?;
        if item.is_null() {
            return Ok(None);
        }
        let text = env
            .call_method(
                &item,
                jni::jni_str!("getText"),
                jni::jni_sig!("()Ljava/lang/CharSequence;"),
                &[],
            )
            .and_then(|value| value.l())
            .map_err(|error| {
                env.exception_clear();
                error.to_string()
            })?;
        if text.is_null() {
            return Ok(None);
        }
        Ok(Some(get_string(env, &text)))
    })
    .ok()
    .flatten()
}

/// 写系统剪贴板文本。
pub fn write_clipboard_android(text: &str) {
    let result = with_env(|env| {
        let manager = clipboard_manager(env)?;
        let label = env.new_string("rgpui").map_err(|error| error.to_string())?;
        let content = env.new_string(text).map_err(|error| error.to_string())?;
        let clip = env
            .call_static_method(
                jni::jni_str!("android/content/ClipData"),
                jni::jni_str!("newPlainText"),
                jni::jni_sig!(
                    "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Landroid/content/ClipData;"
                ),
                &[JValue::Object(&label), JValue::Object(&content)],
            )
            .and_then(|value| value.l())
            .map_err(|error| {
                env.exception_clear();
                error.to_string()
            })?;
        let _ = env.call_method(
            &manager,
            jni::jni_str!("setPrimaryClip"),
            jni::jni_sig!("(Landroid/content/ClipData;)V"),
            &[JValue::Object(&clip)],
        );
        env.exception_clear();
        Ok(())
    });
    if let Err(error) = result {
        log::warn!("剪贴板写入失败：{error}");
    }
}

// ── 系统打开链接（`ACTION_VIEW`） ─────────────────────────────────────────────
/// 用系统默认应用打开 URL（浏览器/商店等按 scheme 分发）。
pub fn open_url_android(url: &str) {
    let result = with_env(|env| {
        let activity_obj = activity(env)?;
        let uri_string = env.new_string(url).map_err(|error| error.to_string())?;
        let uri = env
            .call_static_method(
                jni::jni_str!("android/net/Uri"),
                jni::jni_str!("parse"),
                jni::jni_sig!("(Ljava/lang/String;)Landroid/net/Uri;"),
                &[JValue::Object(&uri_string)],
            )
            .and_then(|value| value.l())
            .map_err(|error| {
                env.exception_clear();
                error.to_string()
            })?;
        let action = env
            .new_string("android.intent.action.VIEW")
            .map_err(|error| error.to_string())?;
        let intent = env
            .new_object(
                jni::jni_str!("android/content/Intent"),
                jni::jni_sig!("(Ljava/lang/String;Landroid/net/Uri;)V"),
                &[JValue::Object(&action), JValue::Object(&uri)],
            )
            .map_err(|error| {
                env.exception_clear();
                error.to_string()
            })?;
        let _ = env.call_method(
            &intent,
            jni::jni_str!("addFlags"),
            jni::jni_sig!("(I)Landroid/content/Intent;"),
            &[JValue::Int(0x1000_0000)],
        );
        env.exception_clear();
        let _ = env.call_method(
            &activity_obj,
            jni::jni_str!("startActivity"),
            jni::jni_sig!("(Landroid/content/Intent;)V"),
            &[JValue::Object(&intent)],
        );
        env.exception_clear();
        Ok(())
    });
    if let Err(error) = result {
        log::warn!("open_url({url}) 失败：{error}");
    }
}

// ── 振动器（`Vibrator`） ───────────────────────────────────────────────────────
// 需要 `android.permission.VIBRATE`（普通权限，声明即授，不用动态申请）。

/// 取 `Vibrator` 服务对象。
fn vibrator<'local>(env: &mut jni::Env<'local>) -> Result<JObject<'local>, String> {
    let activity_obj = activity(env)?;
    let service = env
        .new_string("vibrator")
        .map_err(|error| error.to_string())?;
    let manager = env
        .call_method(
            &activity_obj,
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
        return Err("振动服务为空".into());
    }
    Ok(manager)
}

/// 触发一次短振动（`VibrationEffect.createOneShot`，API 26+；失败只记日志）。
pub fn vibrate_android(duration_ms: u64) {
    let result = with_env(|env| {
        let vibrator = vibrator(env)?;
        // `VibrationEffect.DEFAULT_AMPLITUDE`（-1 表系统默认）。
        let amplitude = env
            .get_static_field(
                jni::jni_str!("android/os/VibrationEffect"),
                jni::jni_str!("DEFAULT_AMPLITUDE"),
                jni::jni_sig!("I"),
            )
            .and_then(|value| value.i())
            .map_err(|error| {
                env.exception_clear();
                error.to_string()
            })?;
        let effect = env
            .call_static_method(
                jni::jni_str!("android/os/VibrationEffect"),
                jni::jni_str!("createOneShot"),
                jni::jni_sig!("(JI)Landroid/os/VibrationEffect;"),
                &[JValue::Long(duration_ms as i64), JValue::Int(amplitude)],
            )
            .and_then(|value| value.l())
            .map_err(|error| {
                env.exception_clear();
                error.to_string()
            })?;
        let _ = env.call_method(
            &vibrator,
            jni::jni_str!("vibrate"),
            jni::jni_sig!("(Landroid/os/VibrationEffect;)V"),
            &[JValue::Object(&effect)],
        );
        env.exception_clear();
        Ok(())
    });
    if let Err(error) = result {
        log::warn!("振动失败：{error}");
    }
}

// ── 电池状态（粘性广播 `ACTION_BATTERY_CHANGED`） ──────────────────────────────

/// 读电池电量百分比与充电状态（读不到给未知）。
pub fn battery_status_android() -> rgpui::BatteryStatus {
    with_env(|env| {
        let activity_obj = activity(env)?;
        let action = env
            .new_string("android.intent.action.BATTERY_CHANGED")
            .map_err(|error| error.to_string())?;
        let filter = env
            .new_object(
                jni::jni_str!("android/content/IntentFilter"),
                jni::jni_sig!("(Ljava/lang/String;)V"),
                &[JValue::Object(&action)],
            )
            .map_err(|error| {
                env.exception_clear();
                error.to_string()
            })?;
        // 传 null receiver 取粘性广播的最新 Intent（不同步注册）。
        let intent = env
            .call_method(
                &activity_obj,
                jni::jni_str!("registerReceiver"),
                jni::jni_sig!(
                    "(Landroid/content/BroadcastReceiver;Landroid/content/IntentFilter;)Landroid/content/Intent;"
                ),
                &[JValue::Object(&JObject::null()), JValue::Object(&filter)],
            )
            .and_then(|value| value.l())
            .map_err(|error| {
                env.exception_clear();
                error.to_string()
            })?;
        if intent.is_null() {
            return Err("电池广播为空".into());
        }
        let mut int_extra = |name: &str| -> Result<i32, String> {
            let key = env.new_string(name).map_err(|error| error.to_string())?;
            env.call_method(
                &intent,
                jni::jni_str!("getIntExtra"),
                jni::jni_sig!("(Ljava/lang/String;I)I"),
                &[JValue::Object(&key), JValue::Int(-1)],
            )
            .and_then(|value| value.i())
            .map_err(|error| {
                env.exception_clear();
                error.to_string()
            })
        };
        let level = int_extra("level")?;
        let scale = int_extra("scale")?;
        let status = int_extra("status")?;
        let level_percent = if level >= 0 && scale > 0 {
            Some((level * 100 / scale).clamp(0, 100) as u8)
        } else {
            None
        };
        // `BatteryManager.BATTERY_STATUS_CHARGING = 2`，`BATTERY_STATUS_FULL = 5`。
        let charging = status == 2 || status == 5;
        Ok(rgpui::BatteryStatus {
            level_percent,
            charging,
        })
    })
    .unwrap_or_else(|error| {
        log::warn!("电池状态读取失败：{error}");
        rgpui::BatteryStatus::unknown()
    })
}
