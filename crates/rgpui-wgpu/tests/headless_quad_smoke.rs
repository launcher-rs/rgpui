// 无头渲染器在 `test-support` feature 后才导出，整套冒烟测试随之门控。
#![cfg(feature = "test-support")]

//! 无头 GPU 渲染冒烟测试：验证 WGSL 编译、深度态与不透明/混合双通道。
//!
//! 不建窗口，把两层 quad（不透明红全屏 + 半透明蓝左上角）渲染进离屏纹理
//! 回读像素——深度缓冲 anti-overdraw 落地后，这是三后端里唯一能在本地
//! 端到端跑通的 shader 管线验证路径。

use rgpui::{
    Bounds, ContentMask, Corners, DevicePixels, Hsla, Point, Quad, ScaledPixels, Scene, Shadow,
    Size, Underline, hsla, solid_background,
};
use rgpui_wgpu::WgpuHeadlessRenderer;

/// 全屏（16x16）边界。
fn full_bounds() -> Bounds<ScaledPixels> {
    Bounds {
        origin: Point {
            x: ScaledPixels(0.0),
            y: ScaledPixels(0.0),
        },
        size: Size {
            width: ScaledPixels(16.0),
            height: ScaledPixels(16.0),
        },
    }
}

/// 指定边界与背景的实心 quad。
fn solid_quad(bounds: Bounds<ScaledPixels>, color: Hsla) -> Quad {
    Quad {
        bounds,
        content_mask: ContentMask { bounds },
        background: solid_background(color),
        border_color: color,
        ..Quad::default()
    }
}

#[test]
fn headless_render_opaque_and_blended_quads() {
    let size = Size {
        width: DevicePixels(16),
        height: DevicePixels(16),
    };
    let mut renderer = WgpuHeadlessRenderer::new(size).expect("无头渲染器创建失败");

    let red = hsla(0.0, 1.0, 0.5, 1.0);
    let blue = hsla(0.66, 1.0, 0.5, 0.5);

    let mut scene = Scene::default();
    // 先画的远、后画的近：不透明红全屏，再叠半透明蓝（左上 8x8）。
    scene.insert_primitive(solid_quad(full_bounds(), red));
    let half = Bounds {
        origin: Point {
            x: ScaledPixels(0.0),
            y: ScaledPixels(0.0),
        },
        size: Size {
            width: ScaledPixels(8.0),
            height: ScaledPixels(8.0),
        },
    };
    scene.insert_primitive(solid_quad(half, blue));
    scene.finish();

    let (width, height, pixels) = renderer
        .render_to_pixels(&scene, size)
        .expect("离屏渲染失败");
    assert_eq!((width, height), (16, 16));

    let px = |x: u32, y: u32| -> [u8; 4] {
        let i = ((y * width + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };
    let near =
        |c: [u8; 4], t: [i16; 4]| -> bool { (0..4).all(|k| (c[k] as i16 - t[k]).abs() <= 40) };

    // 未被蓝覆盖处：不透明红。
    assert!(
        near(px(12, 12), [255, 0, 0, 255]),
        "红矩形应覆盖右下: {:?}",
        px(12, 12)
    );
    // 蓝覆盖处：50% 蓝在红上 → 紫。
    assert!(
        near(px(4, 4), [127, 0, 127, 255]),
        "半透明蓝应叠加在红上: {:?}",
        px(4, 4)
    );
}

/// 阴影 halo 与先画方块重叠时，完整绘制边界必须让阴影排在方块之后。
#[test]
fn headless_shadow_halo_overlaps_earlier_opaque_quad_in_paint_order() {
    let size = Size {
        width: DevicePixels(80),
        height: DevicePixels(64),
    };
    let mut renderer = WgpuHeadlessRenderer::new(size).expect("无头渲染器创建失败");
    let full = Bounds {
        origin: Point {
            x: ScaledPixels(0.0),
            y: ScaledPixels(0.0),
        },
        size: Size {
            width: ScaledPixels(80.0),
            height: ScaledPixels(64.0),
        },
    };
    let earlier = Bounds {
        origin: Point {
            x: ScaledPixels(0.0),
            y: ScaledPixels(20.0),
        },
        size: Size {
            width: ScaledPixels(24.0),
            height: ScaledPixels(24.0),
        },
    };
    let shape = Bounds {
        origin: Point {
            x: ScaledPixels(32.0),
            y: ScaledPixels(20.0),
        },
        size: Size {
            width: ScaledPixels(28.0),
            height: ScaledPixels(24.0),
        },
    };
    let blur_radius = ScaledPixels(12.0);
    let mut scene = Scene::default();
    scene.insert_primitive(solid_quad(earlier, hsla(0.33, 1.0, 0.5, 1.0)));
    scene.insert_primitive(Shadow {
        order: 0,
        blur_radius,
        bounds: shape.dilate(blur_radius * 3.0),
        shape_bounds: shape,
        corner_radii: Corners::default(),
        content_mask: ContentMask { bounds: full },
        color: hsla(0.0, 0.0, 0.0, 0.8),
        element_bounds: shape,
        element_corner_radii: Corners::default(),
        inset: 0,
        pad: 0,
    });
    scene.finish();
    assert!(scene.shadows[0].order > scene.quads[0].order);

    let (width, height, pixels) = renderer
        .render_to_pixels(&scene, size)
        .expect("离屏渲染失败");
    assert_eq!((width, height), (80, 64));
    let i = ((32 * width + 16) * 4) as usize;
    let pixel = [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]];
    assert!(
        pixel[1] < 245 && pixel[0] < 40 && pixel[2] < 40 && pixel[3] >= 250,
        "先画绿色方块上的阴影 halo 应可见: {pixel:?}"
    );
}

/// 外阴影晚于同形状不透明面板绘制时，也不能把模糊后的形状内部涂暗。
#[test]
fn headless_drop_shadow_does_not_darken_its_shape_interior() {
    let size = Size {
        width: DevicePixels(80),
        height: DevicePixels(64),
    };
    let mut renderer = WgpuHeadlessRenderer::new(size).expect("无头渲染器创建失败");
    let full = Bounds {
        origin: Point {
            x: ScaledPixels(0.0),
            y: ScaledPixels(0.0),
        },
        size: Size {
            width: ScaledPixels(80.0),
            height: ScaledPixels(64.0),
        },
    };
    let shape = Bounds {
        origin: Point {
            x: ScaledPixels(24.0),
            y: ScaledPixels(20.0),
        },
        size: Size {
            width: ScaledPixels(32.0),
            height: ScaledPixels(24.0),
        },
    };
    let blur_radius = ScaledPixels(12.0);
    let mut scene = Scene::default();
    scene.insert_primitive(solid_quad(shape, hsla(0.33, 1.0, 0.5, 1.0)));
    scene.insert_primitive(Shadow {
        order: 0,
        blur_radius,
        bounds: shape.dilate(blur_radius * 3.0),
        shape_bounds: shape,
        corner_radii: Corners::default(),
        content_mask: ContentMask { bounds: full },
        color: hsla(0.0, 0.0, 0.0, 0.8),
        element_bounds: shape,
        element_corner_radii: Corners::default(),
        inset: 0,
        pad: 0,
    });
    scene.finish();
    assert!(scene.shadows[0].order > scene.quads[0].order);

    let (width, height, pixels) = renderer
        .render_to_pixels(&scene, size)
        .expect("离屏渲染失败");
    assert_eq!((width, height), (80, 64));
    let px = |x: u32, y: u32| -> [u8; 4] {
        let i = ((y * width + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };
    let center = px(40, 32);
    assert!(
        center[1] >= 245 && center[0] < 40 && center[2] < 40 && center[3] >= 250,
        "外阴影不应涂暗自身形状内部: {center:?}"
    );
    let halo = px(70, 32);
    assert!(
        halo[3] > 0 && halo[3] < 250,
        "形状外侧仍应保留可见的阴影 halo: {halo:?}"
    );
}

/// 多个外阴影必须共用元素边框盒作为裁剪边界，否则双层阴影会被切成两圈分离的环带。
#[test]
fn headless_multiple_drop_shadows_share_element_clip() {
    let size = Size {
        width: DevicePixels(80),
        height: DevicePixels(64),
    };
    let mut renderer = WgpuHeadlessRenderer::new(size).expect("无头渲染器创建失败");
    let full = Bounds {
        origin: Point {
            x: ScaledPixels(0.0),
            y: ScaledPixels(0.0),
        },
        size: Size {
            width: ScaledPixels(80.0),
            height: ScaledPixels(64.0),
        },
    };
    let element = Bounds {
        origin: Point {
            x: ScaledPixels(20.0),
            y: ScaledPixels(20.0),
        },
        size: Size {
            width: ScaledPixels(40.0),
            height: ScaledPixels(20.0),
        },
    };
    // 两层阴影的偏移/扩散不同，但都属于同一个元素。
    let shape_far = Bounds {
        origin: Point {
            x: ScaledPixels(22.0),
            y: ScaledPixels(26.0),
        },
        size: Size {
            width: ScaledPixels(36.0),
            height: ScaledPixels(20.0),
        },
    };
    let shape_near = Bounds {
        origin: Point {
            x: ScaledPixels(21.0),
            y: ScaledPixels(21.0),
        },
        size: Size {
            width: ScaledPixels(38.0),
            height: ScaledPixels(20.0),
        },
    };
    let shadow = |shape: Bounds<ScaledPixels>, blur_radius: ScaledPixels| Shadow {
        order: 0,
        blur_radius,
        bounds: shape.dilate(blur_radius * 3.0),
        shape_bounds: shape,
        corner_radii: Corners::default(),
        content_mask: ContentMask { bounds: full },
        color: hsla(0.0, 0.0, 0.0, 0.8),
        element_bounds: element,
        element_corner_radii: Corners::default(),
        inset: 0,
        pad: 0,
    };

    let mut scene = Scene::default();
    scene.insert_primitive(shadow(shape_far, ScaledPixels(12.0)));
    scene.insert_primitive(shadow(shape_near, ScaledPixels(6.0)));
    scene.finish();

    let (width, _height, pixels) = renderer
        .render_to_pixels(&scene, size)
        .expect("离屏渲染失败");
    let px = |x: u32, y: u32| -> [u8; 4] {
        let i = ((y * width + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };

    // 元素边框盒内部（即使在某层阴影形状之外）不允许出现阴影。
    assert_eq!(px(21, 30)[3], 0, "元素内部不应出现阴影环: {:?}", px(21, 30));
    // 元素外侧应留有连续阴影；两层阴影的裁剪线不能形成突变环带。
    let above = px(40, 40)[3];
    let below = px(40, 41)[3];
    assert!(
        above > 0 && below > 0,
        "元素下方应可见阴影: {above}/{below}"
    );
    assert!(
        (above as i16 - below as i16).abs() <= 24,
        "两层阴影不应在各自形状边界处突变: {above}/{below}"
    );
}

/// 视口深度游标（D2 backlog）：插入更晚的不透明矩形必须盖住先画的非 quad 图元。
/// 修复前非 quad 用 Always 深度态，会浮在最先绘制的不透明通道之上（z-order 反转）。
#[test]
fn headless_opaque_quad_occludes_earlier_underline() {
    let size = Size {
        width: DevicePixels(16),
        height: DevicePixels(16),
    };
    let mut renderer = WgpuHeadlessRenderer::new(size).expect("无头渲染器创建失败");

    let bounds = full_bounds();
    let mut scene = Scene::default();
    // 先插红色下划线（非 quad，绘制序更早），再插覆盖它的不透明绿色矩形。
    scene.insert_primitive(Underline {
        order: 0,
        pad: 0,
        bounds,
        content_mask: ContentMask { bounds },
        color: hsla(0.0, 1.0, 0.5, 1.0), // 红
        thickness: ScaledPixels(16.0),
        wavy: 0,
    });
    scene.insert_primitive(solid_quad(
        bounds,
        hsla(0.33, 1.0, 0.5, 1.0), // 绿
    ));
    scene.finish();

    let (width, height, pixels) = renderer
        .render_to_pixels(&scene, size)
        .expect("离屏渲染失败");
    assert_eq!((width, height), (16, 16));
    let px = |x: u32, y: u32| -> [u8; 4] {
        let i = ((y * width + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };
    let near =
        |c: [u8; 4], t: [i16; 4]| -> bool { (0..4).all(|k| (c[k] as i16 - t[k]).abs() <= 40) };

    assert!(
        near(px(8, 8), [0, 255, 0, 255]),
        "后画的不透明绿矩形应盖住先画的红下划线: {:?}",
        px(8, 8)
    );
}
