//! X11 显示器模块
//!
//! 提供 X11 环境下的显示器抽象实现

use anyhow::Context as _;
use uuid::Uuid;
use x11rb::{connection::Connection as _, xcb_ffi::XCBConnection};

use rgpui::{Bounds, DisplayId, Pixels, PlatformDisplay, Size, px};

/// X11 显示器实现
#[derive(Debug)]
pub(crate) struct X11Display {
    x_screen_index: usize,
    bounds: Bounds<Pixels>,
    uuid: Uuid,
}

impl X11Display {
    /// 创建新的 X11 显示器实例
    ///
    /// # 参数
    ///
    /// * `xcb` - XCB 连接
    /// * `scale_factor` - 缩放因子
    /// * `x_screen_index` - X 屏幕索引
    pub(crate) fn new(
        xcb: &XCBConnection,
        scale_factor: f32,
        x_screen_index: usize,
    ) -> anyhow::Result<Self> {
        let screen = xcb
            .setup()
            .roots
            .get(x_screen_index)
            .with_context(|| format!("No screen found with index {x_screen_index}"))?;
        Ok(Self {
            x_screen_index,
            bounds: Bounds {
                origin: Default::default(),
                size: Size {
                    width: px(screen.width_in_pixels as f32 / scale_factor),
                    height: px(screen.height_in_pixels as f32 / scale_factor),
                },
            },
            uuid: Uuid::from_bytes([0; 16]),
        })
    }
}

impl PlatformDisplay for X11Display {
    fn id(&self) -> DisplayId {
        DisplayId::new(self.x_screen_index as u64)
    }

    fn uuid(&self) -> anyhow::Result<Uuid> {
        Ok(self.uuid)
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }
}
