use bytemuck::{Pod, Zeroable};


// ============================================================================
// GPU Uniform 类型
// ============================================================================

/// 每帧 uniform 数据，对应 WGSL FrameUniform
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct FrameUniform {
    pub(crate) view_projection: [[f32; 4]; 4],
    pub(crate) camera_position_frame: [f32; 4],
    pub(crate) resolution: [f32; 4],
}

/// 每个对象的 uniform 数据，对应 WGSL ObjectUniform
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct ObjectUniform {
    pub(crate) world: [[f32; 4]; 4],
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
    pub(crate) fn from_material(material: &scenix::RendererMaterial) -> Self {
        let color = material.preview_color();
        let has_texture = match material {
            scenix::RendererMaterial::Pbr(pbr) => pbr.albedo_texture.is_some(),
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

/// 皮肤信息 uniform，对应 WGSL SkinInfo
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct SkinInfoUniform {
    pub(crate) first_joint: u32,
    pub(crate) _pad: [u32; 3],
}

/// CPU 端皮肤数据
pub(crate) struct SkinData {
    pub(crate) inverse_bind_matrices: Vec<scenix::Mat4>,
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
pub(crate) struct AnimSampler {
    pub(crate) times: Vec<f32>,
    pub(crate) outputs: Vec<[f32; 4]>,
}

/// 动画通道
pub(crate) struct AnimChannel {
    pub(crate) node: usize,
    /// 0=平移, 1=旋转, 2=缩放
    pub(crate) target: u32,
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
}

/// 已上传的 GPU 纹理，包含绑定点
pub(crate) struct TextureEntry {
    pub(crate) _texture: wgpu::Texture,
    pub(crate) bind_group: wgpu::BindGroup,
}
