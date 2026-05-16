use crate::{App, Image, MenuItem, SharedString, SvgRenderer};
use anyhow::Result;
use std::rc::Rc;

/// 系统托盘图标事件类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayIconEvent {
    /// 用户左键点击托盘图标
    LeftClick,
    /// 用户右键点击托盘图标
    RightClick,
    /// 用户双击托盘图标
    DoubleClick,
}

/// 系统托盘菜单项类型
#[derive(Debug, Clone)]
pub enum TrayMenuItem {
    /// 可点击的操作项
    Action {
        /// 显示标签
        label: SharedString,
        /// 此操作的唯一标识符
        id: SharedString,
    },
    /// 菜单项之间的分隔线
    Separator,
    /// 包含嵌套项的子菜单
    Submenu {
        /// 显示标签
        label: SharedString,
        /// 嵌套的菜单项
        items: Vec<TrayMenuItem>,
    },
    /// 可切换的菜单项（带复选标记）
    Toggle {
        /// 显示标签
        label: SharedString,
        /// 当前是否选中
        checked: bool,
        /// 此切换项的唯一标识符
        id: SharedString,
    },
}

/// 系统托盘图标
#[derive(Clone)]
pub struct Tray {
    /// 工具提示文本
    pub tooltip: Option<SharedString>,
    /// 托盘图标图像
    pub icon: Option<Rc<Image>>,
    /// 渲染后的图标数据，供平台使用
    pub icon_data: Option<TrayIconData>,
    /// 构建上下文菜单的函数
    pub menu_builder: Option<Rc<dyn Fn(&mut App) -> Vec<MenuItem>>>,
    /// 托盘图标的可见性
    pub visible: bool,
}

impl Tray {
    /// 将图标渲染为平台兼容的图标数据
    pub fn render_icon(&mut self, svg_renderer: SvgRenderer) -> Result<()> {
        if let Some(icon) = &self.icon {
            let image = icon.to_image_data(svg_renderer)?;
            let bytes = image.as_bytes(0).unwrap_or_default();
            let size = image.size(0);

            self.icon_data = Some(TrayIconData {
                data: Rc::new(bytes.to_vec()),
                width: size.width.0 as u32,
                height: size.height.0 as u32,
            })
        }
        Ok(())
    }
}

/// 渲染后的图标数据，供平台使用
#[derive(Clone)]
pub struct TrayIconData {
    /// 原始 RGBA 图像数据
    pub data: Rc<Vec<u8>>,
    /// 图标宽度（像素）
    pub width: u32,
    /// 图标高度（像素）
    pub height: u32,
}

impl Tray {
    /// 创建一个新的托盘图标，使用默认属性
    pub fn new() -> Self {
        Self {
            tooltip: None,
            icon: None,
            icon_data: None,
            menu_builder: None,
            visible: true,
        }
    }

    /// 设置工具提示文本，默认为 None
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// 设置图标图像，默认为 None
    pub fn icon(mut self, icon: impl Into<Image>) -> Self {
        self.icon = Some(Rc::new(icon.into()));
        self
    }

    /// 设置上下文菜单
    pub fn menu<F>(mut self, builder: F) -> Self
    where
        F: Fn(&mut App) -> Vec<MenuItem> + 'static,
    {
        self.menu_builder = Some(Rc::new(builder));
        self
    }

    /// 设置托盘图标的可见性，默认为 true
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
}
