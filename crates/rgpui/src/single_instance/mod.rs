//! 跨平台单实例锁实现
//!
//! 防止应用程序同时运行多个实例，并把「第二个实例启动」变成第一个实例里的激活回调：
//! - Windows：命名互斥量做锁，命名事件做唤醒 —— 第二个实例 `SetEvent`，第一个实例的
//!   监听线程收到后投递激活信号；
//! - Unix：Unix 域套接字同时承担锁与消息通道 —— 第二个实例连接并写入 `activate`；
//! - 其他平台：只有占位实现，`acquire` 恒成功，激活回调不会触发。
//!
//! 激活回调在主线程执行：监听线程只往无界通道投递一个 token，主线程的前台任务收到后
//! 调用用户回调，因此回调里可以直接操作 [`App`]（恢复窗口、把应用带到前台等）。

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::{App, AsyncApp};

/// 第二个实例启动时写入 Unix 套接字的消息
#[cfg(unix)]
const ACTIVATE_MESSAGE: &[u8] = b"activate";

/// 表示应用已经在运行的错误
#[derive(Debug)]
pub struct AlreadyRunning;

impl std::fmt::Display for AlreadyRunning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "应用程序已经在运行中")
    }
}

impl std::error::Error for AlreadyRunning {}

/// 激活回调的共享状态
///
/// 监听线程与主线程共用。回调本身只做「往通道投递 token」这类轻量、线程安全的操作，
/// 真正碰 [`App`] 的代码在主线程的前台任务里执行。
struct ActivationState {
    /// 主线程注册的回调；后注册的覆盖先注册的
    callback: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    /// 回调尚未注册时到达过激活信号（只缓存一次，激活语义本身幂等）
    pending: AtomicBool,
    /// 析构时置位，监听线程看到后退出
    stopping: AtomicBool,
}

impl ActivationState {
    /// 创建空的共享状态
    fn new() -> Arc<Self> {
        Arc::new(Self {
            callback: Mutex::new(None),
            pending: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
        })
    }

    /// 安装回调；如果此前已有待处理信号，立即补发一次
    fn install(&self, callback: Arc<dyn Fn() + Send + Sync>) {
        let pending = {
            let Ok(mut slot) = self.callback.lock() else {
                return;
            };
            *slot = Some(callback.clone());
            self.pending.swap(false, Ordering::SeqCst)
        };
        if pending {
            callback();
        }
    }

    /// 监听线程侧：投递一次激活事件
    fn deliver(&self) {
        // 回调在锁外调用，避免用户代码再次注册时死锁
        let callback = match self.callback.lock() {
            Ok(slot) => slot.clone(),
            Err(_) => None,
        };
        match callback {
            Some(callback) => callback(),
            None => self.pending.store(true, Ordering::SeqCst),
        }
    }
}

/// 单实例锁
pub struct SingleInstance {
    #[cfg(windows)]
    inner: Option<WindowsSingleInstance>,
    #[cfg(unix)]
    inner: Option<UnixSingleInstance>,
    #[cfg(not(any(windows, unix)))]
    _placeholder: (),
    /// 激活回调的共享状态
    activation: Arc<ActivationState>,
}

#[cfg(windows)]
struct WindowsSingleInstance {
    /// 单实例锁（命名互斥量）
    mutex: windows::Win32::Foundation::HANDLE,
    /// 激活事件；创建失败时为 `None`，锁仍然有效，只是无法被唤醒
    event: Option<windows::Win32::Foundation::HANDLE>,
    /// 等待激活事件的监听线程
    thread: Option<std::thread::JoinHandle<()>>,
}

#[cfg(unix)]
struct UnixSingleInstance {
    /// 套接字路径，析构时删除
    socket_path: std::path::PathBuf,
    /// 持有监听器以保持套接字绑定（锁），监听线程用的是它的克隆
    listener: Option<std::os::unix::net::UnixListener>,
    /// 接受连接并投递激活信号的监听线程
    thread: Option<std::thread::JoinHandle<()>>,
}

impl SingleInstance {
    /// 获取单实例锁
    ///
    /// 成功时会同时启动激活监听线程：第二个实例调用
    /// [`send_activate_to_existing`] 后，本实例通过 [`SingleInstance::on_activate`]
    /// 注册的回调会被触发。
    ///
    /// # 参数
    /// * `app_id` - 应用程序唯一标识符
    ///
    /// # 返回
    /// 成功时返回 `SingleInstance`，如果应用已经在运行则返回 `AlreadyRunning`
    pub fn acquire(app_id: &str) -> Result<Self, AlreadyRunning> {
        let activation = ActivationState::new();
        #[cfg(windows)]
        {
            Self::acquire_windows(app_id, activation)
        }
        #[cfg(unix)]
        {
            Self::acquire_unix(app_id, activation)
        }
        #[cfg(not(any(windows, unix)))]
        {
            let _ = app_id;
            Ok(Self {
                _placeholder: (),
                activation,
            })
        }
    }

    #[cfg(windows)]
    fn acquire_windows(
        app_id: &str,
        activation: Arc<ActivationState>,
    ) -> Result<Self, AlreadyRunning> {
        use windows::Win32::Foundation::{
            CloseHandle, ERROR_ALREADY_EXISTS, ERROR_SUCCESS, GetLastError, SetLastError,
        };
        use windows::Win32::System::Threading::{
            CreateEventW, CreateMutexW, INFINITE, WaitForSingleObject,
        };
        use windows::core::PCWSTR;

        let mutex_name = to_wide(&format!("Global\\{}", app_id));
        // 命名对象是否已存在只能靠 GetLastError 判断，先清掉同线程残留的错误码
        unsafe { SetLastError(ERROR_SUCCESS) };
        let handle = unsafe { CreateMutexW(None, true, PCWSTR::from_raw(mutex_name.as_ptr())) };

        let Ok(mutex) = handle else {
            return Err(AlreadyRunning);
        };

        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe {
                let _ = CloseHandle(mutex);
            }
            return Err(AlreadyRunning);
        }

        // 激活事件：第二个实例 OpenEventW 后 SetEvent 即可唤醒本实例
        let mut event = None;
        let mut thread = None;
        let event_name = to_wide(&format!("Global\\{}.activate", app_id));
        if let Ok(handle) =
            unsafe { CreateEventW(None, false, false, PCWSTR::from_raw(event_name.as_ptr())) }
        {
            event = Some(handle);
            let listener_activation = activation.clone();
            // HANDLE 内含裸指针，不能直接跨线程；事件句柄在进程内有效，按整数传递
            let event_value = handle.0 as isize;
            thread = std::thread::Builder::new()
                .name("single-instance".into())
                .spawn(move || {
                    use windows::Win32::Foundation::WAIT_OBJECT_0;

                    let event =
                        windows::Win32::Foundation::HANDLE(event_value as *mut core::ffi::c_void);
                    loop {
                        let wait = unsafe { WaitForSingleObject(event, INFINITE) };
                        if wait != WAIT_OBJECT_0 {
                            break;
                        }
                        // `Drop` 会用同一个事件唤醒线程，看到停止位就直接退出
                        if listener_activation.stopping.load(Ordering::SeqCst) {
                            break;
                        }
                        listener_activation.deliver();
                    }
                })
                .ok();
        }

        Ok(Self {
            inner: Some(WindowsSingleInstance {
                mutex,
                event,
                thread,
            }),
            activation,
        })
    }

    #[cfg(unix)]
    fn acquire_unix(
        app_id: &str,
        activation: Arc<ActivationState>,
    ) -> Result<Self, AlreadyRunning> {
        use std::os::unix::net::UnixListener;

        let socket_path = std::env::temp_dir().join(format!("{}.sock", app_id));

        // 能连上说明已经有实例在监听
        if std::os::unix::net::UnixStream::connect(&socket_path).is_ok() {
            return Err(AlreadyRunning);
        }

        // 清掉崩溃留下的套接字文件
        let _ = std::fs::remove_file(&socket_path);

        let Ok(listener) = UnixListener::bind(&socket_path) else {
            return Err(AlreadyRunning);
        };

        let thread = listener.try_clone().ok().and_then(|listener| {
            // 非阻塞 + 短轮询：`Drop` 只置停止位，不必再自连唤醒
            let _ = listener.set_nonblocking(true);
            let listener_activation = activation.clone();
            std::thread::Builder::new()
                .name("single-instance".into())
                .spawn(move || {
                    use std::io::Read;

                    let mut buf = [0u8; 16];
                    loop {
                        if listener_activation.stopping.load(Ordering::SeqCst) {
                            break;
                        }
                        let mut stream = match listener.accept() {
                            Ok((stream, _)) => stream,
                            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                                std::thread::sleep(std::time::Duration::from_millis(50));
                                continue;
                            }
                            Err(_) => break,
                        };
                        // 半开连接不能把监听线程永久卡住
                        let _ =
                            stream.set_read_timeout(Some(std::time::Duration::from_millis(500)));
                        let read = stream.read(&mut buf).unwrap_or(0);
                        if read >= ACTIVATE_MESSAGE.len()
                            && buf.get(..ACTIVATE_MESSAGE.len()) == Some(ACTIVATE_MESSAGE)
                        {
                            listener_activation.deliver();
                        }
                    }
                })
                .ok()
        });

        Ok(Self {
            inner: Some(UnixSingleInstance {
                socket_path,
                listener: Some(listener),
                thread,
            }),
            activation,
        })
    }

    /// 注册「另一个实例启动」时的激活回调
    ///
    /// 回调在应用主线程执行，可以直接操作 [`App`]（恢复窗口、把应用带到前台等）。
    /// 监听线程在 [`SingleInstance::acquire`] 成功时就已启动；在回调注册之前到达的
    /// 激活信号会缓存一次，注册后立即补发。后注册的回调覆盖先注册的。
    ///
    /// # 参数
    /// * `cx` - 应用上下文，用于把回调调度到主线程
    /// * `callback` - 激活回调
    pub fn on_activate(&self, cx: &App, mut callback: impl FnMut(&mut App) + 'static) {
        let (sender, mut receiver) = futures::channel::mpsc::unbounded::<()>();
        // 监听线程只负责投递 token，避免在非主线程碰 `App`
        self.activation.install(Arc::new(move || {
            let _ = sender.unbounded_send(());
        }));

        cx.spawn(async move |cx: &mut AsyncApp| {
            use futures::StreamExt;

            while receiver.next().await.is_some() {
                cx.update(|cx| callback(cx));
            }
        })
        .detach();
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        // 先置停止位：Windows 的等待线程要等事件唤醒后才检查它
        self.activation.stopping.store(true, Ordering::SeqCst);

        #[cfg(windows)]
        {
            use windows::Win32::Foundation::CloseHandle;
            use windows::Win32::System::Threading::SetEvent;

            if let Some(inner) = self.inner.take() {
                if let Some(event) = inner.event {
                    unsafe {
                        let _ = SetEvent(event);
                    }
                }
                if let Some(thread) = inner.thread {
                    let _ = thread.join();
                }
                unsafe {
                    if let Some(event) = inner.event {
                        let _ = CloseHandle(event);
                    }
                    let _ = CloseHandle(inner.mutex);
                }
            }
        }
        #[cfg(unix)]
        {
            if let Some(inner) = self.inner.take() {
                if let Some(thread) = inner.thread {
                    let _ = thread.join();
                }
                drop(inner.listener);
                let _ = std::fs::remove_file(&inner.socket_path);
            }
        }
    }
}

/// 向已运行的实例发送激活信号
///
/// 已有实例通过 [`SingleInstance::on_activate`] 注册的回调会被触发。
///
/// # 参数
/// * `app_id` - 应用程序唯一标识符
///
/// # 返回
/// 成功时返回 `Ok(())`，失败时返回错误
pub fn send_activate_to_existing(app_id: &str) -> Result<(), io::Error> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::net::UnixStream;

        let socket_path = std::env::temp_dir().join(format!("{}.sock", app_id));
        let mut stream = UnixStream::connect(socket_path)?;
        stream.write_all(ACTIVATE_MESSAGE)?;
        Ok(())
    }
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{EVENT_MODIFY_STATE, OpenEventW, SetEvent};
        use windows::core::PCWSTR;

        let event_name = to_wide(&format!("Global\\{}.activate", app_id));
        let mut last_error = io::Error::new(io::ErrorKind::NotFound, "已有实例的激活事件不存在");
        // 第一个实例「拿到互斥量」到「建好事件」之间的窗口很小，重试几次即可覆盖
        for attempt in 0..10 {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            let event = unsafe {
                OpenEventW(
                    EVENT_MODIFY_STATE,
                    false,
                    PCWSTR::from_raw(event_name.as_ptr()),
                )
            };
            match event {
                Ok(event) => {
                    let result = unsafe { SetEvent(event) };
                    unsafe {
                        let _ = CloseHandle(event);
                    }
                    return result.map_err(|err| io::Error::other(err.message()));
                }
                Err(err) => last_error = io::Error::other(err.message()),
            }
        }
        Err(last_error)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = app_id;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "当前平台不支持单实例锁",
        ))
    }
}

/// 把 Rust 字符串转成以 NUL 结尾的 UTF-16 缓冲
#[cfg(windows)]
fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::{Duration, Instant};

    // 回调注册之前到达的激活信号会缓存一次，注册后立刻补发
    #[test]
    fn pending_activation_is_delivered_on_registration() {
        let state = ActivationState::new();
        state.deliver();

        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        state.install(Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }));
        assert_eq!(hits.load(Ordering::SeqCst), 1, "注册时应补发缓存的激活信号");

        // 补发只发生一次
        state.install(Arc::new(|| {}));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    // 注册之后的激活信号立即投递
    #[test]
    fn activation_is_delivered_after_registration() {
        let state = ActivationState::new();
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        state.install(Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }));

        state.deliver();
        state.deliver();
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    // 第二个实例拿不到锁，激活信号能真正唤醒第一个实例；释放后可以重新获取
    #[test]
    fn second_instance_signals_the_first() {
        let app_id = format!("com.rgpui.test.single-instance.{}", std::process::id());
        let first = SingleInstance::acquire(&app_id).expect("首个实例应能获取锁");

        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        first.activation.install(Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }));

        assert!(
            SingleInstance::acquire(&app_id).is_err(),
            "第二个实例不能获取锁"
        );
        send_activate_to_existing(&app_id).expect("激活信号应送达");

        let deadline = Instant::now() + Duration::from_secs(2);
        while hits.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "第二个实例的激活信号应唤醒首个实例"
        );

        drop(first);

        // 锁释放后（套接字/互斥量都已回收）新实例可以重新获取
        let again = SingleInstance::acquire(&app_id).expect("首个实例释放后应能重新获取锁");
        drop(again);
    }
}
