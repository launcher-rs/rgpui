//! 系统集成子系统：托盘、焦点陷阱、Tab 导航、窗口边框/扩展、SVG 渲染。
//!
//! 均为 crate 内部模块，对外经 `rgpui.rs` 根重导出。

#[deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub(crate) mod focus_trap;
#[deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub(crate) mod svg_renderer;
pub(crate) mod tab_stop;
pub(crate) mod tray;
pub(crate) mod window_border;
pub(crate) mod window_ext;
