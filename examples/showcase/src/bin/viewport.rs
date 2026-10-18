//! 视口演示：可缩放/平移的 A4 文档页视图。
//!
//! 页面由 `canvas` 自绘，坐标全部经 `rgpui::viewport::Viewport` 在
//! 文档坐标与屏幕坐标间换算；滚轮 `Ctrl/⌘` 以光标为中心缩放、否则平移，
//! 左键拖拽平移。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::viewport::Viewport;
use rgpui::{
    App, Bounds, Button, Context, FocusHandle, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, Render, ScrollWheelEvent, Size, Window, WindowBounds,
    WindowOptions, bounds, canvas, div, fill, h_flex, outline, point, prelude::*, px, rgb, size,
    v_flex,
};
use rgpui_platform::application;

/// A4 页面尺寸（96 DPI，像素）。
fn page_size() -> Size<Pixels> {
    size(px(794.0), px(1123.0))
}

/// 页面边距。
const MARGIN: f32 = 72.0;

/// 演示视图。
struct ViewportDemo {
    viewport: Viewport,
    focus_handle: FocusHandle,
    dragging: bool,
    last_pointer: Point<Pixels>,
    anchor: Point<Pixels>,
    view_size: Size<Pixels>,
}

impl ViewportDemo {
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            viewport: Viewport::new()
                .with_zoom(0.6)
                .with_offset(point(px(80.0), px(24.0))),
            focus_handle: cx.focus_handle(),
            dragging: false,
            last_pointer: point(px(0.0), px(0.0)),
            anchor: point(px(400.0), px(300.0)),
            view_size: size(px(800.0), px(600.0)),
        }
    }
}

impl Render for ViewportDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 记录视口可用尺寸，供「适应窗口」使用。
        self.view_size = window.bounds().size;

        // `Viewport` 为 `Copy`，复制一份供绘制闭包按值捕获。
        let viewport = self.viewport;
        let page = page_size();

        v_flex()
            .id("viewport-demo")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(rgb(0xf1f5f9))
            .child(
                h_flex()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .py(px(8.0))
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(0xcbd5e1))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(rgpui::FontWeight::SEMIBOLD)
                            .child("视口（Viewport）"),
                    )
                    .child(
                        Button::new("vp-zoom-out")
                            .label("缩小")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let anchor = this.anchor;
                                this.viewport.zoom_out_at(anchor);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("vp-zoom-in")
                            .label("放大")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let anchor = this.anchor;
                                this.viewport.zoom_in_at(anchor);
                                cx.notify();
                            })),
                    )
                    .child(Button::new("vp-100").label("100%").on_click(cx.listener(
                        |this, _, _, cx| {
                            let anchor = this.anchor;
                            this.viewport.set_zoom_at(1.0, anchor);
                            cx.notify();
                        },
                    )))
                    .child(
                        Button::new("vp-fit")
                            .label("适应窗口")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let view = this.view_size;
                                this.viewport.fit(page_size(), view, px(32.0));
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .ml_auto()
                            .text_xs()
                            .text_color(rgb(0x64748b))
                            .child(format!(
                                "缩放 {:.0}% · 偏移 ({:.0}, {:.0})",
                                viewport.zoom() * 100.0,
                                viewport.offset().x.as_f32(),
                                viewport.offset().y.as_f32()
                            )),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .relative()
                    .overflow_hidden()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            this.dragging = true;
                            this.last_pointer = event.position;
                            cx.notify();
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, cx| {
                            this.dragging = false;
                            cx.notify();
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        this.anchor = event.position;
                        if this.dragging {
                            let delta = event.position - this.last_pointer;
                            this.last_pointer = event.position;
                            this.viewport.pan_by(delta);
                        }
                        cx.notify();
                    }))
                    .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                        this.viewport.apply_wheel(event, px(50.0));
                        cx.notify();
                    }))
                    .child(canvas(
                        |bounds, _window, _cx| bounds,
                        move |area, _state, window, _cx| {
                            let to_screen = |x: f32, y: f32| -> Point<Pixels> {
                                viewport.document_to_screen(point(px(x), px(y)))
                            };
                            let doc_rect = |x: f32, y: f32, w: f32, h: f32| -> Bounds<Pixels> {
                                bounds(
                                    to_screen(x, y),
                                    viewport.document_size_to_screen(size(px(w), px(h))),
                                )
                            };

                            window.paint_quad(fill(area, rgb(0xe2e8f0)));

                            let paper =
                                doc_rect(0.0, 0.0, page.width.as_f32(), page.height.as_f32());
                            window.paint_quad(fill(paper, rgb(0xffffff)));
                            window.paint_quad(outline(
                                paper,
                                rgb(0x94a3b8),
                                rgpui::BorderStyle::default(),
                            ));

                            let content_width = page.width.as_f32() - MARGIN * 2.0;
                            window.paint_quad(fill(
                                doc_rect(MARGIN, MARGIN, content_width, 40.0),
                                rgb(0x1e293b),
                            ));

                            // 用等宽色条模拟正文段落。
                            let mut y = MARGIN + 80.0;
                            let mut line = 0usize;
                            while y < page.height.as_f32() - MARGIN {
                                let width = if line % 5 == 4 {
                                    content_width * 0.6
                                } else {
                                    content_width
                                };
                                window.paint_quad(fill(
                                    doc_rect(MARGIN, y, width, 10.0),
                                    rgb(0xcbd5e1),
                                ));
                                y += 24.0;
                                line += 1;
                            }
                        },
                    )),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(900.0), px(680.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| ViewportDemo::new(window, cx)),
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
