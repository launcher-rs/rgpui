//! 阴影渲染独立复现：同时展示普通圆角卡片、右键菜单、通知和对话框。

use rgpui::prelude::*;
use rgpui::{
    App, Button, ButtonVariants, Context, ContextMenuExt, Dialog, DropdownMenu, Entity,
    InteractiveElement, IntoElement, Notification, NotificationList, NotificationType,
    ParentElement, PopupMenuItem, Render, Root, SharedString, Styled, TitlebarOptions, Window,
    WindowBounds, WindowOptions, div, h_flex, px, rgb, size,
};
use rgpui_platform::application;

struct ShadowRepro {
    notifications: Entity<NotificationList>,
}

impl ShadowRepro {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            notifications: cx.new(|cx| NotificationList::new(window, cx)),
        }
    }
}

impl Render for ShadowRepro {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let app = &mut **cx;
        let notifications = self.notifications.clone();
        let info_list = self.notifications.clone();
        let success_list = self.notifications.clone();
        let error_list = self.notifications.clone();

        div()
            .id("shadow-repro")
            .relative()
            .size_full()
            .bg(rgb(0xf4f5f7))
            .text_color(rgb(0x1f2329))
            .p(px(24.0))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_size(px(20.0))
                    .font_semibold()
                    .child("BoxShadow 渲染复现"),
            )
            .child(
                h_flex()
                    .gap(px(8.0))
                    .child(Button::new("notify-info").label("信息通知").on_click(
                        move |_, window, cx| {
                            info_list.update(cx, |list, cx| {
                                list.push(
                                    Notification::new()
                                        .message("信息通知：检查右上阴影")
                                        .with_type(NotificationType::Info),
                                    window,
                                    cx,
                                );
                            });
                        },
                    ))
                    .child(Button::new("notify-success").label("成功通知").on_click(
                        move |_, window, cx| {
                            success_list.update(cx, |list, cx| {
                                list.push(
                                    Notification::new()
                                        .message("成功通知：检查右上阴影")
                                        .with_type(NotificationType::Success),
                                    window,
                                    cx,
                                );
                            });
                        },
                    ))
                    .child(Button::new("notify-error").label("错误通知").on_click(
                        move |_, window, cx| {
                            error_list.update(cx, |list, cx| {
                                list.push(
                                    Notification::new()
                                        .message("错误通知：检查右上阴影")
                                        .with_type(NotificationType::Error),
                                    window,
                                    cx,
                                );
                            });
                        },
                    ))
                    .child(
                        Dialog::new(app)
                            .trigger(Button::new("open-dialog").label("打开对话框"))
                            .title("阴影对话框")
                            .content(|content, _, _| {
                                content.child(div().child("检查卡片内部是否出现斜向灰块"))
                            })
                            .footer(div().child(Button::new("close-dialog").label("关闭").ghost())),
                    )
                    .child(
                        Button::new("theme-menu")
                            .label("Default Light")
                            .dropdown_menu(|menu, _, _| {
                                menu.label("选择主题")
                                    .item(PopupMenuItem::new("Default Light").checked(true))
                                    .item(PopupMenuItem::new("Default Dark"))
                            }),
                    ),
            )
            .child(
                div()
                    .w(px(440.0))
                    .h(px(130.0))
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0xd9dde3))
                    .rounded(px(10.0))
                    .shadow_lg()
                    .p(px(20.0))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap(px(6.0))
                    .child(div().font_semibold().child("普通圆角阴影卡片（shadow_lg）"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x667085))
                            .child("整个白色内部应完整，阴影只能出现在卡片外缘"),
                    ),
            )
            .child(
                div()
                    .id("context-menu-target")
                    .w(px(440.0))
                    .h(px(130.0))
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0xd9dde3))
                    .rounded(px(10.0))
                    .p(px(20.0))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap(px(6.0))
                    .child(div().font_semibold().child("右键菜单目标区域"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x667085))
                            .child("在此卡片内右键点击，检查弹出菜单的白色区域"),
                    )
                    .context_menu(|menu, _, _| {
                        menu.label("编辑操作")
                            .item(PopupMenuItem::new("打开").on_click(|_, _, _| {}))
                            .item(PopupMenuItem::new("保存").on_click(|_, _, _| {}))
                            .separator()
                            .item(PopupMenuItem::new("删除").on_click(|_, _, _| {}))
                    }),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x667085))
                    .child("通知列表固定在窗口右上角；按 Esc 可关闭菜单或对话框。"),
            )
            .child(
                div()
                    .absolute()
                    .top(px(24.0))
                    .right(px(24.0))
                    .w(px(460.0))
                    .child(notifications),
            )
            .when_some(dialog_layer, |this, layer| this.child(layer))
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::theme::init(cx);
        rgpui::input_ui::init(cx);
        rgpui::menu::init(cx);
        rgpui::list::init(cx);
        rgpui::table::init(cx);
        rgpui::components::init(cx);

        let title: SharedString = "Shadow Repro".into();
        cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(title),
                    ..Default::default()
                }),
                window_background: rgpui::WindowBackgroundAppearance::Opaque,
                window_bounds: Some(WindowBounds::centered(size(px(920.0), px(680.0)), cx)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| ShadowRepro::new(window, cx)),
        )
        .expect("阴影复现窗口应能打开");
        cx.activate(true);
    });
}

fn main() {
    run_example();
}
