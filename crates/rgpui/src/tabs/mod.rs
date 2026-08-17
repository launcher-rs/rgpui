//! 标签页/手风琴/折叠面板子系统 - Tabs、Accordion、Collapsible 组件

/// 手风琴组件模块 - 可展开/折叠的面板组
mod accordion;
/// 折叠面板组件模块 - 简单的展开/折叠容器
mod collapsible;
/// 标签页组件模块 - 单个 Tab 元素与变体样式
mod tab;
/// 标签页栏组件模块 - 包含多个 Tab 的 TabBar 与滑动指示器
mod tab_bar;

/// 重导出手风琴相关类型
pub use accordion::*;
/// 重导出折叠面板相关类型
pub use collapsible::*;
/// 重导出标签页相关类型
pub use tab::*;
/// 重导出标签页栏相关类型
pub use tab_bar::*;
