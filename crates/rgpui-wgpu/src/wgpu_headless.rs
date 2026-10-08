//! 无头渲染器：不建窗口，把场景渲染进离屏纹理并回读像素。
//!
//! 视觉测试（`VisualTestAppContext`）与 `#[rgpui::bench]` 都要靠它拿到「这一帧画成了什么」。
//! 关键在于**没有 surface**：真实窗口的交换链帧要等平台事件循环释放才退休，
//! 在主线程里同步回读会一直等不到 GPU 完成（本机 lavapipe 实测：连空提交都
//! `PollError::Timeout`，而无 surface 的设备立刻 `QueueEmpty`）。

use crate::WgpuRenderer;
use image::RgbaImage;
use rgpui::{DevicePixels, PlatformAtlas, PlatformHeadlessRenderer, Scene, Size};
use std::sync::Arc;

/// 离屏渲染器，实现 [`PlatformHeadlessRenderer`]。
///
/// 每个实例自带一套 GPU 资源与精灵图集：测试窗口绘制用的图集必须是它 `sprite_atlas()`
/// 返回的那一个，否则图元拿不到纹理。
pub struct WgpuHeadlessRenderer {
    renderer: WgpuRenderer,
}

impl WgpuHeadlessRenderer {
    /// 新建离屏渲染器，初始目标尺寸取 `size`。
    ///
    /// 尺寸只是初值：每次渲染都按传入的 `size` 调整目标，测试里缩放窗口不需要重建渲染器。
    pub fn new(size: Size<DevicePixels>) -> anyhow::Result<Self> {
        // transparent=false 让混合走非预乘的 ALPHA_BLENDING，回读出来的 RGBA 才是直 alpha
        let renderer = WgpuRenderer::new_headless(size, false)?;
        Ok(Self { renderer })
    }

    /// 渲染场景并回读 RGBA8 像素，供不需要 `RgbaImage` 的调用方使用。
    pub fn render_to_pixels(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> anyhow::Result<(u32, u32, Vec<u8>)> {
        if size.width.0 <= 0 || size.height.0 <= 0 {
            anyhow::bail!(
                "渲染目标尺寸必须大于 0，收到 {}x{}",
                size.width.0,
                size.height.0
            );
        }
        self.renderer.render_scene_to_pixels_at(scene, size)
    }
}

impl PlatformHeadlessRenderer for WgpuHeadlessRenderer {
    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> anyhow::Result<RgbaImage> {
        let (width, height, pixels) = self.render_to_pixels(scene, size)?;
        RgbaImage::from_raw(width, height, pixels)
            .ok_or_else(|| anyhow::anyhow!("回读像素与 {width}x{height} 尺寸不符"))
    }

    fn render_scene(&mut self, scene: &Scene, size: Size<DevicePixels>) -> anyhow::Result<()> {
        // 离屏目标不呈现，回读是确认这一帧真的提交并跑完的唯一手段
        self.render_to_pixels(scene, size)?;
        Ok(())
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.renderer.sprite_atlas().clone()
    }
}
