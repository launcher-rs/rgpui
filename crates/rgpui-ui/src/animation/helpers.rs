//! 动画插值辅助函数。

use rgpui::{Hsla, Pixels, px};

/// 在像素值之间线性插值（`t` 会被钳制到 [0, 1]）。
pub fn lerp_pixels(from: Pixels, to: Pixels, t: f32) -> Pixels {
    let t = t.clamp(0.0, 1.0);
    px(f32::from(from) + (f32::from(to) - f32::from(from)) * t)
}

/// 在颜色之间线性插值（色相 h、饱和度 s、明度 l、透明度 a 各自插值）。
pub fn lerp_color(from: Hsla, to: Hsla, t: f32) -> Hsla {
    let t = t.clamp(0.0, 1.0);
    Hsla {
        h: from.h + (to.h - from.h) * t,
        s: from.s + (to.s - from.s) * t,
        l: from.l + (to.l - from.l) * t,
        a: from.a + (to.a - from.a) * t,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证像素插值在端点与中点正确。
    #[test]
    fn test_lerp_pixels() {
        let from = px(0.0);
        let to = px(100.0);
        assert_eq!(lerp_pixels(from, to, 0.0), px(0.0));
        assert_eq!(lerp_pixels(from, to, 1.0), px(100.0));
        assert_eq!(lerp_pixels(from, to, 0.5), px(50.0));
        // 越界 t 会被钳制
        assert_eq!(lerp_pixels(from, to, 2.0), px(100.0));
    }

    /// 验证颜色插值在端点正确。
    #[test]
    fn test_lerp_color() {
        let from = Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.0,
            a: 0.0,
        };
        let to = Hsla {
            h: 1.0,
            s: 1.0,
            l: 1.0,
            a: 1.0,
        };
        let mid = lerp_color(from, to, 0.5);
        assert!((mid.h - 0.5).abs() < 1e-6);
        assert!((mid.a - 0.5).abs() < 1e-6);
    }
}
