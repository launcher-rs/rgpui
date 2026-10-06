//! Linux 托盘：StatusNotifierItem 的数据层与主线程事件通道。
//!
//! Linux 桌面通过 session bus 上的 freedesktop StatusNotifierItem（SNI）协议
//! 与 `com.canonical.dbusmenu` 菜单协议暴露托盘图标。本模块只负责主线程侧的
//! 共享状态、数据转换与事件通道；DBus 服务端在 [`tray_sni`] 中实现。

use crate::linux::LinuxCommon;
use calloop::channel::Channel;
use calloop::{EventSource, Poll, PostAction, Readiness, Token, TokenFactory};
use futures::channel::mpsc;
use image::RgbaImage;
use parking_lot::Mutex;
use rgpui::{MenuItem, SharedString, TrayIconEvent, TrayMenuItem};
use std::sync::Arc;

/// SNI `IconPixmap` 属性：每项为 `(宽, 高, ARGB32 大端字节)`
pub(crate) type IconPixmap = Vec<(i32, i32, Vec<u8>)>;

/// SNI `ToolTip` 属性：`(图标名, 图标 pixmap, 标题, 描述)`
pub(crate) type ToolTip = (String, IconPixmap, String, String);

/// 托盘图标的首选边长（像素）。
/// 主机的选择逻辑是「取不小于所需尺寸的最小项」，因此同时提供常规与 HiDPI 两档，
/// 绝不直接发送数百像素的原图。
const ICON_SIZES: [u32; 2] = [22, 44];

/// dbusmenu 菜单项类型字段取值
pub(crate) const MENU_TYPE_STANDARD: &str = "standard";
/// dbusmenu 分隔线类型
pub(crate) const MENU_TYPE_SEPARATOR: &str = "separator";
/// dbusmenu 子菜单类型
pub(crate) const MENU_TYPE_SUBMENU: &str = "submenu";

/// 托盘事件：由 DBus 服务线程产生，投递到主线程处理
pub(crate) enum TrayEvent {
    /// 托盘图标被交互（双击等）
    Icon(TrayIconEvent),
    /// 托盘菜单项被点击，携带应用侧的菜单项标识
    MenuAction(SharedString),
}

/// 派发托盘事件到用户回调
///
/// 回调里常会调用 `quit`、`activate_window` 等再次借用客户端状态的 API，所以必须
/// 先把回调从 [`LinuxCommon`] 取出并释放借用，执行完再放回。`with_common` 由调用方
/// 提供，只允许短暂借用状态。
pub(crate) fn dispatch_tray_event(
    event: TrayEvent,
    with_common: &mut dyn FnMut(&mut dyn FnMut(&mut LinuxCommon)),
) {
    let mut icon_callback: Option<Box<dyn FnMut(TrayIconEvent)>> = None;
    let mut menu_callback: Option<Box<dyn FnMut(SharedString)>> = None;
    with_common(&mut |common| {
        icon_callback = common.callbacks.tray_icon_event.take();
        menu_callback = common.callbacks.tray_menu_action.take();
    });

    match event {
        TrayEvent::Icon(icon_event) => {
            if let Some(callback) = icon_callback.as_mut() {
                callback(icon_event);
            }
        }
        TrayEvent::MenuAction(id) => {
            if let Some(callback) = menu_callback.as_mut() {
                callback(id);
            }
        }
    }

    with_common(&mut |common| {
        common.callbacks.tray_icon_event = icon_callback.take();
        common.callbacks.tray_menu_action = menu_callback.take();
    });
}

/// 托盘命令：由主线程发出，通知 DBus 服务线程状态已变化
pub(crate) enum TrayCommand {
    /// 图标、工具提示等 SNI 属性已更新
    PropertiesChanged,
    /// dbusmenu 菜单布局已更新
    MenuChanged,
}

/// dbusmenu 菜单节点
#[derive(Clone)]
pub(crate) struct MenuNode {
    /// dbusmenu 内部数字标识，主机用它引用菜单项；根节点固定为 0
    pub(crate) id: i32,
    /// 菜单项类型：standard / separator / submenu
    pub(crate) type_: &'static str,
    /// 显示文本
    pub(crate) label: SharedString,
    /// 是否可点击
    pub(crate) enabled: bool,
    /// 是否可见
    pub(crate) visible: bool,
    /// 切换类型，复选项必须为 `checkmark`
    pub(crate) toggle_type: Option<SharedString>,
    /// 切换状态
    pub(crate) toggle_state: Option<bool>,
    /// 应用侧的菜单项标识，点击回调时返回给 `on_tray_menu_action`
    pub(crate) action_id: Option<SharedString>,
    /// 子节点
    pub(crate) children: Vec<MenuNode>,
}

/// SNI 与主线程共享的托盘状态
pub(crate) struct SniShared {
    /// SNI `Id`，主机要求非空
    pub(crate) id: String,
    /// SNI `Title`
    pub(crate) title: String,
    /// SNI `Status`：`Active` 显示图标，`Passive` 会被主机隐藏
    pub(crate) status: String,
    /// 托盘图标像素
    pub(crate) icon_pixmap: IconPixmap,
    /// 工具提示
    pub(crate) tool_tip: ToolTip,
    /// 左键是否直接激活应用而非弹出菜单
    pub(crate) item_is_menu: bool,
    /// dbusmenu 布局版本号，每次菜单变更后自增
    pub(crate) menu_revision: u32,
    /// 顶层菜单节点
    pub(crate) menu: Vec<MenuNode>,
}

impl SniShared {
    /// 创建一个尚无图标的默认托盘状态
    pub(crate) fn new() -> Self {
        let id = app_sni_id();
        Self {
            title: id.clone(),
            id,
            // 主机对 Status 大小写敏感，Passive 会隐藏图标
            status: "Active".to_owned(),
            icon_pixmap: Vec::new(),
            tool_tip: (String::new(), Vec::new(), String::new(), String::new()),
            item_is_menu: true,
            menu_revision: 0,
            menu: Vec::new(),
        }
    }

    /// 按数字 id 深度查找菜单节点，用于把主机点击映射回应用侧标识
    pub(crate) fn find_action_id(&self, id: i32) -> Option<SharedString> {
        Self::find_in(&self.menu, id)
    }

    /// 在给定节点列表中递归查找 `id` 对应的应用侧标识
    fn find_in(nodes: &[MenuNode], id: i32) -> Option<SharedString> {
        for node in nodes {
            if node.id == id {
                return node.action_id.clone();
            }
            if let Some(found) = Self::find_in(&node.children, id) {
                return Some(found);
            }
        }
        None
    }
}

/// 推导 SNI `Id`：主机要求非空，取当前可执行文件名
///
/// 用进程名而非 `CARGO_PKG_NAME`，后者在托盘实现里恒为 `rgpui-linux`，
/// 会让主机把所有 rgpui 应用当成同一个托盘项。
fn app_sni_id() -> String {
    std::env::args_os()
        .next()
        .as_ref()
        .and_then(|arg| {
            std::path::Path::new(arg)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "rgpui".to_owned())
}

/// 主线程持有的托盘句柄，用于向 DBus 服务线程写入状态与命令
#[derive(Clone)]
pub(crate) struct TrayHandle {
    /// 与服务线程共享的状态
    pub(crate) shared: Arc<Mutex<SniShared>>,
    /// 变更通知通道
    pub(crate) commands: mpsc::UnboundedSender<TrayCommand>,
}

impl TrayHandle {
    /// 写入图标像素并通知主机重新读取属性
    pub(crate) fn set_icon_pixmap(&self, pixmap: IconPixmap) {
        self.shared.lock().icon_pixmap = pixmap;
        let _ = self.commands.unbounded_send(TrayCommand::PropertiesChanged);
    }

    /// 写入工具提示并通知主机重新读取属性
    pub(crate) fn set_tooltip(&self, tooltip: &str) {
        {
            let mut shared = self.shared.lock();
            shared.tool_tip = (String::new(), Vec::new(), tooltip.to_owned(), String::new());
        }
        let _ = self.commands.unbounded_send(TrayCommand::PropertiesChanged);
    }

    /// 写入菜单布局并通知主机重新拉取
    pub(crate) fn set_menu(&self, items: &[TrayMenuItem]) {
        {
            let mut shared = self.shared.lock();
            let mut next_id = 1;
            shared.menu = build_nodes(items, &mut next_id);
            shared.menu_revision += 1;
        }
        let _ = self.commands.unbounded_send(TrayCommand::MenuChanged);
    }

    /// 设置左键行为：`enabled` 为 true 时点击直接激活应用
    pub(crate) fn set_item_is_menu(&self, item_is_menu: bool) {
        {
            let mut shared = self.shared.lock();
            shared.item_is_menu = item_is_menu;
        }
        let _ = self.commands.unbounded_send(TrayCommand::PropertiesChanged);
    }
}

/// 托盘事件的 calloop 事件源，把服务线程产生的事件接入主线程事件循环
pub(crate) struct TrayEventSource {
    channel: Channel<TrayEvent>,
}

impl TrayEventSource {
    /// 用 [`LinuxCommon`] 创建的通道接收端构造事件源
    ///
    /// [`LinuxCommon`]: super::LinuxCommon
    pub(crate) fn new(channel: Channel<TrayEvent>) -> Self {
        Self { channel }
    }
}

impl EventSource for TrayEventSource {
    type Event = TrayEvent;
    type Metadata = ();
    type Ret = ();
    type Error = anyhow::Error;

    fn process_events<F>(
        &mut self,
        readiness: Readiness,
        token: Token,
        mut callback: F,
    ) -> Result<PostAction, Self::Error>
    where
        F: FnMut(Self::Event, &mut Self::Metadata) -> Self::Ret,
    {
        self.channel.process_events(readiness, token, |evt, _| {
            if let calloop::channel::Event::Msg(msg) = evt {
                (callback)(msg, &mut ())
            }
        })?;

        Ok(PostAction::Continue)
    }

    fn register(
        &mut self,
        poll: &mut Poll,
        token_factory: &mut TokenFactory,
    ) -> calloop::Result<()> {
        self.channel.register(poll, token_factory)
    }

    fn reregister(
        &mut self,
        poll: &mut Poll,
        token_factory: &mut TokenFactory,
    ) -> calloop::Result<()> {
        self.channel.reregister(poll, token_factory)
    }

    fn unregister(&mut self, poll: &mut Poll) -> calloop::Result<()> {
        self.channel.unregister(poll)
    }
}

/// 解码 PNG/ICO 等图标字节，产出 SNI 需要的多尺寸 ARGB32 pixmap
pub(crate) fn icon_pixmap_from_bytes(bytes: &[u8]) -> Option<IconPixmap> {
    let image = image::load_from_memory(bytes).ok()?;
    Some(pixmap_from_rgba(&image.to_rgba8()))
}

/// 把已渲染的原始 RGBA 字节（`Tray::render_icon` 的产物）转为 pixmap
pub(crate) fn pixmap_from_rgba_bytes(data: &[u8], width: u32, height: u32) -> Option<IconPixmap> {
    RgbaImage::from_raw(width, height, data.to_vec()).map(|image| pixmap_from_rgba(&image))
}

/// 把 RGBA 图像缩放为 [`ICON_SIZES`] 各档，并逐像素转为 ARGB32 大端字节
///
/// 主机端直接把字节按 `[A, R, G, B]` 重排为 RGBA，且**不做反预乘**，
/// 因此这里必须写入直通（非预乘）alpha。
fn pixmap_from_rgba(rgba: &RgbaImage) -> IconPixmap {
    ICON_SIZES
        .iter()
        .map(|&size| {
            let scaled =
                image::imageops::resize(rgba, size, size, image::imageops::FilterType::Lanczos3);
            let width = scaled.width() as i32;
            let height = scaled.height() as i32;
            let mut data = Vec::with_capacity((scaled.width() * scaled.height() * 4) as usize);
            for pixel in scaled.pixels() {
                let [r, g, b, a] = pixel.0;
                data.extend_from_slice(&[a, r, g, b]);
            }
            (width, height, data)
        })
        .collect()
}

/// 递归构建菜单节点
///
/// dbusmenu 的数字 id 由遍历顺序分配（从 1 开始，0 保留给根节点），应用侧的字符串
/// id 保存在 `action_id` 中，点击时回查；`next_id` 是贯穿整棵树的分配器。
fn build_nodes(items: &[TrayMenuItem], next_id: &mut i32) -> Vec<MenuNode> {
    items
        .iter()
        .map(|item| {
            let id = *next_id;
            *next_id += 1;
            match item {
                TrayMenuItem::Action {
                    label,
                    id: action_id,
                } => MenuNode {
                    id,
                    type_: MENU_TYPE_STANDARD,
                    label: label.clone(),
                    enabled: true,
                    visible: true,
                    toggle_type: None,
                    toggle_state: None,
                    action_id: Some(action_id.clone()),
                    children: Vec::new(),
                },
                TrayMenuItem::Separator => MenuNode {
                    id,
                    type_: MENU_TYPE_SEPARATOR,
                    label: SharedString::default(),
                    enabled: false,
                    visible: true,
                    toggle_type: None,
                    toggle_state: None,
                    action_id: None,
                    children: Vec::new(),
                },
                TrayMenuItem::Submenu { label, items } => MenuNode {
                    id,
                    type_: MENU_TYPE_SUBMENU,
                    label: label.clone(),
                    enabled: true,
                    visible: true,
                    toggle_type: None,
                    toggle_state: None,
                    action_id: None,
                    children: build_nodes(items, next_id),
                },
                TrayMenuItem::Toggle {
                    label,
                    checked,
                    id: action_id,
                } => MenuNode {
                    id,
                    type_: MENU_TYPE_STANDARD,
                    label: label.clone(),
                    enabled: true,
                    visible: true,
                    // 主机只认 "checkmark" 这一种 toggle-type
                    toggle_type: Some("checkmark".into()),
                    toggle_state: Some(*checked),
                    action_id: Some(action_id.clone()),
                    children: Vec::new(),
                },
            }
        })
        .collect()
}

/// 将旧的 `MenuItem` API 转换为 `TrayMenuItem`（`set_tray` 兼容路径）
pub(crate) fn convert_menu_items_to_tray(items: &[MenuItem]) -> Vec<TrayMenuItem> {
    items
        .iter()
        .filter_map(|item| match item {
            MenuItem::Separator => Some(TrayMenuItem::Separator),
            MenuItem::Action { name, .. } => Some(TrayMenuItem::Action {
                label: name.clone(),
                id: name.clone(),
            }),
            MenuItem::Submenu(menu) => Some(TrayMenuItem::Submenu {
                label: menu.name.clone(),
                items: convert_menu_items_to_tray(&menu.items),
            }),
            _ => None,
        })
        .collect()
}
