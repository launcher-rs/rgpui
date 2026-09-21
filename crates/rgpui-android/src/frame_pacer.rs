//! 帧需求门控（平台无关）：有自驱循环的宿主不需要它，
//! 跑 `ALooper` 的 Android 循环用它决定**何时**画。
//!
//! 需求到来即投一发 vsync 回调（`AChoreographer`），回调触发即欠一帧，
//! 循环一欠一还；回调已投/已欠期间的新需求折叠进去，
//! 两次 vsync 之间被通知五十次也只画一次。
//!
//! 上一帧耗时超过一整拍时不再等 vsync（等也是白等，还拖慢输入采样），
//! 直接到期；掉回拍内即恢复 vsync 节拍。无 vsync 源（单测）时退化为挂钟。
//!
//! 仅主线程使用（与 GPUI 其余部分一致）。

use std::{
    cell::Cell,
    time::{Duration, Instant},
};

/// 投一发 vsync 回调（触发后必须调 [`FramePacer::on_vsync`]）。
pub(crate) type PostVsync = Box<dyn Fn()>;

/// 帧节拍器。
pub(crate) struct FramePacer {
    /// `None` 时按挂钟以 `interval` 打点。
    post_vsync: Option<PostVsync>,
    /// 屏幕刷新拍：帧耗时超它即不等 vsync，也是挂钟退化的打点间隔。
    interval: Duration,
    /// 已投回调尚未触发。
    posted: Cell<bool>,
    /// 已欠帧尚未取走。
    due: Cell<bool>,
    /// 挂钟退化：需求在等的时刻。
    fallback_due_at: Cell<Option<Instant>>,
    /// 挂钟退化：上一帧取走的时刻（给下一帧定间距）。
    last_frame_at: Cell<Option<Instant>>,
}

impl FramePacer {
    /// vsync 驱动：`post_vsync` 投一发必须以 [`Self::on_vsync`] 收尾的回调。
    pub(crate) fn with_vsync(post_vsync: PostVsync, interval: Duration) -> Self {
        Self {
            post_vsync: Some(post_vsync),
            interval,
            posted: Cell::new(false),
            due: Cell::new(false),
            fallback_due_at: Cell::new(None),
            last_frame_at: Cell::new(None),
        }
    }

    /// 无 vsync 源：按 `interval` 挂钟打点。
    pub(crate) fn with_clock(interval: Duration) -> Self {
        Self {
            post_vsync: None,
            interval,
            posted: Cell::new(false),
            due: Cell::new(false),
            fallback_due_at: Cell::new(None),
            last_frame_at: Cell::new(None),
        }
    }

    /// 记录一帧需求：上帧之后首个需求投回调（或直接到期），其余折叠。
    pub(crate) fn schedule(&self, now: Instant) {
        if self.posted.get() || self.due.get() || self.fallback_due_at.get().is_some() {
            return;
        }
        let interval_elapsed = self
            .last_frame_at
            .get()
            .is_none_or(|last| now.duration_since(last) >= self.interval);
        match &self.post_vsync {
            Some(_) if interval_elapsed => self.due.set(true),
            Some(post) => {
                self.posted.set(true);
                post();
            }
            None => {
                let earliest = self
                    .last_frame_at
                    .get()
                    .map_or(now, |last| last + self.interval);
                self.fallback_due_at.set(Some(earliest.max(now)));
            }
        }
    }

    /// 已投的 vsync 回调触发：它欠的那帧到期。
    pub(crate) fn on_vsync(&self) {
        self.posted.set(false);
        self.due.set(true);
    }

    /// 取走欠帧（循环画完一帧调一次；窗口不可见时不取，欠着）。
    pub(crate) fn take_frame(&self, now: Instant) -> bool {
        if self.due.replace(false) {
            self.last_frame_at.set(Some(now));
            return true;
        }
        match self.fallback_due_at.get() {
            Some(due_at) if due_at <= now => {
                self.fallback_due_at.set(None);
                self.last_frame_at.set(Some(now));
                true
            }
            _ => false,
        }
    }

    /// 是否有未满足的帧需求（投了/欠了/挂钟等着）。
    #[cfg(test)]
    fn has_demand(&self) -> bool {
        self.posted.get() || self.due.get() || self.fallback_due_at.get().is_some()
    }

    /// 忘掉已投回调并重投（后台回来时旧回调可能永远不触发）。
    pub(crate) fn resume(&self, now: Instant) {
        self.posted.set(false);
        self.schedule(now);
    }

    /// 循环本轮可阻塞多久：欠帧即零，挂钟等帧即到期差，
    /// 纯 vsync 等待即空（靠 looper 投递叫醒）。
    pub(crate) fn poll_timeout(&self, now: Instant) -> Option<Duration> {
        if self.due.get() {
            return Some(Duration::ZERO);
        }
        self.fallback_due_at
            .get()
            .map(|due_at| due_at.saturating_duration_since(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    /// 测试用拍（16ms）。
    const INTERVAL: Duration = Duration::from_millis(16);

    /// 刚在 `now` 画完一帧的 vsync 节拍器（下个需求在拍内，走等 vsync）。
    fn vsync_pacer_after_frame(now: Instant) -> (FramePacer, Rc<Cell<u32>>) {
        let posts = Rc::new(Cell::new(0));
        let counter = Rc::clone(&posts);
        let pacer =
            FramePacer::with_vsync(Box::new(move || counter.set(counter.get() + 1)), INTERVAL);
        pacer.schedule(now);
        assert!(pacer.take_frame(now), "首帧直接到期");
        (pacer, posts)
    }

    /// 首帧不等 vsync。
    #[test]
    fn first_frame_does_not_wait_for_a_vsync() {
        let posts = Rc::new(Cell::new(0));
        let counter = Rc::clone(&posts);
        let pacer =
            FramePacer::with_vsync(Box::new(move || counter.set(counter.get() + 1)), INTERVAL);
        let now = Instant::now();

        pacer.schedule(now);

        assert_eq!(posts.get(), 0);
        assert_eq!(pacer.poll_timeout(now), Some(Duration::ZERO));
        assert!(pacer.take_frame(now));
    }

    /// 需求折叠：回调触发前只投一发。
    #[test]
    fn demand_posts_one_callback_until_it_fires() {
        let now = Instant::now();
        let (pacer, posts) = vsync_pacer_after_frame(now);
        let now = now + Duration::from_millis(1);

        pacer.schedule(now);
        pacer.schedule(now);
        pacer.schedule(now);

        assert_eq!(posts.get(), 1);
        assert!(pacer.has_demand());
        assert!(!pacer.take_frame(now), "vsync 前无到期");
        assert_eq!(pacer.poll_timeout(now), None, "等 looper 投递 vsync");
    }

    /// vsync 欠一帧，且只欠一帧；下个需求重投。
    #[test]
    fn vsync_makes_one_frame_due_and_the_next_demand_posts_again() {
        let now = Instant::now();
        let (pacer, posts) = vsync_pacer_after_frame(now);
        let now = now + Duration::from_millis(1);

        pacer.schedule(now);
        pacer.on_vsync();
        assert_eq!(pacer.poll_timeout(now), Some(Duration::ZERO));
        assert!(pacer.take_frame(now));
        assert!(!pacer.take_frame(now), "一发 vsync 只欠一帧");
        assert!(!pacer.has_demand());

        pacer.schedule(now + Duration::from_millis(1));
        assert_eq!(posts.get(), 2);
    }

    /// 欠帧期间的需求不重投。
    #[test]
    fn demand_while_a_frame_is_due_does_not_post() {
        let now = Instant::now();
        let (pacer, posts) = vsync_pacer_after_frame(now);
        let now = now + Duration::from_millis(1);

        pacer.schedule(now);
        pacer.on_vsync();
        pacer.schedule(now);

        assert_eq!(posts.get(), 1);
        assert!(pacer.take_frame(now));
        assert!(!pacer.take_frame(now));
    }

    /// 超拍的帧直接到期，不等 vsync；回到拍内恢复节拍。
    #[test]
    fn a_frame_that_overran_the_interval_is_served_at_once() {
        let now = Instant::now();
        let (pacer, posts) = vsync_pacer_after_frame(now);

        let later = now + Duration::from_millis(40);
        pacer.schedule(later);

        assert_eq!(posts.get(), 0);
        assert_eq!(pacer.poll_timeout(later), Some(Duration::ZERO));
        assert!(pacer.take_frame(later));

        pacer.schedule(later + Duration::from_millis(5));
        assert_eq!(posts.get(), 1);
    }

    /// 窗口不可见时欠帧保留，回来再取。
    #[test]
    fn untaken_frame_stays_due_for_an_inactive_window() {
        let now = Instant::now();
        let (pacer, _posts) = vsync_pacer_after_frame(now);
        let now = now + Duration::from_millis(1);

        pacer.schedule(now);
        pacer.on_vsync();
        assert!(pacer.has_demand());
        assert!(pacer.take_frame(now + Duration::from_secs(5)));
    }

    /// resume 重投可能永不触发的旧回调。
    #[test]
    fn resume_reposts_a_callback_that_may_never_fire() {
        let now = Instant::now();
        let (pacer, posts) = vsync_pacer_after_frame(now);
        let now = now + Duration::from_millis(1);

        pacer.schedule(now);
        assert_eq!(posts.get(), 1);

        pacer.resume(now);
        assert_eq!(posts.get(), 2);

        pacer.on_vsync();
        pacer.on_vsync();
        assert!(pacer.take_frame(now));
        assert!(!pacer.take_frame(now));
    }

    /// 挂钟退化按拍打点。
    #[test]
    fn clock_fallback_spaces_frames_by_the_interval() {
        let pacer = FramePacer::with_clock(INTERVAL);
        let t0 = Instant::now();

        pacer.schedule(t0);
        assert_eq!(pacer.poll_timeout(t0), Some(Duration::ZERO));
        assert!(pacer.take_frame(t0), "首帧直接到期");

        pacer.schedule(t0 + Duration::from_millis(1));
        assert_eq!(
            pacer.poll_timeout(t0 + Duration::from_millis(1)),
            Some(Duration::from_millis(15))
        );
        assert!(!pacer.take_frame(t0 + Duration::from_millis(10)));
        assert!(pacer.take_frame(t0 + INTERVAL));
        assert!(!pacer.has_demand());
    }

    /// 挂钟退化的需求折叠进待取帧。
    #[test]
    fn clock_fallback_folds_demand_into_the_pending_frame() {
        let pacer = FramePacer::with_clock(INTERVAL);
        let t0 = Instant::now();

        pacer.schedule(t0);
        pacer.schedule(t0 + Duration::from_millis(5));
        assert!(pacer.take_frame(t0 + Duration::from_millis(5)));
        assert!(!pacer.take_frame(t0 + Duration::from_millis(5)));
    }
}
