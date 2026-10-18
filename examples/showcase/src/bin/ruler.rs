//! 标尺与对齐参考线演示：顶部标尺刻度 + 拖拽方块互相吸附并显示参考线。
//!
//! 刻度由 `rgpui::guides::ruler_ticks` 计算，吸附由 `rgpui::guides::align` 计算。
//! `Ctrl/⌘` + 滚轮缩放标尺步长（仅影响标尺演示）。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::guides::{Guide, GuideAxis, align, ruler_ticks};
use rgpui::{
    App, Bounds, Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    Render, ScrollWheelEvent, Window, WindowBounds, WindowOptions, bounds, canvas, div, fill,
    outline, point, prelude::*, px, rgb, size, v_flex,
};
use rgpui_platform::application;

/// 标尺条高度。
const RULER_HEIGHT: f32 = 28.0;
/// 方块边长。
const BOX: f32 = 80.0;
/// 吸附阈值。
const SNAP: f32 = 6.0;

/// 演示视图。
struct RulerDemo {
    zoom: f32,
    boxes: [Bounds<Pixels>; 2],
    dragging: Option<usize>,
    drag_offset: Point<Pixels>,
    guides: Vec<Guide>,
}

impl RulerDemo {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            zoom: 1.0,
            boxes: [
                bounds(point(px(160.0), px(160.0)), size(px(BOX), px(BOX))),
                bounds(point(px(360.0), px(280.0)), size(px(BOX), px(BOX))),
            ],
            dragging: None,
            drag_offset: point(px(0.0), px(0.0)),
            guides: Vec::new(),
        }
    }
}

impl Render for RulerDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let zoom = self.zoom;
        let boxes = self.boxes;
        let guides = self.guides.clone();

        v_flex().size_full().child(
            div()
                .size_full()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, _, cx| {
                        for (index, rect) in this.boxes.iter().enumerate() {
                            if rect.contains(&event.position) {
                                this.dragging = Some(index);
                                this.drag_offset = event.position - rect.origin;
                                break;
                            }
                        }
                        cx.notify();
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, _, cx| {
                        this.dragging = None;
                        this.guides.clear();
                        cx.notify();
                    }),
                )
                .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                    let Some(index) = this.dragging else {
                        return;
                    };
                    let origin = event.position - this.drag_offset;
                    let mut rect = bounds(origin, size(px(BOX), px(BOX)));
                    let other = this.boxes[1 - index];
                    let result = align(rect, &[other], &[], &[], px(SNAP));
                    rect.origin += result.offset;
                    this.boxes[index] = rect;
                    this.guides = result.guides;
                    cx.notify();
                }))
                .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                    if event.modifiers.secondary() {
                        let dy = match event.delta {
                            rgpui::ScrollDelta::Lines(delta) => delta.y,
                            rgpui::ScrollDelta::Pixels(delta) => delta.y.as_f32() / 40.0,
                        };
                        this.zoom = (this.zoom * (1.0 + dy * 0.1)).clamp(0.25, 4.0);
                        cx.notify();
                    }
                }))
                .child(canvas(
                    |area, _window, _cx| area,
                    move |area, _state, window, _cx| {
                        window.paint_quad(fill(area, rgb(0xf8fafc)));

                        // 顶部标尺条。
                        let ruler = bounds(area.origin, size(area.size.width, px(RULER_HEIGHT)));
                        window.paint_quad(fill(ruler, rgb(0xe2e8f0)));
                        window.paint_quad(outline(
                            ruler,
                            rgb(0xcbd5e1),
                            rgpui::BorderStyle::default(),
                        ));

                        // 刻度：可见文档区间 0..(宽度/zoom)。
                        let visible = area.size.width.as_f32() / zoom;
                        let ticks = ruler_ticks(0.0..visible, zoom, 60.0);
                        for (index, tick) in ticks.ticks.iter().enumerate() {
                            let x = area.origin.x + px(tick * zoom);
                            let major = index % 5 == 0;
                            let height = if major { 12.0 } else { 6.0 };
                            window.paint_quad(fill(
                                bounds(
                                    point(x, area.origin.y + px(RULER_HEIGHT - height)),
                                    size(px(1.0), px(height)),
                                ),
                                rgb(0x64748b),
                            ));
                        }

                        // 方块。
                        for rect in boxes.iter() {
                            window.paint_quad(fill(*rect, rgb(0xdbeafe)));
                            window.paint_quad(outline(
                                *rect,
                                rgb(0x2563eb),
                                rgpui::BorderStyle::default(),
                            ));
                        }

                        // 参考线。
                        for guide in guides.iter() {
                            let (line, color) = match guide.axis {
                                GuideAxis::Vertical => (
                                    bounds(
                                        point(guide.position, area.origin.y),
                                        size(px(1.0), area.size.height),
                                    ),
                                    rgb(0xdc2626),
                                ),
                                GuideAxis::Horizontal => (
                                    bounds(
                                        point(area.origin.x, guide.position),
                                        size(area.size.width, px(1.0)),
                                    ),
                                    rgb(0xdc2626),
                                ),
                            };
                            window.paint_quad(fill(line, color));
                        }
                    },
                )),
        )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(820.0), px(560.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| RulerDemo::new(window, cx)),
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
