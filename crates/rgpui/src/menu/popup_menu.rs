use crate::menu::actions::{Cancel, Confirm, SelectDown, SelectUp};
use crate::menu::actions::{SelectLeft, SelectRight};
use crate::menu::menu_item::MenuItemElement;
use crate::{
    Action, Anchor, AnyElement, App, AppContext, Bounds, Context, DismissEvent, Edges, Entity,
    EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyBinding,
    ParentElement, Pixels, Render, Role, ScrollHandle, SharedString, StatefulInteractiveElement,
    Styled, WeakEntity, Window, anchored, deferred, div, prelude::FluentBuilder, px, rems,
};
use crate::{ActiveTheme, ElementExt, Icon, IconName, Sizable as _, h_flex, v_flex};
use crate::{ClickEvent, Half, MouseDownEvent, OwnedMenuItem, Point, Subscription};
use crate::{ElementSize, ScrollableElement as _, Side, StyledExt, elements::Kbd};

use std::rc::Rc;

const CONTEXT: &str = "PopupMenu";

/// 初始化弹窗菜单的快捷键绑定
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", Confirm { secondary: false }, Some(CONTEXT)),
        KeyBinding::new("escape", Cancel, Some(CONTEXT)),
        KeyBinding::new("up", SelectUp, Some(CONTEXT)),
        KeyBinding::new("down", SelectDown, Some(CONTEXT)),
        KeyBinding::new("left", SelectLeft, Some(CONTEXT)),
        KeyBinding::new("right", SelectRight, Some(CONTEXT)),
    ]);
}

/// 弹窗菜单中的菜单项
pub enum PopupMenuItem {
    /// 菜单分隔符项
    Separator,
    /// 非交互标签项
    Label(SharedString),
    /// 标准菜单项
    Item {
        /// 菜单项图标
        icon: Option<Icon>,
        /// 菜单项标签
        label: SharedString,
        /// 是否禁用
        disabled: bool,
        /// 是否选中
        checked: bool,
        /// 是否为链接项
        is_link: bool,
        /// 菜单项动作
        action: Option<Box<dyn Action>>,
        /// 链接项点击处理器
        handler: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    },
    /// 自定义元素渲染的菜单项
    ElementItem {
        /// 菜单项图标
        icon: Option<Icon>,
        /// 是否禁用
        disabled: bool,
        /// 是否选中
        checked: bool,
        /// 菜单项动作
        action: Option<Box<dyn Action>>,
        /// 自定义渲染函数
        render: Box<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>,
        /// 点击处理器
        handler: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    },
    /// 打开另一个弹窗菜单的子菜单项
    ///
    /// 注意：仅当父菜单不是 `scrollable` 时才支持。
    Submenu {
        /// 子菜单图标
        icon: Option<Icon>,
        /// 子菜单标签
        label: SharedString,
        /// 是否禁用
        disabled: bool,
        /// 子菜单实体
        menu: Entity<PopupMenu>,
    },
}

impl FluentBuilder for PopupMenuItem {}
impl PopupMenuItem {
    /// 使用给定的标签创建新的菜单项
    #[inline]
    pub fn new(label: impl Into<SharedString>) -> Self {
        PopupMenuItem::Item {
            icon: None,
            label: label.into(),
            disabled: false,
            checked: false,
            action: None,
            is_link: false,
            handler: None,
        }
    }

    /// 使用自定义元素渲染创建菜单项
    #[inline]
    pub fn element<F, E>(builder: F) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        PopupMenuItem::ElementItem {
            icon: None,
            disabled: false,
            checked: false,
            action: None,
            render: Box::new(move |window, cx| builder(window, cx).into_any_element()),
            handler: None,
        }
    }

    /// 创建打开另一个弹窗菜单的子菜单项
    #[inline]
    pub fn submenu(label: impl Into<SharedString>, menu: Entity<PopupMenu>) -> Self {
        PopupMenuItem::Submenu {
            icon: None,
            label: label.into(),
            disabled: false,
            menu,
        }
    }

    /// 创建分隔符菜单项
    #[inline]
    pub fn separator() -> Self {
        PopupMenuItem::Separator
    }

    /// 创建标签菜单项
    #[inline]
    pub fn label(label: impl Into<SharedString>) -> Self {
        PopupMenuItem::Label(label.into())
    }

    /// 设置菜单项的图标
    ///
    /// 仅适用于 [`PopupMenuItem::Item`]、[`PopupMenuItem::ElementItem`] 和 [`PopupMenuItem::Submenu`]。
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        match &mut self {
            PopupMenuItem::Item { icon: i, .. } => {
                *i = Some(icon.into());
            }
            PopupMenuItem::ElementItem { icon: i, .. } => {
                *i = Some(icon.into());
            }
            PopupMenuItem::Submenu { icon: i, .. } => {
                *i = Some(icon.into());
            }
            _ => {}
        }
        self
    }

    /// 设置菜单项的 action
    ///
    /// 仅适用于 [`PopupMenuItem::Item`] 和 [`PopupMenuItem::ElementItem`]。
    pub fn action(mut self, action: Box<dyn Action>) -> Self {
        match &mut self {
            PopupMenuItem::Item { action: a, .. } => {
                *a = Some(action);
            }
            PopupMenuItem::ElementItem { action: a, .. } => {
                *a = Some(action);
            }
            _ => {}
        }
        self
    }

    /// 设置菜单项的禁用状态
    ///
    /// 仅适用于 [`PopupMenuItem::Item`]、[`PopupMenuItem::ElementItem`] 和 [`PopupMenuItem::Submenu`]。
    pub fn disabled(mut self, disabled: bool) -> Self {
        match &mut self {
            PopupMenuItem::Item { disabled: d, .. } => {
                *d = disabled;
            }
            PopupMenuItem::ElementItem { disabled: d, .. } => {
                *d = disabled;
            }
            PopupMenuItem::Submenu { disabled: d, .. } => {
                *d = disabled;
            }
            _ => {}
        }
        self
    }

    /// 设置菜单项的选中状态
    ///
    /// 注意：如果 `check_side` 为 [`Side::Left`]，图标将被替换为选中图标。
    pub fn checked(mut self, checked: bool) -> Self {
        match &mut self {
            PopupMenuItem::Item { checked: c, .. } => {
                *c = checked;
            }
            PopupMenuItem::ElementItem { checked: c, .. } => {
                *c = checked;
            }
            _ => {}
        }
        self
    }

    /// 为菜单项添加点击处理器
    ///
    /// 仅适用于 [`PopupMenuItem::Item`] 和 [`PopupMenuItem::ElementItem`]。
    pub fn on_click<F>(mut self, handler: F) -> Self
    where
        F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    {
        match &mut self {
            PopupMenuItem::Item { handler: h, .. } => {
                *h = Some(Rc::new(handler));
            }
            PopupMenuItem::ElementItem { handler: h, .. } => {
                *h = Some(Rc::new(handler));
            }
            _ => {}
        }
        self
    }

    /// 创建链接菜单项
    #[inline]
    pub fn link(label: impl Into<SharedString>, href: impl Into<String>) -> Self {
        let href = href.into();
        PopupMenuItem::Item {
            icon: None,
            label: label.into(),
            disabled: false,
            checked: false,
            action: None,
            is_link: true,
            handler: Some(Rc::new(move |_, _, cx| cx.open_url(&href))),
        }
    }

    #[inline]
    fn is_clickable(&self) -> bool {
        !matches!(self, PopupMenuItem::Separator)
            && matches!(
                self,
                PopupMenuItem::Item {
                    disabled: false,
                    ..
                } | PopupMenuItem::ElementItem {
                    disabled: false,
                    ..
                } | PopupMenuItem::Submenu {
                    disabled: false,
                    ..
                }
            )
    }

    #[inline]
    fn is_separator(&self) -> bool {
        matches!(self, PopupMenuItem::Separator)
    }

    fn has_left_icon(&self, check_side: Side) -> bool {
        match self {
            PopupMenuItem::Item { icon, checked, .. } => {
                icon.is_some() || (check_side.is_left() && *checked)
            }
            PopupMenuItem::ElementItem { icon, checked, .. } => {
                icon.is_some() || (check_side.is_left() && *checked)
            }
            PopupMenuItem::Submenu { icon, .. } => icon.is_some(),
            _ => false,
        }
    }

    #[inline]
    fn is_checked(&self) -> bool {
        match self {
            PopupMenuItem::Item { checked, .. } => *checked,
            PopupMenuItem::ElementItem { checked, .. } => *checked,
            _ => false,
        }
    }

    fn a11y_label(&self) -> Option<SharedString> {
        match self {
            PopupMenuItem::Item { label, .. }
            | PopupMenuItem::Label(label)
            | PopupMenuItem::Submenu { label, .. } => Some(label.clone()),
            PopupMenuItem::Separator | PopupMenuItem::ElementItem { .. } => None,
        }
    }
}

/// 弹窗菜单实体
pub struct PopupMenu {
    pub(crate) focus_handle: FocusHandle,
    pub(crate) menu_items: Vec<PopupMenuItem>,
    /// 处理动作的实体的焦点句柄
    pub(crate) action_context: Option<FocusHandle>,
    /// 关闭时要恢复的焦点句柄。与 `action_context` 不同，这不会改变动作的派发位置：
    /// 它们仍然从菜单自身的焦点路径向上冒泡（经过触发器元素的祖先链）。
    pub(crate) previous_focus_handle: Option<FocusHandle>,
    selected_index: Option<usize>,
    min_width: Option<Pixels>,
    max_width: Option<Pixels>,
    max_height: Option<Pixels>,
    bounds: Bounds<Pixels>,
    size: ElementSize,
    check_side: Side,

    /// 此菜单的父菜单（如果是子菜单）
    parent_menu: Option<WeakEntity<Self>>,
    scrollable: bool,
    external_link_icon: bool,
    scroll_handle: ScrollHandle,
    // 渲染时更新
    submenu_anchor: (Anchor, Pixels),

    /// 此菜单层的绘制优先级。顶层菜单从 1 开始，每个嵌套子菜单递增，
    /// 这样更深的层级总是绘制在更浅的层级之上。这修复了背景内容
    /// （例如底层的列表）透过多层子菜单渗出的问题——当嵌套的 `anchored`
    /// 弹层共享相同的绘制顺序时会发生这种情况。
    ///
    /// 顶层菜单依赖其容器（例如 `Popover`、`ContextMenu`）进行 `deferred` 绘制，
    /// 每个子菜单在 `render_item` 中以 `priority + 1` 延迟一次。每层保持单一的
    /// 延迟层级很重要，因为 GPUI 限制了嵌套延迟的深度（参见 `prepaint_deferred_draws`）。
    priority: usize,

    _subscriptions: Vec<Subscription>,
}

impl PopupMenu {
    /// 创建新的弹窗菜单
    pub(crate) fn new(cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            action_context: None,
            previous_focus_handle: None,
            parent_menu: None,
            menu_items: Vec::new(),
            selected_index: None,
            min_width: None,
            max_width: None,
            max_height: None,
            check_side: Side::Left,
            bounds: Bounds::default(),
            scrollable: false,
            scroll_handle: ScrollHandle::default(),
            external_link_icon: true,
            size: ElementSize::default(),
            submenu_anchor: (Anchor::TopLeft, Pixels::ZERO),
            priority: 1,
            _subscriptions: vec![],
        }
    }

    /// 构建弹窗菜单实体
    pub fn build(
        window: &mut Window,
        cx: &mut App,
        f: impl FnOnce(Self, &mut Window, &mut Context<PopupMenu>) -> Self,
    ) -> Entity<Self> {
        cx.new(|cx| f(Self::new(cx), window, cx))
    }

    /// 设置处理动作的实体的焦点句柄
    ///
    /// 当菜单关闭或动作触发前，焦点将返回到此句柄。
    ///
    /// 然后动作将被派发到该句柄。
    pub fn action_context(mut self, handle: FocusHandle) -> Self {
        self.action_context = Some(handle);
        self
    }

    pub(crate) fn set_action_context(
        &mut self,
        action_context: Option<FocusHandle>,
        cx: &mut Context<Self>,
    ) {
        self.action_context = action_context.clone();

        for item in &self.menu_items {
            if let PopupMenuItem::Submenu { menu, .. } = item {
                menu.update(cx, |menu, cx| {
                    menu.set_action_context(action_context.clone(), cx);
                });
            }
        }
    }

    /// 设置菜单关闭时要恢复的焦点，而不改变动作的派发位置
    pub(crate) fn set_previous_focus(
        &mut self,
        handle: Option<FocusHandle>,
        cx: &mut Context<Self>,
    ) {
        self.previous_focus_handle = handle.clone();

        for item in &self.menu_items {
            if let PopupMenuItem::Submenu { menu, .. } = item {
                menu.update(cx, |menu, cx| {
                    menu.set_previous_focus(handle.clone(), cx);
                });
            }
        }
    }

    /// 设置弹窗菜单的最小宽度，默认 120px
    pub fn min_w(mut self, width: impl Into<Pixels>) -> Self {
        self.min_width = Some(width.into());
        self
    }

    /// 设置弹窗菜单的最大宽度，默认 500px
    pub fn max_w(mut self, width: impl Into<Pixels>) -> Self {
        self.max_width = Some(width.into());
        self
    }

    /// 设置弹窗菜单的最大高度，默认为窗口高度的一半
    pub fn max_h(mut self, height: impl Into<Pixels>) -> Self {
        self.max_height = Some(height.into());
        self
    }

    /// 设置菜单是否可滚动以显示垂直滚动条
    ///
    /// 注意：如果为 true，子菜单将无法支持。
    pub fn scrollable(mut self, scrollable: bool) -> Self {
        self.scrollable = scrollable;
        self
    }

    /// 设置选中图标的显示侧，默认为 [`Side::Left`]
    pub fn check_side(mut self, side: Side) -> Self {
        self.check_side = side;
        self
    }

    /// 设置菜单是否显示外部链接图标，默认为 true
    pub fn external_link_icon(mut self, visible: bool) -> Self {
        self.external_link_icon = visible;
        self
    }

    /// 添加菜单项
    pub fn menu(self, label: impl Into<SharedString>, action: Box<dyn Action>) -> Self {
        self.menu_with_disabled(label, action, false)
    }

    /// 添加带启用状态的菜单项
    pub fn menu_with_enable(
        mut self,
        label: impl Into<SharedString>,
        action: Box<dyn Action>,
        enable: bool,
    ) -> Self {
        self.add_menu_item(label, None, action, !enable, false);
        self
    }

    /// 添加带禁用状态的菜单项
    pub fn menu_with_disabled(
        mut self,
        label: impl Into<SharedString>,
        action: Box<dyn Action>,
        disabled: bool,
    ) -> Self {
        self.add_menu_item(label, None, action, disabled, false);
        self
    }

    /// 添加标签
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.menu_items.push(PopupMenuItem::label(label.into()));
        self
    }

    /// 添加打开链接的菜单项
    pub fn link(self, label: impl Into<SharedString>, href: impl Into<String>) -> Self {
        self.link_with_disabled(label, href, false)
    }

    /// 添加带禁用状态的打开链接菜单项
    pub fn link_with_disabled(
        mut self,
        label: impl Into<SharedString>,
        href: impl Into<String>,
        disabled: bool,
    ) -> Self {
        let href = href.into();
        self.menu_items
            .push(PopupMenuItem::link(label, href).disabled(disabled));
        self
    }

    /// 添加带图标打开链接的菜单项
    pub fn link_with_icon(
        self,
        label: impl Into<SharedString>,
        icon: impl Into<Icon>,
        href: impl Into<String>,
    ) -> Self {
        self.link_with_icon_and_disabled(label, icon, href, false)
    }

    /// 添加带图标和禁用状态的打开链接菜单项
    fn link_with_icon_and_disabled(
        mut self,
        label: impl Into<SharedString>,
        icon: impl Into<Icon>,
        href: impl Into<String>,
        disabled: bool,
    ) -> Self {
        let href = href.into();
        self.menu_items.push(
            PopupMenuItem::link(label, href)
                .icon(icon)
                .disabled(disabled),
        );
        self
    }

    /// 添加带图标的菜单项
    pub fn menu_with_icon(
        self,
        label: impl Into<SharedString>,
        icon: impl Into<Icon>,
        action: Box<dyn Action>,
    ) -> Self {
        self.menu_with_icon_and_disabled(label, icon, action, false)
    }

    /// 添加带图标和禁用状态的菜单项
    pub fn menu_with_icon_and_disabled(
        mut self,
        label: impl Into<SharedString>,
        icon: impl Into<Icon>,
        action: Box<dyn Action>,
        disabled: bool,
    ) -> Self {
        self.add_menu_item(label, Some(icon.into()), action, disabled, false);
        self
    }

    /// 添加带选中图标的菜单项
    pub fn menu_with_check(
        self,
        label: impl Into<SharedString>,
        checked: bool,
        action: Box<dyn Action>,
    ) -> Self {
        self.menu_with_check_and_disabled(label, checked, action, false)
    }

    /// 添加带选中图标和禁用状态的菜单项
    pub fn menu_with_check_and_disabled(
        mut self,
        label: impl Into<SharedString>,
        checked: bool,
        action: Box<dyn Action>,
        disabled: bool,
    ) -> Self {
        self.add_menu_item(label, None, action, disabled, checked);
        self
    }

    /// 添加带自定义元素渲染的菜单项
    pub fn menu_element<F, E>(self, action: Box<dyn Action>, builder: F) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        self.menu_element_with_check(false, action, builder)
    }

    /// 添加带自定义元素渲染和禁用状态的菜单项
    pub fn menu_element_with_disabled<F, E>(
        self,
        action: Box<dyn Action>,
        disabled: bool,
        builder: F,
    ) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        self.menu_element_with_check_and_disabled(false, action, disabled, builder)
    }

    /// 添加带自定义元素渲染和图标的菜单项
    pub fn menu_element_with_icon<F, E>(
        self,
        icon: impl Into<Icon>,
        action: Box<dyn Action>,
        builder: F,
    ) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        self.menu_element_with_icon_and_disabled(icon, action, false, builder)
    }

    /// 添加带自定义元素渲染和选中状态的菜单项
    pub fn menu_element_with_check<F, E>(
        self,
        checked: bool,
        action: Box<dyn Action>,
        builder: F,
    ) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        self.menu_element_with_check_and_disabled(checked, action, false, builder)
    }

    /// 添加带自定义元素渲染、图标和禁用状态的菜单项
    fn menu_element_with_icon_and_disabled<F, E>(
        mut self,
        icon: impl Into<Icon>,
        action: Box<dyn Action>,
        disabled: bool,
        builder: F,
    ) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        self.menu_items.push(
            PopupMenuItem::element(builder)
                .action(action)
                .icon(icon)
                .disabled(disabled),
        );
        self
    }

    /// 添加带自定义元素渲染、选中状态和禁用状态的菜单项
    fn menu_element_with_check_and_disabled<F, E>(
        mut self,
        checked: bool,
        action: Box<dyn Action>,
        disabled: bool,
        builder: F,
    ) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        self.menu_items.push(
            PopupMenuItem::element(builder)
                .action(action)
                .checked(checked)
                .disabled(disabled),
        );
        self
    }

    /// 添加分隔符菜单项
    pub fn separator(mut self) -> Self {
        if self.menu_items.is_empty() {
            return self;
        }

        if let Some(PopupMenuItem::Separator) = self.menu_items.last() {
            return self;
        }

        self.menu_items.push(PopupMenuItem::separator());
        self
    }

    /// 添加子菜单
    pub fn submenu(
        self,
        label: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> Self {
        self.submenu_with_icon(None, label, window, cx, f)
    }

    /// 添加带图标的子菜单项
    pub fn submenu_with_icon(
        mut self,
        icon: Option<Icon>,
        label: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> Self {
        let submenu = PopupMenu::build(window, cx, f);
        let parent_menu = cx.entity().downgrade();
        let parent_priority = self.priority;
        submenu.update(cx, |view, _| {
            view.parent_menu = Some(parent_menu);
            view.priority = parent_priority + 1;
        });

        self.menu_items.push(
            PopupMenuItem::submenu(label, submenu).when_some(icon, |this, icon| this.icon(icon)),
        );
        self
    }

    /// 添加菜单项
    pub fn item(mut self, item: impl Into<PopupMenuItem>) -> Self {
        let item: PopupMenuItem = item.into();
        self.menu_items.push(item);
        self
    }

    /// 通过重新运行构建器来替换所有菜单项，保持其标识（焦点、父菜单、层级优先级）
    pub fn rebuild(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(Self, &mut Window, &mut Context<Self>) -> Self,
    ) {
        let mut menu = std::mem::replace(self, Self::new(cx));
        menu.menu_items.clear();
        menu.selected_index = None;
        *self = f(menu, window, cx);
        cx.notify();
    }

    fn add_menu_item(
        &mut self,
        label: impl Into<SharedString>,
        icon: Option<Icon>,
        action: Box<dyn Action>,
        disabled: bool,
        checked: bool,
    ) -> &mut Self {
        self.menu_items.push(
            PopupMenuItem::new(label)
                .when_some(icon, |item, icon| item.icon(icon))
                .disabled(disabled)
                .checked(checked)
                .action(action),
        );
        self
    }

    pub(super) fn with_menu_items<I>(
        mut self,
        items: impl IntoIterator<Item = I>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self
    where
        I: Into<OwnedMenuItem>,
    {
        for item in items {
            match item.into() {
                OwnedMenuItem::Action {
                    name,
                    action,
                    checked,
                    disabled,
                    ..
                } => {
                    self = self.menu_with_check_and_disabled(
                        name,
                        checked,
                        action.boxed_clone(),
                        disabled,
                    )
                }
                OwnedMenuItem::Separator => {
                    self = self.separator();
                }
                OwnedMenuItem::Submenu(submenu) => {
                    self = self.submenu(submenu.name, window, cx, move |menu, window, cx| {
                        menu.with_menu_items(submenu.items.clone(), window, cx)
                    })
                }
                OwnedMenuItem::SystemMenu(_) => {}
            }
        }

        if self.menu_items.len() > 20 {
            self.scrollable = true;
        }

        self
    }

    /// 返回激活的子菜单
    pub(crate) fn active_submenu(&self) -> Option<Entity<PopupMenu>> {
        if let Some(ix) = self.selected_index {
            if let Some(item) = self.menu_items.get(ix) {
                return match item {
                    PopupMenuItem::Submenu { menu, .. } => Some(menu.clone()),
                    _ => None,
                };
            }
        }

        None
    }

    /// 返回菜单是否为空
    pub fn is_empty(&self) -> bool {
        self.menu_items.is_empty()
    }

    fn clickable_menu_items(&self) -> impl Iterator<Item = (usize, &PopupMenuItem)> {
        self.menu_items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.is_clickable())
    }

    fn on_click(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        window.prevent_default();
        self.selected_index = Some(ix);
        self.confirm(&Confirm { secondary: false }, window, cx);
    }

    fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        match self.selected_index {
            Some(index) => {
                let item = self.menu_items.get(index);
                match item {
                    Some(PopupMenuItem::Item {
                        handler, action, ..
                    }) => {
                        if let Some(handler) = handler {
                            handler(&ClickEvent::default(), window, cx);
                        } else if let Some(action) = action.as_ref() {
                            self.dispatch_confirm_action(action, window, cx);
                        }

                        self.dismiss(&Cancel, window, cx)
                    }
                    Some(PopupMenuItem::ElementItem {
                        handler, action, ..
                    }) => {
                        if let Some(handler) = handler {
                            handler(&ClickEvent::default(), window, cx);
                        } else if let Some(action) = action.as_ref() {
                            self.dispatch_confirm_action(action, window, cx);
                        }
                        self.dismiss(&Cancel, window, cx)
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn dispatch_confirm_action(
        &self,
        action: &Box<dyn Action>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(context) = self.action_context.as_ref() {
            context.focus(window, cx);
        }

        window.dispatch_action(action.boxed_clone(), cx);
    }

    fn set_selected_index(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.selected_index != Some(ix) {
            self.selected_index = Some(ix);
            self.scroll_handle.scroll_to_item(ix);
            cx.notify();
        }
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let ix = self.selected_index.unwrap_or(0);

        if let Some((prev_ix, _)) = self
            .menu_items
            .iter()
            .enumerate()
            .rev()
            .find(|(i, item)| *i < ix && item.is_clickable())
        {
            self.set_selected_index(prev_ix, cx);
            return;
        }

        let last_clickable_ix = self.clickable_menu_items().last().map(|(ix, _)| ix);
        self.set_selected_index(last_clickable_ix.unwrap_or(0), cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let Some(ix) = self.selected_index else {
            self.set_selected_index(0, cx);
            return;
        };

        if let Some((next_ix, _)) = self
            .menu_items
            .iter()
            .enumerate()
            .find(|(i, item)| *i > ix && item.is_clickable())
        {
            self.set_selected_index(next_ix, cx);
            return;
        }

        self.set_selected_index(0, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, window: &mut Window, cx: &mut Context<Self>) {
        let handled = if matches!(self.submenu_anchor.0, Anchor::TopLeft | Anchor::BottomLeft) {
            self._unselect_submenu(window, cx)
        } else {
            self._select_submenu(window, cx)
        };

        if self.parent_side(cx).is_left() {
            self._focus_parent_menu(window, cx);
        }

        if handled {
            return;
        }

        // 让父 AppMenuBar 处理
        if self.parent_menu.is_none() {
            cx.propagate();
        }
    }

    fn select_right(&mut self, _: &SelectRight, window: &mut Window, cx: &mut Context<Self>) {
        let handled = if matches!(self.submenu_anchor.0, Anchor::TopLeft | Anchor::BottomLeft) {
            self._select_submenu(window, cx)
        } else {
            self._unselect_submenu(window, cx)
        };

        if self.parent_side(cx).is_right() {
            self._focus_parent_menu(window, cx);
        }

        if handled {
            return;
        }

        // 让父 AppMenuBar 处理
        if self.parent_menu.is_none() {
            cx.propagate();
        }
    }

    fn _select_submenu(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Some(active_submenu) = self.active_submenu() {
            // 聚焦子菜单，以便其处理动作
            active_submenu.update(cx, |view, cx| {
                view.set_selected_index(0, cx);
                view.focus_handle.focus(window, cx);
            });
            cx.notify();
            return true;
        }

        return false;
    }

    fn _unselect_submenu(&mut self, _: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Some(active_submenu) = self.active_submenu() {
            active_submenu.update(cx, |view, cx| {
                view.selected_index = None;
                cx.notify();
            });
            return true;
        }

        return false;
    }

    fn _focus_parent_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(parent) = self.parent_menu.as_ref() else {
            return;
        };
        let Some(parent) = parent.upgrade() else {
            return;
        };

        self.selected_index = None;
        parent.update(cx, |view, cx| {
            view.focus_handle.focus(window, cx);
            cx.notify();
        });
    }

    fn parent_side(&self, cx: &App) -> Side {
        let Some(parent) = self.parent_menu.as_ref() else {
            return Side::Left;
        };

        let Some(parent) = parent.upgrade() else {
            return Side::Left;
        };

        match parent.read(cx).submenu_anchor.0 {
            Anchor::TopLeft | Anchor::BottomLeft => Side::Left,
            Anchor::TopRight | Anchor::BottomRight => Side::Right,
            // 居中锚点不用于子菜单定位，但必须覆盖它们
            _ => Side::Left,
        }
    }

    fn dismiss(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_submenu().is_some() {
            return;
        }

        cx.emit(DismissEvent);

        // 聚焦回之前的焦点句柄
        if let Some(handle) = self
            .previous_focus_handle
            .as_ref()
            .or(self.action_context.as_ref())
        {
            window.focus(handle, cx);
        }

        let Some(parent_menu) = self.parent_menu.clone() else {
            return;
        };

        // 此菜单关闭时，同时关闭父菜单
        _ = parent_menu.update(cx, |view, cx| {
            view.selected_index = None;
            view.dismiss(&Cancel, window, cx);
        });
    }

    fn handle_dismiss(
        &mut self,
        position: &Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 如果点击在父菜单内部，则不关闭
        if let Some(parent) = self.parent_menu.as_ref() {
            if let Some(parent) = parent.upgrade() {
                if parent.read(cx).bounds.contains(position) {
                    return;
                }
            }
        }

        self.dismiss(&Cancel, window, cx);
    }

    fn on_mouse_down_out(
        &mut self,
        e: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.handle_dismiss(&e.position, window, cx);
    }

    fn render_key_binding(
        &self,
        action: Option<Box<dyn Action>>,
        window: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Kbd> {
        let action = action?;

        match self
            .action_context
            .as_ref()
            .or(self.previous_focus_handle.as_ref())
            .and_then(|handle| Kbd::binding_for_action_in(action.as_ref(), handle, window))
        {
            Some(kbd) => Some(kbd),
            // 回退到 App 级别键绑定
            None => Kbd::binding_for_action(action.as_ref(), None, window),
        }
        .map(|this| {
            this.p_0()
                .flex_nowrap()
                .border_0()
                .bg(crate::transparent_white())
        })
    }

    fn render_icon(
        has_icon: bool,
        checked: bool,
        icon: Option<Icon>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        if !has_icon {
            return None;
        }

        let icon = if let Some(icon) = icon {
            icon.clone()
        } else if checked {
            Icon::new(IconName::Check)
        } else {
            Icon::empty()
        };

        Some(icon.xsmall())
    }

    #[inline]
    fn max_width(&self) -> Pixels {
        self.max_width.unwrap_or(px(500.))
    }

    /// 计算子菜单的锚点角与左偏移
    fn update_submenu_menu_anchor(&mut self, window: &Window) {
        let bounds = self.bounds;
        let max_width = self.max_width();
        let (anchor, left) = if max_width + bounds.origin.x > window.bounds().size.width {
            (Anchor::TopRight, -px(16.))
        } else {
            (Anchor::TopLeft, bounds.size.width - px(8.))
        };

        let is_bottom_pos = bounds.origin.y + bounds.size.height > window.bounds().size.height;
        self.submenu_anchor = if is_bottom_pos {
            (anchor.other_side_along(crate::Axis::Vertical), left)
        } else {
            (anchor, left)
        };
    }

    fn render_item(
        &self,
        ix: usize,
        item: &PopupMenuItem,
        options: RenderOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> MenuItemElement {
        let has_left_icon = options.has_left_icon;
        let is_left_check = options.check_side.is_left() && item.is_checked();
        let right_check_icon = if options.check_side.is_right() && item.is_checked() {
            Some(Icon::new(IconName::Check).xsmall())
        } else {
            None
        };

        let selected = self.selected_index == Some(ix);
        const EDGE_PADDING: Pixels = px(4.);
        const INNER_PADDING: Pixels = px(8.);

        let is_submenu = matches!(item, PopupMenuItem::Submenu { .. });
        let group_name = format!("{}:item-{}", cx.entity().entity_id(), ix);

        let (item_height, radius) = match self.size {
            ElementSize::Small => (px(20.), options.radius.half()),
            _ => (px(26.), options.radius),
        };

        let this = MenuItemElement::new(ix, &group_name)
            .relative()
            .text_sm()
            .py_0()
            .px(INNER_PADDING)
            .rounded(radius)
            .items_center()
            .selected(selected)
            .on_hover(cx.listener(move |this, hovered, _, cx| {
                if *hovered {
                    this.selected_index = Some(ix);
                } else if !is_submenu && this.selected_index == Some(ix) {
                    // TODO: 更好地处理悬停移出子菜单时的取消选中
                    this.selected_index = None;
                }

                cx.notify();
            }))
            .when_some(item.a11y_label(), |this, label| this.aria_label(label));

        match item {
            PopupMenuItem::Separator => this
                .h_auto()
                .p_0()
                .my_0p5()
                .mx_neg_1()
                .border_b(px(2.))
                .border_color(cx.theme().border)
                .disabled(true),
            PopupMenuItem::Label(label) => this.disabled(true).cursor_default().child(
                h_flex()
                    .cursor_default()
                    .items_center()
                    .gap_x_1()
                    .children(Self::render_icon(has_left_icon, false, None, window, cx))
                    .child(div().flex_1().child(label.clone())),
            ),
            PopupMenuItem::ElementItem {
                render,
                icon,
                disabled,
                ..
            } => this
                .when(!disabled, |this| {
                    this.on_click(
                        cx.listener(move |this, _, window, cx| this.on_click(ix, window, cx)),
                    )
                })
                .disabled(*disabled)
                .child(
                    h_flex()
                        .flex_1()
                        .min_h(item_height)
                        .items_center()
                        .gap_x_1()
                        .children(Self::render_icon(
                            has_left_icon,
                            is_left_check,
                            icon.clone(),
                            window,
                            cx,
                        ))
                        .child((render)(window, cx))
                        .children(right_check_icon.map(|icon| icon.ml_3())),
                ),
            PopupMenuItem::Item {
                icon,
                label,
                action,
                disabled,
                is_link,
                ..
            } => {
                let show_link_icon = *is_link && self.external_link_icon;
                let action = action.as_ref().map(|action| action.boxed_clone());
                let key = self.render_key_binding(action, window, cx);

                this.when(!disabled, |this| {
                    this.on_click(
                        cx.listener(move |this, _, window, cx| this.on_click(ix, window, cx)),
                    )
                })
                .disabled(*disabled)
                .h(item_height)
                .gap_x_1()
                .children(Self::render_icon(
                    has_left_icon,
                    is_left_check,
                    icon.clone(),
                    window,
                    cx,
                ))
                .child(
                    h_flex()
                        .w_full()
                        .gap_3()
                        .items_center()
                        .justify_between()
                        .when(!show_link_icon, |this| this.child(label.clone()))
                        .children(right_check_icon)
                        .when(show_link_icon, |this| {
                            this.child(
                                h_flex()
                                    .w_full()
                                    .justify_between()
                                    .gap_1p5()
                                    .child(label.clone())
                                    .child(
                                        Icon::new(IconName::ExternalLink)
                                            .xsmall()
                                            .text_color(cx.theme().muted_foreground),
                                    ),
                            )
                        })
                        .children(key),
                )
            }
            PopupMenuItem::Submenu {
                icon,
                label,
                menu,
                disabled,
            } => this
                .selected(selected)
                .disabled(*disabled)
                .items_start()
                .child(
                    h_flex()
                        .min_h(item_height)
                        .size_full()
                        .items_center()
                        .gap_x_1()
                        .children(Self::render_icon(
                            has_left_icon,
                            false,
                            icon.clone(),
                            window,
                            cx,
                        ))
                        .child(
                            h_flex()
                                .flex_1()
                                .gap_2()
                                .items_center()
                                .justify_between()
                                .child(label.clone())
                                .child(
                                    Icon::new(IconName::ChevronRight)
                                        .xsmall()
                                        .text_color(cx.theme().muted_foreground),
                                ),
                        ),
                )
                .when(selected, |this| {
                    this.child({
                        let (anchor, left) = self.submenu_anchor;
                        let is_bottom_pos =
                            matches!(anchor, Anchor::BottomLeft | Anchor::BottomRight);
                        deferred(
                            anchored()
                                .anchor(anchor)
                                .child(
                                    div()
                                        .id("submenu")
                                        .occlude()
                                        .when(is_bottom_pos, |this| this.bottom_0())
                                        .when(!is_bottom_pos, |this| this.top_neg_1())
                                        .left(left)
                                        .child(menu.clone()),
                                )
                                .snap_to_window_with_margin(Edges::all(EDGE_PADDING)),
                        )
                        .with_priority(self.priority + 1)
                    })
                }),
        }
    }
}

impl FluentBuilder for PopupMenu {}
impl EventEmitter<DismissEvent> for PopupMenu {}
impl Focusable for PopupMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[derive(Clone, Copy)]
struct RenderOptions {
    has_left_icon: bool,
    check_side: Side,
    radius: Pixels,
}

impl Render for PopupMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.update_submenu_menu_anchor(window);

        // 通过公共 `item()` + `PopupMenuItem::submenu()` 路径附加的子菜单
        // （来自只有菜单值的上下文，例如表委托的 `context_menu`）在构建时没有
        // 关联父菜单。在这里关联它们，使关闭链、外部点击检查和键盘导航
        // 与 `submenu()` 构建的子菜单保持一致。
        let parent = cx.entity().downgrade();
        let parent_priority = self.priority;
        for item in &self.menu_items {
            if let PopupMenuItem::Submenu { menu, .. } = item {
                if menu.read(cx).parent_menu.is_none() {
                    menu.update(cx, |menu, _| {
                        menu.parent_menu = Some(parent.clone());
                        menu.priority = parent_priority + 1;
                    });
                }
            }
        }

        let view = cx.entity().clone();
        let items_count = self.menu_items.len();

        let max_height = self.max_height.unwrap_or_else(|| {
            let window_half_height = window.window_bounds().get_bounds().size.height * 0.5;
            window_half_height.min(px(450.))
        });

        let has_left_icon = self
            .menu_items
            .iter()
            .any(|item| item.has_left_icon(self.check_side));

        let max_width = self.max_width();
        let options = RenderOptions {
            has_left_icon,
            check_side: self.check_side,
            radius: cx.theme().radius.min(px(8.)),
        };

        v_flex()
            .id("popup-menu")
            .role(Role::Menu)
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::dismiss))
            .on_mouse_down_out(cx.listener(Self::on_mouse_down_out))
            .popover_style(cx)
            .text_color(cx.theme().popover_foreground)
            .relative()
            .occlude()
            .child(
                v_flex()
                    .id("items")
                    .p_1()
                    .gap_y_0p5()
                    .min_w(rems(8.))
                    .when_some(self.min_width, |this, min_width| this.min_w(min_width))
                    .max_w(max_width)
                    .when(self.scrollable, |this| {
                        this.max_h(max_height)
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                    })
                    .children(
                        self.menu_items
                            .iter()
                            .enumerate()
                            // 忽略最后一个分隔符
                            .filter(|(ix, item)| !(*ix + 1 == items_count && item.is_separator()))
                            .map(|(ix, item)| self.render_item(ix, item, options, window, cx)),
                    )
                    .on_prepaint(move |bounds, _, cx| view.update(cx, |r, _| r.bounds = bounds)),
            )
            .when(self.scrollable, |this| {
                // TODO: 当菜单受 `overflow_y_scroll` 限制时，子菜单将无法显示
                this.vertical_scrollbar(&self.scroll_handle)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rgpui::test]
    fn popup_menu_item_a11y_label_uses_visible_label(cx: &mut rgpui::TestAppContext) {
        let submenu = cx.update(|cx| cx.new(|cx| PopupMenu::new(cx)));

        assert_eq!(PopupMenuItem::new("Open").a11y_label(), Some("Open".into()));
        assert_eq!(
            PopupMenuItem::link("Docs", "https://example.com").a11y_label(),
            Some("Docs".into())
        );
        assert_eq!(
            PopupMenuItem::label("Recent files").a11y_label(),
            Some("Recent files".into())
        );
        assert_eq!(
            PopupMenuItem::submenu("More", submenu).a11y_label(),
            Some("More".into())
        );
        assert_eq!(PopupMenuItem::separator().a11y_label(), None);
        assert_eq!(PopupMenuItem::element(|_, _| div()).a11y_label(), None);
    }
}
