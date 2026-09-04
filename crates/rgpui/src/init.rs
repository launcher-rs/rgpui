//! 统一初始化模块。
//!
//! 提供 `init_all` 函数，聚合所有子系统的初始化调用，
//! 自动处理初始化顺序依赖。应用启动时调用一次即可。

use crate::App;

/// 统一初始化所有子系统。
///
/// 聚合主题、菜单、输入、列表、表格、树、组件等子系统的 `init()` 调用，
/// 按正确顺序初始化全局状态和快捷键绑定。
///
/// 应用启动时在 `application().run(|cx| { ... })` 的闭包开头调用：
///
/// ```rust,ignore
/// use rgpui::init_all;
///
/// rgpui_platform::application().run(|cx| {
///     init_all(cx);
///     // ... 应用逻辑
/// });
/// ```
pub fn init_all(cx: &mut App) {
    crate::theme::init(cx);
    crate::menu::init(cx);
    crate::input_ui::init(cx);
    crate::list::init(cx);
    crate::table::init(cx);
    crate::tree::init(cx);
    crate::components::init(cx);
    #[cfg(feature = "tokio")]
    crate::tokio::init(cx);
}
