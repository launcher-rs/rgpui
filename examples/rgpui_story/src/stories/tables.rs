//! 表格示例：静态表格与数据表格。

use rgpui::prelude::*;
use rgpui::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use rgpui::{Context, IntoElement, ParentElement, Styled, Window, div, px, v_flex};

use super::StoryItem;

/// 表格故事注册表。
pub fn stories() -> Vec<StoryItem> {
    vec![
        StoryItem {
            title: "静态表格",
            build: |_, cx| cx.new(|cx| TableStory::new(cx)).into(),
        },
        StoryItem {
            title: "数据表格",
            build: |_, cx| cx.new(|cx| DataTableStory::new(cx)).into(),
        },
    ]
}

/// 静态表格示例视图。
struct TableStory;

impl TableStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for TableStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("table-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("静态表格（Table）"))
            .child(
                v_flex().child(
                    Table::new()
                        .child(
                            TableHeader::new().child(
                                TableRow::new()
                                    .child(TableHead::new().child("姓名"))
                                    .child(TableHead::new().child("年龄"))
                                    .child(TableHead::new().child("城市")),
                            ),
                        )
                        .child(
                            TableBody::new()
                                .child(
                                    TableRow::new()
                                        .child(TableCell::new().child("张三"))
                                        .child(TableCell::new().child("28"))
                                        .child(TableCell::new().child("北京")),
                                )
                                .child(
                                    TableRow::new()
                                        .child(TableCell::new().child("李四"))
                                        .child(TableCell::new().child("34"))
                                        .child(TableCell::new().child("上海")),
                                ),
                        ),
                ),
            )
    }
}

/// 数据表格示例视图。
struct DataTableStory;

impl DataTableStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for DataTableStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("data-table-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("数据表格（DataTable）"))
            .child(div().child("完整的数据表格实现需要 TableDelegate 与 TableState，详见代码库中的 data_table 示例。"))
    }
}

/// 章节标题辅助函数。
fn section_title(text: impl Into<rgpui::SharedString> + IntoElement) -> impl IntoElement {
    div()
        .text_size(px(14.0))
        .text_color(rgpui::hsla(0.0, 0.0, 0.55, 1.0))
        .child(text)
}
