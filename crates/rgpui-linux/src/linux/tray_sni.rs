//! Linux 托盘：StatusNotifierItem 与 com.canonical.dbusmenu 的 DBus 服务端。
//!
//! 主机（GNOME 的 ubuntu-appindicators、KDE、XFCE 等）通过 session bus 反向读取
//! 本模块暴露的接口来绘制托盘图标与菜单。签名取自本机实测的
//! `/usr/share/gnome-shell/extensions/ubuntu-appindicators@ubuntu.com`。

use super::tray::{
    IconPixmap, MENU_TYPE_SUBMENU, MenuNode, SniShared, ToolTip, TrayCommand, TrayEvent,
};
use anyhow::{Context as _, Result};
use ashpd::zbus::{Connection, object_server::SignalEmitter};
// zbus 的宏按其名字本身（`zbus`）识别属性前缀，因此这里必须先把 re-export 以
// `zbus` 的名字引入，再写 `#[zbus::interface(...)]`；生成的代码则通过下面的
// `crate = "ashpd::zbus"` 找到真实的 crate 路径（该选项对外名为 `crate`，
// 尽管宏内部字段叫 crate_path）。
use ashpd::zbus;
use ashpd::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Str, Structure};
use calloop::channel::Sender;
use futures::{StreamExt, channel::mpsc};
use parking_lot::Mutex;
use rgpui::TrayIconEvent;
use std::collections::HashMap;
use std::sync::Arc;

/// 托盘图标在 session bus 上的对象路径
const ITEM_PATH: &str = "/StatusNotifierItem";
/// dbusmenu 菜单对象的对象路径
const MENU_PATH: &str = "/StatusNotifierItem/Menu";
/// StatusNotifierWatcher 的总线名
const WATCHER_BUS: &str = "org.kde.StatusNotifierWatcher";
/// StatusNotifierWatcher 的对象路径
const WATCHER_PATH: &str = "/StatusNotifierWatcher";

/// dbusmenu `GetLayout` 的布局节点：`(id, properties, children)`，对应 `(ia{sv}av)`
type Layout = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);

/// `org.kde.StatusNotifierItem` 接口实现
struct Sni {
    /// 与主线程共享的托盘状态
    shared: Arc<Mutex<SniShared>>,
    /// 投递交互事件回主线程
    events: Sender<TrayEvent>,
}

#[zbus::interface(crate = "ashpd::zbus", name = "org.kde.StatusNotifierItem")]
impl Sni {
    /// SNI `Id`：主机要求非空，否则拒绝注册
    #[zbus(property)]
    async fn id(&self) -> String {
        self.shared.lock().id.clone()
    }

    /// SNI `Category`
    #[zbus(property)]
    async fn category(&self) -> String {
        "ApplicationStatus".to_owned()
    }

    /// SNI `Title`
    #[zbus(property)]
    async fn title(&self) -> String {
        self.shared.lock().title.clone()
    }

    /// SNI `Status`：主机大小写敏感，`Passive` 会隐藏图标
    #[zbus(property)]
    async fn status(&self) -> String {
        self.shared.lock().status.clone()
    }

    /// SNI `IconThemePath`：本实现直接内联像素，不使用主题图标名
    #[zbus(property)]
    async fn icon_theme_path(&self) -> String {
        String::new()
    }

    /// SNI `IconName`
    #[zbus(property)]
    async fn icon_name(&self) -> String {
        String::new()
    }

    /// SNI `IconPixmap`：ARGB32 大端、直通 alpha 的多尺寸位图
    #[zbus(property)]
    async fn icon_pixmap(&self) -> IconPixmap {
        self.shared.lock().icon_pixmap.clone()
    }

    /// SNI `ToolTip`
    ///
    /// 注意 GNOME 的 appindicators 扩展并未渲染工具提示，此属性仅为协议完整性。
    #[zbus(property)]
    async fn tool_tip(&self) -> ToolTip {
        self.shared.lock().tool_tip.clone()
    }

    /// SNI `ItemIsMenu`：为 true 时左键直接激活应用而不是弹出菜单
    #[zbus(property)]
    async fn item_is_menu(&self) -> bool {
        self.shared.lock().item_is_menu
    }

    /// SNI `Menu`：dbusmenu 对象的路径
    #[zbus(property)]
    async fn menu(&self) -> OwnedObjectPath {
        ObjectPath::from_str_unchecked(MENU_PATH).into()
    }

    /// 主机报告图标被左键单击
    async fn activate(&self, _x: i32, _y: i32) {
        push_event(&self.events, TrayEvent::Icon(TrayIconEvent::LeftClick));
    }

    /// 主机报告图标被左键双击
    async fn secondary_activate(&self, _x: i32, _y: i32) {
        push_event(&self.events, TrayEvent::Icon(TrayIconEvent::DoubleClick));
    }

    /// 主机报告图标被右键单击
    async fn context_menu(&self, _x: i32, _y: i32) {
        push_event(&self.events, TrayEvent::Icon(TrayIconEvent::RightClick));
    }

    /// 图标已变化，请主机重新读取 `Icon*` 属性
    #[zbus(signal)]
    async fn new_icon(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    /// 工具提示已变化，请主机重新读取 `ToolTip` 属性
    #[zbus(signal)]
    async fn new_tool_tip(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    /// 状态已变化
    #[zbus(signal)]
    async fn new_status(emitter: &SignalEmitter<'_>, status: &str) -> zbus::Result<()>;
}

/// `com.canonical.dbusmenu` 接口实现
struct DBusMenu {
    /// 与主线程共享的托盘状态
    shared: Arc<Mutex<SniShared>>,
    /// 投递菜单点击事件回主线程
    events: Sender<TrayEvent>,
}

#[zbus::interface(crate = "ashpd::zbus", name = "com.canonical.dbusmenu")]
impl DBusMenu {
    /// dbusmenu 协议版本
    #[zbus(property)]
    async fn version(&self) -> u32 {
        3
    }

    /// 文本方向
    #[zbus(property)]
    async fn text_direction(&self) -> String {
        "ltr".to_owned()
    }

    /// 菜单状态
    #[zbus(property)]
    async fn status(&self) -> String {
        "normal".to_owned()
    }

    /// 图标主题路径
    #[zbus(property)]
    async fn icon_theme_path(&self) -> Vec<String> {
        Vec::new()
    }

    /// 返回菜单布局
    ///
    /// `property_names` 被刻意忽略：主机在增量更新时会据此过滤，但 Ubuntu 的扩展
    /// 只读取 `type` 与 `children-display`，返回完整属性字典可以避免属性变更后
    /// 主机拿不到新值的问题。
    async fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        _property_names: Vec<String>,
    ) -> zbus::fdo::Result<(u32, Layout)> {
        let shared = self.shared.lock();
        let revision = shared.menu_revision;

        if parent_id == 0 {
            let children = collect_children(&shared.menu, recursion_depth)?;
            return Ok((revision, (0, HashMap::new(), children)));
        }

        let node = find_node(&shared.menu, parent_id).ok_or_else(|| {
            zbus::fdo::Error::Failed(format!("dbusmenu 中不存在 id 为 {parent_id} 的菜单项"))
        })?;
        let children = collect_children(&node.children, recursion_depth)?;
        Ok((revision, (node.id, node_properties(node), children)))
    }

    /// 批量读取菜单项属性
    async fn get_group_properties(
        &self,
        ids: Vec<i32>,
        _property_names: Vec<String>,
    ) -> Vec<(i32, HashMap<String, OwnedValue>)> {
        let shared = self.shared.lock();
        ids.into_iter()
            .filter_map(|id| {
                if id == 0 {
                    return Some((0, HashMap::new()));
                }
                find_node(&shared.menu, id).map(|node| (node.id, node_properties(node)))
            })
            .collect()
    }

    /// 主机上报菜单项交互事件
    async fn event(&self, id: i32, event_id: String, _data: OwnedValue, _timestamp: u32) {
        if event_id != "clicked" {
            return;
        }
        let action_id = self.shared.lock().find_action_id(id);
        let Some(action_id) = action_id else {
            log::debug!("dbusmenu id {id} 没有对应的应用侧菜单项标识");
            return;
        };
        push_event(&self.events, TrayEvent::MenuAction(action_id));
    }

    /// 主机即将展示某个子菜单；本实现始终预先构建好布局，无需增量更新
    async fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    /// 菜单布局已变化，请主机重新调用 `GetLayout`
    #[zbus(signal)]
    async fn layout_updated(
        emitter: &SignalEmitter<'_>,
        revision: u32,
        parent: i32,
    ) -> zbus::Result<()>;
}

/// 把事件投递回主线程
///
/// 投递失败只可能因为主线程事件循环已经退出，此时应用正在关闭，丢弃即可。
fn push_event(events: &Sender<TrayEvent>, event: TrayEvent) {
    if events.send(event).is_err() {
        log::debug!("托盘事件通道已关闭，丢弃事件");
    }
}

/// 构造 dbusmenu 菜单项的属性字典
///
/// 主机用 `propertyGetInt` 读取 `toggle-state`，因此它必须是整数而非布尔值。
fn node_properties(node: &MenuNode) -> HashMap<String, OwnedValue> {
    let mut properties = HashMap::new();
    properties.insert("type".to_owned(), OwnedValue::from(Str::from(node.type_)));
    properties.insert(
        "label".to_owned(),
        OwnedValue::from(Str::from(node.label.as_str())),
    );
    properties.insert("enabled".to_owned(), OwnedValue::from(node.enabled));
    properties.insert("visible".to_owned(), OwnedValue::from(node.visible));

    if let Some(toggle_type) = &node.toggle_type {
        properties.insert(
            "toggle-type".to_owned(),
            OwnedValue::from(Str::from(toggle_type.as_str())),
        );
        let state = i32::from(node.toggle_state.unwrap_or(false));
        properties.insert("toggle-state".to_owned(), OwnedValue::from(state));
    }

    // 主机靠 children-display 判断是否为可展开的子菜单
    if node.type_ == MENU_TYPE_SUBMENU {
        properties.insert(
            "children-display".to_owned(),
            OwnedValue::from(Str::from("submenu")),
        );
    }

    properties
}

/// 在菜单树中按数字 id 深度查找节点
fn find_node(nodes: &[MenuNode], id: i32) -> Option<&MenuNode> {
    nodes.iter().find_map(|node| {
        if node.id == id {
            Some(node)
        } else {
            find_node(&node.children, id)
        }
    })
}

/// 递归收集子布局，`recursion_depth` 为 -1 表示不限深度
fn collect_children(
    nodes: &[MenuNode],
    recursion_depth: i32,
) -> zbus::fdo::Result<Vec<OwnedValue>> {
    if recursion_depth == 0 {
        return Ok(Vec::new());
    }
    let next_depth = recursion_depth.saturating_sub(1);
    nodes
        .iter()
        .map(|node| {
            let children = collect_children(&node.children, next_depth)?;
            to_variant((node.id, node_properties(node), children))
        })
        .collect()
}

/// 把布局节点装箱成 `av` 的元素
fn to_variant(layout: Layout) -> zbus::fdo::Result<OwnedValue> {
    OwnedValue::try_from(Structure::from(layout))
        .map_err(|err| zbus::fdo::Error::Failed(format!("无法序列化 dbusmenu 布局: {err}")))
}

/// 向 watcher 注册托盘图标
///
/// 以对象路径形式注册，无需申请 well-known name。
async fn register(conn: &Connection) -> Result<()> {
    conn.call_method(
        Some(WATCHER_BUS),
        WATCHER_PATH,
        Some("org.kde.StatusNotifierWatcher"),
        "RegisterStatusNotifierItem",
        &(ITEM_PATH,),
    )
    .await
    .context("RegisterStatusNotifierItem 调用失败")?;
    Ok(())
}

/// 运行托盘 DBus 服务：注册接口、向 watcher 登记，然后持续响应主线程的变更命令
///
/// `Connection::session()` 自带内部执行器线程，因此只要本 future 在后台执行器上
/// 持续存活即可，无需额外的 serve-forever 循环。
pub(crate) async fn serve(
    shared: Arc<Mutex<SniShared>>,
    mut commands: mpsc::UnboundedReceiver<TrayCommand>,
    events: Sender<TrayEvent>,
) -> Result<()> {
    let conn = Connection::session()
        .await
        .context("无法连接 session bus")?;

    let item_path = ObjectPath::from_str_unchecked(ITEM_PATH);
    let menu_path = ObjectPath::from_str_unchecked(MENU_PATH);

    let server = conn.object_server();
    server
        .at(
            item_path.clone(),
            Sni {
                shared: shared.clone(),
                events: events.clone(),
            },
        )
        .await
        .context("注册 StatusNotifierItem 接口失败")?;
    server
        .at(
            menu_path.clone(),
            DBusMenu {
                shared: shared.clone(),
                events,
            },
        )
        .await
        .context("注册 com.canonical.dbusmenu 接口失败")?;

    let sni_emitter = SignalEmitter::new(&conn, item_path)?;
    let menu_emitter = SignalEmitter::new(&conn, menu_path)?;

    // watcher 可能尚未就绪（桌面环境重启等）。不在这里轮询等待，而是借主线程后续
    // 发来的每一次变更命令重试注册：设置图标与菜单本身就会产生数次命令。
    let mut registered = register(&conn).await.is_ok();
    if !registered {
        log::debug!("StatusNotifierWatcher 尚未就绪，将在下次托盘更新时重试注册");
    }

    while let Some(command) = commands.next().await {
        if !registered {
            if register(&conn).await.is_ok() {
                registered = true;
            } else {
                continue;
            }
        }

        match command {
            TrayCommand::PropertiesChanged => {
                let status = shared.lock().status.clone();
                let _ = sni_emitter.new_icon().await;
                let _ = sni_emitter.new_tool_tip().await;
                let _ = sni_emitter.new_status(&status).await;
            }
            TrayCommand::MenuChanged => {
                let revision = shared.lock().menu_revision;
                let _ = menu_emitter.layout_updated(revision, 0).await;
            }
        }
    }

    Ok(())
}
