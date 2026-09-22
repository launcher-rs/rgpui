//! vsync 节拍的按需帧源（Android 循环用）。
//!
//! 循环只在 GPUI 要帧**且** vsync 到时才画：
//! 需求经 [`schedule_frame`] 投一发 `AChoreographer` 回调，回调触发欠一帧，
//! 循环用 [`take_frame`] 取走欠帧 tick 一次 GPUI。
//!  input/主任务/生命周期都经各自 fd 进 looper，睡着也不丢事。
//!
//! 节拍本体在平台无关、可单测的 [`FramePacer`](super::frame_pacer)；
//! 本模块是线程局部实例 + vsync 线程 + looper 胶水。
//! 主机无循环线程：`resume`/`wake` 为空操作（接口与真机对齐）。

#[cfg(target_os = "android")]
use std::time::Duration;

/// 无事时循环最长阻塞多久；漏叫醒最多慢这么多，不会冻住。
#[cfg(target_os = "android")]
const IDLE_POLL_CAP: Duration = Duration::from_secs(1);

/// 拍假设（60Hz）：超拍的帧不等 vsync；无 choreographer 时挂钟按它打点。
#[cfg(target_os = "android")]
const REFRESH_INTERVAL: Duration = Duration::from_micros(16_667);

#[cfg(target_os = "android")]
mod real {
    use super::{IDLE_POLL_CAP, REFRESH_INTERVAL};
    use crate::frame_pacer::FramePacer;
    use std::{
        cell::RefCell,
        ptr,
        sync::{
            OnceLock,
            atomic::{AtomicBool, AtomicPtr, Ordering},
            mpsc,
        },
        time::{Duration, Instant},
    };

    thread_local! {
        /// 循环线程的节拍器（[`install`] 装）。
        static PACER: RefCell<Option<FramePacer>> = const { RefCell::new(None) };
    }

    /// 循环线程 looper（供跨线程 [`wake`](super::wake)）。
    static MAIN_LOOPER: AtomicPtr<ndk_sys::ALooper> = AtomicPtr::new(ptr::null_mut());

    /// vsync 线程 looper（无 choreographer 时空）。
    static VSYNC_LOOPER: AtomicPtr<ndk_sys::ALooper> = AtomicPtr::new(ptr::null_mut());

    /// vsync 线程（每进程一根），值表它是否找到 choreographer。
    static VSYNC_THREAD: OnceLock<bool> = OnceLock::new();

    /// 循环请 vsync 线程投帧回调。
    static POST_REQUESTED: AtomicBool = AtomicBool::new(false);

    /// 帧回调触发，欠帧尚未交给节拍器。
    static FRAME_DUE: AtomicBool = AtomicBool::new(false);

    /// 非循环线程提的需求；循环下轮转成 [`schedule_frame`](super::schedule_frame)。
    static OFF_THREAD_DEMAND: AtomicBool = AtomicBool::new(false);

    /// 在调用线程装节拍器（须有 looper；幂等，首次起 vsync 线程）。
    pub(crate) fn install() {
        // SAFETY：纯查询调用线程 looper。
        let looper = unsafe { ndk_sys::ALooper_forThread() };
        // 先清旧进程/旧 Activity 残留的帧需求，再换新 looper：
        // 残留需求会叫醒已销毁的旧 looper（FORTIFY abort）。
        POST_REQUESTED.store(false, Ordering::Release);
        FRAME_DUE.store(false, Ordering::Release);
        OFF_THREAD_DEMAND.store(false, Ordering::Release);
        MAIN_LOOPER.store(looper, Ordering::Release);

        let has_choreographer = *VSYNC_THREAD.get_or_init(spawn_vsync_thread);
        PACER.with(|slot| {
            if slot.borrow().is_some() {
                return;
            }
            let pacer = if has_choreographer {
                FramePacer::with_vsync(
                    Box::new(|| {
                        POST_REQUESTED.store(true, Ordering::Release);
                        wake_looper(&VSYNC_LOOPER);
                    }),
                    REFRESH_INTERVAL,
                )
            } else {
                log::warn!("frame_source：无 AChoreographer，按挂钟打点");
                FramePacer::with_clock(REFRESH_INTERVAL)
            };
            *slot.borrow_mut() = Some(pacer);
        });
    }

    /// 起 vsync 线程并等它报告有无 choreographer（调用方据此选节拍器）。
    fn spawn_vsync_thread() -> bool {
        let (ready_tx, ready_rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("rgpui-vsync".into())
            .spawn(move || vsync_thread(ready_tx));
        match spawned {
            Ok(_) => ready_rx.recv().unwrap_or(false),
            Err(error) => {
                log::warn!("frame_source：vsync 线程起不来：{error}");
                false
            }
        }
    }

    /// vsync 线程：等投帧请求，发 `AChoreographer` 回调。
    fn vsync_thread(ready: mpsc::Sender<bool>) {
        // SAFETY：新线程上调一次；looper 先备好，否则 choreographer 为空。
        let (looper, choreographer) = unsafe {
            let looper = ndk_sys::ALooper_prepare(0);
            let choreographer = if looper.is_null() {
                ptr::null_mut()
            } else {
                ndk_sys::AChoreographer_getInstance()
            };
            (looper, choreographer)
        };
        if choreographer.is_null() {
            let _ = ready.send(false);
            return;
        }
        VSYNC_LOOPER.store(looper, Ordering::Release);
        let _ = ready.send(true);

        loop {
            if POST_REQUESTED.swap(false, Ordering::AcqRel) {
                // SAFETY：choreographer 归本线程、与线程同寿命；回调不带数据。
                unsafe {
                    ndk_sys::AChoreographer_postFrameCallback(
                        choreographer,
                        Some(on_vsync),
                        ptr::null_mut(),
                    )
                };
            }
            // 帧回调（经本 looper 投递）或 POST_REQUESTED 叫醒时返回。
            // SAFETY：只在拥有该 looper 的线程调。
            unsafe {
                ndk_sys::ALooper_pollOnce(-1, ptr::null_mut(), ptr::null_mut(), ptr::null_mut())
            };
        }
    }

    /// 帧回调：欠一帧并叫醒主循环。
    unsafe extern "C" fn on_vsync(
        _frame_time_nanos: std::os::raw::c_long,
        _data: *mut std::ffi::c_void,
    ) {
        FRAME_DUE.store(true, Ordering::Release);
        wake_looper(&MAIN_LOOPER);
    }

    /// 叫醒指定 looper（空即无操作）。
    fn wake_looper(looper: &AtomicPtr<ndk_sys::ALooper>) {
        let looper = looper.load(Ordering::Acquire);
        if !looper.is_null() {
            // SAFETY：`ALooper_wake` 文档注明任意线程可调。
            unsafe { ndk_sys::ALooper_wake(looper) };
        }
    }

    /// 把 vsync 线程上报的欠帧先交节拍器，再跑 `f`。
    fn with_pacer<R>(f: impl FnOnce(&FramePacer) -> R) -> Option<R> {
        PACER.with(|slot| {
            let slot = slot.borrow();
            let pacer = slot.as_ref()?;
            if FRAME_DUE.swap(false, Ordering::AcqRel) {
                pacer.on_vsync();
            }
            Some(f(pacer))
        })
    }

    /// 要一帧（循环线程投回调；他线程记需求并叫醒循环代投）。
    pub(crate) fn schedule_frame() {
        if with_pacer(|pacer| pacer.schedule(Instant::now())).is_none() {
            OFF_THREAD_DEMAND.store(true, Ordering::Release);
            super::wake();
        }
    }

    /// surface 切换/前台回来后重投（后台前投的回调可能永不触发）。
    pub(crate) fn resume() {
        if with_pacer(|pacer| pacer.resume(Instant::now())).is_none() {
            schedule_frame();
        }
    }

    /// 循环本轮该不该 tick GPUI（每轮调一次，仅可画时调；不取则继续欠）。
    pub(crate) fn take_frame() -> bool {
        if OFF_THREAD_DEMAND.swap(false, Ordering::AcqRel) {
            schedule_frame();
        }
        with_pacer(|pacer| pacer.take_frame(Instant::now())).unwrap_or(false)
    }

    /// 循环本轮 looper 可阻塞多久（欠帧即零，否则到下个延后任务/ fallback 帧）。
    pub(crate) fn poll_timeout(next_delayed_task: Option<Instant>, can_draw: bool) -> Duration {
        let now = Instant::now();
        let mut timeout = IDLE_POLL_CAP;
        if let Some(due) = next_delayed_task {
            timeout = timeout.min(due.saturating_duration_since(now));
        }
        if can_draw {
            if let Some(Some(frame)) = with_pacer(|pacer| pacer.poll_timeout(now)) {
                timeout = timeout.min(frame);
            }
        }
        // 循环按整毫秒睡，向上取整防亚毫秒余数变零自旋。
        Duration::from_millis(timeout.as_micros().div_ceil(1000) as u64)
    }

    /// 跨线程叫醒循环（looper fd 之外的事到了，如键盘文本脏）。
    pub(crate) fn wake() {
        wake_looper(&MAIN_LOOPER);
    }
}

#[cfg(target_os = "android")]
pub(crate) use real::{install, poll_timeout, resume, schedule_frame, take_frame, wake};

/// 主机桩：无循环线程，`resume`/`wake` 为空操作（接口与真机对齐，
/// `window.rs` 可无 `cfg` 直调）。
#[cfg(not(target_os = "android"))]
mod stub {
    /// 重投帧需求（主机空操作）。
    pub(crate) fn resume() {}

    /// 叫醒循环（主机空操作）。
    pub(crate) fn wake() {}
}

#[cfg(not(target_os = "android"))]
pub(crate) use stub::{resume, wake};
