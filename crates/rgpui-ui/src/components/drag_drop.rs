//! 拖拽组件：可拖拽元素与放置区域（DropZone）。

use rgpui::{prelude::FluentBuilder as _, *};
use std::{fmt::Debug, rc::Rc};

/// 拖拽数据：包含被拖拽的值、可选标签与可选预览工厂。
pub struct DragData<T: Clone + Debug> {
    /// 被拖拽的数据值。
    pub data: T,
    /// 拖拽时显示的标签文本。
    pub label: Option<SharedString>,
    /// 自定义预览元素工厂。
    pub preview_factory: Option<Rc<dyn Fn() -> AnyElement>>,
    /// 拖拽过程中鼠标的位置。
    pub position: Point<Pixels>,
}

impl<T: Clone + Debug> Clone for DragData<T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            label: self.label.clone(),
            preview_factory: self.preview_factory.clone(),
            position: self.position,
        }
    }
}

impl<T: Clone + Debug> Debug for DragData<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DragData")
            .field("data", &self.data)
            .field("label", &self.label)
            .field("preview_factory", &self.preview_factory.is_some())
            .field("position", &self.position)
            .finish()
    }
}

impl<T: Clone + Debug> DragData<T> {
    /// 创建拖拽数据。
    pub fn new(data: T) -> Self {
        Self {
            data,
            label: None,
            preview_factory: None,
            position: Point::default(),
        }
    }

    /// 设置拖拽标签文本。
    pub fn with_label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 设置自定义预览元素工厂。
    pub fn with_preview<F>(mut self, factory: F) -> Self
    where
        F: Fn() -> AnyElement + 'static,
    {
        self.preview_factory = Some(Rc::new(factory));
        self
    }

    /// 设置拖拽位置。
    pub fn with_position(mut self, position: Point<Pixels>) -> Self {
        self.position = position;
        self
    }
}

impl<T: Clone + Debug + 'static> Render for DragData<T> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        if let Some(factory) = &self.preview_factory {
            let preview = factory();
            return div()
                .absolute()
                .left(self.position.x)
                .top(self.position.y)
                .child(preview);
        }

        let size = rgpui::size(px(250.0), px(80.0));

        div()
            .pl(self.position.x - size.width / 2.0)
            .pt(self.position.y - size.height / 2.0)
            .child(
                div()
                    .flex()
                    .justify_center()
                    .items_center()
                    .min_w(size.width)
                    .max_w(px(300.0))
                    .min_h(size.height)
                    .px(px(16.0))
                    .py(px(12.0))
                    .bg(theme.tokens.popover.opacity(0.95))
                    .border_1()
                    .border_color(theme.tokens.border)
                    .text_color(theme.tokens.foreground)
                    .font_family(theme.font_family.clone())
                    .text_size(px(14.0))
                    .font_weight(FontWeight::MEDIUM)
                    .rounded(theme.radius)
                    .shadow(vec![BoxShadow {
                        color: hsla(0.0, 0.0, 0.0, 0.3),
                        offset: point(px(0.0), px(4.0)),
                        blur_radius: px(12.0),
                        spread_radius: px(0.0),
                        inset: false,
                    }])
                    .when_some(self.label.clone(), |this, label| this.child(label))
                    .when(self.label.is_none(), |this| this.child("Dragging...")),
            )
    }
}

/// 可拖拽元素：包裹任意内容并提供拖拽能力。
#[derive(IntoElement)]
pub struct Draggable<T: Clone + Debug + 'static> {
    /// 底层有状态容器。
    base: Stateful<Div>,
    /// 拖拽数据。
    drag_data: DragData<T>,
    /// 鼠标悬停时的光标样式。
    cursor_style: CursorStyle,
    /// 悬停背景色。
    hover_bg: Option<Hsla>,
    /// 子元素列表。
    children: Vec<AnyElement>,
    /// 用户样式。
    style: StyleRefinement,
}

impl<T: Clone + Debug + 'static> Draggable<T> {
    /// 创建可拖拽元素。
    pub fn new(id: impl Into<ElementId>, drag_data: DragData<T>) -> Self {
        Self {
            base: div().id(id.into()),
            drag_data,
            cursor_style: CursorStyle::PointingHand,
            hover_bg: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// 设置光标样式。
    pub fn cursor_style(mut self, cursor: CursorStyle) -> Self {
        self.cursor_style = cursor;
        self
    }

    /// 设置悬停背景色。
    pub fn hover_bg(mut self, color: Hsla) -> Self {
        self.hover_bg = Some(color);
        self
    }

    /// 添加子元素。
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    /// 批量添加子元素。
    pub fn children<I>(mut self, children: impl IntoIterator<Item = I>) -> Self
    where
        I: IntoElement,
    {
        for child in children {
            self.children.push(child.into_any_element());
        }
        self
    }
}

impl<T: Clone + Debug + 'static> Styled for Draggable<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<T: Clone + Debug + 'static> ParentElement for Draggable<T> {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl<T: Clone + Debug + 'static> RenderOnce for Draggable<T> {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let drag_data = self.drag_data.clone();
        let user_style = self.style;

        self.base
            .cursor(self.cursor_style)
            .when_some(self.hover_bg, |this, bg| {
                this.hover(move |style| style.bg(bg))
            })
            .on_drag(drag_data, |data: &DragData<T>, position, _, cx| {
                cx.new(|_| data.clone().with_position(position))
            })
            .map(|this| {
                let mut div = this;
                div.style().refine(&user_style);
                div
            })
            .children(self.children)
    }
}

/// 放置区域的外观样式。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DropZoneStyle {
    /// 虚线边框。
    Dashed,
    /// 实线边框。
    Solid,
    /// 填充背景。
    Filled,
}

/// 放置区域：接收拖拽数据的容器。
#[derive(IntoElement)]
pub struct DropZone<T: Clone + Debug + 'static> {
    /// 底层有状态容器。
    base: Stateful<Div>,
    /// 外观样式。
    drop_style: DropZoneStyle,
    /// 是否处于激活（拖拽悬停）状态。
    active: bool,
    /// 最小高度。
    min_height: Option<Pixels>,
    /// 子元素列表。
    children: Vec<AnyElement>,
    /// 用户样式。
    user_style: StyleRefinement,
    /// 放置回调。
    on_drop: Option<Rc<dyn Fn(&DragData<T>, &mut Window, &mut App)>>,
    /// 泛型占位。
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Clone + Debug + 'static> DropZone<T> {
    /// 创建放置区域。
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            base: div().id(id.into()),
            drop_style: DropZoneStyle::Dashed,
            active: false,
            min_height: None,
            children: Vec::new(),
            user_style: StyleRefinement::default(),
            on_drop: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// 设置外观样式。
    pub fn drop_zone_style(mut self, style: DropZoneStyle) -> Self {
        self.drop_style = style;
        self
    }

    /// 设置放置回调。
    pub fn on_drop<F>(mut self, handler: F) -> Self
    where
        F: Fn(&DragData<T>, &mut Window, &mut App) + 'static,
    {
        self.on_drop = Some(Rc::new(handler));
        self
    }

    /// 设置是否激活。
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// 设置最小高度。
    pub fn min_h(mut self, height: impl Into<Pixels>) -> Self {
        self.min_height = Some(height.into());
        self
    }

    /// 添加子元素。
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    /// 批量添加子元素。
    pub fn children<I>(mut self, children: impl IntoIterator<Item = I>) -> Self
    where
        I: IntoElement,
    {
        for child in children {
            self.children.push(child.into_any_element());
        }
        self
    }
}

impl<T: Clone + Debug + 'static> Styled for DropZone<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.user_style
    }
}

impl<T: Clone + Debug + 'static> InteractiveElement for DropZone<T> {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl<T: Clone + Debug + 'static> StatefulInteractiveElement for DropZone<T> {}

impl<T: Clone + Debug + 'static> ParentElement for DropZone<T> {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl<T: Clone + Debug + 'static> RenderOnce for DropZone<T> {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.user_style;

        let (border_width, border_color, bg_color) = match (self.drop_style, self.active) {
            (DropZoneStyle::Dashed, false) => (
                px(2.0),
                theme.tokens.border.color,
                rgpui::transparent_black(),
            ),
            (DropZoneStyle::Dashed, true) => (
                px(2.0),
                theme.tokens.primary.color,
                theme.tokens.primary.opacity(0.05),
            ),
            (DropZoneStyle::Solid, false) => (
                px(2.0),
                theme.tokens.border.color,
                rgpui::transparent_black(),
            ),
            (DropZoneStyle::Solid, true) => (
                px(2.0),
                theme.tokens.primary.color,
                theme.tokens.primary.opacity(0.1),
            ),
            (DropZoneStyle::Filled, false) => {
                (px(1.0), theme.tokens.border.color, theme.tokens.muted.color)
            }
            (DropZoneStyle::Filled, true) => (
                px(2.0),
                theme.tokens.primary.color,
                theme.tokens.primary.opacity(0.15),
            ),
        };

        self.base
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .w_full()
            .when_some(self.min_height, |this, h| this.min_h(h))
            .px(px(16.0))
            .py(px(16.0))
            .rounded(theme.radius_lg)
            .bg(bg_color)
            .border_color(border_color)
            .when(self.drop_style == DropZoneStyle::Dashed, |this| {
                this.border(border_width)
            })
            .when(self.drop_style != DropZoneStyle::Dashed, |this| {
                this.border(border_width)
            })
            .map(|this| {
                let mut div = this;
                div.style().refine(&user_style);
                div
            })
            .children(self.children)
    }
}
