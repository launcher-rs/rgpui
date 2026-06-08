//! 对话框示例程序
//!
//! 演示各种对话框（Dialog 和 AlertDialog）的使用方式，包括：
//! - 基础对话框
//! - 自定义按钮文本和样式的对话框
//! - 不可关闭的模态对话框
//! - 警告对话框（AlertDialog）
//! - 带确认/取消按钮的警告对话框
//! - 自定义底部区域的对话框

use rgpui::*;
use rgpui_component::{
    *,
    button::{Button, ButtonVariant, ButtonVariants},
    dialog::{DialogAction, DialogButtonProps, DialogClose, DialogFooter},
    scroll::ScrollableElement,
};
use rgpui_component_assets::Assets;

/// 示例应用主结构体
pub struct Example;

impl Example {
    /// 打开基础对话框
    ///
    /// 最简单的对话框用法，只包含标题和内容。
    fn open_basic_dialog(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.open_dialog(cx, |dialog, _, _| {
            dialog.title("基础对话框").child("这是一个基础对话框示例。")
        });
    }

    /// 打开带自定义按钮的对话框
    ///
    /// 通过 DialogButtonProps 自定义确定/取消按钮的文本和样式。
    fn open_custom_buttons_dialog(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.open_dialog(cx, |dialog, _, _| {
            dialog
                .title("自定义按钮")
                .child("点击下方按钮进行操作。")
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("保存")
                        .cancel_text("放弃")
                        .show_cancel(true),
                )
        });
    }

    /// 打开不可关闭的模态对话框
    ///
    /// 禁用遮罩层点击关闭和关闭按钮，只能通过按钮操作关闭。
    fn open_modal_dialog(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.open_dialog(cx, |dialog, _, _| {
            dialog
                .title("重要操作")
                .child("此对话框必须通过按钮操作才能关闭。")
                .overlay_closable(false)
                .close_button(false)
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("确认")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
        });
    }

    /// 打开带确定回调的对话框
    ///
    /// 点击确定按钮时执行回调，返回 true 关闭对话框，返回 false 保持打开。
    fn open_callback_dialog(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.open_dialog(cx, |dialog, _, _| {
            dialog
                .title("带回调的对话框")
                .child("点击确定按钮后会执行回调操作。")
                .on_ok(|_, _, _| {
                    println!("确定按钮被点击！");
                    true
                })
                .on_cancel(|_, _, _| {
                    println!("取消按钮被点击！");
                    true
                })
                .on_close(|_, _, _| {
                    println!("对话框已关闭。");
                })
        });
    }

    /// 打开简单警告对话框
    ///
    /// 使用 AlertDialog 的命令式 API，只显示标题和确定按钮。
    fn open_simple_alert(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.open_alert_dialog(cx, |alert, _, _| {
            alert.title("操作成功").description("数据已保存。")
        });
    }

    /// 打开确认对话框
    ///
    /// 使用 AlertDialog 的 confirm 模式，显示确定和取消按钮。
    fn open_confirm_alert(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.open_alert_dialog(cx, |alert, _, _| {
            alert
                .title("确认删除")
                .description("确定要删除此文件吗？此操作无法撤销。")
                .confirm()
                .on_ok(|_, _, _| {
                    println!("文件已删除！");
                    true
                })
        });
    }

    /// 打开自定义按钮样式的警告对话框
    ///
    /// 通过 button_props 自定义按钮文本、样式变体和回调。
    fn open_styled_alert(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.open_alert_dialog(cx, |alert, _, _| {
            alert
                .title("危险操作")
                .description("此操作将永久删除所有数据，且无法恢复。")
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("永久删除")
                        .ok_variant(ButtonVariant::Danger)
                        .cancel_text("取消")
                        .show_cancel(true),
                )
                .on_ok(|_, _, _| {
                    println!("执行了危险操作！");
                    true
                })
        });
    }

    /// 打开自定义底部区域的对话框
    ///
    /// 使用 DialogFooter、DialogClose 和 DialogAction 自定义底部按钮布局。
    fn open_custom_footer_dialog(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.open_dialog(cx, |dialog, _, _| {
            dialog
                .title("自定义底部区域")
                .child("使用 DialogFooter 自定义底部按钮布局。")
                .footer(
                    DialogFooter::new()
                        .child(
                            DialogClose::new().child(
                                Button::new("cancel")
                                    .label("取消")
                                    .secondary(),
                            ),
                        )
                        .child(
                            DialogAction::new().child(
                                Button::new("confirm")
                                    .label("确认")
                                    .primary(),
                            ),
                        ),
                )
        });
    }
}

impl Render for Example {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .v_flex()
            .child(TitleBar::new().child("对话框示例"))
            .child(
                div()
                    .id("dialog-examples")
                    .p_6()
                    .v_flex()
                    .gap_4()
                    .flex_1()
                    .overflow_y_scrollbar()
                    // ===== 基础对话框区域 =====
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child("基础对话框")
                                    .child(
                                        Button::new("basic")
                                            .small()
                                            .outline()
                                            .label("打开")
                                            .on_click(cx.listener(Self::open_basic_dialog)),
                                    ),
                            )
                            .child("最简单的对话框，只包含标题和内容。"),
                    )
                    // ===== 自定义按钮对话框区域 =====
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child("自定义按钮")
                                    .child(
                                        Button::new("custom-buttons")
                                            .small()
                                            .outline()
                                            .label("打开")
                                            .on_click(cx.listener(Self::open_custom_buttons_dialog)),
                                    ),
                            )
                            .child("通过 DialogButtonProps 自定义按钮文本和样式。"),
                    )
                    // ===== 模态对话框区域 =====
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child("不可关闭的模态对话框")
                                    .child(
                                        Button::new("modal")
                                            .small()
                                            .outline()
                                            .label("打开")
                                            .on_click(cx.listener(Self::open_modal_dialog)),
                                    ),
                            )
                            .child("禁用遮罩层点击关闭和关闭按钮，只能通过按钮操作关闭。"),
                    )
                    // ===== 带回调的对话框区域 =====
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child("带回调的对话框")
                                    .child(
                                        Button::new("callback")
                                            .small()
                                            .outline()
                                            .label("打开")
                                            .on_click(cx.listener(Self::open_callback_dialog)),
                                    ),
                            )
                            .child("点击确定/取消按钮时执行回调，观察控制台输出。"),
                    )
                    // ===== 简单警告对话框区域 =====
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child("简单警告对话框")
                                    .child(
                                        Button::new("simple-alert")
                                            .small()
                                            .outline()
                                            .label("打开")
                                            .on_click(cx.listener(Self::open_simple_alert)),
                                    ),
                            )
                            .child("使用 AlertDialog 的命令式 API，只显示标题和确定按钮。"),
                    )
                    // ===== 确认对话框区域 =====
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child("确认对话框")
                                    .child(
                                        Button::new("confirm-alert")
                                            .small()
                                            .outline()
                                            .label("打开")
                                            .on_click(cx.listener(Self::open_confirm_alert)),
                                    ),
                            )
                            .child("使用 confirm 模式，显示确定和取消按钮。"),
                    )
                    // ===== 自定义样式警告对话框区域 =====
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child("自定义样式警告对话框")
                                    .child(
                                        Button::new("styled-alert")
                                            .small()
                                            .outline()
                                            .label("打开")
                                            .on_click(cx.listener(Self::open_styled_alert)),
                                    ),
                            )
                            .child("自定义按钮文本、样式变体（如危险按钮）和回调。"),
                    )
                    // ===== 自定义底部区域对话框区域 =====
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child("自定义底部区域对话框")
                                    .child(
                                        Button::new("custom-footer")
                                            .small()
                                            .outline()
                                            .label("打开")
                                            .on_click(cx.listener(Self::open_custom_footer_dialog)),
                                    ),
                            )
                            .child("使用 DialogFooter、DialogClose 和 DialogAction 自定义底部按钮布局。"),
                    ),
            )
            // 必须包含：渲染对话框浮层
            .children(Root::render_dialog_layer(window, cx))
            // 必须包含：渲染通知浮层
            .children(Root::render_notification_layer(window, cx))
    }
}

/// 程序入口
fn main() {
    let app = rgpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        rgpui_component::init(cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(800.), px(700.)), cx)),
            ..Default::default()
        };

        cx.spawn(async move |cx| {
            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|_| Example);
                // 窗口的第一层必须是 Root
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("打开窗口失败");
        })
        .detach();
    });
}
