use crate::math::*;
use crate::shader::{SHADER_SRC, SKIN_SHADER_SRC};
use crate::types::*;
use bytemuck;
use rgpui::RenderImage;
use scenekit::{
    Geometry, GpuError, GpuScene, MaterialId, MeshId, PackedVertex, PerspectiveCamera,
    RendererLight, RendererMaterial, SceneGraph, ScenixError, TextureId, collect_visible_draws,
    sort_opaque_front_to_back, sort_transparent_back_to_front,
};
use std::collections::{BTreeMap, HashMap};

// ============================================================================
// 公开类型
// ============================================================================

/// 3D 渲染结果，包含 BGRA 格式像素数据和尺寸
pub struct RenderResult {
    /// BGRA 格式像素数据
    pub data: Vec<u8>,
    /// 图像宽度（像素）
    pub width: u32,
    /// 图像高度（像素）
    pub height: u32,
}

impl RenderResult {
    /// 转换为 rgpui RenderImage，可直接在窗口中绘制
    ///
    /// 像素缓冲与宽高不匹配（GPU 回读异常）时不再 panic：记一条警告并退回一帧
    /// 1×1 全透明占位图，宁可这一帧画不出内容，也不让整个应用跟着崩掉。
    pub fn into_render_image(self) -> RenderImage {
        let expected = (self.width as usize)
            .saturating_mul(self.height as usize)
            .saturating_mul(4);
        let actual = self.data.len();
        let img_buffer = match image::RgbaImage::from_raw(self.width, self.height, self.data) {
            Some(buffer) => buffer,
            None => {
                log::warn!(
                    "3D 回读像素尺寸不符：期望 {expected} 字节，实得 {actual} 字节（{}×{}），本帧以透明占位",
                    self.width,
                    self.height
                );
                image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 0]))
            }
        };
        let frame = image::Frame::new(img_buffer);
        RenderImage::new(smallvec::smallvec![frame])
    }
}

/// 3D 场景渲染上下文
///
/// 管理 wgpu 设备、离屏渲染目标、scenekit GPU 场景资源和渲染管线。
///
/// # 生命周期
/// 1. 使用 `Scenix3D::new(width, height)` 或 `Scenix3D::new_shared(device, queue, w, h)` 创建
/// 2. 注册网格和材质：`register_mesh()`, `register_pbr_material()` 等
/// 3. 构建 scenekit `SceneGraph`，添加 `SceneNode`
/// 4. 调用 `render()` 渲染并获取像素结果
/// 5. 将结果转为 `RenderImage` 在 rgpui 窗口中显示
pub struct Scenix3D {
    // wgpu 资源
    device: wgpu::Device,
    queue: wgpu::Queue,

    // 渲染目标
    color_texture: wgpu::Texture,
    depth_texture: wgpu::Texture,

    // 绑定组布局和管线
    _frame_layout: wgpu::BindGroupLayout,
    object_layout: wgpu::BindGroupLayout,
    material_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    pipeline_layout: wgpu::PipelineLayout,

    // uniform 资源
    frame_buffer: wgpu::Buffer,
    frame_bind_group: wgpu::BindGroup,
    object_stride: u64,
    material_stride: u64,
    object_buffer: wgpu::Buffer,
    material_buffer: wgpu::Buffer,
    object_bind_group: wgpu::BindGroup,
    material_bind_group: wgpu::BindGroup,
    draw_capacity: usize,

    // 纹理资源
    white_bind_group: wgpu::BindGroup,
    textures: HashMap<TextureId, TextureEntry>,

    // GPU 场景资源
    gpu_scene: GpuScene,

    // 抗锯齿配置
    msaa_sample_count: u32,
    resolve_texture: Option<wgpu::Texture>,

    // 配置
    width: u32,
    height: u32,
    color_format: wgpu::TextureFormat,
    clear_color: [f32; 4],

    // 帧计数器
    frame_index: u64,

    // 蒙皮/骨骼动画
    skin_pipeline: wgpu::RenderPipeline,
    skin_pipeline_layout: wgpu::PipelineLayout,
    skin_layout: wgpu::BindGroupLayout,
    skinned_meshes: HashMap<MeshId, GpuSkinnedMesh>,
    skins: Vec<SkinData>,
    mesh_to_skin: HashMap<MeshId, usize>,
    mesh_to_node: HashMap<MeshId, usize>,
    joints: Vec<JointNode>,
    anim_clips: Vec<AnimClip>,
    active_anim: usize,
    anim_time: f32,
    anim_speed: f32,
    anim_paused: bool,
    bone_buffer: wgpu::Buffer,
    bone_capacity: u32,
    bone_bg: wgpu::BindGroup,

    // Morph target 资源
    morph_meshes: BTreeMap<MeshId, MorphMesh>,
    /// morph 绘制所属的 glTF 节点（动画权重通道按节点寻址）。
    node_to_morph_meshes: HashMap<usize, Vec<MeshId>>,
    morph_delta_buffer: wgpu::Buffer,
    morph_delta_capacity: u32,
    morph_weight_buffer: wgpu::Buffer,
    morph_weight_capacity: u32,

    cached_local_trs: Vec<([f32; 3], [f32; 4], [f32; 3])>,
    cached_global_mats: Vec<scenekit::Mat4>,
    cached_bone_data: Vec<f32>,

    // 多层动画与 cross-fade
    anim_layers: Vec<AnimLayer>,
    anim_fade: Option<AnimFade>,

    joint_overrides: Vec<JointOverride>,
}

impl Scenix3D {
    /// 异步创建新的 3D 渲染上下文。
    ///
    /// 当启用 `shared-wgpu` feature 且 rgpui 已创建共享 wgpu 上下文时，
    /// 自动复用现有设备/队列，避免创建多个 `wgpu::Instance`。
    /// 否则创建独立的 wgpu 实例。
    pub async fn new(width: u32, height: u32) -> Result<Self, ScenixError> {
        #[cfg(feature = "shared-wgpu")]
        if let Some(shared) = rgpui_wgpu::shared_context::try_get() {
            return Self::new_inner(
                wgpu::Device::clone(&shared.device),
                wgpu::Queue::clone(&shared.queue),
                width,
                height,
                1,
            );
        }

        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.flags = desc.flags.difference(wgpu::InstanceFlags::VALIDATION);
        let instance = wgpu::Instance::new(desc);
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .map_err(|_| ScenixError::Gpu(GpuError::Init))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("rgpui-3d.device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits {
                    max_bind_groups: 5,
                    max_texture_dimension_2d: 4096,
                    max_storage_buffers_per_shader_stage: 4,
                    ..Default::default()
                },
                ..Default::default()
            })
            .await
            .map_err(|_| ScenixError::Gpu(GpuError::Init))?;

        Self::new_inner(device, queue, width, height, 1)
    }

    /// 同步创建 3D 渲染上下文（推荐在非 async 上下文中使用）。
    ///
    /// 内部将 async future 放到堆上（`Box::pin`），避免 debug 模式下
    /// 大型 wgpu async future + D3D11 调试层栈帧导致栈溢出（STATUS_ACCESS_VIOLATION）。
    pub fn blocking_new(width: u32, height: u32) -> Result<Self, ScenixError> {
        smol::block_on(Box::pin(Self::new(width, height)))
    }

    /// 从已有的 wgpu 设备+队列创建（共享 rgpui 的 GPU 上下文，不创建新实例）
    ///
    /// 当 rgpui 已经创建了 wgpu 设备时使用此方法，可避免多个
    /// `wgpu::Instance` 导致的 GPU 资源重复和调度竞争问题。
    pub fn from_device(
        device: wgpu::Device,
        queue: wgpu::Queue,
        width: u32,
        height: u32,
    ) -> Result<Self, ScenixError> {
        Self::new_inner(device, queue, width, height, 1)
    }

    /// 内部初始化逻辑，从已有的 device/queue 创建完整的 3D 上下文
    fn new_inner(
        device: wgpu::Device,
        queue: wgpu::Queue,
        width: u32,
        height: u32,
        sample_count: u32,
    ) -> Result<Self, ScenixError> {
        let color_format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let msaa = sample_count > 1;

        let color_usage = if msaa {
            wgpu::TextureUsages::RENDER_ATTACHMENT
        } else {
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC
        };
        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rgpui-3d.color"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: color_format,
            usage: color_usage,
            view_formats: &[],
        });

        let resolve_texture = if msaa {
            Some(device.create_texture(&wgpu::TextureDescriptor {
                label: Some("rgpui-3d.resolve"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: color_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            }))
        } else {
            None
        };

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rgpui-3d.depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rgpui-3d.frame_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let object_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rgpui-3d.object_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rgpui-3d.material_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rgpui-3d.texture_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("rgpui-3d.texture_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });

        let white_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rgpui-3d.white_texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &white_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let white_view = white_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let white_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rgpui-3d.white_bind_group"),
            layout: &texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&white_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture_sampler),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rgpui-3d.shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        let msaa_state = wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        };

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rgpui-3d.pipeline_layout"),
            bind_group_layouts: &[
                Some(&frame_layout),
                Some(&object_layout),
                Some(&material_layout),
                Some(&texture_layout),
            ],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rgpui-3d.pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(PackedVertex::layout())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: msaa_state,
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview_mask: None,
            cache: None,
        });

        let object_stride = aligned_size::<ObjectUniform>();
        let material_stride = aligned_size::<MaterialUniform>();
        let draw_capacity = 256usize;

        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgpui-3d.frame_buffer"),
            size: std::mem::size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let frame_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rgpui-3d.frame_bind_group"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &frame_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<FrameUniform>() as u64),
                }),
            }],
        });

        let object_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgpui-3d.object_buffer"),
            size: object_stride * draw_capacity as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgpui-3d.material_buffer"),
            size: material_stride * draw_capacity as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let object_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rgpui-3d.object_bind_group"),
            layout: &object_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &object_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<ObjectUniform>() as u64),
                }),
            }],
        });

        let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rgpui-3d.material_bind_group"),
            layout: &material_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &material_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<MaterialUniform>() as u64),
                }),
            }],
        });

        // 蒙皮管线绑定组布局：骨骼矩阵 + morph 增量 + morph 权重。
        let skin_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rgpui-3d.skin_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let skin_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rgpui-3d.skin_shader"),
            source: wgpu::ShaderSource::Wgsl(SKIN_SHADER_SRC.into()),
        });

        let skin_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rgpui-3d.skin_pipeline_layout"),
            bind_group_layouts: &[
                Some(&frame_layout),
                Some(&object_layout),
                Some(&material_layout),
                Some(&texture_layout),
                Some(&skin_layout),
            ],
            immediate_size: 0,
        });

        let skin_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rgpui-3d.skin_pipeline"),
            layout: Some(&skin_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &skin_shader,
                entry_point: Some("vs_skin"),
                buffers: &[Some(SkinnedVertex::layout())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: msaa_state,
            fragment: Some(wgpu::FragmentState {
                module: &skin_shader,
                entry_point: Some("fs_skin"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview_mask: None,
            cache: None,
        });

        let bone_capacity = 128u32;
        let bone_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgpui-3d.bone_buffer"),
            size: (bone_capacity as u64) * 64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let morph_delta_capacity = 1u32;
        let morph_delta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgpui-3d.morph_delta_buffer"),
            size: morph_delta_capacity as u64 * 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let morph_weight_capacity = 1u32;
        let morph_weight_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgpui-3d.morph_weight_buffer"),
            size: morph_weight_capacity as u64 * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bone_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rgpui-3d.bone_bg"),
            layout: &skin_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &bone_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(bone_capacity as u64 * 64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &morph_delta_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(morph_delta_capacity as u64 * 16),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &morph_weight_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(morph_weight_capacity as u64 * 4),
                    }),
                },
            ],
        });

        Ok(Self {
            device,
            queue,
            color_texture,
            depth_texture,
            msaa_sample_count: sample_count,
            resolve_texture,
            _frame_layout: frame_layout,
            object_layout,
            material_layout,
            texture_layout,
            pipeline,
            pipeline_layout,
            frame_buffer,
            frame_bind_group,
            object_stride,
            material_stride,
            object_buffer,
            material_buffer,
            object_bind_group,
            material_bind_group,
            draw_capacity,
            white_bind_group,
            textures: HashMap::new(),
            gpu_scene: GpuScene::new(),
            width,
            height,
            color_format,
            clear_color: [1.0, 1.0, 1.0, 1.0],
            frame_index: 0,
            skin_pipeline,
            skin_pipeline_layout,
            skin_layout,
            skinned_meshes: HashMap::new(),
            skins: Vec::new(),
            mesh_to_skin: HashMap::new(),
            mesh_to_node: HashMap::new(),
            joints: Vec::new(),
            anim_clips: Vec::new(),
            active_anim: 0,
            anim_time: 0.0,
            anim_speed: 1.0,
            anim_paused: false,
            bone_buffer,
            bone_capacity,
            bone_bg,
            morph_meshes: BTreeMap::new(),
            node_to_morph_meshes: HashMap::new(),
            morph_delta_buffer,
            morph_delta_capacity,
            morph_weight_buffer,
            morph_weight_capacity,
            cached_local_trs: Vec::new(),
            cached_global_mats: Vec::new(),
            cached_bone_data: Vec::new(),
            anim_layers: Vec::new(),
            anim_fade: None,
            joint_overrides: Vec::new(),
        })
    }

    /// 调整渲染尺寸
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.recreate_render_targets();
    }

    /// 重建渲染目标纹理（颜色、深度、resolve），使用当前 msaa_sample_count
    fn recreate_render_targets(&mut self) {
        let msaa = self.msaa_sample_count > 1;
        let color_usage = if msaa {
            wgpu::TextureUsages::RENDER_ATTACHMENT
        } else {
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC
        };
        self.color_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rgpui-3d.color"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: self.msaa_sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: self.color_format,
            usage: color_usage,
            view_formats: &[],
        });
        if msaa {
            self.resolve_texture = Some(self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("rgpui-3d.resolve"),
                size: wgpu::Extent3d {
                    width: self.width,
                    height: self.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.color_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            }));
        } else {
            self.resolve_texture = None;
        }
        self.depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rgpui-3d.depth"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: self.msaa_sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
    }

    /// 重建渲染管线，使用当前 msaa_sample_count
    fn recreate_pipelines(&mut self) {
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("rgpui-3d.shader"),
                source: wgpu::ShaderSource::Wgsl(crate::shader::SHADER_SRC.into()),
            });
        let skin_shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("rgpui-3d.skin_shader"),
                source: wgpu::ShaderSource::Wgsl(crate::shader::SKIN_SHADER_SRC.into()),
            });
        let msaa_state = wgpu::MultisampleState {
            count: self.msaa_sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        };

        self.pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("rgpui-3d.pipeline"),
                layout: Some(&self.pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(PackedVertex::layout())],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                primitive: wgpu::PrimitiveState {
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: msaa_state,
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.color_format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                multiview_mask: None,
                cache: None,
            });

        self.skin_pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("rgpui-3d.skin_pipeline"),
                layout: Some(&self.skin_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &skin_shader,
                    entry_point: Some("vs_skin"),
                    buffers: &[Some(SkinnedVertex::layout())],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                primitive: wgpu::PrimitiveState {
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: msaa_state,
                fragment: Some(wgpu::FragmentState {
                    module: &skin_shader,
                    entry_point: Some("fs_skin"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.color_format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                multiview_mask: None,
                cache: None,
            });
    }

    /// 设置 MSAA 采样数（1 = 关闭抗锯齿，推荐 4 或 8）
    pub fn set_msaa_sample_count(&mut self, sample_count: u32) {
        let count = match sample_count {
            0 | 1 => 1,
            n => n.next_power_of_two().min(16),
        };
        if count == self.msaa_sample_count {
            return;
        }
        self.msaa_sample_count = count;
        self.recreate_render_targets();
        self.recreate_pipelines();
    }

    /// 获取当前 MSAA 采样数（1 = 关闭）
    pub fn msaa_sample_count(&self) -> u32 {
        self.msaa_sample_count
    }

    /// 开关抗锯齿（开启使用 4x MSAA）
    pub fn set_msaa_enabled(&mut self, enabled: bool) {
        self.set_msaa_sample_count(if enabled { 4 } else { 1 });
    }

    /// 抗锯齿是否开启
    pub fn msaa_enabled(&self) -> bool {
        self.msaa_sample_count > 1
    }

    /// 获取渲染宽度
    pub fn width(&self) -> u32 {
        self.width
    }
    /// 获取渲染高度
    pub fn height(&self) -> u32 {
        self.height
    }

    /// 设置渲染背景颜色（RGBA，0.0-1.0）
    pub fn set_clear_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.clear_color = [r, g, b, a];
    }

    // ========================================================================
    // 资源注册
    // ========================================================================

    /// 注册网格到 GPU 场景
    pub fn register_mesh(&mut self, id: MeshId, geometry: &Geometry) -> Result<(), ScenixError> {
        self.gpu_scene.register_mesh(&self.device, id, geometry)?;
        Ok(())
    }

    /// 注册 PBR 材质
    pub fn register_pbr_material(
        &mut self,
        id: MaterialId,
        material: &scenekit::PbrMaterial,
    ) -> Result<(), ScenixError> {
        self.gpu_scene.register_pbr_material(id, material)?;
        Ok(())
    }

    /// 注册无光照材质
    pub fn register_unlit_material(
        &mut self,
        id: MaterialId,
        material: &scenekit::UnlitMaterial,
    ) -> Result<(), ScenixError> {
        self.gpu_scene.register_unlit_material(id, material)?;
        Ok(())
    }

    /// 注册 Lambert 材质
    pub fn register_lambert_material(
        &mut self,
        id: MaterialId,
        material: &scenekit::LambertMaterial,
    ) -> Result<(), ScenixError> {
        self.gpu_scene.register_lambert_material(id, material)?;
        Ok(())
    }

    /// 注册卡通着色材质
    pub fn register_toon_material(
        &mut self,
        id: MaterialId,
        material: &scenekit::ToonMaterial,
    ) -> Result<(), ScenixError> {
        self.gpu_scene.register_toon_material(id, material)?;
        Ok(())
    }

    /// 注册光照
    pub fn register_light(
        &mut self,
        id: scenekit::LightId,
        light: RendererLight,
    ) -> Result<(), ScenixError> {
        self.gpu_scene.register_light(id, light)?;
        Ok(())
    }

    /// 上传纹理到 GPU 并注册
    pub fn register_texture(
        &mut self,
        id: TextureId,
        texture: &scenekit::Texture2D,
        sampler: scenekit::Sampler,
    ) -> Result<(), ScenixError> {
        let wgpu_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rgpui-3d.texture"),
            size: wgpu::Extent3d {
                width: texture.width,
                height: texture.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let bytes_per_row = texture.width * 4;
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &wgpu_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &texture.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(texture.height),
            },
            wgpu::Extent3d {
                width: texture.width,
                height: texture.height,
                depth_or_array_layers: 1,
            },
        );

        let view = wgpu_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let wgpu_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: filter_to_wgpu(sampler.mag_filter),
            min_filter: filter_to_wgpu(sampler.min_filter),
            mipmap_filter: mip_filter_to_wgpu(sampler.mip_filter),
            address_mode_u: address_to_wgpu(sampler.address_u),
            address_mode_v: address_to_wgpu(sampler.address_v),
            address_mode_w: address_to_wgpu(sampler.address_w),
            ..Default::default()
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rgpui-3d.texture_bind_group"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&wgpu_sampler),
                },
            ],
        });

        self.gpu_scene.register_texture2d(id, texture, sampler)?;
        self.textures.insert(
            id,
            TextureEntry {
                _texture: wgpu_texture,
                bind_group,
            },
        );
        Ok(())
    }

    /// 注册 glTF 资产的所有网格、材质和纹理到 GPU 场景
    pub fn register_gltf_asset(&mut self, asset: &scenekit::GltfAsset) -> Result<(), ScenixError> {
        for (id, geometry) in &asset.meshes {
            self.register_mesh(*id, geometry)?;
        }
        for (id, texture) in &asset.textures {
            let sampler = asset.samplers.get(id).copied().unwrap_or_default();
            self.register_texture(*id, texture, sampler)?;
        }
        for (id, material) in &asset.materials {
            self.register_pbr_material(*id, material)?;
        }
        Ok(())
    }

    // ========================================================================
    // glTF 蒙皮/动画加载
    // ========================================================================

    /// 加载 glTF 文件的蒙皮和动画数据，创建蒙皮网格 GPU 资源
    pub fn load_gltf_skins(
        &mut self,
        path: &str,
        asset: &scenekit::GltfAsset,
    ) -> Result<(), ScenixError> {
        let (document, buffer_data, _image_data) =
            gltf::import(path).map_err(|_| ScenixError::Load(scenekit::LoadError::Io))?;

        let gltf_nodes: Vec<gltf::Node> = document.nodes().collect();
        let mut parent_of: Vec<Option<usize>> = vec![None; gltf_nodes.len()];
        for n in &gltf_nodes {
            for child in n.children() {
                if child.index() < gltf_nodes.len() {
                    parent_of[child.index()] = Some(n.index());
                }
            }
        }
        let joints: Vec<JointNode> = gltf_nodes
            .iter()
            .map(|n| {
                let (t, r, s) = n.transform().decomposed();
                JointNode {
                    parent: parent_of[n.index()],
                    default_trs: (t, r, s),
                    name: n.name().unwrap_or_default().to_string(),
                }
            })
            .collect();

        self.skins.clear();
        self.skinned_meshes.clear();
        self.mesh_to_skin.clear();
        self.morph_meshes.clear();
        self.node_to_morph_meshes.clear();
        let mut mesh_to_skin: HashMap<MeshId, usize> = HashMap::new();
        let mut morph_meshes: BTreeMap<MeshId, MorphMesh> = BTreeMap::new();
        let mut node_to_morph_meshes: HashMap<usize, Vec<MeshId>> = HashMap::new();

        // 计算每个 glTF mesh 的起始 MeshId（匹配 GltfLoader 的 1-based 顺序分配）
        let mut mesh_prim_start: Vec<u64> = Vec::new();
        let mut next_id = 1u64;
        for gltf_mesh in document.meshes() {
            mesh_prim_start.push(next_id);
            next_id += gltf_mesh.primitives().count() as u64;
        }

        // 建立 mesh_id → node 索引映射（用于动画场景中非蒙皮网格跟随骨骼）
        self.mesh_to_node.clear();
        for node in document.nodes() {
            if let Some(mesh_gltf) = node.mesh() {
                let mesh_idx = mesh_gltf.index();
                if mesh_idx < mesh_prim_start.len() {
                    let start = mesh_prim_start[mesh_idx];
                    let count = mesh_gltf.primitives().count() as u64;
                    for prim_offset in 0..count {
                        let mesh_id = MeshId::from(start + prim_offset);
                        self.mesh_to_node.insert(mesh_id, node.index());
                    }
                }
            }
        }

        for node in document.nodes() {
            let Some(mesh_gltf) = node.mesh() else {
                continue;
            };
            let mesh_idx = mesh_gltf.index();
            let Some(prim_start) = mesh_prim_start.get(mesh_idx).copied() else {
                continue;
            };
            let Some(gltf_mesh) = document.meshes().nth(mesh_idx) else {
                continue;
            };

            // 同一个 skin 的逆绑定矩阵只解析一次，供该节点下所有 primitive 共享。
            let skin_index = if let Some(skin) = node.skin() {
                let ibm: Vec<scenekit::Mat4> = {
                    let reader = skin.reader(|buffer| Some(&buffer_data[buffer.index()].0));
                    let Some(iter) = reader.read_inverse_bind_matrices() else {
                        continue;
                    };
                    iter.map(|cols| {
                        scenekit::Mat4::from_cols(
                            scenekit::Vec4::new(cols[0][0], cols[0][1], cols[0][2], cols[0][3]),
                            scenekit::Vec4::new(cols[1][0], cols[1][1], cols[1][2], cols[1][3]),
                            scenekit::Vec4::new(cols[2][0], cols[2][1], cols[2][2], cols[2][3]),
                            scenekit::Vec4::new(cols[3][0], cols[3][1], cols[3][2], cols[3][3]),
                        )
                    })
                    .collect()
                };
                let joint_node_indices: Vec<usize> = skin.joints().map(|j| j.index()).collect();
                let index = self.skins.len();
                self.skins.push(SkinData {
                    inverse_bind_matrices: ibm,
                    joint_node_indices,
                });
                Some(index)
            } else {
                None
            };

            let morph_names = morph_target_names(&gltf_mesh);
            let mesh_weights = node
                .weights()
                .or_else(|| gltf_mesh.weights())
                .map(<[f32]>::to_vec);

            // 为每个 primitive 创建独立的 GPU 网格。蒙皮节点与 morph-only 节点
            // （VRM 表情网格没有 JOINTS/WEIGHTS）都走皮肤顶点格式。
            for (prim_i, primitive) in gltf_mesh.primitives().enumerate() {
                let prim_mesh_id = MeshId::new(prim_start + prim_i as u64);
                if !asset.meshes.contains_key(&prim_mesh_id)
                    || self.skinned_meshes.contains_key(&prim_mesh_id)
                {
                    continue;
                }

                let reader = primitive.reader(|buffer| Some(&buffer_data[buffer.index()].0));
                let positions: Vec<[f32; 3]> = reader
                    .read_positions()
                    .map(|iter| iter.collect())
                    .unwrap_or_default();
                let normals: Vec<[f32; 3]> = reader
                    .read_normals()
                    .map(|iter| iter.collect())
                    .unwrap_or_default();
                let uvs: Vec<[f32; 2]> = reader
                    .read_tex_coords(0)
                    .map(|iter| iter.into_f32().collect())
                    .unwrap_or_default();
                let colors: Vec<[f32; 4]> = reader
                    .read_colors(0)
                    .map(|iter| iter.into_rgba_f32().collect())
                    .unwrap_or_default();

                let joint_indices: Vec<[u32; 4]> = reader
                    .read_joints(0)
                    .map(|joints| match joints {
                        gltf::mesh::util::ReadJoints::U8(iter) => iter
                            .map(|j| [j[0] as u32, j[1] as u32, j[2] as u32, j[3] as u32])
                            .collect(),
                        gltf::mesh::util::ReadJoints::U16(iter) => iter
                            .map(|j| [j[0] as u32, j[1] as u32, j[2] as u32, j[3] as u32])
                            .collect(),
                    })
                    .unwrap_or_default();
                let joint_weights: Vec<[f32; 4]> = reader
                    .read_weights(0)
                    .map(|weights| match weights {
                        gltf::mesh::util::ReadWeights::U8(iter) => iter
                            .map(|w| {
                                [
                                    w[0] as f32 / 255.0,
                                    w[1] as f32 / 255.0,
                                    w[2] as f32 / 255.0,
                                    w[3] as f32 / 255.0,
                                ]
                            })
                            .collect(),
                        gltf::mesh::util::ReadWeights::U16(iter) => iter
                            .map(|w| {
                                [
                                    w[0] as f32 / 65535.0,
                                    w[1] as f32 / 65535.0,
                                    w[2] as f32 / 65535.0,
                                    w[3] as f32 / 65535.0,
                                ]
                            })
                            .collect(),
                        gltf::mesh::util::ReadWeights::F32(iter) => iter.collect(),
                    })
                    .unwrap_or_default();

                let has_skin =
                    skin_index.is_some() && !joint_indices.is_empty() && !joint_weights.is_empty();
                let indices: Vec<u32> = reader
                    .read_indices()
                    .map(|iter| iter.into_u32().collect())
                    .unwrap_or_default();

                // glTF morph target：POSITION 是增量，NORMAL 是目标绝对法线，
                // 这里换算成相对基法线的增量（与 scenekit-loader 口径一致）。
                let morph_targets: Vec<MorphTargetData> = {
                    let mut base_normals: Option<Vec<[f32; 3]>> = None;
                    let normalize = |n: [f32; 3]| {
                        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                        if len > 1e-6 {
                            [n[0] / len, n[1] / len, n[2] / len]
                        } else {
                            [0.0; 3]
                        }
                    };
                    reader
                        .read_morph_targets()
                        .enumerate()
                        .map(|(target_index, (target_positions, target_normals, _))| {
                            let name = morph_names
                                .get(target_index)
                                .filter(|name| !name.is_empty())
                                .cloned()
                                .unwrap_or_else(|| format!("morph-{target_index}"));
                            let positions_delta = target_positions
                                .map(|iter| iter.collect())
                                .unwrap_or_default();
                            let normals_delta = match target_normals {
                                Some(iter) => {
                                    let base = base_normals.get_or_insert_with(|| {
                                        reader
                                            .read_normals()
                                            .map(|iter| iter.map(normalize).collect())
                                            .unwrap_or_default()
                                    });
                                    if base.is_empty() {
                                        Vec::new()
                                    } else {
                                        iter.enumerate()
                                            .map(|(i, normal)| {
                                                let absolute = normalize(normal);
                                                let origin =
                                                    base.get(i).copied().unwrap_or([0.0; 3]);
                                                [
                                                    absolute[0] - origin[0],
                                                    absolute[1] - origin[1],
                                                    absolute[2] - origin[2],
                                                ]
                                            })
                                            .collect()
                                    }
                                }
                                None => Vec::new(),
                            };
                            MorphTargetData {
                                name,
                                positions: positions_delta,
                                normals: normals_delta,
                            }
                        })
                        .collect()
                };
                let has_morph = !morph_targets.is_empty();

                // 既没有蒙皮也没有 morph 的网格继续走普通管线，不复制顶点。
                if !has_skin && !has_morph {
                    continue;
                }

                let mut verts: Vec<SkinnedVertex> = Vec::with_capacity(positions.len());
                let count = positions.len();
                for vi in 0..count {
                    let c = colors.get(vi).copied().unwrap_or([1.0, 1.0, 1.0, 1.0]);
                    let (ji, jw) = if has_skin {
                        (
                            joint_indices.get(vi).copied().unwrap_or([0; 4]),
                            joint_weights
                                .get(vi)
                                .copied()
                                .unwrap_or([1.0, 0.0, 0.0, 0.0]),
                        )
                    } else {
                        ([0; 4], [0.0; 4])
                    };
                    verts.push(SkinnedVertex {
                        position: positions.get(vi).copied().unwrap_or([0.0; 3]),
                        normal: normals.get(vi).copied().unwrap_or([0.0; 3]),
                        uv: uvs.get(vi).copied().unwrap_or([0.0; 2]),
                        color: c,
                        tangent: [0.0; 4],
                        joint_indices: ji,
                        joint_weights: jw,
                    });
                }

                if verts.is_empty() || indices.is_empty() {
                    continue;
                }
                let index_format = if indices.len() <= u16::MAX as usize {
                    wgpu::IndexFormat::Uint16
                } else {
                    wgpu::IndexFormat::Uint32
                };
                let index_data: Vec<u8> = match index_format {
                    wgpu::IndexFormat::Uint16 => bytemuck::cast_slice(
                        &indices.iter().map(|&i| i as u16).collect::<Vec<u16>>(),
                    )
                    .to_vec(),
                    wgpu::IndexFormat::Uint32 => bytemuck::cast_slice(&indices).to_vec(),
                };
                // wgpu 要求 write_buffer 的数据长度满足 COPY_BUFFER_ALIGNMENT (4 字节对齐)
                let aligned_len = (index_data.len() + 3) & !3;
                let index_data = if index_data.len() < aligned_len {
                    let mut padded = index_data;
                    padded.resize(aligned_len, 0);
                    padded
                } else {
                    index_data
                };

                let vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("rgpui-3d.skin_vb_{}", prim_mesh_id.get())),
                    size: (verts.len() * std::mem::size_of::<SkinnedVertex>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.queue
                    .write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&verts));

                let index_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("rgpui-3d.skin_ib_{}", prim_mesh_id.get())),
                    size: index_data.len() as u64,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.queue.write_buffer(&index_buffer, 0, &index_data);

                self.skinned_meshes.insert(
                    prim_mesh_id,
                    GpuSkinnedMesh {
                        vertex_buffer,
                        index_buffer,
                        index_count: indices.len() as u32,
                        index_format,
                    },
                );
                if let Some(skin_index) = skin_index {
                    mesh_to_skin.insert(prim_mesh_id, skin_index);
                }

                if has_morph {
                    let mut weights = vec![0.0f32; morph_targets.len()];
                    if let Some(source) = mesh_weights.as_ref() {
                        for (i, weight) in weights.iter_mut().enumerate() {
                            *weight = source.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                        }
                    }
                    morph_meshes.insert(
                        prim_mesh_id,
                        MorphMesh {
                            base: 0,
                            weight_base: 0,
                            vertex_count: verts.len() as u32,
                            targets: morph_targets,
                            overrides: vec![None; weights.len()],
                            weights,
                        },
                    );
                    node_to_morph_meshes
                        .entry(node.index())
                        .or_default()
                        .push(prim_mesh_id);
                }
            }
        }

        self.mesh_to_skin = mesh_to_skin;
        self.joints = joints;
        self.morph_meshes = morph_meshes;
        self.node_to_morph_meshes = node_to_morph_meshes;
        self.rebuild_morph_meshes();

        self.anim_clips.clear();
        for anim in document.animations() {
            // glTF 的 sampler 索引是文档全局编号，而 `AnimChannel::sampler` 用来索引
            // 当前剪辑内的 `samplers` 数组。这里按通道首次出现顺序重新分配局部槽位，
            // 否则第二段动画会越界或采样到别的剪辑的曲线。
            let mut sampler_slot: HashMap<usize, usize> = HashMap::new();
            let mut samplers: Vec<AnimSampler> = Vec::new();
            for channel in anim.channels() {
                let global_index = channel.sampler().index();
                if sampler_slot.contains_key(&global_index) {
                    continue;
                }
                let reader = channel.reader(|buffer| Some(&buffer_data[buffer.index()].0));
                let times: Vec<f32> = reader
                    .read_inputs()
                    .map(|i| i.collect())
                    .unwrap_or_default();
                let is_cubic = channel.sampler().interpolation()
                    == gltf::animation::Interpolation::CubicSpline;
                let mut outputs: Vec<[f32; 4]> = Vec::new();
                let mut morph_weights: Vec<f32> = Vec::new();
                let mut morph_weight_count = 0usize;
                match reader.read_outputs() {
                    Some(gltf::animation::util::ReadOutputs::Translations(iter)) => {
                        let all: Vec<[f32; 3]> = iter.collect();
                        outputs = if is_cubic {
                            all.iter()
                                .skip(1)
                                .step_by(3)
                                .map(|v| [v[0], v[1], v[2], 0.0])
                                .collect()
                        } else {
                            all.iter().map(|v| [v[0], v[1], v[2], 0.0]).collect()
                        };
                    }
                    Some(gltf::animation::util::ReadOutputs::Rotations(iter)) => {
                        let all: Vec<[f32; 4]> =
                            iter.into_f32().map(|v| [v[0], v[1], v[2], v[3]]).collect();
                        outputs = if is_cubic {
                            all.iter().skip(1).step_by(3).copied().collect()
                        } else {
                            all
                        };
                    }
                    Some(gltf::animation::util::ReadOutputs::Scales(iter)) => {
                        let all: Vec<[f32; 3]> = iter.collect();
                        outputs = if is_cubic {
                            all.iter()
                                .skip(1)
                                .step_by(3)
                                .map(|v| [v[0], v[1], v[2], 0.0])
                                .collect()
                        } else {
                            all.iter().map(|v| [v[0], v[1], v[2], 0.0]).collect()
                        };
                    }
                    Some(gltf::animation::util::ReadOutputs::MorphTargetWeights(iter)) => {
                        let all: Vec<f32> = iter.into_f32().collect();
                        let keyframes = times.len().max(1);
                        let per_key = all.len() / keyframes;
                        // 三次样条每帧布局为 [入切线 N][值 N][出切线 N]，
                        // 只保留中间的值块；其余插值类型每帧就是 N 个权重。
                        let weight_count = if is_cubic { per_key / 3 } else { per_key };
                        if weight_count > 0 && per_key >= weight_count {
                            morph_weight_count = weight_count;
                            let value_offset = if is_cubic { weight_count } else { 0 };
                            morph_weights = Vec::with_capacity(keyframes * weight_count);
                            for frame in 0..keyframes {
                                let start = frame * per_key + value_offset;
                                if start >= all.len() {
                                    break;
                                }
                                let end = (start + weight_count).min(all.len());
                                morph_weights.extend_from_slice(&all[start..end]);
                            }
                        }
                    }
                    None => {}
                }
                let local_index = samplers.len();
                sampler_slot.insert(global_index, local_index);
                samplers.push(AnimSampler {
                    times,
                    outputs,
                    morph_weights,
                    morph_weight_count,
                });
            }
            let channels: Vec<AnimChannel> = anim
                .channels()
                .filter_map(|c| {
                    use gltf::animation::Property;
                    let target = match c.target().property() {
                        Property::Translation => 0,
                        Property::Rotation => 1,
                        Property::Scale => 2,
                        Property::MorphTargetWeights => 3,
                    };
                    let sampler = *sampler_slot.get(&c.sampler().index())?;
                    Some(AnimChannel {
                        node: c.target().node().index(),
                        target,
                        sampler,
                    })
                })
                .collect();
            self.anim_clips.push(AnimClip {
                _name: anim.name().unwrap_or("").to_string(),
                samplers,
                channels,
            });
        }

        self.active_anim = 0;
        self.anim_time = 0.0;
        self.anim_layers.clear();
        self.anim_fade = None;
        self.joint_overrides.clear();

        Ok(())
    }

    // ========================================================================
    // 动画控制
    // ========================================================================

    /// 采样一个剪辑到 `pose`（先填入静止姿态），同时输出它驱动的 morph 权重。
    fn sample_clip_into(
        &self,
        clip_index: usize,
        time: f32,
        pose: &mut [([f32; 3], [f32; 4], [f32; 3])],
        morph: &mut HashMap<MeshId, Vec<f32>>,
    ) {
        let Some(clip) = self.anim_clips.get(clip_index) else {
            return;
        };
        for channel in &clip.channels {
            let Some(sampler) = clip.samplers.get(channel.sampler) else {
                continue;
            };
            if sampler.times.is_empty() {
                continue;
            }
            let (low, high, frac) = sample_factors(&sampler.times, time);
            if channel.target == 3 {
                if sampler.morph_weight_count == 0 || sampler.morph_weights.is_empty() {
                    continue;
                }
                let Some(meshes) = self.node_to_morph_meshes.get(&channel.node) else {
                    continue;
                };
                let count = sampler.morph_weight_count;
                for mesh_id in meshes {
                    let Some(mesh) = self.morph_meshes.get(mesh_id) else {
                        continue;
                    };
                    let values = morph
                        .entry(*mesh_id)
                        .or_insert_with(|| vec![0.0f32; mesh.targets.len()]);
                    for (i, value) in values.iter_mut().enumerate().take(count) {
                        let lo = sampler
                            .morph_weights
                            .get(low * count + i)
                            .copied()
                            .unwrap_or(0.0);
                        let hi = sampler
                            .morph_weights
                            .get(high * count + i)
                            .copied()
                            .unwrap_or(lo);
                        *value = lo + (hi - lo) * frac;
                    }
                }
                continue;
            }
            if low >= sampler.outputs.len() || high >= sampler.outputs.len() {
                continue;
            }
            let Some(slot) = pose.get_mut(channel.node) else {
                continue;
            };
            let value = lerp_value(
                sampler.outputs[low],
                sampler.outputs[high],
                frac,
                channel.target,
            );
            match channel.target {
                0 => slot.0 = [value[0], value[1], value[2]],
                1 => slot.1 = value,
                2 => slot.2 = [value[0], value[1], value[2]],
                _ => {}
            }
        }
    }

    /// 剪辑总时长（最后一个关键帧时间）。
    fn clip_duration(&self, clip_index: usize) -> f32 {
        self.anim_clips
            .get(clip_index)
            .map(|clip| {
                clip.samplers
                    .iter()
                    .filter_map(|s| s.times.last().copied())
                    .fold(0.0f32, f32::max)
            })
            .unwrap_or(0.0)
    }

    /// 推进动画时间，混合所有层与 cross-fade，计算骨骼矩阵并上传到 GPU。
    ///
    /// 即使暂停、没有动画剪辑或模型没有蒙皮，也会用静止姿态 + 关节覆盖
    /// 重算一次局部/世界矩阵，保证手动摆姿势在静态模型上同样生效。
    pub fn advance_animation(&mut self, dt: f32) {
        let num_joints = self.joints.len();

        if !self.anim_paused {
            if self.active_anim < self.anim_clips.len() {
                self.anim_time += dt * self.anim_speed;
                let duration = self.clip_duration(self.active_anim);
                if duration > 0.0 {
                    self.anim_time = self.anim_time.rem_euclid(duration);
                }
            }
            let layer_durations: Vec<f32> = self
                .anim_layers
                .iter()
                .map(|layer| self.clip_duration(layer.clip))
                .collect();
            for (layer, duration) in self.anim_layers.iter_mut().zip(layer_durations) {
                layer.time += dt * layer.speed;
                if duration > 0.0 {
                    layer.time = layer.time.rem_euclid(duration);
                }
            }
            let fade_duration = self
                .anim_fade
                .as_ref()
                .map(|fade| self.clip_duration(fade.clip))
                .unwrap_or(0.0);
            if let Some(fade) = self.anim_fade.as_mut() {
                fade.time += dt * fade.speed;
                fade.elapsed += dt;
                if fade_duration > 0.0 {
                    fade.time = fade.time.rem_euclid(fade_duration);
                }
            }
            if self
                .anim_fade
                .as_ref()
                .is_some_and(|fade| fade.elapsed >= fade.duration)
            {
                self.anim_fade = None;
            }
        }

        if num_joints == 0 {
            return;
        }

        // 静止姿态：未被子剪辑覆盖的分量保持模型原始 TRS。
        let rest: Vec<([f32; 3], [f32; 4], [f32; 3])> =
            self.joints.iter().map(|j| j.default_trs).collect();
        let mut pose = rest.clone();
        let mut morph_pose: HashMap<MeshId, Vec<f32>> = HashMap::new();
        if self.active_anim < self.anim_clips.len() {
            self.sample_clip_into(self.active_anim, self.anim_time, &mut pose, &mut morph_pose);
        }

        // cross-fade：旧剪辑按 smoothstep 权重让位给当前剪辑。
        if let Some(fade) = self.anim_fade.as_ref() {
            let t = (fade.elapsed / fade.duration.max(1e-6)).clamp(0.0, 1.0);
            let eased = t * t * (3.0 - 2.0 * t);
            let mut old_pose = rest.clone();
            let mut old_morph: HashMap<MeshId, Vec<f32>> = HashMap::new();
            self.sample_clip_into(fade.clip, fade.time, &mut old_pose, &mut old_morph);
            for (slot, old) in pose.iter_mut().zip(old_pose.iter()) {
                blend_trs(slot, *old, 1.0 - eased);
            }
            for (mesh_id, old_values) in &old_morph {
                let Some(mesh) = self.morph_meshes.get(mesh_id) else {
                    continue;
                };
                let values = morph_pose
                    .entry(*mesh_id)
                    .or_insert_with(|| vec![0.0f32; mesh.targets.len()]);
                for (i, value) in values.iter_mut().enumerate().take(old_values.len()) {
                    *value = old_values[i] + (*value - old_values[i]) * eased;
                }
            }
        }

        // 叠加层：非 additive 层按权重覆盖，additive 层以静止姿态为基准叠加增量。
        for layer in &self.anim_layers {
            if layer.weight <= 0.0 {
                continue;
            }
            let mut layer_pose = rest.clone();
            let mut layer_morph: HashMap<MeshId, Vec<f32>> = HashMap::new();
            self.sample_clip_into(layer.clip, layer.time, &mut layer_pose, &mut layer_morph);
            for i in 0..num_joints {
                if let Some(mask) = &layer.joint_mask {
                    if !mask.get(i).copied().unwrap_or(false) {
                        continue;
                    }
                }
                if layer.additive {
                    apply_additive_trs(&mut pose[i], layer_pose[i], rest[i], layer.weight);
                } else {
                    blend_trs(&mut pose[i], layer_pose[i], layer.weight);
                }
            }
            for (mesh_id, layer_values) in &layer_morph {
                let Some(mesh) = self.morph_meshes.get(mesh_id) else {
                    continue;
                };
                let values = morph_pose
                    .entry(*mesh_id)
                    .or_insert_with(|| vec![0.0f32; mesh.targets.len()]);
                for (i, value) in values.iter_mut().enumerate().take(layer_values.len()) {
                    *value = if layer.additive {
                        *value + layer_values[i] * layer.weight
                    } else {
                        *value + (layer_values[i] - *value) * layer.weight
                    };
                }
            }
        }

        // 手动覆盖最后应用：无论是否暂停、是否切到别的剪辑，覆盖都优先。
        for (i, over) in self.joint_overrides.iter().enumerate() {
            if i >= num_joints {
                break;
            }
            if let Some(translation) = over.translation {
                pose[i].0 = translation;
            }
            if let Some(rotation) = over.rotation {
                pose[i].1 = rotation;
            }
            if let Some(scale) = over.scale {
                pose[i].2 = scale;
            }
        }

        // 写回 CPU 缓存并更新 morph 权重（手动覆盖过的目标不参与动画）。
        self.cached_local_trs.clear();
        self.cached_local_trs.extend_from_slice(&pose);
        let mut morph_changed = false;
        for (mesh_id, values) in &morph_pose {
            let Some(mesh) = self.morph_meshes.get_mut(mesh_id) else {
                continue;
            };
            for i in 0..values.len().min(mesh.weights.len()) {
                if mesh.overrides[i].is_some() {
                    continue;
                }
                let weight = values[i].clamp(0.0, 1.0);
                if (mesh.weights[i] - weight).abs() > 1e-6 {
                    mesh.weights[i] = weight;
                    morph_changed = true;
                }
            }
        }
        if morph_changed {
            self.upload_morph_weights();
        }

        self.cached_global_mats.clear();
        self.cached_global_mats
            .resize(num_joints, scenekit::Mat4::IDENTITY);
        let local_mats: Vec<scenekit::Mat4> = pose
            .iter()
            .map(|(t, r, s)| trs_to_mat4(*t, *r, *s))
            .collect();

        let mut children_of: Vec<Vec<usize>> = vec![Vec::new(); num_joints];
        for i in 0..num_joints {
            if let Some(p) = self.joints[i].parent {
                if p < num_joints {
                    children_of[p].push(i)
                }
            }
        }

        let mut queue: Vec<usize> = Vec::new();
        for i in 0..num_joints {
            // 没有父节点、或父索引越界（数据里指向了不存在的关节）都当作根节点处理。
            let is_root = match self.joints[i].parent {
                None => true,
                Some(p) => p >= num_joints,
            };
            if is_root {
                self.cached_global_mats[i] = local_mats[i];
                queue.push(i);
            }
        }
        while let Some(parent) = queue.pop() {
            for &child in &children_of[parent] {
                self.cached_global_mats[child] =
                    mat4_mul(&self.cached_global_mats[parent], &local_mats[child]);
                queue.push(child);
            }
        }

        let mut total_bones = 0usize;
        for skin in &self.skins {
            total_bones += skin.joint_node_indices.len()
        }
        if total_bones == 0 {
            return;
        }

        let needed_bytes = total_bones as u64 * 64;
        if needed_bytes > (self.bone_capacity as u64) * 64 {
            self.bone_capacity = (total_bones as u32).next_power_of_two();
            self.bone_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rgpui-3d.bone_buffer"),
                size: self.bone_capacity as u64 * 64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.rebuild_skin_bind_group();
        }

        self.cached_bone_data.clear();
        self.cached_bone_data.reserve(total_bones * 16);
        for skin in &self.skins {
            for (joint_idx, ibm) in skin
                .joint_node_indices
                .iter()
                .zip(skin.inverse_bind_matrices.iter())
            {
                let bone_mat = if *joint_idx < num_joints {
                    mat4_mul(&self.cached_global_mats[*joint_idx], ibm)
                } else {
                    mat4_identity()
                };
                self.cached_bone_data
                    .extend_from_slice(&mat4_to_flat(&bone_mat));
            }
        }
        self.queue.write_buffer(
            &self.bone_buffer,
            0,
            bytemuck::cast_slice(&self.cached_bone_data),
        );
    }

    /// 获取动画剪辑名称列表
    pub fn animation_names(&self) -> Vec<String> {
        self.anim_clips.iter().map(|c| c._name.clone()).collect()
    }
    /// 切换到指定动画剪辑
    pub fn set_active_animation(&mut self, index: usize) {
        if index < self.anim_clips.len() {
            self.active_anim = index;
            self.anim_time = 0.0
        }
    }
    /// 获取当前动画时间
    pub fn animation_time(&self) -> f32 {
        self.anim_time
    }
    /// 设置当前动画时间（秒）
    pub fn set_animation_time(&mut self, t: f32) {
        self.anim_time = t
    }
    /// 获取动画播放速度
    pub fn animation_speed(&self) -> f32 {
        self.anim_speed
    }
    /// 设置动画播放速度
    pub fn set_animation_speed(&mut self, speed: f32) {
        self.anim_speed = speed
    }
    /// 动画是否暂停
    pub fn is_animation_paused(&self) -> bool {
        self.anim_paused
    }
    /// 暂停/恢复动画播放
    pub fn set_animation_paused(&mut self, paused: bool) {
        self.anim_paused = paused
    }
    /// 获取当前动画剪辑的总时长（秒）
    pub fn animation_duration(&self) -> f32 {
        if self.anim_clips.is_empty() {
            return 0.0;
        }
        let clip = &self.anim_clips[self.active_anim];
        clip.samplers
            .iter()
            .filter_map(|s| s.times.last().copied())
            .fold(0.0f32, f32::max)
    }
    /// 获取当前动画剪辑索引
    pub fn active_animation_index(&self) -> usize {
        self.active_anim
    }
    /// 获取蒙皮网格数量
    pub fn skinned_mesh_count(&self) -> usize {
        self.skinned_meshes.len()
    }
    /// 获取关节数量
    pub fn joint_count(&self) -> usize {
        self.joints.len()
    }

    /// 生成程序化行走动画
    pub fn generate_walk_animation(&mut self, cycle_duration: f32, stride_angle: f32) {
        if self.skins.is_empty() {
            return;
        }
        let skin = &self.skins[0];
        let joint_set: std::collections::HashSet<usize> =
            skin.joint_node_indices.iter().copied().collect();

        let skeleton_root = skin
            .joint_node_indices
            .iter()
            .find(|&&node_idx| {
                node_idx < self.joints.len()
                    && match self.joints[node_idx].parent {
                        None => true,
                        Some(p) => !joint_set.contains(&p),
                    }
            })
            .copied();

        let Some(hips) = skeleton_root else { return };
        let hips_children: Vec<usize> = skin
            .joint_node_indices
            .iter()
            .filter(|&&node_idx| {
                node_idx < self.joints.len() && self.joints[node_idx].parent == Some(hips)
            })
            .copied()
            .collect();

        let (leg_indices, spine_idx) = if hips_children.len() >= 3 {
            let mut with_size: Vec<(usize, usize)> = hips_children
                .iter()
                .map(|&c| {
                    let size = skin
                        .joint_node_indices
                        .iter()
                        .filter(|&&node_idx| {
                            let mut cur = self.joints.get(node_idx).and_then(|j| j.parent);
                            while let Some(p) = cur {
                                if p == c {
                                    return true;
                                }
                                cur = self.joints.get(p).and_then(|j| j.parent)
                            }
                            false
                        })
                        .count();
                    (c, size)
                })
                .collect();
            with_size.sort_by_key(|&(_, s)| std::cmp::Reverse(s));
            let spine = with_size[0].0;
            let legs: Vec<usize> = with_size[1..].iter().map(|&(c, _)| c).collect();
            (legs, Some(spine))
        } else if hips_children.len() == 2 {
            (hips_children, None)
        } else {
            (vec![], None)
        };

        if leg_indices.is_empty() {
            return;
        }

        let dt = cycle_duration / 16.0;
        let frames = 17;
        let times: Vec<f32> = (0..frames).map(|i| i as f32 * dt).collect();
        let mut samplers = Vec::new();
        let mut channels = Vec::new();

        for (leg_idx, &leg_node) in leg_indices.iter().enumerate() {
            let phase_offset = if leg_idx == 0 {
                0.0
            } else {
                std::f32::consts::PI
            };
            let dir = if leg_idx == 0 { 1.0 } else { -1.0 };

            let rotation_outputs: Vec<[f32; 4]> = (0..frames)
                .map(|i| {
                    let t = i as f32 / 16.0 * std::f32::consts::TAU + phase_offset;
                    let angle = dir * stride_angle * t.sin();
                    let half = angle * 0.5;
                    [half.sin(), 0.0, 0.0, half.cos()]
                })
                .collect();

            let sampler_idx = samplers.len();
            samplers.push(AnimSampler {
                times: times.clone(),
                outputs: rotation_outputs,
                morph_weights: Vec::new(),
                morph_weight_count: 0,
            });
            channels.push(AnimChannel {
                node: leg_node,
                target: 1,
                sampler: sampler_idx,
            });

            if let Some(knee) = skin
                .joint_node_indices
                .iter()
                .find(|&&node_idx| {
                    node_idx < self.joints.len() && self.joints[node_idx].parent == Some(leg_node)
                })
                .copied()
            {
                let knee_outputs: Vec<[f32; 4]> = (0..frames)
                    .map(|i| {
                        let t = i as f32 / 16.0 * std::f32::consts::TAU + phase_offset;
                        let bend = -0.3 * (t + std::f32::consts::FRAC_PI_4).cos().max(0.0);
                        let half = bend * 0.5;
                        [half.sin(), 0.0, 0.0, half.cos()]
                    })
                    .collect();

                let sampler_idx = samplers.len();
                samplers.push(AnimSampler {
                    times: times.clone(),
                    outputs: knee_outputs,
                    morph_weights: Vec::new(),
                    morph_weight_count: 0,
                });
                channels.push(AnimChannel {
                    node: knee,
                    target: 1,
                    sampler: sampler_idx,
                });
            }
        }

        if let Some(spine) = spine_idx {
            let spine_outputs: Vec<[f32; 4]> = (0..frames)
                .map(|i| {
                    let t = i as f32 / 16.0 * std::f32::consts::TAU;
                    let angle = 0.05 * t.sin();
                    let half = angle * 0.5;
                    [0.0, half.sin(), 0.0, half.cos()]
                })
                .collect();
            let sampler_idx = samplers.len();
            samplers.push(AnimSampler {
                times: times.clone(),
                outputs: spine_outputs,
                morph_weights: Vec::new(),
                morph_weight_count: 0,
            });
            channels.push(AnimChannel {
                node: spine,
                target: 1,
                sampler: sampler_idx,
            });
        }

        let hips_outputs: Vec<[f32; 4]> = (0..frames)
            .map(|i| {
                let t = i as f32 / 16.0 * std::f32::consts::TAU;
                [0.0, -0.02 * (t * 2.0).abs().cos(), 0.0, 0.0]
            })
            .collect();
        let sampler_idx = samplers.len();
        samplers.push(AnimSampler {
            times,
            outputs: hips_outputs,
            morph_weights: Vec::new(),
            morph_weight_count: 0,
        });
        channels.push(AnimChannel {
            node: hips,
            target: 0,
            sampler: sampler_idx,
        });

        self.anim_clips.push(AnimClip {
            _name: "Walk".to_string(),
            samplers,
            channels,
        });

        if self.active_anim >= self.anim_clips.len() - 1 {
            self.active_anim = self.anim_clips.len() - 1;
            self.anim_time = 0.0;
        }
    }

    /// 获取关节父节点索引
    pub fn joint_parent(&self, index: usize) -> Option<usize> {
        self.joints.get(index).and_then(|j| j.parent)
    }
    /// 获取关节的世界矩阵（需在 advance_animation 之后调用）
    pub fn joint_world_matrix(&self, index: usize) -> Option<scenekit::Mat4> {
        if index < self.cached_global_mats.len() {
            Some(self.cached_global_mats[index])
        } else {
            None
        }
    }
    /// 获取关节的世界位置
    pub fn joint_world_position(&self, index: usize) -> Option<scenekit::Vec3> {
        self.joint_world_matrix(index)
            .map(|m| scenekit::Vec3::new(m.cols[3].x, m.cols[3].y, m.cols[3].z))
    }
    /// 设置关节的旋转覆盖
    pub fn set_joint_rotation_override(&mut self, index: usize, rotation: scenekit::Quat) {
        self.ensure_joint_override_slot(index);
        if index < self.joints.len() {
            self.joint_overrides[index].rotation =
                Some([rotation.x, rotation.y, rotation.z, rotation.w]);
        }
    }
    /// 设置关节的平移覆盖
    pub fn set_joint_translation_override(&mut self, index: usize, translation: scenekit::Vec3) {
        self.ensure_joint_override_slot(index);
        if index < self.joints.len() {
            self.joint_overrides[index].translation =
                Some([translation.x, translation.y, translation.z]);
        }
    }
    /// 设置关节的缩放覆盖
    pub fn set_joint_scale_override(&mut self, index: usize, scale: scenekit::Vec3) {
        self.ensure_joint_override_slot(index);
        if index < self.joints.len() {
            self.joint_overrides[index].scale = Some([scale.x, scale.y, scale.z]);
        }
    }
    /// 确保覆盖数组能容纳 `index` 位置
    fn ensure_joint_override_slot(&mut self, index: usize) {
        if index < self.joints.len() && self.joint_overrides.len() <= index {
            self.joint_overrides
                .resize(index + 1, JointOverride::default());
        }
    }
    /// 清除单个关节的所有覆盖分量
    pub fn clear_joint_override(&mut self, index: usize) {
        if let Some(over) = self.joint_overrides.get_mut(index) {
            *over = JointOverride::default();
        }
    }
    /// 清除所有关节覆盖
    pub fn clear_joint_overrides(&mut self) {
        self.joint_overrides.clear()
    }
    /// 获取当前被覆盖的关节数量
    pub fn joint_override_count(&self) -> usize {
        self.joint_overrides
            .iter()
            .filter(|over| !over.is_empty())
            .count()
    }
    /// 一次设置关节的完整 TRS 覆盖
    pub fn set_joint_trs_override(
        &mut self,
        index: usize,
        translation: scenekit::Vec3,
        rotation: scenekit::Quat,
        scale: scenekit::Vec3,
    ) {
        self.set_joint_translation_override(index, translation);
        self.set_joint_rotation_override(index, rotation);
        self.set_joint_scale_override(index, scale);
    }
    /// 获取关节名称
    pub fn joint_name(&self, index: usize) -> Option<&str> {
        self.joints.get(index).map(|joint| joint.name.as_str())
    }
    /// 按名称查找关节索引（大小写不敏感，重名时取第一个）
    pub fn joint_index_by_name(&self, name: &str) -> Option<usize> {
        self.joints
            .iter()
            .position(|joint| joint.name.eq_ignore_ascii_case(name))
    }
    /// 获取全部关节名称（顺序与关节索引一致）
    pub fn joint_names(&self) -> Vec<String> {
        self.joints.iter().map(|joint| joint.name.clone()).collect()
    }
    /// 获取关节的静止姿态 TRS
    pub fn joint_rest_trs(&self, index: usize) -> Option<([f32; 3], [f32; 4], [f32; 3])> {
        self.joints.get(index).map(|joint| joint.default_trs)
    }
    /// 获取关节当前的局部 TRS（需在 `advance_animation` 之后调用）
    pub fn joint_local_trs(&self, index: usize) -> Option<([f32; 3], [f32; 4], [f32; 3])> {
        self.cached_local_trs.get(index).copied()
    }

    // ========================================================================
    // Morph target 控制
    // ========================================================================

    /// 列出所有带 morph target 的网格
    pub fn morph_mesh_ids(&self) -> Vec<MeshId> {
        self.morph_meshes.keys().copied().collect()
    }
    /// 带 morph target 的网格数量
    pub fn morph_mesh_count(&self) -> usize {
        self.morph_meshes.len()
    }
    /// 获取网格的 morph target 名称列表
    pub fn morph_target_names(&self, mesh: MeshId) -> Vec<String> {
        self.morph_meshes
            .get(&mesh)
            .map(MorphMesh::target_names)
            .unwrap_or_default()
    }
    /// 获取网格当前的 morph 权重
    pub fn morph_weights(&self, mesh: MeshId) -> Vec<f32> {
        self.morph_meshes
            .get(&mesh)
            .map(|m| m.weights.clone())
            .unwrap_or_default()
    }
    /// 按索引设置 morph 权重并锁定（后续动画不会再改写该目标）。
    pub fn set_morph_weight(&mut self, mesh: MeshId, index: usize, weight: f32) {
        let Some(target) = self.morph_meshes.get_mut(&mesh) else {
            return;
        };
        if index >= target.weights.len() {
            return;
        }
        let clamped = weight.clamp(0.0, 1.0);
        target.weights[index] = clamped;
        target.overrides[index] = Some(clamped);
        let offset = (target.weight_base as u64 + index as u64) * 4;
        self.queue.write_buffer(
            &self.morph_weight_buffer,
            offset,
            bytemuck::bytes_of(&clamped),
        );
    }
    /// 按名称设置 morph 权重并锁定；名称不存在时返回 `false`。
    pub fn set_morph_weight_by_name(&mut self, mesh: MeshId, name: &str, weight: f32) -> bool {
        let Some(index) = self
            .morph_meshes
            .get(&mesh)
            .and_then(|m| m.target_index(name))
        else {
            return false;
        };
        self.set_morph_weight(mesh, index, weight);
        true
    }
    /// 解除网格所有 morph 目标的手动锁定，恢复由动画/默认权重驱动。
    pub fn clear_morph_weights(&mut self, mesh: MeshId) {
        let Some(target) = self.morph_meshes.get_mut(&mesh) else {
            return;
        };
        for over in &mut target.overrides {
            *over = None;
        }
    }

    // ========================================================================
    // 多层动画
    // ========================================================================

    /// 追加一个动画层；剪辑索引非法时返回 `None`。
    ///
    /// `weight` 为 0 的层不会参与混合，可在保留层配置的同时临时禁用。
    pub fn add_animation_layer(&mut self, clip: usize, weight: f32) -> Option<usize> {
        if clip >= self.anim_clips.len() {
            return None;
        }
        self.anim_layers.push(AnimLayer::new(clip, weight));
        Some(self.anim_layers.len() - 1)
    }
    /// 追加一个 additive 层（以静止姿态为基准叠加增量）。
    pub fn add_additive_animation_layer(&mut self, clip: usize, weight: f32) -> Option<usize> {
        let index = self.add_animation_layer(clip, weight)?;
        self.anim_layers[index].additive = true;
        Some(index)
    }
    /// 移除动画层
    pub fn remove_animation_layer(&mut self, index: usize) {
        if index < self.anim_layers.len() {
            self.anim_layers.remove(index);
        }
    }
    /// 清空所有动画层
    pub fn clear_animation_layers(&mut self) {
        self.anim_layers.clear();
    }
    /// 动画层数量
    pub fn animation_layer_count(&self) -> usize {
        self.anim_layers.len()
    }
    /// 设置层权重（0..=1）
    pub fn set_animation_layer_weight(&mut self, index: usize, weight: f32) {
        if let Some(layer) = self.anim_layers.get_mut(index) {
            layer.weight = weight.clamp(0.0, 1.0);
        }
    }
    /// 设置层的局部播放时间
    pub fn set_animation_layer_time(&mut self, index: usize, time: f32) {
        if let Some(layer) = self.anim_layers.get_mut(index) {
            layer.time = time;
        }
    }
    /// 设置层播放速度
    pub fn set_animation_layer_speed(&mut self, index: usize, speed: f32) {
        if let Some(layer) = self.anim_layers.get_mut(index) {
            layer.speed = speed;
        }
    }
    /// 设置层是否以静止姿态为基准叠加
    pub fn set_animation_layer_additive(&mut self, index: usize, additive: bool) {
        if let Some(layer) = self.anim_layers.get_mut(index) {
            layer.additive = additive;
        }
    }
    /// 设置层的关节参与掩码；`None` 表示整副骨架参与
    pub fn set_animation_layer_joint_mask(&mut self, index: usize, mask: Option<Vec<bool>>) {
        if let Some(layer) = self.anim_layers.get_mut(index) {
            layer.joint_mask = mask;
        }
    }

    /// 启动从当前剪辑到 `index` 的 cross-fade。
    ///
    /// `duration` 为 0 时立即切换；fade 期间旧剪辑以 smoothstep 权重淡出。
    pub fn fade_to(&mut self, index: usize, duration: f32) {
        if index >= self.anim_clips.len() {
            return;
        }
        if duration <= 0.0 {
            self.anim_fade = None;
            self.active_anim = index;
            self.anim_time = 0.0;
            return;
        }
        self.anim_fade = Some(AnimFade {
            clip: self.active_anim,
            time: self.anim_time,
            speed: self.anim_speed,
            elapsed: 0.0,
            duration,
        });
        self.active_anim = index;
        self.anim_time = 0.0;
    }
    /// 是否正在 cross-fade
    pub fn is_cross_fading(&self) -> bool {
        self.anim_fade.is_some()
    }

    /// 清除所有蒙皮、morph 和动画数据（用于在 load_gltf_skins 失败后恢复干净状态）
    pub fn clear_skin_data(&mut self) {
        self.skinned_meshes.clear();
        self.mesh_to_skin.clear();
        self.joints.clear();
        self.anim_clips.clear();
        self.skins.clear();
        self.morph_meshes.clear();
        self.node_to_morph_meshes.clear();
        self.anim_layers.clear();
        self.anim_fade = None;
        self.joint_overrides.clear();
        self.active_anim = 0;
        self.anim_time = 0.0;
        self.anim_paused = false;
    }

    // ========================================================================
    // Morph GPU 资源
    // ========================================================================

    /// 重建全局 morph 增量/权重布局并上传。
    ///
    /// 每个目标占用 `2 * vertex_count` 个 `vec3` 槽位（位置块在前、法线块在后），
    /// 与 scenekit 的打包方式一致；`vec3` 在 storage array 中按 16 字节对齐。
    fn rebuild_morph_meshes(&mut self) {
        let mut delta_count = 0u32;
        let mut weight_count = 0u32;
        for mesh in self.morph_meshes.values() {
            delta_count += mesh.targets.len() as u32 * mesh.vertex_count * 2;
            weight_count += mesh.targets.len() as u32;
        }

        let delta_elems = delta_count.max(1);
        let weight_elems = weight_count.max(1);
        let mut bind_group_dirty = false;
        if delta_elems > self.morph_delta_capacity {
            self.morph_delta_capacity = delta_elems.next_power_of_two();
            self.morph_delta_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rgpui-3d.morph_delta_buffer"),
                size: self.morph_delta_capacity as u64 * 16,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            bind_group_dirty = true;
        }
        if weight_elems > self.morph_weight_capacity {
            self.morph_weight_capacity = weight_elems.next_power_of_two();
            self.morph_weight_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rgpui-3d.morph_weight_buffer"),
                size: self.morph_weight_capacity as u64 * 4,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            bind_group_dirty = true;
        }
        if bind_group_dirty {
            self.rebuild_skin_bind_group();
        }

        let mut deltas = vec![[0.0f32; 4]; delta_elems as usize];
        let mut base = 0u32;
        let mut weight_base = 0u32;
        for mesh in self.morph_meshes.values_mut() {
            mesh.base = base;
            mesh.weight_base = weight_base;
            let vertex_count = mesh.vertex_count as usize;
            for target in &mesh.targets {
                for i in 0..vertex_count {
                    let value = target.positions.get(i).copied().unwrap_or([0.0; 3]);
                    deltas[(base + i as u32) as usize] = [value[0], value[1], value[2], 0.0];
                }
                base += mesh.vertex_count;
                for i in 0..vertex_count {
                    let value = target.normals.get(i).copied().unwrap_or([0.0; 3]);
                    deltas[(base + i as u32) as usize] = [value[0], value[1], value[2], 0.0];
                }
                base += mesh.vertex_count;
                weight_base += 1;
            }
        }
        self.queue
            .write_buffer(&self.morph_delta_buffer, 0, bytemuck::cast_slice(&deltas));
        self.upload_morph_weights();
    }

    /// 把 CPU 侧的 morph 权重整体同步到 GPU。
    fn upload_morph_weights(&mut self) {
        if self.morph_weight_capacity == 0 {
            return;
        }
        let mut data = vec![0.0f32; self.morph_weight_capacity as usize];
        for mesh in self.morph_meshes.values() {
            let start = mesh.weight_base as usize;
            for (i, weight) in mesh.weights.iter().enumerate() {
                if let Some(slot) = data.get_mut(start + i) {
                    *slot = *weight;
                }
            }
        }
        self.queue
            .write_buffer(&self.morph_weight_buffer, 0, bytemuck::cast_slice(&data));
    }

    /// 用当前的骨骼/morph 缓冲重建蒙皮绑定组。
    fn rebuild_skin_bind_group(&mut self) {
        let bone_size = self.bone_capacity as u64 * 64;
        let delta_size = self.morph_delta_capacity as u64 * 16;
        let weight_size = self.morph_weight_capacity as u64 * 4;
        self.bone_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rgpui-3d.bone_bg"),
            layout: &self.skin_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.bone_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(bone_size),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.morph_delta_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(delta_size),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.morph_weight_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(weight_size),
                    }),
                },
            ],
        });
    }
    /// 获取 GPU 场景的可变引用
    pub fn gpu_scene_mut(&mut self) -> &mut GpuScene {
        &mut self.gpu_scene
    }
    /// 获取 GPU 场景的引用
    pub fn gpu_scene(&self) -> &GpuScene {
        &self.gpu_scene
    }

    // ========================================================================
    // 渲染
    // ========================================================================

    /// 渲染场景并返回像素结果
    pub fn render(
        &mut self,
        scene: &mut SceneGraph,
        camera: &PerspectiveCamera,
    ) -> Result<RenderResult, ScenixError> {
        scene.update_world_transforms();
        let (draws, _env_draws, _stats) = collect_visible_draws(scene, &self.gpu_scene, camera)?;

        let mut opaque = Vec::new();
        let mut transparent = Vec::new();
        for draw in draws {
            if draw.transparent {
                transparent.push(draw)
            } else {
                opaque.push(draw)
            }
        }
        sort_opaque_front_to_back(&mut opaque);
        sort_transparent_back_to_front(&mut transparent);

        let draw_count = opaque.len() + transparent.len();
        self.ensure_capacity(draw_count);

        let vp = camera.view_projection();
        let pos = camera.position;
        self.queue.write_buffer(
            &self.frame_buffer,
            0,
            bytemuck::bytes_of(&FrameUniform {
                view_projection: mat4_to_array(vp),
                camera_position_frame: [pos.x, pos.y, pos.z, self.frame_index as f32],
                resolution: [
                    self.width as f32,
                    self.height as f32,
                    1.0 / self.width.max(1) as f32,
                    1.0 / self.height.max(1) as f32,
                ],
            }),
        );

        let all_draws: Vec<_> = opaque.iter().chain(transparent.iter()).collect();
        let mut draw_is_skinned = Vec::with_capacity(all_draws.len());
        for draw in &all_draws {
            draw_is_skinned.push(self.skinned_meshes.contains_key(&draw.mesh_id));
        }

        // 预计算每个 skin 的骨骼偏移量
        let skin_offsets: Vec<u32> = self
            .skins
            .iter()
            .scan(0u32, |acc, s| {
                let off = *acc;
                *acc += s.joint_node_indices.len() as u32;
                Some(off)
            })
            .collect();

        for (i, draw) in all_draws.iter().enumerate() {
            let object_off = i as u64 * self.object_stride;
            let material_off = i as u64 * self.material_stride;

            let mut flags = 0u32;
            let mut bone_offset = 0u32;
            // 蒙皮网格遵循 glTF 规范：忽略节点自身变换，由关节世界矩阵承担；
            // morph-only 网格（没有 JOINTS/WEIGHTS）仍需节点世界矩阵。
            let world = if let Some(&skin_index) = self.mesh_to_skin.get(&draw.mesh_id) {
                flags |= SKIN_FLAG_JOINTS;
                bone_offset = skin_offsets.get(skin_index).copied().unwrap_or(0);
                mat4_to_array(mat4_identity())
            } else if let Some(&node_idx) = self.mesh_to_node.get(&draw.mesh_id) {
                if node_idx < self.cached_global_mats.len() {
                    mat4_to_array(self.cached_global_mats[node_idx])
                } else {
                    mat4_to_array(draw.world_matrix)
                }
            } else {
                mat4_to_array(draw.world_matrix)
            };

            let (morph_base, weight_base, vertex_count, morph_count) =
                match self.morph_meshes.get(&draw.mesh_id) {
                    Some(morph) => {
                        flags |= SKIN_FLAG_MORPH;
                        (
                            morph.base,
                            morph.weight_base,
                            morph.vertex_count,
                            morph.targets.len() as u32,
                        )
                    }
                    None => (0, 0, 0, 0),
                };

            self.queue.write_buffer(
                &self.object_buffer,
                object_off,
                bytemuck::bytes_of(&ObjectUniform {
                    world,
                    bone_offset,
                    morph_base,
                    weight_base,
                    flags,
                    vertex_count,
                    morph_count,
                    _pad: [0u32; 2],
                }),
            );

            if let Some(mat) = self.gpu_scene.material(draw.material_id) {
                self.queue.write_buffer(
                    &self.material_buffer,
                    material_off,
                    bytemuck::bytes_of(&MaterialUniform::from_material(mat)),
                );
            }
        }

        let color_view = self
            .color_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = self
            .depth_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let resolve_view = self
            .resolve_texture
            .as_ref()
            .map(|t| t.create_view(&wgpu::TextureViewDescriptor::default()));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rgpui-3d.encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rgpui-3d.render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    depth_slice: None,
                    resolve_target: resolve_view.as_ref(),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.clear_color[0] as f64,
                            g: self.clear_color[1] as f64,
                            b: self.clear_color[2] as f64,
                            a: self.clear_color[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if draw_count > 0 {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.frame_bind_group, &[]);

                for (i, draw) in all_draws.iter().enumerate() {
                    if draw_is_skinned[i] {
                        continue;
                    }
                    let Some(mesh) = self.gpu_scene.mesh(draw.mesh_id) else {
                        continue;
                    };
                    if self.gpu_scene.material(draw.material_id).is_none() {
                        continue;
                    }

                    pass.set_bind_group(
                        1,
                        &self.object_bind_group,
                        &[(i as u64 * self.object_stride) as u32],
                    );
                    pass.set_bind_group(
                        2,
                        &self.material_bind_group,
                        &[(i as u64 * self.material_stride) as u32],
                    );

                    let tex_bg = self
                        .gpu_scene
                        .material(draw.material_id)
                        .and_then(|m| match m {
                            RendererMaterial::Pbr(pbr) => pbr.albedo_texture,
                            _ => None,
                        })
                        .and_then(|tex_id| self.textures.get(&tex_id))
                        .map(|entry| &entry.bind_group)
                        .unwrap_or(&self.white_bind_group);
                    pass.set_bind_group(3, tex_bg, &[]);

                    pass.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
                    pass.set_index_buffer(
                        mesh.index_buffer().slice(..),
                        mesh.packed().index_format.to_wgpu(),
                    );
                    pass.draw_indexed(0..mesh.packed().index_count, 0, 0..1);
                }

                let has_skinned = draw_is_skinned.iter().any(|&s| s);
                if has_skinned {
                    pass.set_pipeline(&self.skin_pipeline);
                    pass.set_bind_group(4, &self.bone_bg, &[]);

                    for (i, draw) in all_draws.iter().enumerate() {
                        if !draw_is_skinned[i] {
                            continue;
                        }
                        let Some(skinned) = self.skinned_meshes.get(&draw.mesh_id) else {
                            continue;
                        };
                        if self.gpu_scene.material(draw.material_id).is_none() {
                            continue;
                        }

                        pass.set_bind_group(
                            1,
                            &self.object_bind_group,
                            &[(i as u64 * self.object_stride) as u32],
                        );
                        pass.set_bind_group(
                            2,
                            &self.material_bind_group,
                            &[(i as u64 * self.material_stride) as u32],
                        );

                        let tex_bg = self
                            .gpu_scene
                            .material(draw.material_id)
                            .and_then(|m| match m {
                                RendererMaterial::Pbr(pbr) => pbr.albedo_texture,
                                _ => None,
                            })
                            .and_then(|tex_id| self.textures.get(&tex_id))
                            .map(|entry| &entry.bind_group)
                            .unwrap_or(&self.white_bind_group);
                        pass.set_bind_group(3, tex_bg, &[]);

                        pass.set_vertex_buffer(0, skinned.vertex_buffer.slice(..));
                        pass.set_index_buffer(skinned.index_buffer.slice(..), skinned.index_format);
                        pass.draw_indexed(0..skinned.index_count, 0, 0..1);
                    }
                }
            }
        }

        let bpp = 4u32;
        let row_unpadded = self.width * bpp;
        let row_padded = align_to_256(row_unpadded);

        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgpui-3d.readback"),
            size: row_padded as u64 * self.height as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let readback_texture = self.resolve_texture.as_ref().unwrap_or(&self.color_texture);
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: readback_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_padded),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|_| ScenixError::Gpu(GpuError::Upload))?;
        rx.recv()
            .map_err(|_| ScenixError::Gpu(GpuError::Upload))?
            .map_err(|_| ScenixError::Gpu(GpuError::Upload))?;

        let mapped = slice
            .get_mapped_range()
            .map_err(|_| ScenixError::Gpu(GpuError::Unsupported))?;
        let mut pixels = Vec::with_capacity((self.width * self.height * bpp) as usize);
        for row in 0..self.height as usize {
            let off = row * row_padded as usize;
            pixels.extend_from_slice(&mapped[off..off + row_unpadded as usize]);
        }
        drop(mapped);
        readback.unmap();

        self.frame_index += 1;

        Ok(RenderResult {
            data: pixels,
            width: self.width,
            height: self.height,
        })
    }

    /// 确保 uniform 缓冲区能容纳 `needed` 个 draw call
    fn ensure_capacity(&mut self, needed: usize) {
        if needed <= self.draw_capacity {
            return;
        }
        self.draw_capacity = needed.next_power_of_two();
        let obj_size = self.object_stride * self.draw_capacity as u64;
        let mat_size = self.material_stride * self.draw_capacity as u64;

        self.object_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgpui-3d.object_buffer"),
            size: obj_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.material_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgpui-3d.material_buffer"),
            size: mat_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.object_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rgpui-3d.object_bind_group"),
            layout: &self.object_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &self.object_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<ObjectUniform>() as u64),
                }),
            }],
        });
        self.material_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rgpui-3d.material_bind_group"),
            layout: &self.material_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &self.material_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<MaterialUniform>() as u64),
                }),
            }],
        });
    }
}

unsafe impl Send for Scenix3D {}
unsafe impl Sync for Scenix3D {}

/// 读取 glTF mesh 的 `extras.targetNames`（morph target 名称列表）。
///
/// VRM 等资产靠这个字段给表情命名；缺失或格式不对时返回空列表，
/// 调用方会退化成 `morph-<索引>`。
fn morph_target_names(mesh: &gltf::Mesh<'_>) -> Vec<String> {
    let Some(extras) = mesh.extras() else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(extras.get()) else {
        return Vec::new();
    };
    value
        .get("targetNames")
        .and_then(|names| names.as_array())
        .map(|names| {
            names
                .iter()
                .map(|name| name.as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}
