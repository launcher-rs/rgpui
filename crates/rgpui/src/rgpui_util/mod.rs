#![allow(missing_docs)]

//! GPUI 内部工具函数。

use std::{
    env,
    ops::AddAssign,
    panic::Location,
    pin::Pin,
    sync::OnceLock,
    task::{Context, Poll},
    time::Instant,
};

pub mod arc_cow;

/// 递增一个值并返回先前的值
pub fn post_inc<T: From<u8> + AddAssign<T> + Copy>(value: &mut T) -> T {
    let prev = *value;
    *value += T::from(1);
    prev
}

/// 如果设置了 `ZED_MEASUREMENTS` 环境变量，则测量闭包的执行时间
pub fn measure<R>(label: &str, f: impl FnOnce() -> R) -> R {
    static ZED_MEASUREMENTS: OnceLock<bool> = OnceLock::new();
    let zed_measurements = ZED_MEASUREMENTS.get_or_init(|| {
        env::var("ZED_MEASUREMENTS")
            .map(|measurements| measurements == "1" || measurements == "true")
            .unwrap_or(false)
    });

    if *zed_measurements {
        let start = Instant::now();
        let result = f();
        let elapsed = start.elapsed();
        eprintln!("{}: {:?}", label, elapsed);
        result
    } else {
        f()
    }
}

/// 在调试模式下 panic，在发布模式下记录错误及回溯
#[macro_export]
macro_rules! debug_panic {
    ( $($fmt_arg:tt)* ) => {
        if cfg!(debug_assertions) {
            panic!( $($fmt_arg)* );
        } else {
            let backtrace = std::backtrace::Backtrace::capture();
            log::error!("{}\n{:?}", format_args!($($fmt_arg)*), backtrace);
        }
    };
}

/// 返回选项值，如果在调试模式下为 None 则 panic
#[track_caller]
pub fn some_or_debug_panic<T>(option: Option<T>) -> Option<T> {
    #[cfg(debug_assertions)]
    if option.is_none() {
        panic!("Unexpected None");
    }
    option
}

/// 展开为立即调用的函数表达式。适用于在不返回 Option 或 Result 的函数中使用 ? 运算符
#[macro_export]
macro_rules! maybe {
    ($block:block) => {
        (|| $block)()
    };
    (async $block:block) => {
        (async || $block)()
    };
    (async move $block:block) => {
        (async move || $block)()
    };
}
/// Result 类型的扩展特征，提供日志工具
pub trait ResultExt<E> {
    /// Result 的 Ok 类型
    type Ok;

    /// 在 Error 级别记录错误，如果为 Err 则返回 None
    fn log_err(self) -> Option<Self::Ok>;
    /// 使用 Debug 格式（回溯）记录错误，如果为 Err 则返回 None
    fn log_err_with_backtrace(self) -> Option<Self::Ok>
    where
        E: std::fmt::Debug;
    /// 断言此结果在开发中不应为错误
    fn debug_assert_ok(self, reason: &str) -> Self;
    /// 在 Warn 级别记录错误，如果为 Err 则返回 None
    fn warn_on_err(self) -> Option<Self::Ok>;
    /// 在指定级别记录错误，如果为 Err 则返回 None
    fn log_with_level(self, level: log::Level) -> Option<Self::Ok>;
    /// 将错误转换为 anyhow::Error
    fn anyhow(self) -> anyhow::Result<Self::Ok>
    where
        E: Into<anyhow::Error>;
}

impl<T, E> ResultExt<E> for Result<T, E>
where
    E: std::fmt::Display,
{
    type Ok = T;

    #[track_caller]
    fn log_err(self) -> Option<T> {
        self.log_with_level(log::Level::Error)
    }

    #[track_caller]
    fn log_err_with_backtrace(self) -> Option<T>
    where
        E: std::fmt::Debug,
    {
        match self {
            Ok(value) => Some(value),
            Err(error) => {
                log_error_with_caller(
                    *Location::caller(),
                    DebugAsDisplay(&error),
                    log::Level::Error,
                );
                None
            }
        }
    }

    #[track_caller]
    fn debug_assert_ok(self, reason: &str) -> Self {
        if let Err(error) = &self {
            debug_panic!("{reason} - {error:#}");
        }
        self
    }

    #[track_caller]
    fn warn_on_err(self) -> Option<T> {
        self.log_with_level(log::Level::Warn)
    }

    #[track_caller]
    fn log_with_level(self, level: log::Level) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(error) => {
                log_error_with_caller(*Location::caller(), error, level);
                None
            }
        }
    }

    fn anyhow(self) -> anyhow::Result<T>
    where
        E: Into<anyhow::Error>,
    {
        self.map_err(Into::into)
    }
}

fn log_error_with_caller<E>(caller: core::panic::Location<'_>, error: E, level: log::Level)
where
    E: std::fmt::Display,
{
    #[cfg(not(windows))]
    let file = caller.file();
    #[cfg(windows)]
    let file = caller.file().replace('\\', "/");
    let file = file.split_once("crates/");
    let target = file.as_ref().and_then(|(_, s)| s.split_once("/src/"));

    let module_path = target.map(|(krate, module)| {
        if module.starts_with(krate) {
            module.trim_end_matches(".rs").replace('/', "::")
        } else {
            krate.to_owned() + "::" + &module.trim_end_matches(".rs").replace('/', "::")
        }
    });
    let file = file.map(|(_, file)| format!("crates/{file}"));
    log::logger().log(
        &log::Record::builder()
            .target(module_path.as_deref().unwrap_or(""))
            .module_path(file.as_deref())
            .args(format_args!("{:#}", error))
            .file(Some(caller.file()))
            .line(Some(caller.line()))
            .level(level)
            .build(),
    );
}

pub fn log_err<E: std::fmt::Display>(error: &E) {
    log_error_with_caller(*Location::caller(), error, log::Level::Error);
}

struct DebugAsDisplay<'a, E>(&'a E);

impl<E: std::fmt::Debug> std::fmt::Display for DebugAsDisplay<'_, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

/// Future 类型的扩展特征，提供日志工具
pub trait TryFutureExt {
    /// 在 Error 级别记录错误，如果为 Err 则返回 None
    fn log_err(self) -> LogErrorFuture<Self>
    where
        Self: Sized;

    /// 在 Error 级别记录错误并跟踪位置
    fn log_tracked_err(self, location: core::panic::Location<'static>) -> LogErrorFuture<Self>
    where
        Self: Sized;

    /// 在 Warn 级别记录错误，如果为 Err 则返回 None
    fn warn_on_err(self) -> LogErrorFuture<Self>
    where
        Self: Sized;
    /// 解包结果，如果为 Err 则 panic
    fn unwrap(self) -> UnwrapFuture<Self>
    where
        Self: Sized;
}

/// Future 类型的扩展特征，提供回溯日志
pub trait TryFutureExtBacktrace {
    /// 使用 Debug 格式（回溯）记录错误，如果为 Err 则返回 None
    fn log_err_with_backtrace(self) -> LogErrorWithBacktraceFuture<Self>
    where
        Self: Sized;

    /// 使用 Debug 格式记录错误并跟踪位置
    fn log_tracked_err_with_backtrace(
        self,
        location: core::panic::Location<'static>,
    ) -> LogErrorWithBacktraceFuture<Self>
    where
        Self: Sized;
}

impl<F, T, E> TryFutureExt for F
where
    F: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    #[track_caller]
    fn log_err(self) -> LogErrorFuture<Self>
    where
        Self: Sized,
    {
        let location = Location::caller();
        LogErrorFuture(self, log::Level::Error, *location)
    }

    fn log_tracked_err(self, location: core::panic::Location<'static>) -> LogErrorFuture<Self>
    where
        Self: Sized,
    {
        LogErrorFuture(self, log::Level::Error, location)
    }

    #[track_caller]
    fn warn_on_err(self) -> LogErrorFuture<Self>
    where
        Self: Sized,
    {
        let location = Location::caller();
        LogErrorFuture(self, log::Level::Warn, *location)
    }

    fn unwrap(self) -> UnwrapFuture<Self>
    where
        Self: Sized,
    {
        UnwrapFuture(self)
    }
}

impl<F, T, E> TryFutureExtBacktrace for F
where
    F: Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    #[track_caller]
    fn log_err_with_backtrace(self) -> LogErrorWithBacktraceFuture<Self>
    where
        Self: Sized,
    {
        let location = Location::caller();
        LogErrorWithBacktraceFuture(self, log::Level::Error, *location)
    }

    fn log_tracked_err_with_backtrace(
        self,
        location: core::panic::Location<'static>,
    ) -> LogErrorWithBacktraceFuture<Self>
    where
        Self: Sized,
    {
        LogErrorWithBacktraceFuture(self, log::Level::Error, location)
    }
}

/// 当内部 future 解析为 Err 时在指定级别记录错误的 future
#[must_use]
pub struct LogErrorFuture<F>(F, log::Level, core::panic::Location<'static>);

impl<F, T, E> Future for LogErrorFuture<F>
where
    F: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let level = self.1;
        let location = self.2;
        let inner = unsafe { Pin::new_unchecked(&mut self.get_unchecked_mut().0) };
        match inner.poll(cx) {
            Poll::Ready(output) => Poll::Ready(match output {
                Ok(output) => Some(output),
                Err(error) => {
                    log_error_with_caller(location, error, level);
                    None
                }
            }),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// 当内部 future 解析为 Err 时使用 Debug 格式（回溯）记录错误的 future
#[must_use]
pub struct LogErrorWithBacktraceFuture<F>(F, log::Level, core::panic::Location<'static>);

impl<F, T, E> Future for LogErrorWithBacktraceFuture<F>
where
    F: Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let level = self.1;
        let location = self.2;
        let inner = unsafe { Pin::new_unchecked(&mut self.get_unchecked_mut().0) };
        match inner.poll(cx) {
            Poll::Ready(output) => Poll::Ready(match output {
                Ok(output) => Some(output),
                Err(error) => {
                    log_error_with_caller(location, DebugAsDisplay(&error), level);
                    None
                }
            }),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// 解包结果的 future，如果为 Err 则 panic
pub struct UnwrapFuture<F>(F);

impl<F, T, E> Future for UnwrapFuture<F>
where
    F: Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let inner = unsafe { Pin::new_unchecked(&mut self.get_unchecked_mut().0) };
        match inner.poll(cx) {
            Poll::Ready(result) => Poll::Ready(result.unwrap()),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// 丢弃时运行闭包的守卫，除非被中止
pub struct Deferred<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> Deferred<F> {
    /// 丢弃时不运行延迟函数
    pub fn abort(mut self) {
        self.0.take();
    }
}

impl<F: FnOnce()> Drop for Deferred<F> {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f()
        }
    }
}

/// 运行给定函数，当返回值被丢弃时（除非被中止）
#[must_use]
pub fn defer<F: FnOnce()>(f: F) -> Deferred<F> {
    Deferred(Some(f))
}
