//! 原生应用菜单栏构建器，用于桌面应用设置系统菜单。

use rgpui::{Menu, MenuItem, SharedString, StyleRefinement, Styled, SystemMenuType};

/// 应用菜单栏：包含多个菜单的构建器。
pub struct AppMenuBar {
    /// 菜单列表。
    menus: Vec<Menu>,
}

impl AppMenuBar {
    /// 创建菜单栏构建器。
    pub fn new() -> Self {
        Self { menus: Vec::new() }
    }

    /// 添加一个菜单。
    pub fn menu(mut self, menu: AppMenu) -> Self {
        self.menus.push(menu.build());
        self
    }

    /// 构建菜单列表。
    pub fn build(self) -> Vec<Menu> {
        self.menus
    }
}

impl Default for AppMenuBar {
    fn default() -> Self {
        Self::new()
    }
}

/// 应用菜单：单个菜单的构建器。
pub struct AppMenu {
    /// 菜单名称。
    name: SharedString,
    /// 菜单项列表。
    items: Vec<MenuItem>,
    /// 用户样式。
    style: StyleRefinement,
}

impl AppMenu {
    /// 创建菜单构建器。
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            items: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// 添加一个执行操作的菜单项。
    pub fn action<A: rgpui::Action>(mut self, label: impl Into<SharedString>, action: A) -> Self {
        self.items.push(MenuItem::action(label.into(), action));
        self
    }

    /// 添加分隔符。
    pub fn separator(mut self) -> Self {
        self.items.push(MenuItem::separator());
        self
    }

    /// 添加子菜单。
    pub fn submenu(mut self, submenu: AppMenu) -> Self {
        let submenu_built = submenu.build();
        self.items.push(MenuItem::submenu(submenu_built));
        self
    }

    /// 添加由操作系统管理的子菜单。
    pub fn os_submenu(mut self, label: impl Into<SharedString>, menu_type: SystemMenuType) -> Self {
        self.items
            .push(MenuItem::os_submenu(label.into(), menu_type));
        self
    }

    /// 构建菜单。
    pub fn build(self) -> Menu {
        Menu {
            name: self.name,
            items: self.items,
            disabled: false,
        }
    }
}

impl Styled for AppMenu {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// 创建"文件"菜单。
pub fn file_menu() -> AppMenu {
    AppMenu::new("File")
}

/// 创建"编辑"菜单。
pub fn edit_menu() -> AppMenu {
    AppMenu::new("Edit")
}

/// 创建"视图"菜单。
pub fn view_menu() -> AppMenu {
    AppMenu::new("View")
}

/// 创建"窗口"菜单。
pub fn window_menu() -> AppMenu {
    AppMenu::new("Window")
}

/// 创建"帮助"菜单。
pub fn help_menu() -> AppMenu {
    AppMenu::new("Help")
}

/// 标准 macOS 菜单栏：带应用菜单与常见系统菜单。
pub struct StandardMacMenuBar {
    /// 应用名称。
    _app_name: SharedString,
    /// 文件菜单。
    file_menu: Option<AppMenu>,
    /// 编辑菜单。
    edit_menu: Option<AppMenu>,
    /// 视图菜单。
    view_menu: Option<AppMenu>,
    /// 窗口菜单。
    window_menu: Option<AppMenu>,
    /// 帮助菜单。
    help_menu: Option<AppMenu>,
}

impl StandardMacMenuBar {
    /// 创建标准 macOS 菜单栏构建器。
    pub fn new(app_name: impl Into<SharedString>) -> Self {
        Self {
            _app_name: app_name.into(),
            file_menu: None,
            edit_menu: None,
            view_menu: None,
            window_menu: None,
            help_menu: None,
        }
    }

    /// 设置文件菜单。
    pub fn file_menu(mut self, menu: AppMenu) -> Self {
        self.file_menu = Some(menu);
        self
    }

    /// 设置编辑菜单。
    pub fn edit_menu(mut self, menu: AppMenu) -> Self {
        self.edit_menu = Some(menu);
        self
    }

    /// 设置视图菜单。
    pub fn view_menu(mut self, menu: AppMenu) -> Self {
        self.view_menu = Some(menu);
        self
    }

    /// 设置窗口菜单。
    pub fn window_menu(mut self, menu: AppMenu) -> Self {
        self.window_menu = Some(menu);
        self
    }

    /// 设置帮助菜单。
    pub fn help_menu(mut self, menu: AppMenu) -> Self {
        self.help_menu = Some(menu);
        self
    }

    /// 构建菜单列表。
    pub fn build(self) -> Vec<Menu> {
        let mut menus = Vec::new();

        #[cfg(target_os = "macos")]
        {
            let app_menu = AppMenu::new(&self._app_name)
                .os_submenu("Services", SystemMenuType::Services)
                .separator();
            menus.push(app_menu.build());
        }

        if let Some(file_menu) = self.file_menu {
            menus.push(file_menu.build());
        }

        if let Some(edit_menu) = self.edit_menu {
            menus.push(edit_menu.build());
        }

        if let Some(view_menu) = self.view_menu {
            menus.push(view_menu.build());
        }

        if let Some(window_menu) = self.window_menu {
            menus.push(window_menu.build());
        }

        if let Some(help_menu) = self.help_menu {
            menus.push(help_menu.build());
        }

        menus
    }
}
