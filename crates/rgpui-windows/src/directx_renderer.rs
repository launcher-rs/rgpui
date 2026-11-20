use std::{
    slice,
    sync::{Arc, OnceLock},
};

use anyhow::{Context, Result};
use rgpui::ResultExt;
use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct3D::*,
            Direct3D11::*,
            DirectComposition::*,
            DirectWrite::*,
            Dxgi::{Common::*, *},
        },
    },
    core::Interface,
};

use crate::directx_renderer::shader_resources::{RawShaderBytes, ShaderModule, ShaderTarget};
use crate::*;
use rgpui::*;

/// 环境变量名，用于禁用 DirectComposition
pub(crate) const DISABLE_DIRECT_COMPOSITION: &str = "GPUI_DISABLE_DIRECT_COMPOSITION";
pub(crate) const RENDER_TARGET_FORMAT: DXGI_FORMAT = DXGI_FORMAT_B8G8R8A8_UNORM;

/// 字体渲染信息，包含伽马校正、对比度等参数
pub(crate) struct FontInfo {
    pub gamma_ratios: [f32; 4],
    pub grayscale_enhanced_contrast: f32,
    pub subpixel_enhanced_contrast: f32,
    pub is_bgr: bool,
}

/// DirectX 渲染器，负责将 GPUI 场景渲染到窗口
///
/// 使用 DirectX 11 API 进行 GPU 加速渲染，支持：
/// - 多种图元类型（阴影、矩形、路径、下划线、精灵）
/// - 纹理图集管理
/// - DirectComposition 窗口合成
/// - GPU 设备丢失检测和恢复
pub(crate) struct DirectXRenderer {
    hwnd: HWND,
    atlas: Arc<DirectXAtlas>,
    devices: Option<DirectXRendererDevices>,
    resources: Option<DirectXResources>,
    globals: DirectXGlobalElements,
    pipelines: DirectXRenderPipelines,
    direct_composition: Option<DirectComposition>,
    font_info: &'static FontInfo,

    width: u32,
    height: u32,

    /// Whether we want to skip drawing due to device lost events.
    ///
    /// In that case we want to discard the first frame that we draw as we got reset in the middle of a frame
    /// meaning we lost all the allocated gpu textures and scene resources.
    skip_draws: bool,

    /// D4 部分重绘状态（软件适配器 + 开关）。整帧路径不受影响。
    partial: crate::fast::partial::Partial,
}

/// Direct3D 设备对象
///
/// 包含 Direct3D 11 渲染所需的核心设备对象，这些对象在应用程序生命周期内保持不变
#[derive(Clone)]
pub(crate) struct DirectXRendererDevices {
    pub(crate) adapter: IDXGIAdapter1,
    pub(crate) dxgi_factory: IDXGIFactory6,
    pub(crate) device: ID3D11Device,
    pub(crate) device_context: ID3D11DeviceContext,
    dxgi_device: Option<IDXGIDevice>,
}

struct DirectXResources {
    // Direct3D rendering objects
    swap_chain: IDXGISwapChain1,
    pub(crate) render_target: Option<ID3D11Texture2D>,
    render_target_view: Option<ID3D11RenderTargetView>,

    // anti-overdraw 深度模板（D32_FLOAT，随渲染目标同尺寸）
    depth_stencil_view: Option<ID3D11DepthStencilView>,

    // Cached viewport
    viewport: D3D11_VIEWPORT,
}

struct DirectXRenderPipelines {
    shadow_pipeline: PipelineState<Shadow>,
    quad_pipeline: PipelineState<Quad>,
    /// 路径瓦片管线：着色器 + 瓦片实例缓冲。曲线与绘制参数在
    /// `path_curves` / `path_paints` 两个独立缓冲里，随瓦片批一并绑定。
    path_tile_pipeline: PipelineState<PathTile>,
    path_curves: InstanceBuffer<PathCurve>,
    path_paints: InstanceBuffer<PathPaint>,
    underline_pipeline: PipelineState<Underline>,
    mono_sprites: PipelineState<MonochromeSprite>,
    subpixel_sprites: PipelineState<SubpixelSprite>,
    poly_sprites: PipelineState<PolychromeSprite>,
    /// anti-overdraw 不透明通道：着色器 + 无混合 + 写深度态。
    /// 实例数据直接读 `quad_pipeline` 的全量缓冲视图，不另设缓冲。
    opaque_quads: OpaqueQuadPipeline,
}

/// 只带结构化缓冲与视图的实例数据持有者（无着色器；由管线绑定为额外 SRV）。
struct InstanceBuffer<T> {
    label: &'static str,
    buffer: ID3D11Buffer,
    buffer_size: usize,
    view: Option<ID3D11ShaderResourceView>,
    _marker: std::marker::PhantomData<T>,
}

impl<T> InstanceBuffer<T> {
    fn new(device: &ID3D11Device, label: &'static str, buffer_size: usize) -> Result<Self> {
        let buffer = create_buffer(device, std::mem::size_of::<T>(), buffer_size)?;
        let view = create_buffer_view(device, &buffer)?;
        Ok(Self {
            label,
            buffer,
            buffer_size,
            view,
            _marker: std::marker::PhantomData,
        })
    }

    fn update_buffer(
        &mut self,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        data: &[T],
    ) -> Result<()> {
        if self.buffer_size < data.len() {
            let new_buffer_size = data.len().next_power_of_two();
            log::debug!(
                "Updating {} buffer size from {} to {}",
                self.label,
                self.buffer_size,
                new_buffer_size
            );
            let buffer = create_buffer(device, std::mem::size_of::<T>(), new_buffer_size)?;
            let view = create_buffer_view(device, &buffer)?;
            self.buffer = buffer;
            self.view = view;
            self.buffer_size = new_buffer_size;
        }
        update_buffer(device_context, &self.buffer, data)
    }
}

/// 不透明 quad 通道的着色器与状态（缓冲复用 quad 管线那份）。
struct OpaqueQuadPipeline {
    vertex: ID3D11VertexShader,
    fragment: ID3D11PixelShader,
    blend_state: ID3D11BlendState,
    depth_state: ID3D11DepthStencilState,
}

struct DirectXGlobalElements {
    global_params_buffer: Option<ID3D11Buffer>,
    sampler: Option<ID3D11SamplerState>,
}

struct DirectComposition {
    comp_device: IDCompositionDevice,
    comp_target: IDCompositionTarget,
    comp_visual: IDCompositionVisual,
}

impl DirectXRendererDevices {
    pub(crate) fn new(
        directx_devices: &DirectXDevices,
        disable_direct_composition: bool,
    ) -> Result<Self> {
        let DirectXDevices {
            adapter,
            dxgi_factory,
            device,
            device_context,
        } = directx_devices;
        let dxgi_device = if disable_direct_composition {
            None
        } else {
            Some(device.cast().context("Creating DXGI device")?)
        };

        Ok(Self {
            adapter: adapter.clone(),
            dxgi_factory: dxgi_factory.clone(),
            device: device.clone(),
            device_context: device_context.clone(),
            dxgi_device,
        })
    }
}

impl DirectXRenderer {
    /// 创建新的 DirectX 渲染器
    ///
    /// # 参数
    /// * `hwnd` - 窗口句柄
    /// * `directx_devices` - DirectX 设备引用
    /// * `disable_direct_composition` - 是否禁用 DirectComposition
    pub(crate) fn new(
        hwnd: HWND,
        directx_devices: &DirectXDevices,
        disable_direct_composition: bool,
    ) -> Result<Self> {
        if disable_direct_composition {
            log::info!("Direct Composition is disabled.");
        }

        let devices = DirectXRendererDevices::new(directx_devices, disable_direct_composition)
            .context("Creating DirectX devices")?;
        let atlas = Arc::new(DirectXAtlas::new(&devices.device, &devices.device_context));

        let resources = DirectXResources::new(&devices, 1, 1, hwnd, disable_direct_composition)
            .context("Creating DirectX resources")?;
        let globals = DirectXGlobalElements::new(&devices.device)
            .context("Creating DirectX global elements")?;
        let pipelines = DirectXRenderPipelines::new(&devices.device)
            .context("Creating DirectX render pipelines")?;

        let direct_composition = if disable_direct_composition {
            None
        } else {
            let composition = DirectComposition::new(devices.dxgi_device.as_ref().unwrap(), hwnd)
                .context("Creating DirectComposition")?;
            composition
                .set_swap_chain(&resources.swap_chain)
                .context("Setting swap chain for DirectComposition")?;
            Some(composition)
        };

        let partial = crate::fast::partial::Partial::for_adapter(&devices.adapter);

        Ok(DirectXRenderer {
            hwnd,
            atlas,
            devices: Some(devices),
            resources: Some(resources),
            globals,
            pipelines,
            direct_composition,
            font_info: Self::get_font_info(),
            width: 1,
            height: 1,
            skip_draws: false,
            partial,
        })
    }

    /// 获取纹理图集
    pub(crate) fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }

    fn pre_draw(&self, clear_color: &[f32; 4], quad_count: usize) -> Result<()> {
        let resources = self.resources.as_ref().expect("resources missing");
        let device_context = &self
            .devices
            .as_ref()
            .expect("devices missing")
            .device_context;
        update_buffer(
            device_context,
            self.globals.global_params_buffer.as_ref().unwrap(),
            &[GlobalParams {
                gamma_ratios: self.font_info.gamma_ratios,
                viewport_size: [resources.viewport.Width, resources.viewport.Height],
                grayscale_enhanced_contrast: self.font_info.grayscale_enhanced_contrast,
                subpixel_enhanced_contrast: self.font_info.subpixel_enhanced_contrast,
                is_bgr: self.font_info.is_bgr as u32,
                quad_count: quad_count as u32,
                _pad: [0; 2],
            }],
        )?;
        unsafe {
            device_context.ClearRenderTargetView(
                resources
                    .render_target_view
                    .as_ref()
                    .context("missing render target view")?,
                clear_color,
            );
            // 深度从 0 开始（「越大越近」），与主渲染目标一并绑定。
            device_context.ClearDepthStencilView(
                resources
                    .depth_stencil_view
                    .as_ref()
                    .context("missing depth stencil view")?,
                D3D11_CLEAR_DEPTH.0,
                0.0,
                0,
            );
            device_context.OMSetRenderTargets(
                Some(slice::from_ref(&resources.render_target_view)),
                resources.depth_stencil_view.as_ref(),
            );
            device_context.RSSetViewports(Some(slice::from_ref(&resources.viewport)));
        }
        Ok(())
    }

    #[inline]
    fn present(&mut self) -> Result<()> {
        let result = unsafe {
            self.resources
                .as_ref()
                .expect("resources missing")
                .swap_chain
                .Present(0, DXGI_PRESENT(0))
        };
        result.ok().context("Presenting swap chain failed")
    }

    /// 处理 GPU 设备丢失事件
    ///
    /// 当 GPU 设备丢失时（如驱动更新、显示器切换），重新创建所有渲染资源
    pub(crate) fn handle_device_lost(&mut self, directx_devices: &DirectXDevices) -> Result<()> {
        try_to_recover_from_device_lost(|| {
            self.handle_device_lost_impl(directx_devices)
                .context("DirectXRenderer handling device lost")
        })
    }

    fn handle_device_lost_impl(&mut self, directx_devices: &DirectXDevices) -> Result<()> {
        let disable_direct_composition = self.direct_composition.is_none();

        unsafe {
            #[cfg(debug_assertions)]
            if let Some(devices) = &self.devices {
                report_live_objects(&devices.device)
                    .context("Failed to report live objects after device lost")
                    .log_err();
            }

            self.resources.take();
            if let Some(devices) = &self.devices {
                devices.device_context.OMSetRenderTargets(None, None);
                devices.device_context.ClearState();
                devices.device_context.Flush();
                #[cfg(debug_assertions)]
                report_live_objects(&devices.device)
                    .context("Failed to report live objects after device lost")
                    .log_err();
            }

            self.direct_composition.take();
            self.devices.take();
        }

        let devices = DirectXRendererDevices::new(directx_devices, disable_direct_composition)
            .context("Recreating DirectX devices")?;
        let resources = DirectXResources::new(
            &devices,
            self.width,
            self.height,
            self.hwnd,
            disable_direct_composition,
        )
        .context("Creating DirectX resources")?;
        let globals = DirectXGlobalElements::new(&devices.device)
            .context("Creating DirectXGlobalElements")?;
        let pipelines = DirectXRenderPipelines::new(&devices.device)
            .context("Creating DirectXRenderPipelines")?;

        let direct_composition = if disable_direct_composition {
            None
        } else {
            let composition =
                DirectComposition::new(devices.dxgi_device.as_ref().unwrap(), self.hwnd)?;
            composition.set_swap_chain(&resources.swap_chain)?;
            Some(composition)
        };

        self.atlas
            .handle_device_lost(&devices.device, &devices.device_context);

        unsafe {
            devices.device_context.OMSetRenderTargets(
                Some(slice::from_ref(&resources.render_target_view)),
                resources.depth_stencil_view.as_ref(),
            );
        }
        self.devices = Some(devices);
        self.resources = Some(resources);
        self.globals = globals;
        self.pipelines = pipelines;
        self.direct_composition = direct_composition;
        self.skip_draws = true;
        // 图集纹理随设备重建，旧签名与 canvas 已失效，下一帧必须整帧绘制。
        self.partial.reset();
        Ok(())
    }

    /// 渲染 GPUI 场景到窗口
    ///
    /// # 参数
    /// * `scene` - 要渲染的场景
    /// * `background_appearance` - 窗口背景外观
    pub(crate) fn draw(
        &mut self,
        scene: &Scene,
        background_appearance: WindowBackgroundAppearance,
    ) -> Result<()> {
        if self.draw_partial(scene, background_appearance)? {
            return Ok(());
        }
        self.draw_into(scene, background_appearance)?;
        self.present()
    }

    /// D4 部分重绘：软件适配器上从上一帧 canvas 造当前帧，拷回交换链并提交。
    /// 返回 `true` 表示本帧已完整处理（含 present）。整帧路径返回 `false`。
    fn draw_partial(
        &mut self,
        scene: &Scene,
        background_appearance: WindowBackgroundAppearance,
    ) -> Result<bool> {
        if self.skip_draws || !self.partial.is_on() {
            return Ok(false);
        }
        let clear = match background_appearance {
            WindowBackgroundAppearance::Opaque => [1.0f32; 4],
            _ => [0.0f32; 4],
        };
        let devices = self.devices.as_ref().context("devices missing")?;
        let (device, context) = (devices.device.clone(), devices.device_context.clone());
        let context1 = match context.cast::<ID3D11DeviceContext1>() {
            Ok(c) => c,
            Err(_) => {
                self.partial.reset();
                return Ok(false);
            }
        };
        let size = (self.width, self.height);
        let uploaded = self.atlas.take_uploads();
        let mut partial = std::mem::take(&mut self.partial);
        let result = (|| -> Result<bool> {
            let Some(uploaded) = uploaded else {
                partial.reset();
                return Ok(false);
            };
            let canvas = {
                let mut drawer = PartialDrawer {
                    renderer: self,
                    scene,
                    clear,
                    saved: None,
                };
                partial.frame(
                    &device,
                    &context,
                    &context1,
                    size,
                    scene,
                    &uploaded,
                    &mut drawer,
                )?
            };
            let Some(canvas) = canvas else {
                return Ok(false);
            };
            let resources = self.resources.as_ref().context("resources missing")?;
            let buffer = resources
                .render_target
                .as_ref()
                .context("render target missing")?;
            // SAFETY: 两张纹理同尺寸同格式，都是活的渲染目标。
            unsafe { context.CopyResource(buffer, &canvas) };
            Ok(true)
        })();
        self.partial = partial;
        match result {
            Ok(true) => {
                self.present()?;
                Ok(true)
            }
            Ok(false) => Ok(false),
            Err(e) => {
                self.partial.reset();
                Err(e)
            }
        }
    }

    /// 把场景画进当前绑定的渲染目标，但不提交（`Present`）。
    ///
    /// 上屏与离屏读回共用这一段：差别只在渲染目标是谁、以及最后要不要 `Present`。
    fn draw_into(
        &mut self,
        scene: &Scene,
        background_appearance: WindowBackgroundAppearance,
    ) -> Result<()> {
        if self.skip_draws {
            // skip drawing this frame, we just recovered from a device lost event
            // and so likely do not have the textures anymore that are required for drawing
            return Ok(());
        }
        self.pre_draw(
            &match background_appearance {
                WindowBackgroundAppearance::Opaque => [1.0f32; 4],
                _ => [0.0f32; 4],
            },
            scene.quads.len(),
        )?;

        self.upload_scene_buffers(scene)?;
        self.draw_batches(scene)
    }

    /// 把场景的所有批次画一遍（不透明通道 + 混合通道），不含清屏与上传。
    /// partial 路径在 damage 的 scissor 内复用本方法。
    fn draw_batches(&mut self, scene: &Scene) -> Result<()> {
        // 不透明通道先画：从前往后、写深度，被遮住的后续片元在深度测试处失败。
        self.draw_opaque_quads(scene.quads.len())?;

        for batch in scene.batches() {
            match batch {
                PrimitiveBatch::Shadows(range) => self.draw_shadows(range.start, range.len()),
                PrimitiveBatch::Quads(range) => self.draw_quads(range.start, range.len()),
                PrimitiveBatch::Paths(range) => self.draw_path_tiles(&scene.paths[range]),
                PrimitiveBatch::Underlines(range) => self.draw_underlines(range.start, range.len()),
                PrimitiveBatch::MonochromeSprites { texture_id, range } => {
                    self.draw_monochrome_sprites(texture_id, range.start, range.len())
                }
                PrimitiveBatch::SubpixelSprites { texture_id, range } => {
                    self.draw_subpixel_sprites(texture_id, range.start, range.len())
                }
                PrimitiveBatch::PolychromeSprites { texture_id, range } => {
                    self.draw_polychrome_sprites(texture_id, range.start, range.len())
                }
                PrimitiveBatch::Surfaces(range) => self.draw_surfaces(&scene.surfaces[range]),
            }
            .context(format!(
                "scene too large:\
                {} paths, {} shadows, {} quads, {} underlines, {} mono, {} subpixel, {} poly, {} surfaces",
                scene.paths.len(),
                scene.shadows.len(),
                scene.quads.len(),
                scene.underlines.len(),
                scene.monochrome_sprites.len(),
                scene.subpixel_sprites.len(),
                scene.polychrome_sprites.len(),
                scene.surfaces.len(),
            ))?;
        }
        Ok(())
    }

    /// 把场景离屏渲染成 RGBA 像素（视觉测试 / 导出用）。
    ///
    /// 换一张与窗口同尺寸、同格式的离屏渲染目标，再走一遍 [`Self::draw_into`]，
    /// 因此**不会**把这一帧提交到屏幕，也不影响窗口当前显示的内容；读回后把窗口的
    /// 渲染目标原样换回去。顶点缓冲与字形图集仍复用窗口那份渲染器资源，
    /// 所以像素结果与上屏路径一致。
    #[cfg(feature = "test-support")]
    pub(crate) fn render_to_image(
        &mut self,
        scene: &Scene,
        background_appearance: WindowBackgroundAppearance,
    ) -> Result<image::RgbaImage> {
        if self.skip_draws {
            anyhow::bail!("渲染器刚从设备丢失中恢复，本次不能离屏渲染");
        }

        let width = self.width.max(1);
        let height = self.height.max(1);
        let devices = self.devices.clone().context("devices missing")?;

        let target = create_offscreen_render_target(&devices.device, width, height)?;
        let mut target_view = None;
        unsafe {
            devices
                .device
                .CreateRenderTargetView(&target, None, Some(&mut target_view))?
        };
        let target_view = target_view.context("创建离屏渲染目标视图失败")?;

        // 换成离屏目标：`draw_paths_from_intermediate` 恢复主目标时读的也是
        // `resources.render_target_view`，所以整条绘制链路（含路径的中间纹理回贴）
        // 都会落到离屏目标上。
        let (previous_target, previous_view) = {
            let resources = self.resources.as_mut().context("resources missing")?;
            (
                resources.render_target.replace(target.clone()),
                resources.render_target_view.replace(target_view),
            )
        };

        let result = self.draw_into(scene, background_appearance).and_then(|()| {
            read_render_target(
                &devices.device,
                &devices.device_context,
                &target,
                width,
                height,
            )
        });

        // 无论成功与否都把窗口的渲染目标换回去，别让后续正常绘制画到离屏纹理上。
        if let Some(resources) = self.resources.as_mut() {
            resources.render_target = previous_target;
            resources.render_target_view = previous_view;
        }

        result
    }

    /// 调整渲染器大小以匹配新的窗口尺寸
    ///
    /// # 参数
    /// * `new_size` - 新的窗口尺寸（设备像素）
    pub(crate) fn resize(&mut self, new_size: Size<DevicePixels>) -> Result<()> {
        let width = new_size.width.0.max(1) as u32;
        let height = new_size.height.0.max(1) as u32;
        if self.width == width && self.height == height {
            return Ok(());
        }
        self.width = width;
        self.height = height;

        // Clear the render target before resizing
        let devices = self.devices.as_ref().context("devices missing")?;
        unsafe { devices.device_context.OMSetRenderTargets(None, None) };
        let resources = self.resources.as_mut().context("resources missing")?;
        resources.render_target.take();
        resources.render_target_view.take();

        // Resizing the swap chain requires a call to the underlying DXGI adapter, which can return the device removed error.
        // The app might have moved to a monitor that's attached to a different graphics device.
        // When a graphics device is removed or reset, the desktop resolution often changes, resulting in a window size change.
        // But here we just return the error, because we are handling device lost scenarios elsewhere.
        unsafe {
            resources
                .swap_chain
                .ResizeBuffers(
                    BUFFER_COUNT as u32,
                    width,
                    height,
                    RENDER_TARGET_FORMAT,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )
                .context("Failed to resize swap chain")?;
        }

        resources.recreate_resources(devices, width, height)?;

        unsafe {
            devices.device_context.OMSetRenderTargets(
                Some(slice::from_ref(&resources.render_target_view)),
                resources.depth_stencil_view.as_ref(),
            );
        }

        Ok(())
    }

    fn upload_scene_buffers(&mut self, scene: &Scene) -> Result<()> {
        let devices = self.devices.as_ref().context("devices missing")?;

        if !scene.shadows.is_empty() {
            self.pipelines.shadow_pipeline.update_buffer(
                &devices.device,
                &devices.device_context,
                &scene.shadows,
            )?;
        }

        if !scene.quads.is_empty() {
            self.pipelines.quad_pipeline.update_buffer(
                &devices.device,
                &devices.device_context,
                &scene.quads,
            )?;
        }

        if !scene.underlines.is_empty() {
            self.pipelines.underline_pipeline.update_buffer(
                &devices.device,
                &devices.device_context,
                &scene.underlines,
            )?;
        }

        if !scene.monochrome_sprites.is_empty() {
            self.pipelines.mono_sprites.update_buffer(
                &devices.device,
                &devices.device_context,
                &scene.monochrome_sprites,
            )?;
        }

        if !scene.subpixel_sprites.is_empty() {
            self.pipelines.subpixel_sprites.update_buffer(
                &devices.device,
                &devices.device_context,
                &scene.subpixel_sprites,
            )?;
        }

        if !scene.polychrome_sprites.is_empty() {
            self.pipelines.poly_sprites.update_buffer(
                &devices.device,
                &devices.device_context,
                &scene.polychrome_sprites,
            )?;
        }

        Ok(())
    }

    fn draw_shadows(&mut self, start: usize, len: usize) -> Result<()> {
        if len == 0 {
            return Ok(());
        }
        let devices = self.devices.as_ref().context("devices missing")?;
        self.pipelines.shadow_pipeline.draw_range(
            &devices.device,
            &devices.device_context,
            slice::from_ref(
                &self
                    .resources
                    .as_ref()
                    .context("resources missing")?
                    .viewport,
            ),
            slice::from_ref(&self.globals.global_params_buffer),
            4,
            start as u32,
            len as u32,
        )
    }

    fn draw_quads(&mut self, start: usize, len: usize) -> Result<()> {
        if len == 0 {
            return Ok(());
        }
        let devices = self.devices.as_ref().context("devices missing")?;
        self.pipelines.quad_pipeline.draw_range(
            &devices.device,
            &devices.device_context,
            slice::from_ref(
                &self
                    .resources
                    .as_ref()
                    .context("resources missing")?
                    .viewport,
            ),
            slice::from_ref(&self.globals.global_params_buffer),
            4,
            start as u32,
            len as u32,
        )
    }

    /// 不透明通道：用 quad 管线的全量缓冲视图 + 不透明着色器，从前往后
    /// 画一遍（非候选实例在顶点着色器里被裁掉），开深度写、无混合。
    fn draw_opaque_quads(&mut self, quad_count: usize) -> Result<()> {
        if quad_count == 0 {
            return Ok(());
        }
        let devices = self.devices.as_ref().context("devices missing")?;
        let resources = self.resources.as_ref().context("resources missing")?;
        let quad_pipeline = &self.pipelines.quad_pipeline;
        let opaque = &self.pipelines.opaque_quads;
        set_pipeline_state(
            &devices.device_context,
            slice::from_ref(&quad_pipeline.view),
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            slice::from_ref(&resources.viewport),
            &opaque.vertex,
            &opaque.fragment,
            slice::from_ref(&self.globals.global_params_buffer),
            &opaque.blend_state,
            &opaque.depth_state,
        );
        unsafe {
            devices
                .device_context
                .DrawInstanced(4, quad_count as u32, 0, 0);
        }
        Ok(())
    }

    /// 路径瓦片批：把批内路径平铺成绘制参数 / 曲线 / 瓦片三条数组，瓦片视图
    /// 按批区间绑定（实例下标批内从 0 起），曲线与绘制参数绑全量视图
    /// （`tile.paint` / `tile.curve_start` 是全帧下标）。
    fn draw_path_tiles(&mut self, paths: &[PaintedPath]) -> Result<()> {
        let mut paints = Vec::new();
        let mut curves = Vec::new();
        let mut tiles = Vec::new();
        for path in paths {
            path.append_to(&mut paints, &mut curves, &mut tiles);
        }
        if tiles.is_empty() {
            return Ok(());
        }

        let devices = self.devices.as_ref().context("devices missing")?;
        self.pipelines.path_paints.update_buffer(
            &devices.device,
            &devices.device_context,
            paints.as_slice(),
        )?;
        self.pipelines.path_curves.update_buffer(
            &devices.device,
            &devices.device_context,
            curves.as_slice(),
        )?;
        self.pipelines.path_tile_pipeline.update_buffer(
            &devices.device,
            &devices.device_context,
            tiles.as_slice(),
        )?;

        let Some(paints_view) = self.pipelines.path_paints.view.clone() else {
            return Ok(());
        };
        let Some(curves_view) = self.pipelines.path_curves.view.clone() else {
            return Ok(());
        };
        let tile_view = create_buffer_view_range(
            &devices.device,
            &self.pipelines.path_tile_pipeline.buffer,
            0,
            tiles.len() as u32,
        )?;
        if tile_view.is_none() {
            return Ok(());
        }

        let resources = self.resources.as_ref().context("resources missing")?;
        set_pipeline_state(
            &devices.device_context,
            slice::from_ref(&tile_view),
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            slice::from_ref(&resources.viewport),
            &self.pipelines.path_tile_pipeline.vertex,
            &self.pipelines.path_tile_pipeline.fragment,
            slice::from_ref(&self.globals.global_params_buffer),
            &self.pipelines.path_tile_pipeline.blend_state,
            &self.pipelines.path_tile_pipeline.depth_state,
        );
        let paints_binding = [Some(paints_view)];
        let curves_binding = [Some(curves_view)];
        unsafe {
            // t2 = 曲线（全量视图），t3 = 绘制参数（全量视图）。
            devices
                .device_context
                .VSSetShaderResources(2, Some(&curves_binding));
            devices
                .device_context
                .PSSetShaderResources(2, Some(&curves_binding));
            devices
                .device_context
                .VSSetShaderResources(3, Some(&paints_binding));
            devices
                .device_context
                .PSSetShaderResources(3, Some(&paints_binding));
            devices
                .device_context
                .DrawInstanced(4, tiles.len() as u32, 0, 0);
        }
        Ok(())
    }

    fn draw_underlines(&mut self, start: usize, len: usize) -> Result<()> {
        if len == 0 {
            return Ok(());
        }
        let devices = self.devices.as_ref().context("devices missing")?;
        let resources = self.resources.as_ref().context("resources missing")?;
        self.pipelines.underline_pipeline.draw_range(
            &devices.device,
            &devices.device_context,
            slice::from_ref(&resources.viewport),
            slice::from_ref(&self.globals.global_params_buffer),
            4,
            start as u32,
            len as u32,
        )
    }

    fn draw_monochrome_sprites(
        &mut self,
        texture_id: AtlasTextureId,
        start: usize,
        len: usize,
    ) -> Result<()> {
        if len == 0 {
            return Ok(());
        }
        let devices = self.devices.as_ref().context("devices missing")?;
        let resources = self.resources.as_ref().context("resources missing")?;
        // 纹理可能在场景构建后被释放（缓存视图重放旧绘制），此时跳过这批精灵。
        let Some(texture_view) = self.atlas.get_texture_view(texture_id) else {
            return Ok(());
        };
        self.pipelines.mono_sprites.draw_range_with_texture(
            &devices.device,
            &devices.device_context,
            &texture_view,
            slice::from_ref(&resources.viewport),
            slice::from_ref(&self.globals.global_params_buffer),
            slice::from_ref(&self.globals.sampler),
            start as u32,
            len as u32,
        )
    }

    fn draw_subpixel_sprites(
        &mut self,
        texture_id: AtlasTextureId,
        start: usize,
        len: usize,
    ) -> Result<()> {
        if len == 0 {
            return Ok(());
        }
        let devices = self.devices.as_ref().context("devices missing")?;
        let resources = self.resources.as_ref().context("resources missing")?;
        // 纹理可能在场景构建后被释放（缓存视图重放旧绘制），此时跳过这批精灵。
        let Some(texture_view) = self.atlas.get_texture_view(texture_id) else {
            return Ok(());
        };
        self.pipelines.subpixel_sprites.draw_range_with_texture(
            &devices.device,
            &devices.device_context,
            &texture_view,
            slice::from_ref(&resources.viewport),
            slice::from_ref(&self.globals.global_params_buffer),
            slice::from_ref(&self.globals.sampler),
            start as u32,
            len as u32,
        )
    }

    fn draw_polychrome_sprites(
        &mut self,
        texture_id: AtlasTextureId,
        start: usize,
        len: usize,
    ) -> Result<()> {
        if len == 0 {
            return Ok(());
        }
        let devices = self.devices.as_ref().context("devices missing")?;
        let resources = self.resources.as_ref().context("resources missing")?;
        // 纹理可能在场景构建后被释放（缓存视图重放旧绘制），此时跳过这批精灵。
        let Some(texture_view) = self.atlas.get_texture_view(texture_id) else {
            return Ok(());
        };
        self.pipelines.poly_sprites.draw_range_with_texture(
            &devices.device,
            &devices.device_context,
            &texture_view,
            slice::from_ref(&resources.viewport),
            slice::from_ref(&self.globals.global_params_buffer),
            slice::from_ref(&self.globals.sampler),
            start as u32,
            len as u32,
        )
    }

    fn draw_surfaces(&mut self, surfaces: &[PaintSurface]) -> Result<()> {
        if surfaces.is_empty() {
            return Ok(());
        }
        Ok(())
    }

    /// 获取 GPU 规格信息
    pub(crate) fn gpu_specs(&self) -> Result<GpuSpecs> {
        let devices = self.devices.as_ref().context("devices missing")?;
        let desc = unsafe { devices.adapter.GetDesc1() }?;
        let is_software_emulated = (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0;
        let device_name = String::from_utf16_lossy(&desc.Description)
            .trim_matches(char::from(0))
            .to_string();
        let driver_name = match desc.VendorId {
            0x10DE => "NVIDIA Corporation".to_string(),
            0x1002 => "AMD Corporation".to_string(),
            0x8086 => "Intel Corporation".to_string(),
            id => format!("Unknown Vendor (ID: {:#X})", id),
        };
        let driver_version = match desc.VendorId {
            0x10DE => nvidia::get_driver_version(),
            0x1002 => amd::get_driver_version(),
            // For Intel and other vendors, we use the DXGI API to get the driver version.
            _ => dxgi::get_driver_version(&devices.adapter),
        }
        .context("Failed to get gpu driver info")
        .log_err()
        .unwrap_or("Unknown Driver".to_string());
        Ok(GpuSpecs {
            is_software_emulated,
            device_name,
            driver_name,
            driver_info: driver_version,
        })
    }

    /// 获取字体渲染信息（缓存的静态实例）
    pub(crate) fn get_font_info() -> &'static FontInfo {
        static CACHED_FONT_INFO: OnceLock<FontInfo> = OnceLock::new();
        CACHED_FONT_INFO.get_or_init(|| {
            let factory: IDWriteFactory5 =
                unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }
                    .expect("创建 DWrite 工厂失败");
            let render_params: IDWriteRenderingParams1 = unsafe {
                factory
                    .CreateRenderingParams()
                    .expect("创建渲染参数失败")
                    .cast()
                    .expect("渲染参数类型转换失败")
            };
            FontInfo {
                gamma_ratios: rgpui::get_gamma_correction_ratios(unsafe {
                    render_params.GetGamma()
                }),
                grayscale_enhanced_contrast: unsafe {
                    render_params.GetGrayscaleEnhancedContrast()
                },
                subpixel_enhanced_contrast: unsafe { render_params.GetEnhancedContrast() },
                is_bgr: unsafe { render_params.GetPixelGeometry() } == DWRITE_PIXEL_GEOMETRY_BGR,
            }
        })
    }

    /// 标记渲染器为可绘制状态
    ///
    /// 在 GPU 设备丢失恢复后调用，允许下一帧正常渲染
    pub(crate) fn mark_drawable(&mut self) {
        self.skip_draws = false;
    }
}

impl DirectXResources {
    pub fn new(
        devices: &DirectXRendererDevices,
        width: u32,
        height: u32,
        hwnd: HWND,
        disable_direct_composition: bool,
    ) -> Result<Self> {
        let swap_chain = if disable_direct_composition {
            create_swap_chain(&devices.dxgi_factory, &devices.device, hwnd, width, height)?
        } else {
            create_swap_chain_for_composition(
                &devices.dxgi_factory,
                &devices.device,
                width,
                height,
            )?
        };

        let (render_target, render_target_view, depth_stencil_view, viewport) =
            create_resources(devices, &swap_chain, width, height)?;
        set_rasterizer_state(&devices.device, &devices.device_context)?;

        Ok(Self {
            swap_chain,
            render_target: Some(render_target),
            render_target_view,
            depth_stencil_view,
            viewport,
        })
    }

    #[inline]
    fn recreate_resources(
        &mut self,
        devices: &DirectXRendererDevices,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let (render_target, render_target_view, depth_stencil_view, viewport) =
            create_resources(devices, &self.swap_chain, width, height)?;
        self.render_target = Some(render_target);
        self.render_target_view = render_target_view;
        self.depth_stencil_view = depth_stencil_view;
        self.viewport = viewport;
        Ok(())
    }
}

impl DirectXRenderPipelines {
    pub fn new(device: &ID3D11Device) -> Result<Self> {
        // 三档深度态：非 quad 图元关闭深度；混合 quad 只测不写；
        // 不透明 quad 测试并写入。比较都是 Greater（「越大越近」）。
        let no_depth_state = create_depth_stencil_state(device, false, false)?;
        let quad_depth_state = create_depth_stencil_state(device, true, false)?;
        let opaque_depth_state = create_depth_stencil_state(device, true, true)?;

        let shadow_pipeline = PipelineState::new(
            device,
            "shadow_pipeline",
            ShaderModule::Shadow,
            4,
            create_blend_state(device)?,
            no_depth_state.clone(),
        )?;
        let quad_pipeline = PipelineState::new(
            device,
            "quad_pipeline",
            ShaderModule::Quad,
            64,
            create_blend_state(device)?,
            quad_depth_state,
        )?;
        let opaque_quads = OpaqueQuadPipeline {
            vertex: {
                let raw_shader =
                    RawShaderBytes::new(ShaderModule::OpaqueQuad, ShaderTarget::Vertex)?;
                create_vertex_shader(device, raw_shader.as_bytes())?
            },
            fragment: {
                let raw_shader =
                    RawShaderBytes::new(ShaderModule::OpaqueQuad, ShaderTarget::Fragment)?;
                create_fragment_shader(device, raw_shader.as_bytes())?
            },
            blend_state: create_opaque_blend_state(device)?,
            depth_state: opaque_depth_state,
        };
        let path_tile_pipeline = PipelineState::new(
            device,
            "path_tile_pipeline",
            ShaderModule::PathTile,
            64,
            create_blend_state(device)?,
            no_depth_state.clone(),
        )?;
        let path_curves = InstanceBuffer::new(device, "path_curves", 256)?;
        let path_paints = InstanceBuffer::new(device, "path_paints", 16)?;
        let underline_pipeline = PipelineState::new(
            device,
            "underline_pipeline",
            ShaderModule::Underline,
            4,
            create_blend_state(device)?,
            no_depth_state.clone(),
        )?;
        let mono_sprites = PipelineState::new(
            device,
            "monochrome_sprite_pipeline",
            ShaderModule::MonochromeSprite,
            512,
            create_blend_state(device)?,
            no_depth_state.clone(),
        )?;
        let subpixel_sprites = PipelineState::new(
            device,
            "subpixel_sprite_pipeline",
            ShaderModule::SubpixelSprite,
            512,
            create_blend_state_for_subpixel_rendering(device)?,
            no_depth_state.clone(),
        )?;
        let poly_sprites = PipelineState::new(
            device,
            "polychrome_sprite_pipeline",
            ShaderModule::PolychromeSprite,
            16,
            create_blend_state(device)?,
            no_depth_state,
        )?;

        Ok(Self {
            shadow_pipeline,
            quad_pipeline,
            path_tile_pipeline,
            path_curves,
            path_paints,
            underline_pipeline,
            mono_sprites,
            subpixel_sprites,
            poly_sprites,
            opaque_quads,
        })
    }
}

impl DirectComposition {
    pub fn new(dxgi_device: &IDXGIDevice, hwnd: HWND) -> Result<Self> {
        let comp_device = get_comp_device(dxgi_device)?;
        let comp_target = unsafe { comp_device.CreateTargetForHwnd(hwnd, false) }?;
        let comp_visual = unsafe { comp_device.CreateVisual() }?;

        Ok(Self {
            comp_device,
            comp_target,
            comp_visual,
        })
    }

    pub fn set_swap_chain(&self, swap_chain: &IDXGISwapChain1) -> Result<()> {
        unsafe {
            self.comp_visual.SetContent(swap_chain)?;
            self.comp_target.SetRoot(&self.comp_visual)?;
            self.comp_device.Commit()?;
        }
        Ok(())
    }
}

impl DirectXGlobalElements {
    pub fn new(device: &ID3D11Device) -> Result<Self> {
        let global_params_buffer = unsafe {
            let desc = D3D11_BUFFER_DESC {
                ByteWidth: std::mem::size_of::<GlobalParams>() as u32,
                Usage: D3D11_USAGE_DYNAMIC,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                ..Default::default()
            };
            let mut buffer = None;
            device.CreateBuffer(&desc, None, Some(&mut buffer))?;
            buffer
        };

        let sampler = unsafe {
            let desc = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                AddressU: D3D11_TEXTURE_ADDRESS_WRAP,
                AddressV: D3D11_TEXTURE_ADDRESS_WRAP,
                AddressW: D3D11_TEXTURE_ADDRESS_WRAP,
                MipLODBias: 0.0,
                MaxAnisotropy: 1,
                ComparisonFunc: D3D11_COMPARISON_ALWAYS,
                BorderColor: [0.0; 4],
                MinLOD: 0.0,
                MaxLOD: D3D11_FLOAT32_MAX,
            };
            let mut output = None;
            device.CreateSamplerState(&desc, Some(&mut output))?;
            output
        };

        Ok(Self {
            global_params_buffer,
            sampler,
        })
    }
}

#[derive(Debug, Default)]
#[repr(C)]
struct GlobalParams {
    gamma_ratios: [f32; 4],
    viewport_size: [f32; 2],
    grayscale_enhanced_contrast: f32,
    subpixel_enhanced_contrast: f32,
    is_bgr: u32,
    /// 不透明通道反向索引用的全局 quad 数。
    quad_count: u32,
    _pad: [u32; 2],
}

struct PipelineState<T> {
    label: &'static str,
    vertex: ID3D11VertexShader,
    fragment: ID3D11PixelShader,
    buffer: ID3D11Buffer,
    buffer_size: usize,
    view: Option<ID3D11ShaderResourceView>,
    blend_state: ID3D11BlendState,
    depth_state: ID3D11DepthStencilState,
    _marker: std::marker::PhantomData<T>,
}

impl<T> PipelineState<T> {
    fn new(
        device: &ID3D11Device,
        label: &'static str,
        shader_module: ShaderModule,
        buffer_size: usize,
        blend_state: ID3D11BlendState,
        depth_state: ID3D11DepthStencilState,
    ) -> Result<Self> {
        let vertex = {
            let raw_shader = RawShaderBytes::new(shader_module, ShaderTarget::Vertex)?;
            create_vertex_shader(device, raw_shader.as_bytes())?
        };
        let fragment = {
            let raw_shader = RawShaderBytes::new(shader_module, ShaderTarget::Fragment)?;
            create_fragment_shader(device, raw_shader.as_bytes())?
        };
        let buffer = create_buffer(device, std::mem::size_of::<T>(), buffer_size)?;
        let view = create_buffer_view(device, &buffer)?;

        Ok(PipelineState {
            label,
            vertex,
            fragment,
            buffer,
            buffer_size,
            view,
            blend_state,
            depth_state,
            _marker: std::marker::PhantomData,
        })
    }

    fn update_buffer(
        &mut self,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        data: &[T],
    ) -> Result<()> {
        if self.buffer_size < data.len() {
            let new_buffer_size = data.len().next_power_of_two();
            log::debug!(
                "Updating {} buffer size from {} to {}",
                self.label,
                self.buffer_size,
                new_buffer_size
            );
            let buffer = create_buffer(device, std::mem::size_of::<T>(), new_buffer_size)?;
            let view = create_buffer_view(device, &buffer)?;
            self.buffer = buffer;
            self.view = view;
            self.buffer_size = new_buffer_size;
        }
        update_buffer(device_context, &self.buffer, data)
    }

    fn draw_range(
        &self,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        viewport: &[D3D11_VIEWPORT],
        global_params: &[Option<ID3D11Buffer>],
        vertex_count: u32,
        first_instance: u32,
        instance_count: u32,
    ) -> Result<()> {
        let view = create_buffer_view_range(device, &self.buffer, first_instance, instance_count)?;
        set_pipeline_state(
            device_context,
            slice::from_ref(&view),
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            viewport,
            &self.vertex,
            &self.fragment,
            global_params,
            &self.blend_state,
            &self.depth_state,
        );
        unsafe {
            device_context.DrawInstanced(vertex_count, instance_count, 0, 0);
        }
        Ok(())
    }

    fn draw_range_with_texture(
        &self,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        texture: &[Option<ID3D11ShaderResourceView>],
        viewport: &[D3D11_VIEWPORT],
        global_params: &[Option<ID3D11Buffer>],
        sampler: &[Option<ID3D11SamplerState>],
        first_instance: u32,
        instance_count: u32,
    ) -> Result<()> {
        let view = create_buffer_view_range(device, &self.buffer, first_instance, instance_count)?;
        set_pipeline_state(
            device_context,
            slice::from_ref(&view),
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            viewport,
            &self.vertex,
            &self.fragment,
            global_params,
            &self.blend_state,
            &self.depth_state,
        );
        unsafe {
            device_context.PSSetSamplers(0, Some(sampler));
            device_context.VSSetShaderResources(0, Some(texture));
            device_context.PSSetShaderResources(0, Some(texture));
            device_context.DrawInstanced(4, instance_count, 0, 0);
        }
        Ok(())
    }
}

// 路径与瓦片/曲线/绘制参数的 GPU 布局与核心 scene.rs 的同名类型一致，
// 直接复用 `rgpui::{PathCurve, PathTile, PathPaint}`，避免两处维护。

/// 部分重绘时供 `fast::partial` 调用的绘制接口。
///
/// 持有渲染器可变借用；每次 `begin` 临时把 canvas 的颜色/深度视图换进
/// `resources`，`drop` 时恢复——路径栅格化完成后会读回 `render_target_view`。
struct PartialDrawer<'a> {
    renderer: &'a mut DirectXRenderer,
    scene: &'a Scene,
    clear: [f32; 4],
    /// begin 前的窗口渲染目标，drop 时恢复。
    saved: Option<(
        Option<ID3D11RenderTargetView>,
        Option<ID3D11DepthStencilView>,
    )>,
}

impl crate::fast::partial::Drawer for PartialDrawer<'_> {
    fn clear(&self) -> [f32; 4] {
        self.clear
    }

    fn prepare(&mut self) -> Result<()> {
        self.renderer.upload_scene_buffers(self.scene)
    }

    fn begin(
        &mut self,
        color: &Option<ID3D11RenderTargetView>,
        depth: &Option<ID3D11DepthStencilView>,
        clear: [f32; 4],
    ) -> Result<()> {
        let resources = self
            .renderer
            .resources
            .as_mut()
            .context("resources missing")?;
        // 只保存一次：整帧路径里 begin 可能被调用多轮（每个 damage 矩形）。
        if self.saved.is_none() {
            self.saved = Some((
                resources.render_target_view.clone(),
                resources.depth_stencil_view.clone(),
            ));
        }
        resources.render_target_view = color.clone();
        resources.depth_stencil_view = depth.clone();
        self.renderer.pre_draw(&clear, self.scene.quads.len())
    }

    fn draw(&mut self) -> Result<()> {
        self.renderer.draw_batches(self.scene)
    }
}

impl Drop for PartialDrawer<'_> {
    fn drop(&mut self) {
        if let (Some(saved), Some(resources)) =
            (self.saved.take(), self.renderer.resources.as_mut())
        {
            resources.render_target_view = saved.0;
            resources.depth_stencil_view = saved.1;
        }
    }
}

impl Drop for DirectXRenderer {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        if let Some(devices) = &self.devices {
            report_live_objects(&devices.device).ok();
        }
    }
}

#[inline]
fn get_comp_device(dxgi_device: &IDXGIDevice) -> Result<IDCompositionDevice> {
    Ok(unsafe { DCompositionCreateDevice(dxgi_device)? })
}

fn create_swap_chain_for_composition(
    dxgi_factory: &IDXGIFactory6,
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<IDXGISwapChain1> {
    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: width,
        Height: height,
        Format: RENDER_TARGET_FORMAT,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: BUFFER_COUNT as u32,
        // Composition SwapChains only support the DXGI_SCALING_STRETCH Scaling.
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
        Flags: 0,
    };
    Ok(unsafe { dxgi_factory.CreateSwapChainForComposition(device, &desc, None)? })
}

fn create_swap_chain(
    dxgi_factory: &IDXGIFactory6,
    device: &ID3D11Device,
    hwnd: HWND,
    width: u32,
    height: u32,
) -> Result<IDXGISwapChain1> {
    use windows::Win32::Graphics::Dxgi::DXGI_MWA_NO_ALT_ENTER;

    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: width,
        Height: height,
        Format: RENDER_TARGET_FORMAT,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: BUFFER_COUNT as u32,
        Scaling: DXGI_SCALING_NONE,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_IGNORE,
        Flags: 0,
    };
    let swap_chain =
        unsafe { dxgi_factory.CreateSwapChainForHwnd(device, hwnd, &desc, None, None) }?;
    unsafe { dxgi_factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER) }?;
    Ok(swap_chain)
}

#[inline]
fn create_resources(
    devices: &DirectXRendererDevices,
    swap_chain: &IDXGISwapChain1,
    width: u32,
    height: u32,
) -> Result<(
    ID3D11Texture2D,
    Option<ID3D11RenderTargetView>,
    Option<ID3D11DepthStencilView>,
    D3D11_VIEWPORT,
)> {
    let (render_target, render_target_view) =
        create_render_target_and_its_view(swap_chain, &devices.device)?;
    let depth_stencil_view = create_depth_stencil_and_view(&devices.device, width, height)?;
    let viewport = set_viewport(&devices.device_context, width as f32, height as f32);
    Ok((
        render_target,
        render_target_view,
        depth_stencil_view,
        viewport,
    ))
}

/// anti-overdraw 深度纹理（D32_FLOAT）与其深度模板视图。
/// 只返回视图：D3D11 的 DSV 自持纹理引用，纹理随视图存活。
#[inline]
fn create_depth_stencil_and_view(
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<Option<ID3D11DepthStencilView>> {
    let texture = unsafe {
        let mut output = None;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_D32_FLOAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_DEPTH_STENCIL.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        device.CreateTexture2D(&desc, None, Some(&mut output))?;
        output.context("创建深度纹理失败")?
    };
    let mut view = None;
    unsafe { device.CreateDepthStencilView(&texture, None, Some(&mut view))? };
    Ok(view)
}

#[inline]
fn create_render_target_and_its_view(
    swap_chain: &IDXGISwapChain1,
    device: &ID3D11Device,
) -> Result<(ID3D11Texture2D, Option<ID3D11RenderTargetView>)> {
    let render_target: ID3D11Texture2D = unsafe { swap_chain.GetBuffer(0) }?;
    let mut render_target_view = None;
    unsafe { device.CreateRenderTargetView(&render_target, None, Some(&mut render_target_view))? };
    Ok((render_target, render_target_view))
}

/// 创建离屏渲染目标（与交换链同格式，默认用途，可作渲染目标）。
#[cfg(any(test, feature = "test-support"))]
#[inline]
fn create_offscreen_render_target(
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<ID3D11Texture2D> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: RENDER_TARGET_FORMAT,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut texture = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture))? };
    texture.context("创建离屏渲染目标纹理失败")
}

/// 把离屏渲染目标读回成 RGBA8 像素（源格式是 BGRA，这里交换 R / B 通道）。
///
/// 默认用途的纹理不能直接 `Map`：先 `CopyResource` 到
/// `D3D11_USAGE_STAGING` + `CPU_ACCESS_READ` 的副本，再按行（行跨度是 `RowPitch`，
/// 通常大于 `width * 4`）拷进紧密排布的像素缓冲。
#[cfg(any(test, feature = "test-support"))]
fn read_render_target(
    device: &ID3D11Device,
    device_context: &ID3D11DeviceContext,
    render_target: &ID3D11Texture2D,
    width: u32,
    height: u32,
) -> Result<image::RgbaImage> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: RENDER_TARGET_FORMAT,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
    };
    let mut staging = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut staging))? };
    let staging = staging.context("创建读回纹理失败")?;

    unsafe { device_context.CopyResource(&staging, render_target) };

    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe { device_context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))? };

    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    for row in 0..height as usize {
        let source_row = unsafe { (mapped.pData as *const u8).add(row * mapped.RowPitch as usize) };
        for column in 0..width as usize {
            let source = unsafe { source_row.add(column * 4) };
            let offset = (row * width as usize + column) * 4;
            unsafe {
                pixels[offset] = *source.add(2);
                pixels[offset + 1] = *source.add(1);
                pixels[offset + 2] = *source;
                pixels[offset + 3] = *source.add(3);
            }
        }
    }

    unsafe { device_context.Unmap(&staging, 0) };

    image::RgbaImage::from_raw(width, height, pixels).context("读回像素尺寸与图像不符")
}

#[inline]
fn set_viewport(device_context: &ID3D11DeviceContext, width: f32, height: f32) -> D3D11_VIEWPORT {
    let viewport = [D3D11_VIEWPORT {
        TopLeftX: 0.0,
        TopLeftY: 0.0,
        Width: width,
        Height: height,
        MinDepth: 0.0,
        MaxDepth: 1.0,
    }];
    unsafe { device_context.RSSetViewports(Some(&viewport)) };
    viewport[0]
}

#[inline]
fn set_rasterizer_state(device: &ID3D11Device, device_context: &ID3D11DeviceContext) -> Result<()> {
    let desc = D3D11_RASTERIZER_DESC {
        FillMode: D3D11_FILL_SOLID,
        CullMode: D3D11_CULL_NONE,
        FrontCounterClockwise: false.into(),
        DepthBias: 0,
        DepthBiasClamp: 0.0,
        SlopeScaledDepthBias: 0.0,
        DepthClipEnable: true.into(),
        ScissorEnable: false.into(),
        MultisampleEnable: true.into(),
        AntialiasedLineEnable: false.into(),
    };
    let rasterizer_state = unsafe {
        let mut state = None;
        device.CreateRasterizerState(&desc, Some(&mut state))?;
        state.unwrap()
    };
    unsafe { device_context.RSSetState(&rasterizer_state) };
    Ok(())
}

// 参见 https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ns-d3d11-d3d11_blend_desc
#[inline]
fn create_blend_state(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = true.into();
    desc.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].SrcBlend = D3D11_BLEND_SRC_ALPHA;
    desc.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
    desc.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;
    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.unwrap())
    }
}

#[inline]
fn create_blend_state_for_subpixel_rendering(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = true.into();
    desc.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].SrcBlend = D3D11_BLEND_SRC1_COLOR;
    desc.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC1_COLOR;
    // It does not make sense to draw transparent subpixel-rendered text, since it cannot be meaningfully alpha-blended onto anything else.
    desc.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_ZERO;
    desc.RenderTarget[0].RenderTargetWriteMask =
        D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8 & !D3D11_COLOR_WRITE_ENABLE_ALPHA.0 as u8;

    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.unwrap())
    }
}

/// 深度模板状态：`enabled=false` 关闭深度测试（非 quad 图元）；
/// `write=true` 时写入深度。比较一律 Greater——深度「越大越近」，
/// 不透明通道从前往后画，更远的片元直接失败。
#[inline]
fn create_depth_stencil_state(
    device: &ID3D11Device,
    enabled: bool,
    write: bool,
) -> Result<ID3D11DepthStencilState> {
    let desc = D3D11_DEPTH_STENCIL_DESC {
        DepthEnable: enabled.into(),
        DepthWriteMask: if write {
            D3D11_DEPTH_WRITE_MASK_ALL
        } else {
            D3D11_DEPTH_WRITE_MASK_ZERO
        },
        DepthFunc: D3D11_COMPARISON_GREATER,
        ..Default::default()
    };
    unsafe {
        let mut state = None;
        device.CreateDepthStencilState(&desc, Some(&mut state))?;
        Ok(state.unwrap())
    }
}

/// 不透明通道的混合状态：完全关闭混合，颜色直写。
#[inline]
fn create_opaque_blend_state(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    let desc = D3D11_BLEND_DESC {
        RenderTarget: [D3D11_RENDER_TARGET_BLEND_DESC {
            BlendEnable: false.into(),
            RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            ..Default::default()
        }; 8],
        ..Default::default()
    };
    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.unwrap())
    }
}

#[inline]
fn create_vertex_shader(device: &ID3D11Device, bytes: &[u8]) -> Result<ID3D11VertexShader> {
    unsafe {
        let mut shader = None;
        device.CreateVertexShader(bytes, None, Some(&mut shader))?;
        Ok(shader.unwrap())
    }
}

#[inline]
fn create_fragment_shader(device: &ID3D11Device, bytes: &[u8]) -> Result<ID3D11PixelShader> {
    unsafe {
        let mut shader = None;
        device.CreatePixelShader(bytes, None, Some(&mut shader))?;
        Ok(shader.unwrap())
    }
}

#[inline]
fn create_buffer(
    device: &ID3D11Device,
    element_size: usize,
    buffer_size: usize,
) -> Result<ID3D11Buffer> {
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: (element_size * buffer_size) as u32,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        MiscFlags: D3D11_RESOURCE_MISC_BUFFER_STRUCTURED.0 as u32,
        StructureByteStride: element_size as u32,
    };
    let mut buffer = None;
    unsafe { device.CreateBuffer(&desc, None, Some(&mut buffer)) }?;
    Ok(buffer.unwrap())
}

#[inline]
fn create_buffer_view(
    device: &ID3D11Device,
    buffer: &ID3D11Buffer,
) -> Result<Option<ID3D11ShaderResourceView>> {
    let mut view = None;
    unsafe { device.CreateShaderResourceView(buffer, None, Some(&mut view)) }?;
    Ok(view)
}

#[inline]
fn create_buffer_view_range(
    device: &ID3D11Device,
    buffer: &ID3D11Buffer,
    first_element: u32,
    num_elements: u32,
) -> Result<Option<ID3D11ShaderResourceView>> {
    let desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
        Format: DXGI_FORMAT_UNKNOWN,
        ViewDimension: D3D11_SRV_DIMENSION_BUFFER,
        Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
            Buffer: D3D11_BUFFER_SRV {
                Anonymous1: D3D11_BUFFER_SRV_0 {
                    FirstElement: first_element,
                },
                Anonymous2: D3D11_BUFFER_SRV_1 {
                    NumElements: num_elements,
                },
            },
        },
    };
    let mut view = None;
    unsafe { device.CreateShaderResourceView(buffer, Some(&desc), Some(&mut view)) }?;
    Ok(view)
}

#[inline]
fn update_buffer<T>(
    device_context: &ID3D11DeviceContext,
    buffer: &ID3D11Buffer,
    data: &[T],
) -> Result<()> {
    unsafe {
        let mut dest = std::mem::zeroed();
        device_context.Map(buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut dest))?;
        std::ptr::copy_nonoverlapping(data.as_ptr(), dest.pData as _, data.len());
        device_context.Unmap(buffer, 0);
    }
    Ok(())
}

#[inline]
fn set_pipeline_state(
    device_context: &ID3D11DeviceContext,
    buffer_view: &[Option<ID3D11ShaderResourceView>],
    topology: D3D_PRIMITIVE_TOPOLOGY,
    viewport: &[D3D11_VIEWPORT],
    vertex_shader: &ID3D11VertexShader,
    fragment_shader: &ID3D11PixelShader,
    global_params: &[Option<ID3D11Buffer>],
    blend_state: &ID3D11BlendState,
    depth_state: &ID3D11DepthStencilState,
) {
    unsafe {
        device_context.VSSetShaderResources(1, Some(buffer_view));
        device_context.PSSetShaderResources(1, Some(buffer_view));
        device_context.IASetPrimitiveTopology(topology);
        device_context.RSSetViewports(Some(viewport));
        device_context.VSSetShader(vertex_shader, None);
        device_context.PSSetShader(fragment_shader, None);
        device_context.VSSetConstantBuffers(0, Some(global_params));
        device_context.PSSetConstantBuffers(0, Some(global_params));
        device_context.OMSetBlendState(blend_state, None, 0xFFFFFFFF);
        device_context.OMSetDepthStencilState(depth_state, 0);
    }
}

#[cfg(debug_assertions)]
fn report_live_objects(device: &ID3D11Device) -> Result<()> {
    let debug_device: ID3D11Debug = device.cast()?;
    unsafe {
        debug_device.ReportLiveDeviceObjects(D3D11_RLDO_DETAIL)?;
    }
    Ok(())
}

const BUFFER_COUNT: usize = 3;

pub(crate) mod shader_resources {
    use anyhow::Result;

    #[cfg(debug_assertions)]
    use windows::{
        Win32::Graphics::Direct3D::{
            Fxc::{D3DCOMPILE_DEBUG, D3DCOMPILE_SKIP_OPTIMIZATION, D3DCompileFromFile},
            ID3DBlob,
        },
        core::{HSTRING, PCSTR},
    };

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub(crate) enum ShaderModule {
        Quad,
        OpaqueQuad,
        Shadow,
        Underline,
        PathTile,
        MonochromeSprite,
        SubpixelSprite,
        PolychromeSprite,
        EmojiRasterization,
    }

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub(crate) enum ShaderTarget {
        Vertex,
        Fragment,
    }

    pub(crate) struct RawShaderBytes<'t> {
        inner: &'t [u8],

        #[cfg(debug_assertions)]
        _blob: ID3DBlob,
    }

    impl<'t> RawShaderBytes<'t> {
        pub(crate) fn new(module: ShaderModule, target: ShaderTarget) -> Result<Self> {
            #[cfg(not(debug_assertions))]
            {
                Ok(Self::from_bytes(module, target))
            }
            #[cfg(debug_assertions)]
            {
                let blob = build_shader_blob(module, target)?;
                let inner = unsafe {
                    std::slice::from_raw_parts(
                        blob.GetBufferPointer() as *const u8,
                        blob.GetBufferSize(),
                    )
                };
                Ok(Self { inner, _blob: blob })
            }
        }

        pub(crate) fn as_bytes(&'t self) -> &'t [u8] {
            self.inner
        }

        #[cfg(not(debug_assertions))]
        fn from_bytes(module: ShaderModule, target: ShaderTarget) -> Self {
            let bytes = match module {
                ShaderModule::Quad => match target {
                    ShaderTarget::Vertex => QUAD_VERTEX_BYTES,
                    ShaderTarget::Fragment => QUAD_FRAGMENT_BYTES,
                },
                ShaderModule::OpaqueQuad => match target {
                    ShaderTarget::Vertex => OPAQUE_QUAD_VERTEX_BYTES,
                    ShaderTarget::Fragment => OPAQUE_QUAD_FRAGMENT_BYTES,
                },
                ShaderModule::Shadow => match target {
                    ShaderTarget::Vertex => SHADOW_VERTEX_BYTES,
                    ShaderTarget::Fragment => SHADOW_FRAGMENT_BYTES,
                },
                ShaderModule::Underline => match target {
                    ShaderTarget::Vertex => UNDERLINE_VERTEX_BYTES,
                    ShaderTarget::Fragment => UNDERLINE_FRAGMENT_BYTES,
                },
                ShaderModule::PathTile => match target {
                    ShaderTarget::Vertex => PATH_TILE_VERTEX_BYTES,
                    ShaderTarget::Fragment => PATH_TILE_FRAGMENT_BYTES,
                },
                ShaderModule::MonochromeSprite => match target {
                    ShaderTarget::Vertex => MONOCHROME_SPRITE_VERTEX_BYTES,
                    ShaderTarget::Fragment => MONOCHROME_SPRITE_FRAGMENT_BYTES,
                },
                ShaderModule::SubpixelSprite => match target {
                    ShaderTarget::Vertex => SUBPIXEL_SPRITE_VERTEX_BYTES,
                    ShaderTarget::Fragment => SUBPIXEL_SPRITE_FRAGMENT_BYTES,
                },
                ShaderModule::PolychromeSprite => match target {
                    ShaderTarget::Vertex => POLYCHROME_SPRITE_VERTEX_BYTES,
                    ShaderTarget::Fragment => POLYCHROME_SPRITE_FRAGMENT_BYTES,
                },
                ShaderModule::EmojiRasterization => match target {
                    ShaderTarget::Vertex => EMOJI_RASTERIZATION_VERTEX_BYTES,
                    ShaderTarget::Fragment => EMOJI_RASTERIZATION_FRAGMENT_BYTES,
                },
            };
            Self { inner: bytes }
        }
    }

    #[cfg(debug_assertions)]
    pub(super) fn build_shader_blob(entry: ShaderModule, target: ShaderTarget) -> Result<ID3DBlob> {
        unsafe {
            use windows::Win32::Graphics::{
                Direct3D::ID3DInclude, Hlsl::D3D_COMPILE_STANDARD_FILE_INCLUDE,
            };

            let shader_name = if matches!(entry, ShaderModule::EmojiRasterization) {
                "color_text_raster.hlsl"
            } else {
                "shaders.hlsl"
            };

            let entry = format!(
                "{}_{}\0",
                entry.as_str(),
                match target {
                    ShaderTarget::Vertex => "vertex",
                    ShaderTarget::Fragment => "fragment",
                }
            );
            let target = match target {
                ShaderTarget::Vertex => "vs_4_1\0",
                ShaderTarget::Fragment => "ps_4_1\0",
            };

            let mut compile_blob = None;
            let mut error_blob = None;
            let shader_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(&format!("src/{}", shader_name))
                .canonicalize()?;

            let entry_point = PCSTR::from_raw(entry.as_ptr());
            let target_cstr = PCSTR::from_raw(target.as_ptr());

            // D3D_COMPILE_STANDARD_FILE_INCLUDE 是 windows-rs 中的 usize 常量（哨兵值 (usize)-1），
            // 需要 transmute 为 ID3DInclude 后取引用，作为 D3DCompileFromFile 的 include handler。
            let include_handler = &std::mem::transmute::<usize, ID3DInclude>(
                D3D_COMPILE_STANDARD_FILE_INCLUDE as usize,
            );

            let ret = D3DCompileFromFile(
                &HSTRING::from(shader_path.to_str().unwrap()),
                None,
                include_handler,
                entry_point,
                target_cstr,
                D3DCOMPILE_DEBUG | D3DCOMPILE_SKIP_OPTIMIZATION,
                0,
                &mut compile_blob,
                Some(&mut error_blob),
            );
            if ret.is_err() {
                let Some(error_blob) = error_blob else {
                    return Err(anyhow::anyhow!("{ret:?}"));
                };

                let error_string =
                    std::ffi::CStr::from_ptr(error_blob.GetBufferPointer() as *const i8)
                        .to_string_lossy();
                log::error!("Shader compile error: {}", error_string);
                return Err(anyhow::anyhow!("Compile error: {}", error_string));
            }
            Ok(compile_blob.unwrap())
        }
    }

    #[cfg(not(debug_assertions))]
    include!(concat!(env!("OUT_DIR"), "/shaders_bytes.rs"));

    #[cfg(debug_assertions)]
    impl ShaderModule {
        pub fn as_str(self) -> &'static str {
            match self {
                ShaderModule::Quad => "quad",
                ShaderModule::OpaqueQuad => "opaque_quad",
                ShaderModule::Shadow => "shadow",
                ShaderModule::Underline => "underline",
                ShaderModule::PathTile => "path_tile",
                ShaderModule::MonochromeSprite => "monochrome_sprite",
                ShaderModule::SubpixelSprite => "subpixel_sprite",
                ShaderModule::PolychromeSprite => "polychrome_sprite",
                ShaderModule::EmojiRasterization => "emoji_rasterization",
            }
        }
    }
}

mod nvidia {
    //! DirectX 渲染器实现
    //!
    //! 本模块使用 DirectX 11 提供 GPU 加速的 2D 渲染功能，包括：
    //! - 阴影、矩形、路径、下划线渲染
    //! - 单色、亚像素、彩色精灵渲染
    //! - DirectComposition 窗口合成支持
    //! - GPU 设备丢失恢复

    use std::{
        ffi::CStr,
        os::raw::{c_char, c_int, c_uint},
    };

    use anyhow::Result;
    use windows::{Win32::System::LibraryLoader::GetProcAddress, core::s};

    use crate::with_dll_library;

    // https://github.com/NVIDIA/nvapi/blob/7cb76fce2f52de818b3da497af646af1ec16ce27/nvapi_lite_common.h#L180
    const NVAPI_SHORT_STRING_MAX: usize = 64;

    // https://github.com/NVIDIA/nvapi/blob/7cb76fce2f52de818b3da497af646af1ec16ce27/nvapi_lite_common.h#L235
    #[allow(non_camel_case_types)]
    type NvAPI_ShortString = [c_char; NVAPI_SHORT_STRING_MAX];

    // https://github.com/NVIDIA/nvapi/blob/7cb76fce2f52de818b3da497af646af1ec16ce27/nvapi_lite_common.h#L447
    #[allow(non_camel_case_types)]
    type NvAPI_SYS_GetDriverAndBranchVersion_t = unsafe extern "C" fn(
        driver_version: *mut c_uint,
        build_branch_string: *mut NvAPI_ShortString,
    ) -> c_int;

    pub(super) fn get_driver_version() -> Result<String> {
        #[cfg(target_pointer_width = "64")]
        let nvidia_dll_name = s!("nvapi64.dll");
        #[cfg(target_pointer_width = "32")]
        let nvidia_dll_name = s!("nvapi.dll");

        with_dll_library(nvidia_dll_name, |nvidia_dll| unsafe {
            let nvapi_query_addr = GetProcAddress(nvidia_dll, s!("nvapi_QueryInterface"))
                .ok_or_else(|| anyhow::anyhow!("Failed to get nvapi_QueryInterface address"))?;
            let nvapi_query: extern "C" fn(u32) -> *mut () = std::mem::transmute(nvapi_query_addr);

            // https://github.com/NVIDIA/nvapi/blob/7cb76fce2f52de818b3da497af646af1ec16ce27/nvapi_interface.h#L41
            let nvapi_get_driver_version_ptr = nvapi_query(0x2926aaad);
            if nvapi_get_driver_version_ptr.is_null() {
                anyhow::bail!("Failed to get NVIDIA driver version function pointer");
            }
            let nvapi_get_driver_version: NvAPI_SYS_GetDriverAndBranchVersion_t =
                std::mem::transmute(nvapi_get_driver_version_ptr);

            let mut driver_version: c_uint = 0;
            let mut build_branch_string: NvAPI_ShortString = [0; NVAPI_SHORT_STRING_MAX];
            let result = nvapi_get_driver_version(
                &mut driver_version as *mut c_uint,
                &mut build_branch_string as *mut NvAPI_ShortString,
            );

            if result != 0 {
                anyhow::bail!(
                    "Failed to get NVIDIA driver version, error code: {}",
                    result
                );
            }
            let major = driver_version / 100;
            let minor = driver_version % 100;
            let branch_string = CStr::from_ptr(build_branch_string.as_ptr());
            Ok(format!(
                "{}.{} {}",
                major,
                minor,
                branch_string.to_string_lossy()
            ))
        })
    }
}

mod amd {
    use std::os::raw::{c_char, c_int, c_void};

    use anyhow::Result;
    use windows::{Win32::System::LibraryLoader::GetProcAddress, core::s};

    use crate::with_dll_library;

    // https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/5d8812d703d0335741b6f7ffc37838eeb8b967f7/ags_lib/inc/amd_ags.h#L145
    const AGS_CURRENT_VERSION: i32 = (6 << 22) | (3 << 12);

    // https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/5d8812d703d0335741b6f7ffc37838eeb8b967f7/ags_lib/inc/amd_ags.h#L204
    // This is an opaque type, using struct to represent it properly for FFI
    #[repr(C)]
    struct AGSContext {
        _private: [u8; 0],
    }

    #[repr(C)]
    pub struct AGSGPUInfo {
        pub driver_version: *const c_char,
        pub radeon_software_version: *const c_char,
        pub num_devices: c_int,
        pub devices: *mut c_void,
    }

    // https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/5d8812d703d0335741b6f7ffc37838eeb8b967f7/ags_lib/inc/amd_ags.h#L429
    #[allow(non_camel_case_types)]
    type agsInitialize_t = unsafe extern "C" fn(
        version: c_int,
        config: *const c_void,
        context: *mut *mut AGSContext,
        gpu_info: *mut AGSGPUInfo,
    ) -> c_int;

    // https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/5d8812d703d0335741b6f7ffc37838eeb8b967f7/ags_lib/inc/amd_ags.h#L436
    #[allow(non_camel_case_types)]
    type agsDeInitialize_t = unsafe extern "C" fn(context: *mut AGSContext) -> c_int;

    pub(super) fn get_driver_version() -> Result<String> {
        #[cfg(target_pointer_width = "64")]
        let amd_dll_name = s!("amd_ags_x64.dll");
        #[cfg(target_pointer_width = "32")]
        let amd_dll_name = s!("amd_ags_x86.dll");

        with_dll_library(amd_dll_name, |amd_dll| unsafe {
            let ags_initialize_addr = GetProcAddress(amd_dll, s!("agsInitialize"))
                .ok_or_else(|| anyhow::anyhow!("Failed to get agsInitialize address"))?;
            let ags_deinitialize_addr = GetProcAddress(amd_dll, s!("agsDeInitialize"))
                .ok_or_else(|| anyhow::anyhow!("Failed to get agsDeInitialize address"))?;

            let ags_initialize: agsInitialize_t = std::mem::transmute(ags_initialize_addr);
            let ags_deinitialize: agsDeInitialize_t = std::mem::transmute(ags_deinitialize_addr);

            let mut context: *mut AGSContext = std::ptr::null_mut();
            let mut gpu_info: AGSGPUInfo = AGSGPUInfo {
                driver_version: std::ptr::null(),
                radeon_software_version: std::ptr::null(),
                num_devices: 0,
                devices: std::ptr::null_mut(),
            };

            let result = ags_initialize(
                AGS_CURRENT_VERSION,
                std::ptr::null(),
                &mut context,
                &mut gpu_info,
            );
            if result != 0 {
                anyhow::bail!("Failed to initialize AMD AGS, error code: {}", result);
            }

            // Vulkan actually returns this as the driver version
            let software_version = if !gpu_info.radeon_software_version.is_null() {
                std::ffi::CStr::from_ptr(gpu_info.radeon_software_version)
                    .to_string_lossy()
                    .into_owned()
            } else {
                "Unknown Radeon Software Version".to_string()
            };

            let driver_version = if !gpu_info.driver_version.is_null() {
                std::ffi::CStr::from_ptr(gpu_info.driver_version)
                    .to_string_lossy()
                    .into_owned()
            } else {
                "Unknown Radeon Driver Version".to_string()
            };

            ags_deinitialize(context);
            Ok(format!("{} ({})", software_version, driver_version))
        })
    }
}

mod dxgi {
    use windows::{
        Win32::Graphics::Dxgi::{IDXGIAdapter1, IDXGIDevice},
        core::Interface,
    };

    pub(super) fn get_driver_version(adapter: &IDXGIAdapter1) -> anyhow::Result<String> {
        let number = unsafe { adapter.CheckInterfaceSupport(&IDXGIDevice::IID as _) }?;
        Ok(format!(
            "{}.{}.{}.{}",
            number >> 48,
            (number >> 32) & 0xFFFF,
            (number >> 16) & 0xFFFF,
            number & 0xFFFF
        ))
    }
}

#[cfg(test)]
mod readback_tests {
    use super::{create_offscreen_render_target, read_render_target};
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_WARP;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
        ID3D11DeviceContext,
    };

    /// 用 WARP 软件设备跑测试：CI 的 Windows runner 没有独立显卡也能工作，
    /// 连 WARP 都建不起来（极少数环境）就跳过，不把环境问题变成回归噪声。
    fn create_warp_device() -> Option<(ID3D11Device, ID3D11DeviceContext)> {
        let mut device: Option<ID3D11Device> = None;
        let mut device_context: Option<ID3D11DeviceContext> = None;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_WARP,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut device_context),
            )
        }
        .ok()?;
        Some((device?, device_context?))
    }

    /// 离屏目标读回：通道顺序（BGRA -> RGBA）与尺寸都要对。
    ///
    /// 这条覆盖 `render_to_image` 的后半段（建离屏目标 -> 复制到 staging -> Map 读回），
    /// 用纯红做断言可以顺便验出「忘了交换 R / B」的错误 —— 绿色是对称的，测不出来。
    #[test]
    fn offscreen_target_reads_back_bgra_as_rgba() {
        let Some((device, device_context)) = create_warp_device() else {
            return;
        };

        let target = create_offscreen_render_target(&device, 4, 3).expect("离屏渲染目标应能创建");
        let mut view = None;
        unsafe { device.CreateRenderTargetView(&target, None, Some(&mut view)) }
            .expect("渲染目标视图应能创建");
        let view = view.expect("渲染目标视图不能为空");
        unsafe { device_context.ClearRenderTargetView(&view, &[1.0, 0.0, 0.0, 1.0]) };

        let image = read_render_target(&device, &device_context, &target, 4, 3)
            .expect("离屏目标应能读回像素");

        assert_eq!(image.dimensions(), (4, 3));
        for pixel in image.pixels() {
            assert_eq!(pixel.0, [255, 0, 0, 255], "读回的像素应是红色且通道不串");
        }
    }

    /// anti-overdraw 端到端（WARP）：
    /// 1) 不透明通道从前往后画、写深度——近处绿块遮住远处红块；
    /// 2) 混合通道把「实心不透明矩形」整个裁掉——清空重画后像素保持清屏色。
    #[test]
    fn opaque_channel_occludes_and_blended_channel_culls_opaque_rects() {
        use super::{
            DirectXGlobalElements, DirectXRenderPipelines, GlobalParams,
            create_depth_stencil_and_view, set_pipeline_state, set_rasterizer_state, set_viewport,
            update_buffer,
        };
        use rgpui::{
            Bounds, ContentMask, Hsla, Quad, ScaledPixels, hsla, point, size, solid_background,
        };
        use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP;
        use windows::Win32::Graphics::Direct3D11::D3D11_CLEAR_DEPTH;

        let Some((device, device_context)) = create_warp_device() else {
            return;
        };

        let mut pipelines = DirectXRenderPipelines::new(&device).expect("管线应能创建");
        let globals = DirectXGlobalElements::new(&device).expect("全局参数应能创建");
        update_buffer(
            &device_context,
            globals.global_params_buffer.as_ref().unwrap(),
            &[GlobalParams {
                gamma_ratios: [1.0, 0.0, 0.0, 0.0],
                viewport_size: [8.0, 8.0],
                grayscale_enhanced_contrast: 0.0,
                subpixel_enhanced_contrast: 0.0,
                is_bgr: 0,
                quad_count: 2,
                _pad: [0; 2],
            }],
        )
        .expect("全局参数应能上传");

        let red = hsla(0.0, 1.0, 0.5, 1.0);
        let green = hsla(0.33, 1.0, 0.5, 1.0);
        let solid = |color: Hsla, depth: f32| -> Quad {
            let bounds = Bounds {
                origin: point(ScaledPixels(0.0), ScaledPixels(0.0)),
                size: size(ScaledPixels(8.0), ScaledPixels(8.0)),
            };
            Quad {
                bounds,
                content_mask: ContentMask { bounds },
                background: solid_background(color),
                border_color: color,
                depth,
                ..Quad::default()
            }
        };
        // 排序后的场景序：红远（先画）、绿近（后画），深度按「越大越近」。
        let quads = [solid(red, 0.4), solid(green, 0.8)];
        pipelines
            .quad_pipeline
            .update_buffer(&device, &device_context, &quads)
            .expect("quad 缓冲应能更新");

        let target = create_offscreen_render_target(&device, 8, 8).expect("离屏目标应能创建");
        let mut rtv_option = None;
        unsafe { device.CreateRenderTargetView(&target, None, Some(&mut rtv_option)) }
            .expect("渲染目标视图应能创建");
        let rtv = rtv_option.expect("渲染目标视图不能为空");
        let rtv_slot: Option<windows::Win32::Graphics::Direct3D11::ID3D11RenderTargetView> =
            Some(rtv);
        let dsv = create_depth_stencil_and_view(&device, 8, 8)
            .expect("深度视图应能创建")
            .expect("深度视图不能为空");
        let viewport = set_viewport(&device_context, 8.0, 8.0);
        set_rasterizer_state(&device, &device_context).expect("光栅化状态应能设置");

        let clear = |device_context: &ID3D11DeviceContext, color: [f32; 4]| unsafe {
            device_context.ClearRenderTargetView(rtv_slot.as_ref().unwrap(), &color);
            device_context.ClearDepthStencilView(&dsv, D3D11_CLEAR_DEPTH.0, 0.0, 0);
            device_context.OMSetRenderTargets(Some(std::slice::from_ref(&rtv_slot)), Some(&dsv));
        };

        // 第一段：不透明通道（反向索引：instance 0 → quad 1 绿，instance 1 → quad 0 红）。
        clear(&device_context, [1.0, 1.0, 1.0, 1.0]);
        set_pipeline_state(
            &device_context,
            std::slice::from_ref(&pipelines.quad_pipeline.view),
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            std::slice::from_ref(&viewport),
            &pipelines.opaque_quads.vertex,
            &pipelines.opaque_quads.fragment,
            std::slice::from_ref(&globals.global_params_buffer),
            &pipelines.opaque_quads.blend_state,
            &pipelines.opaque_quads.depth_state,
        );
        unsafe { device_context.DrawInstanced(4, quads.len() as u32, 0, 0) };

        let image = read_render_target(&device, &device_context, &target, 8, 8)
            .expect("不透明通道后应能读回");
        for pixel in image.pixels() {
            assert!(
                (pixel.0[1] as i16 - 255).abs() <= 8 && pixel.0[0] < 40,
                "近处不透明绿应完全遮住远处红: {:?}",
                pixel.0
            );
        }

        // 第二段：只走混合通道——两个都是不透明矩形，应在顶点着色器被裁掉，
        // 深度缓冲里还留着绿写入的深度，但没有任何东西被重画。
        clear(&device_context, [0.0, 0.0, 1.0, 1.0]);
        set_pipeline_state(
            &device_context,
            std::slice::from_ref(&pipelines.quad_pipeline.view),
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            std::slice::from_ref(&viewport),
            &pipelines.quad_pipeline.vertex,
            &pipelines.quad_pipeline.fragment,
            std::slice::from_ref(&globals.global_params_buffer),
            &pipelines.quad_pipeline.blend_state,
            &pipelines.quad_pipeline.depth_state,
        );
        unsafe { device_context.DrawInstanced(4, quads.len() as u32, 0, 0) };

        let image = read_render_target(&device, &device_context, &target, 8, 8)
            .expect("混合通道后应能读回");
        for pixel in image.pixels() {
            assert_eq!(
                pixel.0,
                [0, 0, 255, 255],
                "不透明矩形应被混合通道整个剔除: {:?}",
                pixel.0
            );
        }
    }

    /// 环绕数路径渲染端到端（WARP，D3）：
    /// `PathBuilder::fill` 产出三角形曲线 → CPU 分桶成瓦片 → path_tile 管线
    /// 逐像素算环绕数，内部实心、外部透明、边缘带部分覆盖。
    #[test]
    fn path_tile_pipeline_fills_triangle_with_winding() {
        use super::{
            DirectXGlobalElements, DirectXRenderPipelines, GlobalParams, create_buffer_view_range,
            set_pipeline_state, set_rasterizer_state, set_viewport, update_buffer,
        };
        use rgpui::{
            Bounds, ContentMask, PaintedPath, PathBuilder, hsla, point, px, size, solid_background,
        };
        use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP;

        let Some((device, device_context)) = create_warp_device() else {
            return;
        };

        let mut pipelines = DirectXRenderPipelines::new(&device).expect("管线应能创建");
        let globals = DirectXGlobalElements::new(&device).expect("全局参数应能创建");
        update_buffer(
            &device_context,
            globals.global_params_buffer.as_ref().unwrap(),
            &[GlobalParams {
                gamma_ratios: [1.0, 0.0, 0.0, 0.0],
                viewport_size: [16.0, 16.0],
                grayscale_enhanced_contrast: 0.0,
                subpixel_enhanced_contrast: 0.0,
                is_bgr: 0,
                quad_count: 0,
                _pad: [0; 2],
            }],
        )
        .expect("全局参数应能上传");

        // 三角形 (2,2)-(14,2)-(8,14)，实心红，scale 1.0。
        let mut builder = PathBuilder::fill();
        builder.add_polygon(
            &[
                point(px(2.), px(2.)),
                point(px(14.), px(2.)),
                point(px(8.), px(14.)),
            ],
            true,
        );
        let path = builder.build().expect("三角形路径应能构建");
        let full = Bounds {
            origin: point(px(0.), px(0.)),
            size: size(px(16.), px(16.)),
        };
        let painted = PaintedPath::new(
            path,
            1.0,
            ContentMask { bounds: full },
            solid_background(hsla(0.0, 1.0, 0.5, 1.0)),
        );
        let mut paints: Vec<rgpui::PathPaint> = Vec::new();
        let mut curves: Vec<rgpui::PathCurve> = Vec::new();
        let mut tiles: Vec<rgpui::PathTile> = Vec::new();
        painted.append_to(&mut paints, &mut curves, &mut tiles);
        assert!(!tiles.is_empty(), "分桶应产出瓦片");

        pipelines
            .path_paints
            .update_buffer(&device, &device_context, paints.as_slice())
            .expect("绘制参数缓冲应能更新");
        pipelines
            .path_curves
            .update_buffer(&device, &device_context, curves.as_slice())
            .expect("曲线缓冲应能更新");
        pipelines
            .path_tile_pipeline
            .update_buffer(&device, &device_context, tiles.as_slice())
            .expect("瓦片缓冲应能更新");

        let target = create_offscreen_render_target(&device, 16, 16).expect("离屏目标应能创建");
        let mut rtv_option = None;
        unsafe { device.CreateRenderTargetView(&target, None, Some(&mut rtv_option)) }
            .expect("渲染目标视图应能创建");
        let rtv = rtv_option.as_ref().expect("渲染目标视图不能为空").clone();
        unsafe { device_context.ClearRenderTargetView(&rtv, &[0.0, 0.0, 0.0, 0.0]) };
        let viewport = set_viewport(&device_context, 16.0, 16.0);
        set_rasterizer_state(&device, &device_context).expect("光栅化状态应能设置");
        unsafe {
            device_context.OMSetRenderTargets(Some(std::slice::from_ref(&rtv_option)), None);
        }

        // 与 draw_path_tiles 相同的绑定口径：t1 = 本批瓦片区间视图，
        // t2 = 全量曲线，t3 = 全量绘制参数。
        let tile_view = create_buffer_view_range(
            &device,
            &pipelines.path_tile_pipeline.buffer,
            0,
            tiles.len() as u32,
        )
        .expect("瓦片区间视图应能创建");
        assert!(tile_view.is_some(), "瓦片区间视图不能为空");
        let paints_view = pipelines
            .path_paints
            .view
            .as_ref()
            .expect("绘制参数视图应存在")
            .clone();
        let curves_view = pipelines
            .path_curves
            .view
            .as_ref()
            .expect("曲线视图应存在")
            .clone();

        set_pipeline_state(
            &device_context,
            std::slice::from_ref(&tile_view),
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            std::slice::from_ref(&viewport),
            &pipelines.path_tile_pipeline.vertex,
            &pipelines.path_tile_pipeline.fragment,
            std::slice::from_ref(&globals.global_params_buffer),
            &pipelines.path_tile_pipeline.blend_state,
            &pipelines.path_tile_pipeline.depth_state,
        );
        unsafe {
            // t2 = 曲线，t3 = 绘制参数（与 HLSL 寄存器一致）。
            device_context.VSSetShaderResources(2, Some(&[Some(curves_view.clone())]));
            device_context.PSSetShaderResources(2, Some(&[Some(curves_view)]));
            device_context.VSSetShaderResources(3, Some(&[Some(paints_view.clone())]));
            device_context.PSSetShaderResources(3, Some(&[Some(paints_view)]));
            device_context.DrawInstanced(4, tiles.len() as u32, 0, 0);
        }

        let image = read_render_target(&device, &device_context, &target, 16, 16)
            .expect("路径绘制后应能读回");
        let px = |x: u32, y: u32| image.get_pixel(x, y).0;
        // 内部点：红色不透明。
        let inside = px(8, 8);
        assert!(
            inside[0] >= 215 && inside[1] <= 40 && inside[2] <= 40 && inside[3] >= 215,
            "三角形内部应为红色不透明（读回 BGRA->RGBA），实际 {inside:?}"
        );
        // 外部点：透明。
        for (x, y) in [(0u32, 0u32), (15, 0), (0, 15), (1, 1)] {
            let outside = px(x, y);
            assert_eq!(
                outside[3], 0,
                "三角形外部 ({x},{y}) 应透明，实际 {outside:?}"
            );
        }
    }
}
