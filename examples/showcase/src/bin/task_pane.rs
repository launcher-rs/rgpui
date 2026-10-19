//! 任务窗格示例
//!
//! 演示 `task_pane`：右侧停靠的任务窗格（标题栏 + 正文 + 内缘拖拽把手 + 关闭按钮）。
//! 拖动内缘把手改宽，`✕` 关闭，工具栏可切换停靠侧 / 浮动 / 显隐。浮动态变成一块可自由
//! 拖动标题栏、八向缩放的窗格（复用 MDI 的缩放几何）。窗格正文实时读取状态，展示宽度 /
//! 停靠侧 / 浮动状态。
//!
//! 运行：`cargo run -p showcase --bin task_pane`

use rgpui::task_pane::{TaskPane, TaskPaneEvent, TaskPaneSide, TaskPaneState};
use rgpui::{
    AnyElement, App, Bounds, Button, Context, Entity, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, size, v_flex,
};
use rgpui_platform::application;

/// 任务窗格示例应用。
struct TaskPaneApp {
    /// 窗格状态。
    pane: Entity<TaskPaneState>,
    /// 状态栏文案。
    status: SharedString,
}

impl TaskPaneApp {
    /// 创建示例应用并观察窗格状态。
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let pane = cx.new(|cx| {
            TaskPaneState::new(cx)
                .with_side(TaskPaneSide::Right)
                .with_width(px(300.0))
        });

        // 窗格元素只负责画与转发事件，几何变化要靠宿主观察状态来触发重绘。
        cx.observe(&pane, |_this, _pane, cx| cx.notify()).detach();
        cx.subscribe_in(
            &pane,
            window,
            |this: &mut Self, _pane, event: &TaskPaneEvent, _window, cx| {
                this.status = match event {
                    TaskPaneEvent::VisibilityChanged(true) => "显示窗格".into(),
                    TaskPaneEvent::VisibilityChanged(false) => "隐藏窗格".into(),
                    TaskPaneEvent::SideChanged(TaskPaneSide::Left) => "停靠到左侧".into(),
                    TaskPaneEvent::SideChanged(TaskPaneSide::Right) => "停靠到右侧".into(),
                    TaskPaneEvent::WidthChanged(width) => format!("窗格宽 {width:.0}").into(),
                    TaskPaneEvent::FloatingChanged(true) => "切到浮动态".into(),
                    TaskPaneEvent::FloatingChanged(false) => "回到停靠态".into(),
                };
                cx.notify();
            },
        )
        .detach();

        Self {
            pane,
            status: "拖动内缘把手改宽 · ✕ 关闭 · 工具栏可换侧 / 浮动".into(),
        }
    }

    /// 切换浮动 / 停靠（浮动时贴到窗口右下角内侧）。
    fn toggle_float(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let parent = Bounds::new(rgpui::point(px(0.0), px(0.0)), window.viewport_size());
        self.pane
            .update(cx, |pane, cx| pane.toggle_float(parent, cx));
    }

    /// 相对当前宽度加宽 / 收窄。
    fn resize_by(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.pane.update(cx, |pane, cx| {
            let width = pane.width() + px(delta);
            pane.set_width(width, cx);
        });
    }
}

impl Render for TaskPaneApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (visible, side, width, floating, closable) = self.pane.read_with(cx, |pane, _| {
            (
                pane.is_visible(),
                pane.side(),
                pane.width(),
                pane.is_floating(),
                pane.is_closable(),
            )
        });
        let status = self.status.clone();

        // 正文：实时读取窗格状态（宽度 / 停靠侧 / 浮动态）。
        let pane_for_body = self.pane.clone();
        let body = move |_window: &mut Window, cx: &mut App| -> AnyElement {
            let (width, side, floating) = pane_for_body.read_with(cx, |pane, _| {
                (pane.width(), pane.side(), pane.is_floating())
            });
            let theme = cx.theme();
            let muted = theme.muted_foreground;
            let foreground = theme.foreground;
            let border = theme.border;
            let row = move |label: &'static str, value: String| {
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .py_1()
                    .border_b_1()
                    .border_color(border)
                    .text_sm()
                    .child(div().text_color(muted).child(label))
                    .child(div().text_color(foreground).child(value))
            };
            v_flex()
                .gap_1()
                .p_3()
                .child(div().text_xs().text_color(muted).child("页面设置"))
                .child(row("宽度", format!("{width:.0}")))
                .child(row(
                    "停靠侧",
                    if floating {
                        "浮动".to_string()
                    } else if side == TaskPaneSide::Left {
                        "左侧".to_string()
                    } else {
                        "右侧".to_string()
                    },
                ))
                .child(row("纸张", "A4 · 纵向".into()))
                .child(row("页边距", "上下 2.54 · 左右 3.17".into()))
                .child(
                    div()
                        .pt_2()
                        .text_xs()
                        .text_color(muted)
                        .child("属性行随窗格状态实时刷新；拖动内缘把手即可改宽。"),
                )
                .into_any_element()
        };

        // 主内容区。
        let main = div().flex_1().h_full().p_4().child(
            v_flex()
                .gap_2()
                .size_full()
                .child(div().text_base().child("文档正文"))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("这里是被停靠窗格挤出的可用区域。"),
                )
                .child(
                    div()
                        .flex_1()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .p_3()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("拖动窗格内缘的细条改宽 · 点标题栏右侧 ✕ 关闭 · 工具栏可切到浮动态"),
                ),
        );

        let pane = TaskPane::new(&self.pane)
            .title("属性")
            .body(body)
            .into_any_element();

        // 容器是浮动态的定位祖先。
        let row = div()
            .relative()
            .flex()
            .flex_row()
            .flex_1()
            .overflow_hidden()
            .border_t_1()
            .border_color(cx.theme().border);
        let row = if floating || side == TaskPaneSide::Right {
            row.child(main).child(pane)
        } else {
            row.child(pane).child(main)
        };

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .p_3()
                    .child(
                        Button::new("toggle")
                            .label(if visible {
                                "隐藏窗格"
                            } else {
                                "显示窗格"
                            })
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.pane.update(cx, |pane, cx| pane.toggle(cx));
                            })),
                    )
                    .child(
                        Button::new("dock-left")
                            .label("停靠左")
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.pane
                                    .update(cx, |pane, cx| pane.set_side(TaskPaneSide::Left, cx));
                            })),
                    )
                    .child(
                        Button::new("dock-right")
                            .label("停靠右")
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.pane
                                    .update(cx, |pane, cx| pane.set_side(TaskPaneSide::Right, cx));
                            })),
                    )
                    .child(
                        Button::new("float")
                            .label(if floating { "回到停靠" } else { "浮动" })
                            .on_click(cx.listener(|this, _event, window, cx| {
                                this.toggle_float(window, cx);
                            })),
                    )
                    .child(Button::new("wider").label("加宽").on_click(
                        cx.listener(|this, _event, _window, cx| this.resize_by(40.0, cx)),
                    ))
                    .child(Button::new("narrower").label("收窄").on_click(
                        cx.listener(|this, _event, _window, cx| this.resize_by(-40.0, cx)),
                    ))
                    .child(
                        Button::new("closable")
                            .label(if closable {
                                "关闭按钮：开"
                            } else {
                                "关闭按钮：关"
                            })
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.pane.update(cx, |pane, cx| {
                                    let next = !pane.is_closable();
                                    pane.set_closable(next, cx);
                                });
                            })),
                    )
                    .child(div().text_sm().child(format!(
                        "宽 {width:.0} · {}{} · {status}",
                        if floating { "浮动" } else { "停靠" },
                        if visible { "" } else { " · 已隐藏" }
                    ))),
            )
            .child(row)
    }
}

fn main() {
    rgpui::init_logging();
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1120.), px(720.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Task Pane Example".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| TaskPaneApp::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}
