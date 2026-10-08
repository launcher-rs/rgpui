//! Linux/FreeBSD 平台后端实现，支持 Wayland 和 X11 显示服务器。
//!
//! 本 crate 为 RGPUI 提供 Linux 和 FreeBSD 系统上的平台抽象，包括窗口管理、
//! 输入处理、剪贴板、文件选择器等功能。支持通过特性标志选择 Wayland 或 X11 后端。

#![cfg(any(target_os = "linux", target_os = "freebsd"))]

mod linux;

/// 返回当前操作系统的默认平台实现。
pub use linux::current_platform;

/// 返回一个 Linux 离屏渲染器，供视觉测试与 `#[rgpui::bench]` 出图。
///
/// 用的是 wgpu 的**无 surface** 设备：真实窗口的交换链帧要等平台事件循环释放才退休，
/// 在主线程里同步回读像素会一直等不到 GPU 完成（本机 lavapipe 实测连空提交都超时），
/// 所以测试路径不能借用窗口渲染器。
/// 建不出设备（没有可用 GPU 驱动）时返回 `None`，测试窗口会退回到不绘制的空图集。
#[cfg(all(feature = "test-support", any(feature = "x11", feature = "wayland")))]
pub fn current_headless_renderer() -> Option<Box<dyn rgpui::PlatformHeadlessRenderer>> {
    use rgpui::{DevicePixels, Size};
    // 初值尺寸无所谓：每次渲染都按传入 size 调整离屏目标
    match rgpui_wgpu::WgpuHeadlessRenderer::new(Size {
        width: DevicePixels(1),
        height: DevicePixels(1),
    }) {
        Ok(renderer) => Some(Box::new(renderer)),
        Err(error) => {
            log::error!("创建离屏渲染器失败：{error:#}");
            None
        }
    }
}
