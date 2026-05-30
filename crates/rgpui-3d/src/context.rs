use crate::math::*;
use crate::shader::{SHADER_SRC, SKIN_SHADER_SRC};
use crate::types::*;
use bytemuck;
use rgpui::RenderImage;
use scenix::{
    Geometry, GpuError, GpuScene, MaterialId, MeshId, PackedVertex, PerspectiveCamera,
    RendererLight, RendererMaterial, SceneGraph, ScenixError, TextureId, collect_visible_draws,
    sort_opaque_front_to_back, sort_transparent_back_to_front,
};
use std::collections::HashMap;

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
    pub fn into_render_image(self) -> RenderImage {
        let img_buffer = image::RgbaImage::from_raw(self.width, self.height, self.data)
            .expect("无效的像素数据尺寸");
        let frame = image::Frame::new(img_buffer);
        RenderImage::new(smallvec::smallvec![frame])
    }
}

/// 3D 场景渲染上下文
///
/// 管理 wgpu 设备、离屏渲染目标、scenix GPU 场景资源和渲染管线。
///
/// # 生命周期
/// 1. 使用 `Scenix3D::new(width, height)` 或 `Scenix3D::new_shared(device, queue, w, h)` 创建
/// 2. 注册网格和材质：`register_mesh()`, `register_pbr_material()` 等
/// 3. 构建 scenix `SceneGraph`，添加 `SceneNode`
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

    // 配置
    width: u32,
    height: u32,
    color_format: wgpu::TextureFormat,
    clear_color: [f32; 4],

    // 帧计数器
    frame_index: u64,

    // 蒙皮/骨骼动画
    skin_pipeline: wgpu::RenderPipeline,
    skin_layout: wgpu::BindGroupLayout,
    skinned_meshes: HashMap<MeshId, GpuSkinnedMesh>,
    skins: Vec<SkinData>,
    mesh_to_skin: HashMap<MeshId, usize>,
    joints: Vec<JointNode>,
    anim_clips: Vec<AnimClip>,
    active_anim: usize,
    anim_time: f32,
    anim_speed: f32,
    anim_paused: bool,
    bone_buffer: wgpu::Buffer,
    bone_capacity: u32,
    skin_info_buffer: wgpu::Buffer,
    skin_info_bg: wgpu::BindGroup,

    cached_local_trs: Vec<([f32; 3], [f32; 4], [f32; 3])>,
    cached_global_mats: Vec<scenix::Mat4>,
    cached_bone_data: Vec<f32>,

    joint_overrides: Vec<Option<scenix::Quat>>,
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
            );
        }

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
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

        Self::new_inner(device, queue, width, height)
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
        Self::new_inner(device, queue, width, height)
    }

    /// 内部初始化逻辑，从已有的 device/queue 创建完整的 3D 上下文
    fn new_inner(
        device: wgpu::Device,
        queue: wgpu::Queue,
        width: u32,
        height: u32,
    ) -> Result<Self, ScenixError> {
        let color_format = wgpu::TextureFormat::Bgra8UnormSrgb;

        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rgpui-3d.color"),
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
        });

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rgpui-3d.depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
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
                buffers: &[PackedVertex::layout()],
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
            multisample: wgpu::MultisampleState::default(),
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

        // 蒙皮管线绑定组布局
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
                        ty: wgpu::BufferBindingType::Uniform,
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
                buffers: &[SkinnedVertex::layout()],
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
            multisample: wgpu::MultisampleState::default(),
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

        let skin_info_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgpui-3d.skin_info_buffer"),
            size: std::mem::size_of::<SkinInfoUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &skin_info_buffer,
            0,
            bytemuck::bytes_of(&SkinInfoUniform {
                first_joint: 0,
                _pad: [0u32; 3],
            }),
        );

        let skin_info_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rgpui-3d.skin_info_bg"),
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
                        buffer: &skin_info_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<SkinInfoUniform>() as u64),
                    }),
                },
            ],
        });

        Ok(Self {
            device,
            queue,
            color_texture,
            depth_texture,
            _frame_layout: frame_layout,
            object_layout,
            material_layout,
            texture_layout,
            pipeline,
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
            skin_layout,
            skinned_meshes: HashMap::new(),
            skins: Vec::new(),
            mesh_to_skin: HashMap::new(),
            joints: Vec::new(),
            anim_clips: Vec::new(),
            active_anim: 0,
            anim_time: 0.0,
            anim_speed: 1.0,
            anim_paused: false,
            bone_buffer,
            bone_capacity,
            skin_info_buffer,
            skin_info_bg,
            cached_local_trs: Vec::new(),
            cached_global_mats: Vec::new(),
            cached_bone_data: Vec::new(),
            joint_overrides: Vec::new(),
        })
    }

    /// 调整渲染尺寸
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.color_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rgpui-3d.color"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rgpui-3d.depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
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
        material: &scenix::PbrMaterial,
    ) -> Result<(), ScenixError> {
        self.gpu_scene.register_pbr_material(id, material)?;
        Ok(())
    }

    /// 注册无光照材质
    pub fn register_unlit_material(
        &mut self,
        id: MaterialId,
        material: &scenix::UnlitMaterial,
    ) -> Result<(), ScenixError> {
        self.gpu_scene.register_unlit_material(id, material)?;
        Ok(())
    }

    /// 注册 Lambert 材质
    pub fn register_lambert_material(
        &mut self,
        id: MaterialId,
        material: &scenix::LambertMaterial,
    ) -> Result<(), ScenixError> {
        self.gpu_scene.register_lambert_material(id, material)?;
        Ok(())
    }

    /// 注册卡通着色材质
    pub fn register_toon_material(
        &mut self,
        id: MaterialId,
        material: &scenix::ToonMaterial,
    ) -> Result<(), ScenixError> {
        self.gpu_scene.register_toon_material(id, material)?;
        Ok(())
    }

    /// 注册光照
    pub fn register_light(
        &mut self,
        id: scenix::LightId,
        light: RendererLight,
    ) -> Result<(), ScenixError> {
        self.gpu_scene.register_light(id, light)?;
        Ok(())
    }

    /// 上传纹理到 GPU 并注册
    pub fn register_texture(
        &mut self,
        id: TextureId,
        texture: &scenix::Texture2D,
        sampler: scenix::Sampler,
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
    pub fn register_gltf_asset(&mut self, asset: &scenix::GltfAsset) -> Result<(), ScenixError> {
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
        asset: &scenix::GltfAsset,
    ) -> Result<(), ScenixError> {
        let (document, buffer_data, _image_data) =
            gltf::import(path).map_err(|_| ScenixError::Load(scenix::LoadError::Io))?;

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
                }
            })
            .collect();

        self.skins.clear();
        let mut mesh_to_skin: HashMap<MeshId, usize> = HashMap::new();

        // 计算每个 glTF mesh 的起始 MeshId（匹配 GltfLoader 的 1-based 顺序分配）
        let mut mesh_prim_start: Vec<u64> = Vec::new();
        let mut next_id = 1u64;
        for gltf_mesh in document.meshes() {
            mesh_prim_start.push(next_id);
            next_id += gltf_mesh.primitives().count() as u64;
        }

        for node in document.nodes() {
            let (Some(mesh_gltf), Some(skin)) = (node.mesh(), node.skin()) else {
                continue;
            };
            let skin_index = self.skins.len();

            let ibm: Vec<scenix::Mat4> = {
                let reader = skin.reader(|buffer| Some(&buffer_data[buffer.index()].0));
                let Some(iter) = reader.read_inverse_bind_matrices() else {
                    continue;
                };
                iter.map(|cols| {
                    scenix::Mat4::from_cols(
                        scenix::Vec4::new(cols[0][0], cols[0][1], cols[0][2], cols[0][3]),
                        scenix::Vec4::new(cols[1][0], cols[1][1], cols[1][2], cols[1][3]),
                        scenix::Vec4::new(cols[2][0], cols[2][1], cols[2][2], cols[2][3]),
                        scenix::Vec4::new(cols[3][0], cols[3][1], cols[3][2], cols[3][3]),
                    )
                })
                .collect()
            };

            let joint_node_indices: Vec<usize> = skin.joints().map(|j| j.index()).collect();

            self.skins.push(SkinData {
                inverse_bind_matrices: ibm,
                joint_node_indices,
            });

            let mesh_idx = mesh_gltf.index();
            let prim_start = mesh_prim_start[mesh_idx];
            let gltf_mesh = document.meshes().nth(mesh_idx).unwrap();

            // 为每个 primitive 创建独立的 skinned mesh
            for (prim_i, primitive) in gltf_mesh.primitives().enumerate() {
                let prim_mesh_id = MeshId::new(prim_start + prim_i as u64);
                if !asset.meshes.contains_key(&prim_mesh_id) {
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

                let has_skin = !joint_indices.is_empty() && !joint_weights.is_empty();
                let indices: Vec<u32> = reader
                    .read_indices()
                    .map(|iter| iter.into_u32().collect())
                    .unwrap_or_default();

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
                        ([0; 4], [1.0, 0.0, 0.0, 0.0])
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
                mesh_to_skin.insert(prim_mesh_id, skin_index);
            }
        }

        self.mesh_to_skin = mesh_to_skin;
        self.joints = joints;

        use std::collections::HashMap as HashMap2;
        self.anim_clips.clear();
        for anim in document.animations() {
            let mut sampler_map: HashMap2<usize, AnimSampler> = HashMap2::new();
            let samplers_in_order: Vec<usize> = anim.samplers().map(|s| s.index()).collect();
            for channel in anim.channels() {
                let sampler_idx = channel.sampler().index();
                if sampler_map.contains_key(&sampler_idx) {
                    continue;
                }
                let reader = channel.reader(|buffer| Some(&buffer_data[buffer.index()].0));
                let times: Vec<f32> = reader
                    .read_inputs()
                    .map(|i| i.collect())
                    .unwrap_or_default();
                let outputs: Vec<[f32; 4]> = reader
                    .read_outputs()
                    .map(|output| match output {
                        gltf::animation::util::ReadOutputs::Translations(iter) => {
                            iter.map(|v| [v[0], v[1], v[2], 0.0]).collect()
                        }
                        gltf::animation::util::ReadOutputs::Rotations(iter) => {
                            iter.into_f32().map(|v| [v[0], v[1], v[2], v[3]]).collect()
                        }
                        gltf::animation::util::ReadOutputs::Scales(iter) => {
                            iter.map(|v| [v[0], v[1], v[2], 0.0]).collect()
                        }
                        _ => Vec::new(),
                    })
                    .unwrap_or_default();
                sampler_map.insert(sampler_idx, AnimSampler { times, outputs });
            }
            let samplers: Vec<AnimSampler> = samplers_in_order
                .iter()
                .filter_map(|i| sampler_map.remove(i))
                .collect();
            let channels: Vec<AnimChannel> = anim
                .channels()
                .map(|c| {
                    use gltf::animation::Property;
                    let target = match c.target().property() {
                        Property::Translation => 0,
                        Property::Rotation => 1,
                        Property::Scale => 2,
                        Property::MorphTargetWeights => 3,
                    };
                    AnimChannel {
                        node: c.target().node().index(),
                        target,
                        sampler: c.sampler().index(),
                    }
                })
                .collect();
            self.anim_clips.push(AnimClip {
                _name: anim.name().unwrap_or("").to_string(),
                samplers,
                channels,
            });
        }

        Ok(())
    }

    // ========================================================================
    // 动画控制
    // ========================================================================

    /// 推进动画时间，计算骨骼矩阵并上传到 GPU
    pub fn advance_animation(&mut self, dt: f32) {
        if self.anim_clips.is_empty() || self.joints.is_empty() || self.anim_paused {
            return;
        }

        self.anim_time += dt * self.anim_speed;
        let clip = &self.anim_clips[self.active_anim];
        let num_joints = self.joints.len();

        self.cached_local_trs.clear();
        self.cached_local_trs
            .extend(self.joints.iter().map(|j| j.default_trs));
        self.cached_local_trs
            .resize(num_joints, ([0.0; 3], [1.0, 0.0, 0.0, 0.0], [1.0; 3]));

        for channel in &clip.channels {
            if channel.node >= num_joints || channel.sampler >= clip.samplers.len() {
                continue;
            }
            let sampler = &clip.samplers[channel.sampler];
            if sampler.times.is_empty() || sampler.outputs.is_empty() {
                continue;
            }

            let t = self.anim_time;
            let last = sampler.times.len() - 1;
            let sample = if t <= sampler.times[0] {
                sampler.outputs[0]
            } else if t >= sampler.times[last] {
                let loop_t = t - sampler.times[last];
                let dur = sampler.times[last] - sampler.times[0];
                if dur > 0.0 {
                    let wrapped = loop_t % dur;
                    let (low, high, frac) =
                        find_lerp_factors(&sampler.times, sampler.times[0] + wrapped);
                    lerp_value(
                        sampler.outputs[low],
                        sampler.outputs[high],
                        frac,
                        channel.target,
                    )
                } else {
                    sampler.outputs[last]
                }
            } else {
                let (low, high, frac) = find_lerp_factors(&sampler.times, t);
                lerp_value(
                    sampler.outputs[low],
                    sampler.outputs[high],
                    frac,
                    channel.target,
                )
            };

            match channel.target {
                0 => self.cached_local_trs[channel.node].0 = [sample[0], sample[1], sample[2]],
                1 => self.cached_local_trs[channel.node].1 = sample,
                2 => self.cached_local_trs[channel.node].2 = [sample[0], sample[1], sample[2]],
                _ => {}
            }
        }

        for (i, override_q) in self.joint_overrides.iter().enumerate() {
            if i >= num_joints {
                break;
            }
            if let Some(q) = override_q {
                self.cached_local_trs[i].1 = [q.x, q.y, q.z, q.w];
            }
        }

        self.cached_global_mats.clear();
        self.cached_global_mats
            .resize(num_joints, scenix::Mat4::IDENTITY);
        let local_mats: Vec<scenix::Mat4> = (0..num_joints)
            .map(|i| {
                trs_to_mat4(
                    self.cached_local_trs[i].0,
                    self.cached_local_trs[i].1,
                    self.cached_local_trs[i].2,
                )
            })
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
            if self.joints[i].parent.is_none() || self.joints[i].parent.unwrap() >= num_joints {
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
            self.skin_info_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rgpui-3d.skin_info_bg"),
                layout: &self.skin_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.bone_buffer,
                            offset: 0,
                            size: wgpu::BufferSize::new(self.bone_capacity as u64 * 64),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.skin_info_buffer,
                            offset: 0,
                            size: wgpu::BufferSize::new(
                                std::mem::size_of::<SkinInfoUniform>() as u64
                            ),
                        }),
                    },
                ],
            });
        }

        self.cached_bone_data.clear();
        self.cached_bone_data.reserve(total_bones * 16);
        let mut first_joint = 0u32;
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
            self.queue.write_buffer(
                &self.skin_info_buffer,
                0,
                bytemuck::bytes_of(&SkinInfoUniform {
                    first_joint,
                    _pad: [0u32; 3],
                }),
            );
            first_joint += skin.joint_node_indices.len() as u32;
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
    pub fn joint_world_matrix(&self, index: usize) -> Option<scenix::Mat4> {
        if index < self.cached_global_mats.len() {
            Some(self.cached_global_mats[index])
        } else {
            None
        }
    }
    /// 获取关节的世界位置
    pub fn joint_world_position(&self, index: usize) -> Option<scenix::Vec3> {
        self.joint_world_matrix(index)
            .map(|m| scenix::Vec3::new(m.cols[3].x, m.cols[3].y, m.cols[3].z))
    }
    /// 设置关节的旋转覆盖
    pub fn set_joint_rotation_override(&mut self, index: usize, rotation: scenix::Quat) {
        if index < self.joints.len() {
            if self.joint_overrides.len() <= index {
                self.joint_overrides.resize(index + 1, None)
            }
            self.joint_overrides[index] = Some(rotation);
        }
    }
    /// 清除所有关节覆盖
    pub fn clear_joint_overrides(&mut self) {
        self.joint_overrides.clear()
    }
    /// 获取当前被覆盖的关节数量
    pub fn joint_override_count(&self) -> usize {
        self.joint_overrides.iter().filter(|o| o.is_some()).count()
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
        let (draws, _stats) = collect_visible_draws(scene, &self.gpu_scene, camera)?;

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

        for (i, draw) in all_draws.iter().enumerate() {
            let object_off = i as u64 * self.object_stride;
            let material_off = i as u64 * self.material_stride;
            let is_skinned = draw_is_skinned[i];

            self.queue.write_buffer(
                &self.object_buffer,
                object_off,
                bytemuck::bytes_of(&ObjectUniform {
                    world: if is_skinned {
                        mat4_to_array(mat4_identity())
                    } else {
                        mat4_to_array(draw.world_matrix)
                    },
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
                    resolve_target: None,
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
                    pass.set_bind_group(4, &self.skin_info_bg, &[]);

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

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.color_texture,
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

        let mapped = slice.get_mapped_range();
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
