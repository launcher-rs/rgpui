//! 单窗口帧统计：普通模式基线与 Retained 模式对比的统一口径。
//!
//! 骨架阶段只记录“帧数 + draw 段墙钟耗时 + 开关状态”，后续阶段在此追加
//! `views_reused / views_rebuilt / spliced / layout_nodes_reused` 等计数器，
//! 对比口径（同一场景、同一窗口尺寸、release 构建）保持不变。

use std::fmt;
use std::time::Duration;

/// 单窗口累计帧统计（仅追加记录，零行为变更）。
#[derive(Debug)]
pub(crate) struct FrameStats {
    /// 已记录帧数。
    frames: u64,
    /// draw 段累计耗时。
    total_draw: Duration,
    /// 上一帧 draw 段耗时。
    last_draw: Duration,
    /// 历史最大单帧 draw 段耗时。
    max_draw: Duration,
    /// 记录上一帧时保留总开关的状态（用于区分基线与 Retained 数据）。
    retention_on: bool,
    /// 复用上帧输出的视图数（P2a：全部状态视图）。
    views_reused: u64,
    /// 重新绘制的视图数。
    views_rebuilt: u64,
    /// 无写入复用的布局节点数（P3a：样式／子节点／测量全命中）。
    layout_nodes_reused: u64,
    /// 命中但改写的布局节点数（仅省分配）。
    layout_nodes_rewritten: u64,
    /// 新分配的布局节点数（含临时节点）。
    layout_nodes_allocated: u64,
}

impl FrameStats {
    /// 创建空统计。
    pub(crate) fn new() -> Self {
        Self {
            frames: 0,
            total_draw: Duration::ZERO,
            last_draw: Duration::ZERO,
            max_draw: Duration::ZERO,
            retention_on: super::retention_enabled(),
            views_reused: 0,
            views_rebuilt: 0,
            layout_nodes_reused: 0,
            layout_nodes_rewritten: 0,
            layout_nodes_allocated: 0,
        }
    }

    /// 记录一帧 draw 段耗时（含 `draw_roots` 三阶段与其前后帧装配）。
    pub(crate) fn record_draw(&mut self, elapsed: Duration, retention_on: bool) {
        self.frames += 1;
        self.total_draw += elapsed;
        self.last_draw = elapsed;
        self.max_draw = self.max_draw.max(elapsed);
        self.retention_on = retention_on;
    }

    /// 记录一次视图复用（上帧输出直接重放）。
    pub(crate) fn note_view_reused(&mut self) {
        self.views_reused += 1;
    }

    /// 记录一次视图重建（重新渲染／布局／绘制）。
    pub(crate) fn note_view_rebuilt(&mut self) {
        self.views_rebuilt += 1;
    }

    /// 记录一帧布局节点统计（Retained P3a：帧末汇总）。
    pub(crate) fn note_layout_nodes(&mut self, reused: u64, rewritten: u64, allocated: u64) {
        self.layout_nodes_reused += reused;
        self.layout_nodes_rewritten += rewritten;
        self.layout_nodes_allocated += allocated;
    }

    /// 生成当前快照（测量输出与基准测试的统一读取口）。
    pub(crate) fn snapshot(&self) -> FrameStatsSnapshot {
        let avg_ms = if self.frames == 0 {
            0.0
        } else {
            self.total_draw.as_secs_f64() * 1000.0 / self.frames as f64
        };
        FrameStatsSnapshot {
            frames: self.frames,
            last_ms: self.last_draw.as_secs_f64() * 1000.0,
            avg_ms,
            max_ms: self.max_draw.as_secs_f64() * 1000.0,
            retention_on: self.retention_on,
            views_reused: self.views_reused,
            views_rebuilt: self.views_rebuilt,
            layout_nodes_reused: self.layout_nodes_reused,
            layout_nodes_rewritten: self.layout_nodes_rewritten,
            layout_nodes_allocated: self.layout_nodes_allocated,
        }
    }
}

/// 可打印、可断言的帧统计快照。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FrameStatsSnapshot {
    /// 已记录帧数。
    pub(crate) frames: u64,
    /// 上一帧耗时（毫秒）。
    pub(crate) last_ms: f64,
    /// 平均帧耗时（毫秒）。
    pub(crate) avg_ms: f64,
    /// 最大单帧耗时（毫秒）。
    pub(crate) max_ms: f64,
    /// 记录时保留总开关是否开启。
    pub(crate) retention_on: bool,
    /// 复用上帧输出的视图数。
    pub(crate) views_reused: u64,
    /// 重新绘制的视图数。
    pub(crate) views_rebuilt: u64,
    /// 无写入复用的布局节点数。
    pub(crate) layout_nodes_reused: u64,
    /// 命中但改写的布局节点数。
    pub(crate) layout_nodes_rewritten: u64,
    /// 新分配的布局节点数。
    pub(crate) layout_nodes_allocated: u64,
}

impl fmt::Display for FrameStatsSnapshot {
    /// 单行输出，供 `MEASUREMENTS` 日志与基准测试直接采集。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[fast] frames={} last={:.3}ms avg={:.3}ms max={:.3}ms retention={} reused={} rebuilt={} layout(reused/rewrote/alloc)={}/{}/{}",
            self.frames,
            self.last_ms,
            self.avg_ms,
            self.max_ms,
            if self.retention_on { "on" } else { "off" },
            self.views_reused,
            self.views_rebuilt,
            self.layout_nodes_reused,
            self.layout_nodes_rewritten,
            self.layout_nodes_allocated
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 累计、平均与开关状态快照正确。
    #[test]
    fn snapshot_reports_counts_and_average() {
        let mut stats = FrameStats::new();
        stats.record_draw(Duration::from_millis(2), false);
        stats.record_draw(Duration::from_millis(4), false);
        stats.note_view_reused();
        stats.note_view_rebuilt();
        stats.note_view_rebuilt();
        let snapshot = stats.snapshot();
        assert_eq!(snapshot.frames, 2);
        assert!((snapshot.avg_ms - 3.0).abs() < f64::EPSILON);
        assert!((snapshot.last_ms - 4.0).abs() < f64::EPSILON);
        assert!((snapshot.max_ms - 4.0).abs() < f64::EPSILON);
        assert!(!snapshot.retention_on);
        assert_eq!(snapshot.views_reused, 1);
        assert_eq!(snapshot.views_rebuilt, 2);
        assert!(format!("{snapshot}").contains("retention=off"));
    }
}
