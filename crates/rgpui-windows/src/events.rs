//! Windows 消息事件处理
//!
//! 本模块处理所有 Windows 窗口消息事件，包括：
//! - 鼠标事件（点击、移动、滚轮）
//! - 键盘事件（按键、IME 输入）
//! - 窗口事件（大小变化、移动、DPI 变化）
//! - 自定义 GPUI 消息

use std::{rc::Rc, sync::atomic::Ordering};

use ::rgpui::util::ResultExt;
use anyhow::Context as _;
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::SystemServices::*,
        UI::{
            Controls::*,
            HiDpi::*,
            Input::{Ime::*, KeyboardAndMouse::*},
            WindowsAndMessaging::*,
        },
    },
    core::PCWSTR,
};

use crate::*;
use rgpui::*;

/// 自定义消息：光标样式改变
pub(crate) const WM_GPUI_CURSOR_STYLE_CHANGED: u32 = WM_USER + 1;
/// 自定义消息：关闭一个窗口
pub(crate) const WM_GPUI_CLOSE_ONE_WINDOW: u32 = WM_USER + 2;
/// 自定义消息：主线程任务已分发
pub(crate) const WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD: u32 = WM_USER + 3;
/// 自定义消息：停靠菜单操作
pub(crate) const WM_GPUI_DOCK_MENU_ACTION: u32 = WM_USER + 4;
/// 自定义消息：强制更新窗口
pub(crate) const WM_GPUI_FORCE_UPDATE_WINDOW: u32 = WM_USER + 5;
/// 自定义消息：键盘布局改变
pub(crate) const WM_GPUI_KEYBOARD_LAYOUT_CHANGED: u32 = WM_USER + 6;
/// 自定义消息：GPU 设备丢失
pub(crate) const WM_GPUI_GPU_DEVICE_LOST: u32 = WM_USER + 7;
/// 自定义消息：按键按下（用于快捷键加速）
pub(crate) const WM_GPUI_KEYDOWN: u32 = WM_USER + 8;

/// 托盘图标消息
pub(crate) const WM_GPUI_TRAY_ICON: u32 = WM_USER + 9;

const SIZE_MOVE_LOOP_TIMER_ID: usize = 1;

impl WindowsWindowInner {
    /// 处理 Windows 窗口消息
    ///
    /// 根据消息类型分发到对应的处理函数
    ///
    /// # 参数
    /// * `handle` - 窗口句柄
    /// * `msg` - 消息 ID
    /// * `wparam` - 消息的 wParam 参数
    /// * `lparam` - 消息的 lParam 参数
    ///
    /// # 返回
    /// 返回消息处理结果
    pub(crate) fn handle_msg(
        self: &Rc<Self>,
        handle: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let handled = match msg {
            // 当 mouse_passthrough=true（穿透模式）时，不激活窗口，
            // 返回 MA_NOACTIVATE 让 Windows 继续发送鼠标消息到底层窗口。
            // 穿透模式下激活窗口会抢走焦点，导致底层窗口无法响应点击。
            WM_MOUSEACTIVATE => {
                if self.state.mouse_passthrough.get() {
                    Some(MA_NOACTIVATE as isize)
                } else {
                    unsafe { SetActiveWindow(handle).ok() };
                    None
                }
            }
            WM_ACTIVATE => self.handle_activate_msg(wparam),
            WM_CREATE => self.handle_create_msg(handle),
            WM_MOVE => self.handle_move_msg(handle, lparam),
            WM_SIZE => self.handle_size_msg(wparam, lparam),
            WM_GETMINMAXINFO => self.handle_get_min_max_info_msg(lparam),
            WM_ENTERSIZEMOVE | WM_ENTERMENULOOP => self.handle_size_move_loop(handle),
            WM_EXITSIZEMOVE | WM_EXITMENULOOP => self.handle_size_move_loop_exit(handle),
            WM_TIMER => self.handle_timer_msg(handle, wparam),
            WM_NCCALCSIZE => self.handle_calc_client_size(handle, wparam, lparam),
            WM_DPICHANGED => self.handle_dpi_changed_msg(handle, wparam, lparam),
            WM_DISPLAYCHANGE => self.handle_display_change_msg(handle),
            WM_NCHITTEST => self.handle_hit_test_msg(handle, lparam),
            WM_PAINT => self.handle_paint_msg(handle),
            WM_CLOSE => self.handle_close_msg(),
            WM_DESTROY => self.handle_destroy_msg(handle),
            WM_MOUSEMOVE => self.handle_mouse_move_msg(handle, lparam, wparam),
            WM_MOUSELEAVE | WM_NCMOUSELEAVE => self.handle_mouse_leave_msg(),
            WM_NCMOUSEMOVE => self.handle_nc_mouse_move_msg(handle, lparam),
            // 将双击视为第二次单击，因为我们自行跟踪双击
            // 如果不与任何元素交互，将回退到 Windows 默认行为
            // 即切换窗口是否最大化
            WM_NCLBUTTONDBLCLK | WM_NCLBUTTONDOWN => {
                self.handle_nc_mouse_down_msg(handle, MouseButton::Left, wparam, lparam)
            }
            WM_NCRBUTTONDOWN => {
                self.handle_nc_mouse_down_msg(handle, MouseButton::Right, wparam, lparam)
            }
            WM_NCMBUTTONDOWN => {
                self.handle_nc_mouse_down_msg(handle, MouseButton::Middle, wparam, lparam)
            }
            WM_NCLBUTTONUP => {
                self.handle_nc_mouse_up_msg(handle, MouseButton::Left, wparam, lparam)
            }
            WM_NCRBUTTONUP => {
                self.handle_nc_mouse_up_msg(handle, MouseButton::Right, wparam, lparam)
            }
            WM_NCMBUTTONUP => {
                self.handle_nc_mouse_up_msg(handle, MouseButton::Middle, wparam, lparam)
            }
            WM_LBUTTONDOWN => self.handle_mouse_down_msg(handle, MouseButton::Left, lparam),
            WM_RBUTTONDOWN => self.handle_mouse_down_msg(handle, MouseButton::Right, lparam),
            WM_MBUTTONDOWN => self.handle_mouse_down_msg(handle, MouseButton::Middle, lparam),
            WM_XBUTTONDOWN => {
                self.handle_xbutton_msg(handle, wparam, lparam, Self::handle_mouse_down_msg)
            }
            WM_LBUTTONUP => self.handle_mouse_up_msg(handle, MouseButton::Left, lparam),
            WM_RBUTTONUP => self.handle_mouse_up_msg(handle, MouseButton::Right, lparam),
            WM_MBUTTONUP => self.handle_mouse_up_msg(handle, MouseButton::Middle, lparam),
            WM_XBUTTONUP => {
                self.handle_xbutton_msg(handle, wparam, lparam, Self::handle_mouse_up_msg)
            }
            WM_MOUSEWHEEL => self.handle_mouse_wheel_msg(handle, wparam, lparam),
            WM_MOUSEHWHEEL => self.handle_mouse_horizontal_wheel_msg(handle, wparam, lparam),
            WM_SYSKEYUP => self.handle_syskeyup_msg(wparam, lparam),
            WM_KEYUP => self.handle_keyup_msg(wparam, lparam),
            WM_GPUI_KEYDOWN => self.handle_keydown_msg(wparam, lparam),
            WM_CHAR => self.handle_char_msg(wparam),
            WM_IME_STARTCOMPOSITION => self.handle_ime_position(handle),
            WM_IME_COMPOSITION => self.handle_ime_composition(handle, lparam),
            WM_SETCURSOR => self.handle_set_cursor(handle, lparam),
            WM_SETTINGCHANGE => self.handle_system_settings_changed(handle, wparam, lparam),
            WM_INPUTLANGCHANGE => self.handle_input_language_changed(),
            WM_SHOWWINDOW => self.handle_window_visibility_changed(handle, wparam),
            WM_GPUI_CURSOR_STYLE_CHANGED => self.handle_cursor_changed(lparam),
            WM_GPUI_FORCE_UPDATE_WINDOW => self.draw_window(handle, true),
            WM_GPUI_GPU_DEVICE_LOST => self.handle_device_lost(lparam),
            DM_POINTERHITTEST => self.handle_dm_pointer_hit_test(wparam),
            WM_GETOBJECT => self.handle_wm_getobject(wparam, lparam),
            _ => None,
        };
        if let Some(n) = handled {
            LRESULT(n)
        } else {
            unsafe { DefWindowProcW(handle, msg, wparam, lparam) }
        }
    }

    fn handle_move_msg(&self, handle: HWND, lparam: LPARAM) -> Option<isize> {
        let origin = logical_point(
            lparam.signed_loword() as f32,
            lparam.signed_hiword() as f32,
            self.state.scale_factor.get(),
        );
        self.state.origin.set(origin);
        let size = self.state.logical_size.get();
        let center_x = origin.x.as_f32() + size.width.as_f32() / 2.;
        let center_y = origin.y.as_f32() + size.height.as_f32() / 2.;
        let monitor_bounds = self.state.display.get().bounds();
        if center_x < monitor_bounds.left().as_f32()
            || center_x > monitor_bounds.right().as_f32()
            || center_y < monitor_bounds.top().as_f32()
            || center_y > monitor_bounds.bottom().as_f32()
        {
            // 窗口中心可能已移动到另一个显示器
            let monitor = unsafe { MonitorFromWindow(handle, MONITOR_DEFAULTTONULL) };
            // 最小化窗口时也会触发此事件，此时
            // monitor 无效，我们不做任何处理
            if !monitor.is_invalid() && self.state.display.get().handle != monitor {
                // 如果只有一个显示器，我们将获得相同的显示器
                self.state.display.set(WindowsDisplay::new(
                    WindowsDisplay::display_id_for_monitor(monitor),
                )?);
            }
        }
        if let Some(mut callback) = self.state.callbacks.moved.take() {
            callback();
            self.state.callbacks.moved.set(Some(callback));
        }
        Some(0)
    }

    fn handle_get_min_max_info_msg(&self, lparam: LPARAM) -> Option<isize> {
        let min_size = self.state.min_size?;
        let scale_factor = self.state.scale_factor.get();
        let boarder_offset = &self.state.border_offset;

        unsafe {
            let minmax_info = &mut *(lparam.0 as *mut MINMAXINFO);
            minmax_info.ptMinTrackSize.x = min_size.width.scale(scale_factor).as_f32() as i32
                + boarder_offset.width_offset.get();
            minmax_info.ptMinTrackSize.y = min_size.height.scale(scale_factor).as_f32() as i32
                + boarder_offset.height_offset.get();
        }
        Some(0)
    }

    fn handle_size_msg(&self, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        // 窗口最小化时不要调整渲染器大小，但记录它已被最小化，以便
        // 恢复时可以通过 `update_drawable_size_even_if_unchanged` 重新创建交换链
        if wparam.0 == SIZE_MINIMIZED as usize {
            self.state
                .restore_from_minimized
                .set(self.state.callbacks.request_frame.take());
            return Some(0);
        }

        let width = lparam.loword().max(1) as i32;
        let height = lparam.hiword().max(1) as i32;
        let new_size = size(DevicePixels(width), DevicePixels(height));

        let scale_factor = self.state.scale_factor.get();
        let mut should_resize_renderer = false;
        if let Some(restore_from_minimized) = self.state.restore_from_minimized.take() {
            self.state
                .callbacks
                .request_frame
                .set(Some(restore_from_minimized));
        } else {
            should_resize_renderer = true;
        }

        self.handle_size_change(new_size, scale_factor, should_resize_renderer);
        Some(0)
    }

    /// 处理窗口大小变化
    ///
    /// # 参数
    /// * `device_size` - 设备像素大小
    /// * `scale_factor` - 缩放因子
    /// * `should_resize_renderer` - 是否应该调整渲染器大小
    fn handle_size_change(
        &self,
        device_size: Size<DevicePixels>,
        scale_factor: f32,
        should_resize_renderer: bool,
    ) {
        let new_logical_size = device_size.to_pixels(scale_factor);

        self.state.logical_size.set(new_logical_size);
        if should_resize_renderer
            && let Err(e) = self.state.renderer.borrow_mut().resize(device_size)
        {
            log::error!("Failed to resize renderer, invalidating devices: {}", e);
            self.state
                .invalidate_devices
                .store(true, std::sync::atomic::Ordering::Release);
        }
        if let Some(mut callback) = self.state.callbacks.resize.take() {
            callback(new_logical_size, scale_factor);
            self.state.callbacks.resize.set(Some(callback));
        }
    }

    fn handle_size_move_loop(&self, handle: HWND) -> Option<isize> {
        unsafe {
            let ret = SetTimer(
                Some(handle),
                SIZE_MOVE_LOOP_TIMER_ID,
                USER_TIMER_MINIMUM,
                None,
            );
            if ret == 0 {
                log::error!(
                    "unable to create timer: {}",
                    std::io::Error::last_os_error()
                );
            }
        }
        None
    }

    fn handle_size_move_loop_exit(&self, handle: HWND) -> Option<isize> {
        unsafe {
            KillTimer(Some(handle), SIZE_MOVE_LOOP_TIMER_ID).log_err();
        }
        None
    }

    fn handle_timer_msg(&self, handle: HWND, wparam: WPARAM) -> Option<isize> {
        if wparam.0 == SIZE_MOVE_LOOP_TIMER_ID {
            let mut runnables = self.main_receiver.clone().try_iter();
            while let Some(Ok(runnable)) = runnables.next() {
                WindowsDispatcher::execute_runnable(runnable);
            }
            self.handle_paint_msg(handle)
        } else {
            None
        }
    }

    fn handle_paint_msg(&self, handle: HWND) -> Option<isize> {
        self.draw_window(handle, false)
    }

    /// 处理关闭消息
    ///
    /// 调用 should_close 回调决定是否允许关闭
    ///
    /// # 返回
    /// 如果允许关闭返回 None，否则返回 Some(0)
    fn handle_close_msg(&self) -> Option<isize> {
        let mut callback = self.state.callbacks.should_close.take()?;
        let should_close = callback();
        self.state.callbacks.should_close.set(Some(callback));
        if should_close { None } else { Some(0) }
    }

    /// 处理窗口销毁消息
    ///
    /// 触发关闭回调，如果是模态对话框则重新启用父窗口
    ///
    /// # 参数
    /// * `handle` - 窗口句柄
    fn handle_destroy_msg(&self, handle: HWND) -> Option<isize> {
        let callback = { self.state.callbacks.close.take() };
        // 如果是模态对话框，重新启用父窗口
        if let Some(parent_hwnd) = self.parent_hwnd {
            unsafe {
                let _ = EnableWindow(parent_hwnd, true);
                let _ = SetForegroundWindow(parent_hwnd);
            }
        }

        if let Some(callback) = callback {
            callback();
        }
        unsafe {
            PostMessageW(
                Some(self.platform_window_handle),
                WM_GPUI_CLOSE_ONE_WINDOW,
                WPARAM(self.validation_number),
                LPARAM(handle.0 as isize),
            )
            .log_err();
        }
        Some(0)
    }

    fn handle_mouse_move_msg(&self, handle: HWND, lparam: LPARAM, wparam: WPARAM) -> Option<isize> {
        self.start_tracking_mouse(handle, TME_LEAVE);
        self.restore_cursor_after_hide();

        let Some(mut func) = self.state.callbacks.input.take() else {
            return Some(1);
        };
        let scale_factor = self.state.scale_factor.get();

        let pressed_button = match MODIFIERKEYS_FLAGS(wparam.loword() as u32) {
            flags if flags.contains(MK_LBUTTON) => Some(MouseButton::Left),
            flags if flags.contains(MK_RBUTTON) => Some(MouseButton::Right),
            flags if flags.contains(MK_MBUTTON) => Some(MouseButton::Middle),
            flags if flags.contains(MK_XBUTTON1) => {
                Some(MouseButton::Navigate(NavigationDirection::Back))
            }
            flags if flags.contains(MK_XBUTTON2) => {
                Some(MouseButton::Navigate(NavigationDirection::Forward))
            }
            _ => None,
        };
        let x = lparam.signed_loword() as f32;
        let y = lparam.signed_hiword() as f32;
        let input = PlatformInput::MouseMove(MouseMoveEvent {
            position: logical_point(x, y, scale_factor),
            pressed_button,
            modifiers: current_modifiers(),
        });
        let handled = !func(input).propagate;
        self.state.callbacks.input.set(Some(func));

        if handled { Some(0) } else { Some(1) }
    }

    fn handle_mouse_leave_msg(&self) -> Option<isize> {
        self.state.hovered.set(false);
        // The next window's `WM_SETCURSOR` picks its own cursor, so we just clear
        // the flag for tight `is_cursor_visible()` semantics.
        self.state.cursor_visible.store(true, Ordering::Relaxed);
        if let Some(mut callback) = self.state.callbacks.hovered_status_change.take() {
            callback(false);
            self.state
                .callbacks
                .hovered_status_change
                .set(Some(callback));
        }

        Some(0)
    }

    fn handle_syskeyup_msg(&self, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        let input = handle_key_event(wparam, lparam, &self.state, |keystroke, _| {
            PlatformInput::KeyUp(KeyUpEvent { keystroke })
        })?;
        let mut func = self.state.callbacks.input.take()?;

        func(input);
        self.state.callbacks.input.set(Some(func));

        // 始终返回 0 表示消息已处理，以便正确处理 `ModifiersChanged` 事件
        Some(0)
    }

    // 已知 bug：无法触发 `ctrl-shift-0`。参见：
    // https://superuser.com/questions/1455762/ctrl-shift-number-key-combination-has-stopped-working-for-a-few-numbers
    fn handle_keydown_msg(&self, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        let Some(input) = handle_key_event(
            wparam,
            lparam,
            &self.state,
            |keystroke, prefer_character_input| {
                PlatformInput::KeyDown(KeyDownEvent {
                    keystroke,
                    is_held: lparam.0 & (0x1 << 30) > 0,
                    prefer_character_input,
                })
            },
        ) else {
            return Some(1);
        };

        let Some(mut func) = self.state.callbacks.input.take() else {
            return Some(1);
        };

        let handled = !func(input).propagate;

        self.state.callbacks.input.set(Some(func));

        if handled { Some(0) } else { Some(1) }
    }

    fn handle_keyup_msg(&self, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        let Some(input) = handle_key_event(wparam, lparam, &self.state, |keystroke, _| {
            PlatformInput::KeyUp(KeyUpEvent { keystroke })
        }) else {
            return Some(1);
        };

        let Some(mut func) = self.state.callbacks.input.take() else {
            return Some(1);
        };

        let handled = !func(input).propagate;
        self.state.callbacks.input.set(Some(func));

        if handled { Some(0) } else { Some(1) }
    }

    fn handle_char_msg(&self, wparam: WPARAM) -> Option<isize> {
        let input = self.parse_char_message(wparam)?;
        self.with_input_handler(|input_handler| {
            input_handler.replace_text_in_range(None, &input);
        });

        Some(0)
    }

    fn handle_mouse_down_msg(
        &self,
        handle: HWND,
        button: MouseButton,
        lparam: LPARAM,
    ) -> Option<isize> {
        unsafe { SetCapture(handle) };

        let Some(mut func) = self.state.callbacks.input.take() else {
            return Some(1);
        };
        let x = lparam.signed_loword();
        let y = lparam.signed_hiword();
        let physical_point = point(DevicePixels(x as i32), DevicePixels(y as i32));
        let click_count = self.state.click_state.update(button, physical_point);
        let scale_factor = self.state.scale_factor.get();

        let input = PlatformInput::MouseDown(MouseDownEvent {
            button,
            position: logical_point(x as f32, y as f32, scale_factor),
            modifiers: current_modifiers(),
            click_count,
            first_mouse: false,
        });
        let handled = !func(input).propagate;
        self.state.callbacks.input.set(Some(func));

        if handled { Some(0) } else { Some(1) }
    }

    fn handle_mouse_up_msg(
        &self,
        _handle: HWND,
        button: MouseButton,
        lparam: LPARAM,
    ) -> Option<isize> {
        unsafe { ReleaseCapture().log_err() };

        let Some(mut func) = self.state.callbacks.input.take() else {
            return Some(1);
        };
        let x = lparam.signed_loword() as f32;
        let y = lparam.signed_hiword() as f32;
        let click_count = self.state.click_state.current_count.get();
        let scale_factor = self.state.scale_factor.get();

        let input = PlatformInput::MouseUp(MouseUpEvent {
            button,
            position: logical_point(x, y, scale_factor),
            modifiers: current_modifiers(),
            click_count,
        });
        let handled = !func(input).propagate;
        self.state.callbacks.input.set(Some(func));

        if handled { Some(0) } else { Some(1) }
    }

    fn handle_xbutton_msg(
        &self,
        handle: HWND,
        wparam: WPARAM,
        lparam: LPARAM,
        handler: impl Fn(&Self, HWND, MouseButton, LPARAM) -> Option<isize>,
    ) -> Option<isize> {
        let nav_dir = match wparam.hiword() {
            XBUTTON1 => NavigationDirection::Back,
            XBUTTON2 => NavigationDirection::Forward,
            _ => return Some(1),
        };
        handler(self, handle, MouseButton::Navigate(nav_dir), lparam)
    }

    fn handle_mouse_wheel_msg(
        &self,
        handle: HWND,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<isize> {
        let modifiers = current_modifiers();

        let Some(mut func) = self.state.callbacks.input.take() else {
            return Some(1);
        };
        let scale_factor = self.state.scale_factor.get();
        let wheel_scroll_amount = match modifiers.shift {
            true => self
                .system_settings()
                .mouse_wheel_settings
                .wheel_scroll_chars
                .get(),
            false => self
                .system_settings()
                .mouse_wheel_settings
                .wheel_scroll_lines
                .get(),
        };

        let wheel_distance =
            (wparam.signed_hiword() as f32 / WHEEL_DELTA as f32) * wheel_scroll_amount as f32;
        let mut cursor_point = POINT {
            x: lparam.signed_loword().into(),
            y: lparam.signed_hiword().into(),
        };
        unsafe { ScreenToClient(handle, &mut cursor_point).ok().log_err() };
        let input = PlatformInput::ScrollWheel(ScrollWheelEvent {
            position: logical_point(cursor_point.x as f32, cursor_point.y as f32, scale_factor),
            delta: ScrollDelta::Lines(match modifiers.shift {
                true => Point {
                    x: wheel_distance,
                    y: 0.0,
                },
                false => Point {
                    y: wheel_distance,
                    x: 0.0,
                },
            }),
            modifiers,
            touch_phase: TouchPhase::Moved,
        });
        let handled = !func(input).propagate;
        self.state.callbacks.input.set(Some(func));

        if handled { Some(0) } else { Some(1) }
    }

    fn handle_mouse_horizontal_wheel_msg(
        &self,
        handle: HWND,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<isize> {
        let Some(mut func) = self.state.callbacks.input.take() else {
            return Some(1);
        };
        let scale_factor = self.state.scale_factor.get();
        let wheel_scroll_chars = self
            .system_settings()
            .mouse_wheel_settings
            .wheel_scroll_chars
            .get();

        let wheel_distance =
            (-wparam.signed_hiword() as f32 / WHEEL_DELTA as f32) * wheel_scroll_chars as f32;
        let mut cursor_point = POINT {
            x: lparam.signed_loword().into(),
            y: lparam.signed_hiword().into(),
        };
        unsafe { ScreenToClient(handle, &mut cursor_point).ok().log_err() };
        let event = PlatformInput::ScrollWheel(ScrollWheelEvent {
            position: logical_point(cursor_point.x as f32, cursor_point.y as f32, scale_factor),
            delta: ScrollDelta::Lines(Point {
                x: wheel_distance,
                y: 0.0,
            }),
            modifiers: current_modifiers(),
            touch_phase: TouchPhase::Moved,
        });
        let handled = !func(event).propagate;
        self.state.callbacks.input.set(Some(func));

        if handled { Some(0) } else { Some(1) }
    }

    fn retrieve_caret_position(&self) -> Option<POINT> {
        self.with_input_handler_and_scale_factor(|input_handler, scale_factor| {
            let caret_range = input_handler.selected_text_range(false)?;
            let caret_position = input_handler.bounds_for_range(caret_range.range)?;
            Some(POINT {
                // 逻辑坐标转物理坐标
                x: (caret_position.origin.x.as_f32() * scale_factor) as i32,
                y: (caret_position.origin.y.as_f32() * scale_factor) as i32
                    + ((caret_position.size.height.as_f32() * scale_factor) as i32 / 2),
            })
        })
    }

    fn handle_ime_position(&self, handle: HWND) -> Option<isize> {
        if let Some(caret_position) = self.retrieve_caret_position() {
            self.update_ime_position(handle, caret_position);
        }
        Some(0)
    }

    pub(crate) fn update_ime_position(&self, handle: HWND, caret_position: POINT) {
        let Some(ctx) = ImeContext::get(handle) else {
            return;
        };
        unsafe {
            ImmSetCompositionWindow(
                *ctx,
                &COMPOSITIONFORM {
                    dwStyle: CFS_POINT,
                    ptCurrentPos: caret_position,
                    ..Default::default()
                },
            )
            .ok()
            .log_err();

            ImmSetCandidateWindow(
                *ctx,
                &CANDIDATEFORM {
                    dwStyle: CFS_CANDIDATEPOS,
                    ptCurrentPos: caret_position,
                    ..Default::default()
                },
            )
            .ok()
            .log_err();
        }
    }

    fn update_ime_enabled(&self, handle: HWND) {
        let ime_enabled = self
            .with_input_handler(|input_handler| input_handler.query_accepts_text_input())
            .unwrap_or(false);
        if ime_enabled == self.state.ime_enabled.get() {
            return;
        }
        self.state.ime_enabled.set(ime_enabled);
        unsafe {
            if ime_enabled {
                ImmAssociateContextEx(handle, HIMC::default(), IACE_DEFAULT)
                    .ok()
                    .log_err();
            } else {
                if let Some(ctx) = ImeContext::get(handle) {
                    ImmNotifyIME(*ctx, NI_COMPOSITIONSTR, CPS_COMPLETE, 0)
                        .ok()
                        .log_err();
                }
                ImmAssociateContextEx(handle, HIMC::default(), 0)
                    .ok()
                    .log_err();
            }
        }
    }

    fn handle_ime_composition(&self, handle: HWND, lparam: LPARAM) -> Option<isize> {
        let ctx = ImeContext::get(handle)?;
        self.handle_ime_composition_inner(*ctx, lparam)
    }

    fn handle_ime_composition_inner(&self, ctx: HIMC, lparam: LPARAM) -> Option<isize> {
        let lparam = lparam.0 as u32;
        if lparam == 0 {
            // 日文 IME 可能发送 lparam = 0 的消息，表示没有组合字符串
            self.with_input_handler(|input_handler| {
                input_handler.replace_text_in_range(None, "");
            })?;
            Some(0)
        } else {
            if lparam & GCS_RESULTSTR.0 > 0 {
                let comp_result = parse_ime_composition_string(ctx, GCS_RESULTSTR)?;
                self.with_input_handler(|input_handler| {
                    input_handler
                        .replace_text_in_range(None, &String::from_utf16_lossy(&comp_result));
                })?;
            }
            if lparam & GCS_COMPSTR.0 > 0 {
                let comp_string = parse_ime_composition_string(ctx, GCS_COMPSTR)?;
                let caret_pos =
                    (!comp_string.is_empty() && lparam & GCS_CURSORPOS.0 > 0).then(|| {
                        let cursor_pos = retrieve_composition_cursor_position(ctx);
                        let pos = if should_use_ime_cursor_position(ctx, cursor_pos) {
                            cursor_pos
                        } else {
                            comp_string.len()
                        };
                        pos..pos
                    });
                self.with_input_handler(|input_handler| {
                    input_handler.replace_and_mark_text_in_range(
                        None,
                        &String::from_utf16_lossy(&comp_string),
                        caret_pos,
                    );
                })?;
            }
            if lparam & (GCS_RESULTSTR.0 | GCS_COMPSTR.0) > 0 {
                return Some(0);
            }

            // 目前我们不关心其他内容
            None
        }
    }

    fn handle_calc_client_size(
        &self,
        handle: HWND,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<isize> {
        // 无边框窗口：整个窗口都是客户区，没有非客户区
        if self.client_decorations {
            return Some(0);
        }

        if self.state.titlebar_visible.get() || self.state.is_fullscreen() || wparam.0 == 0 {
            return None;
        }

        unsafe {
            let params = lparam.0 as *mut NCCALCSIZE_PARAMS;
            let saved_top = (*params).rgrc[0].top;
            let result = DefWindowProcW(handle, WM_NCCALCSIZE, wparam, lparam);
            (*params).rgrc[0].top = saved_top;
            if self.state.is_maximized() {
                let dpi = GetDpiForWindow(handle);
                (*params).rgrc[0].top += get_frame_thicknessx(dpi);
            }
            Some(result.0 as isize)
        }
    }

    fn handle_activate_msg(self: &Rc<Self>, wparam: WPARAM) -> Option<isize> {
        let activated = wparam.loword() > 0;
        let events = self
            .state
            .a11y
            .try_borrow_mut()
            .ok()
            .and_then(|mut a11y| a11y.as_mut()?.adapter.update_window_focus_state(activated));
        if let Some(events) = events {
            events.raise();
        }
        let this = self.clone();

        if !activated {
            this.state.cursor_visible.store(true, Ordering::Relaxed);
        }

        // 当窗口被激活（获得焦点）时，重置修饰键跟踪状态
        // 这修复了 Alt-Tab 离开再返回时留下过期修饰键状态
        // （尤其是 Alt 键）的问题，因为 Windows 不总是向失去焦点的窗口发送按键释放事件
        if activated {
            this.state.last_reported_modifiers.set(None);
            this.state.last_reported_capslock.set(None);

            if let Some(mut func) = this.state.callbacks.input.take() {
                let input = PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                    modifiers: current_modifiers(),
                    capslock: current_capslock(),
                });
                func(input);
                this.state.callbacks.input.set(Some(func));
            }
        }

        self.executor
            .spawn(async move {
                if let Some(mut func) = this.state.callbacks.active_status_change.take() {
                    func(activated);
                    this.state.callbacks.active_status_change.set(Some(func));
                }
            })
            .detach();

        None
    }

    fn handle_create_msg(&self, handle: HWND) -> Option<isize> {
        if !self.state.titlebar_visible.get() {
            notify_frame_changed(handle);
            Some(0)
        } else {
            None
        }
    }

    fn handle_dpi_changed_msg(
        &self,
        handle: HWND,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<isize> {
        let new_dpi = wparam.loword() as f32;

        let is_maximized = self.state.is_maximized();
        let new_scale_factor = new_dpi / USER_DEFAULT_SCREEN_DPI as f32;
        self.state.scale_factor.set(new_scale_factor);
        self.state.border_offset.update(handle).log_err();

        self.state
            .direct_manipulation
            .set_scale_factor(new_scale_factor);

        if is_maximized {
            // 获取新 DPI 下的显示器及其工作区域
            let monitor = unsafe { MonitorFromWindow(handle, MONITOR_DEFAULTTONEAREST) };
            let mut monitor_info: MONITORINFO = unsafe { std::mem::zeroed() };
            monitor_info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if unsafe { GetMonitorInfoW(monitor, &mut monitor_info) }.as_bool() {
                let work_area = monitor_info.rcWork;
                let width = work_area.right - work_area.left;
                let height = work_area.bottom - work_area.top;

                // 更新窗口大小以匹配新的显示器工作区域
                // 这将触发 WM_SIZE 来处理大小变化
                unsafe {
                    SetWindowPos(
                        handle,
                        None,
                        work_area.left,
                        work_area.top,
                        width,
                        height,
                        SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                    )
                    .context("unable to set maximized window position after dpi has changed")
                    .log_err();
                }

                // SetWindowPos 在某些情况下可能不会为最大化窗口发送 WM_SIZE，
                // 所以我们手动更新大小以确保正确渲染
                let device_size = size(DevicePixels(width), DevicePixels(height));
                self.handle_size_change(device_size, new_scale_factor, true);
            }
        } else {
            // 对于非最大化窗口，使用系统建议的 RECT
            let rect = unsafe { &*(lparam.0 as *const RECT) };
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            // 这将在此函数返回之前立即发送 `WM_SIZE` 和 `WM_MOVE`
            // 新大小在 `WM_SIZE` 中处理
            unsafe {
                SetWindowPos(
                    handle,
                    None,
                    rect.left,
                    rect.top,
                    width,
                    height,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )
                .context("unable to set window position after dpi has changed")
                .log_err();
            }
        }

        Some(0)
    }

    fn handle_display_change_msg(&self, handle: HWND) -> Option<isize> {
        let new_monitor = unsafe { MonitorFromWindow(handle, MONITOR_DEFAULTTONULL) };
        if new_monitor.is_invalid() {
            log::error!("No monitor detected!");
            return None;
        }
        let new_display = WindowsDisplay::new(WindowsDisplay::display_id_for_monitor(new_monitor))?;
        self.state.display.set(new_display);
        Some(0)
    }

    fn handle_hit_test_msg(&self, handle: HWND, lparam: LPARAM) -> Option<isize> {
        if self.state.is_fullscreen() {
            return None;
        }

        let callback = self.state.callbacks.hit_test_window_control.take();
        let drag_area = if let Some(mut callback) = callback {
            let area = callback();
            self.state
                .callbacks
                .hit_test_window_control
                .set(Some(callback));
            area.and_then(|area| match area {
                WindowControlArea::Drag if self.is_movable => Some(HTCAPTION as _),
                WindowControlArea::Drag => None,
                WindowControlArea::Close => Some(HTCLOSE as _),
                WindowControlArea::Max => Some(HTMAXBUTTON as _),
                WindowControlArea::Min => Some(HTMINBUTTON as _),
            })
        } else {
            None
        };

        let dpi = unsafe { GetDpiForWindow(handle) };
        let frame_y = get_frame_thicknessx(dpi);
        let frame_x = get_frame_thicknessy(dpi);
        let mut cursor_point = POINT {
            x: lparam.signed_loword().into(),
            y: lparam.signed_hiword().into(),
        };

        unsafe { ScreenToClient(handle, &mut cursor_point).ok().log_err() };

        // 检查是否在边框调整大小区域
        let in_resize_area =
            if !self.state.is_maximized() && 0 <= cursor_point.y && cursor_point.y <= frame_y {
                Some(if cursor_point.x <= 0 {
                    HTTOPLEFT
                } else {
                    let mut rect = Default::default();
                    unsafe { GetWindowRect(handle, &mut rect) }.log_err();
                    let right = rect.right - rect.left - 1;
                    if right - 2 * frame_x <= cursor_point.x {
                        HTTOPRIGHT
                    } else {
                        HTTOP
                    }
                } as _)
            } else {
                None
            };

        // Ctrl 键按下时：始终允许拖动窗口，覆盖鼠标穿透设置
        // 使用 GetAsyncKeyState 而非 GetKeyState，因穿透窗口不接收焦点
        if unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) < 0 } {
            // 优先返回边框调整大小区域
            if let Some(hit) = in_resize_area {
                return Some(hit);
            }
            return Some(HTCAPTION as _);
        }

        // 穿透模式：边框和标题栏区域保持交互，其他区域穿透
        if self.state.mouse_passthrough.get() {
            // 优先返回边框调整大小区域
            if let Some(hit) = in_resize_area {
                return Some(hit);
            }

            // 如果使用系统标题栏，标题栏区域保持可拖动
            if self.state.titlebar_visible.get() {
                if cursor_point.y <= frame_y {
                    return Some(HTCAPTION as _);
                }
            }

            // 其他区域穿透
            return Some(HTTRANSPARENT as _);
        }

        if self.state.titlebar_visible.get() {
            // 系统绘制标题栏（WS_CAPTION），由 DefWindowProcW 处理命中测试
            // 包括关闭、最小化、最大化按钮和标题栏拖动
            // WS_EX_LAYERED 已在 set_titlebar_visible 中移除，不会误返回 HTTRANSPARENT
            return drag_area;
        }

        // 非穿透模式：返回边框调整大小区域或拖动区域
        if let Some(hit) = in_resize_area {
            return Some(hit);
        }

        // 优先使用自定义拖动区域
        if let Some(hit) = drag_area {
            return Some(hit);
        }

        // 客户区显式返回 HTCLIENT，防止 DefWindowProcW 对特殊窗口样式（如 Overlay）处理不正确
        Some(HTCLIENT as _)
    }

    fn handle_nc_mouse_move_msg(&self, handle: HWND, lparam: LPARAM) -> Option<isize> {
        self.start_tracking_mouse(handle, TME_LEAVE | TME_NONCLIENT);
        self.restore_cursor_after_hide();

        let mut func = self.state.callbacks.input.take()?;
        let scale_factor = self.state.scale_factor.get();

        let mut cursor_point = POINT {
            x: lparam.signed_loword().into(),
            y: lparam.signed_hiword().into(),
        };
        unsafe { ScreenToClient(handle, &mut cursor_point).ok().log_err() };
        let input = PlatformInput::MouseMove(MouseMoveEvent {
            position: logical_point(cursor_point.x as f32, cursor_point.y as f32, scale_factor),
            pressed_button: None,
            modifiers: current_modifiers(),
        });
        let handled = !func(input).propagate;
        self.state.callbacks.input.set(Some(func));

        if handled { Some(0) } else { None }
    }

    fn handle_nc_mouse_down_msg(
        &self,
        handle: HWND,
        button: MouseButton,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<isize> {
        // 先存储按钮状态（HTMINBUTTON/HTMAXBUTTON/HTCLOSE），
        // 确保后续 WM_NCLBUTTONUP 无论应用层是否消费事件都能正确匹配。
        // 应用层可能消费事件（如 TitleBar 上的点击），
        // 所以不能依赖 "应用层未处理 -> 才存储" 的逻辑。
        let nc_action = if button == MouseButton::Left {
            match wparam.0 as u32 {
                HTMINBUTTON | HTMAXBUTTON | HTCLOSE => {
                    self.state.nc_button_pressed.set(Some(wparam.0 as u32));
                    true
                }
                _ => false,
            }
        } else {
            false
        };

        // 仍向应用层分发鼠标按下事件（用于更新 UI 视觉效果等）
        if let Some(mut func) = self.state.callbacks.input.take() {
            let scale_factor = self.state.scale_factor.get();
            let mut cursor_point = POINT {
                x: lparam.signed_loword().into(),
                y: lparam.signed_hiword().into(),
            };
            unsafe { ScreenToClient(handle, &mut cursor_point).ok().log_err() };
            let physical_point = point(DevicePixels(cursor_point.x), DevicePixels(cursor_point.y));
            let click_count = self.state.click_state.update(button, physical_point);

            let input = PlatformInput::MouseDown(MouseDownEvent {
                button,
                position: logical_point(cursor_point.x as f32, cursor_point.y as f32, scale_factor),
                modifiers: current_modifiers(),
                click_count,
                first_mouse: false,
            });
            let _handled = !func(input).propagate;
            self.state.callbacks.input.set(Some(func));
        }

        // 对 NC 按钮（最小化/最大化/关闭）阻止默认窗口过程
        if nc_action { Some(0) } else { None }
    }

    fn handle_nc_mouse_up_msg(
        &self,
        handle: HWND,
        button: MouseButton,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<isize> {
        // 先检测按钮状态（在 down 时存储的），无论应用层是否消费鼠标释放事件都执行动作
        let last_pressed = self.state.nc_button_pressed.take();
        let button_action = if button == MouseButton::Left
            && let Some(last_pressed) = last_pressed
        {
            match (wparam.0 as u32, last_pressed) {
                (HTMINBUTTON, HTMINBUTTON) => {
                    unsafe { ShowWindowAsync(handle, SW_MINIMIZE).ok().log_err() };
                    true
                }
                (HTMAXBUTTON, HTMAXBUTTON) => {
                    if self.state.is_maximized() {
                        unsafe { ShowWindowAsync(handle, SW_NORMAL).ok().log_err() };
                    } else {
                        unsafe { ShowWindowAsync(handle, SW_MAXIMIZE).ok().log_err() };
                    }
                    true
                }
                (HTCLOSE, HTCLOSE) => {
                    unsafe {
                        PostMessageW(Some(handle), WM_CLOSE, WPARAM::default(), LPARAM::default())
                            .log_err()
                    };
                    true
                }
                _ => false,
            }
        } else {
            false
        };

        // 仍向应用层分发鼠标释放事件
        if let Some(mut func) = self.state.callbacks.input.take() {
            let scale_factor = self.state.scale_factor.get();

            let mut cursor_point = POINT {
                x: lparam.signed_loword().into(),
                y: lparam.signed_hiword().into(),
            };
            unsafe { ScreenToClient(handle, &mut cursor_point).ok().log_err() };
            let input = PlatformInput::MouseUp(MouseUpEvent {
                button,
                position: logical_point(cursor_point.x as f32, cursor_point.y as f32, scale_factor),
                modifiers: current_modifiers(),
                click_count: 1,
            });
            let _handled = !func(input).propagate;
            self.state.callbacks.input.set(Some(func));
        }

        if button_action { Some(0) } else { None }
    }

    fn handle_cursor_changed(&self, lparam: LPARAM) -> Option<isize> {
        let had_cursor = self.state.current_cursor.get().is_some();

        self.state.current_cursor.set(if lparam.0 == 0 {
            None
        } else {
            Some(HCURSOR(lparam.0 as _))
        });

        if had_cursor != self.state.current_cursor.get().is_some() {
            unsafe { SetCursor(self.state.current_cursor.get()) };
        }

        Some(0)
    }

    fn handle_set_cursor(&self, handle: HWND, lparam: LPARAM) -> Option<isize> {
        if unsafe { !IsWindowEnabled(handle).as_bool() }
            || matches!(
                lparam.loword() as u32,
                HTLEFT
                    | HTRIGHT
                    | HTTOP
                    | HTTOPLEFT
                    | HTTOPRIGHT
                    | HTBOTTOM
                    | HTBOTTOMLEFT
                    | HTBOTTOMRIGHT
            )
        {
            return None;
        }
        let cursor = if self.state.cursor_visible.load(Ordering::Relaxed) {
            self.state.current_cursor.get()
        } else {
            None
        };
        unsafe {
            SetCursor(cursor);
        };
        Some(0)
    }

    fn handle_system_settings_changed(
        &self,
        handle: HWND,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<isize> {
        if wparam.0 != 0 {
            self.state.click_state.system_update(wparam.0);
            self.state.border_offset.update(handle).log_err();
            // 系统设置可能发出窗口消息，该消息想要获取 refcell self.state，所以先释放它

            self.system_settings().update(wparam.0);
        } else {
            self.handle_system_theme_changed(handle, lparam)?;
        };

        Some(0)
    }

    fn handle_system_theme_changed(&self, handle: HWND, lparam: LPARAM) -> Option<isize> {
        // lParam 是指向字符串的指针，表示包含系统参数的区域
        // 该参数指示了哪个系统参数被更改了
        let parameter = PCWSTR::from_raw(lparam.0 as _);
        if unsafe { !parameter.is_null() && !parameter.is_empty() }
            && let Some(parameter_string) = unsafe { parameter.to_string() }.log_err()
        {
            log::info!("System settings changed: {}", parameter_string);
            if parameter_string.as_str() == "ImmersiveColorSet" {
                let new_appearance = system_appearance()
                    .context("unable to get system appearance when handling ImmersiveColorSet")
                    .log_err()?;

                if new_appearance != self.state.appearance.get() {
                    self.state.appearance.set(new_appearance);
                    let mut callback = self.state.callbacks.appearance_changed.take()?;

                    callback();
                    self.state.callbacks.appearance_changed.set(Some(callback));
                    configure_dwm_dark_mode(handle, new_appearance);
                }
            }
        }
        Some(0)
    }

    fn handle_input_language_changed(&self) -> Option<isize> {
        unsafe {
            PostMessageW(
                Some(self.platform_window_handle),
                WM_GPUI_KEYBOARD_LAYOUT_CHANGED,
                WPARAM(self.validation_number),
                LPARAM(0),
            )
            .log_err();
        }
        Some(0)
    }

    fn handle_window_visibility_changed(&self, handle: HWND, wparam: WPARAM) -> Option<isize> {
        if wparam.0 == 1 {
            self.draw_window(handle, false);
        }
        None
    }

    fn handle_device_lost(&self, lparam: LPARAM) -> Option<isize> {
        let devices = lparam.0 as *const DirectXDevices;
        let devices = unsafe { &*devices };
        if let Err(err) = self
            .state
            .renderer
            .borrow_mut()
            .handle_device_lost(&devices)
        {
            panic!("Device lost: {err}");
        }
        // 确保设备丢失恢复后的第一次 `draw_window`（无论是来自
        // 强制的 WM_GPUI_FORCE_UPDATE_WINDOW 还是中间的 WM_PAINT）
        // 被视为强制渲染，这样既清除 `skip_draws` 又绕过视图缓存
        self.state.force_render_after_recovery.set(true);
        Some(0)
    }

    fn handle_dm_pointer_hit_test(&self, wparam: WPARAM) -> Option<isize> {
        self.state.direct_manipulation.on_pointer_hit_test(wparam);
        None
    }

    /// 绘制窗口内容
    ///
    /// # 参数
    /// * `handle` - 窗口句柄
    /// * `force_render` - 是否强制渲染
    #[inline]
    fn draw_window(&self, handle: HWND, force_render: bool) -> Option<isize> {
        let mut request_frame = self.state.callbacks.request_frame.take()?;

        self.state.direct_manipulation.update();

        let events = self.state.direct_manipulation.drain_events();
        if !events.is_empty() {
            if let Some(mut func) = self.state.callbacks.input.take() {
                for event in events {
                    func(event);
                }
                self.state.callbacks.input.set(Some(func));
            }
        }

        let force_render = force_render || self.state.force_render_after_recovery.take();
        if force_render {
            // 设备丢失恢复后重新启用绘制。强制渲染
            // 将使用新的图集纹理重建场景
            self.state.renderer.borrow_mut().mark_drawable();
        }
        request_frame(RequestFrameOptions {
            require_presentation: false,
            force_render,
        });

        self.state.callbacks.request_frame.set(Some(request_frame));
        self.update_ime_enabled(handle);
        unsafe { ValidateRect(Some(handle), None).ok().log_err() };

        Some(0)
    }

    /// 解析字符消息
    ///
    /// 处理 Unicode 代理对，将 WPARAM 转换为字符串
    ///
    /// # 参数
    /// * `wparam` - 包含字符代码的 WPARAM
    ///
    /// # 返回
    /// 返回解析后的字符串，如果是高代理对则返回 None 等待低代理对
    #[inline]
    fn parse_char_message(&self, wparam: WPARAM) -> Option<String> {
        let code_point = wparam.loword();

        // https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-3/#G2630
        match code_point {
            0xD800..=0xDBFF => {
                // 高代理对，等待低代理对
                self.state.pending_surrogate.set(Some(code_point));
                None
            }
            0xDC00..=0xDFFF => {
                if let Some(high_surrogate) = self.state.pending_surrogate.take() {
                    // 低代理对，与等待中的高代理对组合
                    String::from_utf16(&[high_surrogate, code_point]).ok()
                } else {
                    // 无效的低代理对，前面没有高代理对
                    log::warn!(
                        "Received low surrogate without a preceding high surrogate: {code_point:x}"
                    );
                    None
                }
            }
            _ => {
                self.state.pending_surrogate.set(None);
                char::from_u32(code_point as u32)
                    .filter(|c| !c.is_control())
                    .map(|c| c.to_string())
            }
        }
    }

    /// 立即清除隐藏标志并恢复光标
    fn restore_cursor_after_hide(&self) {
        if !self.state.cursor_visible.swap(true, Ordering::Relaxed) {
            unsafe {
                SetCursor(self.state.current_cursor.get());
            }
        }
    }

    fn start_tracking_mouse(&self, handle: HWND, flags: TRACKMOUSEEVENT_FLAGS) {
        if !self.state.hovered.get() {
            self.state.hovered.set(true);
            unsafe {
                TrackMouseEvent(&mut TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: flags,
                    hwndTrack: handle,
                    dwHoverTime: HOVER_DEFAULT,
                })
                .log_err()
            };
            if let Some(mut callback) = self.state.callbacks.hovered_status_change.take() {
                callback(true);
                self.state
                    .callbacks
                    .hovered_status_change
                    .set(Some(callback));
            }
        }
    }

    fn handle_wm_getobject(&self, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        let result = {
            let mut a11y = self.state.a11y.borrow_mut();
            let a11y = a11y.as_mut()?;
            a11y.adapter.handle_wm_getobject(
                accesskit_windows::WPARAM(wparam.0),
                accesskit_windows::LPARAM(lparam.0),
                &mut a11y.activation_handler,
            )?
        };
        let lresult: accesskit_windows::LRESULT = result.into();
        Some(lresult.0)
    }

    fn with_input_handler<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut PlatformInputHandler) -> R,
    {
        let mut input_handler = self.state.input_handler.take()?;
        let result = f(&mut input_handler);
        self.state.input_handler.set(Some(input_handler));
        Some(result)
    }

    fn with_input_handler_and_scale_factor<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut PlatformInputHandler, f32) -> Option<R>,
    {
        let mut input_handler = self.state.input_handler.take()?;
        let scale_factor = self.state.scale_factor.get();

        let result = f(&mut input_handler, scale_factor);
        self.state.input_handler.set(Some(input_handler));
        result
    }
}

struct ImeContext {
    hwnd: HWND,
    himc: HIMC,
}

impl ImeContext {
    fn get(hwnd: HWND) -> Option<Self> {
        let himc = unsafe { ImmGetContext(hwnd) };
        if himc.is_invalid() {
            return None;
        }
        Some(Self { hwnd, himc })
    }
}

impl std::ops::Deref for ImeContext {
    type Target = HIMC;
    fn deref(&self) -> &HIMC {
        &self.himc
    }
}

impl Drop for ImeContext {
    fn drop(&mut self) {
        unsafe {
            ImmReleaseContext(self.hwnd, self.himc).ok().log_err();
        }
    }
}

fn handle_key_event<F>(
    wparam: WPARAM,
    lparam: LPARAM,
    state: &WindowsWindowState,
    f: F,
) -> Option<PlatformInput>
where
    F: FnOnce(Keystroke, bool) -> PlatformInput,
{
    let virtual_key = VIRTUAL_KEY(wparam.loword());
    let modifiers = current_modifiers();

    match virtual_key {
        VK_SHIFT | VK_CONTROL | VK_MENU | VK_LMENU | VK_RMENU | VK_LWIN | VK_RWIN => {
            if state
                .last_reported_modifiers
                .get()
                .is_some_and(|prev_modifiers| prev_modifiers == modifiers)
            {
                return None;
            }
            state.last_reported_modifiers.set(Some(modifiers));
            Some(PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                modifiers,
                capslock: current_capslock(),
            }))
        }
        VK_PACKET => None,
        VK_CAPITAL => {
            let capslock = current_capslock();
            if state
                .last_reported_capslock
                .get()
                .is_some_and(|prev_capslock| prev_capslock == capslock)
            {
                return None;
            }
            state.last_reported_capslock.set(Some(capslock));
            Some(PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                modifiers,
                capslock,
            }))
        }
        vkey => {
            let keystroke = parse_normal_key(vkey, lparam, modifiers)?;
            Some(f(keystroke.0, keystroke.1))
        }
    }
}

fn parse_immutable(vkey: VIRTUAL_KEY) -> Option<String> {
    Some(
        match vkey {
            VK_SPACE => "space",
            VK_BACK => "backspace",
            VK_RETURN => "enter",
            VK_TAB => "tab",
            VK_UP => "up",
            VK_DOWN => "down",
            VK_RIGHT => "right",
            VK_LEFT => "left",
            VK_HOME => "home",
            VK_END => "end",
            VK_PRIOR => "pageup",
            VK_NEXT => "pagedown",
            VK_BROWSER_BACK => "back",
            VK_BROWSER_FORWARD => "forward",
            VK_ESCAPE => "escape",
            VK_INSERT => "insert",
            VK_DELETE => "delete",
            VK_APPS => "menu",
            VK_F1 => "f1",
            VK_F2 => "f2",
            VK_F3 => "f3",
            VK_F4 => "f4",
            VK_F5 => "f5",
            VK_F6 => "f6",
            VK_F7 => "f7",
            VK_F8 => "f8",
            VK_F9 => "f9",
            VK_F10 => "f10",
            VK_F11 => "f11",
            VK_F12 => "f12",
            VK_F13 => "f13",
            VK_F14 => "f14",
            VK_F15 => "f15",
            VK_F16 => "f16",
            VK_F17 => "f17",
            VK_F18 => "f18",
            VK_F19 => "f19",
            VK_F20 => "f20",
            VK_F21 => "f21",
            VK_F22 => "f22",
            VK_F23 => "f23",
            VK_F24 => "f24",
            _ => return None,
        }
        .to_string(),
    )
}

fn parse_normal_key(
    vkey: VIRTUAL_KEY,
    lparam: LPARAM,
    mut modifiers: Modifiers,
) -> Option<(Keystroke, bool)> {
    let (key_char, prefer_character_input) = process_key(vkey, lparam.hiword());

    let key = parse_immutable(vkey).or_else(|| {
        let scan_code = lparam.hiword() & 0xFF;
        get_keystroke_key(vkey, scan_code as u32, &mut modifiers)
    })?;

    Some((
        Keystroke {
            modifiers,
            key,
            key_char,
        },
        prefer_character_input,
    ))
}

fn process_key(vkey: VIRTUAL_KEY, scan_code: u16) -> (Option<String>, bool) {
    let mut keyboard_state = [0u8; 256];
    unsafe {
        if GetKeyboardState(&mut keyboard_state).is_err() {
            return (None, false);
        }
    }

    let mut buffer_c = [0u16; 8];
    let result_c = unsafe {
        ToUnicode(
            vkey.0 as u32,
            scan_code as u32,
            Some(&keyboard_state),
            &mut buffer_c,
            0x4,
        )
    };

    if result_c == 0 {
        return (None, false);
    }

    let c = &buffer_c[..result_c.unsigned_abs() as usize];
    let key_char = String::from_utf16(c)
        .ok()
        .filter(|s| !s.is_empty() && !s.chars().next().unwrap().is_control());

    if result_c < 0 {
        return (key_char, true);
    }

    if key_char.is_none() {
        return (None, false);
    }

    // Workaround for some bug that makes the compiler think keyboard_state is still zeroed out
    let keyboard_state = std::hint::black_box(keyboard_state);
    let ctrl_down = (keyboard_state[VK_CONTROL.0 as usize] & 0x80) != 0;
    let alt_down = (keyboard_state[VK_MENU.0 as usize] & 0x80) != 0;
    let win_down = (keyboard_state[VK_LWIN.0 as usize] & 0x80) != 0
        || (keyboard_state[VK_RWIN.0 as usize] & 0x80) != 0;

    let has_modifiers = ctrl_down || alt_down || win_down;
    if !has_modifiers {
        return (key_char, false);
    }

    let mut state_no_modifiers = keyboard_state;
    state_no_modifiers[VK_CONTROL.0 as usize] = 0;
    state_no_modifiers[VK_LCONTROL.0 as usize] = 0;
    state_no_modifiers[VK_RCONTROL.0 as usize] = 0;
    state_no_modifiers[VK_MENU.0 as usize] = 0;
    state_no_modifiers[VK_LMENU.0 as usize] = 0;
    state_no_modifiers[VK_RMENU.0 as usize] = 0;
    state_no_modifiers[VK_LWIN.0 as usize] = 0;
    state_no_modifiers[VK_RWIN.0 as usize] = 0;

    let mut buffer_c_no_modifiers = [0u16; 8];
    let result_c_no_modifiers = unsafe {
        ToUnicode(
            vkey.0 as u32,
            scan_code as u32,
            Some(&state_no_modifiers),
            &mut buffer_c_no_modifiers,
            0x4,
        )
    };

    let c_no_modifiers = &buffer_c_no_modifiers[..result_c_no_modifiers.unsigned_abs() as usize];
    (
        key_char,
        result_c != result_c_no_modifiers || c != c_no_modifiers,
    )
}

fn parse_ime_composition_string(ctx: HIMC, comp_type: IME_COMPOSITION_STRING) -> Option<Vec<u16>> {
    unsafe {
        let string_len = ImmGetCompositionStringW(ctx, comp_type, None, 0);
        if string_len >= 0 {
            let mut buffer = vec![0u8; string_len as usize + 2];
            ImmGetCompositionStringW(
                ctx,
                comp_type,
                Some(buffer.as_mut_ptr() as _),
                string_len as _,
            );
            let wstring = std::slice::from_raw_parts::<u16>(
                buffer.as_mut_ptr().cast::<u16>(),
                string_len as usize / 2,
            );
            Some(wstring.to_vec())
        } else {
            None
        }
    }
}

#[inline]
fn retrieve_composition_cursor_position(ctx: HIMC) -> usize {
    unsafe { ImmGetCompositionStringW(ctx, GCS_CURSORPOS, None, 0) as usize }
}

fn should_use_ime_cursor_position(ctx: HIMC, cursor_pos: usize) -> bool {
    let attrs_size = unsafe { ImmGetCompositionStringW(ctx, GCS_COMPATTR, None, 0) } as usize;
    if attrs_size == 0 {
        return false;
    }

    let mut attrs = vec![0u8; attrs_size];
    let result = unsafe {
        ImmGetCompositionStringW(
            ctx,
            GCS_COMPATTR,
            Some(attrs.as_mut_ptr() as *mut _),
            attrs_size as u32,
        )
    };
    if result <= 0 {
        return false;
    }

    // Keep the cursor adjacent to the inserted text by only using the suggested position
    // if it's adjacent to unconverted text.
    let at_cursor_is_input = cursor_pos < attrs.len() && attrs[cursor_pos] == (ATTR_INPUT as u8);
    let before_cursor_is_input = cursor_pos > 0
        && (cursor_pos - 1) < attrs.len()
        && attrs[cursor_pos - 1] == (ATTR_INPUT as u8);

    at_cursor_is_input || before_cursor_is_input
}

#[inline]
fn is_virtual_key_pressed(vkey: VIRTUAL_KEY) -> bool {
    unsafe { GetKeyState(vkey.0 as i32) < 0 }
}

#[inline]
pub(crate) fn current_modifiers() -> Modifiers {
    Modifiers {
        control: is_virtual_key_pressed(VK_CONTROL),
        alt: is_virtual_key_pressed(VK_MENU),
        shift: is_virtual_key_pressed(VK_SHIFT),
        platform: is_virtual_key_pressed(VK_LWIN) || is_virtual_key_pressed(VK_RWIN),
        function: false,
    }
}

#[inline]
pub(crate) fn current_capslock() -> Capslock {
    let on = unsafe { GetKeyState(VK_CAPITAL.0 as i32) & 1 } > 0;
    Capslock { on }
}

// Windows 窗口边框存在额外的不可见空间：
// - SM_CXSIZEFRAME：调整大小的手柄
// - SM_CXPADDEDBORDER：不属于调整大小手柄的额外边框空间
fn get_frame_thicknessx(dpi: u32) -> i32 {
    let resize_frame_thickness = unsafe { GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi) };
    let padding_thickness = unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
    resize_frame_thickness + padding_thickness
}

fn get_frame_thicknessy(dpi: u32) -> i32 {
    let resize_frame_thickness = unsafe { GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) };
    let padding_thickness = unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
    resize_frame_thickness + padding_thickness
}

fn notify_frame_changed(handle: HWND) {
    unsafe {
        SetWindowPos(
            handle,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED
                | SWP_NOACTIVATE
                | SWP_NOCOPYBITS
                | SWP_NOMOVE
                | SWP_NOOWNERZORDER
                | SWP_NOREPOSITION
                | SWP_NOSENDCHANGING
                | SWP_NOSIZE
                | SWP_NOZORDER,
        )
        .log_err();
    }
}
