//! Retained oracle 测试：增量帧必须等于从零绘制的帧（P2c）。
//!
//! 方法：同尺寸双窗跑同一操作历史，A 窗保留开启（增量），B 窗保留关闭
//! （逐帧全量重建，即“从零绘制”的真值）。每步绘制后逐帧比对场景与命中盒，
//! 任一 divergent 即失败；末尾断言 A 确实发生过复用（防空洞通过）。
//!
//! 语义约定（与 [`crate::fast`] 文档一致）：视图自身变更必须 `notify`（与
//! cached 视图契约统一）；被读取的实体无 `notify` 更新也必须可见（generation
//! 捕获）。故 oracle 只含这两种，不含“更新自身而不 notify”（该情形要求 notify）。

mod oracle;
mod retained_bench;
