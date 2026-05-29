use crate::{ActiveTheme, Disableable, Icon, Selectable, Sizable as _, StyledExt, h_flex};
use rgpui::{
    AnyElement, App, ClickEvent, Div, ElementId, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement, RenderOnce, Stateful,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _,
};
use smallvec::SmallVec;
use std::collections::HashMap;

/// 列表项的模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ListItemMode {
    /// 普通条目
    #[default]
    Entry,
    /// 分隔线
    Separator,
}

impl ListItemMode {
    /// 检查是否为分隔线
    #[inline]
    fn is_separator(&self) -> bool {
        matches!(self, ListItemMode::Separator)
    }
}

/// 列表项组件
#[derive(IntoElement)]
pub struct ListItem {
    /// 基础 Stateful Div
    base: Stateful<Div>,
    /// 列表项模式
    mode: ListItemMode,
    /// 样式引用
    style: StyleRefinement,
    /// 是否禁用
    disabled: bool,
    /// 是否选中
    selected: bool,
    /// 是否次要选中（如右键菜单）
    secondary_selected: bool,
    /// 是否确认状态
    confirmed: bool,
    /// 选中标记图标
    check_icon: Option<Icon>,
    /// 点击回调
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    /// 鼠标按下回调
    on_mouse_down:
        HashMap<MouseButton, Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>>,
    /// 鼠标进入回调
    on_mouse_enter: Option<Box<dyn Fn(&MouseMoveEvent, &mut Window, &mut App) + 'static>>,
    /// 后缀元素
    suffix: Option<Box<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>>,
    /// 子元素
    children: SmallVec<[AnyElement; 2]>,
}

impl ListItem {
    /// 创建新的列表项
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id: ElementId = id.into();
        Self {
            mode: ListItemMode::Entry,
            base: h_flex().id(id),
            style: StyleRefinement::default(),
            disabled: false,
            selected: false,
            secondary_selected: false,
            confirmed: false,
            on_click: None,
            on_mouse_down: HashMap::new(),
            on_mouse_enter: None,
            check_icon: None,
            suffix: None,
            children: SmallVec::new(),
        }
    }

    /// 将此列表项设置为分隔线，不可选中
    pub fn separator(mut self) -> Self {
        self.mode = ListItemMode::Separator;
        self
    }

    /// 设置显示选中标记图标，默认为 None
    pub fn check_icon(mut self, icon: impl Into<Icon>) -> Self {
        self.check_icon = Some(icon.into());
        self
    }

    /// 设置列表项为选中样式
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// 设置列表项为确认样式，将显示勾选图标
    pub fn confirmed(mut self, confirmed: bool) -> Self {
        self.confirmed = confirmed;
        self
    }

    /// 设置禁用状态
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置输入字段的后缀元素，例如清除按钮
    pub fn suffix<F, E>(mut self, builder: F) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        self.suffix = Some(Box::new(move |window, cx| {
            builder(window, cx).into_any_element()
        }));
        self
    }

    /// 添加点击事件处理
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    /// 添加鼠标按下事件处理
    pub fn on_mouse_down(
        mut self,
        button: MouseButton,
        handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_mouse_down.insert(button, Box::new(handler));
        self
    }

    /// 添加鼠标进入事件处理
    pub fn on_mouse_enter(
        mut self,
        handler: impl Fn(&MouseMoveEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_mouse_enter = Some(Box::new(handler));
        self
    }
}

impl Disableable for ListItem {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Selectable for ListItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }

    fn secondary_selected(mut self, selected: bool) -> Self {
        self.secondary_selected = selected;
        self
    }
}

impl Styled for ListItem {
    fn style(&mut self) -> &mut rgpui::StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for ListItem {
    fn extend(&mut self, elements: impl IntoIterator<Item = rgpui::AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ListItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let is_active = self.confirmed || self.selected || self.secondary_selected;

        let corner_radii = self.style.corner_radii.clone();

        let mut selected_style = StyleRefinement::default();
        selected_style.corner_radii = corner_radii;

        let is_selectable = !(self.disabled || self.mode.is_separator());

        self.base
            .relative()
            .gap_x_1()
            .py_1()
            .px_3()
            .text_base()
            .text_color(cx.theme().foreground)
            .relative()
            .items_center()
            .justify_between()
            .refine_style(&self.style)
            .when(is_selectable, |this| {
                this.when_some(self.on_click, |this, on_click| this.on_click(on_click))
                    .when_some(self.on_mouse_enter, |this, on_mouse_enter| {
                        this.on_mouse_move(move |ev, window, cx| (on_mouse_enter)(ev, window, cx))
                    })
                    .map(|this| {
                        self.on_mouse_down
                            .into_iter()
                            .fold(this, |this, (button, handler)| {
                                this.on_mouse_down(button, move |ev, window, cx| {
                                    handler(ev, window, cx)
                                })
                            })
                    })
                    .when(!is_active, |this| {
                        this.hover(|this| this.bg(cx.theme().list_hover))
                    })
            })
            .when(!is_selectable, |this| {
                this.text_color(cx.theme().muted_foreground)
            })
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_x_1()
                    .child(div().w_full().children(self.children))
                    .when_some(self.check_icon, |this, icon| {
                        this.child(
                            div().w_5().items_center().justify_center().when(
                                self.confirmed,
                                |this| {
                                    this.child(icon.small().text_color(cx.theme().muted_foreground))
                                },
                            ),
                        )
                    }),
            )
            .when_some(self.suffix, |this, suffix| this.child(suffix(window, cx)))
            .map(|this| {
                if is_selectable && (self.selected || self.secondary_selected) {
                    let bg = if self.selected && cx.theme().list.active_highlight {
                        cx.theme().list_active
                    } else {
                        cx.theme().accent
                    };

                    this.when(!self.secondary_selected, |this| this.bg(bg))
                        .when(cx.theme().list.active_highlight, |this| {
                            this.child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .right_0()
                                    .bottom_0()
                                    .border_1()
                                    .border_color(cx.theme().list_active_border)
                                    .refine_style(&selected_style),
                            )
                        })
                } else {
                    this
                }
            })
    }
}
