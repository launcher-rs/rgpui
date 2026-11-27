//! 组件示例大全的故事模块。
//!
//! 每个 story 是一个 [`StoryItem`]：包含标题与构建函数。
//! [`registry`] 返回按分类分组的故事列表，供根视图渲染导航与内容区。
//!
//! 排序原则：基础组件 → 输入与表单 → 布局/列表 → 交互/对话框 → 高级/专项。
//! 同一分类内简单组件在前、复杂组件在后（参考 gpui-kit 的组件组织方式）。

pub mod basics;
pub mod dialogs;
pub mod extended;
pub mod inputs;
pub mod lists;
pub mod markdown;
pub mod menus;
pub mod platform;
pub mod rich_text;
pub mod showcase;
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
    // 基础组件：按钮 / 选择 / 滑块 / 加载 / 徽标 / 快捷键（简单 → 稍复杂）。
    let mut groups = vec![
        ("基础组件", basics::stories()),
        ("输入与表单", inputs::stories()),
        ("标签页与折叠", tabs::stories()),
        ("列表与虚拟列表", lists::stories()),
        ("表格", tables::stories()),
        ("菜单与通知", menus::stories()),
        ("对话框", dialogs::stories()),
        ("富文本", rich_text::stories()),
        ("扩展组件", extended::stories()),
        ("主题", themes::stories()),
    ];

    // showcase 汇总的 8 个大类：核心/表单/文本/创意/媒体/套件/图表/工具。
    // 其中「输入与表单」已由上面的 inputs::stories() 填充（form_gallery 是单文件页面，
    // 没有独立 build），因此这里跳过该空分组。
    for (name, items) in showcase::groups() {
        if name != "输入与表单" {
            groups.push((name, items));
        }
    }

    // 专项能力放最后。
    groups.push(("Markdown 渲染", markdown::stories()));
    groups.push(("平台契约", platform::stories()));
    groups
}
