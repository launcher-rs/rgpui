/// 嵌入的 WGSL 着色器源码（支持 UV 和漫反射纹理）
pub(crate) const SHADER_SRC: &str = r#"
struct FrameUniform {
    view_projection: mat4x4<f32>,
    camera_position_frame: vec4<f32>,
    resolution: vec4<f32>,
};

struct ObjectUniform {
    world: mat4x4<f32>,
    bone_offset: u32,
    morph_base: u32,
    weight_base: u32,
    flags: u32,
    vertex_count: u32,
    morph_count: u32,
    pad0: u32,
    pad1: u32,
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
pub(crate) const SKIN_SHADER_SRC: &str = r#"
struct FrameUniform {
    view_projection: mat4x4<f32>,
    camera_position_frame: vec4<f32>,
    resolution: vec4<f32>,
};

struct ObjectUniform {
    world: mat4x4<f32>,
    bone_offset: u32,
    morph_base: u32,
    weight_base: u32,
    flags: u32,
    vertex_count: u32,
    morph_count: u32,
    pad0: u32,
    pad1: u32,
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
@group(4) @binding(0) var<storage> bones: array<mat4x4<f32>>;
@group(4) @binding(1) var<storage> morph_deltas: array<vec3<f32>>;
@group(4) @binding(2) var<storage> morph_weights: array<f32>;

const SKIN_FLAG_JOINTS: u32 = 1u;
const SKIN_FLAG_MORPH: u32 = 2u;

@vertex
fn vs_skin(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(5) joint_indices: vec4<u32>,
    @location(6) joint_weights: vec4<f32>,
    @builtin(vertex_index) vertex_index: u32,
) -> VsOut {
    var local_position = position;
    var local_normal = normal;

    // morph 在蒙皮之前、于绑定姿态空间叠加，与
    // scenekit_mesh::skin::apply_morph + cpu_skin 的顺序一致。
    if ((object.flags & SKIN_FLAG_MORPH) != 0u) {
        let stride = object.vertex_count * 2u;
        for (var morph = 0u; morph < object.morph_count; morph = morph + 1u) {
            let w = morph_weights[object.weight_base + morph];
            if (w == 0.0) {
                continue;
            }
            let block = object.morph_base + morph * stride;
            local_position += morph_deltas[block + vertex_index] * w;
            local_normal += morph_deltas[block + object.vertex_count + vertex_index] * w;
        }
    }

    var skinned_pos = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var skinned_nml = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    if ((object.flags & SKIN_FLAG_JOINTS) != 0u) {
        let base = object.bone_offset;
        for (var i = 0u; i < 4u; i++) {
            let w = joint_weights[i];
            if (w > 0.0) {
                let bone = bones[base + joint_indices[i]];
                skinned_pos += w * (bone * vec4<f32>(local_position, 1.0));
                skinned_nml += w * (bone * vec4<f32>(local_normal, 0.0));
            }
        }
    } else {
        // morph-only 网格（VRM 表情等）没有蒙皮：直接用物体矩阵变换。
        skinned_pos = vec4<f32>(local_position, 1.0);
        skinned_nml = vec4<f32>(local_normal, 0.0);
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
