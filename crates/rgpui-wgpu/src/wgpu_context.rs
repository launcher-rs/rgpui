#[cfg(not(target_family = "wasm"))]
use anyhow::Context as _;
#[cfg(not(target_family = "wasm"))]
use rgpui::ResultExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use wgpu::TextureFormat;

/// wgpu GPU 上下文，包含设备、队列和适配器等信息
pub struct WgpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    dual_source_blending: bool,
    color_texture_format: wgpu::TextureFormat,
    device_lost: Arc<AtomicBool>,
}

/// 合成器 GPU 提示，用于适配器选择
#[derive(Clone, Copy)]
pub struct CompositorGpuHint {
    pub vendor_id: u32,
    pub device_id: u32,
}

impl WgpuContext {
    #[cfg(not(target_family = "wasm"))]
    pub fn new(
        instance: wgpu::Instance,
        surface: &wgpu::Surface<'_>,
        compositor_gpu: Option<CompositorGpuHint>,
    ) -> anyhow::Result<Self> {
        Self::new_with_options(instance, Some(surface), compositor_gpu, false)
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn new_rejecting_software(
        instance: wgpu::Instance,
        surface: &wgpu::Surface<'_>,
        compositor_gpu: Option<CompositorGpuHint>,
    ) -> anyhow::Result<Self> {
        Self::new_with_options(instance, Some(surface), compositor_gpu, true)
    }

    /// 创建**没有 surface** 的离屏上下文，用于无头渲染与视觉测试。
    ///
    /// 窗口路径靠「实际配置一次 surface」来判定适配器兼容性，离屏没有显示器可问，
    /// 只能退到「能建出设备即可」；因此软件渲染（lavapipe/llvmpipe）在这里是保留的，
    /// 无头环境往往只有它。
    #[cfg(not(target_family = "wasm"))]
    pub fn new_headless() -> anyhow::Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            // 与窗口路径同理：不开这个标志，Mesa 软件驱动会被直接过滤掉
            flags: wgpu::InstanceFlags::default()
                | wgpu::InstanceFlags::ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER,
            backend_options: wgpu::BackendOptions::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            display: None,
        });
        Self::new_with_options(instance, None, None, false)
    }

    #[cfg(not(target_family = "wasm"))]
    fn new_with_options(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
        compositor_gpu: Option<CompositorGpuHint>,
        reject_software: bool,
    ) -> anyhow::Result<Self> {
        let device_id_filter = match std::env::var("ZED_DEVICE_ID") {
            Ok(val) => parse_pci_id(&val)
                .context("Failed to parse device ID from `ZED_DEVICE_ID` environment variable")
                .log_err(),
            Err(std::env::VarError::NotPresent) => None,
            err => {
                err.context("读取 `ZED_DEVICE_ID` 环境变量失败").log_err();
                None
            }
        };

        // 通过实际测试表面配置来选择适配器。
        // 这是在混合 GPU 系统上确定兼容性的唯一可靠方法。
        let (adapter, device, queue, dual_source_blending, color_texture_format) =
            rgpui::block_on(Self::select_adapter_and_device(
                &instance,
                device_id_filter,
                surface,
                compositor_gpu.as_ref(),
                reject_software,
            ))?;

        let device_lost = Arc::new(AtomicBool::new(false));
        device.set_device_lost_callback({
            let device_lost = Arc::clone(&device_lost);
            move |reason, message| {
                log::error!("wgpu device lost: reason={reason:?}, message={message}");
                if reason != wgpu::DeviceLostReason::Destroyed {
                    device_lost.store(true, Ordering::Relaxed);
                }
            }
        });

        log::info!(
            "Selected GPU adapter: {:?} ({:?})",
            adapter.get_info().name,
            adapter.get_info().backend
        );
        // 同步注册到核心 GPU 信息表（Inspector“运行”卡片读取；重复调用保持首次值）。
        {
            let info = adapter.get_info();
            rgpui::set_gpu_info(info.name, format!("{:?}", info.backend));
        }

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // 注册到共享上下文，供 rgpui-3d 等第三方渲染器复用
        // 离屏上下文不注册：它随时可能随测试窗口一起销毁，不该被后续真实窗口捡走
        #[cfg(not(target_family = "wasm"))]
        if surface.is_some() {
            crate::shared_context::register(instance.clone(), device.clone(), queue.clone());
        }

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            dual_source_blending,
            color_texture_format,
            device_lost,
        })
    }

    #[cfg(target_family = "wasm")]
    /// 为 Web/WASM 平台创建 wgpu 上下文
    pub async fn new_web() -> anyhow::Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            flags: wgpu::InstanceFlags::default(),
            backend_options: wgpu::BackendOptions::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            display: None,
        });

        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
        {
            Ok(adapter) => adapter,
            Err(_) => {
                log::warn!("未找到高性能 GPU 适配器，尝试使用回退适配器（软件渲染）");
                instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::LowPower,
                        compatible_surface: None,
                        force_fallback_adapter: true,
                        apply_limit_buckets: false,
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to request GPU adapter: {e}"))?
            }
        };

        log::info!(
            "Selected GPU adapter: {:?} ({:?})",
            adapter.get_info().name,
            adapter.get_info().backend
        );
        // 同上：注册到核心 GPU 信息表（WASM 平台同样上报）。
        {
            let info = adapter.get_info();
            rgpui::set_gpu_info(info.name, format!("{:?}", info.backend));
        }

        let device_lost = Arc::new(AtomicBool::new(false));
        let (device, queue, dual_source_blending, color_texture_format) =
            Self::create_device(&adapter).await?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // WASM 平台不注册共享上下文（wgpu WebGPU 后端不满足 Send+Sync）

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            dual_source_blending,
            color_texture_format,
            device_lost,
        })
    }

    /// 创建 wgpu 设备和队列
    async fn create_device(
        adapter: &wgpu::Adapter,
    ) -> anyhow::Result<(wgpu::Device, wgpu::Queue, bool, TextureFormat)> {
        let dual_source_blending = adapter
            .features()
            .contains(wgpu::Features::DUAL_SOURCE_BLENDING);

        let mut required_features = wgpu::Features::empty();
        if dual_source_blending {
            required_features |= wgpu::Features::DUAL_SOURCE_BLENDING;
        } else {
            log::warn!(
                "Dual-source blending not available on this GPU. \
                Subpixel text antialiasing will be disabled."
            );
        }

        let color_atlas_texture_format = Self::select_color_texture_format(adapter)?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("gpui_device"),
                required_features,
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits())
                    .using_alignment(adapter.limits()),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create wgpu device: {e}"))?;

        Ok((
            device,
            queue,
            dual_source_blending,
            color_atlas_texture_format,
        ))
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn instance(display: Box<dyn wgpu::wgt::WgpuHasDisplayHandle>) -> wgpu::Instance {
        wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            // 允许暴露未通过 Vulkan 一致性测试的驱动（如 Mesa lavapipe/llvmpipe 软件渲染），
            // 否则在无独立显卡的虚拟机上会枚举不到任何适配器
            flags: wgpu::InstanceFlags::default()
                | wgpu::InstanceFlags::ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER,
            backend_options: wgpu::BackendOptions::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            display: Some(display),
        })
    }

    /// 检查适配器是否与表面兼容
    pub fn check_compatible_with_surface(&self, surface: &wgpu::Surface<'_>) -> anyhow::Result<()> {
        let caps = surface.get_capabilities(&self.adapter);
        if caps.formats.is_empty() {
            let info = self.adapter.get_info();
            anyhow::bail!(
                "Adapter {:?} (backend={:?}, device={:#06x}) is not compatible with the \
                 display surface for this window.",
                info.name,
                info.backend,
                info.device,
            );
        }
        Ok(())
    }

    /// 选择适配器并创建设备。
    ///
    /// 有 surface 时「实际配置一次」是判定兼容性的唯一可靠办法——适配器可能通过
    /// `get_capabilities()` 报告兼容，但真正 configure 时失败（例如 NVIDIA 报告支持
    /// Vulkan Wayland，而合成器跑在 Intel GPU 上）。没有 surface 时只能退到
    /// 「能建出设备即可」。
    #[cfg(not(target_family = "wasm"))]
    async fn select_adapter_and_device(
        instance: &wgpu::Instance,
        device_id_filter: Option<u32>,
        surface: Option<&wgpu::Surface<'_>>,
        compositor_gpu: Option<&CompositorGpuHint>,
        reject_software: bool,
    ) -> anyhow::Result<(
        wgpu::Adapter,
        wgpu::Device,
        wgpu::Queue,
        bool,
        TextureFormat,
    )> {
        let mut adapters: Vec<_> = instance.enumerate_adapters(wgpu::Backends::all()).await;

        if adapters.is_empty() {
            anyhow::bail!("No GPU adapters found");
        }

        if let Some(device_id) = device_id_filter {
            log::info!("ZED_DEVICE_ID filter: {:#06x}", device_id);
        }

        // 将适配器按单一优先级排序。层级（从高到低）：
        //
        // 1. ZED_DEVICE_ID 匹配 — 用户显式覆盖
        // 2. 合成器 GPU 匹配 — 显示服务器正在渲染的 GPU
        // 3. 设备类型（Discrete > Integrated > Other > Virtual > Cpu）。
        //    "Other" 排在 "Virtual" 之上，因为 OpenGL 似乎被归类为 "Other"。
        // 4. 后端 — 优先选择 Vulkan/Metal/Dx12 而非 GL 等。
        adapters.sort_by_key(|adapter| {
            let info = adapter.get_info();

            // OpenGL 等后端对所有适配器报告 device=0，
            // 因此基于设备的匹配仅在非零时有意义。
            let device_known = info.device != 0;

            let user_override: u8 = match device_id_filter {
                Some(id) if device_known && info.device == id => 0,
                _ => 1,
            };

            let compositor_match: u8 = match compositor_gpu {
                Some(hint)
                    if device_known
                        && info.vendor == hint.vendor_id
                        && info.device == hint.device_id =>
                {
                    0
                }
                _ => 1,
            };

            let type_priority: u8 = if info.device_type == wgpu::DeviceType::Cpu {
                4
            } else {
                match info.device_type {
                    wgpu::DeviceType::DiscreteGpu => 0,
                    wgpu::DeviceType::IntegratedGpu => 1,
                    wgpu::DeviceType::Other => 2,
                    wgpu::DeviceType::VirtualGpu => 3,
                    wgpu::DeviceType::Cpu => 4,
                }
            };

            let backend_priority: u8 = match info.backend {
                wgpu::Backend::Vulkan | wgpu::Backend::Metal | wgpu::Backend::Dx12 => 0,
                _ => 1,
            };

            (
                user_override,
                compositor_match,
                type_priority,
                backend_priority,
            )
        });

        // 记录所有可用的适配器（按排序顺序）
        log::info!("Found {} GPU adapter(s):", adapters.len());
        for adapter in &adapters {
            let info = adapter.get_info();
            log::info!(
                "  - {} (vendor={:#06x}, device={:#06x}, backend={:?}, type={:?})",
                info.name,
                info.vendor,
                info.device,
                info.backend,
                info.device_type,
            );
        }

        // 测试每个适配器，创建设备并配置表面
        for adapter in adapters {
            let info = adapter.get_info();

            if reject_software && info.device_type == wgpu::DeviceType::Cpu {
                log::info!(
                    "Skipping software renderer: {} ({:?})",
                    info.name,
                    info.backend
                );
                continue;
            }

            log::info!("Testing adapter: {} ({:?})...", info.name, info.backend);

            let attempt = match surface {
                Some(surface) => Self::try_adapter_with_surface(&adapter, surface).await,
                None => Self::try_adapter_offscreen(&adapter).await,
            };
            match attempt {
                Ok((device, queue, dual_source_blending, color_atlas_texture_format)) => {
                    log::info!(
                        "Selected GPU (passed configuration test): {} ({:?})",
                        info.name,
                        info.backend
                    );
                    return Ok((
                        adapter,
                        device,
                        queue,
                        dual_source_blending,
                        color_atlas_texture_format,
                    ));
                }
                Err(e) => {
                    log::info!(
                        "  Adapter {} ({:?}) failed: {}, trying next...",
                        info.name,
                        info.backend,
                        e
                    );
                }
            }
        }

        anyhow::bail!(
            "No GPU adapter found that can {}",
            if surface.is_some() {
                "configure the display surface"
            } else {
                "create an offscreen device"
            }
        )
    }

    /// 离屏（无表面）适配器的兼容性测试：先建设备，再真的渲染一帧并回读像素。
    ///
    /// 只验证「能否建设备」是不够的：本机实测 Mesa 的 lavapipe（Vulkan 软件实现）在
    /// `PollType::Wait{timeout}` 下提交围栏不会推进，而 GL 后端的 llvmpipe 正常。
    /// 这里用一次真实的回读把不支持同步回读的驱动筛掉，避免截图路径永久卡住。
    #[cfg(not(target_family = "wasm"))]
    async fn try_adapter_offscreen(
        adapter: &wgpu::Adapter,
    ) -> anyhow::Result<(wgpu::Device, wgpu::Queue, bool, TextureFormat)> {
        let (device, queue, dual_source_blending, color_texture_format) =
            Self::create_device(adapter).await?;
        Self::probe_offscreen_readback(&device, &queue)
            .map_err(|error| anyhow::anyhow!("离屏回读探测失败: {error:#}"))?;
        Ok((device, queue, dual_source_blending, color_texture_format))
    }

    /// 渲染一次 16×16 的清空并回读，验证这台驱动真的会退休 `MAP_READ` 提交。
    ///
    /// 等待只用 `PollType::Poll` 自旋：`Wait{timeout}` 在本机 lavapipe 上立即返回
    /// `Timeout` 且不推进围栏（详见 `docs/linux-platform-audit.md` §2.6）。
    #[cfg(not(target_family = "wasm"))]
    fn probe_offscreen_readback(device: &wgpu::Device, queue: &wgpu::Queue) -> anyhow::Result<()> {
        const SIZE: u32 = 16;
        let bytes_per_row: u32 = (SIZE * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let extent = wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("readback_probe_target"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("readback_probe_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
        }
        drop(view);
        queue.submit(std::iter::once(encoder.finish()));

        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback_probe_buffer"),
            size: u64::from(bytes_per_row) * SIZE as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfoBase {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(SIZE),
                },
            },
            extent,
        );
        queue.submit(std::iter::once(encoder.finish()));

        let (sender, receiver) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            match receiver.try_recv() {
                Ok(result) => {
                    result.map_err(|error| anyhow::anyhow!("映射探测缓冲失败: {error:?}"))?;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    anyhow::bail!("回读回调没有触发");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    if std::time::Instant::now() >= deadline {
                        anyhow::bail!("等待回读超时，这台驱动不支持同步回读");
                    }
                    device.poll(wgpu::PollType::Poll)?;
                }
            }
        }
        readback.unmap();
        Ok(())
    }

    /// 尝试使用适配器与表面，创建设备并测试配置。
    /// 成功时返回设备和队列，以便复用。
    #[cfg(not(target_family = "wasm"))]
    async fn try_adapter_with_surface(
        adapter: &wgpu::Adapter,
        surface: &wgpu::Surface<'_>,
    ) -> anyhow::Result<(wgpu::Device, wgpu::Queue, bool, TextureFormat)> {
        let caps = surface.get_capabilities(adapter);
        if caps.formats.is_empty() {
            anyhow::bail!("no compatible surface formats");
        }
        if caps.alpha_modes.is_empty() {
            anyhow::bail!("no compatible alpha modes");
        }

        let (device, queue, dual_source_blending, color_atlas_texture_format) =
            Self::create_device(adapter).await?;
        let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);

        let test_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: caps.formats[0],
            width: 64,
            height: 64,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };

        surface.configure(&device, &test_config);

        let error = error_scope.pop().await;
        if let Some(e) = error {
            anyhow::bail!("surface configuration failed: {e}");
        }

        Ok((
            device,
            queue,
            dual_source_blending,
            color_atlas_texture_format,
        ))
    }

    /// 选择适合的彩色纹理格式
    fn select_color_texture_format(adapter: &wgpu::Adapter) -> anyhow::Result<wgpu::TextureFormat> {
        let required_usages = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
        let bgra_features = adapter.get_texture_format_features(wgpu::TextureFormat::Bgra8Unorm);
        if bgra_features.allowed_usages.contains(required_usages) {
            return Ok(wgpu::TextureFormat::Bgra8Unorm);
        }

        let rgba_features = adapter.get_texture_format_features(wgpu::TextureFormat::Rgba8Unorm);
        if rgba_features.allowed_usages.contains(required_usages) {
            let info = adapter.get_info();
            log::warn!(
                "Adapter {} ({:?}) does not support Bgra8Unorm atlas textures with usages {:?}; \
                 falling back to Rgba8Unorm atlas textures.",
                info.name,
                info.backend,
                required_usages,
            );
            return Ok(wgpu::TextureFormat::Rgba8Unorm);
        }

        let info = adapter.get_info();
        Err(anyhow::anyhow!(
            "Adapter {} ({:?}, device={:#06x}) does not support a usable color atlas texture \
             format with usages {:?}. Bgra8Unorm allowed usages: {:?}; \
             Rgba8Unorm allowed usages: {:?}.",
            info.name,
            info.backend,
            info.device,
            required_usages,
            bgra_features.allowed_usages,
            rgba_features.allowed_usages,
        ))
    }
    /// 检查是否支持双源混合
    pub fn supports_dual_source_blending(&self) -> bool {
        self.dual_source_blending
    }

    /// 获取彩色纹理格式
    pub fn color_texture_format(&self) -> wgpu::TextureFormat {
        self.color_texture_format
    }

    /// 返回 GPU 设备是否丢失（例如由于驱动崩溃、挂起/恢复）。
    /// 当返回 true 时，需要重新创建上下文。
    pub fn device_lost(&self) -> bool {
        self.device_lost.load(Ordering::Relaxed)
    }

    /// 返回 device_lost 标志的克隆，用于与渲染器共享
    pub(crate) fn device_lost_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.device_lost)
    }
}

#[cfg(not(target_family = "wasm"))]
/// 解析 PCI 设备 ID 字符串为 u32
fn parse_pci_id(id: &str) -> anyhow::Result<u32> {
    let mut id = id.trim();

    if id.starts_with("0x") || id.starts_with("0X") {
        id = &id[2..];
    }
    let is_hex_string = id.chars().all(|c| c.is_ascii_hexdigit());
    let is_4_chars = id.len() == 4;
    anyhow::ensure!(
        is_4_chars && is_hex_string,
        "Expected a 4 digit PCI ID in hexadecimal format"
    );

    u32::from_str_radix(id, 16).context("parsing PCI ID as hex")
}

#[cfg(test)]
mod tests {
    use super::parse_pci_id;

    #[test]
    fn test_parse_device_id() {
        assert!(parse_pci_id("0xABCD").is_ok());
        assert!(parse_pci_id("ABCD").is_ok());
        assert!(parse_pci_id("abcd").is_ok());
        assert!(parse_pci_id("1234").is_ok());
        assert!(parse_pci_id("123").is_err());
        assert_eq!(
            parse_pci_id(&format!("{:x}", 0x1234)).unwrap(),
            parse_pci_id(&format!("{:X}", 0x1234)).unwrap(),
        );

        assert_eq!(
            parse_pci_id(&format!("{:#x}", 0x1234)).unwrap(),
            parse_pci_id(&format!("{:#X}", 0x1234)).unwrap(),
        );
    }
}
