use crate::{
    App, Axis, ElementSize, IntoElement, ParentElement, Pixels, Rems, RenderOnce, Sizable,
    StyleRefinement, Styled, Window, div, px,
};

use crate::{
    form::{Field, FieldProps},
    v_flex,
};

/// 包含多个表单字段的表单元素。
#[derive(IntoElement)]
pub struct Form {
    style: StyleRefinement,
    fields: Vec<Field>,
    props: FieldProps,
}

impl Form {
    fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            props: FieldProps::default(),
            fields: Vec::new(),
        }
    }

    /// 创建一个水平布局的新表单。
    pub fn horizontal() -> Self {
        Self::new().layout(Axis::Horizontal)
    }

    /// 创建一个垂直布局的新表单。
    pub fn vertical() -> Self {
        Self::new().layout(Axis::Vertical)
    }

    /// 设置表单的布局，默认是 `Axis::Vertical`。
    pub fn layout(mut self, layout: Axis) -> Self {
        self.props.layout = layout;
        self
    }

    /// 设置表单中标签的宽度。默认是 `px(100.)`。
    pub fn label_width(mut self, width: Pixels) -> Self {
        self.props.label_width = Some(width);
        self
    }

    /// 设置表单中标签的文本大小。默认是 `None`。
    pub fn label_text_size(mut self, size: Rems) -> Self {
        self.props.label_text_size = Some(size);
        self
    }

    /// 向表单添加一个字段。
    pub fn child(mut self, field: impl Into<Field>) -> Self {
        self.fields.push(field.into());
        self
    }

    /// 向表单添加多个字段。
    pub fn children(mut self, fields: impl IntoIterator<Item = Field>) -> Self {
        self.fields.extend(fields);
        self
    }

    /// 设置表单的列数。
    ///
    /// 默认是 1。
    pub fn columns(mut self, columns: usize) -> Self {
        self.props.columns = columns;
        self
    }
}

impl Styled for Form {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Sizable for Form {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.props.size = size.into();
        self
    }
}

impl RenderOnce for Form {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let props = self.props;

        let gap = match props.size {
            ElementSize::XSmall | ElementSize::Small => px(6.),
            ElementSize::Large => px(12.),
            _ => px(8.),
        };

        // 添加 `div` 包装器以避免宽度有时不满的问题
        div().child(
            v_flex()
                .w_full()
                .gap_x(gap * 3.)
                .gap_y(gap)
                .grid()
                .grid_cols(props.columns as u16)
                .children(
                    self.fields
                        .into_iter()
                        .enumerate()
                        .map(|(ix, field)| field.props(ix, props)),
                ),
        )
    }
}
