// From:
// https://github.com/zed-industries/zed/blob/56daba28d40301ee4c05546fadb691d070b7b2b6/crates/rgpui/examples/window_shadow.rs
use crate::{
    ActiveTheme, AnyElement, App, BoxShadow, CursorStyle, Decorations, Edges, Hsla,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement, Pixels, Point, RenderOnce,
    ResizeEdge, Size, Styled as _, Tiling, Window, div, point, prelude::FluentBuilder as _, px,
    transparent_black,
};

/// 非 Linux 平台的阴影尺寸。
#[cfg(not(target_os = "linux"))]
const SHADOW_SIZE: Pixels = px(0.0);
/// Linux 平台的阴影尺寸。
#[cfg(target_os = "linux")]
const SHADOW_SIZE: Pixels = px(20.0);
/// 可见边框的宽度。
const BORDER_SIZE: Pixels = px(1.0);
/// 可见边框（内边框）每侧可调整大小的命中带半宽。
const RESIZE_HIT_SIZE: Pixels = px(4.0);
/// 窗口外框的圆角半径。
///
/// GPUI 目前会将溢出的子元素裁剪为矩形内容遮罩。非零半径会圆化外框本身，
/// 但会留下子元素背景在角落可见，因此在存在圆角内容遮罩之前保持通用窗口
/// 包装器为方形。
const BORDER_RADIUS: Pixels = px(0.0);

/// 创建一个新的窗口边框。
pub fn window_border() -> WindowBorder {
    WindowBorder::new()
}

/// 在 Linux 上渲染自定义窗口边框与阴影。
#[derive(IntoElement)]
pub struct WindowBorder {
    shadow_size: Pixels,
    resize_hit_size: Pixels,
    children: Vec<AnyElement>,
}

impl Default for WindowBorder {
    fn default() -> Self {
        Self {
            shadow_size: SHADOW_SIZE,
            resize_hit_size: RESIZE_HIT_SIZE,
            children: Vec::new(),
        }
    }
}

impl WindowBorder {
    /// 创建一个新的 `WindowBorder`。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置典型 Linux 客户端装饰的阴影尺寸。
    ///
    /// 默认值：[`SHADOW_SIZE`]
    pub fn shadow_size(mut self, size: impl Into<Pixels>) -> Self {
        self.shadow_size = size.into();
        self
    }

    /// 设置可见内边框边缘周围的可调整大小命中带半宽。
    ///
    /// 默认值：[`RESIZE_HIT_SIZE`]
    pub fn resize_hit_size(mut self, size: impl Into<Pixels>) -> Self {
        self.resize_hit_size = size.into();
        self
    }
}

/// 每侧可见边框距窗口外边界的内边距。
fn client_frame_insets(shadow_size: Pixels, tiling: &Tiling) -> Edges<Pixels> {
    let mut insets = Edges::all(shadow_size);
    if tiling.top {
        insets.top = px(0.0);
    }
    if tiling.bottom {
        insets.bottom = px(0.0);
    }
    if tiling.left {
        insets.left = px(0.0);
    }
    if tiling.right {
        insets.right = px(0.0);
    }
    insets
}

/// 获取窗口内边距。
pub fn window_paddings(window: &Window) -> Edges<Pixels> {
    let shadow_size = window.client_inset().unwrap_or(SHADOW_SIZE);
    match window.window_decorations() {
        Decorations::Server => Edges::all(px(0.0)),
        Decorations::Client { tiling } => client_frame_insets(shadow_size, &tiling),
    }
}

impl ParentElement for WindowBorder {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for WindowBorder {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let decorations = window.window_decorations();
        // 保持平台客户端内边距稳定。当窗口在所有侧边平铺时，我们停止绘制
        // 阴影内边距，但 `set_client_inset` 仍必须使用完整的阴影尺寸。清除
        // 它会使得恢复后的首次调整大小在 `compute_outer_size` 中双重计算阴影，
        // 导致窗口跳变变大。
        let platform_inset = self.shadow_size;
        let visual_shadow = match decorations {
            Decorations::Client { tiling }
                if tiling.top && tiling.bottom && tiling.left && tiling.right =>
            {
                px(0.0)
            }
            _ => self.shadow_size,
        };
        let resize_hit_size = self.resize_hit_size;
        if matches!(decorations, Decorations::Client { .. }) {
            window.set_client_inset(platform_inset);
        }
        let window_size = window.window_bounds().get_bounds().size;
        let is_window_active = window.is_window_active();
        let border_color = if cx.theme().is_dark() {
            Hsla {
                h: 0.,
                s: 0.,
                l: 0.2,
                a: 1.0,
            }
        } else {
            Hsla {
                h: 0.,
                s: 0.,
                l: 0.8,
                a: 1.0,
            }
        };

        div()
            .id("window-backdrop")
            .bg(transparent_black())
            .map(|div| match decorations {
                Decorations::Server => div,
                Decorations::Client { tiling, .. } => div
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .bg(transparent_black())
                    .when(!(tiling.top || tiling.right), |div| {
                        div.rounded_tr(BORDER_RADIUS)
                    })
                    .when(!(tiling.top || tiling.left), |div| {
                        div.rounded_tl(BORDER_RADIUS)
                    })
                    .when(!tiling.top, |div| div.pt(visual_shadow))
                    .when(!tiling.bottom, |div| div.pb(visual_shadow))
                    .when(!tiling.left, |div| div.pl(visual_shadow))
                    .when(!tiling.right, |div| div.pr(visual_shadow))
                    .on_mouse_down(MouseButton::Left, move |_, window, _| {
                        let Decorations::Client { tiling } = window.window_decorations() else {
                            return;
                        };
                        if tiling.top && tiling.bottom && tiling.left && tiling.right {
                            return;
                        }
                        let size = window.window_bounds().get_bounds().size;
                        let pos = window.mouse_position();
                        let insets = client_frame_insets(platform_inset, &tiling);

                        match resize_edge(pos, size, insets, &tiling, resize_hit_size) {
                            Some(edge) => window.start_window_resize(edge),
                            None => {}
                        };
                    }),
            })
            .size_full()
            .child(
                div()
                    .cursor(CursorStyle::default())
                    .map(|div| match decorations {
                        Decorations::Server => div.size_full(),
                        Decorations::Client { tiling } => div
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .overflow_hidden()
                            .when(!(tiling.top || tiling.right), |div| {
                                div.rounded_tr(BORDER_RADIUS)
                            })
                            .when(!(tiling.top || tiling.left), |div| {
                                div.rounded_tl(BORDER_RADIUS)
                            })
                            .border_color(border_color)
                            .when(!tiling.top, |div| div.border_t(BORDER_SIZE))
                            .when(!tiling.bottom, |div| div.border_b(BORDER_SIZE))
                            .when(!tiling.left, |div| div.border_l(BORDER_SIZE))
                            .when(!tiling.right, |div| div.border_r(BORDER_SIZE))
                            .when(!tiling.is_tiled(), |div| {
                                let opacity = if is_window_active { 1.0 } else { 0.7 };
                                div.shadow(vec![
                                    // 保持有效外延低于 SHADOW_SIZE。GPUI 不会为模糊
                                    // 扩展绘制边界，因此更大的模糊或偏移会被窗口表面
                                    // 明显裁剪。
                                    BoxShadow {
                                        color: Hsla {
                                            h: 0.,
                                            s: 0.,
                                            l: 0.,
                                            a: 0.18 * opacity,
                                        },
                                        // GNOME 风格的环境阴影：水平居中，
                                        // 仅轻微向下偏移。
                                        blur_radius: px(10.),
                                        spread_radius: px(-1.),
                                        offset: point(px(0.0), px(2.0)),
                                        inset: false,
                                    },
                                    // 接触层在不增加内容与窗口外边界之间
                                    // 空间的情况下增加定义感。
                                    BoxShadow {
                                        color: Hsla {
                                            h: 0.,
                                            s: 0.,
                                            l: 0.,
                                            a: 0.18 * opacity,
                                        },
                                        blur_radius: px(3.),
                                        spread_radius: px(0.),
                                        offset: point(px(0.0), px(1.0)),
                                        inset: false,
                                    },
                                ])
                            }),
                    })
                    .on_mouse_move(|_e, _, cx| {
                        cx.stop_propagation();
                    })
                    .bg(transparent_black())
                    .children(self.children),
            )
            .when(matches!(decorations, Decorations::Client { .. }), |this| {
                let Decorations::Client { tiling, .. } = decorations else {
                    return this;
                };
                this.child(div().absolute().size_full().children(resize_hit_zones(
                    window_size,
                    platform_inset,
                    resize_hit_size,
                    &tiling,
                )))
            })
    }
}

/// 返回给定调整大小边缘对应的光标样式。
fn cursor_style_for_resize_edge(edge: ResizeEdge) -> CursorStyle {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
    }
}

/// 为每个调整大小边缘/角落创建仅光标覆盖层。调整大小从背景层
/// `on_mouse_down` 通过 [`resize_edge`] 开始。`.cursor()` 在命中区域
/// 变化时立即更新，无需 `window.refresh()`（PR #617）。
fn resize_hit_zones(
    window_size: Size<Pixels>,
    shadow_size: Pixels,
    hit_size: Pixels,
    tiling: &Tiling,
) -> Vec<AnyElement> {
    if tiling.top && tiling.bottom && tiling.left && tiling.right {
        return Vec::new();
    }

    let insets = client_frame_insets(shadow_size, tiling);
    let inner_left = insets.left;
    let inner_right = window_size.width - insets.right;
    let inner_top = insets.top;
    let inner_bottom = window_size.height - insets.bottom;
    // 覆盖层布局在带内边距的内容盒中；从窗口坐标转换。
    let frame_origin = point(insets.left, insets.top);
    let band = hit_size + hit_size;
    let span_x = inner_right - inner_left + band;
    let span_y = inner_bottom - inner_top + band;

    let mut zones: Vec<AnyElement> = Vec::new();

    let mut push_zone = |edge: ResizeEdge, origin: Point<Pixels>, zone_size: Size<Pixels>| {
        let origin = origin - frame_origin;
        zones.push(
            div()
                .absolute()
                .left(origin.x)
                .top(origin.y)
                .w(zone_size.width)
                .h(zone_size.height)
                .cursor(cursor_style_for_resize_edge(edge))
                .into_any_element(),
        );
    };

    if !tiling.top {
        push_zone(
            ResizeEdge::Top,
            point(inner_left - hit_size, inner_top - hit_size),
            Size::new(span_x, band),
        );
    }
    if !tiling.bottom {
        push_zone(
            ResizeEdge::Bottom,
            point(inner_left - hit_size, inner_bottom - hit_size),
            Size::new(span_x, band),
        );
    }
    if !tiling.left {
        push_zone(
            ResizeEdge::Left,
            point(inner_left - hit_size, inner_top - hit_size),
            Size::new(band, span_y),
        );
    }
    if !tiling.right {
        push_zone(
            ResizeEdge::Right,
            point(inner_right - hit_size, inner_top - hit_size),
            Size::new(band, span_y),
        );
    }

    // 角落带在边缘带之后压入，使命中测试优先选择它们而不是相邻边缘。
    if !tiling.top && !tiling.left {
        push_zone(
            ResizeEdge::TopLeft,
            point(inner_left - hit_size, inner_top - hit_size),
            Size::new(band, band),
        );
    }
    if !tiling.top && !tiling.right {
        push_zone(
            ResizeEdge::TopRight,
            point(inner_right - hit_size, inner_top - hit_size),
            Size::new(band, band),
        );
    }
    if !tiling.bottom && !tiling.left {
        push_zone(
            ResizeEdge::BottomLeft,
            point(inner_left - hit_size, inner_bottom - hit_size),
            Size::new(band, band),
        );
    }
    if !tiling.bottom && !tiling.right {
        push_zone(
            ResizeEdge::BottomRight,
            point(inner_right - hit_size, inner_bottom - hit_size),
            Size::new(band, band),
        );
    }

    zones
}

/// 在可见内边框周围狭窄带上命中测试调整大小边缘，而不是整个阴影内边距。
fn resize_edge(
    pos: Point<Pixels>,
    size: Size<Pixels>,
    insets: Edges<Pixels>,
    tiling: &Tiling,
    hit_size: Pixels,
) -> Option<ResizeEdge> {
    let inner_left = insets.left;
    let inner_right = size.width - insets.right;
    let inner_top = insets.top;
    let inner_bottom = size.height - insets.bottom;

    // 每条边缘仅沿其对应的内框线段生效；它不沿阴影内边距的"延伸线"扩展。
    let on_left = pos.x >= inner_left - hit_size
        && pos.x <= inner_left + hit_size
        && pos.y >= inner_top - hit_size
        && pos.y <= inner_bottom + hit_size;
    let on_right = pos.x >= inner_right - hit_size
        && pos.x <= inner_right + hit_size
        && pos.y >= inner_top - hit_size
        && pos.y <= inner_bottom + hit_size;
    let on_top = pos.y >= inner_top - hit_size
        && pos.y <= inner_top + hit_size
        && pos.x >= inner_left - hit_size
        && pos.x <= inner_right + hit_size;
    let on_bottom = pos.y >= inner_bottom - hit_size
        && pos.y <= inner_bottom + hit_size
        && pos.x >= inner_left - hit_size
        && pos.x <= inner_right + hit_size;

    if !tiling.top && !tiling.left && on_top && on_left {
        return Some(ResizeEdge::TopLeft);
    }
    if !tiling.top && !tiling.right && on_top && on_right {
        return Some(ResizeEdge::TopRight);
    }
    if !tiling.bottom && !tiling.left && on_bottom && on_left {
        return Some(ResizeEdge::BottomLeft);
    }
    if !tiling.bottom && !tiling.right && on_bottom && on_right {
        return Some(ResizeEdge::BottomRight);
    }
    if !tiling.top && on_top {
        return Some(ResizeEdge::Top);
    }
    if !tiling.bottom && on_bottom {
        return Some(ResizeEdge::Bottom);
    }
    if !tiling.left && on_left {
        return Some(ResizeEdge::Left);
    }
    if !tiling.right && on_right {
        return Some(ResizeEdge::Right);
    }
    None
}
