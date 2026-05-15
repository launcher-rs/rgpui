//! 文本系统模块。
//!
//! 在 Wayland/X11 环境下使用基于 Cosmic Text 的文本渲染系统。

/// 使用 gpui_wgpu 提供的 Cosmic Text 文本系统实现。
pub(crate) use gpui_wgpu::CosmicTextSystem;
