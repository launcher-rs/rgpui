// 无头渲染器在 `test-support` feature 后才导出，整套冒烟测试随之门控。
#![cfg(feature = "test-support")]

//! 无头 GPU 渲染冒烟测试：验证 WGSL 编译、深度态与不透明/混合双通道。
//!
//! 不建窗口，把两层 quad（不透明红全屏 + 半透明蓝左上角）渲染进离屏纹理
//! 回读像素——深度缓冲 anti-overdraw 落地后，这是三后端里唯一能在本地
//! 端到端跑通的 shader 管线验证路径。

use rgpui::{
    Bounds, ContentMask, DevicePixels, Hsla, Point, Quad, ScaledPixels, Scene, Size, hsla,
    solid_background,
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
