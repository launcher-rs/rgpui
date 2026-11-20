//! Windows 端渲染保留层：对标 gpui-fast 的 `fast/` 隔离纪律。
//!
//! 管线文件（`directx_renderer.rs` 等）只允许出现 hook：持有 `fast/` 类型、
//! 调用一行 `fast/` 逻辑；算法与解释性注释一律写在子模块内。

pub(crate) mod partial;
