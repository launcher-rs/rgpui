//! X11 显示服务器后端实现。
//!
//! 本模块提供基于 Xlib/XCB 的 X11 客户端实现，包括窗口管理、
//! 剪贴板、显示检测以及 XIM（X Input Method）输入法支持。

mod client;
mod clipboard;
mod display;
mod event;
mod window;
mod xim_handler;

pub(crate) use client::*;
pub(crate) use display::*;
pub(crate) use event::*;
pub(crate) use window::*;
pub(crate) use xim_handler::*;
