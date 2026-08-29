use std::rc::Rc;

use crate::{
    ActiveTheme as _, AlignItems, AnyElement, AnyView, App, Axis, Div, ElementId, ElementSize,
    InteractiveElement as _, IntoElement, ParentElement, Pixels, Rems, RenderOnce, SharedString,
    StyleRefinement, Styled, StyledExt as _, Window, div, prelude::FluentBuilder as _, px,
};

use crate::{h_flex, v_flex};

/// 表单字段的共享属性，由 [`Form`] 同步到各个字段。
#[derive(Clone, Copy)]
pub(super) struct FieldProps {
    pub(super) size: ElementSize,
    pub(super) layout: Axis,
    pub(super) columns: usize,

    pub(super) label_width: Option<Pixels>,
    pub(super) label_text_size: Option<Rems>,
}

impl Default for FieldProps {
    fn default() -> Self {
        Self {
            layout: Axis::Vertical,
            size: ElementSize::default(),
            columns: 1,
            label_width: Some(px(140.)),
            label_text_size: None,
        }
    }
}

/// 字段的构建器，可以是字符串、渲染函数或视图。
pub enum FieldBuilder {
    /// 字符串
    String(SharedString),
    /// 渲染函数
    Element(Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>),
    /// 视图
    View(AnyView),
}

impl Default for FieldBuilder {
    fn default() -> Self {
        Self::String(SharedString::default())
    }
}

impl From<AnyView> for FieldBuilder {
    fn from(view: AnyView) -> Self {
        Self::View(view)
    }
}

impl RenderOnce for FieldBuilder {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        match self {
            FieldBuilder::String(value) => value.into_any_element(),
            FieldBuilder::Element(builder) => builder(window, cx),
            FieldBuilder::View(view) => view.into_any_element(),
        }
    }
}

impl From<&'static str> for FieldBuilder {
    fn from(value: &'static str) -> Self {
        Self::String(value.into())
    }
}

impl From<String> for FieldBuilder {
    fn from(value: String) -> Self {
        Self::String(value.into())
    }
}

impl From<SharedString> for FieldBuilder {
    fn from(value: SharedString) -> Self {
        Self::String(value)
    }
}

/// 表单字段元素。
#[derive(IntoElement)]
pub struct Field {
    id: ElementId,
    props: FieldProps,
    style: StyleRefinement,
    label: Option<FieldBuilder>,
    label_indent: bool,
    description: Option<FieldBuilder>,
    /// 用于渲染实际的表单字段，例如：Input、Switch 等。
    children: Vec<AnyElement>,
    visible: bool,
    required: bool,
    /// 表单字段的对齐方式。
    align_items: Option<AlignItems>,
    col_span: u16,
    col_start: Option<i16>,
    col_end: Option<i16>,
}

impl Field {
    /// 创建一个新的表单字段。
    pub fn new() -> Self {
        Self {
            id: 0.into(),
            props: FieldProps::default(),
            style: StyleRefinement::default(),
            label: None,
            description: None,
            children: Vec::new(),
            visible: true,
            required: false,
            label_indent: true,
            align_items: None,
            col_span: 1,
            col_start: None,
            col_end: None,
        }
    }

    /// 设置表单字段的标签。
    pub fn label(mut self, label: impl Into<FieldBuilder>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 设置是否使用标签宽度缩进（水平布局时），默认是 `true`。
    ///
    /// 有时你希望将输入表单左对齐（水平布局中默认是在标签宽度之后对齐）。
    ///
    /// 仅在未设置 `label` 时生效。
    pub fn label_indent(mut self, indent: bool) -> Self {
        self.label_indent = indent;
        self
    }

    /// 使用函数设置表单字段的标签。
    pub fn label_fn<F, E>(mut self, label: F) -> Self
    where
        E: IntoElement,
        F: Fn(&mut Window, &mut App) -> E + 'static,
    {
        self.label = Some(FieldBuilder::Element(Rc::new(move |window, cx| {
            label(window, cx).into_any_element()
        })));
        self
    }

    /// 设置表单字段的描述。
    pub fn description(mut self, description: impl Into<FieldBuilder>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 使用函数设置表单字段的描述。
    pub fn description_fn<F, E>(mut self, description: F) -> Self
    where
        E: IntoElement,
        F: Fn(&mut Window, &mut App) -> E + 'static,
    {
        self.description = Some(FieldBuilder::Element(Rc::new(move |window, cx| {
            description(window, cx).into_any_element()
        })));
        self
    }

    /// 设置表单字段的可见性，默认是 `true`。
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// 设置表单字段的必填状态，默认是 `false`。
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// 设置表单字段的属性。
    ///
    /// 这是用于从 Form 同步属性的内部 API。
    pub(super) fn props(mut self, ix: usize, props: FieldProps) -> Self {
        self.id = ix.into();
        self.props = props;
        self
    }

    /// 将表单字段项对齐到起始位置，这是默认值。
    pub fn items_start(mut self) -> Self {
        self.align_items = Some(AlignItems::Start);
        self
    }

    /// 将表单字段项对齐到末尾。
    pub fn items_end(mut self) -> Self {
        self.align_items = Some(AlignItems::End);
        self
    }

    /// 将表单字段项对齐到中心。
    pub fn items_center(mut self) -> Self {
        self.align_items = Some(AlignItems::Center);
        self
    }

    /// 设置表单字段的列跨度。
    ///
    /// 默认是 1。
    pub fn col_span(mut self, col_span: u16) -> Self {
        self.col_span = col_span;
        self
    }

    /// 设置表单字段的列起始位置。
    pub fn col_start(mut self, col_start: i16) -> Self {
        self.col_start = Some(col_start);
        self
    }

    /// 设置表单字段的列结束位置。
    pub fn col_end(mut self, col_end: i16) -> Self {
        self.col_end = Some(col_end);
        self
    }
}

impl ParentElement for Field {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Field {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Field {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let layout = self.props.layout;

        let label_width = if layout == Axis::Vertical {
            None
        } else {
            self.props.label_width
        };
        let has_label = self.label_indent;

        #[inline]
        fn wrap_div(layout: Axis) -> Div {
            if layout == Axis::Vertical {
                v_flex()
            } else {
                h_flex()
            }
        }

        #[inline]
        fn wrap_label(label_width: Option<Pixels>) -> Div {
            div().when_some(label_width, |this, width| this.w(width).flex_shrink_0())
        }

        let gap = match self.props.size {
            ElementSize::Large => px(8.),
            ElementSize::XSmall | ElementSize::Small => px(4.),
            _ => px(4.),
        };
        let inner_gap = if layout == Axis::Horizontal {
            gap
        } else {
            gap / 2.
        };

        v_flex()
            .flex_1()
            .gap(gap / 2.)
            .col_span(self.col_span)
            .when_some(self.col_start, |this, start| this.col_start(start))
            .when_some(self.col_end, |this, end| this.col_end(end))
            .refine_style(&self.style)
            .child(
                // 此包装用于对齐标签 + 输入框
                wrap_div(layout)
                    .id(self.id)
                    .gap(inner_gap)
                    .when_some(self.align_items, |this, align| {
                        this.map(|this| match align {
                            AlignItems::Start => this.items_start(),
                            AlignItems::End => this.items_end(),
                            AlignItems::Center => this.items_center(),
                            AlignItems::Baseline => this.items_baseline(),
                            _ => this,
                        })
                    })
                    .when(has_label, |this| {
                        // 标签
                        this.child(
                            wrap_label(label_width)
                                .text_sm()
                                .when_some(self.props.label_text_size, |this, size| {
                                    this.text_size(size)
                                })
                                .font_medium()
                                .gap_1()
                                .items_center()
                                .when_some(self.label, |this, builder| {
                                    this.child(
                                        h_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .overflow_x_hidden()
                                                    .child(builder.render(window, cx)),
                                            )
                                            .when(self.required, |this| {
                                                this.child(
                                                    div().text_color(cx.theme().danger).child("*"),
                                                )
                                            }),
                                    )
                                }),
                        )
                    })
                    .child(
                        div()
                            .w_full()
                            .flex_1()
                            .overflow_x_hidden()
                            .children(self.children),
                    ),
            )
            .child(
                // 其他
                wrap_div(layout)
                    .gap(inner_gap)
                    .when(has_label && layout == Axis::Horizontal, |this| {
                        this.child(
                            // 留空用于与输入框对齐
                            wrap_label(label_width),
                        )
                    })
                    .when_some(self.description, |this, builder| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(builder.render(window, cx)),
                        )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use crate::ParentElement as _;
    use crate::{Form, TestAppContext, div};

    use super::Field;

    #[rgpui::test]
    fn form_builder_chaining(cx: &mut TestAppContext) {
        cx.update(|_cx| {
            let _form = Form::horizontal()
                .columns(2)
                .child(Field::new().label("Name").child(div().child("input")))
                .child(Field::new().label("Email").required(true).child(div()));
            let field = Field::new().label("Label").description("desc");
            let _ = field;
        });
    }
}
