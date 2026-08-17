use std::f32;

use crate::{
    Bounds, Context, Edges, Empty, EntityId, IntoElement, ParentElement as _, Pixels, Render,
    SharedString, Styled as _, TextAlign, Window, div, prelude::FluentBuilder, px,
};

use crate::ActiveTheme as _;

/// 表示表中的一列，用于初始化表格列。
#[derive(Debug, Clone)]
pub struct Column {
    /// 列的唯一键。
    ///
    /// 用于在表中识别列以及你的数据源。
    ///
    /// 在大多数情况下，它应与数据源中的字段名保持一致。
    pub key: SharedString,
    /// 列的显示名称。
    pub name: SharedString,
    /// 列文本的对齐方式。
    pub align: TextAlign,
    /// 列的排序行为，如果有的话。
    ///
    /// 如果为 `None`，则该列不可排序。
    pub sort: Option<ColumnSort>,
    /// 列的内边距。
    pub paddings: Option<Edges<Pixels>>,
    /// 列的宽度。
    pub width: Pixels,
    /// 列是否固定，固定列将在水平滚动时固定在左侧。
    pub fixed: Option<ColumnFixed>,
    /// 列是否可调整大小。
    pub resizable: bool,
    /// 列是否可移动。
    pub movable: bool,
    /// 列是否可选中。
    ///
    /// 为 `true` 时：
    /// - 在列选择模式下：整列可被选中
    /// - 在单元格选择模式下：该列中的单元格可被选中
    ///
    /// 为 `false` 时：
    /// - 该列及其单元格不能被选中
    /// - 适用于操作列（如按钮、复选框）等不应参与选择的列
    pub selectable: bool,
    /// 列的最小宽度。
    pub min_width: Pixels,
    /// 列的最大宽度。
    pub max_width: Pixels,
}

/// 列组，可将多列归入单个表头之下。
#[derive(Debug, Clone)]
pub struct ColumnGroup {
    /// 列组在表头中显示的标签文本。
    pub label: SharedString,
    /// 列组跨越的列数量。
    pub span: usize,
}

impl ColumnGroup {
    /// 创建新的列组。
    ///
    /// `label` 为表头显示的标签，`span` 为该组包含的列数。
    pub fn new(label: impl Into<SharedString>, span: usize) -> Self {
        Self {
            label: label.into(),
            span,
        }
    }
}

impl Default for Column {
    fn default() -> Self {
        Self {
            key: SharedString::new(""),
            name: SharedString::new(""),
            align: TextAlign::Left,
            sort: None,
            paddings: None,
            width: px(100.),
            fixed: None,
            resizable: true,
            movable: true,
            selectable: true,
            min_width: px(20.0),
            max_width: px(f32::MAX),
        }
    }
}

impl Column {
    /// 使用给定的键和名称创建新列。
    pub fn new(key: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            name: name.into(),
            ..Default::default()
        }
    }

    /// 设置列的自定义排序方式，默认是 None（不可排序）。
    ///
    /// 参见 [`Column::sortable`] 使用默认排序。
    pub fn sort(mut self, sort: ColumnSort) -> Self {
        self.sort = Some(sort);
        self
    }

    /// 设置列是否可排序，默认是 true。
    ///
    /// 参见 [`Column::sort`]。
    pub fn sortable(mut self) -> Self {
        self.sort = Some(ColumnSort::Default);
        self
    }

    /// 设置列以升序排序。
    pub fn ascending(mut self) -> Self {
        self.sort = Some(ColumnSort::Ascending);
        self
    }

    /// 设置列以降序排序。
    pub fn descending(mut self) -> Self {
        self.sort = Some(ColumnSort::Descending);
        self
    }

    /// 设置列文本居中对齐。
    pub fn text_center(mut self) -> Self {
        self.align = TextAlign::Center;
        self
    }

    /// 设置列文本对齐方式，默认为左对齐。
    ///
    /// 仅支持 `text_left`、`text_right`。
    pub fn text_right(mut self) -> Self {
        self.align = TextAlign::Right;
        self
    }

    /// 设置列的内边距，默认为 None。
    pub fn paddings(mut self, paddings: impl Into<Edges<Pixels>>) -> Self {
        self.paddings = Some(paddings.into());
        self
    }

    /// 设置列的内边距为 0px。
    pub fn p_0(mut self) -> Self {
        self.paddings = Some(Edges::all(px(0.)));
        self
    }

    /// 设置列的宽度，默认为 100px。
    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = width.into();
        self
    }

    /// 设置列是否固定，默认为 false。
    pub fn fixed(mut self, fixed: impl Into<ColumnFixed>) -> Self {
        self.fixed = Some(fixed.into());
        self
    }

    /// 设置列固定在左侧，默认为 false。
    pub fn fixed_left(mut self) -> Self {
        self.fixed = Some(ColumnFixed::Left);
        self
    }

    /// 设置列是否可调整大小，默认为 true。
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// 设置列是否可移动，默认为 true。
    pub fn movable(mut self, movable: bool) -> Self {
        self.movable = movable;
        self
    }

    /// 设置列是否可选中，默认为 true。
    ///
    /// 为 `false` 时，该列及其单元格不参与选择：
    /// - 在列选择模式下：列头不可被点击选中
    /// - 在单元格选择模式下：该列中的单元格不可被选中
    ///
    /// 这适用于操作列（如带按钮或复选框的列），不应成为选择系统的一部分。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// Column::new("actions", "Actions")
    ///     .width(px(100.))
    ///     .selectable(false)  // 阻止选中操作按钮
    /// ```
    pub fn selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self
    }

    /// 设置列的最小宽度，默认为 20px
    pub fn min_width(mut self, min_width: impl Into<Pixels>) -> Self {
        let min_width = min_width.into();
        self.min_width = min_width;

        // 如果当前宽度小于新的最小值，则将宽度提升到最小值。
        if self.width < min_width {
            self.width = min_width;
        }
        self
    }

    /// 设置列的最小宽度，默认为 1200px
    pub fn max_width(mut self, max_width: impl Into<Pixels>) -> Self {
        let max_width = max_width.into();
        self.max_width = max_width;

        // 如果当前宽度大于新的最大值，则将宽度拉低到最大值。
        if self.width > max_width {
            self.width = max_width;
        }
        self
    }
}

impl FluentBuilder for Column {}

/// 列的固定方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnFixed {
    /// 固定在左侧，横向滚动时保持可见。
    Left,
}

/// 用于在 Table 内部对列进行排序的运行时信息。
#[derive(Debug, Clone)]
pub(crate) struct ColGroup {
    pub(crate) column: Column,
    /// 列的运行时宽度，列被调整大小时可能会更新。
    ///
    /// 包含通过 col_span 与后续列合并的宽度。
    pub(crate) width: Pixels,
    /// 列在渲染后在表中的边界。
    pub(crate) bounds: Bounds<Pixels>,
}

impl ColGroup {
    pub(crate) fn is_resizable(&self) -> bool {
        self.column.resizable
    }
}

#[derive(Clone)]
pub(crate) struct DragColumn {
    pub(crate) entity_id: EntityId,
    pub(crate) name: SharedString,
    pub(crate) width: Pixels,
    pub(crate) col_ix: usize,
}

/// 列的排序行为。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ColumnSort {
    /// 不排序。
    #[default]
    Default,
    /// 升序排序。
    Ascending,
    /// 降序排序。
    Descending,
}

impl Render for DragColumn {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_4()
            .py_1()
            .bg(cx.theme().tokens.table_head)
            .text_color(cx.theme().muted_foreground)
            .opacity(0.9)
            .border_1()
            .border_color(cx.theme().border)
            .shadow_md()
            .w(self.width)
            .min_w(px(100.))
            .max_w(px(450.))
            .child(self.name.clone())
    }
}

#[derive(Clone)]
pub(crate) struct ResizeColumn(pub(crate) (EntityId, usize));
impl Render for ResizeColumn {
    fn render(&mut self, _window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}
