#![cfg(test)]

//! 验证所有 GPU 传输结构体的内存布局与 WGSL 着色器中的定义完全匹配。
//! 这是防止渲染错误的关键守卫——布局不匹配会导致 GPU 读取错误数据。

use rgpui::{Background, Hsla, Quad, Shadow, TransformationMatrix};
use std::mem::{align_of, offset_of, size_of};

#[test]
fn verify_hsla_layout() {
    // WGSL: struct Hsla { h: f32, s: f32, l: f32, a: f32 }
    assert_eq!(size_of::<Hsla>(), 16, "Hsla 大小必须为 16 字节");
    assert_eq!(align_of::<Hsla>(), 4);
}

#[test]
fn verify_background_total_size() {
    // WGSL Background 总大小必须为 136 字节，align 8
    assert_eq!(
        size_of::<Background>(),
        136,
        "Background 大小必须为 136 字节"
    );
    assert_eq!(align_of::<Background>(), 8, "Background 对齐必须为 8");
}

#[test]
fn verify_transformation_matrix_layout() {
    // WGSL: struct TransformationMatrix {
    //     rotation_scale: mat2x2<f32>,  // offset 0, size 16, align 8
    //     translation: vec2<f32>,       // offset 16, size 8, align 8
    // }
    assert_eq!(size_of::<TransformationMatrix>(), 24);
    assert_eq!(align_of::<TransformationMatrix>(), 8);
    assert_eq!(offset_of!(TransformationMatrix, rotation_scale), 0);
    assert_eq!(offset_of!(TransformationMatrix, translation), 16);
}

#[test]
fn verify_quad_layout() {
    // WGSL Quad 布局（所有偏移量必须精确匹配）:
    // order: u32                        offset 0
    // border_style: u32                 offset 4
    // bounds: Bounds                    offset 8
    // content_mask: Bounds              offset 24
    // background: Background            offset 40  (align 8, 40 % 8 == 0)
    // border_color: Hsla                offset 176
    // corner_radii: Corners             offset 192
    // border_widths: Edges              offset 208
    // continuous_corners: u32            offset 224
    // [padding 4 bytes]                 offset 228
    // transform: TransformationMatrix   offset 232 (align 8)
    // blend_mode: u32                   offset 256
    // pad_quad: u32                     offset 260
    // 总大小: 264, align: 8
    assert_eq!(size_of::<Quad>(), 264, "Quad 大小必须为 264 字节");
    assert_eq!(align_of::<Quad>(), 8, "Quad 对齐必须为 8");
    assert_eq!(offset_of!(Quad, order), 0);
    assert_eq!(offset_of!(Quad, border_style), 4);
    assert_eq!(offset_of!(Quad, bounds), 8);
    assert_eq!(offset_of!(Quad, content_mask), 24);
    assert_eq!(offset_of!(Quad, background), 40);
    assert_eq!(offset_of!(Quad, border_color), 176);
    assert_eq!(offset_of!(Quad, corner_radii), 192);
    assert_eq!(offset_of!(Quad, border_widths), 208);
    assert_eq!(offset_of!(Quad, continuous_corners), 224);
    assert_eq!(offset_of!(Quad, transform), 232);
    assert_eq!(offset_of!(Quad, blend_mode), 256);
    assert_eq!(offset_of!(Quad, pad_quad), 260);
}

#[test]
fn verify_shadow_layout() {
    // WGSL Shadow 布局:
    // order: u32                        offset 0
    // blur_radius: f32                  offset 4
    // bounds: Bounds                    offset 8
    // corner_radii: Corners             offset 24
    // content_mask: Bounds              offset 40
    // color: Hsla                       offset 56
    // element_bounds: Bounds            offset 72
    // element_corner_radii: Corners     offset 88
    // inset: u32                        offset 104
    // pad: u32                          offset 108
    // 总大小: 112
    assert_eq!(size_of::<Shadow>(), 112, "Shadow 大小必须为 112 字节");
    assert_eq!(offset_of!(Shadow, order), 0);
    assert_eq!(offset_of!(Shadow, blur_radius), 4);
    assert_eq!(offset_of!(Shadow, bounds), 8);
    assert_eq!(offset_of!(Shadow, corner_radii), 24);
    assert_eq!(offset_of!(Shadow, content_mask), 40);
    assert_eq!(offset_of!(Shadow, color), 56);
    assert_eq!(offset_of!(Shadow, element_bounds), 72);
    assert_eq!(offset_of!(Shadow, element_corner_radii), 88);
    assert_eq!(offset_of!(Shadow, inset), 104);
}
