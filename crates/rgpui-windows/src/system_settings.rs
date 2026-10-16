use std::{
    cell::Cell,
    ffi::{c_int, c_uint, c_void},
};

use ::rgpui::util::ResultExt;
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETCLIENTAREAANIMATION, SPI_GETWHEELSCROLLCHARS, SPI_GETWHEELSCROLLLINES,
    SYSTEM_PARAMETERS_INFO_ACTION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};

/// 读取系统「窗口内动画」开关（`SPI_GETCLIENTAREAANIMATION`）。
///
/// 它对应「设置 → 辅助功能 → 视觉效果 → 动画效果」，取反即为「减少动态效果」。
/// 读取失败时返回 `true`（保持动画），不把「拿不到设置」误判成用户请求减少动画。
pub(crate) fn client_area_animation_enabled() -> bool {
    let mut enabled = c_int::default();
    let result = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&mut enabled) as *mut c_int as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
        )
    };

    result.log_err().is_some() && enabled != 0
}

/// Windows settings pulled from SystemParametersInfo
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-systemparametersinfow
#[derive(Default, Debug, Clone)]
pub(crate) struct WindowsSystemSettings {
    pub(crate) mouse_wheel_settings: MouseWheelSettings,
}

#[derive(Default, Debug, Clone)]
pub(crate) struct MouseWheelSettings {
    /// SEE: SPI_GETWHEELSCROLLCHARS
    pub(crate) wheel_scroll_chars: Cell<u32>,
    /// SEE: SPI_GETWHEELSCROLLLINES
    pub(crate) wheel_scroll_lines: Cell<u32>,
}

impl WindowsSystemSettings {
    pub(crate) fn new() -> Self {
        let mut settings = Self::default();
        settings.init();
        settings
    }

    fn init(&mut self) {
        self.mouse_wheel_settings.update();
    }

    pub(crate) fn update(&self, wparam: usize) {
        match SYSTEM_PARAMETERS_INFO_ACTION(wparam as u32) {
            SPI_GETWHEELSCROLLLINES | SPI_GETWHEELSCROLLCHARS => self.update_mouse_wheel_settings(),
            _ => {}
        }
    }

    fn update_mouse_wheel_settings(&self) {
        self.mouse_wheel_settings.update();
    }
}

impl MouseWheelSettings {
    fn update(&self) {
        self.update_wheel_scroll_chars();
        self.update_wheel_scroll_lines();
    }

    fn update_wheel_scroll_chars(&self) {
        let mut value = c_uint::default();
        let result = unsafe {
            SystemParametersInfoW(
                SPI_GETWHEELSCROLLCHARS,
                0,
                Some((&mut value) as *mut c_uint as *mut c_void),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
            )
        };

        if result.log_err() != None && self.wheel_scroll_chars.get() != value {
            self.wheel_scroll_chars.set(value);
        }
    }

    fn update_wheel_scroll_lines(&self) {
        let mut value = c_uint::default();
        let result = unsafe {
            SystemParametersInfoW(
                SPI_GETWHEELSCROLLLINES,
                0,
                Some((&mut value) as *mut c_uint as *mut c_void),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
            )
        };

        if result.log_err() != None && self.wheel_scroll_lines.get() != value {
            self.wheel_scroll_lines.set(value);
        }
    }
}
