//! Linux 聚焦窗口信息实现
//!
//! X11 会话走 EWMH：`_NET_ACTIVE_WINDOW` 找活动窗口，`_NET_WM_PID` 还原进程，
//! `_NET_WM_NAME` / `WM_NAME` 取标题，`WM_CLASS` 补应用名。
//!
//! Wayland 侧没有等价的全局协议 —— 合成器不会把「当前聚焦窗口」暴露给普通客户端
//! （这是有意的隐私边界），因此 Wayland 原生会话下返回 `None`，而不是编一个
//! `"Unknown"` 让调用方误以为拿到了结果。

use rgpui::FocusedWindowInfo;

/// 获取当前系统聚焦窗口信息。
///
/// # 返回
/// 能确定活动窗口时返回窗口信息；Wayland 原生会话、无法连接 X11、或窗口
/// 尚未设置 EWMH 属性时返回 `None`。
pub fn get_focused_window_info() -> Option<FocusedWindowInfo> {
    #[cfg(feature = "x11")]
    return x11_focused_window();

    #[cfg(not(feature = "x11"))]
    return None;
}

#[cfg(feature = "x11")]
fn x11_focused_window() -> Option<FocusedWindowInfo> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};

    let (connection, screen_num) = x11rb::connect(None).ok()?;
    let root = connection.setup().roots.get(screen_num)?.root;

    let active_window_atom = intern_atom(&connection, b"_NET_ACTIVE_WINDOW")?;
    let reply = connection
        .get_property(false, root, active_window_atom, AtomEnum::WINDOW, 0, 1)
        .ok()?
        .reply()
        .ok()?;
    let window = reply.value32()?.next()?;
    if window == x11rb::NONE {
        return None;
    }

    let pid = cardinal_property(&connection, window, b"_NET_WM_PID");
    let window_title = utf8_property(&connection, window, b"_NET_WM_NAME")
        .or_else(|| string_property(&connection, window, AtomEnum::WM_NAME))
        .unwrap_or_default();

    // WM_CLASS 是两个 NUL 分隔的字符串：实例名、类名；类名更接近应用名
    let class_name = string_property(&connection, window, AtomEnum::WM_CLASS)
        .and_then(|value| value.split('\0').nth(1).map(str::to_owned));

    let app_name = pid
        .and_then(process_name)
        .or_else(|| class_name.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    Some(FocusedWindowInfo {
        app_name,
        window_title,
        bundle_id: class_name,
        pid,
    })
}

/// 在连接上按名字取原子（只查询已存在的原子，不创建）。
#[cfg(feature = "x11")]
fn intern_atom(connection: &impl x11rb::connection::Connection, name: &[u8]) -> Option<u32> {
    use x11rb::protocol::xproto::ConnectionExt;

    Some(connection.intern_atom(false, name).ok()?.reply().ok()?.atom)
}

/// 读取一个 32 位属性（CARDINAL/WINDOW）的首个值。
#[cfg(feature = "x11")]
fn cardinal_property(
    connection: &impl x11rb::connection::Connection,
    window: u32,
    name: &[u8],
) -> Option<u32> {
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};

    let atom = intern_atom(connection, name)?;
    let reply = connection
        .get_property(false, window, atom, AtomEnum::CARDINAL, 0, 1)
        .ok()?
        .reply()
        .ok()?;
    reply.value32()?.next()
}

/// 读取 UTF-8 文本属性（`_NET_WM_NAME`），空值按缺失处理。
#[cfg(feature = "x11")]
fn utf8_property(
    connection: &impl x11rb::connection::Connection,
    window: u32,
    name: &[u8],
) -> Option<String> {
    use x11rb::protocol::xproto::ConnectionExt;

    let atom = intern_atom(connection, name)?;
    let utf8_string = intern_atom(connection, b"UTF8_STRING")?;
    let reply = connection
        .get_property(false, window, atom, utf8_string, 0, 1024)
        .ok()?
        .reply()
        .ok()?;
    if reply.value.is_empty() {
        None
    } else {
        String::from_utf8(reply.value).ok()
    }
}

/// 读取 Latin-1/STRING 文本属性（`WM_NAME`、`WM_CLASS`）。
#[cfg(feature = "x11")]
fn string_property(
    connection: &impl x11rb::connection::Connection,
    window: u32,
    atom: x11rb::protocol::xproto::AtomEnum,
) -> Option<String> {
    use x11rb::protocol::xproto::ConnectionExt;

    let reply = connection
        .get_property(
            false,
            window,
            atom,
            x11rb::protocol::xproto::AtomEnum::STRING,
            0,
            1024,
        )
        .ok()?
        .reply()
        .ok()?;
    if reply.value.is_empty() {
        None
    } else {
        Some(String::from_utf8_lossy(&reply.value).into_owned())
    }
}

/// 从 `/proc/<pid>/comm` 读取进程名。
#[cfg(feature = "x11")]
fn process_name(pid: u32) -> Option<String> {
    let name = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}
