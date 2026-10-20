//! 多文档界面（MDI）示例
//!
//! 演示 `mdi`：一块 MDI 区域内并列多个子文档窗口。每个窗口带标题栏（最小化 / 最大化 /
//! 关闭）与八向缩放边框；拖拽标题栏移动、拖拽边框缩放、双击标题栏切换最大化、
//! 最小化的窗口缩为底部图标条。工具栏提供新建 / 层叠 / 平铺 / 全部最小化 / 关闭全部，
//! 激活窗口内按 `Ctrl+Tab`（或 `Ctrl+F6`）循环切换、`Ctrl+F4` 关闭、`Ctrl+M` 最小化。
//! 窗口正文里放了一个按钮，用来验证「正文可交互 + 点击自动激活窗口」。
//!
//! 运行：`cargo run -p showcase --bin mdi`

use std::collections::HashMap;

use rgpui::mdi::{MdiArea, MdiEvent, MdiLayout, MdiState, MdiWindowFlags};
use rgpui::{
    App, Bounds, Button, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div, hsla, prelude::*, px,
    size, v_flex,
};
use rgpui_platform::application;

/// 打开一个带全部能力开关的普通文档窗口。
fn open_document(app: &mut MdiApp, title: String, cx: &mut Context<MdiApp>) {
    app.mdi.update(cx, |state, cx| state.open(title, cx));
}

/// MDI 示例应用。
struct MdiApp {
    /// MDI 状态。
    mdi: Entity<MdiState>,
    /// 各窗口正文里的点击次数（演示正文可交互）。
    clicks: HashMap<u64, usize>,
    /// 下一个文档序号。
    next_index: usize,
    /// 状态栏文案。
    status: SharedString,
}

impl MdiApp {
    /// 创建示例应用并订阅 MDI 事件。
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 拖动松手时吸附到工作区/邻窗边缘（6px 内）
        let mdi = cx.new(|cx| MdiState::new(cx).with_snap(px(6.0)));

        // MDI 元素只负责画与转发事件，几何变化要靠宿主观察状态来触发重绘。
        cx.observe(&mdi, |_this, _mdi, cx| cx.notify()).detach();

        cx.subscribe_in(
            &mdi,
            window,
            |this: &mut Self, _mdi, event: &MdiEvent, _window, cx| {
                this.status = match event {
                    MdiEvent::Opened(id) => format!("打开窗口 {id}").into(),
                    MdiEvent::Activated(id) => format!("激活窗口 {id}").into(),
                    MdiEvent::Closed(id) => format!("关闭窗口 {id}").into(),
                    MdiEvent::StateChanged(id) => format!("窗口 {id} 状态变化").into(),
                };
                cx.notify();
            },
        )
        .detach();

        let mut app = Self {
            mdi,
            clicks: HashMap::new(),
            next_index: 0,
            status: "拖拽标题栏移动（松手吸附边缘）· 拖拽边框缩放 · 双击标题栏最大化".into(),
        };
        // 初始铺开三个文档，方便一眼看到层叠效果。
        for name in ["季度报告", "预算表", "会议纪要"] {
            app.new_document(name.into(), cx);
        }
        app.mdi
            .update(cx, |state, cx| state.arrange(MdiLayout::Cascade, cx));
        app
    }

    /// 新建一个文档窗口。
    fn new_document(&mut self, name: String, cx: &mut Context<Self>) {
        self.next_index += 1;
        let title = format!("{name} ({})", self.next_index);
        open_document(self, title, cx);
    }

    /// 新建一个不可关闭 / 不可最小化的只读窗口。
    fn new_readonly(&mut self, cx: &mut Context<Self>) {
        self.next_index += 1;
        let title = format!("只读视图 ({})", self.next_index);
        self.mdi.update(cx, |state, cx| {
            state.open_with(
                title,
                MdiWindowFlags {
                    closable: false,
                    minimizable: false,
                    maximizable: true,
                },
                cx,
            );
        });
    }

    /// 按给定方式排布窗口。
    fn arrange(&mut self, layout: MdiLayout, cx: &mut Context<Self>) {
        self.mdi.update(cx, |state, cx| state.arrange(layout, cx));
        self.status = match layout {
            MdiLayout::Cascade => "层叠排布".into(),
            MdiLayout::TileHorizontal => "水平平铺".into(),
            MdiLayout::TileVertical => "垂直平铺".into(),
        };
    }

    /// 最小化全部窗口。
    fn minimize_all(&mut self, cx: &mut Context<Self>) {
        self.mdi.update(cx, |state, cx| {
            for window in state
                .windows()
                .iter()
                .map(|window| window.id)
                .collect::<Vec<_>>()
            {
                state.minimize(window, cx);
            }
        });
        self.status = "全部最小化（点击底部图标还原）".into();
    }

    /// 关闭全部窗口。
    fn close_all(&mut self, cx: &mut Context<Self>) {
        self.mdi.update(cx, |state, cx| state.close_all(cx));
        self.clicks.clear();
        self.status = "关闭全部窗口".into();
    }
}

impl Render for MdiApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (count, active) = {
            let state = self.mdi.read(cx);
            (state.len(), state.active())
        };
        let status = self.status.clone();

        // 正文回调：每个窗口按 id 现渲染（这里放一个可点击按钮，验证正文可交互）。
        let this = cx.entity();
        let body = move |id: u64, _window: &mut Window, cx: &mut App| {
            let this = this.clone();
            let count = this.read(cx).clicks.get(&id).copied().unwrap_or(0);
            let (hue, tint) = (0.58 + 0.13 * (id % 4) as f32, 0.94);
            div()
                .flex()
                .flex_col()
                .gap_2()
                .size_full()
                .p_3()
                .bg(hsla(hue, 0.25, tint, 1.0))
                .child(
                    div()
                        .text_sm()
                        .child(format!("文档 {id} 的正文（点击计数 {count}）")),
                )
                .child(Button::new(("bump", id)).label("点我 +1").on_click(
                    move |_event, _window, cx| {
                        this.update(cx, |this, cx| {
                            *this.clicks.entry(id).or_default() += 1;
                            this.status = format!("文档 {id} 正文被点击").into();
                            cx.notify();
                        });
                    },
                ))
                .child(
                    div()
                        .text_xs()
                        .child("正文内的控件可以正常交互，点击正文会顺带激活所在窗口。"),
                )
                .into_any_element()
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
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(Button::new("new").label("新建文档").on_click(cx.listener(
                        |this, _event, _window, cx| {
                            this.new_document("新文档".into(), cx);
                        },
                    )))
                    .child(
                        Button::new("new-readonly")
                            .label("只读视图")
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.new_readonly(cx);
                            })),
                    )
                    .child(Button::new("cascade").label("层叠").on_click(cx.listener(
                        |this, _event, _window, cx| this.arrange(MdiLayout::Cascade, cx),
                    )))
                    .child(
                        Button::new("tile-h")
                            .label("水平平铺")
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.arrange(MdiLayout::TileHorizontal, cx)
                            })),
                    )
                    .child(
                        Button::new("tile-v")
                            .label("垂直平铺")
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.arrange(MdiLayout::TileVertical, cx)
                            })),
                    )
                    .child(
                        Button::new("minimize-all").label("全部最小化").on_click(
                            cx.listener(|this, _event, _window, cx| this.minimize_all(cx)),
                        ),
                    )
                    .child(
                        Button::new("close-all")
                            .label("关闭全部")
                            .on_click(cx.listener(|this, _event, _window, cx| this.close_all(cx))),
                    )
                    .child(div().text_sm().child(format!(
                                "窗口 {count} 个 · 激活 {} · {status}",
                                active
                                    .map(|id| format!("#{id}"))
                                    .unwrap_or_else(|| "无".to_string())
                            ))),
            )
            .child(
                div()
                    .flex_1()
                    .p_2()
                    .child(MdiArea::new(&self.mdi).body(body)),
            )
    }
}

fn main() {
    rgpui::init_logging();
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1080.), px(720.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("MDI Example".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| MdiApp::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}
