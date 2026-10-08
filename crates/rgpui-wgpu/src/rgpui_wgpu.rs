/// 文本系统实现，基于 cosmic-text 库
mod cosmic_text_system;
/// 共享 wgpu GPU 上下文（跨 crate 复用）
#[cfg(not(target_family = "wasm"))]
pub mod shared_context;
/// wgpu 纹理图集管理
mod wgpu_atlas;
/// wgpu GPU 上下文封装
mod wgpu_context;
/// 无头（离屏）渲染器，供视觉测试与基准测试使用
#[cfg(all(not(target_family = "wasm"), feature = "test-support"))]
mod wgpu_headless;
/// wgpu 渲染器实现
mod wgpu_renderer;
pub use cosmic_text_system::*;
pub use wgpu;
pub use wgpu_atlas::*;
pub use wgpu_context::*;
#[cfg(all(not(target_family = "wasm"), feature = "test-support"))]
pub use wgpu_headless::WgpuHeadlessRenderer;
pub use wgpu_renderer::{GpuContext, WgpuRenderer, WgpuSurfaceConfig};
