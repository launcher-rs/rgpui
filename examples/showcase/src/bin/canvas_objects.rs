//! 画布对象工具演示：选区 / 缩放手柄 / 旋转手柄 / 对齐 / 分布。
//!
//! 三个矩形对象放在可缩放平移的画布上：点击选中、拖动移动、拖手柄缩放、拖顶部旋转
//! 手柄旋转；工具栏可左/水平居中/右对齐与水平分布。几何全部来自
//! `rgpui::object_toolkit`，屏幕映射来自 `rgpui::viewport::Viewport`。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::object_toolkit::{
    AlignMode, ResizeHandle, SelectionHandles, align, distribute, hit_handle, resize_bounds,
    union_bounds,
};
use rgpui::viewport::Viewport;
use rgpui::{
    App, Axis, Bounds, Button, Context, FocusHandle, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, ScrollWheelEvent, Styled,
    Window, WindowBounds, WindowOptions, div, h_flex, point, prelude::*, px, rgb, size, v_flex,
};
use rgpui_platform::application;

/// 画布对象。
struct CanvasObject {
    /// 边界（文档坐标）。
    rect: Bounds<Pixels>,
    /// 旋转角度（度，顺时针）。
    rotation: f32,
    /// 填充色。
    color: rgpui::Hsla,
    /// 标签。
    label: &'static str,
}

/// 拖拽模式。
enum Drag {
    /// 未拖拽。
    None,
    /// 移动：记录起点文档坐标。
    Move(Point<Pixels>),
    /// 缩放：记录手柄。
    Resize(ResizeHandle),
    /// 旋转：记录中心、起始指针角度与起始角度。
    Rotate {
        /// 旋转中心（文档坐标）。
        center: Point<Pixels>,
        /// 起始指针相对中心的角度（弧度）。
        start_angle: f32,
        /// 起始旋转角度（度）。
        start_rotation: f32,
    },
}

/// 演示视图。
struct CanvasObjectsDemo {
    /// 对象集合。
    objects: Vec<CanvasObject>,
    /// 当前选中对象下标。
    selected: Option<usize>,
    /// 视口。
    viewport: Viewport,
    /// 拖拽状态。
    drag: Drag,
    /// 焦点句柄。
    focus_handle: FocusHandle,
}

impl CanvasObjectsDemo {
    /// 创建演示视图。
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let objects = vec![
            CanvasObject {
                rect: Bounds::new(point(px(80.0), px(80.0)), size(px(220.0), px(140.0))),
                rotation: 0.0,
                color: rgb(0x60a5fa).into(),
                label: "卡片 A",
            },
            CanvasObject {
                rect: Bounds::new(point(px(360.0), px(120.0)), size(px(180.0), px(200.0))),
                rotation: 0.0,
                color: rgb(0x34d399).into(),
                label: "卡片 B",
            },
            CanvasObject {
                rect: Bounds::new(point(px(140.0), px(320.0)), size(px(260.0), px(120.0))),
                rotation: 0.0,
                color: rgb(0xfbbf24).into(),
                label: "卡片 C",
            },
        ];
        Self {
            objects,
            selected: None,
            viewport: Viewport::new().with_offset(point(px(48.0), px(24.0))),
            drag: Drag::None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// 选区（选中对象的边界，文档坐标）。
    fn selection(&self) -> Option<Bounds<Pixels>> {
        self.selected.map(|i| self.objects[i].rect)
    }

    /// 鼠标按下：命中手柄 → 缩放/旋转；命中对象 → 选中并移动；否则取消选择。
    fn on_mouse_down(&mut self, screen: Point<Pixels>, cx: &mut Context<Self>) {
        let doc = self.viewport.screen_to_document(screen);
        if let Some(selection) = self.selection()
            && let Some(handle) = hit_handle(selection, doc, px(7.0))
        {
            let index = self.selected.unwrap();
            if handle.is_rotate() {
                let center = point(
                    selection.origin.x + selection.size.width / 2.0,
                    selection.origin.y + selection.size.height / 2.0,
                );
                let start_angle = (doc.y - center.y)
                    .as_f32()
                    .atan2((doc.x - center.x).as_f32());
                self.drag = Drag::Rotate {
                    center,
                    start_angle,
                    start_rotation: self.objects[index].rotation,
                };
            } else {
                self.drag = Drag::Resize(handle);
            }
            cx.notify();
            return;
        }
        // 从上到下命中对象。
        let hit = (0..self.objects.len())
            .rev()
            .find(|i| self.objects[*i].rect.contains(&doc));
        self.selected = hit;
        self.drag = match hit {
            Some(_) => Drag::Move(doc),
            None => Drag::None,
        };
        cx.notify();
    }

    /// 鼠标移动：按拖拽模式更新。
    fn on_mouse_move(&mut self, screen: Point<Pixels>, cx: &mut Context<Self>) {
        let doc = self.viewport.screen_to_document(screen);
        let Some(index) = self.selected else {
            return;
        };
        match self.drag {
            Drag::Move(last) => {
                let delta_x = doc.x - last.x;
                let delta_y = doc.y - last.y;
                let rect = &mut self.objects[index].rect;
                rect.origin = point(rect.origin.x + delta_x, rect.origin.y + delta_y);
                self.drag = Drag::Move(doc);
                cx.notify();
            }
            Drag::Resize(handle) => {
                let rect = self.objects[index].rect;
                self.objects[index].rect = resize_bounds(rect, handle, doc, px(24.0));
                cx.notify();
            }
            Drag::Rotate {
                center,
                start_angle,
                start_rotation,
            } => {
                let now = (doc.y - center.y)
                    .as_f32()
                    .atan2((doc.x - center.x).as_f32());
                let delta = now - start_angle;
                self.objects[index].rotation = start_rotation + delta.to_degrees();
                cx.notify();
            }
            Drag::None => {}
        }
    }

    /// 鼠标释放：结束拖拽。
    fn on_mouse_up(&mut self, cx: &mut Context<Self>) {
        self.drag = Drag::None;
        cx.notify();
    }

    /// 对全部对象应用对齐。
    fn apply_align(&mut self, mode: AlignMode, cx: &mut Context<Self>) {
        let rects: Vec<Bounds<Pixels>> = self.objects.iter().map(|o| o.rect).collect();
        let aligned = align(&rects, mode);
        for (object, rect) in self.objects.iter_mut().zip(aligned) {
            object.rect = rect;
        }
        cx.notify();
    }

    /// 水平分布全部对象。
    fn apply_distribute(&mut self, cx: &mut Context<Self>) {
        let rects: Vec<Bounds<Pixels>> = self.objects.iter().map(|o| o.rect).collect();
        let distributed = distribute(&rects, Axis::Horizontal);
        for (object, rect) in self.objects.iter_mut().zip(distributed) {
            object.rect = rect;
        }
        cx.notify();
    }
}

impl Render for CanvasObjectsDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = self.viewport;
        let union = union_bounds(&self.objects.iter().map(|o| o.rect).collect::<Vec<_>>());

        // 对象层（后绘制的在上）。
        let objects: Vec<rgpui::AnyElement> = self
            .objects
            .iter()
            .enumerate()
            .map(|(index, object)| {
                let origin = viewport.document_to_screen(object.rect.origin);
                let size = viewport.document_size_to_screen(object.rect.size);
                let selected = self.selected == Some(index);
                div()
                    .absolute()
                    .left(origin.x)
                    .top(origin.y)
                    .w(size.width)
                    .h(size.height)
                    .rotate(object.rotation)
                    .rounded(px(8.0))
                    .border_2()
                    .border_color(if selected {
                        rgb(0x2563eb)
                    } else {
                        rgb(0x00000000)
                    })
                    .bg(object.color)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(rgb(0x0f172a))
                    .child(object.label)
                    .into_any_element()
            })
            .collect();

        let overlay = self.selection().map(|selection| {
            SelectionHandles::new(selection, viewport)
                .rotate(true)
                .into_any_element()
        });

        v_flex()
            .id("canvas-objects-demo")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(rgb(0xf8fafc))
            .child(
                h_flex()
                    .gap(px(6.0))
                    .px(px(12.0))
                    .py(px(8.0))
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(0xcbd5e1))
                    .child(
                        div()
                            .mr(px(6.0))
                            .text_sm()
                            .font_weight(rgpui::FontWeight::SEMIBOLD)
                            .child("画布对象工具"),
                    )
                    .child(Button::new("obj-left").label("左对齐").on_click(
                        cx.listener(|this, _, _, cx| this.apply_align(AlignMode::Left, cx)),
                    ))
                    .child(
                        Button::new("obj-hcenter")
                            .label("水平居中")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.apply_align(AlignMode::CenterHorizontal, cx)
                            })),
                    )
                    .child(Button::new("obj-right").label("右对齐").on_click(
                        cx.listener(|this, _, _, cx| this.apply_align(AlignMode::Right, cx)),
                    ))
                    .child(
                        Button::new("obj-distribute")
                            .label("水平分布")
                            .on_click(cx.listener(|this, _, _, cx| this.apply_distribute(cx))),
                    )
                    .child(
                        div()
                            .ml_auto()
                            .text_xs()
                            .text_color(rgb(0x64748b))
                            .child(format!(
                                "缩放 {:.0}% · 选中 {}",
                                viewport.zoom() * 100.0,
                                self.selected.map(|i| self.objects[i].label).unwrap_or("无")
                            )),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_grow_1()
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            this.on_mouse_down(event.position, cx);
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        this.on_mouse_move(event.position, cx);
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, cx| this.on_mouse_up(cx)),
                    )
                    .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                        this.viewport.apply_wheel(event, px(50.0));
                        cx.notify();
                    }))
                    .children(objects)
                    .children(overlay)
                    .child(
                        div()
                            .absolute()
                            .left(px(12.0))
                            .bottom(px(8.0))
                            .text_xs()
                            .text_color(rgb(0x94a3b8))
                            .child(match union {
                                Some(u) => format!(
                                    "对象外接框：{:.0} × {:.0}",
                                    u.size.width.as_f32(),
                                    u.size.height.as_f32()
                                ),
                                None => String::new(),
                            }),
                    ),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(920.0), px(680.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| CanvasObjectsDemo::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
