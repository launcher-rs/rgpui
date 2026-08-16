//! 滚动子系统 - 提供可滚动容器、滚动条与滚轮事件遮罩。
//!
//! 本模块是 rgpui 的复合组件地基，供 Dialog、List、Table 等组件复用：
//! - [`ScrollableElement`]：为元素添加滚动条与滚动区域包装。
//! - [`Scrollbar`]：悬停/拖动/自动淡出的滚动条。
//! - [`ScrollableMask`]：滚轮事件遮罩，用于嵌套滚动器的轴分发。
//! - [`AutoScroll`]：拖拽选区时的定时自动滚动。

mod auto_scroll;
mod scrollable;
mod scrollable_mask;
mod scrollbar;

pub use auto_scroll::AutoScroll;
pub use scrollable::*;
pub use scrollable_mask::*;
pub use scrollbar::*;
