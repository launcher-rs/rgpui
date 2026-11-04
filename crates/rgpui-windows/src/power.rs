//! Windows 电源事件与电源抑制
//!
//! - 睡眠/唤醒事件走 `PowerRegisterSuspendResumeNotification` 的回调注册，
//!   回调运行在系统线程，只负责把事件 `PostMessageW` 到平台消息窗口，再由主线程
//!   派发给应用（`App` 不是 `Send`，回调必须在主线程执行）。
//! - 抑制走 `SetThreadExecutionState`：句柄持有期间保持 `ES_CONTINUOUS`，
//!   `Drop` 时清掉标志，恢复系统原来的省电策略。

use rgpui::{PowerSaveBlocker, PowerSaveBlockerKind, SystemPowerEvent};
use std::ffi::c_void;
use windows::Win32::{
    Foundation::{HANDLE, HWND, LPARAM, WPARAM},
    System::Power::{
        DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
        HPOWERNOTIFY, PowerRegisterSuspendResumeNotification,
        PowerUnregisterSuspendResumeNotification, SetThreadExecutionState,
    },
    UI::WindowsAndMessaging::{
        DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
        PostMessageW,
    },
};

/// 主线程消息窗口收到的电源事件消息，`wParam` 编码事件类型。
pub(crate) const WM_GPUI_POWER_EVENT: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 10;
/// `WM_GPUI_POWER_EVENT` 的 `wParam`：系统即将睡眠。
pub(crate) const POWER_EVENT_SLEEP: usize = 0;
/// `WM_GPUI_POWER_EVENT` 的 `wParam`：系统已唤醒。
pub(crate) const POWER_EVENT_WAKE: usize = 1;

/// 已注册的睡眠/唤醒通知；持有期间系统会把事件投递到平台消息窗口。
pub(crate) struct PowerEventRegistration {
    // 参数块必须活到注销为止：系统回调的 Context 指向它
    _parameters: Box<DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS>,
    handle: *mut c_void,
}

impl Drop for PowerEventRegistration {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                let _ =
                    PowerUnregisterSuspendResumeNotification(HPOWERNOTIFY(self.handle as isize));
            }
        }
    }
}

/// 在平台消息窗口上注册睡眠/唤醒通知。
///
/// `context` 存窗口句柄（不是指针），系统线程回调里只做一次 `PostMessageW`。
pub(crate) fn register_power_events(hwnd: HWND) -> Option<PowerEventRegistration> {
    let mut parameters = Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
        Callback: Some(power_event_callback),
        Context: hwnd.0,
    });
    let mut handle = std::ptr::null_mut();

    let status = unsafe {
        PowerRegisterSuspendResumeNotification(
            DEVICE_NOTIFY_CALLBACK,
            HANDLE((&mut *parameters) as *mut _ as *mut c_void),
            &mut handle,
        )
    };
    if status.0 != 0 {
        log::warn!("注册系统电源事件失败: {status:?}");
        return None;
    }

    Some(PowerEventRegistration {
        _parameters: parameters,
        handle,
    })
}

/// 系统线程上的通知回调：只转换事件并投递到主线程。
unsafe extern "system" fn power_event_callback(
    context: *const c_void,
    event_type: u32,
    _setting: *const c_void,
) -> u32 {
    let wparam = match event_type {
        PBT_APMSUSPEND => POWER_EVENT_SLEEP,
        PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => POWER_EVENT_WAKE,
        // 其他电源广播（如待机策略变化）与当前 API 无关
        _ => return 0,
    };

    unsafe {
        let _ = PostMessageW(
            Some(HWND(context as *mut c_void)),
            WM_GPUI_POWER_EVENT,
            WPARAM(wparam),
            LPARAM(0),
        );
    }
    // ERROR_SUCCESS
    0
}

/// 把 `WM_GPUI_POWER_EVENT` 的 `wParam` 还原成事件，交给调用方派发。
pub(crate) fn decode_power_event(wparam: WPARAM) -> Option<SystemPowerEvent> {
    match wparam.0 {
        POWER_EVENT_SLEEP => Some(SystemPowerEvent::Sleep),
        POWER_EVENT_WAKE => Some(SystemPowerEvent::WakeUp),
        _ => None,
    }
}

/// 电源抑制句柄：`Drop` 时清掉 `ES_CONTINUOUS`，系统立刻恢复省电策略。
pub(crate) struct WindowsPowerSaveBlocker;

impl PowerSaveBlocker for WindowsPowerSaveBlocker {}

impl Drop for WindowsPowerSaveBlocker {
    fn drop(&mut self) {
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}

/// 按类别设置线程执行状态；系统拒绝（返回 0）时返回 `None`。
pub(crate) fn start_power_save_blocker(
    kind: PowerSaveBlockerKind,
) -> Option<Box<dyn PowerSaveBlocker>> {
    let flags = match kind {
        PowerSaveBlockerKind::PreventSleep => ES_CONTINUOUS | ES_SYSTEM_REQUIRED,
        PowerSaveBlockerKind::PreventDisplaySleep => ES_CONTINUOUS | ES_DISPLAY_REQUIRED,
    };

    let previous = unsafe { SetThreadExecutionState(flags) };
    if previous.0 == 0 {
        log::warn!("SetThreadExecutionState 失败，未能阻止系统省电: {kind:?}");
        return None;
    }

    Some(Box::new(WindowsPowerSaveBlocker))
}
