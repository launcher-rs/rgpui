//! 组件示例大全的故事模块。
//!
//! 每个 story 是一个 [`StoryItem`]：包含标题与构建函数。
//! [`registry`] 返回按分类分组的故事列表，供根视图渲染导航与内容区。

pub mod basics;
pub mod dialogs;
pub mod extended;
pub mod inputs;
pub mod lists;
pub mod menus;
pub mod tables;
pub mod tabs;
pub mod themes;

use rgpui::{AnyView, App, Window};

/// 单个组件示例条目。
pub struct StoryItem {
    /// 条目标题（显示在侧边栏与内容区标题）。
    pub title: &'static str,
    /// 构建故事视图：创建一个持有自身状态实体的视图。
    pub build: fn(&mut Window, &mut App) -> AnyView,
}

/// 返回按分类分组的故事注册表。
pub fn registry() -> Vec<(&'static str, Vec<StoryItem>)> {
    vec![
        ("基础组件", basics::stories()),
        ("输入与表单", inputs::stories()),
        ("菜单与通知", menus::stories()),
        ("对话框", dialogs::stories()),
        ("列表与虚拟列表", lists::stories()),
        ("表格", tables::stories()),
        ("标签页与折叠", tabs::stories()),
        ("扩展组件", extended::stories()),
        ("主题", themes::stories()),
    ]
}
