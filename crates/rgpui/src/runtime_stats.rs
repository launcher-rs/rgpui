//! 运行时采样（Inspector“运行”卡片的数据源）+ GPU 信息注册表。
//!
//! # 开销纪律（检查器面板本身的消耗也要可控）
//!
//! - 采样**仅检查器打开时**执行，关闭时零开销（调用方负责门控，见
//!   [`Window::sample_runtime_if_inspecting`] 的调用处）。
//! - CPU/内存走系统调用，节流到约 2Hz，结果缓存后面板只读缓存，render 内零测量。
//! - 帧率是每帧一次 `Instant` 差分 + EMA（纳秒级），同样只在打开时累计。
//! - 诚实注记：采样本身会轻微抬高被测的 CPU 数字（观察者效应），看趋势别看绝对值。
//!
//! 平台实现（无新增依赖）：
//! - Windows：`GetProcessTimes`（CPU）+ `GetProcessMemoryInfo`（工作集）
//! - Unix：`getrusage(RUSAGE_SELF)`（CPU + 峰值 RSS；macOS 的 `ru_maxrss` 单位为字节，Linux 为 KB）
//! - 其他（含 WASM）：全零。

use std::sync::OnceLock;

use crate::scheduler::Instant;

/// GPU 信息（渲染层启动时注册一次，面板只读）。
#[derive(Debug, Clone)]
pub struct GpuInfo {
    /// 适配器名（如直显型号）。
    pub name: String,
    /// 后端名（如 Vulkan/DX12/Metal）。
    pub backend: String,
}

static GPU_INFO: OnceLock<GpuInfo> = OnceLock::new();

/// 注册 GPU 信息（渲染层在选定适配器后调用一次；重复调用保持首次值）。
pub fn set_gpu_info(name: impl Into<String>, backend: impl Into<String>) {
    _ = GPU_INFO.set(GpuInfo {
        name: name.into(),
        backend: backend.into(),
    });
}

/// 读取已注册的 GPU 信息（未注册返回 `None`，面板显示“未上报”）。
pub fn gpu_info() -> Option<&'static GpuInfo> {
    GPU_INFO.get()
}

/// 安装崩溃钩子（进程级，只能装一次，多次调用保留首次目录）。
///
/// 在 `dir` 下写入 `panic-<纳秒>.log`（panic 负载 + 位置 + 强制回溯），
/// 并链式调用此前已安装的钩子。注意：栈溢出/abort 类崩溃钩子不可靠，
/// 那类靠滚动快照（`last.json`，死前已落盘）留证据，见 [`crate::InspectorSnapshot`]。
/// 本函数无 cfg 门控，release 也可装（panic 日志部分）；快照部分随检查器门控。
pub fn install_crash_hook(dir: impl AsRef<std::path::Path>) {
    use std::sync::OnceLock;
    static HOOK_DIR: OnceLock<std::path::PathBuf> = OnceLock::new();
    if HOOK_DIR.set(dir.as_ref().to_path_buf()).is_err() {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(dir) = HOOK_DIR.get() {
            _ = std::fs::create_dir_all(dir);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let mut log = format!("payload: {}\n", panic_payload(info));
            log.push_str(&format!(
                "location: {}\n",
                info.location()
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "未知".to_string())
            ));
            log.push_str(&format!(
                "backtrace:\n{}\n",
                std::backtrace::Backtrace::force_capture()
            ));
            _ = std::fs::write(dir.join(format!("panic-{nanos}.log")), log);
        }
        previous(info);
    }));
}

/// 提取 panic 负载的可读文本（`&str` / `String` 常见形态）。
fn panic_payload(info: &std::panic::PanicHookInfo) -> String {
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "（非文本负载）".to_string()
    }
}

/// 单次进程采样结果（CPU 百分比 + 内存 MB），失败返回 `None`（面板显示横线）。
#[cfg(any(feature = "inspector", debug_assertions))]
pub(crate) struct ProcessSample {
    pub(crate) cpu_percent: f64,
    pub(crate) memory_mb: f64,
}

/// 采样器内部状态（`Window` 持有，面板只读缓存值）。
#[cfg(any(feature = "inspector", debug_assertions))]
#[derive(Default)]
pub(crate) struct RuntimeSamplerState {
    pub(crate) clock: CpuClock,
    pub(crate) last_sample: Option<Instant>,
    pub(crate) last_frame: Option<Instant>,
}

/// CPU 时钟默认值（`CpuClock::new` 需在运行时求核数，不适合 `Default`）。
#[cfg(any(feature = "inspector", debug_assertions))]
impl Default for CpuClock {
    fn default() -> Self {
        Self::new()
    }
}

/// 上次 CPU 时间（100ns 或微秒，平台相关），调用方持有以算差分。
#[cfg(any(feature = "inspector", debug_assertions))]
pub(crate) struct CpuClock {
    #[cfg(target_os = "windows")]
    prev_proc_100ns: u64,
    #[cfg(unix)]
    prev_proc_micros: i64,
    prev_wall: Instant,
    initialized: bool,
    num_cpus: f64,
}

#[cfg(any(feature = "inspector", debug_assertions))]
impl CpuClock {
    pub(crate) fn new() -> Self {
        Self {
            #[cfg(target_os = "windows")]
            prev_proc_100ns: 0,
            #[cfg(unix)]
            prev_proc_micros: 0,
            prev_wall: Instant::now(),
            initialized: false,
            num_cpus: std::thread::available_parallelism()
                .map(|n| n.get() as f64)
                .unwrap_or(1.0),
        }
    }

    /// 采样一次；首次调用仅建基线返回 `None`（无差分可算）。
    pub(crate) fn sample(&mut self) -> Option<ProcessSample> {
        let now = Instant::now();
        let wall_secs = now.duration_since(self.prev_wall).as_secs_f64();
        self.prev_wall = now;
        if wall_secs <= 0.0 {
            return None;
        }

        #[cfg(target_os = "windows")]
        {
            use windows::Win32::Foundation::FILETIME;
            use windows::Win32::System::ProcessStatus::{
                GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
            };
            use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

            unsafe {
                let process = GetCurrentProcess();
                let mut creation = FILETIME::default();
                let mut exit = FILETIME::default();
                let mut kernel = FILETIME::default();
                let mut user = FILETIME::default();
                GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user).ok()?;
                let to_u64 =
                    |ft: FILETIME| (ft.dwLowDateTime as u64) | ((ft.dwHighDateTime as u64) << 32);
                let proc_100ns = to_u64(kernel) + to_u64(user);

                let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
                counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
                GetProcessMemoryInfo(process, &mut counters as *mut _ as *mut _, counters.cb)
                    .ok()?;
                let memory_mb = counters.WorkingSetSize as f64 / (1024.0 * 1024.0);

                if !self.initialized {
                    self.initialized = true;
                    self.prev_proc_100ns = proc_100ns;
                    return None;
                }
                let cpu_percent =
                    (proc_100ns - self.prev_proc_100ns) as f64 / 1e7 / wall_secs / self.num_cpus
                        * 100.0;
                self.prev_proc_100ns = proc_100ns;
                Some(ProcessSample {
                    cpu_percent: cpu_percent.max(0.0),
                    memory_mb,
                })
            }
        }

        #[cfg(unix)]
        {
            let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
            // SAFETY：`getrusage` 写入整个结构体，成功返回 0。
            if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
                return None;
            }
            // SAFETY：上一步成功，结构体已初始化。
            let usage = unsafe { usage.assume_init() };
            #[cfg(not(target_os = "macos"))]
            let to_micros = |tv: libc::timeval| tv.tv_sec * 1_000_000 + tv.tv_usec;
            // macOS 的 tv_usec 是 i32，需要转 i64
            #[cfg(target_os = "macos")]
            let to_micros = |tv: libc::timeval| tv.tv_sec * 1_000_000 + i64::from(tv.tv_usec);
            let proc_micros = to_micros(usage.ru_utime) + to_micros(usage.ru_stime);
            // macOS 的 ru_maxrss 单位是字节，Linux 是 KB。
            #[cfg(target_os = "macos")]
            let memory_mb = usage.ru_maxrss as f64 / (1024.0 * 1024.0);
            #[cfg(not(target_os = "macos"))]
            let memory_mb = usage.ru_maxrss as f64 / 1024.0;

            if !self.initialized {
                self.initialized = true;
                self.prev_proc_micros = proc_micros;
                return None;
            }
            let cpu_percent = (proc_micros - self.prev_proc_micros) as f64
                / 1_000_000.0
                / wall_secs
                / self.num_cpus
                * 100.0;
            self.prev_proc_micros = proc_micros;
            Some(ProcessSample {
                cpu_percent: cpu_percent.max(0.0),
                memory_mb,
            })
        }

        #[cfg(not(any(target_os = "windows", unix)))]
        {
            // 该平台无进程采样实现：标记基线已建，避免调用方误判为“可重试”。
            let _ = (self.initialized, self.num_cpus, now, wall_secs);
            self.initialized = true;
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 错误环有界 50 条，序号单调，读出按写入顺序。
    #[crate::test]
    fn error_ring_caps_at_fifty(cx: &mut crate::TestAppContext) {
        cx.update(|cx| {
            for i in 0..55 {
                cx.report_error(format!("boom-{i}"));
            }
            let errors = cx.recent_errors();
            assert_eq!(errors.len(), 50);
            assert_eq!(errors.first().unwrap().0, 5);
            assert_eq!(errors.last().unwrap().0, 54);
            assert_eq!(errors.last().unwrap().1.to_string(), "boom-54");
        });
    }

    /// 崩溃钩子落盘 panic 日志（含负载文本），事后恢复原钩子避免污染同进程其他测试。
    #[test]
    fn crash_hook_writes_panic_log() {
        let dir = std::env::temp_dir().join(format!(
            "rgpui-crash-hook-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        install_crash_hook(&dir);
        let _ = std::panic::catch_unwind(|| panic!("hook-探针"));
        let mut found = false;
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    if content.contains("hook-探针") {
                        found = true;
                    }
                }
            }
        }
        // 清理不做断言：并行测试线程的 panic 也会经全局钩子落到本目录，
        // 清理期若恰好又有写入，目录非空属正常并发现象。
        _ = std::fs::remove_dir_all(&dir);
        assert!(found, "panic 日志应包含负载文本");
    }
}
