mod dispatcher;
mod display;
mod platform;
mod window;

pub use dispatcher::*;
pub(crate) use display::*;
pub(crate) use platform::*;
pub(crate) use window::*;

/// 测试平台屏幕捕获源
pub use platform::{TestScreenCaptureSource, TestScreenCaptureStream};
