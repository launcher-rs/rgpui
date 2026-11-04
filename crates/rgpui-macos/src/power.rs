//! macOS 电源事件与电源抑制
//!
//! - 睡眠/唤醒：订阅 `NSWorkspace` 的 `WillSleep` / `DidWake` 通知。
//!   通知由 AppKit 在主线程投递，回调因此可以直接触碰应用状态。
//! - 抑制：IOKit 的电源管理断言（`IOPMAssertionCreateWithName`），
//!   句柄 `Drop` 时释放断言，系统立刻恢复省电策略。

use std::cell::{Cell, RefCell};
use std::ffi::c_void;

use block::ConcreteBlock;
use cocoa::base::{id, nil};
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use objc::{class, msg_send, sel, sel_impl};
use rgpui::{PowerSaveBlocker, PowerSaveBlockerKind, SystemPowerEvent};

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    /// `NSWorkspaceWillSleepNotification`（`NSString * const`）
    static NSWorkspaceWillSleepNotification: *const c_void;
    /// `NSWorkspaceDidWakeNotification`（`NSString * const`）
    static NSWorkspaceDidWakeNotification: *const c_void;
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPMAssertionCreateWithName(
        assertion_type: core_foundation::string::CFStringRef,
        assertion_level: u32,
        assertion_name: core_foundation::string::CFStringRef,
        assertion_id: *mut u32,
    ) -> i32;
    fn IOPMAssertionRelease(assertion_id: u32) -> i32;
}

/// `kIOPMAssertionLevelOn`
const ASSERTION_LEVEL_ON: u32 = 255;

thread_local! {
    /// 电源事件回调；AppKit 在主线程投递通知，回调也只在主线程被取出
    static CALLBACKS: RefCell<PowerCallbacks> = RefCell::new(PowerCallbacks::default());
    /// 观察者是否已注册（进程内只注册一次）
    static OBSERVERS_INSTALLED: Cell<bool> = Cell::new(false);
}

#[derive(Default)]
struct PowerCallbacks {
    on_event: Option<Box<dyn FnMut(SystemPowerEvent)>>,
    on_wake: Option<Box<dyn FnMut()>>,
}

/// 注册通用电源事件回调，首次注册时安装 `NSWorkspace` 观察者。
pub(crate) fn set_power_event_callback(callback: Box<dyn FnMut(SystemPowerEvent)>) {
    ensure_observers();
    CALLBACKS.with(|slot| slot.borrow_mut().on_event = Some(callback));
}

/// 注册系统唤醒回调，首次注册时安装 `NSWorkspace` 观察者。
pub(crate) fn set_wake_callback(callback: Box<dyn FnMut()>) {
    ensure_observers();
    CALLBACKS.with(|slot| slot.borrow_mut().on_wake = Some(callback));
}

/// 把事件派发给应用；回调先取出、调用完再放回，允许回调里再次注册。
fn dispatch(event: SystemPowerEvent) {
    let (mut on_event, mut on_wake) = CALLBACKS.with(|slot| {
        let mut callbacks = slot.borrow_mut();
        (callbacks.on_event.take(), callbacks.on_wake.take())
    });

    if event == SystemPowerEvent::WakeUp {
        if let Some(callback) = on_wake.as_mut() {
            callback();
        }
    }
    if let Some(callback) = on_event.as_mut() {
        callback(event);
    }

    CALLBACKS.with(|slot| {
        let mut callbacks = slot.borrow_mut();
        callbacks.on_event = on_event;
        callbacks.on_wake = on_wake;
    });
}

/// 订阅 `NSWorkspace` 的睡眠/唤醒通知；进程内只装一次。
///
/// `addObserverForName:` 返回的观察者令牌由通知中心自身持有，这里不再保存：
/// 观察者要活到进程结束，提前释放反而会让通知失去订阅方。
fn ensure_observers() {
    OBSERVERS_INSTALLED.with(|installed| {
        if installed.get() {
            return;
        }
        installed.set(true);

        unsafe {
            let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
            let center: id = msg_send![workspace, notificationCenter];
            let sleep_name: id = NSWorkspaceWillSleepNotification as id;
            let wake_name: id = NSWorkspaceDidWakeNotification as id;

            let sleep_block = ConcreteBlock::new(|_note: id| dispatch(SystemPowerEvent::Sleep));
            let sleep_block = sleep_block.copy();
            let _: id = msg_send![
                center,
                addObserverForName: sleep_name
                object: nil
                queue: nil
                usingBlock: sleep_block
            ];

            let wake_block = ConcreteBlock::new(|_note: id| dispatch(SystemPowerEvent::WakeUp));
            let wake_block = wake_block.copy();
            let _: id = msg_send![
                center,
                addObserverForName: wake_name
                object: nil
                queue: nil
                usingBlock: wake_block
            ];
        }
    });
}

/// 电源抑制句柄：持有期间断言生效，`Drop` 时释放。
pub(crate) struct MacPowerSaveBlocker {
    assertion_id: u32,
}

impl PowerSaveBlocker for MacPowerSaveBlocker {}

impl Drop for MacPowerSaveBlocker {
    fn drop(&mut self) {
        unsafe {
            IOPMAssertionRelease(self.assertion_id);
        }
    }
}

/// 创建电源抑制断言；系统拒绝时返回 `None`。
pub(crate) fn start_power_save_blocker(
    kind: PowerSaveBlockerKind,
) -> Option<Box<dyn PowerSaveBlocker>> {
    let assertion_type = match kind {
        PowerSaveBlockerKind::PreventSleep => "PreventUserIdleSystemSleep",
        PowerSaveBlockerKind::PreventDisplaySleep => "PreventUserIdleDisplaySleep",
    };
    let assertion_type = CFString::new(assertion_type);
    let assertion_name = CFString::new("rgpui power save blocker");

    let mut assertion_id = 0;
    let status = unsafe {
        IOPMAssertionCreateWithName(
            assertion_type.as_concrete_TypeRef(),
            ASSERTION_LEVEL_ON,
            assertion_name.as_concrete_TypeRef(),
            &mut assertion_id,
        )
    };
    if status != 0 {
        log::warn!("IOPMAssertionCreateWithName 失败: {status}，未能阻止系统省电");
        return None;
    }

    Some(Box::new(MacPowerSaveBlocker { assertion_id }))
}
