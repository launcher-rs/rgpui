//! 触摸守卫：落在惯性滚动里的新触摸不继承惯性方向。
//!
//! GPUI 的触摸识别器在新触摸开始时停住惯性，但（参考实现验证的上游行为）
//! 会把新触摸锁在被停住的惯性轴上：快速横滑后立刻上滑，页面看起来像冻住，
//! 手指抬起才恢复。同一个坑还会吞掉滚动条拇指拖拽（拇指只在滚动时可见，
//! 而点停惯性的那次触摸永远拿不到 drag）。
//!
//! 平台侧能做的：在可能落到惯性上的真触摸之前，先 relay 一对“同点开始、
//! 同点取消”的合成触摸——它以零位移停住惯性（消费者当无操作），
//! 真触摸再从空闲识别器开始，按自己的首动方向定轴、无 drag 歧视。
//! 识别器的惯性平台侧不可见，只能估：惯性只跟在“滑出过 slop 的触摸抬起”后，
//! 且任何惯性都活不过 [`MOMENTUM_WINDOW`]。
//!
//! 仅主线程使用（与 GPUI 其余部分一致）。

use rgpui::{Pixels, Point, TouchEvent, TouchId, TouchPhase, px};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

/// 长于识别器任何惯性：Android 样条惯性比 iOS 短，4 秒全覆盖。
const MOMENTUM_WINDOW: Duration = Duration::from_secs(4);

/// 识别器触摸 slop：滑出它才算 pan、抬起才可能甩出惯性。
const PAN_SLOP: Pixels = px(8.0);

/// 存活触摸的起点与是否滑出过 slop。
struct Contact {
    /// 起点（逻辑点）。
    start: Point<Pixels>,
    /// 是否滑出过 slop。
    panned: bool,
}

/// 触摸守卫（把原始触摸转交给 GPUI，中途插合成触摸停惯性）。
pub(crate) struct FlingGuard {
    /// 存活触摸表。
    contacts: HashMap<TouchId, Contact>,
    /// 惯性可能存续到的时刻。
    momentum_until: Option<Instant>,
}

impl FlingGuard {
    /// 新守卫。
    pub(crate) fn new() -> Self {
        Self {
            contacts: HashMap::new(),
            momentum_until: None,
        }
    }

    /// 转交 `event`（`next_id` 与平台真触摸同序列发合成触摸 id）。
    pub(crate) fn relay(
        &mut self,
        event: TouchEvent,
        next_id: impl FnOnce() -> TouchId,
        mut emit: impl FnMut(TouchEvent),
    ) {
        self.relay_at(event, Instant::now(), next_id, &mut emit);
    }

    /// 定时版转交（单测可定时间）。
    fn relay_at(
        &mut self,
        event: TouchEvent,
        now: Instant,
        next_id: impl FnOnce() -> TouchId,
        emit: &mut impl FnMut(TouchEvent),
    ) {
        match event.phase {
            TouchPhase::Started => {
                if self.momentum_until.take().is_some_and(|until| now < until) {
                    let id = next_id();
                    for phase in [TouchPhase::Started, TouchPhase::Cancelled] {
                        emit(TouchEvent {
                            id,
                            phase,
                            position: event.position,
                            force: None,
                        });
                    }
                }
                self.contacts.insert(
                    event.id,
                    Contact {
                        start: event.position,
                        panned: false,
                    },
                );
            }
            TouchPhase::Moved => {
                if let Some(contact) = self.contacts.get_mut(&event.id) {
                    if !contact.panned {
                        let travelled = event.position - contact.start;
                        contact.panned = travelled.magnitude() > f64::from(PAN_SLOP);
                    }
                }
            }
            TouchPhase::Ended => {
                if self
                    .contacts
                    .remove(&event.id)
                    .is_some_and(|contact| contact.panned)
                {
                    self.momentum_until = Some(now + MOMENTUM_WINDOW);
                }
            }
            // 被取消的 pan 甩不出惯性。
            TouchPhase::Cancelled => {
                self.contacts.remove(&event.id);
            }
        }
        emit(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgpui::point;

    /// 造触摸事件。
    fn touch(id: u64, phase: TouchPhase, x: f32, y: f32) -> TouchEvent {
        TouchEvent {
            id: TouchId(id),
            phase,
            position: point(px(x), px(y)),
            force: None,
        }
    }

    /// 在 `now` 转交一批事件，返回到达 GPUI 的 `(id, 阶段, 位置)`。
    fn relay_all(
        guard: &mut FlingGuard,
        now: Instant,
        events: impl IntoIterator<Item = TouchEvent>,
    ) -> Vec<(u64, TouchPhase, Point<Pixels>)> {
        let mut out = Vec::new();
        let mut next_id = 100;
        for event in events {
            guard.relay_at(
                event,
                now,
                || {
                    next_id += 1;
                    TouchId(next_id)
                },
                &mut |event| out.push((event.id.0, event.phase, event.position)),
            );
        }
        out
    }

    /// 一次横滑 pan。
    fn pan(guard: &mut FlingGuard, now: Instant) {
        let relayed = relay_all(
            guard,
            now,
            [
                touch(1, TouchPhase::Started, 300.0, 300.0),
                touch(1, TouchPhase::Moved, 250.0, 300.0),
                touch(1, TouchPhase::Ended, 200.0, 300.0),
            ],
        );
        assert_eq!(relayed.len(), 3, "pan 原样转交");
    }

    /// pan 抬起后的触摸：前面插一对合成触摸，真触摸排第三。
    #[test]
    fn a_contact_after_a_pan_release_is_preceded_by_a_cancelled_contact() {
        let mut guard = FlingGuard::new();
        let now = Instant::now();
        pan(&mut guard, now);

        let relayed = relay_all(
            &mut guard,
            now + Duration::from_millis(200),
            [touch(2, TouchPhase::Started, 200.0, 300.0)],
        );

        assert_eq!(
            relayed,
            [
                (101, TouchPhase::Started, point(px(200.0), px(300.0))),
                (101, TouchPhase::Cancelled, point(px(200.0), px(300.0))),
                (2, TouchPhase::Started, point(px(200.0), px(300.0))),
            ]
        );
    }

    /// 守卫一次 pan 只武装一次。
    #[test]
    fn the_guard_is_armed_once_per_release() {
        let mut guard = FlingGuard::new();
        let now = Instant::now();
        pan(&mut guard, now);

        let first = relay_all(
            &mut guard,
            now + Duration::from_millis(200),
            [
                touch(2, TouchPhase::Started, 200.0, 300.0),
                touch(2, TouchPhase::Ended, 200.0, 300.0),
            ],
        );
        let second = relay_all(
            &mut guard,
            now + Duration::from_millis(400),
            [touch(3, TouchPhase::Started, 300.0, 300.0)],
        );

        assert_eq!(first.len(), 4);
        assert_eq!(
            second,
            [(3, TouchPhase::Started, point(px(300.0), px(300.0)))]
        );
    }

    /// 惯性窗口期外的触摸原样转交。
    #[test]
    fn a_contact_after_the_momentum_window_is_relayed_as_is() {
        let mut guard = FlingGuard::new();
        let now = Instant::now();
        pan(&mut guard, now);

        let relayed = relay_all(
            &mut guard,
            now + MOMENTUM_WINDOW,
            [touch(2, TouchPhase::Started, 200.0, 300.0)],
        );

        assert_eq!(
            relayed,
            [(2, TouchPhase::Started, point(px(200.0), px(300.0)))]
        );
    }

    /// 轻点不武装守卫。
    #[test]
    fn a_tap_does_not_arm_the_guard() {
        let mut guard = FlingGuard::new();
        let now = Instant::now();
        relay_all(
            &mut guard,
            now,
            [
                touch(1, TouchPhase::Started, 300.0, 300.0),
                touch(1, TouchPhase::Moved, 303.0, 302.0),
                touch(1, TouchPhase::Ended, 303.0, 302.0),
            ],
        );

        let relayed = relay_all(
            &mut guard,
            now + Duration::from_millis(100),
            [touch(2, TouchPhase::Started, 300.0, 300.0)],
        );

        assert_eq!(
            relayed,
            [(2, TouchPhase::Started, point(px(300.0), px(300.0)))]
        );
    }

    /// 被取消的 pan 不武装守卫。
    #[test]
    fn a_cancelled_pan_does_not_arm_the_guard() {
        let mut guard = FlingGuard::new();
        let now = Instant::now();
        relay_all(
            &mut guard,
            now,
            [
                touch(1, TouchPhase::Started, 300.0, 300.0),
                touch(1, TouchPhase::Moved, 200.0, 300.0),
                touch(1, TouchPhase::Cancelled, 200.0, 300.0),
            ],
        );

        let relayed = relay_all(
            &mut guard,
            now + Duration::from_millis(100),
            [touch(2, TouchPhase::Started, 300.0, 300.0)],
        );

        assert_eq!(
            relayed,
            [(2, TouchPhase::Started, point(px(300.0), px(300.0)))]
        );
    }
}
