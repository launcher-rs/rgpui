use std::{
    cell::RefCell,
    hint::cold_path,
    time::{Duration, Instant},
};

use itertools::Itertools;

use crate::action::Action;

#[doc(hidden)]
#[derive(Clone)]
pub struct ActionStatistics {
    runtime_to_beat: Duration,
    longest_runtimes: heapless::Vec<ActionTiming, 5>,
}

impl std::fmt::Debug for ActionStatistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActionStatistics")
            .field("runtime_to_beat", &self.runtime_to_beat)
            .field("longest_runtimes", &self.longest_runtimes)
            .finish()
    }
}

impl std::fmt::Display for ActionStatistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Actions that blocked the longest\n")?;
        for action in self
            .longest_runtimes(true)
            .sorted_by_key(|action| action.runtime())
            .rev()
        {
            f.write_fmt(format_args!(
                "{:<20} - {}",
                format!("{:?}", action.runtime()),
                action.name
            ))?;
            writeln!(f)?;
        }
        Ok(())
    }
}

impl Default for ActionStatistics {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionStatistics {
    const fn new() -> Self {
        Self {
            runtime_to_beat: Duration::from_micros(100),
            longest_runtimes: heapless::Vec::new(),
        }
    }

    pub fn take(&mut self) -> Self {
        std::mem::take(self)
    }

    pub fn is_empty(&self) -> bool {
        self.longest_runtimes.is_empty()
    }

    pub fn longest_runtimes(&self, include_running: bool) -> impl Iterator<Item = ActionTiming> {
        self.longest_runtimes.iter().copied().chain(
            RUNNING_ACTIONS
                .with(|stack| stack.borrow().last().copied())
                .into_iter()
                .filter(move |_| include_running)
                .map(|(name, start)| ActionTiming {
                    name,
                    start,
                    end: Instant::now(),
                }),
        )
    }
}

#[doc(hidden)]
#[derive(Copy, Clone)]
pub struct ActionTiming {
    pub name: &'static str,
    pub start: Instant,
    pub end: Instant,
}

impl core::fmt::Debug for ActionTiming {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActionTiming")
            .field("name", &self.name)
            .field("runtime", &self.runtime())
            .finish()
    }
}

impl ActionTiming {
    pub fn duration(&self) -> Duration {
        self.end.saturating_duration_since(self.start)
    }
}

impl ActionTiming {
    #[doc(hidden)]
    pub fn runtime(&self) -> Duration {
        self.end - self.start
    }
}

static ACTION_STATISTICS: spin::Mutex<ActionStatistics> =
    const { spin::Mutex::new(ActionStatistics::new()) };

// 记录当前线程正在运行的 action 栈。之所以用线程局部变量而非全局的
// `running` 字段：action 分发可能在任意线程上进行（并行测试、后台任务），
// 若所有线程共享一个 `running`，两个线程交错执行
// `update_running_action`/`save_action_timing` 时会相互覆盖，导致
// `save_action_timing` 读到空值而 panic。每个线程各自维护栈即可避免竞争。
thread_local! {
    static RUNNING_ACTIONS: RefCell<Vec<(&'static str, Instant)>> = const {
        RefCell::new(Vec::new())
    };
}

#[doc(hidden)]
pub(crate) fn update_running_action(action: &(dyn Action + 'static), cx: &mut crate::App) {
    let now = Instant::now();
    let action = action.type_id();
    let action = cx.actions.try_resolve_action(&action).unwrap_or("un-named");
    RUNNING_ACTIONS.with(|stack| stack.borrow_mut().push((action, now)));
}

#[doc(hidden)]
pub(crate) fn save_action_timing() {
    let (action, started) = RUNNING_ACTIONS
        .with(|stack| stack.borrow_mut().pop())
        .expect("only called after `update_running_action`");
    let now = Instant::now();
    let runtime = now.duration_since(started);
    let mut statistics = ACTION_STATISTICS.lock();
    if runtime >= statistics.runtime_to_beat {
        cold_path();
        if statistics.longest_runtimes.is_full()
            && let Some(to_replace) = statistics
                .longest_runtimes
                .iter_mut()
                .min_by_key(|action| runtime >= action.runtime())
        {
            *to_replace = ActionTiming {
                name: action,
                start: started,
                end: now,
            };
        } else {
            statistics
                .longest_runtimes
                .push(ActionTiming {
                    name: action,
                    start: started,
                    end: now,
                })
                .expect("just checked it is not full");
        };
        statistics.runtime_to_beat = statistics
            .longest_runtimes
            .iter()
            .map(|action| action.runtime())
            .min()
            .expect("never empty");
    }
}

#[doc(hidden)]
pub fn take_action_stats() -> ActionStatistics {
    ACTION_STATISTICS.lock().take()
}
