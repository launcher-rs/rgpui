//! 帧率节流：把「窗口失焦」「温控降频」两类省电限制收敛成一条最小帧间隔。
//!
//! 上游 zed `7c8dbadcb5` 把这套逻辑提成了可配置项（`WindowOptions` 上的
//! `inactive_frame_interval` / `thermal_frame_interval` / `low_power_frame_interval`）。
//! rgpui 没有 `PowerMode` 抽象，因此只搬前两档，第三档暂不引入——没有真实的
//! 电源状态来源时加一档只会得到永远不命中的死配置。
//!
//! 口径与上游一致：
//!
//! - 多条限制同时命中时**取最长间隔**（最省电的那条），而不是先命中先返回；
//! - `None` 表示该档关闭（不限帧），不是「回退到默认值」；
//! - 强制呈现（`require_presentation`）与「本帧有下一帧回调」完全不受限，
//!   这部分判断留在 [`crate::Window`] 的帧请求回调里，本模块只回答
//!   「限制命中时这一帧至少要隔多久」。

use std::time::Duration;

use crate::ThermalState;

/// 默认失焦窗口间隔：约 30fps。
pub(crate) const DEFAULT_INACTIVE_FRAME_INTERVAL: Duration = Duration::from_micros(33_333);

/// 默认温控降频间隔：约 60fps。
pub(crate) const DEFAULT_THERMAL_FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

/// 窗口的两档省电帧间隔，来自 [`crate::WindowOptions`] 的同名字段。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrameThrottle {
    /// 失焦窗口的最小帧间隔，`None` 表示失焦时不限帧。
    inactive: Option<Duration>,
    /// 温控 `Serious` / `Critical` 时的最小帧间隔，`None` 表示此时不限帧。
    thermal: Option<Duration>,
}

impl Default for FrameThrottle {
    fn default() -> Self {
        Self {
            inactive: Some(DEFAULT_INACTIVE_FRAME_INTERVAL),
            thermal: Some(DEFAULT_THERMAL_FRAME_INTERVAL),
        }
    }
}

impl FrameThrottle {
    /// 由 `WindowOptions` 的两个字段构造。
    pub(crate) fn new(inactive: Option<Duration>, thermal: Option<Duration>) -> Self {
        Self { inactive, thermal }
    }

    /// 返回本帧的最小帧间隔；`None` 表示不受节流限制。
    ///
    /// `thermal_state` 为 `None` 表示「当前读不到热状态」（例如 `App` 正被可变借用
    /// 而跳过了查询）——此时按无温控压力处理，与改动前的行为一致。
    pub(crate) fn interval(
        &self,
        active: bool,
        high_rate_input: bool,
        thermal_state: Option<ThermalState>,
    ) -> Option<Duration> {
        // 高速率输入期间维持呈现，避免显示器降频导致的输入延迟抖动。
        let inactive = !active && !high_rate_input;
        let thermal = matches!(
            thermal_state,
            Some(ThermalState::Critical | ThermalState::Serious)
        );

        [
            inactive.then_some(self.inactive),
            thermal.then_some(self.thermal),
        ]
        .into_iter()
        .flatten()
        .flatten()
        .max()
    }
}

/// 把「强制呈现 / 下一帧回调 / 场景动画间隔 / 省电节流间隔」合成这一帧的最小间隔。
///
/// `None` 表示本帧不受限。规则：
/// - 强制呈现（`require_presentation`）完全不受限；
/// - 无强制渲染、无下一帧回调、也无场景动画的**纯空闲帧**不限速——以维持显示器
///   刷新率、避免降频。注意场景动画正是靠帧末 `request_frame` 驱动的空闲帧，
///   有动画间隔时**不能**短路，否则 `with_max_fps` 与失焦/温控省电档对动画窗口
///   全部失效；
/// - 其余情况取省电节流与动画间隔里较长的那条。
pub(crate) fn combine_interval(
    require_presentation: bool,
    force_render: bool,
    has_next_frame_callback: bool,
    animation_interval: Option<Duration>,
    throttled: Option<Duration>,
) -> Option<Duration> {
    if require_presentation
        || (!force_render && !has_next_frame_callback && animation_interval.is_none())
    {
        None
    } else {
        match (throttled, animation_interval) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn micros(value: Option<Duration>) -> Option<u64> {
        value.map(|duration| duration.as_micros() as u64)
    }

    #[test]
    fn defaults_match_legacy_behaviour() {
        let throttle = FrameThrottle::default();

        // 失焦：约 30fps。
        assert_eq!(
            micros(throttle.interval(false, false, Some(ThermalState::Nominal))),
            Some(33_333)
        );
        // 温控 Serious/Critical：约 60fps。
        assert_eq!(
            micros(throttle.interval(true, false, Some(ThermalState::Serious))),
            Some(16_667)
        );
        assert_eq!(
            micros(throttle.interval(true, false, Some(ThermalState::Critical))),
            Some(16_667)
        );
        // 焦点在、温度正常：不限帧。
        assert_eq!(
            micros(throttle.interval(true, false, Some(ThermalState::Nominal))),
            None
        );
    }

    #[test]
    fn fair_thermal_state_is_not_throttled() {
        let throttle = FrameThrottle::default();
        // `Fair` 只提示「减少非必要工作」，不构成限帧依据（与上游一致）。
        assert_eq!(
            micros(throttle.interval(true, false, Some(ThermalState::Fair))),
            None
        );
    }

    #[test]
    fn longest_interval_wins_when_both_apply() {
        let throttle = FrameThrottle::new(
            Some(Duration::from_micros(20_000)),
            Some(Duration::from_micros(50_000)),
        );
        // 失焦 + 温控同时命中：取更长的那条（50ms），而不是先命中的 20ms。
        assert_eq!(
            micros(throttle.interval(false, false, Some(ThermalState::Critical))),
            Some(50_000)
        );
    }

    #[test]
    fn high_rate_input_suspends_the_inactive_limit() {
        let throttle = FrameThrottle::default();
        assert_eq!(
            micros(throttle.interval(false, true, Some(ThermalState::Nominal))),
            None
        );
        // 但温控限制不受输入速率影响：此时只剩温控档生效（16_667，不是 33_333）。
        assert_eq!(
            micros(throttle.interval(false, true, Some(ThermalState::Critical))),
            Some(16_667)
        );
    }

    #[test]
    fn none_disables_only_its_own_tier() {
        let throttle = FrameThrottle::new(None, Some(Duration::from_micros(16_667)));
        assert_eq!(
            micros(throttle.interval(false, false, Some(ThermalState::Nominal))),
            None
        );
        assert_eq!(
            micros(throttle.interval(false, false, Some(ThermalState::Critical))),
            Some(16_667)
        );

        let throttle = FrameThrottle::new(Some(Duration::from_micros(33_333)), None);
        assert_eq!(
            micros(throttle.interval(false, false, Some(ThermalState::Nominal))),
            Some(33_333)
        );
        assert_eq!(
            micros(throttle.interval(true, false, Some(ThermalState::Critical))),
            None
        );
    }

    #[test]
    fn unknown_thermal_state_is_treated_as_no_pressure() {
        let throttle = FrameThrottle::default();
        assert_eq!(micros(throttle.interval(true, false, None)), None);
    }

    #[test]
    fn animation_frames_are_not_exempt_from_throttling() {
        let animation = Duration::from_micros(66_667); // with_max_fps(15)

        // 纯空闲帧：无强制呈现、无强制渲染、无下一帧回调、无动画 → 不限速。
        assert_eq!(combine_interval(false, false, false, None, None), None);

        // 场景动画帧也是「空闲帧」（靠帧末 request_frame 驱动），但有动画间隔时
        // 必须参与节流，不能短路——否则 with_max_fps 形同虚设。
        assert_eq!(
            micros(combine_interval(false, false, false, Some(animation), None)),
            Some(66_667)
        );
        // 动画间隔与省电节流同时命中：取较长者。
        assert_eq!(
            micros(combine_interval(
                false,
                false,
                false,
                Some(animation),
                Some(Duration::from_micros(33_333))
            )),
            Some(66_667)
        );
        assert_eq!(
            micros(combine_interval(
                false,
                false,
                false,
                Some(Duration::from_micros(10_000)),
                Some(Duration::from_micros(33_333))
            )),
            Some(33_333)
        );
        // 强制呈现仍完全不受限。
        assert_eq!(
            combine_interval(true, false, false, Some(animation), None),
            None
        );
        // 有下一帧回调的帧：无动画时走省电节流（不因"空闲"短路）。
        assert_eq!(
            micros(combine_interval(
                false,
                false,
                true,
                None,
                Some(Duration::from_micros(33_333))
            )),
            Some(33_333)
        );
    }

    #[test]
    fn window_options_defaults_feed_this_module() {
        // 防止两处默认值各改各的：`WindowOptions::default()` 必须仍走这两个常量。
        let options = crate::WindowOptions::default();
        assert_eq!(
            options.inactive_frame_interval,
            Some(DEFAULT_INACTIVE_FRAME_INTERVAL)
        );
        assert_eq!(
            options.thermal_frame_interval,
            Some(DEFAULT_THERMAL_FRAME_INTERVAL)
        );
        assert_eq!(
            FrameThrottle::new(
                options.inactive_frame_interval,
                options.thermal_frame_interval
            ),
            FrameThrottle::default()
        );
    }
}
