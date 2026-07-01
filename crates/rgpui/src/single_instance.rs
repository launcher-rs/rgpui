//! 跨平台单实例锁实现
//!
//! 防止应用程序同时运行多个实例
//! Windows 使用命名互斥量，Unix 使用 Unix 域套接字

use std::io;

/// 表示应用已经在运行的错误
#[derive(Debug)]
pub struct AlreadyRunning;

impl std::fmt::Display for AlreadyRunning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "应用程序已经在运行中")
    }
}

impl std::error::Error for AlreadyRunning {}

/// 单实例锁
pub struct SingleInstance {
    #[cfg(windows)]
    inner: Option<windows::Win32::Foundation::HANDLE>,
    #[cfg(unix)]
    inner: Option<UnixSingleInstance>,
    #[cfg(not(any(windows, unix)))]
    _placeholder: (),
}

#[cfg(unix)]
struct UnixSingleInstance {
    socket_path: std::path::PathBuf,
    #[allow(dead_code)]
    listener: Option<std::os::unix::net::UnixListener>,
    #[allow(dead_code)]
    activate_callback: Option<Box<dyn Fn() + Send + 'static>>,
}

impl SingleInstance {
    /// 获取单实例锁
    ///
    /// # 参数
    /// * `app_id` - 应用程序唯一标识符
    ///
    /// # 返回
    /// 成功时返回 `SingleInstance`，如果应用已经在运行则返回 `AlreadyRunning`
    pub fn acquire(app_id: &str) -> Result<Self, AlreadyRunning> {
        #[cfg(windows)]
        {
            Self::acquire_windows(app_id)
        }
        #[cfg(unix)]
        {
            Self::acquire_unix(app_id)
        }
        #[cfg(not(any(windows, unix)))]
        {
            let _ = app_id;
            Ok(Self { _placeholder: () })
        }
    }

    #[cfg(windows)]
    fn acquire_windows(app_id: &str) -> Result<Self, AlreadyRunning> {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
        use windows::Win32::System::Threading::CreateMutexW;

        let mutex_name = format!("Global\\{}", app_id);
        let mutex_name_utf16: Vec<u16> = mutex_name.encode_utf16().chain(Some(0)).collect();

        unsafe {
            let handle = CreateMutexW(
                None,
                true,
                windows::core::PCWSTR::from_raw(mutex_name_utf16.as_ptr()),
            );

            if handle.is_err() {
                return Err(AlreadyRunning);
            }

            let handle = handle.unwrap();

            if windows::Win32::Foundation::GetLastError() == ERROR_ALREADY_EXISTS {
                CloseHandle(handle).ok();
                return Err(AlreadyRunning);
            }

            Ok(Self {
                inner: Some(handle),
            })
        }
    }

    #[cfg(unix)]
    fn acquire_unix(app_id: &str) -> Result<Self, AlreadyRunning> {
        use std::os::unix::net::UnixListener;

        let socket_path = std::env::temp_dir().join(format!("{}.sock", app_id));

        // 尝试连接到已有的套接字
        if std::os::unix::net::UnixStream::connect(&socket_path).is_ok() {
            return Err(AlreadyRunning);
        }

        // 删除旧的套接字文件（如果存在）
        let _ = std::fs::remove_file(&socket_path);

        // 绑定新的套接字
        match UnixListener::bind(&socket_path) {
            Ok(listener) => Ok(Self {
                inner: Some(UnixSingleInstance {
                    socket_path,
                    listener: Some(listener),
                    activate_callback: None,
                }),
            }),
            Err(_) => Err(AlreadyRunning),
        }
    }

    /// 注册当另一个实例尝试启动时的回调
    ///
    /// # 参数
    /// * `callback` - 回调函数
    pub fn on_activate(&self, _callback: Box<dyn Fn() + Send + 'static>) {
        #[cfg(unix)]
        {
            if let Some(_inner) = &self.inner {
                // Unix 实现中可以在后台线程监听
                // 这里简化处理，实际实现需要更复杂的逻辑
            }
        }
        #[cfg(windows)]
        {
            // Windows 命名互斥量不支持消息传递
        }
        #[cfg(not(any(windows, unix)))]
        {
            // WASM 平台不支持单实例激活回调
        }
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            if let Some(handle) = self.inner.take() {
                unsafe {
                    let _ = windows::Win32::Foundation::CloseHandle(handle);
                }
            }
        }
        #[cfg(unix)]
        {
            if let Some(inner) = self.inner.take() {
                let _ = std::fs::remove_file(&inner.socket_path);
            }
        }
    }
}

/// 向已运行的实例发送激活信号
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
        stream.write_all(b"activate")?;
        Ok(())
    }
    #[cfg(windows)]
    {
        // Windows 命名互斥量不支持消息传递
        let _ = app_id;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows 不支持向已有实例发送激活信号",
        ))
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
