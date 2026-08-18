//! 菜单与通知示例：下拉菜单、上下文菜单、悬停卡片、通知。

use rgpui::prelude::*;
use rgpui::{
    Action, Button, Context, ContextMenuExt, DropdownMenu, HoverCard, IntoElement, ParentElement,
    Styled, Window, div, px, v_flex,
};

use super::StoryItem;

/// 菜单与通知故事注册表。
pub fn stories() -> Vec<StoryItem> {
    vec![
        StoryItem {
            title: "下拉菜单",
            build: |_, cx| cx.new(|cx| DropdownMenuStory::new(cx)).into(),
        },
        StoryItem {
            title: "上下文菜单",
            build: |_, cx| cx.new(|cx| ContextMenuStory::new(cx)).into(),
        },
        StoryItem {
            title: "悬停卡片",
            build: |_, cx| cx.new(|cx| HoverCardStory::new(cx)).into(),
        },
        StoryItem {
            title: "通知",
            build: |window, cx| cx.new(|cx| NotificationStory::new(window, cx)).into(),
        },
    ]
}

// 定义故事内使用的动作。
#[derive(Action, PartialEq, Eq, Clone, Copy)]
struct OpenFile;

#[derive(Action, PartialEq, Eq, Clone, Copy)]
struct SaveFile;

#[derive(Action, PartialEq, Eq, Clone, Copy)]
struct DeleteFile;

/// 下拉菜单示例视图。
struct DropdownMenuStory;

impl DropdownMenuStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for DropdownMenuStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("dropdown-menu-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("按钮下拉菜单"))
            .child(
                v_flex().gap(px(8.0)).child(
                    Button::new("dropdown-trigger")
                        .label("打开菜单")
                        .dropdown_menu(|menu, _, _| {
                            menu.label("文件操作")
                                .menu("打开", Box::new(OpenFile))
                                .menu("保存", Box::new(SaveFile))
                                .separator()
                                .menu("删除", Box::new(DeleteFile))
                        }),
                ),
            )
    }
}

/// 上下文菜单示例视图。
struct ContextMenuStory;

impl ContextMenuStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for ContextMenuStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("context-menu-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("右键上下文菜单"))
            .child(
                div()
                    .id("context-menu-target")
                    .w(px(300.0))
                    .h(px(120.0))
                    .bg(rgpui::hsla(0.0, 0.0, 0.9, 0.3))
                    .rounded(px(8.0))
                    .child("在此区域右键点击")
                    .context_menu(|menu, _, _| {
                        menu.label("编辑操作")
                            .menu("打开", Box::new(OpenFile))
                            .menu("保存", Box::new(SaveFile))
                            .separator()
                            .menu("删除", Box::new(DeleteFile))
                    }),
            )
    }
}

/// 悬停卡片示例视图。
struct HoverCardStory;

impl HoverCardStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for HoverCardStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("hover-card-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("悬停卡片（HoverCard）"))
            .child(
                v_flex().gap(px(8.0)).child(
                    HoverCard::new("hover-card")
                        .trigger(Button::new("hover-trigger").label("悬停我查看详情"))
                        .content(|_, _, _| div().p(px(16.0)).child("这里是悬停卡片的详细内容")),
                ),
            )
    }
}

/// 通知示例视图。
struct NotificationStory {
    /// 通知列表实体（渲染于内容区右上角）。
    list: rgpui::Entity<rgpui::NotificationList>,
}

impl NotificationStory {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let list = cx.new(|cx| rgpui::NotificationList::new(window, cx));
        Self { list }
    }
}

impl rgpui::Render for NotificationStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let list = self.list.clone();
        let list_info = list.clone();
        let list_success = list.clone();
        let list_error = list.clone();
        v_flex()
            .id("notification-story")
            .relative()
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("通知（Notification）"))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(Button::new("notify-info").label("信息通知").on_click(
                        move |_, window, cx| {
                            list_info.update(cx, |list, cx| {
                                list.push(
                                    rgpui::Notification::new().message("这是一条信息通知"),
                                    window,
                                    cx,
                                )
                            });
                        },
                    ))
                    .child(Button::new("notify-success").label("成功通知").on_click(
                        move |_, window, cx| {
                            list_success.update(cx, |list, cx| {
                                list.push(
                                    rgpui::Notification::new()
                                        .message("操作成功")
                                        .with_type(rgpui::NotificationType::Success),
                                    window,
                                    cx,
                                )
                            });
                        },
                    ))
                    .child(Button::new("notify-error").label("错误通知").on_click(
                        move |_, window, cx| {
                            list_error.update(cx, |list, cx| {
                                list.push(
                                    rgpui::Notification::new()
                                        .message("操作失败")
                                        .with_type(rgpui::NotificationType::Error),
                                    window,
                                    cx,
                                )
                            });
                        },
                    )),
            )
            .child(
                div()
                    .absolute()
                    .top(px(16.0))
                    .right(px(16.0))
                    .w(px(360.0))
                    .child(list),
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
