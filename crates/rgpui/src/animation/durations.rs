//! 常用动画时长常量。

use std::time::Duration;

/// 极快（100ms）——用于悬停、按下等细微状态变化。
pub const ULTRA_FAST: Duration = Duration::from_millis(100);
/// 很快（150ms）——用于状态切换的微妙过渡。
pub const FASTEST: Duration = Duration::from_millis(150);
/// 快（200ms）——用于快速过渡。
pub const FAST: Duration = Duration::from_millis(200);
/// 正常（300ms）——大多数动画的默认时长。
pub const NORMAL: Duration = Duration::from_millis(300);
/// 慢（400ms）——用于强调效果。
pub const SLOW: Duration = Duration::from_millis(400);
/// 很慢（500ms）——用于戏剧化效果。
pub const SLOWEST: Duration = Duration::from_millis(500);
/// 极慢（600ms）——用于非常戏剧化的效果。
pub const EXTRA_SLOW: Duration = Duration::from_millis(600);
