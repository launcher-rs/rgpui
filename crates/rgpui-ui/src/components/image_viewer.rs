//! 图片查看器（ImageViewer / Lightbox）：全屏覆盖层展示多张图片，支持缩放、平移、缩略图导航与快捷键。

use rgpui::{Button, ButtonVariants as _, Focusable, IconName, prelude::FluentBuilder as _, *};
use std::rc::Rc;

/// 图片查看器快捷键上下文。
const CONTEXT: &str = "ImageViewer";

actions!(
    image_viewer,
    [
        ImageViewerClose,
        ImageViewerNext,
        ImageViewerPrev,
        ImageViewerZoomIn,
        ImageViewerZoomOut,
        ImageViewerResetZoom
    ]
);

/// 查看器中的单张图片信息。
#[derive(Clone)]
pub struct ImageItem {
    /// 图片资源地址。
    pub src: SharedString,
    /// 图片替代文本。
    pub alt: Option<SharedString>,
    /// 图片说明文字。
    pub caption: Option<SharedString>,
}

impl ImageItem {
    /// 创建图片项。
    pub fn new(src: impl Into<SharedString>) -> Self {
        Self {
            src: src.into(),
            alt: None,
            caption: None,
        }
    }

    /// 设置替代文本。
    pub fn alt(mut self, alt: impl Into<SharedString>) -> Self {
        self.alt = Some(alt.into());
        self
    }

    /// 设置说明文字。
    pub fn caption(mut self, caption: impl Into<SharedString>) -> Self {
        self.caption = Some(caption.into());
        self
    }
}

/// 图片适应方式（保留扩展位，当前渲染默认使用 Contain）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImageViewerSize {
    /// 自动。
    Auto,
    /// 完整显示。
    Contain,
    /// 覆盖显示。
    Cover,
    /// 自定义比例。
    Custom(f32),
}

impl Default for ImageViewerSize {
    fn default() -> Self {
        Self::Contain
    }
}

/// 最小缩放比例。
const MIN_ZOOM: f32 = 0.1;
/// 最大缩放比例。
const MAX_ZOOM: f32 = 5.0;
/// 每次缩放的步长。
const ZOOM_STEP: f32 = 0.25;

/// 图片查看器状态（作为实体共享）。
pub struct ImageViewerState {
    /// 图片列表。
    images: Vec<ImageItem>,
    /// 当前索引。
    current_index: usize,
    /// 当前缩放比例。
    zoom: f32,
    /// 平移偏移。
    pan_offset: Point<Pixels>,
    /// 是否正在拖拽平移（预留）。
    _is_panning: bool,
    /// 上次鼠标位置（预留）。
    _last_mouse_pos: Point<Pixels>,
    /// 图片加载中标志（预留）。
    _loading: bool,
    /// 是否显示缩略图。
    show_thumbnails: bool,
    /// 图片适应方式（预留）。
    _fit_mode: ImageViewerSize,
}

impl ImageViewerState {
    /// 创建图片查看器状态。
    pub fn new(images: Vec<ImageItem>) -> Self {
        Self {
            images,
            current_index: 0,
            zoom: 1.0,
            pan_offset: point(px(0.0), px(0.0)),
            _is_panning: false,
            _last_mouse_pos: point(px(0.0), px(0.0)),
            _loading: false,
            show_thumbnails: true,
            _fit_mode: ImageViewerSize::default(),
        }
    }

    /// 替换图片列表并回到第一张。
    pub fn set_images(&mut self, images: Vec<ImageItem>) {
        self.images = images;
        self.current_index = 0;
        self.reset_view();
    }

    /// 跳转到指定索引。
    pub fn go_to(&mut self, index: usize) {
        if index < self.images.len() {
            self.current_index = index;
            self.reset_view();
        }
    }

    /// 下一张。
    pub fn next(&mut self) {
        if self.current_index < self.images.len().saturating_sub(1) {
            self.current_index += 1;
            self.reset_view();
        }
    }

    /// 上一张。
    pub fn prev(&mut self) {
        if self.current_index > 0 {
            self.current_index -= 1;
            self.reset_view();
        }
    }

    /// 放大。
    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom + ZOOM_STEP).min(MAX_ZOOM);
    }

    /// 缩小。
    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom - ZOOM_STEP).max(MIN_ZOOM);
    }

    /// 重置缩放与平移。
    pub fn reset_zoom(&mut self) {
        self.zoom = 1.0;
        self.pan_offset = point(px(0.0), px(0.0));
    }

    /// 设置缩放比例（自动限制在最小与最大之间）。
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    /// 切换缩略图显示。
    pub fn toggle_thumbnails(&mut self) {
        self.show_thumbnails = !self.show_thumbnails;
    }

    /// 重置视图。
    fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan_offset = point(px(0.0), px(0.0));
    }

    /// 返回当前图片项。
    pub fn current_image(&self) -> Option<&ImageItem> {
        self.images.get(self.current_index)
    }

    /// 是否还有下一张。
    pub fn has_next(&self) -> bool {
        self.current_index < self.images.len().saturating_sub(1)
    }

    /// 是否还有上一张。
    pub fn has_prev(&self) -> bool {
        self.current_index > 0
    }

    /// 图片总数。
    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    /// 当前索引。
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// 当前缩放比例。
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// 是否已缩放（偏离 1.0）。
    pub fn is_zoomed(&self) -> bool {
        (self.zoom - 1.0).abs() > 0.01
    }
}

/// 图片查看器组件。
pub struct ImageViewer {
    /// 焦点句柄。
    focus_handle: FocusHandle,
    /// 共享状态实体。
    state: Entity<ImageViewerState>,
    /// 关闭回调。
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    /// 点击背景是否关闭。
    close_on_backdrop_click: bool,
    /// 按 Esc 是否关闭。
    close_on_escape: bool,
    /// 是否显示控制按钮。
    show_controls: bool,
    /// 是否显示计数。
    show_counter: bool,
    /// 是否显示缩略图。
    show_thumbnails: bool,
    /// 用户样式。
    style: StyleRefinement,
}

impl ImageViewer {
    /// 创建图片查看器。
    pub fn new(state: Entity<ImageViewerState>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            state,
            on_close: None,
            close_on_backdrop_click: true,
            close_on_escape: true,
            show_controls: true,
            show_counter: true,
            show_thumbnails: true,
            style: StyleRefinement::default(),
        }
    }

    /// 设置关闭回调。
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    /// 设置点击背景是否关闭。
    pub fn close_on_backdrop_click(mut self, close: bool) -> Self {
        self.close_on_backdrop_click = close;
        self
    }

    /// 设置按 Esc 是否关闭。
    pub fn close_on_escape(mut self, close: bool) -> Self {
        self.close_on_escape = close;
        self
    }

    /// 设置是否显示控制按钮。
    pub fn show_controls(mut self, show: bool) -> Self {
        self.show_controls = show;
        self
    }

    /// 设置是否显示计数。
    pub fn show_counter(mut self, show: bool) -> Self {
        self.show_counter = show;
        self
    }

    /// 设置是否显示缩略图。
    pub fn show_thumbnails(mut self, show: bool) -> Self {
        self.show_thumbnails = show;
        self
    }

    /// 触发关闭回调。
    fn handle_close(&self, window: &mut Window, cx: &mut App) {
        if let Some(handler) = &self.on_close {
            (handler)(window, cx);
        }
    }
}

impl Styled for ImageViewer {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Focusable for ImageViewer {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<()> for ImageViewer {}

impl Render for ImageViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        // 预先取出需要使用的主题值，避免 cx 借用与后续实体更新/闭包捕获冲突。
        let font_family = theme.font_family.clone();
        let primary = *theme.tokens.primary;
        let state = self.state.read(cx);
        let current_image = state.current_image().cloned();
        let current_index = state.current_index();
        let image_count = state.image_count();
        let zoom = state.zoom();
        let has_prev = state.has_prev();
        let has_next = state.has_next();
        let images = state.images.clone();
        let _pan_offset = state.pan_offset;
        let show_thumbs = self.show_thumbnails && state.show_thumbnails && image_count > 1;

        let viewer_entity = cx.entity().clone();
        let state_entity = self.state.clone();

        window.focus(&self.focus_handle, cx);

        let close_handler = self.on_close.clone();
        let close_on_escape = self.close_on_escape;
        let close_on_backdrop = self.close_on_backdrop_click;

        div()
            .id("image-viewer-overlay")
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .bg(rgpui::black().opacity(0.9))
            .on_action({
                let viewer_entity = viewer_entity.clone();
                move |_: &ImageViewerClose, window, cx| {
                    if close_on_escape {
                        cx.update_entity(&viewer_entity, |viewer, cx| {
                            viewer.handle_close(window, cx);
                        });
                    }
                }
            })
            .on_action({
                let state_entity = state_entity.clone();
                move |_: &ImageViewerNext, _, cx| {
                    cx.update_entity(&state_entity, |state, _| state.next());
                }
            })
            .on_action({
                let state_entity = state_entity.clone();
                move |_: &ImageViewerPrev, _, cx| {
                    cx.update_entity(&state_entity, |state, _| state.prev());
                }
            })
            .on_action({
                let state_entity = state_entity.clone();
                move |_: &ImageViewerZoomIn, _, cx| {
                    cx.update_entity(&state_entity, |state, _| state.zoom_in());
                }
            })
            .on_action({
                let state_entity = state_entity.clone();
                move |_: &ImageViewerZoomOut, _, cx| {
                    cx.update_entity(&state_entity, |state, _| state.zoom_out());
                }
            })
            .on_action({
                let state_entity = state_entity.clone();
                move |_: &ImageViewerResetZoom, _, cx| {
                    cx.update_entity(&state_entity, |state, _| state.reset_zoom());
                }
            })
            .child(
                div()
                    .id("image-viewer-header")
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(16.0))
                    .py(px(12.0))
                    .child(div().flex().items_center().gap(px(8.0)).when(
                        self.show_counter && image_count > 1,
                        |this| {
                            this.child(
                                div()
                                    .text_size(px(14.0))
                                    .text_color(rgpui::white())
                                    .font_family(font_family.clone())
                                    .child(format!("{} of {}", current_index + 1, image_count)),
                            )
                        },
                    ))
                    .child(div().flex().items_center().gap(px(4.0)).when(
                        self.show_controls,
                        |this| {
                            let state_entity = state_entity.clone();
                            let state_entity2 = state_entity.clone();
                            let state_entity3 = state_entity.clone();

                            this.child(
                                Button::new("zoom-out")
                                    .ghost()
                                    .icon(IconName::Minus)
                                    .on_click(move |_, _, cx| {
                                        cx.update_entity(&state_entity, |state, _| {
                                            state.zoom_out()
                                        });
                                    }),
                            )
                            .child(
                                div()
                                    .min_w(px(50.0))
                                    .text_size(px(13.0))
                                    .text_color(rgpui::white())
                                    .font_family(font_family.clone())
                                    .text_center()
                                    .child(format!("{}%", (zoom * 100.0) as i32)),
                            )
                            .child(
                                Button::new("zoom-in")
                                    .ghost()
                                    .icon(IconName::Plus)
                                    .on_click(move |_, _, cx| {
                                        cx.update_entity(&state_entity2, |state, _| {
                                            state.zoom_in()
                                        });
                                    }),
                            )
                            .child(
                                div()
                                    .w(px(1.0))
                                    .h(px(20.0))
                                    .bg(rgpui::white().opacity(0.2))
                                    .mx(px(8.0)),
                            )
                            .child(
                                Button::new("reset-zoom")
                                    .label("Reset")
                                    .ghost()
                                    .small()
                                    .on_click(move |_, _, cx| {
                                        cx.update_entity(&state_entity3, |state, _| {
                                            state.reset_zoom()
                                        });
                                    }),
                            )
                        },
                    ))
                    .child({
                        let viewer_entity = viewer_entity.clone();
                        Button::new("close-viewer")
                            .ghost()
                            .icon(IconName::Close)
                            .on_click(move |_, window, cx| {
                                cx.update_entity(&viewer_entity, |viewer, cx| {
                                    viewer.handle_close(window, cx);
                                });
                            })
                    }),
            )
            .child(
                div()
                    .id("image-viewer-content")
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .relative()
                    .overflow_hidden()
                    .when(close_on_backdrop, |this| {
                        let close_handler = close_handler.clone();
                        this.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            if let Some(handler) = &close_handler {
                                (handler)(window, cx);
                            }
                        })
                    })
                    .when(has_prev, |this| {
                        let state_entity = state_entity.clone();
                        this.child(
                            div()
                                .id("prev-button")
                                .absolute()
                                .left(px(16.0))
                                .top_0()
                                .bottom_0()
                                .flex()
                                .items_center()
                                .child(
                                    Button::new("prev-image")
                                        .secondary()
                                        .icon(IconName::ArrowLeft)
                                        .on_click(move |_, _, cx| {
                                            cx.update_entity(&state_entity, |state, _| {
                                                state.prev()
                                            });
                                        }),
                                ),
                        )
                    })
                    .when(has_next, |this| {
                        let state_entity = state_entity.clone();
                        this.child(
                            div()
                                .id("next-button")
                                .absolute()
                                .right(px(16.0))
                                .top_0()
                                .bottom_0()
                                .flex()
                                .items_center()
                                .child(
                                    Button::new("next-image")
                                        .secondary()
                                        .icon(IconName::ArrowRight)
                                        .on_click(move |_, _, cx| {
                                            cx.update_entity(&state_entity, |state, _| {
                                                state.next()
                                            });
                                        }),
                                ),
                        )
                    })
                    .when_some(current_image.clone(), |this, image| {
                        this.child(
                            div()
                                .id("image-container")
                                .flex()
                                .items_center()
                                .justify_center()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation();
                                })
                                .child(
                                    img(image.src.clone())
                                        .max_w(relative(0.9 * zoom))
                                        .max_h(relative(0.8 * zoom))
                                        .object_fit(ObjectFit::Contain),
                                ),
                        )
                    }),
            )
            .when_some(current_image.as_ref().and_then(|i| i.caption.clone()), {
                |this, caption| {
                    this.child(
                        div()
                            .id("image-caption")
                            .px(px(16.0))
                            .py(px(8.0))
                            .text_size(px(14.0))
                            .text_color(rgpui::white().opacity(0.8))
                            .font_family(font_family.clone())
                            .text_center()
                            .child(caption),
                    )
                }
            })
            .when(show_thumbs, |this| {
                this.child(
                    div()
                        .id("thumbnail-strip")
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(8.0))
                        .px(px(16.0))
                        .py(px(12.0))
                        .bg(rgpui::black().opacity(0.5))
                        .children(images.iter().enumerate().map(|(idx, image)| {
                            let is_current = idx == current_index;
                            let state_entity = state_entity.clone();

                            div()
                                .id(ElementId::Name(format!("thumb-{}", idx).into()))
                                .size(px(60.0))
                                .rounded(px(4.0))
                                .overflow_hidden()
                                .border_2()
                                .border_color(if is_current {
                                    primary
                                } else {
                                    rgpui::transparent_black()
                                })
                                .cursor(CursorStyle::PointingHand)
                                .hover(|style| style.opacity(0.8))
                                .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                                    cx.stop_propagation();
                                    cx.update_entity(&state_entity, |state, _| state.go_to(idx));
                                })
                                .child(
                                    img(image.src.clone())
                                        .size_full()
                                        .object_fit(ObjectFit::Cover),
                                )
                        })),
                )
            })
    }
}

/// 注册图片查看器的全局快捷键绑定。
pub fn init_image_viewer(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", ImageViewerClose, Some(CONTEXT)),
        KeyBinding::new("left", ImageViewerPrev, Some(CONTEXT)),
        KeyBinding::new("right", ImageViewerNext, Some(CONTEXT)),
        KeyBinding::new("up", ImageViewerZoomIn, Some(CONTEXT)),
        KeyBinding::new("down", ImageViewerZoomOut, Some(CONTEXT)),
        KeyBinding::new("0", ImageViewerResetZoom, Some(CONTEXT)),
        KeyBinding::new("+", ImageViewerZoomIn, Some(CONTEXT)),
        KeyBinding::new("-", ImageViewerZoomOut, Some(CONTEXT)),
        KeyBinding::new("=", ImageViewerZoomIn, Some(CONTEXT)),
    ]);
}
