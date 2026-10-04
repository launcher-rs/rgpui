//! Retained Mode 实验区：增量帧渲染相关代码统一收敛于此。
//!
//! 对标 `gpui-fast` 的 `fast/` 隔离纪律：
//! - 管线文件（`window.rs` / `view.rs` / `taffy.rs` 等）只允许出现 hook：
//!   持有 `fast/` 类型的字段、对 `fast/` 的一行调用、纯转发函数体；
//! - 保留逻辑、数据结构、算法与解释性注释一律写在 `fast/` 内；
//! - 管线文件中引用一律写全路径（`crate::fast::stats::FrameStats`），不 `use fast`。
//!
//! 当前阶段（骨架）：仅提供保留总开关与帧统计，先为“普通模式 vs Retained”
//! 量化对比建立基线口径；视图复用 / splice / 布局保留 / 文本测量 / 滚动层
//! 按阶段逐个落子，每个阶段自带 oracle 式正确性校验。

pub(crate) mod stats;

pub(crate) use stats::FrameStats;

use std::sync::OnceLock;

/// 读取保留总开关（只解析一次，进程内缓存）。
///
/// - 默认开启（为后续阶段预留；骨架阶段不改变任何绘制行为，只记录统计）。
/// - `RGPUI_VIEW_RETENTION=0/false/no` 关闭；兼容 `gpui-fast` 的
///   `GPUI_VIEW_RETENTION`，`RGPUI_` 优先。
pub(crate) fn retention_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        for var in ["RGPUI_VIEW_RETENTION", "GPUI_VIEW_RETENTION"] {
            if let Ok(value) = std::env::var(var) {
                let disabled = matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "0" | "false" | "no" | "off"
                );
                return !disabled;
            }
        }
        true
    })
}

/// 帧耗时测量输出是否开启。
///
/// 沿用仓内惯例：`ZED_MEASUREMENTS` 或 `rgpui_MEASUREMENTS` 任一存在即开启，
/// 仅用于量化对比时输出基线数据，默认关闭、零开销以外的一次环境变量读取。
pub(crate) fn measurements_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("ZED_MEASUREMENTS").is_ok() || std::env::var("rgpui_MEASUREMENTS").is_ok()
    })
}
