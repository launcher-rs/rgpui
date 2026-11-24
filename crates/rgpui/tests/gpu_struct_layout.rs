#![cfg(test)]

//! 验证所有 GPU 传输结构体的内存布局与 WGSL 着色器中的定义完全匹配。
//! 这是防止渲染错误的关键守卫——布局不匹配会导致 GPU 读取错误数据。

use rgpui::{Background, Hsla, PathCurve, PathPaint, PathTile, Quad, Shadow, TransformationMatrix};
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
    // depth: f32                        offset 260
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
    assert_eq!(offset_of!(Quad, depth), 260);
}

#[test]
fn verify_shadow_layout() {
    // WGSL Shadow 布局:
    // order: u32                        offset 0
    // blur_radius: f32                  offset 4
    // bounds: Bounds                    offset 8
    // shape_bounds: Bounds             offset 24
    // corner_radii: Corners             offset 40
    // content_mask: Bounds              offset 56
    // color: Hsla                       offset 72
    // element_bounds: Bounds            offset 88
    // element_corner_radii: Corners     offset 104
    // inset: u32                        offset 120
    // pad: u32                          offset 124
    // 总大小: 128
    assert_eq!(size_of::<Shadow>(), 128, "Shadow 大小必须为 128 字节");
    assert_eq!(offset_of!(Shadow, order), 0);
    assert_eq!(offset_of!(Shadow, blur_radius), 4);
    assert_eq!(offset_of!(Shadow, bounds), 8);
    assert_eq!(offset_of!(Shadow, shape_bounds), 24);
    assert_eq!(offset_of!(Shadow, corner_radii), 40);
    assert_eq!(offset_of!(Shadow, content_mask), 56);
    assert_eq!(offset_of!(Shadow, color), 72);
    assert_eq!(offset_of!(Shadow, element_bounds), 88);
    assert_eq!(offset_of!(Shadow, element_corner_radii), 104);
    assert_eq!(offset_of!(Shadow, inset), 120);
}

#[test]
fn verify_path_curve_layout() {
    // WGSL/HLSL/Metal PathCurve 布局（环绕数路径渲染，D3）:
    // p0: vec2<f32>    offset 0
    // p1: vec2<f32>    offset 8
    // a: vec2<f32>     offset 16
    // b: vec2<f32>     offset 24
    // sign: f32        offset 32
    // pad: u32         offset 36
    // 总大小: 40
    assert_eq!(size_of::<PathCurve>(), 40, "PathCurve 大小必须为 40 字节");
    assert_eq!(align_of::<PathCurve>(), 4, "PathCurve 对齐必须为 4");
    assert_eq!(offset_of!(PathCurve, p0), 0);
    assert_eq!(offset_of!(PathCurve, p1), 8);
    assert_eq!(offset_of!(PathCurve, a), 16);
    assert_eq!(offset_of!(PathCurve, b), 24);
    assert_eq!(offset_of!(PathCurve, sign), 32);
    assert_eq!(offset_of!(PathCurve, pad), 36);
}

#[test]
fn verify_path_tile_layout() {
    // WGSL/HLSL/Metal PathTile 布局:
    // paint: u32                       offset 0
    // curve_start: u32                 offset 4
    // curve_count: u32                 offset 8
    // backdrop: i32                    offset 12
    // top_left: vec2<f32>              offset 16
    // bottom_right: vec2<f32>          offset 24
    // reference: vec2<f32>             offset 32
    // curves_top_left: vec2<f32>       offset 40
    // curves_bottom_right: vec2<f32>   offset 48
    // 总大小: 56
    assert_eq!(size_of::<PathTile>(), 56, "PathTile 大小必须为 56 字节");
    assert_eq!(align_of::<PathTile>(), 4, "PathTile 对齐必须为 4");
    assert_eq!(offset_of!(PathTile, paint), 0);
    assert_eq!(offset_of!(PathTile, curve_start), 4);
    assert_eq!(offset_of!(PathTile, curve_count), 8);
    assert_eq!(offset_of!(PathTile, backdrop), 12);
    assert_eq!(offset_of!(PathTile, top_left), 16);
    assert_eq!(offset_of!(PathTile, bottom_right), 24);
    assert_eq!(offset_of!(PathTile, reference), 32);
    assert_eq!(offset_of!(PathTile, curves_top_left), 40);
    assert_eq!(offset_of!(PathTile, curves_bottom_right), 48);
}

#[test]
fn verify_path_paint_layout() {
    // WGSL/HLSL/Metal PathPaint 布局（rgpui Background 为 136 字节，比上游大）:
    // bounds: Bounds                   offset 0
    // content_mask: Bounds             offset 16
    // color: Background                offset 32 (align 8)
    // even_odd: u32                    offset 168
    // pad: u32                         offset 172
    // 总大小: 176
    assert_eq!(size_of::<PathPaint>(), 176, "PathPaint 大小必须为 176 字节");
    assert_eq!(align_of::<PathPaint>(), 8, "PathPaint 对齐必须为 8");
    assert_eq!(offset_of!(PathPaint, bounds), 0);
    assert_eq!(offset_of!(PathPaint, content_mask), 16);
    assert_eq!(offset_of!(PathPaint, color), 32);
    assert_eq!(offset_of!(PathPaint, even_odd), 168);
    assert_eq!(offset_of!(PathPaint, pad), 172);
}
