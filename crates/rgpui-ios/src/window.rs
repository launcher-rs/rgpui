//! iOS 窗口桩实现（M1：固定逻辑尺寸 + 空渲染，M4 接 `UIWindow` + Metal）。

use parking_lot::Mutex;
use raw_window_handle::{HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle};
use rgpui::{
    AtlasKey, AtlasTextureId, AtlasTile, Bounds, Capslock, DevicePixels, DispatchEventResult,
    GpuSpecs, Modifiers, Pixels, PlatformAtlas, PlatformDisplay, PlatformInput,
    PlatformInputHandler, PlatformWindow, PromptButton, PromptLevel, RequestFrameOptions, Scene,
    Size, TileId, WindowAppearance, WindowBackgroundAppearance, WindowBounds, WindowControlArea,
    point, px, size,
};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// iOS 原生窗口句柄（M1 仅保留几何信息）。
pub struct IosWindow {
    /// 逻辑宽度（点）。
    width: f32,
    /// 逻辑高度（点）。
    height: f32,
    /// 缩放因子。
    scale_factor: f32,
    /// 是否处于前台。
    active: std::sync::atomic::AtomicBool,
}

impl IosWindow {
    /// 构造固定逻辑尺寸的桩窗口。
    pub fn headless(width: f32, height: f32, scale_factor: f32) -> Arc<Self> {
        Arc::new(Self {
            width,
            height,
            scale_factor,
            active: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// 返回缩放因子。
    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    /// 窗口是否处于前台。
    pub fn is_active(&self) -> bool {
        self.active.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 设置前后台状态。
    pub fn set_active(&self, active: bool) {
        self.active
            .store(active, std::sync::atomic::Ordering::Relaxed);
    }
}

/// 分配图集瓦片但不上屏的空图集。
#[derive(Default)]
struct StubAtlas(Mutex<StubAtlasState>);

#[derive(Default)]
struct StubAtlasState {
    /// 下一个纹理/瓦片序号。
    next_id: u32,
    /// 已分配瓦片。
    tiles: HashMap<AtlasKey, AtlasTile>,
}

impl PlatformAtlas for StubAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<Option<AtlasTile>> {
        {
            let state = self.0.lock();
            if let Some(tile) = state.tiles.get(key) {
                return Ok(Some(*tile));
            }
        }
        let Some((size, _)) = build()? else {
            return Ok(None);
        };
        let mut state = self.0.lock();
        state.next_id += 1;
        let texture_id = state.next_id;
        state.next_id += 1;
        let tile_id = state.next_id;
        let tile = AtlasTile {
            texture_id: AtlasTextureId {
                index: texture_id,
                kind: key.texture_kind(),
            },
            tile_id: TileId(tile_id),
            padding: 0,
            bounds: Bounds {
                origin: point(DevicePixels(0), DevicePixels(0)),
                size,
            },
        };
        state.tiles.insert(key.clone(), tile);
        Ok(Some(tile))
    }

    fn remove(&self, key: &AtlasKey) {
        self.0.lock().tiles.remove(key);
    }
}

/// 实现 `PlatformWindow` 的包装器。
pub struct IosPlatformWindow {
    /// 底层窗口。
    window: Arc<IosWindow>,
    /// 所属显示器。
    display: Option<Rc<dyn PlatformDisplay>>,
    /// 窗口标题。
    title: String,
}

impl IosPlatformWindow {
    /// 包装已有窗口与显示器。
    pub fn new(window: Arc<IosWindow>, display: Option<Rc<dyn PlatformDisplay>>) -> Self {
        Self {
            window,
            display,
            title: String::new(),
        }
    }
}

impl HasWindowHandle for IosPlatformWindow {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

impl HasDisplayHandle for IosPlatformWindow {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, HandleError> {
        Ok(unsafe {
            raw_window_handle::DisplayHandle::borrow_raw(RawDisplayHandle::UiKit(
                raw_window_handle::UiKitDisplayHandle::new(),
            ))
        })
    }
}

impl PlatformWindow for IosPlatformWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(self.window.width), px(self.window.height)),
        }
    }

    fn is_maximized(&self) -> bool {
        true
    }

    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Fullscreen(self.bounds())
    }

    fn content_size(&self) -> Size<Pixels> {
        self.bounds().size
    }

    fn resize(&mut self, _size: Size<Pixels>) {}

    fn scale_factor(&self) -> f32 {
        self.window.scale_factor()
    }

    fn appearance(&self) -> WindowAppearance {
        WindowAppearance::Light
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.display.clone()
    }

    fn mouse_position(&self) -> rgpui::Point<Pixels> {
        point(px(0.0), px(0.0))
    }

    fn modifiers(&self) -> Modifiers {
        Modifiers::default()
    }

    fn capslock(&self) -> Capslock {
        Capslock::default()
    }

    fn set_input_handler(&mut self, _input_handler: PlatformInputHandler) {}

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        None
    }

    fn prompt(
        &self,
        _level: PromptLevel,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[PromptButton],
    ) -> Option<futures::channel::oneshot::Receiver<usize>> {
        let (_tx, rx) = futures::channel::oneshot::channel();
        Some(rx)
    }

    fn activate(&self) {
        self.window.set_active(true);
    }

    fn is_active(&self) -> bool {
        self.window.is_active()
    }

    fn is_hovered(&self) -> bool {
        false
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }

    fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    fn set_background_appearance(&self, _background_appearance: WindowBackgroundAppearance) {}

    fn minimize(&self) {}

    fn zoom(&self) {}

    fn toggle_fullscreen(&self) {}

    fn is_fullscreen(&self) -> bool {
        true
    }

    fn on_request_frame(&self, _callback: Box<dyn FnMut(RequestFrameOptions)>) {}

    fn on_input(&self, _callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {}

    fn on_active_status_change(&self, _callback: Box<dyn FnMut(bool)>) {}

    fn on_hover_status_change(&self, _callback: Box<dyn FnMut(bool)>) {}

    fn on_resize(&self, _callback: Box<dyn FnMut(Size<Pixels>, f32)>) {}

    fn on_moved(&self, _callback: Box<dyn FnMut()>) {}

    fn on_should_close(&self, _callback: Box<dyn FnMut() -> bool>) {}

    fn on_hit_test_window_control(&self, _callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
    }

    fn on_close(&self, _callback: Box<dyn FnOnce()>) {}

    fn on_appearance_changed(&self, _callback: Box<dyn FnMut()>) {}

    fn draw(&self, _scene: &Scene) {}

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        Arc::new(StubAtlas::default())
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        None
    }

    fn update_ime_position(&self, _bounds: Bounds<Pixels>) {}

    fn get_title(&self) -> String {
        self.title.clone()
    }

    fn inner_window_bounds(&self) -> WindowBounds {
        self.window_bounds()
    }

    /// 返回空 HWND（仅 Windows 主机编译桩窗口时满足 trait，移动端无窗口句柄）。
    #[cfg(target_os = "windows")]
    fn get_raw_handle(&self) -> windows::Win32::Foundation::HWND {
        windows::Win32::Foundation::HWND(std::ptr::null_mut())
    }
}
