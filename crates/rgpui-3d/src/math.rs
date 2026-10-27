// ============================================================================
// 辅助函数
// ============================================================================

/// 对齐到 256 字节边界
pub(crate) fn align_to_256(n: u32) -> u32 {
    (n + 255) & !255
}

/// 对齐 uniform 大小到 256 字节
pub(crate) fn aligned_size<T>() -> u64 {
    ((std::mem::size_of::<T>() as u64) + 255) & !255
}

/// 将 scenekit Mat4 转为 [[f32; 4]; 4]
pub(crate) fn mat4_to_array(m: scenekit::Mat4) -> [[f32; 4]; 4] {
    [
        [m.cols[0].x, m.cols[0].y, m.cols[0].z, m.cols[0].w],
        [m.cols[1].x, m.cols[1].y, m.cols[1].z, m.cols[1].w],
        [m.cols[2].x, m.cols[2].y, m.cols[2].z, m.cols[2].w],
        [m.cols[3].x, m.cols[3].y, m.cols[3].z, m.cols[3].w],
    ]
}

/// scenekit 过滤模式 → wgpu 过滤模式
pub(crate) fn filter_to_wgpu(f: scenekit::FilterMode) -> wgpu::FilterMode {
    match f {
        scenekit::FilterMode::Nearest => wgpu::FilterMode::Nearest,
        scenekit::FilterMode::Linear => wgpu::FilterMode::Linear,
    }
}

/// scenekit 过滤模式 → wgpu mipmap 过滤模式
pub(crate) fn mip_filter_to_wgpu(f: scenekit::FilterMode) -> wgpu::MipmapFilterMode {
    match f {
        scenekit::FilterMode::Nearest => wgpu::MipmapFilterMode::Nearest,
        scenekit::FilterMode::Linear => wgpu::MipmapFilterMode::Linear,
    }
}

/// scenekit 寻址模式 → wgpu 寻址模式
pub(crate) fn address_to_wgpu(a: scenekit::AddressMode) -> wgpu::AddressMode {
    match a {
        scenekit::AddressMode::Repeat => wgpu::AddressMode::Repeat,
        scenekit::AddressMode::MirrorRepeat => wgpu::AddressMode::MirrorRepeat,
        scenekit::AddressMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
    }
}

// ============================================================================
// 骨骼动画数学辅助函数
// ============================================================================

/// 查找线性插值的两个关键帧索引和插值因子（二分搜索）
pub(crate) fn find_lerp_factors(times: &[f32], t: f32) -> (usize, usize, f32) {
    let last = times.len() - 1;
    if t <= times[0] {
        return (0, 0, 0.0);
    }
    if t >= times[last] {
        return (last, last, 0.0);
    }
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

/// 按循环方式采样关键帧，返回 `(低帧, 高帧, 插值因子)`。
///
/// 时间超出最后关键帧时按剪辑总时长回绕，避免长跑后 `anim_time` 无限增长。
pub(crate) fn sample_factors(times: &[f32], t: f32) -> (usize, usize, f32) {
    if times.is_empty() {
        return (0, 0, 0.0);
    }
    let last = times.len() - 1;
    if t <= times[0] {
        return (0, 0, 0.0);
    }
    if t >= times[last] {
        let duration = times[last] - times[0];
        if duration > 0.0 {
            let wrapped = (t - times[0]) % duration;
            return find_lerp_factors(times, times[0] + wrapped);
        }
        return (last, last, 0.0);
    }
    find_lerp_factors(times, t)
}

/// 插值两个 vec4 值（平移/缩放线性，旋转 slerp）
pub(crate) fn lerp_value(a: [f32; 4], b: [f32; 4], t: f32, target: u32) -> [f32; 4] {
    if target == 1 {
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
        [
            a[0] + t * (b[0] - a[0]),
            a[1] + t * (b[1] - a[1]),
            a[2] + t * (b[2] - a[2]),
            a[3] + t * (b[3] - a[3]),
        ]
    }
}

/// 两个三维向量线性插值。
pub(crate) fn lerp_vec3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + t * (b[0] - a[0]),
        a[1] + t * (b[1] - a[1]),
        a[2] + t * (b[2] - a[2]),
    ]
}

/// 四元数哈密顿乘法（`a * b`）。
pub(crate) fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

/// 四元数共轭（单位四元数下等价于逆）。
pub(crate) fn quat_conjugate(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

/// 将 `target` 的 TRS 按 `weight` 混合进 `base`。
pub(crate) fn blend_trs(
    base: &mut ([f32; 3], [f32; 4], [f32; 3]),
    target: ([f32; 3], [f32; 4], [f32; 3]),
    weight: f32,
) {
    if weight <= 0.0 {
        return;
    }
    let w = weight.clamp(0.0, 1.0);
    base.0 = lerp_vec3(base.0, target.0, w);
    base.1 = lerp_value(base.1, target.1, w, 1);
    base.2 = lerp_vec3(base.2, target.2, w);
}

/// 把 `target` 相对静止姿态 `rest` 的 TRS 增量按 `weight` 叠加到 `base`。
pub(crate) fn apply_additive_trs(
    base: &mut ([f32; 3], [f32; 4], [f32; 3]),
    target: ([f32; 3], [f32; 4], [f32; 3]),
    rest: ([f32; 3], [f32; 4], [f32; 3]),
    weight: f32,
) {
    if weight <= 0.0 {
        return;
    }
    let w = weight.clamp(0.0, 1.0);
    for i in 0..3 {
        base.0[i] += (target.0[i] - rest.0[i]) * w;
        let ratio = if rest.2[i].abs() > 1e-6 {
            target.2[i] / rest.2[i]
        } else {
            1.0
        };
        base.2[i] *= 1.0 + (ratio - 1.0) * w;
    }
    // 旋转增量 = target * rest^-1，再乘到当前姿态上；按权重 slerp，
    // 保持与位置/缩放一致的「叠加量」语义。
    let delta = quat_mul(target.1, quat_conjugate(rest.1));
    let combined = quat_mul(delta, base.1);
    base.1 = lerp_value(base.1, combined, w, 1);
}

/// 从四元数构建 3x3 旋转矩阵列（返回列向量）
pub(crate) fn quat_to_rot_cols(q: [f32; 4]) -> ([f32; 3], [f32; 3], [f32; 3]) {
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

/// TRS → scenekit Mat4
pub(crate) fn trs_to_mat4(t: [f32; 3], r: [f32; 4], s: [f32; 3]) -> scenekit::Mat4 {
    let (rc0, rc1, rc2) = quat_to_rot_cols(r);
    scenekit::Mat4::from_cols(
        scenekit::Vec4::new(rc0[0] * s[0], rc0[1] * s[0], rc0[2] * s[0], 0.0),
        scenekit::Vec4::new(rc1[0] * s[1], rc1[1] * s[1], rc1[2] * s[1], 0.0),
        scenekit::Vec4::new(rc2[0] * s[2], rc2[1] * s[2], rc2[2] * s[2], 0.0),
        scenekit::Vec4::new(t[0], t[1], t[2], 1.0),
    )
}

/// 4x4 矩阵乘法（scenekit Mat4）
pub(crate) fn mat4_mul(a: &scenekit::Mat4, b: &scenekit::Mat4) -> scenekit::Mat4 {
    let a0 = &a.cols[0];
    let a1 = &a.cols[1];
    let a2 = &a.cols[2];
    let a3 = &a.cols[3];
    let b0 = &b.cols[0];
    let b1 = &b.cols[1];
    let b2 = &b.cols[2];
    let b3 = &b.cols[3];

    let mul_col = |bc: &scenekit::Vec4| -> scenekit::Vec4 {
        scenekit::Vec4::new(
            a0.x * bc.x + a1.x * bc.y + a2.x * bc.z + a3.x * bc.w,
            a0.y * bc.x + a1.y * bc.y + a2.y * bc.z + a3.y * bc.w,
            a0.z * bc.x + a1.z * bc.y + a2.z * bc.z + a3.z * bc.w,
            a0.w * bc.x + a1.w * bc.y + a2.w * bc.z + a3.w * bc.w,
        )
    };
    scenekit::Mat4::from_cols(mul_col(b0), mul_col(b1), mul_col(b2), mul_col(b3))
}

/// 将 scenekit Mat4 转为 [f32; 16]（列主序，用于 GPU 上传）
pub(crate) fn mat4_to_flat(m: &scenekit::Mat4) -> [f32; 16] {
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

pub(crate) fn mat4_identity() -> scenekit::Mat4 {
    scenekit::Mat4::from_cols(
        scenekit::Vec4::new(1.0, 0.0, 0.0, 0.0),
        scenekit::Vec4::new(0.0, 1.0, 0.0, 0.0),
        scenekit::Vec4::new(0.0, 0.0, 1.0, 0.0),
        scenekit::Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}
