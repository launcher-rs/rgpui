use bytemuck::{Pod, Zeroable};

// ============================================================================
// GPU Uniform 类型
// ============================================================================

/// 皮肤管线标志：该绘制需要骨骼矩阵混合。
pub(crate) const SKIN_FLAG_JOINTS: u32 = 1;
/// 皮肤管线标志：该绘制需要 morph 增量混合。
pub(crate) const SKIN_FLAG_MORPH: u32 = 2;

/// 每帧 uniform 数据，对应 WGSL FrameUniform
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct FrameUniform {
    pub(crate) view_projection: [[f32; 4]; 4],
    pub(crate) camera_position_frame: [f32; 4],
    pub(crate) resolution: [f32; 4],
}

/// 每个对象的 uniform 数据，对应 WGSL ObjectUniform。
///
/// `bone_offset` 是该 mesh 在全局骨骼数组中的起始索引，`morph_base` /
/// `weight_base` 分别是 morph 增量与权重在全局缓冲中的起始索引；
/// `flags` 由 [`SKIN_FLAG_JOINTS`] / [`SKIN_FLAG_MORPH`] 组合，决定皮肤
/// 着色器是否执行关节混合与 morph 混合。
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct ObjectUniform {
    pub(crate) world: [[f32; 4]; 4],
    pub(crate) bone_offset: u32,
    pub(crate) morph_base: u32,
    pub(crate) weight_base: u32,
    pub(crate) flags: u32,
    pub(crate) vertex_count: u32,
    pub(crate) morph_count: u32,
    pub(crate) _pad: [u32; 2],
}

/// 材质 uniform 数据，对应 WGSL MaterialUniform
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct MaterialUniform {
    pub(crate) base_color: [f32; 4],
    pub(crate) emissive_cutoff: [f32; 4],
    pub(crate) params: [f32; 4],
}

impl MaterialUniform {
    pub(crate) fn from_material(material: &scenekit::RendererMaterial) -> Self {
        let color = material.preview_color();
        let has_texture = match material {
            scenekit::RendererMaterial::Pbr(pbr) => pbr.albedo_texture.is_some(),
            _ => false,
        };
        Self {
            base_color: [color.r, color.g, color.b, color.a],
            emissive_cutoff: [0.0, 0.0, 0.0, 0.0],
            params: [
                0.0,
                1.0,
                material.preview_shader_code(),
                if has_texture { 1.0 } else { 0.0 },
            ],
        }
    }
}

// ============================================================================
// 蒙皮类型与动画数据结构
// ============================================================================

/// 蒙皮顶点（用于骨骼动画的顶点格式）
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct SkinnedVertex {
    pub(crate) position: [f32; 3],
    pub(crate) normal: [f32; 3],
    pub(crate) uv: [f32; 2],
    pub(crate) color: [f32; 4],
    pub(crate) tangent: [f32; 4],
    pub(crate) joint_indices: [u32; 4],
    pub(crate) joint_weights: [f32; 4],
}

impl SkinnedVertex {
    pub(crate) fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SkinnedVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: std::mem::size_of::<[f32; 3]>() as u64,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: (std::mem::size_of::<[f32; 3]>() * 2) as u64,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: (std::mem::size_of::<[f32; 3]>() * 2 + std::mem::size_of::<[f32; 2]>())
                        as u64,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: (std::mem::size_of::<[f32; 3]>() * 2
                        + std::mem::size_of::<[f32; 2]>()
                        + std::mem::size_of::<[f32; 4]>()) as u64,
                    shader_location: 4,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32x4,
                    offset: (std::mem::size_of::<[f32; 3]>() * 2
                        + std::mem::size_of::<[f32; 2]>()
                        + std::mem::size_of::<[f32; 4]>() * 2) as u64,
                    shader_location: 5,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: (std::mem::size_of::<[f32; 3]>() * 2
                        + std::mem::size_of::<[f32; 2]>()
                        + std::mem::size_of::<[f32; 4]>() * 2
                        + std::mem::size_of::<[u32; 4]>()) as u64,
                    shader_location: 6,
                },
            ],
        }
    }
}

/// CPU 端皮肤数据
pub(crate) struct SkinData {
    pub(crate) inverse_bind_matrices: Vec<scenekit::Mat4>,
    pub(crate) joint_node_indices: Vec<usize>,
}

/// GPU 蒙皮网格资源
pub(crate) struct GpuSkinnedMesh {
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) index_buffer: wgpu::Buffer,
    pub(crate) index_count: u32,
    pub(crate) index_format: wgpu::IndexFormat,
}

/// 动画采样曲线（原始 glTF 数据）
///
/// `outputs` 服务于平移/旋转/缩放通道；`morph_weights` 是 morph 权重通道的
/// 展平输出（每个关键帧 `morph_weight_count` 个 `f32`），两者互斥。
pub(crate) struct AnimSampler {
    pub(crate) times: Vec<f32>,
    pub(crate) outputs: Vec<[f32; 4]>,
    pub(crate) morph_weights: Vec<f32>,
    pub(crate) morph_weight_count: usize,
}

/// 动画通道
pub(crate) struct AnimChannel {
    pub(crate) node: usize,
    /// 0=平移, 1=旋转, 2=缩放, 3=morph 权重
    pub(crate) target: u32,
    /// 该剪辑内 `samplers` 的局部索引（不是 glTF 文档全局索引）
    pub(crate) sampler: usize,
}

/// 动画剪辑
pub(crate) struct AnimClip {
    pub(crate) _name: String,
    pub(crate) samplers: Vec<AnimSampler>,
    pub(crate) channels: Vec<AnimChannel>,
}

/// 蒙皮关节节点描述
pub(crate) struct JointNode {
    pub(crate) parent: Option<usize>,
    pub(crate) default_trs: ([f32; 3], [f32; 4], [f32; 3]),
    pub(crate) name: String,
}

/// 单个关节的 TRS 覆盖（仅写入非 `None` 的分量）。
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct JointOverride {
    pub(crate) translation: Option<[f32; 3]>,
    pub(crate) rotation: Option<[f32; 4]>,
    pub(crate) scale: Option<[f32; 3]>,
}

impl JointOverride {
    /// 是否没有任何分量需要覆盖。
    pub(crate) fn is_empty(&self) -> bool {
        self.translation.is_none() && self.rotation.is_none() && self.scale.is_none()
    }
}

/// 额外动画层：在主剪辑之上按权重混合另一段剪辑。
#[derive(Clone, Debug)]
pub(crate) struct AnimLayer {
    pub(crate) clip: usize,
    pub(crate) time: f32,
    pub(crate) speed: f32,
    pub(crate) weight: f32,
    pub(crate) additive: bool,
    /// 逐关节开关；`None` 表示整副骨架都参与混合。
    pub(crate) joint_mask: Option<Vec<bool>>,
}

impl AnimLayer {
    pub(crate) fn new(clip: usize, weight: f32) -> Self {
        Self {
            clip,
            time: 0.0,
            speed: 1.0,
            weight: weight.clamp(0.0, 1.0),
            additive: false,
            joint_mask: None,
        }
    }
}

/// 正在进行的 cross-fade：从旧剪辑淡入新剪辑。
pub(crate) struct AnimFade {
    pub(crate) clip: usize,
    pub(crate) time: f32,
    pub(crate) speed: f32,
    pub(crate) elapsed: f32,
    pub(crate) duration: f32,
}

// ============================================================================
// Morph 数据
// ============================================================================

/// 单个 morph target 的 CPU 增量数据。
///
/// glTF 的 POSITION 存增量，NORMAL 存目标绝对法线；导入时已经换算成
/// 与基法线的差值，因此这里两类数据的语义都是「增量」。
pub(crate) struct MorphTargetData {
    pub(crate) name: String,
    pub(crate) positions: Vec<[f32; 3]>,
    pub(crate) normals: Vec<[f32; 3]>,
}

/// 单个网格的 morph 状态（CPU 权重 + GPU 存储槽位）。
pub(crate) struct MorphMesh {
    /// 全局 morph 增量数组中的元素起点。
    pub(crate) base: u32,
    /// 全局 morph 权重数组中的起点。
    pub(crate) weight_base: u32,
    pub(crate) vertex_count: u32,
    pub(crate) targets: Vec<MorphTargetData>,
    /// 当前生效权重。
    pub(crate) weights: Vec<f32>,
    /// 应用层手动锁定值；`Some` 时动画通道不会覆盖该目标。
    pub(crate) overrides: Vec<Option<f32>>,
}

impl MorphMesh {
    /// 按名称查找 morph target（大小写不敏感，允许重名时取第一个）。
    pub(crate) fn target_index(&self, name: &str) -> Option<usize> {
        self.targets
            .iter()
            .position(|target| target.name.eq_ignore_ascii_case(name))
    }

    /// 返回目标名称列表。
    pub(crate) fn target_names(&self) -> Vec<String> {
        self.targets
            .iter()
            .map(|target| target.name.clone())
            .collect()
    }
}

/// 已上传的 GPU 纹理，包含绑定点
pub(crate) struct TextureEntry {
    pub(crate) _texture: wgpu::Texture,
    pub(crate) bind_group: wgpu::BindGroup,
}
