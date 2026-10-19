//! 文档页视图演示：`PageView` 分页画布 + `Viewport` 缩放/平移。
//!
//! 三页 A4 文档纵向排布，`PageView` 绘制纸页与页眉页脚，正文按
//! `PageLayout::content_rect` 导出的矩形叠加（字号随缩放同步）。滚轮 `Ctrl/⌘`
//! 以光标为中心缩放、否则平移；左键拖拽平移；工具栏可缩小/放大/100%/适应窗口。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::page_view::{PageLabels, PageLayout, PageSetup, PageView};
use rgpui::viewport::Viewport;
use rgpui::{
    App, Bounds, Button, Context, FocusHandle, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, ScrollWheelEvent,
    SharedString, Size, Styled, Window, WindowBounds, WindowOptions, div, h_flex, prelude::*, px,
    rgb, size, v_flex,
};
use rgpui_platform::application;

/// 演示页数。
const PAGE_COUNT: usize = 3;

/// 页眉页脚文本。
struct DocLabels {
    /// 页眉标题。
    title: SharedString,
}

impl PageLabels for DocLabels {
    fn header(&self, _page: usize, _cx: &App) -> SharedString {
        self.title.clone()
    }

    fn footer(&self, page: usize, _cx: &App) -> SharedString {
        format!("第 {} / {PAGE_COUNT} 页", page + 1).into()
    }
}

/// 每页正文（框架不做分页，段落由应用侧提供）。
fn page_text(page: usize) -> &'static str {
    match page {
        0 => {
            "文档页视图（Document Page View）\n\n\
              本演示用 PageView 渲染分页纸页：纸张尺寸、页边距、页眉页脚高度与页间距\
              共同决定文档坐标，屏幕映射交给 Viewport。\n\n\
              正文由应用按 PageLayout::content_rect(page) 的矩形叠加，\
              框架只出几何与纸页外观，分页/断行算法留在应用层。"
        }
        1 => {
            "坐标系统\n\n\
              screen = document × zoom + offset。\n\n\
              滚轮 Ctrl/⌘ 以光标为锚点缩放；普通滚轮平移；左键拖拽亦可平移。\n\n\
              页与页之间保留 gap 间距，命中判定 page_at 会把间距内的点归给上一页。"
        }
        _ => {
            "内容区\n\n\
              content_rect 扣除了左右边距与页眉页脚占位，\
              是正文可安全排布的竖直带状区域。\n\n\
              页眉贴内容区上方、页脚贴内容区下方并居中，随缩放一起移动。\n\n\
              —— 演示结束 ——"
        }
    }
}

/// 演示视图。
struct PageViewDemo {
    /// 分页布局。
    layout: PageLayout,
    /// 视口（缩放 + 平移）。
    viewport: Viewport,
    /// 焦点句柄。
    focus_handle: FocusHandle,
    /// 画布可用尺寸。
    view_size: Size<Pixels>,
    /// 缩放/平移锚点（最近鼠标位置）。
    anchor: Point<Pixels>,
    /// 是否正在拖拽平移。
    dragging: bool,
    /// 上次指针位置。
    last_pointer: Point<Pixels>,
}

impl PageViewDemo {
    /// 创建演示视图。
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let setup = PageSetup::default();
        let layout = PageLayout::new(setup, PAGE_COUNT);
        Self {
            layout,
            viewport: Viewport::new()
                .with_zoom(0.55)
                .with_offset(px_point(120.0, 16.0)),
            focus_handle: cx.focus_handle(),
            view_size: size(px(900.0), px(640.0)),
            anchor: px_point(450.0, 320.0),
            dragging: false,
            last_pointer: px_point(0.0, 0.0),
        }
    }
}

/// `point(px(x), px(y))` 简写。
fn px_point(x: f32, y: f32) -> Point<Pixels> {
    rgpui::point(px(x), px(y))
}

impl Render for PageViewDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if window.bounds().size != self.view_size && !self.dragging {
            self.view_size = window.bounds().size;
        }

        let viewport = self.viewport;
        let layout = self.layout.clone();
        let zoom = viewport.zoom();
        let font_size = px((14.0 * zoom).max(4.0));

        // 正文叠加：按文档矩形换算到屏幕，字号随缩放同步。
        let mut overlays: Vec<rgpui::AnyElement> = Vec::new();
        for page in 0..layout.page_count() {
            let content = layout.content_rect(page);
            let origin = viewport.document_to_screen(content.origin);
            let size = viewport.document_size_to_screen(content.size);
            overlays.push(
                div()
                    .absolute()
                    .left(origin.x)
                    .top(origin.y)
                    .w(size.width)
                    .h(size.height)
                    .p(px(4.0))
                    .text_size(font_size)
                    .line_height(rgpui::relative(1.4))
                    .text_color(rgb(0x1e293b))
                    .child(page_text(page))
                    .into_any_element(),
            );
        }

        v_flex()
            .id("page-view-demo")
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
                            .child("文档页视图（PageView）"),
                    )
                    .child(
                        Button::new("pv-zoom-out")
                            .label("缩小")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let anchor = this.anchor;
                                this.viewport.zoom_out_at(anchor);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("pv-zoom-in")
                            .label("放大")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let anchor = this.anchor;
                                this.viewport.zoom_in_at(anchor);
                                cx.notify();
                            })),
                    )
                    .child(Button::new("pv-100").label("100%").on_click(cx.listener(
                        |this, _, _, cx| {
                            let anchor = this.anchor;
                            this.viewport.set_zoom_at(1.0, anchor);
                            cx.notify();
                        },
                    )))
                    .child(
                        Button::new("pv-fit")
                            .label("适应窗口")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let view = this.view_size;
                                let doc = this.layout.document_size();
                                this.viewport.fit(doc, view, px(24.0));
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .ml_auto()
                            .text_xs()
                            .text_color(rgb(0x64748b))
                            .child(format!("缩放 {:.0}% · {} 页", zoom * 100.0, PAGE_COUNT)),
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
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .child(PageView::new(
                                layout,
                                viewport,
                                DocLabels {
                                    title: "rgpui 文档套件基座".into(),
                                },
                            )),
                    )
                    .children(overlays),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(980.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| PageViewDemo::new(window, cx)),
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
