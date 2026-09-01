//! 单选框组件，支持互斥选择的圆形选择控件。

use std::rc::Rc;

use crate::elements::checkbox::checkbox_check_icon;
use crate::prelude::FluentBuilder as _;
use crate::{
    ActiveTheme, AnyElement, App, Axis, ComponentText, Disableable, ElementId, ElementSize,
    FocusableExt as _, InteractiveElement, IntoElement, ParentElement, RenderOnce, Selectable,
    SharedString, Sizable, StatefulInteractiveElement, StyleRefinement, Styled, StyledExt as _,
    Window, div, h_flex, px, relative, rems, v_flex,
};

/// 单选按钮（Radio）元素。
///
/// 不包含 RadioGroup 实现，可自行管理分组。
#[derive(IntoElement)]
pub struct Radio {
    /// 基础 Div 元素
    base: crate::Div,
    /// 样式精炼
    style: StyleRefinement,
    /// 元素 ID
    id: ElementId,
    /// 标签文本
    label: Option<ComponentText>,
    /// 子元素
    children: Vec<AnyElement>,
    /// 是否选中
    checked: bool,
    /// 是否禁用
    disabled: bool,
    /// 是否为 Tab 停靠点
    tab_stop: bool,
    /// Tab 索引
    tab_index: isize,
    /// 尺寸
    size: ElementSize,
    /// 点击事件回调
    on_click: Option<Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
    /// 提示文本（当前简化存储，暂不渲染）
    tooltip: Option<SharedString>,
}

impl Radio {
    /// 创建带有指定 id 的新 Radio 元素。
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            base: div(),
            style: StyleRefinement::default(),
            label: None,
            children: Vec::new(),
            checked: false,
            disabled: false,
            tab_index: 0,
            tab_stop: true,
            size: ElementSize::default(),
            on_click: None,
            tooltip: None,
        }
    }

    /// 设置 Radio 的提示文本。
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// 设置 Radio 的标签。
    pub fn label(mut self, label: impl Into<ComponentText>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 设置 Radio 的选中状态，默认为 `false`。
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// 设置 Radio 的禁用状态，默认为 `false`。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置 Radio 的 Tab 索引，默认为 `0`。
    pub fn tab_index(mut self, tab_index: isize) -> Self {
        self.tab_index = tab_index;
        self
    }

    /// 设置 Radio 是否为 Tab 停靠点，默认为 `true`。
    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self
    }

    /// 添加 Radio 的点击回调。
    ///
    /// `&bool` 参数表示点击后的**新选中状态**。
    pub fn on_click(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    fn handle_click(
        on_click: &Option<Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
        checked: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        let new_checked = !checked;
        if let Some(f) = on_click {
            (f)(&new_checked, window, cx);
        }
    }
}

impl Sizable for Radio {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl Styled for Radio {
    fn style(&mut self) -> &mut crate::StyleRefinement {
        &mut self.style
    }
}

impl InteractiveElement for Radio {
    fn interactivity(&mut self) -> &mut crate::Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for Radio {}

impl ParentElement for Radio {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Disableable for Radio {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Selectable for Radio {
    fn selected(self, selected: bool) -> Self {
        self.checked(selected)
    }

    fn is_selected(&self) -> bool {
        self.checked
    }
}

impl From<&'static str> for Radio {
    fn from(label: &'static str) -> Self {
        Self::new(label).label(label)
    }
}

impl From<SharedString> for Radio {
    fn from(label: SharedString) -> Self {
        Self::new(label.clone()).label(label)
    }
}

impl From<String> for Radio {
    fn from(label: String) -> Self {
        Self::new(SharedString::from(label.clone())).label(SharedString::from(label))
    }
}

impl RenderOnce for Radio {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let checked = self.checked;
        let focus_handle = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let is_focused = focus_handle.is_focused(window);
        let disabled = self.disabled;

        let (border_color, bg) = if checked {
            (cx.theme().primary, cx.theme().primary)
        } else {
            (cx.theme().input, cx.theme().input.opacity(0.5))
        };
        let (border_color, bg) = if disabled {
            (border_color.opacity(0.5), bg.opacity(0.5))
        } else {
            (border_color, bg)
        };

        // 包裹一个 flex 以修复 Radio 的 inline 显示
        div().child(
            self.base
                .id(self.id.clone())
                .when(!self.disabled, |this| {
                    this.track_focus(
                        &focus_handle
                            .tab_stop(self.tab_stop)
                            .tab_index(self.tab_index),
                    )
                })
                .h_flex()
                .gap_x_2()
                .text_color(cx.theme().foreground)
                .items_start()
                .line_height(relative(1.))
                .rounded(cx.theme().radius * 0.5)
                .focus_ring(is_focused, px(2.), window, cx)
                .map(|this| match self.size {
                    ElementSize::XSmall => this.text_xs(),
                    ElementSize::Small => this.text_sm(),
                    ElementSize::Medium => this.text_base(),
                    ElementSize::Large => this.text_lg(),
                    _ => this,
                })
                .refine_style(&self.style)
                .child(
                    div()
                        .relative()
                        .map(|this| match self.size {
                            ElementSize::XSmall => this.size_3(),
                            ElementSize::Small => this.size_3p5(),
                            ElementSize::Medium => this.size_4(),
                            ElementSize::Large => this.size(rems(1.125)),
                            _ => this.size_4(),
                        })
                        .flex_shrink_0()
                        .rounded_full()
                        .border_1()
                        .border_color(border_color)
                        .when(cx.theme().shadow && !disabled, |this| this.shadow_xs())
                        .map(|this| match self.checked {
                            false => this.bg(cx.theme().input_background()),
                            true if disabled => this.bg(bg),
                            true => this.bg(cx.theme().tokens.primary),
                        })
                        .child(checkbox_check_icon(
                            self.id, self.size, checked, disabled, window, cx,
                        )),
                )
                .when(!self.children.is_empty() || self.label.is_some(), |this| {
                    this.child(
                        v_flex()
                            .w_full()
                            .line_height(relative(1.2))
                            .gap_1()
                            .when_some(self.label, |this, label| {
                                this.child(
                                    div()
                                        .size_full()
                                        .line_height(relative(1.))
                                        .when(self.disabled, |this| {
                                            this.text_color(cx.theme().muted_foreground)
                                        })
                                        .child(label),
                                )
                            })
                            .children(self.children),
                    )
                })
                .on_mouse_down(crate::MouseButton::Left, |_, window, _| {
                    // 避免在鼠标按下时获得焦点
                    window.prevent_default();
                })
                .when(!self.disabled, |this| {
                    this.on_click({
                        let on_click = self.on_click.clone();
                        move |_, window, cx| {
                            window.prevent_default();
                            Self::handle_click(&on_click, checked, window, cx);
                        }
                    })
                }),
        )
    }
}

/// 单选按钮组（RadioGroup）元素。
#[derive(IntoElement)]
pub struct RadioGroup {
    /// 元素 ID
    id: ElementId,
    /// 样式精炼
    style: StyleRefinement,
    /// 子 Radio 列表
    radios: Vec<Radio>,
    /// 布局方向
    layout: Axis,
    /// 选中的索引
    selected_index: Option<usize>,
    /// 是否禁用
    disabled: bool,
    /// 选中索引变化回调
    on_click: Option<Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>>,
}

impl RadioGroup {
    /// 创建新的 RadioGroup，默认垂直布局。
    fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default().flex_1(),
            on_click: None,
            layout: Axis::Vertical,
            selected_index: None,
            disabled: false,
            radios: vec![],
        }
    }

    /// 创建默认垂直布局的 RadioGroup。
    pub fn vertical(id: impl Into<ElementId>) -> Self {
        Self::new(id)
    }

    /// 创建水平布局的 RadioGroup。
    pub fn horizontal(id: impl Into<ElementId>) -> Self {
        Self::new(id).layout(Axis::Horizontal)
    }

    /// 设置 RadioGroup 的布局方向。默认为 `Axis::Vertical`。
    pub fn layout(mut self, layout: Axis) -> Self {
        self.layout = layout;
        self
    }

    /// 添加选中索引变化回调。
    ///
    /// `&usize` 参数表示选中的索引。
    pub fn on_click(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// 设置选中的索引。
    pub fn selected_index(mut self, index: Option<usize>) -> Self {
        self.selected_index = index;
        self
    }

    /// 设置禁用状态。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 添加一个子 Radio 元素。
    pub fn child(mut self, child: impl Into<Radio>) -> Self {
        self.radios.push(child.into());
        self
    }

    /// 添加多个子 Radio 元素。
    pub fn children(mut self, children: impl IntoIterator<Item = impl Into<Radio>>) -> Self {
        self.radios.extend(children.into_iter().map(Into::into));
        self
    }
}

impl Styled for RadioGroup {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for RadioGroup {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let on_click = self.on_click;
        let disabled = self.disabled;
        let selected_ix = self.selected_index;

        let base = if self.layout == Axis::Vertical {
            v_flex()
        } else {
            h_flex().w_full().flex_wrap()
        };

        let mut container = div().id(self.id);
        *container.style() = self.style;

        container.child(
            base.gap_3()
                .children(self.radios.into_iter().enumerate().map(|(ix, mut radio)| {
                    let checked = selected_ix == Some(ix);

                    radio.id = ix.into();
                    radio.disabled(disabled).checked(checked).when_some(
                        on_click.clone(),
                        |this, on_click| {
                            this.on_click(move |_, window, cx| {
                                on_click(&ix, window, cx);
                            })
                        },
                    )
                })),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 Radio 基本构造
    #[test]
    fn test_radio_build() {
        let r = Radio::new("test-radio")
            .label("选项 A")
            .checked(true)
            .with_size(ElementSize::Small);
        assert!(r.checked);
        assert!(r.label.is_some());
    }

    /// 测试从字符串构造 Radio
    #[test]
    fn test_radio_from_str() {
        let r: Radio = "选项 B".into();
        assert_eq!(r.id, ElementId::from("选项 B"));
        assert!(r.label.is_some());
    }

    /// 测试 RadioGroup 构造与布局
    #[test]
    fn test_radio_group_build() {
        let group = RadioGroup::horizontal("test-group")
            .selected_index(Some(1))
            .child(Radio::new("r1").label("A"))
            .child(Radio::new("r2").label("B"));
        assert_eq!(group.radios.len(), 2);
        assert_eq!(group.selected_index, Some(1));
        assert_eq!(group.layout, Axis::Horizontal);
    }
}
