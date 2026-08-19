//! 列表示例：普通列表与虚拟列表。

use rgpui::prelude::*;
use rgpui::{Context, IntoElement, ListState, ParentElement, Styled, Window, div, px, v_flex};

use super::StoryItem;

/// 列表故事注册表。
pub fn stories() -> Vec<StoryItem> {
    vec![
        StoryItem {
            title: "普通列表",
            build: |_, cx| cx.new(|cx| ListStory::new(cx)).into(),
        },
        StoryItem {
            title: "虚拟列表",
            build: |_, cx| cx.new(|cx| VirtualListStory::new(cx)).into(),
        },
    ]
}

/// 普通列表示例视图。
struct ListStory;

impl ListStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for ListStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // 创建列表状态：100 个条目，从顶部对齐。
        let state = ListState::new(100, rgpui::ListAlignment::Top, px(8.0));

        v_flex()
            .id("list-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("普通列表（list）"))
            .child(
                v_flex().w(px(360.0)).h(px(320.0)).child(
                    rgpui::list(state, |ix, _, _| {
                        div()
                            .id(rgpui::ElementId::Name(format!("list-item-{ix}").into()))
                            .h(px(32.0))
                            .flex()
                            .items_center()
                            .px(px(12.0))
                            .child(format!("列表项 {ix}"))
                            .into_any_element()
                    })
                    .w_full()
                    .h_full(),
                ),
            )
    }
}

/// 虚拟列表示例视图。
struct VirtualListStory;

impl VirtualListStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for VirtualListStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // 为 1000 个条目生成等高的尺寸列表。
        let item_count = 1000usize;
        let item_size = rgpui::size(px(360.0), px(36.0));
        let sizes = std::rc::Rc::new(vec![item_size; item_count]);

        v_flex()
            .id("virtual-list-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("虚拟列表（v_virtual_list）"))
            .child(
                v_flex()
                    .w(px(360.0))
                    .h(px(320.0))
                    .child(rgpui::v_virtual_list(
                        _cx.entity(),
                        "virtual-list",
                        sizes,
                        |_, range, _, _| {
                            range
                                .map(|ix| {
                                    div()
                                        .id(rgpui::ElementId::Name(format!("vitem-{ix}").into()))
                                        .h(px(36.0))
                                        .flex()
                                        .items_center()
                                        .px(px(12.0))
                                        .child(format!("虚拟列表项 {ix}"))
                                        .into_any_element()
                                })
                                .collect()
                        },
                    )),
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
