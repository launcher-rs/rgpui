use crate::scheduler::Instant;
use std::{
    cell::LazyCell,
    collections::{HashMap, VecDeque},
    hash::{DefaultHasher, Hash, Hasher},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::ThreadId,
};

use serde::{Deserialize, Serialize};

use crate::SharedString;

#[doc(hidden)]
#[derive(Debug, Copy, Clone)]
pub struct TaskTiming {
    pub location: &'static core::panic::Location<'static>,
    pub start: Instant,
    pub end: Option<Instant>,
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct ThreadTaskTimings {
    pub thread_name: Option<String>,
    pub thread_id: ThreadId,
    pub timings: Vec<TaskTiming>,
    pub total_pushed: u64,
}

impl ThreadTaskTimings {
    /// 将全局线程计时信息转换为其结构化格式。
    pub fn convert(timings: &[GlobalThreadTimings]) -> Vec<Self> {
        timings
            .iter()
            .filter_map(|t| match t.timings.upgrade() {
                Some(timings) => Some((t.thread_id, timings)),
                _ => None,
            })
            .map(|(thread_id, timings)| {
                let timings = timings.lock();
                let thread_name = timings.thread_name.clone();
                let total_pushed = timings.total_pushed;
                let timings = &timings.timings;

                let mut vec = Vec::with_capacity(timings.len());
                let (s1, s2) = timings.as_slices();
                vec.extend_from_slice(s1);
                vec.extend_from_slice(s2);

                ThreadTaskTimings {
                    thread_name,
                    thread_id,
                    timings: vec,
                    total_pushed,
                }
            })
            .collect()
    }
}

/// [`core::panic::Location`] 的可序列化变体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedLocation {
    /// 源文件名称
    pub file: SharedString,
    /// 源文件中的行号
    pub line: u32,
    /// 源文件中的列号
    pub column: u32,
}

impl From<&core::panic::Location<'static>> for SerializedLocation {
    fn from(value: &core::panic::Location<'static>) -> Self {
        SerializedLocation {
            file: value.file().into(),
            line: value.line(),
            column: value.column(),
        }
    }
}

/// [`TaskTiming`] 的可序列化变体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedTaskTiming {
    /// 计时信息的位置
    pub location: SerializedLocation,
    /// 报告测量值的时间（以纳秒为单位）
    pub start: u128,
    /// 测量持续时间（以纳秒为单位）
    pub duration: u128,
}

impl SerializedTaskTiming {
    /// 将 [`TaskTiming`] 数组转换为其可序列化格式
    ///
    /// # 参数
    ///
    /// `anchor` - 应早于所有计时信息的 [`Instant`]，用作基础锚点
    pub fn convert(anchor: Instant, timings: &[TaskTiming]) -> Vec<SerializedTaskTiming> {
        let serialized = timings
            .iter()
            .map(|timing| {
                let start = timing.start.duration_since(anchor).as_nanos();
                let duration = timing
                    .end
                    .unwrap_or_else(|| Instant::now())
                    .duration_since(timing.start)
                    .as_nanos();
                SerializedTaskTiming {
                    location: timing.location.into(),
                    start,
                    duration,
                }
            })
            .collect::<Vec<_>>();

        serialized
    }
}

/// [`ThreadTaskTimings`] 的可序列化变体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedThreadTaskTimings {
    /// 线程名称
    pub thread_name: Option<String>,
    /// 线程 ID 的哈希值
    pub thread_id: u64,
    /// 此线程的计时记录
    pub timings: Vec<SerializedTaskTiming>,
}

impl SerializedThreadTaskTimings {
    /// 将 [`ThreadTaskTimings`] 转换为其可序列化格式
    ///
    /// # 参数
    ///
    /// `anchor` - 应早于所有计时信息的 [`Instant`]，用作基础锚点
    pub fn convert(anchor: Instant, timings: ThreadTaskTimings) -> SerializedThreadTaskTimings {
        let serialized_timings = SerializedTaskTiming::convert(anchor, &timings.timings);

        let mut hasher = DefaultHasher::new();
        timings.thread_id.hash(&mut hasher);
        let thread_id = hasher.finish();

        SerializedThreadTaskTimings {
            thread_name: timings.thread_name,
            thread_id,
            timings: serialized_timings,
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct ThreadTimingsDelta {
    /// 哈希后的线程 ID
    pub thread_id: u64,
    /// 线程名称（如果已知）
    pub thread_name: Option<String>,
    /// 自上次调用以来的新计时信息。如果循环缓冲区自上次轮询以来已环绕，
    /// 则某些条目可能已丢失。
    pub new_timings: Vec<SerializedTaskTiming>,
}

/// 跟踪哪些计时事件已被查看，以便调用者可以请求仅未查看的事件。
#[doc(hidden)]
pub struct ProfilingCollector {
    startup_time: Instant,
    cursors: HashMap<ThreadId, u64>,
}

impl ProfilingCollector {
    pub fn new(startup_time: Instant) -> Self {
        Self {
            startup_time,
            cursors: HashMap::default(),
        }
    }

    pub fn startup_time(&self) -> Instant {
        self.startup_time
    }

    pub fn collect_unseen(
        &mut self,
        all_timings: Vec<ThreadTaskTimings>,
    ) -> Vec<ThreadTimingsDelta> {
        let mut deltas = Vec::with_capacity(all_timings.len());

        for thread in all_timings {
            let mut hasher = DefaultHasher::new();
            thread.thread_id.hash(&mut hasher);
            let hashed_id = hasher.finish();

            let prev_cursor = self.cursors.get(&thread.thread_id).copied().unwrap_or(0);
            let buffer_len = thread.timings.len() as u64;
            let buffer_start = thread.total_pushed.saturating_sub(buffer_len);

            let mut slice = if prev_cursor < buffer_start {
                // 游标落后于缓冲区 —— 某些条目已被驱逐。
                // 返回缓冲区中仍存在的所有内容。
                thread.timings.as_slice()
            } else {
                let skip = (prev_cursor - buffer_start) as usize;
                &thread.timings[skip.min(thread.timings.len())..]
            };

            // 如果最后一个条目仍在进行中（end: None），则不发出。
            let incomplete_at_end = slice.last().is_some_and(|t| t.end.is_none());
            if incomplete_at_end {
                slice = &slice[..slice.len() - 1];
            }

            let cursor_advance = if incomplete_at_end {
                thread.total_pushed.saturating_sub(1)
            } else {
                thread.total_pushed
            };

            self.cursors.insert(thread.thread_id, cursor_advance);

            if slice.is_empty() {
                continue;
            }

            let new_timings = SerializedTaskTiming::convert(self.startup_time, slice);

            deltas.push(ThreadTimingsDelta {
                thread_id: hashed_id,
                thread_name: thread.thread_name,
                new_timings,
            });
        }

        deltas
    }

    pub fn reset(&mut self) {
        self.cursors.clear();
    }
}

// 允许 16MiB 的任务计时条目。
// VecDeque 在满时通过翻倍容量增长，因此将其保持为 2 的幂以避免浪费内存。
const MAX_TASK_TIMINGS: usize = (16 * 1024 * 1024) / core::mem::size_of::<TaskTiming>();

#[doc(hidden)]
pub(crate) type TaskTimings = VecDeque<TaskTiming>;

#[doc(hidden)]
pub type GuardedTaskTimings = spin::Mutex<ThreadTimings>;

#[doc(hidden)]
pub struct GlobalThreadTimings {
    pub thread_id: ThreadId,
    pub timings: std::sync::Weak<GuardedTaskTimings>,
}

#[doc(hidden)]
pub static GLOBAL_THREAD_TIMINGS: spin::Mutex<Vec<GlobalThreadTimings>> =
    spin::Mutex::new(Vec::new());

thread_local! {
    #[doc(hidden)]
    pub static THREAD_TIMINGS: LazyCell<Arc<GuardedTaskTimings>> = LazyCell::new(|| {
        let current_thread = std::thread::current();
        let thread_name = current_thread.name();
        let thread_id = current_thread.id();
        let timings = ThreadTimings::new(thread_name.map(|e| e.to_string()), thread_id);
        let timings = Arc::new(spin::Mutex::new(timings));

        {
            let timings = Arc::downgrade(&timings);
            let global_timings = GlobalThreadTimings {
                thread_id: std::thread::current().id(),
                timings,
            };
            GLOBAL_THREAD_TIMINGS.lock().push(global_timings);
        }

        timings
    });
}

#[doc(hidden)]
pub struct ThreadTimings {
    pub thread_name: Option<String>,
    pub thread_id: ThreadId,
    pub timings: TaskTimings,
    pub total_pushed: u64,
}

impl ThreadTimings {
    pub fn new(thread_name: Option<String>, thread_id: ThreadId) -> Self {
        ThreadTimings {
            thread_name,
            thread_id,
            timings: TaskTimings::new(),
            total_pushed: 0,
        }
    }

    /// 如果此任务与最后一个任务相同，则更新最后一个任务的结束时间。
    ///
    /// 否则，将新任务计时添加到列表中。
    pub fn add_task_timing(&mut self, timing: TaskTiming) {
        if let Some(last_timing) = self.timings.back_mut()
            && last_timing.location == timing.location
            && last_timing.start == timing.start
        {
            last_timing.end = timing.end;
        } else {
            while self.timings.len() + 1 > MAX_TASK_TIMINGS {
                // This should only ever pop one element because it matches the insertion below.
                self.timings.pop_front();
            }
            self.timings.push_back(timing);
            self.total_pushed += 1;
        }
    }

    pub fn get_thread_task_timings(&self) -> ThreadTaskTimings {
        ThreadTaskTimings {
            thread_name: self.thread_name.clone(),
            thread_id: self.thread_id,
            timings: self.timings.iter().cloned().collect(),
            total_pushed: self.total_pushed,
        }
    }
}

impl Drop for ThreadTimings {
    fn drop(&mut self) {
        let mut thread_timings = GLOBAL_THREAD_TIMINGS.lock();

        let Some((index, _)) = thread_timings
            .iter()
            .enumerate()
            .find(|(_, t)| t.thread_id == self.thread_id)
        else {
            return;
        };
        thread_timings.swap_remove(index);
    }
}

#[doc(hidden)]
pub fn add_task_timing(timing: TaskTiming) {
    if !PROFILER_ENABLED.load(Ordering::Acquire) {
        return;
    }
    THREAD_TIMINGS.with(|timings| {
        timings.lock().add_task_timing(timing);
    });
}

#[doc(hidden)]
pub fn get_current_thread_task_timings() -> ThreadTaskTimings {
    THREAD_TIMINGS.with(|timings| timings.lock().get_thread_task_timings())
}

static PROFILER_ENABLED: AtomicBool = AtomicBool::new(false);

/// 在运行时启用或禁用任务计时收集。
///
/// 从启用转换到禁用时，`add_task_timing` 变为
/// 无操作，并清除每个线程的缓冲区，以便在稍后重新启用后不会报告陈旧数据。使用当前值调用是无操作的。
pub fn set_enabled(enabled: bool) -> bool {
    if PROFILER_ENABLED.swap(enabled, Ordering::AcqRel) == enabled {
        return false;
    }

    if !enabled {
        for global in GLOBAL_THREAD_TIMINGS.lock().iter() {
            if let Some(timings) = global.timings.upgrade() {
                let mut timings = timings.lock();
                timings.timings.clear();
                timings.timings.shrink_to_fit();
                timings.total_pushed = 0;
            }
        }
    }
    true
}
