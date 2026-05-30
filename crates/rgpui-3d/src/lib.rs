//! rgpui-3d: 将 scenix 3D 渲染引擎集成到 rgpui 中。
//!
//! 本 crate 提供：
//! - [`Scenix3D`]：管理 wgpu 设备、离屏渲染目标和 scenix GPU 场景资源
//! - [`RenderResult`]：渲染结果，包含 BGRA 像素数据和尺寸
//! - 与 rgpui 的 `RenderImage` 转换支持
//!
//! 模块结构：
//! - [`shader`]：WGSL 着色器源码
//! - [`types`]：GPU uniform 类型、顶点类型、动画数据结构
//! - [`math`]：数学辅助函数
//! - [`context`]：Scenix3D 渲染上下文与渲染管线
//!
//! # 示例
//! ```ignore
//! use rgpui_3d::Scenix3D;
//!
//! let mut ctx = Scenix3D::new(800, 600).await?;
//! let result = ctx.render(&mut scene, &camera)?;
//! let image = result.into_render_image();
//! ```

/// 重新导出 scenix 公开类型
pub use scenix;

mod context;
mod math;
mod shader;
mod types;

pub use context::{RenderResult, Scenix3D};
