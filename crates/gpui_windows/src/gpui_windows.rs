#![cfg(target_os = "windows")]

//! Windows 平台特定的 GPUI 实现
//!
//! 本模块提供了 GPUI 在 Windows 操作系统上的完整平台支持，包括：
//! - 使用 DirectX 11 进行 GPU 加速渲染
//! - 使用 DirectWrite 进行高质量文本渲染
//! - 使用 DirectComposition 进行窗口合成
//! - 使用 Win32 API 处理窗口管理和输入事件

mod clipboard;
mod destination_list;
mod direct_manipulation;
mod direct_write;
mod directx_atlas;
mod directx_devices;
mod directx_renderer;
mod dispatcher;
mod display;
mod events;
mod keyboard;
mod platform;
mod system_settings;
mod tray;
mod util;
mod vsync;
mod window;
mod wrapper;

pub(crate) use clipboard::*;
pub(crate) use destination_list::*;
pub(crate) use direct_write::*;
pub(crate) use directx_atlas::*;
pub(crate) use directx_devices::*;
pub(crate) use directx_renderer::*;
pub(crate) use dispatcher::*;
pub(crate) use display::*;
pub(crate) use events::*;
pub(crate) use keyboard::*;
pub(crate) use platform::*;
pub(crate) use system_settings::*;
pub(crate) use tray::*;
pub(crate) use util::*;
pub(crate) use vsync::*;
pub(crate) use window::*;
pub(crate) use wrapper::*;

/// Windows 平台实现，实现了 GPUI 的 `Platform` trait
pub use platform::WindowsPlatform;

pub(crate) use windows::Win32::Foundation::HWND;
