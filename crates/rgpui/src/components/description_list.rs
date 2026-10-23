//! 描述列表组件：以「标签 / 值」成对展示只读信息（详情页、属性面板）。
//!
//! 主题令牌：`tokens.description_list_label`（标签底色）、
//! `description_list_label_foreground`（标签文字）与 `border`（分隔线）。

use crate::{prelude::FluentBuilder as _, *};

/// 内部不变量：值为 `None` 说明前提被破坏（行分组已保证非空等），必须立刻暴露。
///
/// 统一走这里而不是散落的 `unwrap`：语义与原先一致（同样 panic），但把「这是不变量
/// 断言」的意图写进调用点，模块级 `deny` 仍能拦住随手写的 unwrap。
#[inline]
#[allow(clippy::panic)] // 不变量断言：前提被破坏必须立刻暴露
fn invariant<T>(value: Option<T>, message: &str) -> T {
    match value {
        Some(value) => value,
        None => panic!("{message}"),
    }
}

/// 描述列表项。
pub enum DescriptionItem {
    /// 一个「标签 / 值」项。
    Item {
        /// 标签。
        label: DescriptionText,
        /// 值。
        value: DescriptionText,
        /// 占用的列数（水平布局下）。
        span: usize,
    },
    /// 分隔项。
    Separator,
}

/// 描述列表的标签或值文本。
#[derive(IntoElement)]
pub enum DescriptionText {
    /// 纯文本。
    String(SharedString),
    /// `Text` 元素。
    Text(Text),
    /// 任意元素。
    AnyElement(AnyElement),
}

impl From<&str> for DescriptionText {
    fn from(text: &str) -> Self {
        DescriptionText::String(SharedString::from(text))
    }
}

impl From<Text> for DescriptionText {
    fn from(text: Text) -> Self {
        DescriptionText::Text(text)
    }
}

impl From<AnyElement> for DescriptionText {
    fn from(element: AnyElement) -> Self {
        DescriptionText::AnyElement(element)
    }
}

impl From<SharedString> for DescriptionText {
    fn from(text: SharedString) -> Self {
        DescriptionText::String(text)
    }
}

impl From<String> for DescriptionText {
    fn from(text: String) -> Self {
        DescriptionText::String(SharedString::from(text))
    }
}

impl RenderOnce for DescriptionText {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        match self {
            DescriptionText::String(text) => div().child(text).into_any_element(),
            DescriptionText::Text(text) => text.into_any_element(),
            DescriptionText::AnyElement(element) => element,
        }
    }
}

impl DescriptionItem {
    /// 创建只带标签的项，值初始为空。
    pub fn new(label: impl Into<DescriptionText>) -> Self {
        DescriptionItem::Item {
            label: label.into(),
            value: "".into(),
            span: 1,
        }
    }

    /// 设置项的值。
    pub fn value(mut self, value: impl Into<DescriptionText>) -> Self {
        let new_value = value.into();
        if let DescriptionItem::Item { value, .. } = &mut self {
            *value = new_value;
        }
        self
    }

    /// 设置项的列跨度（仅对 [`DescriptionItem::Item`] 生效）。
    pub fn span(mut self, span: usize) -> Self {
        if let DescriptionItem::Item { span: slot, .. } = &mut self {
            *slot = span;
        }
        self
    }

    /// 取列跨度（分隔项返回 `None`）。
    fn column_span(&self) -> Option<usize> {
        match self {
            DescriptionItem::Item { span, .. } => Some(*span),
            DescriptionItem::Separator => None,
        }
    }
}

/// 描述列表。
#[derive(IntoElement)]
pub struct DescriptionList {
    /// 列表项。
    items: Vec<DescriptionItem>,
    /// 尺寸。
    size: ElementSize,
    /// 布局轴（水平为默认）。
    layout: Axis,
    /// 水平布局下的标签宽度，默认 `120px`。
    label_width: DefiniteLength,
    /// 是否显示边框与分隔线。
    bordered: bool,
    /// 列数（水平布局），范围 `1..=10`。
    columns: usize,
    /// 用户样式。
    style: StyleRefinement,
}

impl DescriptionList {
    /// 创建默认（水平、带边框、3 列）的描述列表。
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            layout: Axis::Horizontal,
            label_width: px(120.).into(),
            size: ElementSize::default(),
            bordered: true,
            columns: 3,
            style: StyleRefinement::default(),
        }
    }

    /// 创建垂直布局（标签在值上方）的描述列表。
    pub fn vertical() -> Self {
        Self::new().layout(Axis::Vertical)
    }

    /// 创建水平布局的描述列表（默认）。
    pub fn horizontal() -> Self {
        Self::new().layout(Axis::Horizontal)
    }

    /// 设置标签宽度（仅水平布局生效），默认 `120px`。
    pub fn label_width(mut self, label_width: impl Into<DefiniteLength>) -> Self {
        self.label_width = label_width.into();
        self
    }

    /// 设置布局轴。
    pub fn layout(mut self, layout: Axis) -> Self {
        self.layout = layout;
        self
    }

    /// 设置是否显示边框与分隔线，默认 `true`。
    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    /// 设置列数（水平布局），取值会被限制在 `1..=10`，默认 `3`。
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = columns.clamp(1, 10);
        self
    }

    /// 追加一个「标签 / 值」项。
    pub fn item(
        mut self,
        label: impl Into<DescriptionText>,
        value: impl Into<DescriptionText>,
        span: usize,
    ) -> Self {
        self.items.push(DescriptionItem::Item {
            label: label.into(),
            value: value.into(),
            span,
        });
        self
    }

    /// 追加一项。
    pub fn child(mut self, child: impl Into<DescriptionItem>) -> Self {
        self.items.push(child.into());
        self
    }

    /// 追加多项。
    pub fn children(
        mut self,
        children: impl IntoIterator<Item = impl Into<DescriptionItem>>,
    ) -> Self {
        self.items
            .extend(children.into_iter().map(Into::into).collect::<Vec<_>>());
        self
    }

    /// 追加一个分隔项。
    pub fn separator(mut self) -> Self {
        self.items.push(DescriptionItem::Separator);
        self
    }

    /// 按列数把项分配到各行：跨度超出剩余列时另起一行。
    fn group_item_rows(items: Vec<DescriptionItem>, columns: usize) -> Vec<Vec<DescriptionItem>> {
        let mut rows = vec![];
        let mut current_span = 0;
        for item in items.into_iter() {
            let span = item.column_span().unwrap_or(columns);
            if rows.is_empty() {
                rows.push(vec![]);
            }
            if current_span + span > columns {
                rows.push(vec![]);
                current_span = 0;
            }
            // 上方 rows.is_empty 时已 push 过一行，必有末行
            let last_group = invariant(rows.last_mut(), "描述列表：行分组必非空");
            last_group.push(item);
            current_span += span;
        }
        // 去掉尾部的空行。
        while let Some(last_group) = rows.last() {
            if !last_group.is_empty() {
                break;
            }
            rows.pop();
        }

        rows
    }
}

impl Default for DescriptionList {
    fn default() -> Self {
        Self::new()
    }
}

impl Sizable for DescriptionList {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl Styled for DescriptionList {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DescriptionList {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let DescriptionList {
            items,
            size,
            layout,
            label_width,
            bordered,
            columns,
            style,
        } = self;

        let base_gap = match size {
            ElementSize::XSmall | ElementSize::Small => px(2.),
            ElementSize::Medium => px(4.),
            ElementSize::Large => px(8.),
            ElementSize::ElementSize(_) => px(4.),
        };

        // 内边距仅用于水平布局。
        let (mut padding_x, mut padding_y) = match size {
            ElementSize::XSmall | ElementSize::Small => (px(4.), px(2.)),
            ElementSize::Medium => (px(8.), px(4.)),
            ElementSize::Large => (px(12.), px(6.)),
            ElementSize::ElementSize(_) => (px(8.), px(4.)),
        };

        let is_horizontal = layout.is_horizontal();
        let label_width = if is_horizontal {
            Some(label_width)
        } else {
            None
        };
        if !bordered {
            padding_x = px(0.);
            padding_y = px(0.);
        }
        let gap = if bordered { px(0.) } else { base_gap };

        let theme = cx.theme();
        let radius = theme.radius;
        let border_color = theme.border;
        let label_bg = theme.tokens.description_list_label;
        let label_fg = theme.description_list_label_foreground;
        let user_style = style;

        // 按列把项分组到各行。
        let rows = Self::group_item_rows(items, columns);
        let rows_len = rows.len();

        v_flex()
            .gap(gap)
            .overflow_hidden()
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
            .when(bordered, |this| {
                this.rounded(radius).border_1().border_color(border_color)
            })
            .children(rows.into_iter().enumerate().map(|(ix, items)| {
                let is_last = ix == rows_len - 1;
                h_flex()
                    .when(bordered && !is_last, |this| {
                        this.border_b_1().border_color(border_color)
                    })
                    .children(items.into_iter().enumerate().map(|(item_ix, item)| {
                        let is_first_col = item_ix == 0;

                        match item {
                            DescriptionItem::Item { label, value, span } => {
                                let el = if layout.is_vertical() {
                                    v_flex()
                                } else {
                                    div().flex().flex_row().h_full()
                                };

                                el.flex_1()
                                    .flex_basis(relative((span as f32) / (columns as f32)))
                                    .overflow_x_hidden()
                                    .child(
                                        div()
                                            .when(is_horizontal, |this| this.h_full())
                                            .text_color(label_fg)
                                            .text_sm()
                                            .px(padding_x)
                                            .py(padding_y)
                                            .when(bordered, |this| {
                                                this.when(is_horizontal, |this| {
                                                    this.border_r_1().when(!is_first_col, |this| {
                                                        this.border_l_1()
                                                    })
                                                })
                                                .when(!is_horizontal, |this| this.border_b_1())
                                                .border_color(border_color)
                                                .bg(label_bg)
                                            })
                                            .map(|this| match label_width {
                                                Some(label_width) => {
                                                    this.w(label_width).flex_shrink_0()
                                                }
                                                None => this,
                                            })
                                            .child(label),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .px(padding_x)
                                            .py(padding_y)
                                            .overflow_hidden()
                                            .child(value),
                                    )
                            }
                            DescriptionItem::Separator => div()
                                .h(px(8.))
                                .w_full()
                                .when(bordered, |this| this.bg(label_bg)),
                        }
                    }))
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::{DescriptionItem, DescriptionList};
    use crate::px;

    /// 列分组：跨度超出剩余列时换行，尾部空行被去掉。
    #[test]
    fn test_group_item_rows() {
        let items = vec![
            DescriptionItem::new("test1"),
            DescriptionItem::new("test2").span(2),
            DescriptionItem::new("test3"),
            DescriptionItem::new("test4"),
            DescriptionItem::new("test5"),
            DescriptionItem::new("test6").span(3),
            DescriptionItem::new("test7"),
        ];
        let rows = DescriptionList::group_item_rows(items, 3);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].len(), 2);
        assert_eq!(rows[1].len(), 3);
        assert_eq!(rows[2].len(), 1);
        assert_eq!(rows[3].len(), 1);
    }

    /// 构建器：列数被限制在 `1..=10`，项/分隔项正确追加，边框默认开启。
    #[test]
    fn test_description_list_builder() {
        let list = DescriptionList::new()
            .columns(99)
            .label_width(px(80.))
            .item("名称", "rgpui", 1)
            .separator()
            .child(DescriptionItem::new("版本").value("1.5.0"));

        assert_eq!(list.columns, 10);
        assert_eq!(list.items.len(), 3);
        assert!(matches!(list.items[1], DescriptionItem::Separator));
        assert!(list.bordered);
    }

    /// 边框开关与列数下限。
    #[test]
    fn test_description_list_bordered_and_columns() {
        let list = DescriptionList::new().bordered(false).columns(0);
        assert!(!list.bordered);
        assert_eq!(list.columns, 1);
    }
}
