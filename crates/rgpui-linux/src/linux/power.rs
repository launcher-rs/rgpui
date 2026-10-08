//! Linux 电源能力：睡眠/息屏抑制（login1 `Inhibit`）与睡眠/唤醒事件。
//!
//! login1 的抑制是**持有 fd 即生效**：`Inhibit` 返回一个 fifo 的 fd，只要它还开着，
//! systemd 就不会让系统进入对应的低功耗状态，fd 一关立刻恢复。所以这里把 fd 封成
//! [`PowerSaveBlocker`] 句柄交给调用方，平台内部不做「ID → 句柄」的记账 ——
//! 那份记账等于把 fd 泄漏在框架里，应用忘没忘停都无人知晓。

use anyhow::Context as _;
use ashpd::zbus::{Connection, Proxy};
use ashpd::zvariant::OwnedFd;
use calloop::channel::Sender;
use futures::StreamExt as _;
use rgpui::{PowerSaveBlocker, PowerSaveBlockerKind, Result, SystemPowerEvent};

/// logind 的总线名
const LOGIN1_BUS: &str = "org.freedesktop.login1";
/// logind 管理器对象路径
const LOGIN1_PATH: &str = "/org/freedesktop/login1";
/// logind 管理器接口名
const LOGIN1_MANAGER: &str = "org.freedesktop.login1.Manager";

/// login1 抑制句柄：字段从不被读，它的价值就是「fd 还开着」这件事本身，
/// `Drop` 关闭 fd 即取消抑制
struct LoginInhibit {
    _fd: OwnedFd,
}

impl PowerSaveBlocker for LoginInhibit {}

/// 向 login1 申请抑制。
///
/// `PreventDisplaySleep` 抑制的是 logind 的空闲动作；桌面会话里真正让屏幕变暗的是
/// 合成器自家的 DPMS/息屏策略，不受 login1 抑制约束，需要连屏幕一起保住时还得
/// 走显示服务器接口。
pub(crate) fn inhibit(kind: PowerSaveBlockerKind) -> Result<Box<dyn PowerSaveBlocker>> {
    let (what, why) = match kind {
        PowerSaveBlockerKind::PreventSleep => ("sleep", "rgpui 应用请求阻止系统休眠"),
        PowerSaveBlockerKind::PreventDisplaySleep => ("idle", "rgpui 应用请求阻止系统息屏"),
    };
    // systemd-inhibit --list 里显示的申请者名字，取当前可执行文件名
    let who = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "rgpui".to_owned());

    let fd = pollster::block_on(inhibit_fd(what, &who, why))?;
    Ok(Box::new(LoginInhibit { _fd: fd }))
}

/// 一次 DBus 往返：`Inhibit(ssss) -> (h)`，返回的 fd 就是抑制凭证
async fn inhibit_fd(what: &str, who: &str, why: &str) -> Result<OwnedFd> {
    let connection = Connection::system()
        .await
        .context("连接系统总线失败，无法申请电源抑制")?;
    let (fd,): (OwnedFd,) = connection
        .call_method(
            Some(LOGIN1_BUS),
            LOGIN1_PATH,
            Some(LOGIN1_MANAGER),
            "Inhibit",
            &(what, who, why, "block"),
        )
        .await
        .context("调用 login1 Inhibit 失败")?
        .body()
        .deserialize()
        .context("解析 login1 Inhibit 返回值失败")?;
    Ok(fd)
}

/// 监听 login1 的 `PrepareForSleep` 信号，把「即将睡眠 / 已唤醒」送回主线程。
///
/// 该信号带一个 bool：`true` = 即将进入睡眠（还可以收尾），`false` = 已从睡眠恢复。
pub(crate) async fn listen_for_system_power(sender: Sender<SystemPowerEvent>) -> Result<()> {
    let connection = Connection::system().await?;
    let proxy = Proxy::new(&connection, LOGIN1_BUS, LOGIN1_PATH, LOGIN1_MANAGER).await?;
    let mut events = proxy.receive_signal("PrepareForSleep").await?;

    while let Some(message) = events.next().await {
        let sleeping = message.body().deserialize::<bool>()?;
        let event = if sleeping {
            SystemPowerEvent::Sleep
        } else {
            SystemPowerEvent::WakeUp
        };
        sender.send(event).ok();
    }

    Ok(())
}
