//! 标签页与折叠示例：标签栏、折叠面板、手风琴。

use rgpui::prelude::*;
use rgpui::tabs::{Accordion, Collapsible, Tab, TabBar, TabVariant};
use rgpui::{Context, IntoElement, ParentElement, Styled, Window, div, px, v_flex};

use super::StoryItem;

/// 标签页与折叠故事注册表。
pub fn stories() -> Vec<StoryItem> {
    vec![
        StoryItem {
            title: "标签栏",
            build: |_, cx| cx.new(|cx| TabBarStory::new(cx)).into(),
        },
        StoryItem {
            title: "手风琴",
            build: |_, cx| cx.new(|cx| AccordionStory::new(cx)).into(),
        },
        StoryItem {
            title: "折叠面板",
            build: |_, cx| cx.new(|cx| CollapsibleStory::new(cx)).into(),
        },
    ]
}

/// 标签栏示例视图。
struct TabBarStory;

impl TabBarStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for TabBarStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("tab-bar-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("标签栏（TabBar）"))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(
                        TabBar::new("tabbar-default")
                            .selected_index(0)
                            .child(Tab::new().label("首页"))
                            .child(Tab::new().label("设置"))
                            .child(Tab::new().label("关于")),
                    )
                    .child(div().h(px(8.0)))
                    .child(
                        TabBar::new("tabbar-pill")
                            .selected_index(1)
                            .with_variant(TabVariant::Pill)
                            .child(Tab::new().label("编辑"))
                            .child(Tab::new().label("预览"))
                            .child(Tab::new().label("导出")),
                    ),
            )
    }
}

/// 手风琴示例视图。
struct AccordionStory;

impl AccordionStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for AccordionStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("accordion-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("手风琴（Accordion）"))
            .child(
                v_flex().child(
                    Accordion::new("accordion")
                        .item(|item| {
                            item.title("第一节")
                                .child(div().p(px(12.0)).child("这里是第一节的内容"))
                        })
                        .item(|item| {
                            item.open(true)
                                .title("第二节")
                                .child(div().p(px(12.0)).child("这里是第二节的内容"))
                        })
                        .item(|item| {
                            item.title("第三节")
                                .child(div().p(px(12.0)).child("这里是第三节的内容"))
                        }),
                ),
            )
    }
}

/// 折叠面板示例视图。
struct CollapsibleStory;

impl CollapsibleStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for CollapsibleStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("collapsible-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("折叠面板（Collapsible）"))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(
                        Collapsible::new()
                            .open(true)
                            .content(div().p(px(12.0)).child("已展开的内容区域")),
                    )
                    .child(
                        Collapsible::new().content(div().p(px(12.0)).child("默认折叠的内容区域")),
                    ),
            )
    }
}

/// 章节标题辅助函数。
fn section_title(text: impl Into<rgpui::SharedString> + IntoElement) -> impl IntoElement {
    div()
        .text_size(px(14.0))
        .text_color(rgpui::hsla(0.0, 0.0, 0.55, 1.0))
        .child(text)
}
