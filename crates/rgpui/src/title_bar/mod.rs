//! 标题栏与窗口边框组件 - 自定义标题栏、窗口控制按钮与客户端装饰边框。

mod title_bar;
mod window_border;

pub use title_bar::*;
pub use window_border::{WindowBorder, window_border, window_paddings};
