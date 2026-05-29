//! rgpui-scenix: 将 scenix 3D 渲染引擎集成到 rgpui 中。
//!
//! 本 crate 提供：
//! - [`Scenix3D`]：管理 wgpu 设备、离屏渲染目标和 scenix GPU 场景资源
//! - [`RenderResult`]：渲染结果，包含 BGRA 像素数据和尺寸
//! - 与 rgpui 的 `RenderImage` 转换支持
//!
//! # 示例
//! ```ignore
//! use rgpui_scenix::Scenix3D;
//!
//! let mut ctx = Scenix3D::new(800, 600).await?;
//! let result = ctx.render(&mut scene, &camera)?;
//! let image = result.into_render_image();
//! ```

/// 重新导出 scenix 公开类型，方便用户使用
pub use scenix;

use bytemuck::{Pod, Zeroable};
use rgpui::RenderImage;
use scenix::{
    Geometry, GpuError, GpuScene, MaterialId, MeshId, PackedVertex, PerspectiveCamera,
    RendererLight, RendererMaterial, SceneGraph, ScenixError, TextureId, collect_visible_draws,
    sort_opaque_front_to_back, sort_transparent_back_to_front,
};
use std::collections::HashMap;

use smallvec::smallvec;

// ============================================================================
// WGSL 着色器
// ============================================================================

/// 嵌入的 WGSL 着色器源码（支持 UV 和漫反射纹理）
const SHADER_SRC: &str = r#"
struct FrameUniform {
    view_projection: mat4x4<f32>,
    camera_position_frame: vec4<f32>,
    resolution: vec4<f32>,
};

struct ObjectUniform {
    world: mat4x4<f32>,
};

struct MaterialUniform {
    base_color: vec4<f32>,
    emissive_cutoff: vec4<f32>,
    params: vec4<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@group(0) @binding(0) var<uniform> frame: FrameUniform;
@group(1) @binding(0) var<uniform> object: ObjectUniform;
@group(2) @binding(0) var<uniform> material: MaterialUniform;
@group(3) @binding(0) var albedo_texture: texture_2d<f32>;
@group(3) @binding(1) var tex_sampler: sampler;

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
) -> VsOut {
    var out: VsOut;
    let world_position = object.world * vec4<f32>(position, 1.0);
    let world_normal = normalize((object.world * vec4<f32>(normal, 0.0)).xyz);
    out.position = frame.view_projection * world_position;
    out.color = material.base_color * vec4<f32>(color.rgb, max(color.a, 1.0));
    out.normal = world_normal;
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    let shader_code = material.params.z;
    let n = normalize(normal);
    if (shader_code > 5.5) {
        return vec4<f32>(n * 0.5 + vec3<f32>(0.5), color.a);
    }

    var base = color.rgb;
    if (material.params.w > 0.5) {
        base *= textureSample(albedo_texture, tex_sampler, uv).rgb;
    }

    let light_dir = normalize(vec3<f32>(-0.35, 0.8, 0.45));
    var diffuse = max(dot(n, light_dir), 0.0);
    if (shader_code > 3.5 && shader_code < 4.5) {
        diffuse = floor(diffuse * 4.0) / 3.0;
    }
    let ambient = 0.22;
    let lit = base * (ambient + diffuse * 0.78) + material.emissive_cutoff.rgb;
    let wire_boost = select(vec3<f32>(1.0), vec3<f32>(1.22), shader_code > 4.5 && shader_code < 5.5);
    return vec4<f32>(lit * wire_boost, color.a);
}
"#;

/// 蒙皮网格专用 WGSL 着色器源码
const SKIN_SHADER_SRC: &str = r#"
struct FrameUniform {
    view_projection: mat4x4<f32>,
    camera_position_frame: vec4<f32>,
    resolution: vec4<f32>,
};

struct ObjectUniform {
    world: mat4x4<f32>,
};

struct MaterialUniform {
    base_color: vec4<f32>,
    emissive_cutoff: vec4<f32>,
    params: vec4<f32>,
};

struct SkinInfo {
    first_joint: u32,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@group(0) @binding(0) var<uniform> frame: FrameUniform;
@group(1) @binding(0) var<uniform> object: ObjectUniform;
@group(2) @binding(0) var<uniform> material: MaterialUniform;
@group(3) @binding(0) var albedo_texture: texture_2d<f32>;
@group(3) @binding(1) var tex_sampler: sampler;
@group(4) @binding(0) var<storage> bones: array<mat4x4<f32>>;
@group(4) @binding(1) var<uniform> skin_info: SkinInfo;

@vertex
fn vs_skin(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(5) joint_indices: vec4<u32>,
    @location(6) joint_weights: vec4<f32>,
) -> VsOut {
    var skinned_pos = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var skinned_nml = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    let base = skin_info.first_joint;
    for (var i = 0u; i < 4u; i++) {
        let w = joint_weights[i];
        if (w > 0.0) {
            let bone = bones[base + joint_indices[i]];
            skinned_pos += w * (bone * vec4<f32>(position, 1.0));
            skinned_nml += w * (bone * vec4<f32>(normal, 0.0));
        }
    }
    let world_position = object.world * skinned_pos;
    let world_normal = normalize((object.world * skinned_nml).xyz);
    var out: VsOut;
    out.position = frame.view_projection * world_position;
    out.color = material.base_color * vec4<f32>(color.rgb, max(color.a, 1.0));
    out.normal = world_normal;
    out.uv = uv;
    return out;
}

@fragment
fn fs_skin(
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    let shader_code = material.params.z;
    let n = normalize(normal);
    if (shader_code > 5.5) {
        return vec4<f32>(n * 0.5 + vec3<f32>(0.5), color.a);
    }
    var base = color.rgb;
    if (material.params.w > 0.5) {
        base *= textureSample(albedo_texture, tex_sampler, uv).rgb;
    }
    let light_dir = normalize(vec3<f32>(-0.35, 0.8, 0.45));
    var diffuse = max(dot(n, light_dir), 0.0);
    if (shader_code > 3.5 && shader_code < 4.5) {
        diffuse = floor(diffuse * 4.0) / 3.0;
    }
    let ambient = 0.22;
    let lit = base * (ambient + diffuse * 0.78) + material.emissive_cutoff.rgb;
    let wire_boost = select(vec3<f32>(1.0), vec3<f32>(1.22), shader_code > 4.5 && shader_code < 5.5);
    return vec4<f32>(lit * wire_boost, color.a);
}
"#;

// ============================================================================
// GPU Uniform 类型
// ============================================================================

/// 每帧 uniform 数据，对应 WGSL FrameUniform
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct FrameUniform {
    view_projection: [[f32; 4]; 4],
    camera_position_frame: [f32; 4],
    resolution: [f32; 4],
}

/// 每个对象的 uniform 数据，对应 WGSL ObjectUniform
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ObjectUniform {
    world: [[f32; 4]; 4],
}

/// 材质 uniform 数据，对应 WGSL MaterialUniform
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct MaterialUniform {
    base_color: [f32; 4],
    emissive_cutoff: [f32; 4],
    params: [f32; 4],
}

impl MaterialUniform {
    fn from_material(material: &RendererMaterial) -> Self {
        let color = material.preview_color();
        let has_texture = match material {
            RendererMaterial::Pbr(pbr) => pbr.albedo_texture.is_some(),
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
struct SkinnedVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    color: [f32; 4],
    tangent: [f32; 4],
    joint_indices: [u32; 4],
    joint_weights: [f32; 4],
}

impl SkinnedVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
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
struct SkinInfoUniform {
    first_joint: u32,
    _pad: [u32; 3],
}

/// CPU 端皮肤数据
struct SkinData {
    inverse_bind_matrices: Vec<scenix::Mat4>,
    joint_node_indices: Vec<usize>,
}

/// GPU 蒙皮网格资源
struct GpuSkinnedMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    index_format: wgpu::IndexFormat,
}

/// 动画采样曲线（原始 glTF 数据）
struct AnimSampler {
    times: Vec<f32>,
    outputs: Vec<[f32; 4]>,
}

/// 动画通道
struct AnimChannel {
    node: usize,
    /// 0=平移, 1=旋转, 2=缩放
    target: u32,
    sampler: usize,
}

/// 动画剪辑
struct AnimClip {
    _name: String,
    samplers: Vec<AnimSampler>,
    channels: Vec<AnimChannel>,
}

/// 蒙皮关节节点描述
struct JointNode {
    parent: Option<usize>,
    default_trs: ([f32; 3], [f32; 4], [f32; 3]),
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 对齐到 256 字节边界
fn align_to_256(n: u32) -> u32 {
    (n + 255) & !255
}

/// 对齐 uniform 大小到 256 字节
fn aligned_size<T>() -> u64 {
    ((std::mem::size_of::<T>() as u64) + 255) & !255
}

/// 将 scenix Mat4 转为 [[f32; 4]; 4]
fn mat4_to_array(m: scenix::Mat4) -> [[f32; 4]; 4] {
    [
        [m.cols[0].x, m.cols[0].y, m.cols[0].z, m.cols[0].w],
        [m.cols[1].x, m.cols[1].y, m.cols[1].z, m.cols[1].w],
        [m.cols[2].x, m.cols[2].y, m.cols[2].z, m.cols[2].w],
        [m.cols[3].x, m.cols[3].y, m.cols[3].z, m.cols[3].w],
    ]
}

/// scenix 过滤模式 → wgpu 过滤模式
fn filter_to_wgpu(f: scenix::FilterMode) -> wgpu::FilterMode {
    match f {
        scenix::FilterMode::Nearest => wgpu::FilterMode::Nearest,
        scenix::FilterMode::Linear => wgpu::FilterMode::Linear,
    }
}

/// scenix 过滤模式 → wgpu mipmap 过滤模式
fn mip_filter_to_wgpu(f: scenix::FilterMode) -> wgpu::MipmapFilterMode {
    match f {
        scenix::FilterMode::Nearest => wgpu::MipmapFilterMode::Nearest,
        scenix::FilterMode::Linear => wgpu::MipmapFilterMode::Linear,
    }
}

/// scenix 寻址模式 → wgpu 寻址模式
fn address_to_wgpu(a: scenix::AddressMode) -> wgpu::AddressMode {
    match a {
        scenix::AddressMode::Repeat => wgpu::AddressMode::Repeat,
        scenix::AddressMode::MirrorRepeat => wgpu::AddressMode::MirrorRepeat,
        scenix::AddressMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
    }
}

// ============================================================================
// 骨骼动画数学辅助函数
// ============================================================================

/// 查找线性插值的两个关键帧索引和插值因子（二分搜索）
fn find_lerp_factors(times: &[f32], t: f32) -> (usize, usize, f32) {
    let last = times.len() - 1;
    if t <= times[0] {
        return (0, 0, 0.0);
    }
    if t >= times[last] {
        return (last, last, 0.0);
    }
    // 二分搜索找到 lo 满足 times[lo] <= t < times[lo+1]
    let mut lo = 0usize;
    let mut hi = last;
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if times[mid] <= t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let frac = if times[hi] > times[lo] {
        (t - times[lo]) / (times[hi] - times[lo])
    } else {
        0.0
    };
    (lo, hi, frac)
}

/// 插值两个 vec4 值（平移/缩放线性，旋转 slerp）
fn lerp_value(a: [f32; 4], b: [f32; 4], t: f32, target: u32) -> [f32; 4] {
    if target == 1 {
        // 旋转：球面线性插值
        let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
        let (bx, by, bz, bw) = if dot < 0.0 {
            (-b[0], -b[1], -b[2], -b[3])
        } else {
            (b[0], b[1], b[2], b[3])
        };
        let dot_abs = dot.abs().clamp(-1.0, 1.0);
        let theta = dot_abs.acos();
        let sin_theta = theta.sin();
        if sin_theta.abs() < 1e-6 {
            return [
                a[0] + t * (bx - a[0]),
                a[1] + t * (by - a[1]),
                a[2] + t * (bz - a[2]),
                a[3] + t * (bw - a[3]),
            ];
        }
        let w0 = ((1.0 - t) * theta).sin() / sin_theta;
        let w1 = (t * theta).sin() / sin_theta;
        [
            w0 * a[0] + w1 * bx,
            w0 * a[1] + w1 * by,
            w0 * a[2] + w1 * bz,
            w0 * a[3] + w1 * bw,
        ]
    } else {
        // 平移/缩放：线性插值
        [
            a[0] + t * (b[0] - a[0]),
            a[1] + t * (b[1] - a[1]),
            a[2] + t * (b[2] - a[2]),
            a[3] + t * (b[3] - a[3]),
        ]
    }
}

/// 从四元数构建 3x3 旋转矩阵列（返回列向量）
fn quat_to_rot_cols(q: [f32; 4]) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    let xx = x * x;
    let xy = x * y;
    let xz = x * z;
    let xw = x * w;
    let yy = y * y;
    let yz = y * z;
    let yw = y * w;
    let zz = z * z;
    let zw = z * w;
    let col0 = [1.0 - 2.0 * (yy + zz), 2.0 * (xy + zw), 2.0 * (xz - yw)];
    let col1 = [2.0 * (xy - zw), 1.0 - 2.0 * (xx + zz), 2.0 * (yz + xw)];
    let col2 = [2.0 * (xz + yw), 2.0 * (yz - xw), 1.0 - 2.0 * (xx + yy)];
    (col0, col1, col2)
}

/// TRS → scenix Mat4
fn trs_to_mat4(t: [f32; 3], r: [f32; 4], s: [f32; 3]) -> scenix::Mat4 {
    let (rc0, rc1, rc2) = quat_to_rot_cols(r);
    scenix::Mat4::from_cols(
        scenix::Vec4::new(rc0[0] * s[0], rc0[1] * s[0], rc0[2] * s[0], 0.0),
        scenix::Vec4::new(rc1[0] * s[1], rc1[1] * s[1], rc1[2] * s[1], 0.0),
        scenix::Vec4::new(rc2[0] * s[2], rc2[1] * s[2], rc2[2] * s[2], 0.0),
        scenix::Vec4::new(t[0], t[1], t[2], 1.0),
    )
}

/// 4x4 矩阵乘法（scenix Mat4）
fn mat4_mul(a: &scenix::Mat4, b: &scenix::Mat4) -> scenix::Mat4 {
    let a0 = &a.cols[0];
    let a1 = &a.cols[1];
    let a2 = &a.cols[2];
    let a3 = &a.cols[3];
    let b0 = &b.cols[0];
    let b1 = &b.cols[1];
    let b2 = &b.cols[2];
    let b3 = &b.cols[3];

    let mul_col = |bc: &scenix::Vec4| -> scenix::Vec4 {
        scenix::Vec4::new(
            a0.x * bc.x + a1.x * bc.y + a2.x * bc.z + a3.x * bc.w,
            a0.y * bc.x + a1.y * bc.y + a2.y * bc.z + a3.y * bc.w,
            a0.z * bc.x + a1.z * bc.y + a2.z * bc.z + a3.z * bc.w,
            a0.w * bc.x + a1.w * bc.y + a2.w * bc.z + a3.w * bc.w,
        )
    };
    scenix::Mat4::from_cols(mul_col(b0), mul_col(b1), mul_col(b2), mul_col(b3))
}

/// 将 scenix Mat4 转为 [f32; 16]（列主序，用于 GPU 上传）
fn mat4_to_flat(m: &scenix::Mat4) -> [f32; 16] {
    [
        m.cols[0].x,
        m.cols[0].y,
        m.cols[0].z,
        m.cols[0].w,
        m.cols[1].x,
        m.cols[1].y,
        m.cols[1].z,
        m.cols[1].w,
        m.cols[2].x,
        m.cols[2].y,
        m.cols[2].z,
        m.cols[2].w,
        m.cols[3].x,
        m.cols[3].y,
        m.cols[3].z,
        m.cols[3].w,
    ]
}

fn mat4_identity() -> scenix::Mat4 {
    scenix::Mat4::from_cols(
        scenix::Vec4::new(1.0, 0.0, 0.0, 0.0),
        scenix::Vec4::new(0.0, 1.0, 0.0, 0.0),
        scenix::Vec4::new(0.0, 0.0, 1.0, 0.0),
        scenix::Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

// ============================================================================
// 公开类型
// ============================================================================

/// 已上传的 GPU 纹理，包含绑定点
struct TextureEntry {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

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
    /// 返回的 RenderImage 包含 BGRA 格式的帧数据，适合 Polychrome 纹理使用。
    pub fn into_render_image(self) -> RenderImage {
        let img_buffer = image::RgbaImage::from_raw(self.width, self.height, self.data)
            .expect("无效的像素数据尺寸");
        let frame = image::Frame::new(img_buffer);
        RenderImage::new(smallvec![frame])
    }
}

/// 3D 场景渲染上下文
///
/// 管理 wgpu 设备、离屏渲染目标、scenix GPU 场景资源和渲染管线。
///
/// # 生命周期
/// 1. 使用 `Scenix3D::new(width, height)` 异步创建
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
    // 每帧缓存，避免重复分配
    cached_local_trs: Vec<([f32; 3], [f32; 4], [f32; 3])>,
    cached_global_mats: Vec<scenix::Mat4>,
    cached_bone_data: Vec<f32>,

    // 关节变换覆盖（允许手动控制关节旋转）
    joint_overrides: Vec<Option<scenix::Quat>>,
}

impl Scenix3D {
    /// 异步创建新的 3D 渲染上下文
    ///
    /// * `width` - 渲染宽度（像素）
    /// * `height` - 渲染高度（像素）
    pub async fn new(width: u32, height: u32) -> Result<Self, ScenixError> {
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
                label: Some("rgpui-scenix.device"),
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

        let color_format = wgpu::TextureFormat::Bgra8UnormSrgb;

        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rgpui-scenix.color"),
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
            label: Some("rgpui-scenix.depth"),
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

        // 绑定组布局
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rgpui-scenix.frame_layout"),
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
            label: Some("rgpui-scenix.object_layout"),
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
            label: Some("rgpui-scenix.material_layout"),
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

        // 纹理绑定组布局
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rgpui-scenix.texture_layout"),
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

        // 默认纹理采样器
        let texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("rgpui-scenix.texture_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });

        // 1x1 白色占位纹理（用于无纹理的材质）
        let white_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rgpui-scenix.white_texture"),
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
            label: Some("rgpui-scenix.white_bind_group"),
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

        // 着色器和管线
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rgpui-scenix.shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rgpui-scenix.pipeline_layout"),
            bind_group_layouts: &[
                Some(&frame_layout),
                Some(&object_layout),
                Some(&material_layout),
                Some(&texture_layout),
            ],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rgpui-scenix.pipeline"),
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

        // uniform 资源
        let object_stride = aligned_size::<ObjectUniform>();
        let material_stride = aligned_size::<MaterialUniform>();
        let draw_capacity = 256usize;

        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgpui-scenix.frame_buffer"),
            size: std::mem::size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let frame_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rgpui-scenix.frame_bind_group"),
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
            label: Some("rgpui-scenix.object_buffer"),
            size: object_stride * draw_capacity as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgpui-scenix.material_buffer"),
            size: material_stride * draw_capacity as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let object_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rgpui-scenix.object_bind_group"),
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
            label: Some("rgpui-scenix.material_bind_group"),
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

        // 蒙皮管线绑定组布局（group 4：骨骼矩阵 storage + 皮肤信息 uniform）
        let skin_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rgpui-scenix.skin_layout"),
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

        // 蒙皮着色和管道
        let skin_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rgpui-scenix.skin_shader"),
            source: wgpu::ShaderSource::Wgsl(SKIN_SHADER_SRC.into()),
        });

        let skin_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rgpui-scenix.skin_pipeline_layout"),
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
            label: Some("rgpui-scenix.skin_pipeline"),
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

        // 骨骼矩阵 storage buffer（初始容量 128 根骨骼）
        let bone_capacity = 128u32;
        let bone_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgpui-scenix.bone_buffer"),
            size: (bone_capacity as u64) * 64, // mat4x4<f32> = 64 bytes
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 皮肤信息 uniform buffer
        let skin_info_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgpui-scenix.skin_info_buffer"),
            size: std::mem::size_of::<SkinInfoUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // 写入初始值 first_joint=0
        queue.write_buffer(
            &skin_info_buffer,
            0,
            bytemuck::bytes_of(&SkinInfoUniform {
                first_joint: 0,
                _pad: [0u32; 3],
            }),
        );

        let skin_info_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rgpui-scenix.skin_info_bg"),
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
            clear_color: [1.0, 1.0, 1.0, 1.0], // 默认白色背景
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
            label: Some("rgpui-scenix.color"),
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
            label: Some("rgpui-scenix.depth"),
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

    /// 上传纹理到 GPU 并注册，支持后续在渲染时绑定
    ///
    /// * `id` - 纹理标识符（应与材质中引用的 `TextureId` 匹配）
    /// * `texture` - CPU 端纹理数据（宽度、高度、格式、像素）
    /// * `sampler` - 采样器状态（过滤、寻址模式等）
    pub fn register_texture(
        &mut self,
        id: TextureId,
        texture: &scenix::Texture2D,
        sampler: scenix::Sampler,
    ) -> Result<(), ScenixError> {
        let wgpu_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rgpui-scenix.texture"),
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
            label: Some("rgpui-scenix.texture_bind_group"),
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
    ///
    /// 遍历 `GltfAsset` 中的网格、纹理和材质，分别注册到 GPU。
    /// 纹理会被上传到 GPU 并在渲染时自动绑定。
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

    /// 加载 glTF 文件的蒙皮和动画数据，创建蒙皮网格 GPU 资源
    ///
    /// 此方法打开 glTF 文件解析皮肤绑定、关节层级和动画剪辑。
    /// 它会为每个蒙皮网格构建包含关节索引/权重的顶点缓冲区。
    /// 对于非蒙皮的网格，此方法不会修改（它们仍然通过 `register_gltf_asset` 注册）。
    ///
    /// * `path` - glTF/GLB 文件路径
    /// * `asset` - 已通过 scenix 加载的资产（用于获取网格几何体）
    /// * `scene` - 对应的场景图（用于匹配节点索引）
    pub fn load_gltf_skins(
        &mut self,
        path: &str,
        asset: &scenix::GltfAsset,
    ) -> Result<(), ScenixError> {
        let (document, buffer_data, _image_data) =
            gltf::import(path).map_err(|_| ScenixError::Load(scenix::LoadError::Io))?;

        // --- 读取关节节点层级（glTF Node 没有 parent()，需手动构建）---
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

        // --- 读取皮肤 ---
        self.skins.clear();
        let mut mesh_to_skin: HashMap<MeshId, usize> = HashMap::new();

        // 扫描所有节点，找到 mesh + skin 的组合
        for node in document.nodes() {
            let (Some(mesh_gltf), Some(skin)) = (node.mesh(), node.skin()) else {
                continue;
            };
            // glTF mesh index 对应 scenix MeshId
            let mesh_id = MeshId::new(mesh_gltf.index() as u64);
            if !asset.meshes.contains_key(&mesh_id) {
                continue;
            }

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
            mesh_to_skin.insert(mesh_id, self.skins.len() - 1);
        }

        // --- 创建蒙皮网格 GPU 资源 ---
        for (mesh_id, _geometry) in &asset.meshes {
            if !mesh_to_skin.contains_key(mesh_id) {
                continue;
            }
            // 找到对应 glTF 网格
            let Some(gltf_mesh) = document.meshes().nth(mesh_id.get() as usize) else {
                continue;
            };

            let mut all_verts: Vec<SkinnedVertex> = Vec::new();
            let mut all_indices: Vec<u32> = Vec::new();
            let mut base_vertex = 0u32;

            for primitive in gltf_mesh.primitives() {
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

                // 读取蒙皮数据
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

                // 读取索引
                let indices: Vec<u32> = reader
                    .read_indices()
                    .map(|iter| iter.into_u32().collect())
                    .unwrap_or_default();

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
                    all_verts.push(SkinnedVertex {
                        position: positions.get(vi).copied().unwrap_or([0.0; 3]),
                        normal: normals.get(vi).copied().unwrap_or([0.0; 3]),
                        uv: uvs.get(vi).copied().unwrap_or([0.0; 2]),
                        color: c,
                        tangent: [0.0; 4],
                        joint_indices: ji,
                        joint_weights: jw,
                    });
                }
                for idx in indices {
                    all_indices.push(base_vertex + idx);
                }
                base_vertex += count as u32;
            }

            if all_verts.is_empty() || all_indices.is_empty() {
                continue;
            }

            let _skin_idx = mesh_to_skin[mesh_id];
            let index_format = if all_indices.len() <= u16::MAX as usize {
                wgpu::IndexFormat::Uint16
            } else {
                wgpu::IndexFormat::Uint32
            };
            let index_data: Vec<u8> = match index_format {
                wgpu::IndexFormat::Uint16 => {
                    let short: Vec<u16> = all_indices.iter().map(|&i| i as u16).collect();
                    bytemuck::cast_slice(&short).to_vec()
                }
                wgpu::IndexFormat::Uint32 => bytemuck::cast_slice(&all_indices).to_vec(),
            };

            let vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("rgpui-scenix.skin_vb_{}", mesh_id.get() as usize)),
                size: (all_verts.len() * std::mem::size_of::<SkinnedVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue
                .write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&all_verts));

            let index_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("rgpui-scenix.skin_ib_{}", mesh_id.get() as usize)),
                size: index_data.len() as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue.write_buffer(&index_buffer, 0, &index_data);

            self.skinned_meshes.insert(
                *mesh_id,
                GpuSkinnedMesh {
                    vertex_buffer,
                    index_buffer,
                    index_count: all_indices.len() as u32,
                    index_format,
                },
            );
        }

        self.mesh_to_skin = mesh_to_skin;
        self.joints = joints;

        // --- 读取动画剪辑 ---
        use std::collections::HashMap as HashMap2;
        self.anim_clips.clear();
        for anim in document.animations() {
            // 收集所有 sampler 数据（去重）
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

            // 按索引顺序排列
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

        if !self.anim_clips.is_empty() {
            self.active_anim = 0;
            self.anim_time = 0.0;
            eprintln!(
                "[rgpui-3d] load_gltf_skins: {} animations loaded, {} joints, {} skins",
                self.anim_clips.len(),
                self.joints.len(),
                self.skins.len()
            );
            for (i, clip) in self.anim_clips.iter().enumerate() {
                eprintln!(
                    "  anim[{}]: '{}' - {} samplers, {} channels",
                    i,
                    clip._name,
                    clip.samplers.len(),
                    clip.channels.len()
                );
            }
            for (i, skin) in self.skins.iter().enumerate() {
                eprintln!("  skin[{}]: {} joints", i, skin.joint_node_indices.len());
            }
        } else {
            eprintln!(
                "[rgpui-3d] load_gltf_skins: NO animations, {} joints, {} skins",
                self.joints.len(),
                self.skins.len()
            );
        }

        Ok(())
    }

    /// 推进动画时间，计算骨骼矩阵并上传到 GPU
    ///
    /// 每帧在渲染之前调用：
    /// - 推进当前动画时间（秒）
    /// - 对每个关节节点采样动画曲线得到局部 TRS
    /// - 从根到叶遍历关节层级计算全局变换
    /// - 对每套皮肤计算骨骼矩阵（全局 × 逆绑定）
    /// - 上传所有骨骼矩阵到 GPU storage buffer
    ///
    /// * `dt` - 时间增量（秒）
    pub fn advance_animation(&mut self, dt: f32) {
        if self.anim_clips.is_empty() || self.joints.is_empty() {
            return;
        }
        if self.anim_paused {
            return;
        }

        self.anim_time += dt * self.anim_speed;

        let clip = &self.anim_clips[self.active_anim];
        let num_joints = self.joints.len();

        // 1. 采样动画曲线得到每个关节的局部 TRS（使用缓存避免分配）
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
                // 循环动画：折返到开头
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

        // 2. 应用关节旋转覆盖（手动控制覆盖动画采样结果）
        for (i, override_q) in self.joint_overrides.iter().enumerate() {
            if i >= num_joints {
                break;
            }
            if let Some(q) = override_q {
                self.cached_local_trs[i].1 = [q.x, q.y, q.z, q.w];
            }
        }

        // 3. 计算全局变换（层级传播，使用 BFS 从根节点遍历，不依赖节点索引顺序）
        self.cached_global_mats.clear();
        self.cached_global_mats
            .resize(num_joints, scenix::Mat4::IDENTITY);

        // 先计算所有局部矩阵
        let local_mats: Vec<scenix::Mat4> = (0..num_joints)
            .map(|i| {
                trs_to_mat4(
                    self.cached_local_trs[i].0,
                    self.cached_local_trs[i].1,
                    self.cached_local_trs[i].2,
                )
            })
            .collect();

        // 建立子节点列表
        let mut children_of: Vec<Vec<usize>> = vec![Vec::new(); num_joints];
        for i in 0..num_joints {
            if let Some(p) = self.joints[i].parent {
                if p < num_joints {
                    children_of[p].push(i);
                }
            }
        }

        // 找根节点（无父节点或父节点越界）
        let mut queue: Vec<usize> = Vec::new();
        for i in 0..num_joints {
            if self.joints[i].parent.is_none() || self.joints[i].parent.unwrap() >= num_joints {
                self.cached_global_mats[i] = local_mats[i];
                queue.push(i);
            }
        }

        // BFS 遍历层级，从根向下传播
        while let Some(parent) = queue.pop() {
            for &child in &children_of[parent] {
                self.cached_global_mats[child] =
                    mat4_mul(&self.cached_global_mats[parent], &local_mats[child]);
                queue.push(child);
            }
        }

        // 4. 计算骨骼矩阵并上传
        let mut total_bones = 0usize;
        for skin in &self.skins {
            total_bones += skin.joint_node_indices.len();
        }

        if total_bones == 0 {
            return;
        }

        // 确保 bone buffer 够大
        let needed_bytes = total_bones as u64 * 64;
        if needed_bytes > (self.bone_capacity as u64) * 64 {
            self.bone_capacity = (total_bones as u32).next_power_of_two();
            self.bone_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rgpui-scenix.bone_buffer"),
                size: self.bone_capacity as u64 * 64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            // 重新创建 bind group
            self.skin_info_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rgpui-scenix.skin_info_bg"),
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

        // 使用缓存的 bone data Vec，避免每帧重新分配
        self.cached_bone_data.clear();
        self.cached_bone_data.reserve(total_bones * 16);
        let mut first_joint = 0u32;

        for skin in &self.skins {
            for (joint_idx, ibm) in skin
                .joint_node_indices
                .iter()
                .zip(skin.inverse_bind_matrices.iter())
            {
                if *joint_idx < num_joints {
                    let bone_mat = mat4_mul(&self.cached_global_mats[*joint_idx], ibm);
                    let arr = mat4_to_flat(&bone_mat);
                    self.cached_bone_data.extend_from_slice(&arr);
                } else {
                    self.cached_bone_data
                        .extend_from_slice(&mat4_to_flat(&mat4_identity()));
                }
            }
            // 写入皮肤信息：每个皮肤对应当前 first_joint 偏移
            // 注意：如果有多个皮肤，需要在渲染时切换 skin_info buffer
            // 为简化，仅支持一个皮肤
            if skin.joint_node_indices.len() > 0 {
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
        }

        self.queue.write_buffer(
            &self.bone_buffer,
            0,
            bytemuck::cast_slice(&self.cached_bone_data),
        );

        // 调试：前5帧打印骨骼状态
        use std::sync::atomic::{AtomicU32, Ordering};
        static FRAME_COUNT: AtomicU32 = AtomicU32::new(0);
        let fc = FRAME_COUNT.fetch_add(1, Ordering::Relaxed);
        if fc < 5 {
            eprintln!(
                "[rgpui-3d] advance_animation frame={}: time={:.3}, total_bones={}, bone_data_len={}",
                fc + 1,
                self.anim_time,
                total_bones,
                self.cached_bone_data.len()
            );
            if self.cached_bone_data.len() >= 16 {
                let m: Vec<f32> = self.cached_bone_data[0..16].to_vec();
                eprintln!(
                    "  bone[0] (Hips): [{:.3}, {:.3}, {:.3}, {:.3}]",
                    m[0], m[4], m[8], m[12]
                );
            }
            let clip = &self.anim_clips[self.active_anim];
            eprintln!(
                "  clip '{}' has {} channels, {} samplers",
                clip._name,
                clip.channels.len(),
                clip.samplers.len()
            );
        }
    }

    /// 获取动画剪辑名称列表
    pub fn animation_names(&self) -> Vec<String> {
        self.anim_clips.iter().map(|c| c._name.clone()).collect()
    }

    /// 切换到指定动画剪辑
    pub fn set_active_animation(&mut self, index: usize) {
        if index < self.anim_clips.len() {
            self.active_anim = index;
            self.anim_time = 0.0;
        }
    }

    /// 获取当前动画时间
    pub fn animation_time(&self) -> f32 {
        self.anim_time
    }

    /// 设置当前动画时间（秒）
    pub fn set_animation_time(&mut self, t: f32) {
        self.anim_time = t;
    }

    /// 获取动画播放速度（1.0 = 正常速度）
    pub fn animation_speed(&self) -> f32 {
        self.anim_speed
    }

    /// 设置动画播放速度（1.0 = 正常速度，2.0 = 两倍速，0.5 = 半速）
    pub fn set_animation_speed(&mut self, speed: f32) {
        self.anim_speed = speed;
    }

    /// 动画是否暂停
    pub fn is_animation_paused(&self) -> bool {
        self.anim_paused
    }

    /// 暂停/恢复动画播放
    pub fn set_animation_paused(&mut self, paused: bool) {
        self.anim_paused = paused;
    }

    /// 获取当前动画剪辑的总时长（秒）
    pub fn animation_duration(&self) -> f32 {
        if self.anim_clips.is_empty() {
            return 0.0;
        }
        let clip = &self.anim_clips[self.active_anim];
        let mut max_time = 0.0f32;
        for sampler in &clip.samplers {
            if let Some(&last_time) = sampler.times.last() {
                max_time = max_time.max(last_time);
            }
        }
        max_time
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

    /// 生成程序化行走动画（当 glTF 没有动画数据时使用）
    ///
    /// 通过皮肤数据找到骨骼根节点（Hips），然后自动查找大腿、膝关节和脊柱，
    /// 生成循环行走动画并添加到动画剪辑列表。
    ///
    /// * `cycle_duration` - 一个行走周期的时长（秒），默认 0.8
    /// * `stride_angle` - 腿部摆动幅度（弧度），默认 0.5
    pub fn generate_walk_animation(&mut self, cycle_duration: f32, stride_angle: f32) {
        if self.skins.is_empty() {
            eprintln!("[rgpui-3d] generate_walk_animation: no skins, aborting");
            return;
        }

        // 使用皮肤的关节列表来确定哪些节点是骨骼关节
        let skin = &self.skins[0];
        let joint_set: std::collections::HashSet<usize> =
            skin.joint_node_indices.iter().copied().collect();

        // 在皮肤关节中找根节点：没有父节点，或父节点不在皮肤关节中
        let skeleton_root = skin
            .joint_node_indices
            .iter()
            .find(|&&node_idx| {
                node_idx < self.joints.len() && {
                    match self.joints[node_idx].parent {
                        None => true,
                        Some(p) => !joint_set.contains(&p),
                    }
                }
            })
            .copied();

        let Some(hips) = skeleton_root else {
            eprintln!("[rgpui-3d] generate_walk_animation: no skeleton root found");
            return;
        };

        eprintln!(
            "[rgpui-3d] generate_walk_animation: skeleton root (hips) = node[{}]",
            hips
        );

        // 找 Hips 的皮肤子关节
        let hips_children: Vec<usize> = skin
            .joint_node_indices
            .iter()
            .filter(|&&node_idx| {
                node_idx < self.joints.len() && self.joints[node_idx].parent == Some(hips)
            })
            .copied()
            .collect();

        eprintln!("[rgpui-3d]   hips_children = {:?}", hips_children);

        // 分类子关节：腿 vs 脊柱
        // 脊柱子树包含更多后代（手臂、头等），腿子树较小
        let (leg_indices, spine_idx) = if hips_children.len() >= 3 {
            let mut with_size: Vec<(usize, usize)> = hips_children
                .iter()
                .map(|&c| {
                    // 计算 c 子树中属于皮肤关节的后代数量
                    let size = skin
                        .joint_node_indices
                        .iter()
                        .filter(|&&node_idx| {
                            let mut cur = self.joints.get(node_idx).and_then(|j| j.parent);
                            while let Some(p) = cur {
                                if p == c {
                                    return true;
                                }
                                cur = self.joints.get(p).and_then(|j| j.parent);
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

        eprintln!(
            "[rgpui-3d]   leg_indices = {:?}, spine_idx = {:?}",
            leg_indices, spine_idx
        );

        if leg_indices.is_empty() {
            return;
        }

        let dt = cycle_duration / 16.0; // 16 帧一个周期
        let frames = 17; // 0..=16
        let times: Vec<f32> = (0..frames).map(|i| i as f32 * dt).collect();

        let mut samplers = Vec::new();
        let mut channels = Vec::new();

        // 对每条腿生成旋转动画
        for (leg_idx, &leg_node) in leg_indices.iter().enumerate() {
            let phase_offset = if leg_idx == 0 {
                0.0
            } else {
                std::f32::consts::PI
            };
            let dir = if leg_idx == 0 { 1.0 } else { -1.0 };

            // 大腿前后摆动（绕 X 轴旋转）
            let rotation_outputs: Vec<[f32; 4]> = (0..frames)
                .map(|i| {
                    let t = i as f32 / 16.0 * std::f32::consts::TAU + phase_offset;
                    let angle = dir * stride_angle * t.sin();
                    // 四元数：绕 X 轴旋转
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
                target: 1, // rotation
                sampler: sampler_idx,
            });

            // 找膝关节（大腿的皮肤子关节）
            if let Some(knee) = skin
                .joint_node_indices
                .iter()
                .find(|&&node_idx| {
                    node_idx < self.joints.len() && self.joints[node_idx].parent == Some(leg_node)
                })
                .copied()
            {
                // 膝关节弯曲（走路时自然弯曲）
                let knee_outputs: Vec<[f32; 4]> = (0..frames)
                    .map(|i| {
                        let t = i as f32 / 16.0 * std::f32::consts::TAU + phase_offset;
                        // 弯曲角度：腿向后摆时弯曲更多
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
                    target: 1, // rotation
                    sampler: sampler_idx,
                });
            }
        }

        // 脊柱轻微摆动
        if let Some(spine) = spine_idx {
            let spine_outputs: Vec<[f32; 4]> = (0..frames)
                .map(|i| {
                    let t = i as f32 / 16.0 * std::f32::consts::TAU;
                    let angle = 0.05 * t.sin(); // 很小的摆动
                    let half = angle * 0.5;
                    [0.0, half.sin(), 0.0, half.cos()] // 绕 Y 轴
                })
                .collect();

            let sampler_idx = samplers.len();
            samplers.push(AnimSampler {
                times: times.clone(),
                outputs: spine_outputs,
            });
            channels.push(AnimChannel {
                node: spine,
                target: 1, // rotation
                sampler: sampler_idx,
            });
        }

        // Hips 上下轻微弹跳
        let hips_outputs: Vec<[f32; 4]> = (0..frames)
            .map(|i| {
                let t = i as f32 / 16.0 * std::f32::consts::TAU;
                let y_offset = -0.02 * (t * 2.0).abs().cos(); // 上下微弹
                [0.0, y_offset, 0.0, 0.0]
            })
            .collect();

        let sampler_idx = samplers.len();
        samplers.push(AnimSampler {
            times,
            outputs: hips_outputs,
        });
        channels.push(AnimChannel {
            node: hips,
            target: 0, // translation
            sampler: sampler_idx,
        });

        self.anim_clips.push(AnimClip {
            _name: "Walk".to_string(),
            samplers,
            channels,
        });

        // 如果之前没有动画，激活这个
        if self.active_anim >= self.anim_clips.len() - 1 {
            self.active_anim = self.anim_clips.len() - 1;
            self.anim_time = 0.0;
        }

        eprintln!(
            "[rgpui-3d] generate_walk_animation: hips={}, legs={:?}, spine={:?}, total_clips={}",
            hips,
            leg_indices,
            spine_idx,
            self.anim_clips.len()
        );
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

    /// 获取关节的世界位置（需在 advance_animation 之后调用）
    pub fn joint_world_position(&self, index: usize) -> Option<scenix::Vec3> {
        self.joint_world_matrix(index)
            .map(|m| scenix::Vec3::new(m.cols[3].x, m.cols[3].y, m.cols[3].z))
    }

    /// 设置关节的旋转覆盖（覆盖动画中的旋转）
    ///
    /// 传入的旋转是局部旋转（四元数），相对于关节的父节点。
    /// 调用 `clear_joint_overrides()` 恢复动画控制。
    pub fn set_joint_rotation_override(&mut self, index: usize, rotation: scenix::Quat) {
        if index < self.joints.len() {
            if self.joint_overrides.len() <= index {
                self.joint_overrides.resize(index + 1, None);
            }
            self.joint_overrides[index] = Some(rotation);
        }
    }

    /// 清除所有关节覆盖，恢复动画控制
    pub fn clear_joint_overrides(&mut self) {
        self.joint_overrides.clear();
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

    /// 渲染场景并返回像素结果
    ///
    /// * `scene` - scenix 场景图（其世界变换将被更新）
    /// * `camera` - 透视相机
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
                transparent.push(draw);
            } else {
                opaque.push(draw);
            }
        }
        sort_opaque_front_to_back(&mut opaque);
        sort_transparent_back_to_front(&mut transparent);

        let draw_count = opaque.len() + transparent.len();
        self.ensure_capacity(draw_count);

        // 写入 frame uniform
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

        // 写入 object / material uniform
        // 分离蒙皮和非蒙皮绘制调用
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

        // 执行渲染
        let color_view = self
            .color_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = self
            .depth_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rgpui-scenix.encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rgpui-scenix.render_pass"),
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
                // Pass 1: 非蒙皮网格（标准管线）
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

                // Pass 2: 蒙皮网格（骨骼动画管线）
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

        // 读回像素到 CPU
        let bpp = 4u32;
        let row_unpadded = self.width * bpp;
        let row_padded = align_to_256(row_unpadded);

        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgpui-scenix.readback"),
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

        // 映射并读取
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
            label: Some("rgpui-scenix.object_buffer"),
            size: obj_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.material_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgpui-scenix.material_buffer"),
            size: mat_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let object_layout = &self.object_layout;
        let material_layout = &self.material_layout;

        self.object_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rgpui-scenix.object_bind_group"),
            layout: object_layout,
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
            label: Some("rgpui-scenix.material_bind_group"),
            layout: material_layout,
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
