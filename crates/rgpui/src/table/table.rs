use crate::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement, Pixels, RenderOnce,
    StyleRefinement, Styled, TextAlign, Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::{
    ActiveTheme as _, AnyChildElement, ChildElement, ElementSize, Sizable, StyledExt as _,
};

const MIN_CELL_WIDTH: Pixels = px(100.);

/// 用于直接渲染表格数据的基础表格组件。
///
/// 与 [`DataTable`](crate::DataTable) 不同，这是一个简单的、无状态的、
/// 可组合的表格，不包含虚拟滚动或列管理。
///
/// 通过 [`Sizable`] 设置的尺寸会自动传递给所有子元素。
///
/// # 示例
///
/// ```rust,ignore
/// Table::new()
///     .small()
///     .child(TableHeader::new().child(
///         TableRow::new()
///             .child(TableHead::new().child("Name"))
///             .child(TableHead::new().child("Email"))
///     ))
///     .child(TableBody::new()
///         .child(TableRow::new()
///             .child(TableCell::new().child("John"))
///             .child(TableCell::new().child("john@example.com")))
///     )
///     .child(TableCaption::new().child("A list of recent invoices."))
/// ```
#[derive(IntoElement)]
pub struct Table {
    ix: usize,
    style: StyleRefinement,
    children: Vec<AnyChildElement>,
    size: ElementSize,
}

impl Table {
    /// 创建新的组件实例。
    pub fn new() -> Self {
        Self {
            ix: 0,
            style: StyleRefinement::default(),
            children: Vec::new(),
            size: ElementSize::default(),
        }
    }

    /// 添加一个子元素到组件中。
    pub fn child(mut self, child: impl ChildElement + 'static) -> Self {
        self.children.push(AnyChildElement::new(child));
        self
    }

    /// 批量添加多个子元素到组件中。
    pub fn children<E: ChildElement + 'static>(
        mut self,
        children: impl IntoIterator<Item = E>,
    ) -> Self {
        self.children
            .extend(children.into_iter().map(AnyChildElement::new));
        self
    }
}

impl Styled for Table {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Sizable for Table {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl ChildElement for Table {
    fn with_ix(mut self, ix: usize) -> Self {
        self.ix = ix;
        self
    }
}

impl RenderOnce for Table {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .id(("table", self.ix))
            .w_full()
            .text_sm()
            .overflow_hidden()
            .bg(cx.theme().tokens.table)
            .refine_style(&self.style)
            .children(
                self.children
                    .into_iter()
                    .enumerate()
                    .map(|(ix, c)| c.into_any(ix, self.size)),
            )
    }
}

/// [`Table`] 的头部区域，包裹头部行。
#[derive(IntoElement)]
pub struct TableHeader {
    ix: usize,
    style: StyleRefinement,
    children: Vec<AnyChildElement>,
    size: ElementSize,
}

impl TableHeader {
    /// 创建新的组件实例。
    pub fn new() -> Self {
        Self {
            ix: 0,
            style: StyleRefinement::default(),
            children: Vec::new(),
            size: ElementSize::default(),
        }
    }

    /// 添加一个子元素到组件中。
    pub fn child(mut self, child: impl ChildElement + 'static) -> Self {
        self.children.push(AnyChildElement::new(child));
        self
    }

    /// 批量添加多个子元素到组件中。
    pub fn children<E: ChildElement + 'static>(
        mut self,
        children: impl IntoIterator<Item = E>,
    ) -> Self {
        self.children
            .extend(children.into_iter().map(AnyChildElement::new));
        self
    }
}

impl Styled for TableHeader {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ChildElement for TableHeader {
    fn with_ix(mut self, ix: usize) -> Self {
        self.ix = ix;
        self
    }
}

impl Sizable for TableHeader {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for TableHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .id(("table-header", self.ix))
            .w_full()
            .bg(cx.theme().tokens.table_head)
            .text_color(cx.theme().table_head_foreground)
            .refine_style(&self.style)
            .border_b_1()
            .border_color(cx.theme().table_row_border)
            .children(
                self.children
                    .into_iter()
                    .enumerate()
                    .map(|(ix, c)| c.into_any(ix, self.size)),
            )
    }
}

/// [`Table`] 的主体区域，包裹数据行。
#[derive(IntoElement)]
pub struct TableBody {
    ix: usize,
    style: StyleRefinement,
    children: Vec<AnyChildElement>,
    size: ElementSize,
}

impl TableBody {
    /// 创建新的组件实例。
    pub fn new() -> Self {
        Self {
            ix: 0,
            style: StyleRefinement::default(),
            children: Vec::new(),
            size: ElementSize::default(),
        }
    }

    /// 添加一个子元素到组件中。
    pub fn child(mut self, child: impl ChildElement + 'static) -> Self {
        self.children.push(AnyChildElement::new(child));
        self
    }

    /// 批量添加多个子元素到组件中。
    pub fn children<E: ChildElement + 'static>(
        mut self,
        children: impl IntoIterator<Item = E>,
    ) -> Self {
        self.children
            .extend(children.into_iter().map(AnyChildElement::new));
        self
    }
}

impl Styled for TableBody {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Sizable for TableBody {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl ChildElement for TableBody {
    fn with_ix(mut self, ix: usize) -> Self {
        self.ix = ix;
        self
    }
}

impl RenderOnce for TableBody {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .id(("table-body", self.ix))
            .w_full()
            .refine_style(&self.style)
            .children(
                self.children
                    .into_iter()
                    .enumerate()
                    .map(|(ix, c)| c.into_any(ix, self.size)),
            )
    }
}

/// [`Table`] 的底部区域，包裹底部行。
#[derive(IntoElement)]
pub struct TableFooter {
    ix: usize,
    style: StyleRefinement,
    children: Vec<AnyChildElement>,
    size: ElementSize,
}

impl TableFooter {
    /// 创建新的组件实例。
    pub fn new() -> Self {
        Self {
            ix: 0,
            style: StyleRefinement::default(),
            children: Vec::new(),
            size: ElementSize::default(),
        }
    }

    /// 添加一个子元素到组件中。
    pub fn child(mut self, child: impl ChildElement + 'static) -> Self {
        self.children.push(AnyChildElement::new(child));
        self
    }

    /// 批量添加多个子元素到组件中。
    pub fn children<E: ChildElement + 'static>(
        mut self,
        children: impl IntoIterator<Item = E>,
    ) -> Self {
        self.children
            .extend(children.into_iter().map(AnyChildElement::new));
        self
    }
}

impl Styled for TableFooter {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Sizable for TableFooter {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl ChildElement for TableFooter {
    fn with_ix(mut self, ix: usize) -> Self {
        self.ix = ix;
        self
    }
}

impl RenderOnce for TableFooter {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .id(("table-footer", self.ix))
            .w_full()
            .bg(cx.theme().tokens.table_foot)
            .text_color(cx.theme().table_foot_foreground)
            .border_t_1()
            .border_color(cx.theme().table_row_border)
            .refine_style(&self.style)
            .children(
                self.children
                    .into_iter()
                    .enumerate()
                    .map(|(ix, c)| c.into_any(ix, self.size)),
            )
    }
}

/// [`Table`] 中的一行。
#[derive(IntoElement)]
pub struct TableRow {
    ix: usize,
    style: StyleRefinement,
    children: Vec<AnyChildElement>,
    size: ElementSize,
}

impl TableRow {
    /// 创建新的组件实例。
    pub fn new() -> Self {
        Self {
            ix: 0,
            style: StyleRefinement::default(),
            children: Vec::new(),
            size: ElementSize::default(),
        }
    }

    /// 添加一个子元素到组件中。
    pub fn child(mut self, child: impl ChildElement + 'static) -> Self {
        self.children.push(AnyChildElement::new(child));
        self
    }

    /// 批量添加多个子元素到组件中。
    pub fn children<E: ChildElement + 'static>(
        mut self,
        children: impl IntoIterator<Item = E>,
    ) -> Self {
        self.children
            .extend(children.into_iter().map(AnyChildElement::new));
        self
    }
}

impl Styled for TableRow {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Sizable for TableRow {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl ChildElement for TableRow {
    fn with_ix(mut self, ix: usize) -> Self {
        self.ix = ix;
        self
    }
}

impl RenderOnce for TableRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .id(("table-row", self.ix))
            .w_full()
            .flex()
            .flex_row()
            .refine_style(&self.style)
            .border_color(cx.theme().table_row_border)
            .when(self.ix > 0, |this| this.border_t_1())
            .children(
                self.children
                    .into_iter()
                    .enumerate()
                    .map(|(ix, c)| c.into_any(ix, self.size)),
            )
    }
}

/// [`TableRow`] 中的表头单元格。
#[derive(IntoElement)]
pub struct TableHead {
    ix: usize,
    style: StyleRefinement,
    children: Vec<AnyElement>,
    col_span: usize,
    align: TextAlign,
    size: ElementSize,
}

impl TableHead {
    /// 创建新的单元格组件实例。
    pub fn new() -> Self {
        Self {
            ix: 0,
            style: StyleRefinement::default(),
            children: Vec::new(),
            col_span: 1,
            align: TextAlign::Left,
            size: ElementSize::default(),
        }
    }

    /// 设置此表头单元格的列跨度。
    pub fn col_span(mut self, span: usize) -> Self {
        self.col_span = span.max(1);
        self
    }

    /// 设置文本居中对齐。
    pub fn text_center(mut self) -> Self {
        self.align = TextAlign::Center;
        self
    }

    /// 设置文本右对齐。
    pub fn text_right(mut self) -> Self {
        self.align = TextAlign::Right;
        self
    }
}

impl ParentElement for TableHead {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Sizable for TableHead {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl ChildElement for TableHead {
    fn with_ix(mut self, ix: usize) -> Self {
        self.ix = ix;
        self
    }
}

impl Styled for TableHead {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TableHead {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let paddings = self.size.table_cell_padding();

        div()
            .id(("table-head", self.ix))
            .flex()
            .items_center()
            .when(self.style.size.width.is_none(), |this| {
                this.flex_shrink_1()
                    .flex_basis(relative(self.col_span as f32))
            })
            .min_w(MIN_CELL_WIDTH * self.col_span)
            .px(paddings.left)
            .py(paddings.top)
            .when(self.align == TextAlign::Center, |this| {
                this.justify_center()
            })
            .when(self.align == TextAlign::Right, |this| this.justify_end())
            .refine_style(&self.style)
            .children(self.children)
    }
}

/// [`TableRow`] 中的数据单元格。
#[derive(IntoElement)]
pub struct TableCell {
    ix: usize,
    style: StyleRefinement,
    children: Vec<AnyElement>,
    col_span: usize,
    align: TextAlign,
    size: ElementSize,
}

impl TableCell {
    /// 创建新的单元格组件实例。
    pub fn new() -> Self {
        Self {
            ix: 0,
            style: StyleRefinement::default(),
            children: Vec::new(),
            col_span: 1,
            align: TextAlign::Left,
            size: ElementSize::default(),
        }
    }

    /// 设置此单元格的列跨度。
    pub fn col_span(mut self, span: usize) -> Self {
        self.col_span = span.max(1);
        self
    }

    /// 设置文本居中对齐。
    pub fn text_center(mut self) -> Self {
        self.align = TextAlign::Center;
        self
    }

    /// 设置文本右对齐。
    pub fn text_right(mut self) -> Self {
        self.align = TextAlign::Right;
        self
    }
}

impl ParentElement for TableCell {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Sizable for TableCell {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl ChildElement for TableCell {
    fn with_ix(mut self, ix: usize) -> Self {
        self.ix = ix;
        self
    }
}

impl Styled for TableCell {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TableCell {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let paddings = self.size.table_cell_padding();

        div()
            .id(("table-cell", self.ix))
            .flex()
            .items_center()
            .when(self.style.size.width.is_none(), |this| {
                this.flex_shrink_1()
                    .flex_basis(relative(self.col_span as f32))
            })
            .min_w(MIN_CELL_WIDTH * self.col_span)
            .px(paddings.left)
            .py(paddings.top)
            .when(self.align == TextAlign::Center, |this| {
                this.justify_center()
            })
            .when(self.align == TextAlign::Right, |this| this.justify_end())
            .refine_style(&self.style)
            .children(self.children)
    }
}

/// 显示在 [`Table`] 下方的说明。
#[derive(IntoElement)]
pub struct TableCaption {
    ix: usize,
    style: StyleRefinement,
    children: Vec<AnyElement>,
    size: ElementSize,
}

impl TableCaption {
    /// 创建新的组件实例。
    pub fn new() -> Self {
        Self {
            ix: 0,
            style: StyleRefinement::default(),
            children: Vec::new(),
            size: ElementSize::default(),
        }
    }
}

impl ParentElement for TableCaption {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Sizable for TableCaption {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl ChildElement for TableCaption {
    fn with_ix(mut self, ix: usize) -> Self {
        self.ix = ix;
        self
    }
}

impl Styled for TableCaption {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TableCaption {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let paddings = self.size.table_cell_padding();

        div()
            .id(("table-caption", self.ix))
            .w_full()
            .px(paddings.left)
            .py(paddings.top)
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .text_center()
            .refine_style(&self.style)
            .children(self.children)
    }
}
