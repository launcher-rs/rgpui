//! Wayland 显示服务器后端实现。
//!
//! 本模块提供基于 Wayland 协议的客户端实现，包括窗口管理、
//! 剪贴板、光标、显示检测和 layer shell 支持。

mod client;
mod clipboard;
mod cursor;
mod display;
mod serial;
mod window;

/// 包含用于配置 layer_shell 表面的类型。
///
/// Layer shell 协议允许创建桌面层（如面板、通知、锁屏等），
/// 这些表面独立于普通窗口管理。
pub mod layer_shell;

pub(crate) use client::*;

use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape;

use rgpui::CursorStyle;

/// 将 GPUI 光标样式转换为 Wayland 光标形状枚举。
///
/// 使用 `wp_cursor_shape_device_v1` 协议定义的标准光标形状。
///
/// # 参数
///
/// * `style` - GPUI 光标样式
pub(super) fn to_shape(style: CursorStyle) -> Shape {
    match style {
        CursorStyle::Arrow => Shape::Default,
        CursorStyle::IBeam => Shape::Text,
        CursorStyle::Crosshair => Shape::Crosshair,
        CursorStyle::ClosedHand => Shape::Grabbing,
        CursorStyle::OpenHand => Shape::Grab,
        CursorStyle::PointingHand => Shape::Pointer,
        CursorStyle::ResizeLeft => Shape::WResize,
        CursorStyle::ResizeRight => Shape::EResize,
        CursorStyle::ResizeLeftRight => Shape::EwResize,
        CursorStyle::ResizeUp => Shape::NResize,
        CursorStyle::ResizeDown => Shape::SResize,
        CursorStyle::ResizeUpDown => Shape::NsResize,
        CursorStyle::ResizeUpLeftDownRight => Shape::NwseResize,
        CursorStyle::ResizeUpRightDownLeft => Shape::NeswResize,
        CursorStyle::ResizeColumn => Shape::ColResize,
        CursorStyle::ResizeRow => Shape::RowResize,
        CursorStyle::IBeamCursorForVerticalLayout => Shape::VerticalText,
        CursorStyle::OperationNotAllowed => Shape::NotAllowed,
        CursorStyle::DragLink => Shape::Alias,
        CursorStyle::DragCopy => Shape::Copy,
        CursorStyle::ContextualMenu => Shape::ContextMenu,
    }
}
