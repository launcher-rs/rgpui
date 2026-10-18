//! 视口 —— 可缩放/平移的画布坐标系统。
//!
//! 提供**屏幕 ↔ 文档坐标映射**与缩放/平移交互辅助，供文档页视图、电子表格网格、
//! 画布编辑器等共用：内容由调用方负责绘制（`canvas` 自绘，或按映射换算后布局控件），
//! 本模块只维护「缩放因子 + 文档原点偏移」并给出正确换算，避免每个应用各写一份、
//! 各错一处。
//!
//! ## 坐标约定
//!
//! - **文档坐标**：内容自身坐标系（如 A4 页面以左上角为原点，单位与 `px` 一致）。
//! - **屏幕坐标**：元素内的局部坐标（如鼠标事件位置）。
//! - 映射公式：`screen = document * zoom + offset`，`offset` 是文档原点在屏幕中的位置。
//!
//! ## 用法
//!
//! ```ignore
//! use rgpui::viewport::Viewport;
//!
//! let viewport = Viewport::new();
//! let doc = viewport.screen_to_document(mouse_position);
//! let screen = viewport.document_to_screen(doc);
//!
//! // 滚轮：Ctrl/Command 以光标为中心缩放，否则平移。
//! viewport.apply_wheel(&event, px(50.0));
//! ```

use crate::{Pixels, Point, ScrollDelta, ScrollWheelEvent, Size, point, px};

/// 默认缩放下限。
pub const DEFAULT_MIN_ZOOM: f32 = 0.1;
/// 默认缩放上限。
pub const DEFAULT_MAX_ZOOM: f32 = 32.0;
/// 每次 `zoom_in` / `zoom_out` 的缩放步进倍率。
pub const ZOOM_STEP: f32 = 1.25;
/// 滚轮缩放灵敏度（越大越迟钝）。
const WHEEL_ZOOM_SENSITIVITY: f32 = 160.0;

/// 视口状态：缩放因子与文档原点偏移（纯数据，可自由复制/比较）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    zoom: f32,
    offset: Point<Pixels>,
    min_zoom: f32,
    max_zoom: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new()
    }
}

impl Viewport {
    /// 创建 1:1、原点对齐的视口。
    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            offset: point(px(0.0), px(0.0)),
            min_zoom: DEFAULT_MIN_ZOOM,
            max_zoom: DEFAULT_MAX_ZOOM,
        }
    }

    /// 当前缩放因子。
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// 文档原点在屏幕中的偏移。
    pub fn offset(&self) -> Point<Pixels> {
        self.offset
    }

    /// 设置缩放因子（自动夹取到上下限）。
    pub fn with_zoom(mut self, zoom: f32) -> Self {
        self.zoom = self.clamp_zoom(zoom);
        self
    }

    /// 设置文档原点偏移。
    pub fn with_offset(mut self, offset: Point<Pixels>) -> Self {
        self.offset = offset;
        self
    }

    /// 设置缩放上下限（`min > max` 时以 `min` 为准，并夹取当前缩放）。
    pub fn with_zoom_limits(mut self, min: f32, max: f32) -> Self {
        self.min_zoom = min;
        self.max_zoom = max.max(self.min_zoom);
        self.zoom = self.clamp_zoom(self.zoom);
        self
    }

    /// 缩放上下限。
    pub fn zoom_limits(&self) -> (f32, f32) {
        (self.min_zoom, self.max_zoom)
    }

    /// 将缩放夹取到当前上下限。
    fn clamp_zoom(&self, zoom: f32) -> f32 {
        zoom.max(self.min_zoom).min(self.max_zoom)
    }

    /// 屏幕坐标 → 文档坐标。
    pub fn screen_to_document(&self, screen: Point<Pixels>) -> Point<Pixels> {
        point(
            (screen.x - self.offset.x) / self.zoom,
            (screen.y - self.offset.y) / self.zoom,
        )
    }

    /// 文档坐标 → 屏幕坐标。
    pub fn document_to_screen(&self, document: Point<Pixels>) -> Point<Pixels> {
        point(
            document.x * self.zoom + self.offset.x,
            document.y * self.zoom + self.offset.y,
        )
    }

    /// 文档矩形（以原点为起点的尺寸）→ 屏幕尺寸（忽略平移）。
    pub fn document_size_to_screen(&self, size: Size<Pixels>) -> Size<Pixels> {
        crate::size(size.width * self.zoom, size.height * self.zoom)
    }

    /// 以屏幕坐标 `anchor` 为不动点，将缩放设为 `zoom`。
    ///
    /// 即缩放前后 `anchor` 对应的文档坐标保持不变。
    pub fn set_zoom_at(&mut self, zoom: f32, anchor: Point<Pixels>) {
        let document = self.screen_to_document(anchor);
        self.zoom = self.clamp_zoom(zoom);
        self.offset = point(
            anchor.x - document.x * self.zoom,
            anchor.y - document.y * self.zoom,
        );
    }

    /// 以屏幕坐标 `anchor` 为不动点，将缩放乘以 `factor`。
    pub fn zoom_by_at(&mut self, factor: f32, anchor: Point<Pixels>) {
        self.set_zoom_at(self.zoom * factor, anchor);
    }

    /// 以视口中心为不动点放大一步。
    pub fn zoom_in_at(&mut self, anchor: Point<Pixels>) {
        self.zoom_by_at(ZOOM_STEP, anchor);
    }

    /// 以视口中心为不动点缩小一步。
    pub fn zoom_out_at(&mut self, anchor: Point<Pixels>) {
        self.zoom_by_at(1.0 / ZOOM_STEP, anchor);
    }

    /// 平移（屏幕像素增量）。
    pub fn pan_by(&mut self, delta: Point<Pixels>) {
        self.offset = point(self.offset.x + delta.x, self.offset.y + delta.y);
    }

    /// 将文档居中于视口（保持当前缩放）。
    pub fn center(&mut self, document: Size<Pixels>, view: Size<Pixels>) {
        let width = document.width.as_f32() * self.zoom;
        let height = document.height.as_f32() * self.zoom;
        self.offset = point(
            px((view.width.as_f32() - width) / 2.0),
            px((view.height.as_f32() - height) / 2.0),
        );
    }

    /// 使文档带 `padding` 留白地等比铺满视口，并居中（不放大超过上限）。
    pub fn fit(&mut self, document: Size<Pixels>, view: Size<Pixels>, padding: Pixels) {
        let doc_width = document.width.max(px(1.0)).as_f32();
        let doc_height = document.height.max(px(1.0)).as_f32();
        let pad = padding.as_f32();
        let avail_width = (view.width.as_f32() - pad * 2.0).max(1.0);
        let avail_height = (view.height.as_f32() - pad * 2.0).max(1.0);
        self.zoom = self.clamp_zoom((avail_width / doc_width).min(avail_height / doc_height));
        self.center(document, view);
    }

    /// 处理滚轮事件：`Ctrl`/`Command` 以光标为中心缩放，否则平移。
    ///
    /// `line_height` 用于把「按行」滚轮换算为像素。
    pub fn apply_wheel(&mut self, event: &ScrollWheelEvent, line_height: Pixels) {
        let (dx, dy) = match event.delta {
            ScrollDelta::Pixels(delta) => (delta.x.as_f32(), delta.y.as_f32()),
            ScrollDelta::Lines(delta) => (
                delta.x * line_height.as_f32(),
                delta.y * line_height.as_f32(),
            ),
        };

        if event.modifiers.secondary() {
            // 向上滚动（dy > 0）放大。
            let factor = (dy / WHEEL_ZOOM_SENSITIVITY).exp();
            self.zoom_by_at(factor, event.position);
        } else {
            self.pan_by(point(px(dx), px(dy)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Viewport;
    use crate::{point, px, size};

    /// 1:1 视口下坐标映射为单位映射。
    #[test]
    fn identity_maps_coordinates() {
        let viewport = Viewport::new();
        let screen = point(px(12.0), px(34.0));
        assert_eq!(viewport.screen_to_document(screen), screen);
        assert_eq!(viewport.document_to_screen(screen), screen);
    }

    /// 缩放后映射正确，且往返一致。
    #[test]
    fn zoom_scales_coordinates() {
        let viewport = Viewport::new().with_zoom(2.0);
        assert_eq!(
            viewport.document_to_screen(point(px(10.0), px(20.0))),
            point(px(20.0), px(40.0))
        );
        assert_eq!(
            viewport.screen_to_document(point(px(20.0), px(40.0))),
            point(px(10.0), px(20.0))
        );
    }

    /// 平移仅移动原点偏移。
    #[test]
    fn pan_translates_offset_only() {
        let mut viewport = Viewport::new();
        viewport.pan_by(point(px(5.0), px(-3.0)));
        assert_eq!(viewport.offset(), point(px(5.0), px(-3.0)));
        assert_eq!(
            viewport.document_to_screen(point(px(0.0), px(0.0))),
            point(px(5.0), px(-3.0))
        );
    }

    /// 以锚点缩放时，锚点对应的文档坐标保持不变。
    #[test]
    fn zoom_at_keeps_anchor_fixed() {
        let mut viewport = Viewport::new();
        let anchor = point(px(100.0), px(50.0));
        let before = viewport.screen_to_document(anchor);
        viewport.set_zoom_at(4.0, anchor);
        let after = viewport.screen_to_document(anchor);
        assert!((before.x.as_f32() - after.x.as_f32()).abs() < 0.01);
        assert!((before.y.as_f32() - after.y.as_f32()).abs() < 0.01);
        assert_eq!(viewport.zoom(), 4.0);
    }

    /// 缩放受上下限约束。
    #[test]
    fn zoom_is_clamped_to_limits() {
        let mut viewport = Viewport::new().with_zoom_limits(0.5, 3.0);
        viewport.set_zoom_at(10.0, point(px(0.0), px(0.0)));
        assert_eq!(viewport.zoom(), 3.0);
        viewport.set_zoom_at(0.01, point(px(0.0), px(0.0)));
        assert_eq!(viewport.zoom(), 0.5);
    }

    /// `fit` 等比铺满并居中。
    #[test]
    fn fit_scales_and_centers() {
        let mut viewport = Viewport::new();
        // 文档 100x100，视口 300x200，无留白：应缩放到 2x 并水平居中。
        viewport.fit(
            size(px(100.0), px(100.0)),
            size(px(300.0), px(200.0)),
            px(0.0),
        );
        assert!((viewport.zoom() - 2.0).abs() < 0.001);
        assert_eq!(viewport.offset(), point(px(50.0), px(0.0)));
    }

    /// `center` 在保持缩放的前提下居中。
    #[test]
    fn center_keeps_zoom() {
        let mut viewport = Viewport::new().with_zoom(2.0);
        viewport.center(size(px(100.0), px(50.0)), size(px(400.0), px(300.0)));
        assert_eq!(viewport.zoom(), 2.0);
        assert_eq!(viewport.offset(), point(px(100.0), px(100.0)));
    }
}
