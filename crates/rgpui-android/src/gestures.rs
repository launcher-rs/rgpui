//! Android 手感常量：可移植识别器在 Android 上的调优值。
//!
//! 核心 `GestureTuning` 默认 iOS 手感（`UIScrollView` 指数衰减，滑得远）；
//! Android（AOSP `OverScroller` 摩擦样条）停得更快更齐。
//! 核心暂未消费 `PlatformGestures`（`gestures.rs` 为占位 API），
//! 本模块先给应用层识别器用，对齐后再由平台统一注入。

use rgpui::{GestureTuning, px};
use std::time::Duration;

/// Android 手感调优（对标 AOSP `ViewConfiguration` + `OverScroller`）。
///
/// * 触摸 slop 8dp（与默认同，AOSP 同值）；连击间隔 400ms；长按 500ms。
/// * 惯性衰减：`OverScroller` 摩擦样条等效每毫秒约 `0.994`
///   （比 iOS 的 `0.998` 收得快，短滑即停）。
/// * 最小甩出速度 50px/s（与默认同，AOSP `ViewConfiguration` 同量级）。
pub fn android_gesture_tuning() -> GestureTuning {
    GestureTuning {
        touch_slop: px(8.0),
        multi_tap_interval: Duration::from_millis(400),
        multi_tap_slop: px(16.0),
        long_press_duration: Duration::from_millis(500),
        momentum_decay_per_ms: 0.994,
        min_fling_velocity: 50.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Android 惯性比默认（iOS）收得快。
    #[test]
    fn android_momentum_decays_faster_than_default() {
        let android = android_gesture_tuning();
        let default = GestureTuning::default();
        assert!(android.momentum_decay_per_ms < default.momentum_decay_per_ms);
    }

    /// 点击/长按阈值与 AOSP 一致。
    #[test]
    fn tap_thresholds_match_aosp() {
        let tuning = android_gesture_tuning();
        assert_eq!(tuning.touch_slop, px(8.0));
        assert_eq!(tuning.multi_tap_interval, Duration::from_millis(400));
        assert_eq!(tuning.long_press_duration, Duration::from_millis(500));
    }
}
