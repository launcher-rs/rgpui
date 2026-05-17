use crate::{Action, App, Platform, SharedString};

/// 应用程序菜单，可以是主菜单或子菜单
pub struct Menu {
    /// 菜单名称
    pub name: SharedString,

    /// 菜单项列表
    pub items: Vec<MenuItem>,

    /// 此菜单是否被禁用
    pub disabled: bool,
}

impl Menu {
    /// 使用给定名称创建新菜单
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            items: vec![],
            disabled: false,
        }
    }

    /// 设置菜单项
    pub fn items(mut self, items: impl IntoIterator<Item = MenuItem>) -> Self {
        self.items = items.into_iter().collect();
        self
    }

    /// 设置此菜单是否被禁用
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 从此菜单创建 OwnedMenu
    pub fn owned(self) -> OwnedMenu {
        OwnedMenu {
            name: self.name.to_string().into(),
            items: self.items.into_iter().map(|item| item.owned()).collect(),
            disabled: self.disabled,
        }
    }
}

/// 操作系统菜单，由操作系统识别
/// 这允许操作系统为这些菜单提供专门的菜单项
pub struct OsMenu {
    /// 菜单名称
    pub name: SharedString,

    /// 菜单类型
    pub menu_type: SystemMenuType,
}

impl OsMenu {
    /// 从此 OsMenu 创建 OwnedOsMenu
    pub fn owned(self) -> OwnedOsMenu {
        OwnedOsMenu {
            name: self.name.to_string().into(),
            menu_type: self.menu_type,
        }
    }
}

/// 系统菜单类型
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum SystemMenuType {
    /// macOS 应用程序菜单中的"服务"菜单
    Services,
}

/// 菜单中可以的不同类型的菜单项
pub enum MenuItem {
    /// 项目之间的分隔符
    Separator,

    /// 子菜单
    Submenu(Menu),

    /// 由系统管理的菜单（例如 macOS 上的服务菜单）
    SystemMenu(OsMenu),

    /// 可执行的操作
    Action {
        /// 此菜单项的名称
        name: SharedString,

        /// 选择此菜单项时执行的操作
        action: Box<dyn Action>,

        /// 与此操作对应的操作系统操作（如果有）
        /// 有关更多信息，请参见 [`OsAction`]
        os_action: Option<OsAction>,

        /// 此操作是否被选中
        checked: bool,

        /// 此操作是否被禁用
        disabled: bool,
    },
}

impl MenuItem {
    /// 创建分隔符菜单项
    pub fn separator() -> Self {
        Self::Separator
    }

    /// 创建子菜单菜单项
    pub fn submenu(menu: Menu) -> Self {
        Self::Submenu(menu)
    }

    /// 创建由操作系统填充的子菜单
    pub fn os_submenu(name: impl Into<SharedString>, menu_type: SystemMenuType) -> Self {
        Self::SystemMenu(OsMenu {
            name: name.into(),
            menu_type,
        })
    }

    /// 创建调用操作的菜单项
    pub fn action(name: impl Into<SharedString>, action: impl Action) -> Self {
        Self::Action {
            name: name.into(),
            action: Box::new(action),
            os_action: None,
            checked: false,
            disabled: false,
        }
    }

    /// 创建调用操作并带有操作系统操作的菜单项
    pub fn os_action(
        name: impl Into<SharedString>,
        action: impl Action,
        os_action: OsAction,
    ) -> Self {
        Self::Action {
            name: name.into(),
            action: Box::new(action),
            os_action: Some(os_action),
            checked: false,
            disabled: false,
        }
    }

    /// 从此 MenuItem 创建 OwnedMenuItem
    pub fn owned(self) -> OwnedMenuItem {
        match self {
            MenuItem::Separator => OwnedMenuItem::Separator,
            MenuItem::Submenu(submenu) => OwnedMenuItem::Submenu(submenu.owned()),
            MenuItem::Action {
                name,
                action,
                os_action,
                checked,
                disabled,
            } => OwnedMenuItem::Action {
                name: name.into(),
                action,
                os_action,
                checked,
                disabled,
            },
            MenuItem::SystemMenu(os_menu) => OwnedMenuItem::SystemMenu(os_menu.owned()),
        }
    }

    /// 设置此菜单项是否被选中
    ///
    /// 仅适用于 [`MenuItem::Action`]，否则将被忽略
    pub fn checked(mut self, checked: bool) -> Self {
        match &mut self {
            MenuItem::Action { checked: old, .. } => {
                *old = checked;
            }
            _ => {}
        }
        self
    }

    /// 返回此菜单项是否被选中
    ///
    /// 仅适用于 [`MenuItem::Action`]，否则返回 false
    #[inline]
    pub fn is_checked(&self) -> bool {
        match self {
            MenuItem::Action { checked, .. } => *checked,
            _ => false,
        }
    }

    /// 设置此菜单项是否被禁用
    pub fn disabled(mut self, disabled: bool) -> Self {
        match &mut self {
            MenuItem::Action { disabled: old, .. } => {
                *old = disabled;
            }
            MenuItem::Submenu(submenu) => {
                submenu.disabled = disabled;
            }
            _ => {}
        }
        self
    }

    /// 返回此菜单项是否被禁用
    ///
    /// 仅适用于 [`MenuItem::Action`] 和 [`MenuItem::Submenu`]，否则返回 false
    #[inline]
    pub fn is_disabled(&self) -> bool {
        match self {
            MenuItem::Action { disabled, .. } => *disabled,
            MenuItem::Submenu(submenu) => submenu.disabled,
            _ => false,
        }
    }
}

/// 操作系统菜单，由操作系统识别
/// 这允许操作系统为这些菜单提供专门的行为
#[derive(Clone)]
pub struct OwnedOsMenu {
    /// 菜单名称
    pub name: SharedString,

    /// 菜单类型
    pub menu_type: SystemMenuType,
}

/// 应用程序菜单，可以是主菜单或子菜单
#[derive(Clone)]
pub struct OwnedMenu {
    /// 菜单名称
    pub name: SharedString,

    /// 菜单项列表
    pub items: Vec<OwnedMenuItem>,

    /// 此菜单是否被禁用
    pub disabled: bool,
}

/// 菜单中可以的不同类型的菜单项
pub enum OwnedMenuItem {
    /// 项目之间的分隔符
    Separator,

    /// 子菜单
    Submenu(OwnedMenu),

    /// 由系统管理的菜单（例如 macOS 上的服务菜单）
    SystemMenu(OwnedOsMenu),

    /// 可执行的操作
    Action {
        /// 此菜单项的名称
        name: String,

        /// 选择此菜单项时执行的操作
        action: Box<dyn Action>,

        /// 与此操作对应的操作系统操作（如果有）
        /// 有关更多信息，请参见 [`OsAction`]
        os_action: Option<OsAction>,

        /// 此操作是否被选中
        checked: bool,

        /// 此操作是否被禁用
        disabled: bool,
    },
}

impl Clone for OwnedMenuItem {
    fn clone(&self) -> Self {
        match self {
            OwnedMenuItem::Separator => OwnedMenuItem::Separator,
            OwnedMenuItem::Submenu(submenu) => OwnedMenuItem::Submenu(submenu.clone()),
            OwnedMenuItem::Action {
                name,
                action,
                os_action,
                checked,
                disabled,
            } => OwnedMenuItem::Action {
                name: name.clone(),
                action: action.boxed_clone(),
                os_action: *os_action,
                checked: *checked,
                disabled: *disabled,
            },
            OwnedMenuItem::SystemMenu(os_menu) => OwnedMenuItem::SystemMenu(os_menu.clone()),
        }
    }
}

// TODO: 作为全局选择重构的一部分，这些应该
// 移动到 GPUI 提供的操作中，在不向 GPUI 用户泄露平台细节的情况下建立此关联

/// 操作系统操作，由操作系统识别
/// 这允许操作系统为这些操作提供专门的行为
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum OsAction {
    /// "剪切"操作
    Cut,

    /// "复制"操作
    Copy,

    /// "粘贴"操作
    Paste,

    /// "全选"操作
    SelectAll,

    /// "撤销"操作
    Undo,

    /// "重做"操作
    Redo,
}

pub(crate) fn init_app_menus(platform: &dyn Platform, cx: &App) {
    platform.on_will_open_app_menu(Box::new({
        let cx = cx.to_async();
        move || {
            if let Some(app) = cx.app.upgrade() {
                app.borrow_mut().update(|cx| cx.clear_pending_keystrokes());
            }
        }
    }));

    platform.on_validate_app_menu_command(Box::new({
        let cx = cx.to_async();
        move |action| {
            cx.app
                .upgrade()
                .map(|app| app.borrow_mut().update(|cx| cx.is_action_available(action)))
                .unwrap_or(false)
        }
    }));

    platform.on_app_menu_action(Box::new({
        let cx = cx.to_async();
        move |action| {
            if let Some(app) = cx.app.upgrade() {
                app.borrow_mut().update(|cx| cx.dispatch_action(action));
            }
        }
    }));
}

#[cfg(test)]
mod tests {
    use crate::Menu;

    #[test]
    fn test_menu() {
        let menu = Menu::new("App")
            .items(vec![
                crate::MenuItem::action("Action 1", rgpui::NoAction),
                crate::MenuItem::separator(),
            ])
            .disabled(true);

        assert_eq!(menu.name.as_ref(), "App");
        assert_eq!(menu.items.len(), 2);
        assert!(menu.disabled);
    }

    #[test]
    fn test_menu_item_builder() {
        use super::MenuItem;

        let item = MenuItem::action("Test Action", rgpui::NoAction);
        assert_eq!(
            match &item {
                MenuItem::Action { name, .. } => name.as_ref(),
                _ => unreachable!(),
            },
            "Test Action"
        );
        assert!(matches!(
            item,
            MenuItem::Action {
                checked: false,
                disabled: false,
                ..
            }
        ));

        assert!(
            MenuItem::action("Test Action", rgpui::NoAction)
                .checked(true)
                .is_checked()
        );
        assert!(
            MenuItem::action("Test Action", rgpui::NoAction)
                .disabled(true)
                .is_disabled()
        );

        let submenu = MenuItem::submenu(super::Menu {
            name: "Submenu".into(),
            items: vec![],
            disabled: true,
        });
        assert_eq!(
            match &submenu {
                MenuItem::Submenu(menu) => menu.name.as_ref(),
                _ => unreachable!(),
            },
            "Submenu"
        );
        assert!(!submenu.is_checked());
        assert!(submenu.is_disabled());
    }
}
