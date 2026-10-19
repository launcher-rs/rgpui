//! 文档页视图 —— 分页画布几何 + 纸页外观（页边距 / 页眉页脚）。
//!
//! 面向文字处理/排版类应用的「分页视口」：给定纸张尺寸、页边距、页眉页脚高度与页间距，
//! 按页数在**文档坐标**里纵向排布纸页，并导出每页的整页 / 页眉 / 页脚 / 正文内容矩形；
//! 屏幕映射交给 [`Viewport`]（缩放 + 平移）。框架只出几何与纸页外观，
//! **分页/断行算法留在应用层**（见参考分析的「不该进框架」一节）。
//!
//! ## 用法
//!
//! ```ignore
//! use rgpui::page_view::{PageLayout, PageSetup, PageView};
//! use rgpui::viewport::Viewport;
//!
//! let setup = PageSetup::default();
//! let layout = PageLayout::new(setup, /* 页数 */ 3);
//! let viewport = Viewport::new();
//!
//! // 视图中：PageView 只画纸页与页眉页脚，正文由应用按 layout.content_rect(i) 叠加。
//! PageView::new(layout, viewport, ())
//! ```
//!
//! 正文叠加流程：用 [`PageLayout::content_rect`] 取文档坐标 → `Viewport::document_to_screen`
//! 换算到屏幕 → 放置自定义元素。

use crate::viewport::Viewport;
use crate::{
    ActiveTheme as _, App, Bounds, IntoElement, ParentElement as _, Pixels, Point, RenderOnce,
    SharedString, Size, Styled as _, Window, div, point, px, size,
};

/// 纸张尺寸（像素，按 96 dpi 换算）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageSize {
    /// 页宽。
    pub width: Pixels,
    /// 页高。
    pub height: Pixels,
}

impl PageSize {
    /// 构造纸张尺寸。
    pub const fn new(width: Pixels, height: Pixels) -> Self {
        Self { width, height }
    }

    /// A4 纵向（210 × 297 mm）。
    pub const A4_PORTRAIT: PageSize = PageSize::new(px(794.0), px(1123.0));
    /// A4 横向（297 × 210 mm）。
    pub const A4_LANDSCAPE: PageSize = PageSize::new(px(1123.0), px(794.0));
    /// US Letter 纵向（8.5 × 11 in）。
    pub const LETTER_PORTRAIT: PageSize = PageSize::new(px(816.0), px(1056.0));

    /// 交换宽高得到横向纸张。
    pub const fn landscape(self) -> Self {
        Self::new(self.height, self.width)
    }

    /// 交换宽高得到纵向纸张。
    pub const fn portrait(self) -> Self {
        Self::new(self.height, self.width)
    }

    /// 转为 [`Size`]。
    pub const fn to_size(self) -> Size<Pixels> {
        size(self.width, self.height)
    }
}

/// 页边距（像素）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageMargins {
    /// 上边距。
    pub top: Pixels,
    /// 右边距。
    pub right: Pixels,
    /// 下边距。
    pub bottom: Pixels,
    /// 左边距。
    pub left: Pixels,
}

impl PageMargins {
    /// 四边相同。
    pub const fn uniform(margin: Pixels) -> Self {
        Self {
            top: margin,
            right: margin,
            bottom: margin,
            left: margin,
        }
    }

    /// 四边分别指定。
    pub const fn new(top: Pixels, right: Pixels, bottom: Pixels, left: Pixels) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// 常规边距（1 英寸）。
    pub const NORMAL: PageMargins = PageMargins::uniform(px(96.0));

    /// 窄边距（0.5 英寸）。
    pub const NARROW: PageMargins = PageMargins::uniform(px(48.0));

    /// 宽边距（1.5 英寸）。
    pub const WIDE: PageMargins = PageMargins::uniform(px(144.0));
}

/// 分页设置：纸张 + 边距 + 页眉页脚高度 + 页间距。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageSetup {
    /// 纸张尺寸。
    pub size: PageSize,
    /// 页边距。
    pub margins: PageMargins,
    /// 页眉区高度（位于上边距内）。
    pub header_height: Pixels,
    /// 页脚区高度（位于下边距内）。
    pub footer_height: Pixels,
    /// 相邻两页之间的间距。
    pub gap: Pixels,
}

impl Default for PageSetup {
    fn default() -> Self {
        Self {
            size: PageSize::A4_PORTRAIT,
            margins: PageMargins::NORMAL,
            header_height: px(48.0),
            footer_height: px(48.0),
            gap: px(24.0),
        }
    }
}

/// 分页布局（文档坐标；单列纵向排布）。
#[derive(Clone, Debug, PartialEq)]
pub struct PageLayout {
    setup: PageSetup,
    page_count: usize,
}

impl PageLayout {
    /// 依设置与页数创建布局。
    pub fn new(setup: PageSetup, page_count: usize) -> Self {
        Self { setup, page_count }
    }

    /// 分页设置。
    pub fn setup(&self) -> PageSetup {
        self.setup
    }

    /// 页数。
    pub fn page_count(&self) -> usize {
        self.page_count
    }

    /// 页间距（页高 + 页间距）。
    pub fn page_stride(&self) -> Pixels {
        self.setup.size.height + self.setup.gap
    }

    /// 第 `page` 页的整页矩形（文档坐标）。
    pub fn page_rect(&self, page: usize) -> Bounds<Pixels> {
        Bounds::new(
            point(px(0.0), self.page_stride() * page as f32),
            self.setup.size.to_size(),
        )
    }

    /// 页眉区矩形（文档坐标）。
    pub fn header_rect(&self, page: usize) -> Bounds<Pixels> {
        let page_rect = self.page_rect(page);
        let margins = self.setup.margins;
        Bounds::new(
            point(
                page_rect.origin.x + margins.left,
                page_rect.origin.y + margins.top,
            ),
            size(
                (self.setup.size.width - margins.left - margins.right).max(px(0.0)),
                self.setup.header_height,
            ),
        )
    }

    /// 页脚区矩形（文档坐标）。
    pub fn footer_rect(&self, page: usize) -> Bounds<Pixels> {
        let page_rect = self.page_rect(page);
        let margins = self.setup.margins;
        Bounds::new(
            point(
                page_rect.origin.x + margins.left,
                page_rect.origin.y + self.setup.size.height
                    - margins.bottom
                    - self.setup.footer_height,
            ),
            size(
                (self.setup.size.width - margins.left - margins.right).max(px(0.0)),
                self.setup.footer_height,
            ),
        )
    }

    /// 正文内容区矩形（扣掉边距与页眉页脚，文档坐标）。
    pub fn content_rect(&self, page: usize) -> Bounds<Pixels> {
        let page_rect = self.page_rect(page);
        let margins = self.setup.margins;
        let top = margins.top + self.setup.header_height;
        let bottom = margins.bottom + self.setup.footer_height;
        Bounds::new(
            point(page_rect.origin.x + margins.left, page_rect.origin.y + top),
            size(
                (self.setup.size.width - margins.left - margins.right).max(px(0.0)),
                (self.setup.size.height - top - bottom).max(px(0.0)),
            ),
        )
    }

    /// 文档总尺寸（含页间距）。
    pub fn document_size(&self) -> Size<Pixels> {
        if self.page_count == 0 {
            return size(self.setup.size.width, px(0.0));
        }
        let height = self.page_stride() * self.page_count as f32 - self.setup.gap;
        size(self.setup.size.width, height.max(px(0.0)))
    }

    /// 命中页（文档坐标，页间距内的点归上一页；越界返回 `None`）。
    pub fn page_at(&self, document: Point<Pixels>) -> Option<usize> {
        if self.page_count == 0 || document.x < px(0.0) || document.x >= self.setup.size.width {
            return None;
        }
        let stride = self.page_stride();
        if stride <= px(0.0) {
            return None;
        }
        let page = (document.y / stride).floor();
        if page < 0.0 || page as usize >= self.page_count {
            return None;
        }
        Some(page as usize)
    }
}

/// 页眉 / 页脚文本提供者（空串表示不绘制）。
pub trait PageLabels: Sized + 'static {
    /// 第 `page` 页页眉文本。
    fn header(&self, _page: usize, _cx: &App) -> SharedString {
        SharedString::default()
    }

    /// 第 `page` 页页脚文本。
    fn footer(&self, _page: usize, _cx: &App) -> SharedString {
        SharedString::default()
    }
}

impl PageLabels for () {}

/// 分页画布元素：绘制叠加的纸页（白底 + 边框），含页眉页脚文本与正文区描边。
///
/// 只负责「纸」的外观；正文由应用用 [`PageLayout`] 导出的矩形自行叠加。
#[derive(IntoElement)]
pub struct PageView<L: PageLabels> {
    /// 分页布局。
    layout: PageLayout,
    /// 屏幕 ↔ 文档坐标映射。
    viewport: Viewport,
    /// 页眉页脚文本。
    labels: L,
    /// 是否描边正文内容区（辅助对齐）。
    content_guide: bool,
    /// 是否描边页眉页脚区。
    area_guide: bool,
}

impl<L: PageLabels> PageView<L> {
    /// 创建分页画布元素。
    pub fn new(layout: PageLayout, viewport: Viewport, labels: L) -> Self {
        Self {
            layout,
            viewport,
            labels,
            content_guide: true,
            area_guide: false,
        }
    }

    /// 是否描边正文内容区（默认 `true`）。
    pub fn content_guide(mut self, show: bool) -> Self {
        self.content_guide = show;
        self
    }

    /// 是否描边页眉页脚区（默认 `false`）。
    pub fn area_guide(mut self, show: bool) -> Self {
        self.area_guide = show;
        self
    }
}

impl<L: PageLabels> RenderOnce for PageView<L> {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let page_bg = theme.background;
        let border = theme.border;
        let guide = theme.border;
        let label_fg = theme.muted_foreground;

        let layout = self.layout;
        let viewport = self.viewport;
        let labels = self.labels;

        let mut pages = div().relative().size_full();

        for page in 0..layout.page_count() {
            let page_rect = layout.page_rect(page);
            let origin = viewport.document_to_screen(page_rect.origin);
            let page_size = viewport.document_size_to_screen(page_rect.size);

            // 屏幕坐标 → 页内坐标（子元素相对整页定位）。
            let inset = |rect: Bounds<Pixels>| {
                let local = viewport
                    .document_to_screen(rect.origin)
                    .relative_to(&origin);
                (local, viewport.document_size_to_screen(rect.size))
            };

            let mut page_div = div()
                .absolute()
                .left(origin.x)
                .top(origin.y)
                .w(page_size.width)
                .h(page_size.height)
                .bg(page_bg)
                .border_1()
                .border_color(border);

            if self.area_guide {
                let (head_local, head_size) = inset(layout.header_rect(page));
                page_div = page_div.child(
                    div()
                        .absolute()
                        .left(head_local.x)
                        .top(head_local.y)
                        .w(head_size.width)
                        .h(head_size.height)
                        .border_1()
                        .border_color(guide),
                );
                let (foot_local, foot_size) = inset(layout.footer_rect(page));
                page_div = page_div.child(
                    div()
                        .absolute()
                        .left(foot_local.x)
                        .top(foot_local.y)
                        .w(foot_size.width)
                        .h(foot_size.height)
                        .border_1()
                        .border_color(guide),
                );
            }

            let header = labels.header(page, cx);
            if !header.is_empty() {
                let (local, size) = inset(layout.header_rect(page));
                page_div = page_div.child(
                    div()
                        .absolute()
                        .left(local.x)
                        .top(local.y)
                        .w(size.width)
                        .h(size.height)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .text_color(label_fg)
                        .child(header),
                );
            }

            let footer = labels.footer(page, cx);
            if !footer.is_empty() {
                let (local, size) = inset(layout.footer_rect(page));
                page_div = page_div.child(
                    div()
                        .absolute()
                        .left(local.x)
                        .top(local.y)
                        .w(size.width)
                        .h(size.height)
                        .flex()
                        .items_end()
                        .justify_center()
                        .text_xs()
                        .text_color(label_fg)
                        .child(footer),
                );
            }

            if self.content_guide {
                let (local, size) = inset(layout.content_rect(page));
                page_div = page_div.child(
                    div()
                        .absolute()
                        .left(local.x)
                        .top(local.y)
                        .w(size.width)
                        .h(size.height)
                        .border_1()
                        .border_color(guide),
                );
            }

            pages = pages.child(page_div);
        }

        div().relative().size_full().overflow_hidden().child(pages)
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{PageLayout, PageMargins, PageSetup, PageSize};
    use crate::{point, px, size};

    /// 默认设置为 A4 纵向 + 常规边距。
    #[test]
    fn default_setup_is_a4_portrait() {
        let setup = PageSetup::default();
        assert_eq!(setup.size, PageSize::A4_PORTRAIT);
        assert_eq!(setup.margins, PageMargins::NORMAL);
        assert_eq!(setup.header_height, px(48.0));
        assert_eq!(setup.footer_height, px(48.0));
        assert_eq!(setup.gap, px(24.0));
    }

    /// 横向/纵向互换宽高。
    #[test]
    fn orientation_swaps_dimensions() {
        assert_eq!(
            PageSize::A4_PORTRAIT.landscape(),
            PageSize::new(px(1123.0), px(794.0))
        );
        assert_eq!(
            PageSize::A4_LANDSCAPE.portrait(),
            PageSize::new(px(794.0), px(1123.0))
        );
    }

    /// 整页矩形按页间距单列纵向排布。
    #[test]
    fn page_rect_uses_stride() {
        let layout = PageLayout::new(PageSetup::default(), 3);
        assert_eq!(layout.page_stride(), px(1147.0));
        assert_eq!(layout.page_rect(0).origin, point(px(0.0), px(0.0)));
        assert_eq!(layout.page_rect(1).origin, point(px(0.0), px(1147.0)));
        assert_eq!(layout.page_rect(1).size, size(px(794.0), px(1123.0)));
    }

    /// 页眉 / 页脚 / 正文区矩形的位置与尺寸。
    #[test]
    fn content_areas_are_inset_by_margins() {
        let layout = PageLayout::new(PageSetup::default(), 1);
        assert_eq!(
            layout.header_rect(0),
            crate::Bounds::new(point(px(96.0), px(96.0)), size(px(602.0), px(48.0)))
        );
        assert_eq!(
            layout.footer_rect(0),
            crate::Bounds::new(point(px(96.0), px(979.0)), size(px(602.0), px(48.0)))
        );
        assert_eq!(
            layout.content_rect(0),
            crate::Bounds::new(point(px(96.0), px(144.0)), size(px(602.0), px(835.0)))
        );
    }

    /// 文档总尺寸含页间距但不含末页之后的间距。
    #[test]
    fn document_size_accounts_for_gaps() {
        let layout = PageLayout::new(PageSetup::default(), 3);
        assert_eq!(layout.document_size(), size(px(794.0), px(3417.0)));
        let empty = PageLayout::new(PageSetup::default(), 0);
        assert_eq!(empty.document_size(), size(px(794.0), px(0.0)));
    }

    /// 命中判定：页内命中、页间距归上一页、横向越界与纵向越界均不命中。
    #[test]
    fn page_at_hits_pages() {
        let layout = PageLayout::new(PageSetup::default(), 3);
        assert_eq!(layout.page_at(point(px(400.0), px(50.0))), Some(0));
        assert_eq!(layout.page_at(point(px(400.0), px(1147.0))), Some(1));
        // 页间距（1123..1147）归上一页。
        assert_eq!(layout.page_at(point(px(400.0), px(1130.0))), Some(0));
        // 横向越界。
        assert_eq!(layout.page_at(point(px(900.0), px(50.0))), None);
        // 纵向越界。
        assert_eq!(layout.page_at(point(px(400.0), px(9000.0))), None);
        // 空文档。
        let empty = PageLayout::new(PageSetup::default(), 0);
        assert_eq!(empty.page_at(point(px(10.0), px(10.0))), None);
    }
}
