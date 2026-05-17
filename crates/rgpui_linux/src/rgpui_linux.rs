//! Linux/FreeBSD 平台后端实现，支持 Wayland 和 X11 显示服务器。
//!
//! 本 crate 为 GPUI 提供 Linux 和 FreeBSD 系统上的平台抽象，包括窗口管理、
//! 输入处理、剪贴板、文件选择器等功能。支持通过特性标志选择 Wayland 或 X11 后端。

#![cfg(any(target_os = "linux", target_os = "freebsd"))]

mod linux;

/// 返回当前操作系统的默认平台实现。
pub use linux::current_platform;
