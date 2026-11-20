// 无头渲染器在 `test-support` feature 后才导出，整套冒烟测试随之门控。
#![cfg(feature = "test-support")]

//! 环绕数路径渲染（D3）的无头 GPU 冒烟测试：验证 WGSL 编译与
//! 「CPU 分桶瓦片 + shader 逐像素环绕数」管线端到端出正确像素。
//!
//! 三角形填充走 `PathBuilder::fill`（even-odd），横线描边走
//! `PathBuilder::stroke`（tiny-skia 轮廓展开）。中心取点避开抗锯齿边缘，
//! 断言内部实心、外部透明。

use rgpui::{
    Bounds, ContentMask, DevicePixels, PaintedPath, PathBuilder, Pixels, Scene, Size, hsla, point,
    px, size, solid_background,
};
use rgpui_wgpu::WgpuHeadlessRenderer;

/// 全屏（16x16）边界（逻辑像素，PaintedPath::new 按 scale=1 缩放）。
fn full_bounds() -> Bounds<Pixels> {
    Bounds {
        origin: point(px(0.), px(0.)),
        size: size(px(16.), px(16.)),
    }
}

fn renderer() -> WgpuHeadlessRenderer {
    WgpuHeadlessRenderer::new(Size {
        width: DevicePixels(16),
        height: DevicePixels(16),
    })
    .expect("无头渲染器创建失败")
}

fn render_path(path: rgpui::Path, renderer: &mut WgpuHeadlessRenderer) -> image::RgbaImage {
    let mut scene = Scene::default();
    scene.insert_primitive(PaintedPath::new(
        path,
        1.0,
        ContentMask {
            bounds: full_bounds(),
        },
        solid_background(hsla(0.0, 1.0, 0.5, 1.0)),
    ));
    scene.finish();

    let (width, height, pixels) = renderer
        .render_to_pixels(
            &scene,
            Size {
                width: DevicePixels(16),
                height: DevicePixels(16),
            },
        )
        .expect("离屏渲染失败");
    assert_eq!((width, height), (16, 16));
    image::RgbaImage::from_raw(16, 16, pixels).expect("像素尺寸与图像不符")
}

/// 三角形填充：内部全红不透明，外部透明（含三个角附近）。
#[test]
fn headless_render_path_triangle_fill() {
    let mut renderer = renderer();

    let mut builder = PathBuilder::fill();
    builder.add_polygon(
        &[
            point(px(2.), px(2.)),
            point(px(14.), px(2.)),
            point(px(8.), px(14.)),
        ],
        true,
    );
    let path = builder.build().expect("三角形路径应能构建");

    let image = render_path(path, &mut renderer);
    let px = |x: u32, y: u32| -> [u8; 4] {
        let p = image.get_pixel(x, y).0;
        p
    };

    // 内部点（离三条边都超过 1px）：红色、不透明。
    let inside = px(8, 8);
    assert!(
        inside[0] >= 215 && inside[1] <= 40 && inside[2] <= 40 && inside[3] >= 215,
        "三角形内部应为红色不透明，实际 {inside:?}"
    );
    // 外部点：透明。
    for (x, y) in [(0u32, 0u32), (15, 0), (0, 15), (1, 1), (14, 13)] {
        let outside = px(x, y);
        assert_eq!(
            outside[3], 0,
            "三角形外部 ({x},{y}) 应透明，实际 {outside:?}"
        );
    }
}

/// 横线描边（宽 2）：线内填充、线外透明。
#[test]
fn headless_render_path_stroke_line() {
    let mut renderer = renderer();

    let mut builder = PathBuilder::stroke(px(2.));
    builder.move_to(point(px(3.), px(8.)));
    builder.line_to(point(px(13.), px(8.)));
    let path = builder.build().expect("描边路径应能构建");

    let image = render_path(path, &mut renderer);
    let px = |x: u32, y: u32| image.get_pixel(x, y).0;

    // 线中央（y=8 在描边带内）：红色、不透明。
    let on_line = px(8, 8);
    assert!(
        on_line[0] >= 215 && on_line[1] <= 40 && on_line[2] <= 40 && on_line[3] >= 215,
        "描边带内应为红色不透明，实际 {on_line:?}"
    );
    // 线外（y=4 与 y=13 都远离描边带）：透明。
    for (x, y) in [(8u32, 4u32), (8, 13), (0, 0)] {
        let outside = px(x, y);
        assert_eq!(outside[3], 0, "描边带外 ({x},{y}) 应透明，实际 {outside:?}");
    }
}
