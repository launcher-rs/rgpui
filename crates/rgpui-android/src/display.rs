//! Android 显示器：几何取自系统给的 `ANativeWindow`，密度经 `AConfiguration`。
//!
//! NDK 的 C 层没有多显示器 API；折叠屏/外接屏表现为多个原生窗口，
//! 即多个 `AndroidDisplay` 实例。

use rgpui::{Bounds, DisplayId, Pixels, PlatformDisplay, point, px, size};
use uuid::Uuid;

/// Android 基准密度（mdpi，160dpi；`scale = dpi / 160`）。
const DENSITY_DEFAULT: i32 = 160;

/// 单块逻辑屏（包一层原生窗口引用计数句柄，真机/桩二态）。
#[derive(Clone, Debug)]
pub struct AndroidDisplay {
    /// 真机原生窗口（主机桩为空）。
    #[cfg(target_os = "android")]
    window: Option<ndk::native_window::NativeWindow>,
    /// 相对 160dpi 的缩放（如 2.0 = xhdpi）。
    scale_factor: f32,
    /// 稳定数字标识（真机取窗口指针，桩取尺寸拼装）。
    id: u64,
    /// 桩显示器物理尺寸（真机实时读窗口）。
    #[cfg(not(target_os = "android"))]
    stub_size: (i32, i32),
}

impl AndroidDisplay {
    /// 头模式显示器（主机单测 + 无 surface 时的占位）。
    pub fn headless(width: i32, height: i32) -> Self {
        Self {
            #[cfg(target_os = "android")]
            window: None,
            scale_factor: 2.0,
            id: ((width as u64) << 32) | (height as u64),
            #[cfg(not(target_os = "android"))]
            stub_size: (width, height),
        }
    }

    /// 物理宽（设备像素）。
    pub fn physical_width(&self) -> i32 {
        #[cfg(target_os = "android")]
        {
            self.window
                .as_ref()
                .map(|window| window.width())
                .unwrap_or(0)
        }
        #[cfg(not(target_os = "android"))]
        {
            self.stub_size.0
        }
    }

    /// 物理高（设备像素）。
    pub fn physical_height(&self) -> i32 {
        #[cfg(target_os = "android")]
        {
            self.window
                .as_ref()
                .map(|window| window.height())
                .unwrap_or(0)
        }
        #[cfg(not(target_os = "android"))]
        {
            self.stub_size.1
        }
    }

    /// 显示缩放因子（设备像素 / 逻辑像素）。
    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    /// 近似 плотности（dpi，四舍五入）。
    pub fn dpi(&self) -> i32 {
        (self.scale_factor * DENSITY_DEFAULT as f32).round() as i32
    }

    /// 是否有真实原生窗口。
    pub fn is_real(&self) -> bool {
        #[cfg(target_os = "android")]
        {
            self.window.is_some()
        }
        #[cfg(not(target_os = "android"))]
        {
            false
        }
    }
}

/// 真机构造（仅 Android）：由原生窗口 + density 建显示器。
#[cfg(target_os = "android")]
impl AndroidDisplay {
    /// 由 `NativeWindow` 与 `AssetManager` 查密度构造（`INIT_WINDOW` 时调用）。
    pub fn from_activity(
        window: &ndk::native_window::NativeWindow,
        asset_manager: &ndk::asset::AssetManager,
    ) -> Self {
        let config = ndk::configuration::Configuration::from_asset_manager(asset_manager);
        let density_dpi = config.density().unwrap_or(DENSITY_DEFAULT as u32) as i32;
        let density = if density_dpi > 0 {
            density_dpi
        } else {
            DENSITY_DEFAULT
        };
        Self {
            window: Some(window.clone()),
            scale_factor: density as f32 / DENSITY_DEFAULT as f32,
            id: window.ptr().as_ptr() as u64,
        }
    }
}

impl PlatformDisplay for AndroidDisplay {
    fn id(&self) -> DisplayId {
        DisplayId::new(self.id)
    }

    fn uuid(&self) -> anyhow::Result<Uuid> {
        // NDK 不给显示器稳定 UUID：用窗口指针派生进程内确定的 v5。
        let name = format!("android-display-{:#x}", self.id);
        Ok(Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes()))
    }

    fn bounds(&self) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(
                px(self.physical_width() as f32 / self.scale_factor),
                px(self.physical_height() as f32 / self.scale_factor),
            ),
        }
    }
}

/// 单显示器列表（直板机永远一个，折叠屏可能两个）。
#[derive(Default)]
pub struct DisplayList {
    /// 已连接显示器。
    displays: Vec<AndroidDisplay>,
}

impl DisplayList {
    /// 建只含主屏的列表。
    pub fn single(display: AndroidDisplay) -> Self {
        Self {
            displays: vec![display],
        }
    }

    /// 取主屏。
    pub fn primary(&self) -> Option<&AndroidDisplay> {
        self.displays.first()
    }

    /// 取全部。
    pub fn all(&self) -> &[AndroidDisplay] {
        &self.displays
    }

    /// 显示器数量。
    pub fn len(&self) -> usize {
        self.displays.len()
    }

    /// 是否空。
    pub fn is_empty(&self) -> bool {
        self.displays.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 桩显示器几何：物理尺寸即构造尺寸，非真实窗口。
    #[test]
    fn headless_display_geometry() {
        let display = AndroidDisplay::headless(1080, 1920);
        assert_eq!(display.physical_width(), 1080);
        assert_eq!(display.physical_height(), 1920);
        assert!(!display.is_real());
    }

    /// 桩显示器缩放固定 2.0（xhdpi，常见真机档）。
    #[test]
    fn headless_scale_factor_is_two() {
        let display = AndroidDisplay::headless(1080, 1920);
        assert!((display.scale_factor() - 2.0).abs() < f32::EPSILON);
        assert_eq!(display.dpi(), 320);
    }

    /// 逻辑边界 = 物理 / 缩放。
    #[test]
    fn logical_bounds_divide_by_scale() {
        let bounds = AndroidDisplay::headless(1080, 1920).bounds();
        assert!((f32::from(bounds.size.width) - 540.0).abs() < f32::EPSILON);
        assert!((f32::from(bounds.size.height) - 960.0).abs() < f32::EPSILON);
    }

    /// 不同尺寸 id 不同。
    #[test]
    fn display_id_differs_for_different_sizes() {
        let first = AndroidDisplay::headless(1080, 1920);
        let second = AndroidDisplay::headless(1440, 2560);
        assert_ne!(first.id(), second.id());
    }

    /// 主屏列表行为。
    #[test]
    fn display_list_primary() {
        let list = DisplayList::single(AndroidDisplay::headless(1080, 1920));
        assert!(!list.is_empty());
        assert_eq!(list.len(), 1);
        assert_eq!(list.primary().expect("主屏").physical_width(), 1080);
    }
}
