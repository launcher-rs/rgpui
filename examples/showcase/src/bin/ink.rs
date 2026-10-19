//! Ink 笔迹示例
//!
//! 演示 `ink` 墨迹层：鼠标拖拽用钢笔 / 荧光笔落笔，橡皮按半径擦除，套索框选后删除；
//! 另含撤销与清空。笔画以中点二次贝塞尔平滑绘制，荧光笔叠加半透明。
//!
//! 运行：`cargo run -p showcase --bin ink`

use rgpui::ink::{InkLayer, InkLayerElement, InkStyle, InkTool};
use rgpui::{
    App, Bounds, Button, Context, IntoElement, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, Render, SharedString, Styled as _, TitlebarOptions, Window,
    WindowBounds, WindowOptions, div, hsla, prelude::*, px, size,
};
use rgpui_platform::application;

/// 钢笔宽度。
const PEN_WIDTH: Pixels = px(3.0);
/// 荧光笔宽度。
const HIGHLIGHTER_WIDTH: Pixels = px(16.0);
/// 橡皮半径。
const ERASER_RADIUS: Pixels = px(14.0);

/// Ink 示例应用。
struct InkApp {
    /// 墨迹层。
    layer: InkLayer,
    /// 当前工具。
    tool: InkTool,
    /// 进行中的套索多边形。
    lasso: Vec<Point<Pixels>>,
    /// 套索选中的笔画索引。
    selected: Vec<usize>,
    /// 是否处于落笔状态。
    drawing: bool,
    /// 状态提示。
    status: SharedString,
}

impl InkApp {
    /// 创建示例应用。
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            layer: InkLayer::new(),
            tool: InkTool::Pen,
            lasso: Vec::new(),
            selected: Vec::new(),
            drawing: false,
            status: "拖拽绘制 · 切换工具试试橡皮与套索".into(),
        }
    }

    /// 当前工具对应的画笔样式。
    fn style(&self) -> InkStyle {
        match self.tool {
            InkTool::Highlighter => {
                InkStyle::highlighter(hsla(0.13, 1.0, 0.5, 1.0), HIGHLIGHTER_WIDTH)
            }
            _ => InkStyle::pen(hsla(0.6, 0.75, 0.35, 1.0), PEN_WIDTH),
        }
    }

    /// 落笔。
    fn on_down(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.drawing = true;
        self.selected.clear();
        match self.tool {
            InkTool::Pen | InkTool::Highlighter => self.layer.begin_stroke(position, self.style()),
            InkTool::Eraser => {
                let removed = self.layer.erase_at(position, ERASER_RADIUS);
                self.status = format!("擦除 {removed} 条笔画").into();
            }
            InkTool::Lasso => self.lasso = vec![position],
        }
        cx.notify();
    }

    /// 拖拽中。
    fn on_move(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if !self.drawing {
            return;
        }
        match self.tool {
            InkTool::Pen | InkTool::Highlighter => self.layer.extend_stroke(position),
            InkTool::Eraser => {
                self.layer.erase_at(position, ERASER_RADIUS);
            }
            InkTool::Lasso => self.lasso.push(position),
        }
        cx.notify();
    }

    /// 抬笔。
    fn on_up(&mut self, cx: &mut Context<Self>) {
        if !self.drawing {
            return;
        }
        self.drawing = false;
        match self.tool {
            InkTool::Pen | InkTool::Highlighter => {
                self.layer.end_stroke();
            }
            InkTool::Lasso => {
                self.selected = self.layer.lasso(&self.lasso);
                self.status = format!("套索选中 {} 条笔画", self.selected.len()).into();
                self.lasso.clear();
            }
            InkTool::Eraser => {}
        }
        cx.notify();
    }

    /// 切换工具。
    fn set_tool(&mut self, tool: InkTool, cx: &mut Context<Self>) {
        self.tool = tool;
        self.status = format!("当前工具：{tool:?}").into();
        cx.notify();
    }

    /// 删除选中的笔画。
    fn delete_selected(&mut self, cx: &mut Context<Self>) {
        let count = self.selected.len();
        self.layer.remove(&self.selected);
        self.selected.clear();
        self.status = format!("删除 {count} 条选中笔画").into();
        cx.notify();
    }

    /// 撤销最近一条笔画。
    fn undo(&mut self, cx: &mut Context<Self>) {
        self.selected.clear();
        let undone = self.layer.undo();
        self.status = if undone {
            "已撤销一条笔画".into()
        } else {
            "没有可撤销的笔画".into()
        };
        cx.notify();
    }

    /// 清空。
    fn clear(&mut self, cx: &mut Context<Self>) {
        self.layer.clear();
        self.selected.clear();
        self.status = "已清空".into();
        cx.notify();
    }
}

impl Render for InkApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tool_button = |id: &'static str, label: &'static str, tool: InkTool| {
            Button::new(id)
                .label(label)
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    this.set_tool(tool, cx);
                }))
        };

        div()
            .relative()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            // 墨迹画布层：铺满窗口（坐标即窗口坐标），在最底层。
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .child(
                        InkLayerElement::new(self.layer.clone())
                            .highlight(self.selected.clone())
                            .lasso(self.lasso.clone()),
                    )
                    .on_mouse_down(
                        rgpui::MouseButton::Left,
                        cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                            this.on_down(ev.position, cx);
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
                        this.on_move(ev.position, cx);
                    }))
                    .on_mouse_up(
                        rgpui::MouseButton::Left,
                        cx.listener(|this, _ev: &MouseUpEvent, _window, cx| this.on_up(cx)),
                    ),
            )
            // 工具条：绘制在画布之上。
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap_2()
                            .child(tool_button("tool-pen", "钢笔", InkTool::Pen))
                            .child(tool_button(
                                "tool-highlighter",
                                "荧光笔",
                                InkTool::Highlighter,
                            ))
                            .child(tool_button("tool-eraser", "橡皮", InkTool::Eraser))
                            .child(tool_button("tool-lasso", "套索", InkTool::Lasso))
                            .child(Button::new("undo").label("撤销").on_click(cx.listener(
                                |this, _event, _window, cx| {
                                    this.undo(cx);
                                },
                            )))
                            .child(Button::new("delete-selected").label("删除选中").on_click(
                                cx.listener(|this, _event, _window, cx| {
                                    this.delete_selected(cx);
                                }),
                            ))
                            .child(
                                Button::new("clear").label("清空").on_click(
                                    cx.listener(|this, _event, _window, cx| this.clear(cx)),
                                ),
                            ),
                    )
                    .child(div().text_sm().child(self.status.clone()))
                    .child(div().text_xs().child(format!(
                        "笔画数：{} · 选中：{}",
                        self.layer.strokes.len(),
                        self.selected.len()
                    ))),
            )
    }
}

fn main() {
    rgpui::init_logging();
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(760.), px(560.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Ink Example".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| InkApp::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}
