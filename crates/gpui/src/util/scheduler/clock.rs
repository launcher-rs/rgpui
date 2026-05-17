use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use std::time::Duration;

pub use web_time::Instant;

/// 用于获取当前时间的时钟 trait。
pub trait Clock {
    /// 获取当前 UTC 时间。
    fn utc_now(&self) -> DateTime<Utc>;
    /// 获取当前瞬时时间。
    fn now(&self) -> Instant;
}

/// 用于测试的可控时钟。
pub struct TestClock(Mutex<TestClockState>);

/// TestClock 的内部状态。
struct TestClockState {
    now: Instant,
    utc_now: DateTime<Utc>,
}

impl TestClock {
    /// 创建一个新的 TestClock，初始时间为 2025-07-01T23:59:58。
    pub fn new() -> Self {
        const START_TIME: &str = "2025-07-01T23:59:58-00:00";
        let utc_now = DateTime::parse_from_rfc3339(START_TIME).unwrap().to_utc();
        Self(Mutex::new(TestClockState {
            now: Instant::now(),
            utc_now,
        }))
    }

    /// 设置当前的 UTC 时间。
    pub fn set_utc_now(&self, now: DateTime<Utc>) {
        let mut state = self.0.lock();
        state.utc_now = now;
    }

    /// 将时钟向前推进指定的持续时间。
    pub fn advance(&self, duration: Duration) {
        let mut state = self.0.lock();
        state.now += duration;
        state.utc_now += duration;
    }
}

impl Clock for TestClock {
    fn utc_now(&self) -> DateTime<Utc> {
        self.0.lock().utc_now
    }

    fn now(&self) -> Instant {
        self.0.lock().now
    }
}
