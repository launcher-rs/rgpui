//! iOS 显示器桩实现（M4 由 `UIScreen` 回填）。

use rgpui::{Bounds, DisplayId, Pixels, PlatformDisplay, point, px, size};
use uuid::Uuid;

/// iOS 显示器（M1 固定逻辑尺寸桩实现）。
#[derive(Clone, Debug)]
pub struct IosDisplay {
    /// 显示器 ID。
    id: DisplayId,
    /// 逻辑边界（点）。
    bounds: Bounds<Pixels>,
}

impl IosDisplay {
    /// 头模式默认显示器。
    pub fn headless(width: i32, height: i32) -> Self {
        Self {
            id: DisplayId::new(0),
            bounds: Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(width as f32), px(height as f32)),
            },
        }
    }
}

impl PlatformDisplay for IosDisplay {
    fn id(&self) -> DisplayId {
        self.id
    }

    fn uuid(&self) -> anyhow::Result<Uuid> {
        Ok(Uuid::nil())
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }
}
