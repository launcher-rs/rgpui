//! 对话框示例：基础对话框与警告对话框。

use rgpui::prelude::*;
use rgpui::{
    Button, ButtonVariants, Context, Dialog, DialogContent, IntoElement, ParentElement, Styled,
    Window, div, px, v_flex,
};

use super::StoryItem;

/// 对话框故事注册表。
pub fn stories() -> Vec<StoryItem> {
    vec![
        StoryItem {
            title: "基础对话框",
            build: |_, cx| cx.new(|cx| DialogStory::new(cx)).into(),
        },
        StoryItem {
            title: "警告对话框",
            build: |_, cx| cx.new(|cx| AlertDialogStory::new(cx)).into(),
        },
    ]
}

/// 基础对话框示例视图。
struct DialogStory;

impl DialogStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for DialogStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = &mut **cx;
        v_flex()
            .id("dialog-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("基础对话框（Dialog）"))
            .child(
                v_flex().gap(px(8.0)).child(
                    Dialog::new(app)
                        .trigger(Button::new("dialog-trigger").label("打开对话框"))
                        .title("基础对话框")
                        .content(|content, _, _| content.child(div().child("这是对话框的内容区域")))
                        .footer(div().child(Button::new("dialog-cancel").label("取消").ghost())),
                ),
            )
    }
}

/// 警告对话框示例视图。
struct AlertDialogStory;

impl AlertDialogStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for AlertDialogStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = &mut **cx;
        v_flex()
            .id("alert-dialog-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("警告对话框（AlertDialog）"))
            .child(
                v_flex().gap(px(8.0)).child(
                    Dialog::new(app)
                        .trigger(
                            Button::new("alert-dialog-trigger")
                                .label("打开警告对话框")
                                .danger(),
                        )
                        .content(|_content, _, _| {
                            DialogContent::new()
                                .child(div().child("这是一个警告对话框，用于确认危险操作。"))
                        })
                        .on_ok(|_, _, _| true)
                        .on_cancel(|_, _, _| true),
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
