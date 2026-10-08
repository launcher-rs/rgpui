//! 原生系统通知示例
//!
//! 演示 `App::show_notification` / `App::push_notification`：
//! 调用经 `Platform` trait 分发到各平台原生实现
//! （Windows Toast、macOS User Notifications、Linux XDG Desktop Portal）。
//! 一个按钮直接发送；另一个把发送结果回显到窗口状态文本。
//!
//! 运行：`cargo run -p native_notification`

use rgpui::{
    App, Bounds, Button, Context, Entity, IntoElement, ParentElement, Render, SharedString,
    Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb,
    size,
};
use rgpui_platform::application;

/// 通知示例应用：记录最近一次发送结果并渲染状态文本与按钮
struct NotificationApp {
    /// 指向自身的句柄，供按钮点击回调更新状态使用
    self_handle: Entity<Self>,
    /// 最近一次通知发送结果的展示文本
    status: SharedString,
}

impl NotificationApp {
    /// 创建示例应用，初始状态提示用户操作
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            self_handle: cx.entity(),
            status: "点击下方按钮发送原生通知".into(),
        }
    }

    /// 发送一条操作系统原生通知，并按发送结果更新窗口内状态文本。
    ///
    /// `show_notification` 返回 `Result`：平台不支持或通知门户拒绝时
    /// 会得到 `Err`，这里如实展示，便于在 headless 等环境下定位问题。
    fn send_notification(&mut self, title: &str, body: &str, cx: &mut Context<Self>) {
        self.status = match cx.show_notification(title, body) {
            Ok(()) => format!("已发送通知：{title}").into(),
            Err(err) => format!("发送失败：{err}").into(),
        };
        cx.notify();
    }
}

impl Render for NotificationApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let handle = self.self_handle.clone();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(rgb(0x2b2b2b))
            .size_full()
            .justify_center()
            .items_center()
            .text_color(rgb(0xffffff))
            .child(div().text_xl().child("原生系统通知示例"))
            .child(div().text_sm().child(self.status.clone()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        // 直接调用 App 便捷方法，不关心发送结果
                        Button::new("send-simple").label("发送通知").on_click(
                            |_event, _window, cx| {
                                let _ =
                                    cx.push_notification("rgpui", "这是一条来自 rgpui 的原生通知");
                            },
                        ),
                    )
                    .child(
                        // 经实体句柄更新状态文本，把成败回显到窗口内
                        Button::new("send-with-status")
                            .label("发送通知并回显结果")
                            .on_click(move |_event, _window, cx| {
                                handle.update(cx, |this, cx| {
                                    this.send_notification(
                                        "rgpui 带参数的通知",
                                        "标题与内容均由调用方传入",
                                        cx,
                                    );
                                });
                            }),
                    ),
            )
    }
}

fn main() {
    // 诊断时用 RUST_LOG=info 运行，可看到通知门户调用等内部日志
    rgpui::init_logging();
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(480.), px(260.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Native Notification Example".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(NotificationApp::new),
        )
        .unwrap();
        cx.activate(true);
    });
}
