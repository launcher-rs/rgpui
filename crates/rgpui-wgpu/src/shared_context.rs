//! 进程级共享的 wgpu GPU 上下文。
//!
//! 当 rgpui 平台创建第一个 `WgpuContext` 时，会在此处注册共享的
//! `wgpu::Device` 和 `wgpu::Queue`。`rgpui-3d` 等第三方渲染器
//! 可以获取这些共享资源，避免创建多个 `wgpu::Instance`。

use std::sync::{Arc, OnceLock};

/// 共享的 wgpu GPU 上下文，用于跨 crate 复用同一 wgpu 实例。
pub struct SharedGpuContext {
    /// 共享的 wgpu 实例
    pub instance: wgpu::Instance,
    /// 共享的 wgpu 设备
    pub device: Arc<wgpu::Device>,
    /// 共享的 wgpu 队列
    pub queue: Arc<wgpu::Queue>,
}

static SHARED: OnceLock<SharedGpuContext> = OnceLock::new();

/// 注册共享的 wgpu GPU 上下文。仅第一次调用生效，后续调用被忽略。
pub fn register(instance: wgpu::Instance, device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) {
    let _ = SHARED.set(SharedGpuContext {
        instance,
        device,
        queue,
    });
}

/// 获取共享的 wgpu GPU 上下文（如果已注册）。
pub fn try_get() -> Option<&'static SharedGpuContext> {
    SHARED.get()
}
