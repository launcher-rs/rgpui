//! 标签页与折叠示例：标签栏、折叠面板、手风琴。

use rgpui::prelude::*;
use rgpui::tabs::{Accordion, Collapsible, Tab, TabBar, TabVariant};
use rgpui::{Button, Context, IntoElement, ParentElement, Styled, Window, div, px, v_flex};

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
struct TabBarStory {
    /// 当前选中的标签索引，供两个 TabBar 共享。
    selected: usize,
}

impl TabBarStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self { selected: 0 }
    }
}

impl rgpui::Render for TabBarStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected;
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
                            .selected_index(selected)
                            .on_click(cx.listener(|this, ix: &usize, _, cx| {
                                this.selected = *ix;
                                cx.notify();
                            }))
                            .child(Tab::new().label("首页"))
                            .child(Tab::new().label("设置"))
                            .child(Tab::new().label("关于")),
                    )
                    .child(div().h(px(8.0)))
                    .child(
                        TabBar::new("tabbar-pill")
                            .selected_index(selected)
                            .with_variant(TabVariant::Pill)
                            .on_click(cx.listener(|this, ix: &usize, _, cx| {
                                this.selected = *ix;
                                cx.notify();
                            }))
                            .child(Tab::new().label("编辑"))
                            .child(Tab::new().label("预览"))
                            .child(Tab::new().label("导出")),
                    ),
            )
    }
}

/// 手风琴示例视图。
struct AccordionStory {
    /// 各项目的展开状态。
    open: [bool; 3],
}

impl AccordionStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            open: [false, true, false],
        }
    }
}

impl rgpui::Render for AccordionStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.open;
        v_flex()
            .id("accordion-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("手风琴（Accordion）"))
            .child(
                v_flex().child(
                    Accordion::new("accordion")
                        .multiple(true)
                        .on_toggle_click(cx.listener(|this, open_ixs: &[usize], _, cx| {
                            for (ix, item_open) in this.open.iter_mut().enumerate() {
                                *item_open = open_ixs.contains(&ix);
                            }
                            cx.notify();
                        }))
                        .item(|item| {
                            item.open(open[0])
                                .title("第一节")
                                .child(div().p(px(12.0)).child("这里是第一节的内容"))
                        })
                        .item(|item| {
                            item.open(open[1])
                                .title("第二节")
                                .child(div().p(px(12.0)).child("这里是第二节的内容"))
                        })
                        .item(|item| {
                            item.open(open[2])
                                .title("第三节")
                                .child(div().p(px(12.0)).child("这里是第三节的内容"))
                        }),
                ),
            )
    }
}

/// 折叠面板示例视图。
struct CollapsibleStory {
    /// 折叠面板是否展开。
    open: bool,
}

impl CollapsibleStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self { open: true }
    }
}

impl rgpui::Render for CollapsibleStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("collapsible-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("折叠面板（Collapsible）"))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(
                        Button::new("toggle-collapsible")
                            .label(if self.open {
                                "折叠内容"
                            } else {
                                "展开内容"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open = !this.open;
                                cx.notify();
                            })),
                    )
                    .child(
                        Collapsible::new()
                            .open(self.open)
                            .content(div().p(px(12.0)).child("已展开的内容区域")),
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
