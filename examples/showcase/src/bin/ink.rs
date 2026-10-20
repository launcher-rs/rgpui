//! Ink 笔迹示例
//!
//! 演示 `ink` 墨迹层：鼠标拖拽用钢笔 / 荧光笔落笔，橡皮按半径擦除，套索框选后删除；
//! 另含撤销与清空。笔画以中点二次贝塞尔平滑绘制，荧光笔叠加半透明；打开「压感笔」后
//! 按轨迹长度合成压感，笔画逐段变宽（真实数位板压感走同一接口）。
//!
//! 多图层：`InkDocument` 管理图层（新增 / 切换 / 隐藏 / 锁定），编辑落在激活图层，
//! 可见层逐层绘制并把图层不透明度传给 `InkLayerElement`。
//!
//! 运行：`cargo run -p showcase --bin ink`

use rgpui::ink::{InkDocument, InkLayerElement, InkStyle, InkTool};
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
    /// 多图层墨迹文档。
    doc: InkDocument,
    /// 当前工具。
    tool: InkTool,
    /// 进行中的套索多边形。
    lasso: Vec<Point<Pixels>>,
    /// 套索选中的 `(图层, 笔画)`。
    selected: Vec<(usize, usize)>,
    /// 是否处于落笔状态。
    drawing: bool,
    /// 是否用合成压感绘制（演示逐段变宽）。
    pressure: bool,
    /// 本次笔画的起点（用来算合成压感）。
    stroke_origin: Option<Point<Pixels>>,
    /// 状态提示。
    status: SharedString,
}

impl InkApp {
    /// 创建示例应用。
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            doc: InkDocument::new(),
            tool: InkTool::Pen,
            lasso: Vec::new(),
            selected: Vec::new(),
            drawing: false,
            pressure: false,
            stroke_origin: None,
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
        self.stroke_origin = Some(position);
        match self.tool {
            InkTool::Pen | InkTool::Highlighter => {
                let started = if self.pressure {
                    self.doc
                        .begin_stroke_with_pressure(position, 0.35, self.style())
                } else {
                    self.doc.begin_stroke(position, self.style())
                };
                if !started {
                    self.status = "当前图层已隐藏或锁定，无法落笔".into();
                }
            }
            InkTool::Eraser => {
                let removed = self.doc.erase_at(position, ERASER_RADIUS);
                self.status = format!("擦除 {removed} 条笔画").into();
            }
            InkTool::Lasso => self.lasso = vec![position],
        }
        cx.notify();
    }

    /// 合成本次采样的压感：沿笔画长度做正弦起伏，便于观察逐段变宽。
    fn synthetic_pressure(&self, position: Point<Pixels>) -> f32 {
        let Some(origin) = self.stroke_origin else {
            return 1.0;
        };
        let dx = (position.x - origin.x).as_f32();
        let dy = (position.y - origin.y).as_f32();
        let distance = (dx * dx + dy * dy).sqrt();
        0.25 + 0.75 * (0.5 + 0.5 * (distance / 45.0).sin())
    }

    /// 拖拽中。
    fn on_move(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if !self.drawing {
            return;
        }
        match self.tool {
            InkTool::Pen | InkTool::Highlighter => {
                if self.pressure {
                    let pressure = self.synthetic_pressure(position);
                    self.doc.extend_stroke_with_pressure(position, pressure);
                } else {
                    self.doc.extend_stroke(position);
                }
            }
            InkTool::Eraser => {
                self.doc.erase_at(position, ERASER_RADIUS);
            }
            InkTool::Lasso => self.lasso.push(position),
        }
        cx.notify();
    }

    /// 抬笔。
    fn on_up(&mut self, cx: &mut Context<Self>) {
        self.stroke_origin = None;
        if !self.drawing {
            return;
        }
        self.drawing = false;
        match self.tool {
            InkTool::Pen | InkTool::Highlighter => {
                self.doc.end_stroke();
            }
            InkTool::Lasso => {
                self.selected = self.doc.lasso(&self.lasso);
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

    /// 删除选中的笔画（跨图层，锁定 / 隐藏层跳过）。
    fn delete_selected(&mut self, cx: &mut Context<Self>) {
        let count = self.selected.len();
        self.doc.remove(&self.selected);
        self.selected.clear();
        self.status = format!("删除 {count} 条选中笔画").into();
        cx.notify();
    }

    /// 撤销激活图层最近一条笔画。
    fn undo(&mut self, cx: &mut Context<Self>) {
        self.selected.clear();
        let undone = self.doc.undo();
        self.status = if undone {
            "已撤销一条笔画".into()
        } else {
            "激活图层没有可撤销的笔画".into()
        };
        cx.notify();
    }

    /// 清空激活图层。
    fn clear(&mut self, cx: &mut Context<Self>) {
        self.doc.clear();
        self.selected.clear();
        self.status = "已清空激活图层".into();
        cx.notify();
    }

    /// 新建图层并激活。
    fn add_layer(&mut self, cx: &mut Context<Self>) {
        let name = format!("图层 {}", self.doc.layer_count() + 1);
        let index = self.doc.add_layer(name);
        self.status = format!("新建图层 #{}", index + 1).into();
        cx.notify();
    }

    /// 切换到下一图层。
    fn next_layer(&mut self, cx: &mut Context<Self>) {
        let next = (self.doc.active_index() + 1) % self.doc.layer_count();
        self.doc.set_active(next);
        self.status = format!("激活图层 #{}", next + 1).into();
        cx.notify();
    }

    /// 隐藏 / 显示激活图层。
    fn toggle_visible(&mut self, cx: &mut Context<Self>) {
        let index = self.doc.active_index();
        let visible = !self.doc.active().visible;
        self.doc.set_visible(index, visible);
        self.status = if visible {
            "显示激活图层".into()
        } else {
            "隐藏激活图层".into()
        };
        cx.notify();
    }

    /// 锁定 / 解锁激活图层。
    fn toggle_locked(&mut self, cx: &mut Context<Self>) {
        let index = self.doc.active_index();
        let locked = !self.doc.active().locked;
        self.doc.set_locked(index, locked);
        self.status = if locked {
            "锁定激活图层".into()
        } else {
            "解锁激活图层".into()
        };
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

        let active = self.doc.active_index();
        let layer_count = self.doc.layer_count();
        // 可见图层逐层绘制：套索只画在激活层，高亮按层过滤。
        let layer_elements: Vec<rgpui::AnyElement> = self
            .doc
            .visible_layers()
            .map(|(index, layer)| {
                let highlight = self
                    .selected
                    .iter()
                    .filter(|(selected_layer, _)| *selected_layer == index)
                    .map(|(_, stroke)| *stroke)
                    .collect();
                let lasso = if index == active {
                    self.lasso.clone()
                } else {
                    Vec::new()
                };
                InkLayerElement::new(layer.layer.clone())
                    .opacity(layer.opacity)
                    .highlight(highlight)
                    .lasso(lasso)
                    // 每层把笔画暴露给辅助技术（画布合成子节点，播报点数 / 工具）。
                    .a11y(("ink-layer", index), layer.name.clone())
                    .into_any_element()
            })
            .collect();
        let active_layer = self.doc.active().clone();
        let active_strokes = active_layer.layer.strokes.len();
        let selected_count = self.selected.len();
        let selected_label = if self.selected.is_empty() {
            String::new()
        } else {
            format!(" · 选中 {selected_count}")
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
                    .children(layer_elements)
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
                            .child(
                                Button::new("pressure")
                                    .label(if self.pressure {
                                        "压感笔：开"
                                    } else {
                                        "压感笔：关"
                                    })
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.pressure = !this.pressure;
                                        this.status = if this.pressure {
                                            "压感笔：笔画按合成压感逐段变宽".into()
                                        } else {
                                            "压感笔：固定宽度".into()
                                        };
                                        cx.notify();
                                    })),
                            )
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
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap_2()
                            .child(Button::new("add-layer").label("新建图层").on_click(
                                cx.listener(|this, _event, _window, cx| this.add_layer(cx)),
                            ))
                            .child(Button::new("next-layer").label("切换图层").on_click(
                                cx.listener(|this, _event, _window, cx| this.next_layer(cx)),
                            ))
                            .child(Button::new("toggle-visible").label("隐藏/显示").on_click(
                                cx.listener(|this, _event, _window, cx| {
                                    this.toggle_visible(cx);
                                }),
                            ))
                            .child(Button::new("toggle-locked").label("锁定/解锁").on_click(
                                cx.listener(|this, _event, _window, cx| {
                                    this.toggle_locked(cx);
                                }),
                            )),
                    )
                    .child(div().text_sm().child(self.status.clone()))
                    .child(div().text_xs().child(format!(
                        "图层 {}/{}（{}）· 激活层笔画 {} · 可见层 {}{}",
                        active + 1,
                        layer_count,
                        if active_layer.visible {
                            "可见"
                        } else {
                            "隐藏"
                        },
                        active_strokes,
                        self.doc.visible_layers().count(),
                        selected_label,
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
