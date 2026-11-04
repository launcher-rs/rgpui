//! OS 原生弹出菜单示例
//!
//! 演示 [`NativeMenu`](rgpui::NativeMenu) + [`Window::show_native_menu`]：菜单交给系统
//! 自己的控件画出来（Windows `TrackPopupMenuEx`、macOS `NSMenu`），选中结果按菜单项的
//! `id` 经通道异步回传。平台没有这项能力时（Linux 的窗口菜单只能由窗管自己画）返回
//! `None`，示例如实提示，并给出框架自绘菜单（[`ContextMenuExt`]）作为替代路径。
//!
//! 运行：`cargo run -p showcase --bin native_menu`

use rgpui::{
    App, Bounds, Button, Context, ContextMenuExt as _, Entity, IntoElement, NativeMenu,
    ParentElement, PopupMenuItem, Render, SharedString, Styled as _, TitlebarOptions, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use rgpui_platform::application;

/// 原生菜单示例应用：记住可切换项的状态，并把最近一次选择显示成状态文本
struct NativeMenuApp {
    /// 指向自身的句柄，供菜单项回调更新状态
    self_handle: Entity<Self>,
    /// 最近一次菜单交互的结果文本
    status: SharedString,
    /// 「自动换行」开关的当前状态，原生菜单里以勾选态呈现
    word_wrap: bool,
}

impl NativeMenuApp {
    /// 创建示例应用
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            self_handle: cx.entity(),
            status: "点击按钮或右键下方区域试试".into(),
            word_wrap: true,
        }
    }

    /// 按当前状态构建要弹出的原生菜单。
    ///
    /// 勾选态与禁用态都在描述阶段给出，平台层只负责把它们翻译成原生菜单的属性，
    /// 所以这份构建在两个平台上表现一致。
    fn build_menu(&self) -> NativeMenu {
        NativeMenu::new()
            .action("复制", "copy")
            .action("粘贴", "paste")
            .disabled("删除", "delete", true)
            .separator()
            .checked("自动换行", "word-wrap", self.word_wrap)
            .submenu("打开方式", |menu| {
                menu.action("新窗口", "open-new")
                    .action("默认程序", "open-default")
            })
            .separator()
            .action("关于 rgpui", "about")
    }

    /// 处理一次菜单选择：切换开关项，其余只更新状态文本。
    fn apply_selection(&mut self, id: &str) {
        match id {
            "word-wrap" => {
                self.word_wrap = !self.word_wrap;
                self.status =
                    format!("自动换行已{}", if self.word_wrap { "开启" } else { "关闭" }).into();
            }
            other => self.status = format!("菜单选中：{other}").into(),
        }
    }

    /// 在鼠标位置弹出 OS 原生菜单，并把选中结果回写到状态文本。
    fn show_native(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let menu = self.build_menu();
        let Some(receiver) = window.show_native_menu(window.mouse_position(), menu) else {
            self.status =
                "本平台没有原生弹出菜单，请改用框架自绘的 PopupMenu（右键下方区域）".into();
            cx.notify();
            return;
        };

        // 菜单在平台自己的事件循环里模态运行，所以结果只能等：用户关掉菜单时通道被丢弃。
        cx.spawn(async move |this, cx| {
            let text = match receiver.await {
                Ok(id) => Selected::Picked(id),
                Err(_) => Selected::Dismissed,
            };
            cx.update(|cx| {
                let _ = this.update(cx, |this, cx| {
                    match text {
                        Selected::Picked(id) => this.apply_selection(&id),
                        Selected::Dismissed => this.status = "菜单已关闭（未选择任何项）".into(),
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }
}

/// 菜单的两种结束方式：选中某项，或用户直接关掉。
enum Selected {
    Picked(SharedString),
    Dismissed,
}

impl Render for NativeMenuApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let native_handle = self.self_handle.clone();
        let menu_handle = self.self_handle.clone();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(rgb(0x2b2b2b))
            .size_full()
            .text_color(rgb(0xffffff))
            .child(div().text_xl().child("OS 原生弹出菜单示例"))
            .child(div().text_sm().child(self.status.clone()))
            .child(
                Button::new("native")
                    .label("在鼠标位置弹出原生菜单")
                    .on_click(move |_, window, cx| {
                        native_handle.update(cx, |this, cx| this.show_native(window, cx));
                    }),
            )
            .child(
                // 框架自绘菜单：跨平台都可用，是原生菜单缺位时的替代
                div()
                    .id("fallback-area")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w_full()
                    .h(px(120.))
                    .rounded(px(8.))
                    .bg(rgb(0x3c3f41))
                    .child("右键此处：框架自绘的 ContextMenu")
                    .context_menu(move |menu, _, _| {
                        let copy_handle = menu_handle.clone();
                        let about_handle = menu_handle.clone();
                        menu.item(PopupMenuItem::new("复制").on_click(move |_, _, cx| {
                            copy_handle.update(cx, |this, cx| {
                                this.status = "框架菜单选中：复制".into();
                                cx.notify();
                            });
                        }))
                        .item(PopupMenuItem::new("粘贴").disabled(true))
                        .separator()
                        .item(PopupMenuItem::new("关于 rgpui").on_click(move |_, _, cx| {
                            about_handle.update(cx, |this, cx| {
                                this.status = "框架菜单选中：关于 rgpui".into();
                                cx.notify();
                            });
                        }))
                    }),
            )
    }
}

fn main() {
    rgpui::init_logging();
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(560.), px(360.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Native Menu Example".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(NativeMenuApp::new),
        )
        .unwrap();
        cx.activate(true);
    });
}
