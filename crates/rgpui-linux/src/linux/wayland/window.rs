use std::{
    cell::{Cell, Ref, RefCell, RefMut},
    ffi::c_void,
    ptr::NonNull,
    rc::Rc,
    sync::Arc,
};

use futures::channel::oneshot::Receiver;
use rgpui::collections::{FxHashMap, HashMap};

use raw_window_handle as rwh;
use wayland_backend::client::ObjectId;
use wayland_client::{Connection, WEnum};
use wayland_client::{
    Proxy,
    protocol::{wl_output, wl_seat, wl_surface},
};
use wayland_protocols::wp::viewporter::client::wp_viewport;
use wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1;
use wayland_protocols::xdg::shell::client::xdg_popup;
use wayland_protocols::xdg::shell::client::xdg_positioner;
use wayland_protocols::xdg::shell::client::xdg_surface;
use wayland_protocols::xdg::shell::client::xdg_toplevel::{self};
use wayland_protocols::{
    wp::fractional_scale::v1::client::wp_fractional_scale_v1,
    xdg::dialog::v1::client::xdg_dialog_v1::XdgDialogV1,
};
use wayland_protocols_plasma::blur::client::org_kde_kwin_blur;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1;

use super::mapping::WindowMapping;
use crate::linux::wayland::{display::WaylandDisplay, serial::SerialKind};
use crate::linux::{Globals, Output, WaylandClientStatePtr, get_window};
use rgpui::{
    AnyWindowHandle, Bounds, Capslock, Decorations, DevicePixels, GpuSpecs, Modifiers, Pixels,
    PlatformAtlas, PlatformDisplay, PlatformInput, PlatformInputHandler, PlatformWindow, Point,
    PromptButton, PromptLevel, RequestFrameOptions, ResizeEdge, Scene, Size, Tiling,
    WindowAppearance, WindowBackgroundAppearance, WindowBounds, WindowControlArea, WindowControls,
    WindowDecorations, WindowKind, WindowParams, layer_shell::Anchor,
    layer_shell::LayerShellNotSupportedError, popup::PopupOptions, px, size,
};
use rgpui_wgpu::{CompositorGpuHint, WgpuRenderer, WgpuSurfaceConfig, wgpu};

#[derive(Default)]
pub(crate) struct Callbacks {
    request_frame: Option<Box<dyn FnMut(RequestFrameOptions)>>,
    input: Option<Box<dyn FnMut(rgpui::PlatformInput) -> rgpui::DispatchEventResult>>,
    active_status_change: Option<Box<dyn FnMut(bool)>>,
    hover_status_change: Option<Box<dyn FnMut(bool)>>,
    resize: Option<Box<dyn FnMut(Size<Pixels>, f32)>>,
    moved: Option<Box<dyn FnMut()>>,
    should_close: Option<Box<dyn FnMut() -> bool>>,
    close: Option<Box<dyn FnOnce()>>,
    appearance_changed: Option<Box<dyn FnMut()>>,
    button_layout_changed: Option<Box<dyn FnMut()>>,
}

#[derive(Debug, Clone, Copy)]
struct RawWindow {
    window: *mut c_void,
    display: *mut c_void,
}

// 安全性：RawWindow 中的原始指针指向窗口生命周期内有效的 Wayland 表面/显示
// 这些仅用于传递给 wgpu，后者需要 Send+Sync 来创建表面
unsafe impl Send for RawWindow {}
unsafe impl Sync for RawWindow {}

impl rwh::HasWindowHandle for RawWindow {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let window = NonNull::new(self.window).unwrap();
        let handle = rwh::WaylandWindowHandle::new(window);
        Ok(unsafe { rwh::WindowHandle::borrow_raw(handle.into()) })
    }
}
impl rwh::HasDisplayHandle for RawWindow {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let display = NonNull::new(self.display).unwrap();
        let handle = rwh::WaylandDisplayHandle::new(display);
        Ok(unsafe { rwh::DisplayHandle::borrow_raw(handle.into()) })
    }
}

#[derive(Debug)]
struct InProgressConfigure {
    size: Option<Size<Pixels>>,
    fullscreen: bool,
    maximized: bool,
    resizing: bool,
    tiling: Tiling,
}

pub struct WaylandWindowState {
    surface_state: WaylandSurfaceState,
    acknowledged_first_configure: bool,
    mapping: WindowMapping,
    /// 取消映射会清除服务端属性，恢复时需重发最小尺寸和窗口状态。
    window_min_size: Option<Size<Pixels>>,
    restore_activation_token: Option<String>,
    /// 只恢复随父窗口一起隐藏的子窗口，不改变原本就隐藏的子窗口。
    hidden_children: Vec<ObjectId>,
    frame_callback_pending: bool,
    remap_retry_scheduled: bool,
    parent: Option<WaylandWindowStatePtr>,
    /// Child surfaces mapped to whether they block this window's input (dialogs
    /// block, popups don't). Children are closed before this window closes.
    children: FxHashMap<ObjectId, bool>,
    pub surface: wl_surface::WlSurface,
    app_id: Option<String>,
    /// 客户端侧保存的窗口标题；Wayland 协议不提供读取，只能自己记
    title: String,
    appearance: WindowAppearance,
    blur: Option<org_kde_kwin_blur::OrgKdeKwinBlur>,
    viewport: Option<wp_viewport::WpViewport>,
    outputs: HashMap<ObjectId, Output>,
    display: Option<(ObjectId, Output)>,
    globals: Globals,
    renderer: WgpuRenderer,
    bounds: Bounds<Pixels>,
    scale: f32,
    input_handler: Option<PlatformInputHandler>,
    decorations: WindowDecorations,
    background_appearance: WindowBackgroundAppearance,
    fullscreen: bool,
    maximized: bool,
    tiling: Tiling,
    window_bounds: Bounds<Pixels>,
    client: WaylandClientStatePtr,
    handle: AnyWindowHandle,
    active: bool,
    hovered: bool,
    pub(crate) force_render_after_recovery: bool,
    renderer_presented: bool,
    in_progress_configure: Option<InProgressConfigure>,
    resize_throttle: bool,
    in_progress_window_controls: Option<WindowControls>,
    /// 合成器上报的能力（WmCapabilities）
    window_controls: WindowControls,
    /// 应用请求的能力：与合成器能力取交集后对外报告，避免画出点了没反应的按钮
    is_resizable: bool,
    is_minimizable: bool,
    client_inset: Option<Pixels>,
    accesskit_adapter: Option<accesskit_unix::Adapter>,
}

pub enum WaylandSurfaceState {
    Xdg(WaylandXdgSurfaceState),
    LayerShell(WaylandLayerSurfaceState),
    Popup(WaylandPopupSurfaceState),
}

impl WaylandSurfaceState {
    fn new(
        surface: &wl_surface::WlSurface,
        globals: &Globals,
        params: &WindowParams,
        parent: Option<WaylandWindowStatePtr>,
        popup_grab: Option<(u32, wl_seat::WlSeat)>,
        target_output: Option<wl_output::WlOutput>,
    ) -> anyhow::Result<Self> {
        // 对于 layer_shell 窗口，创建层表面而不是 xdg 表面
        if let WindowKind::LayerShell(options) = &params.kind {
            let Some(layer_shell) = globals.layer_shell.as_ref() else {
                return Err(LayerShellNotSupportedError.into());
            };

            let layer_surface = layer_shell.get_layer_surface(
                &surface,
                target_output.as_ref(),
                super::layer_shell::wayland_layer(options.layer),
                options.namespace.clone(),
                &globals.qh,
                surface.id(),
            );

            let width = f32::from(params.bounds.size.width);
            let height = f32::from(params.bounds.size.height);
            layer_surface.set_size(width as u32, height as u32);

            layer_surface.set_anchor(super::layer_shell::wayland_anchor(options.anchor));
            layer_surface.set_keyboard_interactivity(
                super::layer_shell::wayland_keyboard_interactivity(options.keyboard_interactivity),
            );

            if let Some(margin) = options.margin {
                layer_surface.set_margin(
                    f32::from(margin.0) as i32,
                    f32::from(margin.1) as i32,
                    f32::from(margin.2) as i32,
                    f32::from(margin.3) as i32,
                )
            }

            if let Some(exclusive_zone) = options.exclusive_zone {
                layer_surface.set_exclusive_zone(f32::from(exclusive_zone) as i32);
            }

            if let Some(exclusive_edge) = options.exclusive_edge {
                layer_surface
                    .set_exclusive_edge(super::layer_shell::wayland_anchor(exclusive_edge));
            }

            return Ok(WaylandSurfaceState::LayerShell(WaylandLayerSurfaceState {
                layer_surface,
            }));
        }

        // 为 AnchoredPopup 创建 xdg_popup 表面
        if let WindowKind::AnchoredPopup(options) = &params.kind {
            let Some(parent) = parent.as_ref() else {
                return Err(anyhow::anyhow!("popup parent window not found"));
            };

            let positioner = build_popup_positioner(
                globals,
                options,
                params.bounds.size,
                parent.window_geometry(),
            );

            let xdg_surface = globals
                .wm_base
                .get_xdg_surface(&surface, &globals.qh, surface.id());

            let xdg_popup = if let Some(parent_layer_surface) = parent.layer_surface() {
                let xdg_popup = xdg_surface.get_popup(None, &positioner, &globals.qh, surface.id());
                parent_layer_surface.get_popup(&xdg_popup);
                xdg_popup
            } else {
                xdg_surface.get_popup(
                    parent.xdg_surface().as_ref(),
                    &positioner,
                    &globals.qh,
                    surface.id(),
                )
            };
            positioner.destroy();

            if let Some((serial, seat)) = popup_grab {
                xdg_popup.grab(&seat, serial);
            }

            parent.add_child(surface.id(), false);

            return Ok(WaylandSurfaceState::Popup(WaylandPopupSurfaceState {
                xdg_surface,
                xdg_popup,
                options: options.clone(),
                next_reposition_token: Cell::new(0),
            }));
        }

        // 所有其他 WindowKind 都创建为常规 xdg 表面
        let xdg_surface = globals
            .wm_base
            .get_xdg_surface(&surface, &globals.qh, surface.id());

        let toplevel = xdg_surface.get_toplevel(&globals.qh, surface.id());
        let xdg_parent = parent.as_ref().and_then(|w| w.toplevel());

        if params.kind == WindowKind::Floating || params.kind == WindowKind::Dialog {
            toplevel.set_parent(xdg_parent.as_ref());
        }

        // Overlay 窗口在 Wayland 上作为常规 toplevel 处理
        // 注意：Wayland 协议不直接支持"始终置顶"，需要合成器支持
        if let WindowKind::Overlay = &params.kind {
            // 可以设置窗口为无边框
            // 透明度由渲染器处理
        }

        let dialog = if params.kind == WindowKind::Dialog {
            let dialog = globals.dialog.as_ref().map(|dialog| {
                let xdg_dialog = dialog.get_xdg_dialog(&toplevel, &globals.qh, ());
                xdg_dialog.set_modal();
                xdg_dialog
            });

            if let Some(parent) = parent.as_ref() {
                parent.add_child(surface.id(), true);
            }

            dialog
        } else {
            None
        };

        if let Some(size) = params.window_min_size {
            toplevel.set_min_size(f32::from(size.width) as i32, f32::from(size.height) as i32);
        }

        // 根据请求的配置尝试设置窗口装饰
        let decoration = globals
            .decoration_manager
            .as_ref()
            .map(|decoration_manager| {
                decoration_manager.get_toplevel_decoration(&toplevel, &globals.qh, surface.id())
            });

        Ok(WaylandSurfaceState::Xdg(WaylandXdgSurfaceState {
            xdg_surface,
            toplevel,
            decoration,
            dialog,
        }))
    }
}

pub struct WaylandXdgSurfaceState {
    xdg_surface: xdg_surface::XdgSurface,
    toplevel: xdg_toplevel::XdgToplevel,
    decoration: Option<zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1>,
    dialog: Option<XdgDialogV1>,
}

pub struct WaylandLayerSurfaceState {
    layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
}

pub struct WaylandPopupSurfaceState {
    xdg_surface: xdg_surface::XdgSurface,
    xdg_popup: xdg_popup::XdgPopup,
    options: PopupOptions,
    next_reposition_token: Cell<u32>,
}

fn build_popup_positioner(
    globals: &Globals,
    options: &PopupOptions,
    size: Size<Pixels>,
    parent_geometry: Bounds<Pixels>,
) -> xdg_positioner::XdgPositioner {
    let positioner = globals.wm_base.create_positioner(&globals.qh, ());
    positioner.set_size(
        f32::from(size.width).max(1.0) as i32,
        f32::from(size.height).max(1.0) as i32,
    );

    let anchor_rect = Bounds {
        origin: options.anchor_rect.origin - parent_geometry.origin,
        size: options.anchor_rect.size,
    };
    let one = Point::new(px(1.0), px(1.0));
    let geometry_bottom_right: Point<Pixels> = parent_geometry.size.into();
    let top_left = anchor_rect
        .origin
        .min(&(geometry_bottom_right - one))
        .max(&Point::default());
    let bottom_right = anchor_rect
        .bottom_right()
        .min(&geometry_bottom_right)
        .max(&(top_left + one));
    let anchor_rect = Bounds::from_corners(top_left, bottom_right);
    positioner.set_anchor_rect(
        f32::from(anchor_rect.origin.x) as i32,
        f32::from(anchor_rect.origin.y) as i32,
        f32::from(anchor_rect.size.width) as i32,
        f32::from(anchor_rect.size.height) as i32,
    );

    positioner.set_anchor(super::popup::wayland_anchor(options.anchor));
    positioner.set_gravity(super::popup::wayland_gravity(options.gravity));
    positioner.set_constraint_adjustment(super::popup::wayland_constraint_adjustment(
        options.constraint_adjustment,
    ));
    positioner.set_offset(
        f32::from(options.offset.x) as i32,
        f32::from(options.offset.y) as i32,
    );
    positioner
}

impl WaylandSurfaceState {
    fn ack_configure(&self, serial: u32) {
        match self {
            WaylandSurfaceState::Xdg(WaylandXdgSurfaceState { xdg_surface, .. }) => {
                xdg_surface.ack_configure(serial);
            }
            WaylandSurfaceState::LayerShell(WaylandLayerSurfaceState { layer_surface, .. }) => {
                layer_surface.ack_configure(serial);
            }
            WaylandSurfaceState::Popup(WaylandPopupSurfaceState { xdg_surface, .. }) => {
                xdg_surface.ack_configure(serial);
            }
        }
    }

    fn decoration(&self) -> Option<&zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1> {
        if let WaylandSurfaceState::Xdg(WaylandXdgSurfaceState { decoration, .. }) = self {
            decoration.as_ref()
        } else {
            None
        }
    }

    fn toplevel(&self) -> Option<&xdg_toplevel::XdgToplevel> {
        if let WaylandSurfaceState::Xdg(WaylandXdgSurfaceState { toplevel, .. }) = self {
            Some(toplevel)
        } else {
            None
        }
    }

    fn xdg_surface(&self) -> Option<&xdg_surface::XdgSurface> {
        match self {
            WaylandSurfaceState::Xdg(WaylandXdgSurfaceState { xdg_surface, .. }) => {
                Some(xdg_surface)
            }
            WaylandSurfaceState::Popup(WaylandPopupSurfaceState { xdg_surface, .. }) => {
                Some(xdg_surface)
            }
            WaylandSurfaceState::LayerShell(_) => None,
        }
    }

    fn layer_surface(&self) -> Option<&zwlr_layer_surface_v1::ZwlrLayerSurfaceV1> {
        if let WaylandSurfaceState::LayerShell(WaylandLayerSurfaceState { layer_surface, .. }) =
            self
        {
            Some(layer_surface)
        } else {
            None
        }
    }

    fn set_geometry(&self, x: i32, y: i32, width: i32, height: i32) {
        match self {
            WaylandSurfaceState::Xdg(WaylandXdgSurfaceState { xdg_surface, .. }) => {
                xdg_surface.set_window_geometry(x, y, width, height);
            }
            WaylandSurfaceState::LayerShell(WaylandLayerSurfaceState { layer_surface, .. }) => {
                // 无法设置层表面的窗口位置
                layer_surface.set_size(width as u32, height as u32);
            }
            WaylandSurfaceState::Popup(WaylandPopupSurfaceState { xdg_surface, .. }) => {
                xdg_surface.set_window_geometry(x, y, width, height);
            }
        }
    }

    fn reposition_popup(
        &self,
        globals: &Globals,
        size: Size<Pixels>,
        parent_geometry: Bounds<Pixels>,
    ) {
        if let WaylandSurfaceState::Popup(WaylandPopupSurfaceState {
            xdg_popup,
            options,
            next_reposition_token,
            ..
        }) = self
            && xdg_popup.version() >= xdg_popup::REQ_REPOSITION_SINCE
        {
            let token = next_reposition_token.get();
            next_reposition_token.set(token.wrapping_add(1));

            let positioner = build_popup_positioner(globals, options, size, parent_geometry);
            xdg_popup.reposition(&positioner, token);
            positioner.destroy();
        }
    }

    fn destroy(&mut self) {
        match self {
            WaylandSurfaceState::Xdg(WaylandXdgSurfaceState {
                xdg_surface,
                toplevel,
                decoration: _decoration,
                dialog,
            }) => {
                // 在 toplevel 之前丢弃 dialog，以便合成器显式取消应用其效果
                if let Some(dialog) = dialog {
                    dialog.destroy();
                }

                // 角色对象（toplevel）必须在 xdg_surface 之前销毁
                // 参见 https://wayland.app/protocols/xdg-shell#xdg_surface:request:destroy
                toplevel.destroy();
                xdg_surface.destroy();
            }
            WaylandSurfaceState::LayerShell(WaylandLayerSurfaceState { layer_surface }) => {
                layer_surface.destroy();
            }
            WaylandSurfaceState::Popup(WaylandPopupSurfaceState {
                xdg_surface,
                xdg_popup,
                ..
            }) => {
                xdg_popup.destroy();
                xdg_surface.destroy();
            }
        }
    }
}

#[derive(Clone)]
pub struct WaylandWindowStatePtr {
    state: Rc<RefCell<WaylandWindowState>>,
    callbacks: Rc<RefCell<Callbacks>>,
}

impl WaylandWindowState {
    pub(crate) fn new(
        handle: AnyWindowHandle,
        surface: wl_surface::WlSurface,
        surface_state: WaylandSurfaceState,
        appearance: WindowAppearance,
        viewport: Option<wp_viewport::WpViewport>,
        client: WaylandClientStatePtr,
        globals: Globals,
        gpu_context: rgpui_wgpu::GpuContext,
        compositor_gpu: Option<CompositorGpuHint>,
        options: WindowParams,
        parent: Option<WaylandWindowStatePtr>,
    ) -> anyhow::Result<Self> {
        let renderer = {
            let raw_window = RawWindow {
                window: surface.id().as_ptr().cast::<c_void>(),
                display: surface
                    .backend()
                    .upgrade()
                    .unwrap()
                    .display_ptr()
                    .cast::<c_void>(),
            };
            let config = WgpuSurfaceConfig {
                size: Size {
                    width: DevicePixels(f32::from(options.bounds.size.width) as i32),
                    height: DevicePixels(f32::from(options.bounds.size.height) as i32),
                },
                transparent: true,
                preferred_present_mode: Some(wgpu::PresentMode::Mailbox),
            };
            WgpuRenderer::new(gpu_context, &raw_window, config, compositor_gpu)?
        };

        // Wayland 没有读取标题的请求，标题只能由客户端自己留着，get_title 才拿得到
        let mut title = String::new();
        if let WaylandSurfaceState::Xdg(ref xdg_state) = surface_state {
            if let Some(titlebar) = options.titlebar.and_then(|titlebar| titlebar.title) {
                title = titlebar.to_string();
                xdg_state.toplevel.set_title(title.clone());
            }
            // 根据 GPU 的最大纹理尺寸设置最大窗口大小
            // 这可以防止窗口被调整为大于 GPU 可渲染的尺寸
            let max_texture_size = renderer.max_texture_size() as i32;
            xdg_state
                .toplevel
                .set_max_size(max_texture_size, max_texture_size);
        }

        Ok(Self {
            surface_state,
            acknowledged_first_configure: false,
            mapping: WindowMapping::default(),
            window_min_size: options.window_min_size,
            restore_activation_token: None,
            hidden_children: Vec::new(),
            frame_callback_pending: false,
            remap_retry_scheduled: false,
            parent,
            children: FxHashMap::default(),
            surface,
            app_id: None,
            title,
            blur: None,
            viewport,
            globals,
            outputs: HashMap::default(),
            display: None,
            renderer,
            bounds: options.bounds,
            scale: 1.0,
            input_handler: None,
            decorations: WindowDecorations::Client,
            background_appearance: WindowBackgroundAppearance::Opaque,
            fullscreen: false,
            maximized: false,
            tiling: Tiling::default(),
            window_bounds: options.bounds,
            in_progress_configure: None,
            resize_throttle: false,
            client,
            appearance,
            handle,
            active: false,
            hovered: false,
            force_render_after_recovery: false,
            renderer_presented: false,
            in_progress_window_controls: None,
            window_controls: WindowControls::default(),
            is_resizable: options.is_resizable,
            is_minimizable: options.is_minimizable,
            client_inset: None,
            accesskit_adapter: None,
        })
    }

    pub fn is_transparent(&self) -> bool {
        self.decorations == WindowDecorations::Client
            || self.background_appearance != WindowBackgroundAppearance::Opaque
    }

    fn update_subpixel_layout(&mut self) {
        use wayland_client::protocol::wl_output::Subpixel;
        let is_bgr = self
            .display
            .as_ref()
            .and_then(|(_, output)| output.subpixel)
            .is_some_and(|s| s == Subpixel::HorizontalBgr);
        self.renderer.set_subpixel_layout(is_bgr);
    }

    pub fn primary_output_scale(&mut self) -> i32 {
        let mut scale = 1;
        let mut current_output = self.display.take();
        for (id, output) in self.outputs.iter() {
            if let Some((_, output_data)) = &current_output {
                if output.scale > output_data.scale {
                    current_output = Some((id.clone(), output.clone()));
                }
            } else {
                current_output = Some((id.clone(), output.clone()));
            }
            scale = scale.max(output.scale);
        }
        self.display = current_output;
        scale
    }

    pub fn inset(&self) -> Pixels {
        match self.decorations {
            WindowDecorations::Server => px(0.0),
            WindowDecorations::Client => self.client_inset.unwrap_or(px(0.0)),
        }
    }
}

pub(crate) struct WaylandWindow(pub WaylandWindowStatePtr);

/// 同步屏障和帧回调都带窗口代数，防止隐藏前的回调启动新的绘制循环。
pub(crate) struct SurfaceCallback {
    pub surface_id: ObjectId,
    pub generation: u64,
    pub kind: SurfaceCallbackKind,
}

pub(crate) enum SurfaceCallbackKind {
    Frame,
    Remap,
}
pub enum ImeInput {
    InsertText(String),
    SetMarkedText(String),
    UnmarkText,
    DeleteText,
}

impl Drop for WaylandWindow {
    fn drop(&mut self) {
        let mut state = self.0.state.borrow_mut();
        state.mapping.hide();
        let surface_id = state.surface.id();
        if let Some(parent) = state.parent.as_ref() {
            parent.state.borrow_mut().children.remove(&surface_id);
        }

        let client = state.client.clone();

        state.renderer.destroy();

        // 首先销毁模糊效果，这没有依赖关系
        if let Some(blur) = &state.blur {
            blur.release();
        }

        // 装饰必须在 xdg 状态之前销毁
        // 参见 https://wayland.app/protocols/xdg-decoration-unstable-v1#zxdg_toplevel_decoration_v1
        if let Some(decoration) = &state.surface_state.decoration() {
            decoration.destroy();
        }

        // 表面状态可能包含 xdg_toplevel/xdg_surface，现在装饰已移除可以销毁
        // layer_surface 没有依赖关系
        state.surface_state.destroy();

        // Viewport 必须在 wl_surface 之前销毁
        // 参见 https://wayland.app/protocols/viewporter#wp_viewport
        if let Some(viewport) = &state.viewport {
            viewport.destroy();
        }

        // wl_surface 本身应该始终最后销毁
        state.surface.destroy();

        let state_ptr = self.0.clone();
        state
            .globals
            .executor
            .spawn(async move {
                state_ptr.close();
                client.drop_window(&surface_id)
            })
            .detach();
        drop(state);
    }
}

impl WaylandWindow {
    fn borrow(&self) -> Ref<'_, WaylandWindowState> {
        self.0.state.borrow()
    }

    fn borrow_mut(&self) -> RefMut<'_, WaylandWindowState> {
        self.0.state.borrow_mut()
    }

    pub fn new(
        handle: AnyWindowHandle,
        globals: Globals,
        gpu_context: rgpui_wgpu::GpuContext,
        compositor_gpu: Option<CompositorGpuHint>,
        client: WaylandClientStatePtr,
        params: WindowParams,
        appearance: WindowAppearance,
        parent: Option<WaylandWindowStatePtr>,
        popup_grab: Option<(u32, wl_seat::WlSeat)>,
        target_output: Option<wl_output::WlOutput>,
    ) -> anyhow::Result<(Self, ObjectId)> {
        let surface = globals.compositor.create_surface(&globals.qh, ());
        let surface_state = WaylandSurfaceState::new(
            &surface,
            &globals,
            &params,
            parent.clone(),
            popup_grab,
            target_output,
        )?;

        if let Some(fractional_scale_manager) = globals.fractional_scale_manager.as_ref() {
            fractional_scale_manager.get_fractional_scale(&surface, &globals.qh, surface.id());
        }

        let viewport = globals
            .viewporter
            .as_ref()
            .map(|viewporter| viewporter.get_viewport(&surface, &globals.qh, ()));

        // params 随后会被移交进窗口状态，先取出穿透意图
        let mouse_passthrough = params.mouse_passthrough;
        let this = Self(WaylandWindowStatePtr {
            state: Rc::new(RefCell::new(WaylandWindowState::new(
                handle,
                surface.clone(),
                surface_state,
                appearance,
                viewport,
                client,
                globals,
                gpu_context,
                compositor_gpu,
                params,
                parent,
            )?)),
            callbacks: Rc::new(RefCell::new(Callbacks::default())),
        });

        // Kick things off
        surface.commit();

        if mouse_passthrough {
            this.set_mouse_passthrough(true);
        }

        Ok((this, surface.id()))
    }
}

impl WaylandWindowStatePtr {
    /// 获取窗口句柄
    pub fn handle(&self) -> AnyWindowHandle {
        self.state.borrow().handle
    }

    /// 获取 wl_surface
    pub fn surface(&self) -> wl_surface::WlSurface {
        self.state.borrow().surface.clone()
    }

    /// 获取 xdg_toplevel（如果存在）
    pub fn toplevel(&self) -> Option<xdg_toplevel::XdgToplevel> {
        self.state.borrow().surface_state.toplevel().cloned()
    }

    /// 获取 xdg_surface（如果存在）。用于锚定子弹窗。
    pub fn xdg_surface(&self) -> Option<xdg_surface::XdgSurface> {
        self.state.borrow().surface_state.xdg_surface().cloned()
    }

    /// 获取 layer_surface（如果存在）。用于锚定子弹窗。
    pub fn layer_surface(&self) -> Option<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1> {
        self.state.borrow().surface_state.layer_surface().cloned()
    }

    /// 窗口的 xdg 窗口几何（surface-local 坐标）。子弹窗的锚定矩形
    /// 相对于此坐标，而 rgpui 坐标是 surface-local 的。
    pub fn window_geometry(&self) -> Bounds<Pixels> {
        let state = self.state.borrow();
        inset_by_tiling(
            state.bounds.map_origin(|_| px(0.0)),
            state.inset(),
            state.tiling,
        )
    }

    /// 请求合成器激活本窗口（xdg-activation 令牌）。
    ///
    /// 合成器有权拒绝（比如没有可用的用户输入序列号）；即使被拒，KWin / Mutter
    /// 也会用 app_id 在任务栏上提示「有应用想抢焦点」。
    pub(crate) fn request_activation(&self) {
        let mut state = self.state.borrow_mut();
        let client = state.client.get_client();
        let supplied_token = client.borrow_mut().common.tray_activation_token.take();
        if !state.mapping.is_visible() {
            if supplied_token.is_some() {
                state.restore_activation_token = supplied_token;
            }
            if let Some(generation) = state.mapping.show() {
                // 同步回复之前丢弃旧 configure；之后再发送空提交，确保新旧配置不串轮。
                if let Some(backend) = state.surface.backend().upgrade() {
                    Connection::from_backend(backend).display().sync(
                        &state.globals.qh,
                        SurfaceCallback {
                            surface_id: state.surface.id(),
                            generation,
                            kind: SurfaceCallbackKind::Remap,
                        },
                    );
                }
            }
            return;
        }
        if let Some(activation) = &state.globals.activation {
            // 托盘点击发生在桌面外壳，不能用本窗口的旧鼠标序列号重新申请令牌。
            // 优先消费宿主传来的令牌；它无需本窗口已设置 app_id。
            if let Some(token) = supplied_token {
                log::debug!("Activate a Wayland window using a token provided by the tray host");
                activation.activate(token, &state.surface);
                return;
            }
            state.client.set_pending_activation(state.surface.id());
            let token = activation.get_activation_token(&state.globals.qh, ());
            let serial = state.client.get_serial(SerialKind::MousePress);
            // app_id 是协议的可选信息，缺失时仍应发送激活请求。
            if let Some(app_id) = state.app_id.clone() {
                token.set_app_id(app_id);
            }
            token.set_serial(serial, &state.globals.seat);
            token.set_surface(&state.surface);
            token.commit();
        }
    }

    /// 处理异步申请的令牌；恢复未完成时延期，已隐藏时丢弃。
    pub(crate) fn activate_with_token(&self, token: String) {
        let mut state = self.state.borrow_mut();
        if state.mapping.is_hidden() {
            return;
        }
        if !state.mapping.is_visible() {
            // 不覆盖恢复期间已收到的托盘宿主令牌。
            state.restore_activation_token.get_or_insert(token);
        } else if let Some(activation) = &state.globals.activation {
            activation.activate(token, &state.surface);
        }
    }

    /// 取消普通窗口映射；保留实体、后台任务及 GPU 缓存，任务栏中不再保留窗口。
    pub(crate) fn hide_window(&self) -> bool {
        let (client, children) = {
            let mut state = self.state.borrow_mut();
            if !matches!(state.surface_state, WaylandSurfaceState::Xdg(_)) {
                return false;
            }
            if !state.mapping.hide() {
                return false;
            }
            state.acknowledged_first_configure = false;
            state.restore_activation_token = None;
            state.renderer_presented = false;
            state.in_progress_configure = None;
            state.in_progress_window_controls = None;
            state.resize_throttle = false;
            state.frame_callback_pending = false;
            state.remap_retry_scheduled = false;
            state.active = false;
            state.hovered = false;
            (
                state.client.get_client(),
                state.children.keys().cloned().collect::<Vec<_>>(),
            )
        };
        // 先隐藏子窗口，避免父窗口取消映射时留下独立的对话框；短生命周期弹窗直接关闭。
        for id in children {
            let child = get_window(&mut client.borrow_mut(), &id);
            if let Some(child) = child {
                let popup = matches!(
                    child.state.borrow().surface_state,
                    WaylandSurfaceState::Popup(_)
                );
                if popup {
                    // 当前调用可能位于应用的关闭回调中，推迟销毁以免重入 App 借用。
                    let executor = self.state.borrow().globals.executor.clone();
                    executor
                        .spawn(async move {
                            child.close();
                        })
                        .detach();
                } else if child.hide_window() {
                    self.state.borrow_mut().hidden_children.push(id);
                }
            }
        }
        let state = self.state.borrow();
        state.surface.attach(None, 0, 0);
        state.surface.commit();
        log::debug!("Unmapped Wayland window; background state retained");
        true
    }

    /// 同步屏障后重发取消映射时丢失的属性，再请求新一轮 configure。
    fn remap_after_sync(&self, generation: u64) {
        let mut state = self.state.borrow_mut();
        if !state.mapping.synced(generation) {
            return;
        }
        state.in_progress_configure = None;
        state.in_progress_window_controls = None;
        if let Some(toplevel) = state.surface_state.toplevel() {
            toplevel.set_title(state.title.clone());
            if let Some(app_id) = &state.app_id {
                toplevel.set_app_id(app_id.clone());
            }
            if let Some(min) = state.window_min_size {
                toplevel.set_min_size(f32::from(min.width) as i32, f32::from(min.height) as i32);
            }
            let max = state.renderer.max_texture_size() as i32;
            toplevel.set_max_size(max, max);
            if let Some(parent) = &state.parent {
                toplevel.set_parent(parent.toplevel().as_ref());
            }
            if state.maximized {
                toplevel.set_maximized();
            }
            if state.fullscreen {
                toplevel.set_fullscreen(None);
            }
        }
        if let Some(decoration) = state.surface_state.decoration() {
            decoration.set_mode(state.decorations.to_xdg());
        }
        if let WaylandSurfaceState::Xdg(xdg) = &state.surface_state {
            if let Some(dialog) = &xdg.dialog {
                dialog.set_modal();
            }
        }
        state.force_render_after_recovery = true;
        // 重置首次 configure 标志：configure 处理仅在该标志为 false 时请求帧回调，
        // 不重置则恢复后永不绘制首帧，窗口卡在 WaitingForFrame 无法重现。
        state.acknowledged_first_configure = false;
        state.surface.commit();
        log::debug!("Remapping Wayland window; waiting for configure");
    }

    /// 只执行当前一轮映射的回调，忽略隐藏前残留的帧回调。
    pub(crate) fn handle_surface_callback(&self, callback: &SurfaceCallback) {
        if callback.generation != self.state.borrow().mapping.generation() {
            return;
        }
        match callback.kind {
            SurfaceCallbackKind::Frame => {
                self.state.borrow_mut().frame_callback_pending = false;
                self.frame();
            }
            SurfaceCallbackKind::Remap => self.remap_after_sync(callback.generation),
        }
    }

    /// 最小化保留任务栏入口，与隐藏到托盘分开处理。
    pub(crate) fn minimize_window(&self) {
        if let Some(toplevel) = self.state.borrow().surface_state.toplevel() {
            toplevel.set_minimized();
        }
    }

    /// 检查两个指针是否指向同一状态
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }

    /// 添加子窗口
    pub fn add_child(&self, child: ObjectId, blocking: bool) {
        let mut state = self.state.borrow_mut();
        state.children.insert(child, blocking);
    }

    /// 检查窗口是否被阻塞（有子窗口或弹窗）
    pub fn is_blocked(&self) -> bool {
        let state = self.state.borrow();
        state.children.values().any(|&blocking| blocking)
    }

    pub fn frame(&self) {
        let mut state = self.state.borrow_mut();
        if !state.mapping.can_render() || !state.acknowledged_first_configure {
            return;
        }
        if !state.frame_callback_pending {
            state.surface.frame(
                &state.globals.qh,
                SurfaceCallback {
                    surface_id: state.surface.id(),
                    generation: state.mapping.generation(),
                    kind: SurfaceCallbackKind::Frame,
                },
            );
            state.frame_callback_pending = true;
        }
        state.resize_throttle = false;
        let force_render = state.force_render_after_recovery;
        state.force_render_after_recovery = false;
        drop(state);

        let mut cb = self.callbacks.borrow_mut();
        if let Some(fun) = cb.request_frame.as_mut() {
            fun(RequestFrameOptions {
                force_render,
                ..Default::default()
            });
        }
    }

    pub fn handle_xdg_surface_event(&self, event: xdg_surface::Event) {
        if !self.state.borrow().mapping.accepts_configure() {
            return;
        }
        if let xdg_surface::Event::Configure { serial } = event {
            {
                let mut state = self.state.borrow_mut();
                if let Some(window_controls) = state.in_progress_window_controls.take() {
                    state.window_controls = window_controls;

                    drop(state);
                    let mut callbacks = self.callbacks.borrow_mut();
                    if let Some(appearance_changed) = callbacks.appearance_changed.as_mut() {
                        appearance_changed();
                    }
                }
            }
            {
                let mut state = self.state.borrow_mut();

                if let Some(mut configure) = state.in_progress_configure.take() {
                    let got_unmaximized = state.maximized && !configure.maximized;
                    state.fullscreen = configure.fullscreen;
                    state.maximized = configure.maximized;
                    state.tiling = configure.tiling;
                    // Limit interactive resizes to once per vblank
                    if configure.resizing && state.resize_throttle {
                        state.surface_state.ack_configure(serial);
                        return;
                    } else if configure.resizing {
                        state.resize_throttle = true;
                    }
                    if !configure.fullscreen && !configure.maximized {
                        configure.size = if got_unmaximized {
                            Some(state.window_bounds.size)
                        } else {
                            compute_outer_size(state.inset(), configure.size, state.tiling)
                        };
                        if let Some(size) = configure.size {
                            state.window_bounds = Bounds {
                                origin: Point::default(),
                                size,
                            };
                        }
                    }
                    drop(state);
                    if let Some(size) = configure.size {
                        self.resize(size);
                    }
                }
            }
            let mut state = self.state.borrow_mut();
            state.surface_state.ack_configure(serial);

            state.mapping.configured();

            let window_geometry = inset_by_tiling(
                state.bounds.map_origin(|_| px(0.0)),
                state.inset(),
                state.tiling,
            )
            .map(|v| f32::from(v) as i32)
            .map_size(|v| if v <= 0 { 1 } else { v });

            state.surface_state.set_geometry(
                window_geometry.origin.x,
                window_geometry.origin.y,
                window_geometry.size.width,
                window_geometry.size.height,
            );

            let request_frame_callback = !state.acknowledged_first_configure;
            if request_frame_callback {
                state.acknowledged_first_configure = true;
                drop(state);
                self.frame();
            }
        }
    }

    pub fn handle_toplevel_decoration_event(&self, event: zxdg_toplevel_decoration_v1::Event) {
        if !self.state.borrow().mapping.accepts_configure() {
            return;
        }
        if let zxdg_toplevel_decoration_v1::Event::Configure { mode } = event {
            match mode {
                WEnum::Value(zxdg_toplevel_decoration_v1::Mode::ServerSide) => {
                    self.state.borrow_mut().decorations = WindowDecorations::Server;
                    let callback = self.callbacks.borrow_mut().appearance_changed.take();
                    if let Some(mut fun) = callback {
                        fun();
                        self.callbacks.borrow_mut().appearance_changed = Some(fun);
                    }
                }
                WEnum::Value(zxdg_toplevel_decoration_v1::Mode::ClientSide) => {
                    self.state.borrow_mut().decorations = WindowDecorations::Client;
                    // Update background to be transparent
                    let callback = self.callbacks.borrow_mut().appearance_changed.take();
                    if let Some(mut fun) = callback {
                        fun();
                        self.callbacks.borrow_mut().appearance_changed = Some(fun);
                    }
                }
                WEnum::Value(_) => {
                    log::warn!("Unknown decoration mode");
                }
                WEnum::Unknown(v) => {
                    log::warn!("Unknown decoration mode: {}", v);
                }
            }
        }
    }

    pub fn handle_fractional_scale_event(&self, event: wp_fractional_scale_v1::Event) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            self.rescale(scale as f32 / 120.0);
        }
    }

    pub fn handle_toplevel_event(&self, event: xdg_toplevel::Event) -> bool {
        if !self.state.borrow().mapping.accepts_configure() {
            return false;
        }
        match event {
            xdg_toplevel::Event::Configure {
                width,
                height,
                states,
            } => {
                let size = if width == 0 || height == 0 {
                    None
                } else {
                    Some(size(px(width as f32), px(height as f32)))
                };

                let states = extract_states::<xdg_toplevel::State>(&states);

                let mut tiling = Tiling::default();
                let mut fullscreen = false;
                let mut maximized = false;
                let mut resizing = false;

                for state in states {
                    match state {
                        xdg_toplevel::State::Maximized => {
                            maximized = true;
                        }
                        xdg_toplevel::State::Fullscreen => {
                            fullscreen = true;
                        }
                        xdg_toplevel::State::Resizing => resizing = true,
                        xdg_toplevel::State::TiledTop => {
                            tiling.top = true;
                        }
                        xdg_toplevel::State::TiledLeft => {
                            tiling.left = true;
                        }
                        xdg_toplevel::State::TiledRight => {
                            tiling.right = true;
                        }
                        xdg_toplevel::State::TiledBottom => {
                            tiling.bottom = true;
                        }
                        _ => {
                            // noop
                        }
                    }
                }

                if fullscreen || maximized {
                    tiling = Tiling::tiled();
                }

                let mut state = self.state.borrow_mut();
                state.in_progress_configure = Some(InProgressConfigure {
                    size,
                    fullscreen,
                    maximized,
                    resizing,
                    tiling,
                });

                false
            }
            xdg_toplevel::Event::Close => {
                let mut cb = self.callbacks.borrow_mut();
                if let Some(mut should_close) = cb.should_close.take() {
                    let result = (should_close)();
                    cb.should_close = Some(should_close);
                    if result {
                        drop(cb);
                        self.close();
                    }
                    result
                } else {
                    true
                }
            }
            xdg_toplevel::Event::WmCapabilities { capabilities } => {
                let mut window_controls = WindowControls {
                    maximize: false,
                    minimize: false,
                    fullscreen: false,
                    window_menu: false,
                };

                let states = extract_states::<xdg_toplevel::WmCapabilities>(&capabilities);

                for state in states {
                    match state {
                        xdg_toplevel::WmCapabilities::Maximize => {
                            window_controls.maximize = true;
                        }
                        xdg_toplevel::WmCapabilities::Minimize => {
                            window_controls.minimize = true;
                        }
                        xdg_toplevel::WmCapabilities::Fullscreen => {
                            window_controls.fullscreen = true;
                        }
                        xdg_toplevel::WmCapabilities::WindowMenu => {
                            window_controls.window_menu = true;
                        }
                        _ => {}
                    }
                }

                let mut state = self.state.borrow_mut();
                state.in_progress_window_controls = Some(window_controls);
                false
            }
            _ => false,
        }
    }

    pub fn handle_layersurface_event(&self, event: zwlr_layer_surface_v1::Event) -> bool {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                width,
                height,
                serial,
            } => {
                let size = if width == 0 || height == 0 {
                    None
                } else {
                    Some(size(px(width as f32), px(height as f32)))
                };

                let mut state = self.state.borrow_mut();
                state.in_progress_configure = Some(InProgressConfigure {
                    size,
                    fullscreen: false,
                    maximized: false,
                    resizing: false,
                    tiling: Tiling::default(),
                });
                drop(state);

                // just do the same thing we'd do as an xdg_surface
                self.handle_xdg_surface_event(xdg_surface::Event::Configure { serial });

                false
            }
            zwlr_layer_surface_v1::Event::Closed => {
                // unlike xdg, we don't have a choice here: the surface is closing.
                true
            }
            _ => false,
        }
    }

    pub fn handle_popup_event(&self, event: xdg_popup::Event) -> bool {
        match event {
            xdg_popup::Event::Configure { width, height, .. } => {
                let size = if width <= 0 || height <= 0 {
                    None
                } else {
                    Some(size(px(width as f32), px(height as f32)))
                };

                self.state.borrow_mut().in_progress_configure = Some(InProgressConfigure {
                    size,
                    fullscreen: false,
                    maximized: false,
                    resizing: false,
                    tiling: Tiling::default(),
                });

                false
            }
            xdg_popup::Event::PopupDone => true,
            xdg_popup::Event::Repositioned { .. } => false,
            _ => false,
        }
    }

    #[allow(clippy::mutable_key_type)]
    pub fn handle_surface_event(
        &self,
        event: wl_surface::Event,
        outputs: HashMap<ObjectId, Output>,
    ) {
        let mut state = self.state.borrow_mut();

        match event {
            wl_surface::Event::Enter { output } => {
                let id = output.id();

                let Some(output) = outputs.get(&id) else {
                    return;
                };

                state.outputs.insert(id, output.clone());

                let scale = state.primary_output_scale();
                state.update_subpixel_layout();

                // We use `PreferredBufferScale` instead to set the scale if it's available
                if state.surface.version() < wl_surface::EVT_PREFERRED_BUFFER_SCALE_SINCE {
                    state.surface.set_buffer_scale(scale);
                    drop(state);
                    self.rescale(scale as f32);
                }
            }
            wl_surface::Event::Leave { output } => {
                state.outputs.remove(&output.id());

                let scale = state.primary_output_scale();
                state.update_subpixel_layout();

                // We use `PreferredBufferScale` instead to set the scale if it's available
                if state.surface.version() < wl_surface::EVT_PREFERRED_BUFFER_SCALE_SINCE {
                    state.surface.set_buffer_scale(scale);
                    drop(state);
                    self.rescale(scale as f32);
                }
            }
            wl_surface::Event::PreferredBufferScale { factor } => {
                // We use `WpFractionalScale` instead to set the scale if it's available
                if state.globals.fractional_scale_manager.is_none() {
                    state.surface.set_buffer_scale(factor);
                    drop(state);
                    self.rescale(factor as f32);
                }
            }
            _ => {}
        }
    }

    pub fn handle_ime(&self, ime: ImeInput) {
        if !self.state.borrow().mapping.can_render() || self.is_blocked() {
            return;
        }
        let mut state = self.state.borrow_mut();
        if let Some(mut input_handler) = state.input_handler.take() {
            drop(state);
            match ime {
                ImeInput::InsertText(text) => {
                    input_handler.replace_text_in_range(None, &text);
                }
                ImeInput::SetMarkedText(text) => {
                    input_handler.replace_and_mark_text_in_range(None, &text, None);
                }
                ImeInput::UnmarkText => {
                    input_handler.unmark_text();
                }
                ImeInput::DeleteText => {
                    if let Some(marked) = input_handler.marked_text_range() {
                        input_handler.replace_text_in_range(Some(marked), "");
                    }
                }
            }
            self.state.borrow_mut().input_handler = Some(input_handler);
        }
    }

    pub fn get_ime_area(&self) -> Option<Bounds<Pixels>> {
        let mut state = self.state.borrow_mut();
        let mut bounds: Option<Bounds<Pixels>> = None;
        if let Some(mut input_handler) = state.input_handler.take() {
            drop(state);
            bounds = input_handler.ime_candidate_bounds();
            self.state.borrow_mut().input_handler = Some(input_handler);
        }
        bounds
    }

    pub fn set_size_and_scale(&self, size: Option<Size<Pixels>>, scale: Option<f32>) {
        let (size, scale) = {
            let mut state = self.state.borrow_mut();
            if size.is_none_or(|size| size == state.bounds.size)
                && scale.is_none_or(|scale| scale == state.scale)
            {
                return;
            }
            if let Some(size) = size {
                state.bounds.size = size;
            }
            if let Some(scale) = scale {
                state.scale = scale;
            }
            let device_bounds = state.bounds.to_device_pixels(state.scale);
            state.renderer.update_drawable_size(device_bounds.size);
            (state.bounds.size, state.scale)
        };

        let callback = self.callbacks.borrow_mut().resize.take();
        if let Some(mut fun) = callback {
            fun(size, scale);
            self.callbacks.borrow_mut().resize = Some(fun);
        }

        {
            let state = self.state.borrow();
            if let Some(viewport) = &state.viewport {
                viewport
                    .set_destination(f32::from(size.width) as i32, f32::from(size.height) as i32);
            }
        }
    }

    pub fn resize(&self, size: Size<Pixels>) {
        self.set_size_and_scale(Some(size), None);
    }

    pub fn rescale(&self, scale: f32) {
        self.set_size_and_scale(None, Some(scale));
    }

    pub fn close(&self) {
        let state = self.state.borrow();
        let client = state.client.get_client();
        let children = state.children.keys().cloned().collect::<Vec<_>>();
        drop(state);

        for child in children {
            let mut client_state = client.borrow_mut();
            let window = get_window(&mut client_state, &child);
            drop(client_state);

            if let Some(child) = window {
                child.close();
            }
        }
        let mut callbacks = self.callbacks.borrow_mut();
        if let Some(fun) = callbacks.close.take() {
            fun()
        }
    }

    pub fn handle_input(&self, input: PlatformInput) {
        if !self.state.borrow().mapping.can_render() || self.is_blocked() {
            return;
        }
        let callback = self.callbacks.borrow_mut().input.take();
        if let Some(mut fun) = callback {
            let result = fun(input.clone());
            self.callbacks.borrow_mut().input = Some(fun);
            if !result.propagate {
                return;
            }
        }
        if let PlatformInput::KeyDown(event) = input
            && event.keystroke.modifiers.is_subset_of(&Modifiers::shift())
            && let Some(key_char) = &event.keystroke.key_char
        {
            let mut state = self.state.borrow_mut();
            if let Some(mut input_handler) = state.input_handler.take() {
                drop(state);
                input_handler.replace_text_in_range(None, key_char);
                self.state.borrow_mut().input_handler = Some(input_handler);
            }
        }
    }

    pub fn set_focused(&self, focus: bool) {
        self.state.borrow_mut().active = focus;
        let callback = self.callbacks.borrow_mut().active_status_change.take();
        if let Some(mut fun) = callback {
            fun(focus);
            self.callbacks.borrow_mut().active_status_change = Some(fun);
        }
        if let Some(adapter) = self.state.borrow_mut().accesskit_adapter.as_mut() {
            adapter.update_window_focus_state(focus);
        }
    }

    pub fn set_hovered(&self, focus: bool) {
        let callback = self.callbacks.borrow_mut().hover_status_change.take();
        if let Some(mut fun) = callback {
            fun(focus);
            self.callbacks.borrow_mut().hover_status_change = Some(fun);
        }
    }

    pub fn set_appearance(&mut self, appearance: WindowAppearance) {
        self.state.borrow_mut().appearance = appearance;

        let callback = self.callbacks.borrow_mut().appearance_changed.take();
        if let Some(mut fun) = callback {
            fun();
            self.callbacks.borrow_mut().appearance_changed = Some(fun);
        }
    }

    pub fn set_button_layout(&self) {
        let callback = self.callbacks.borrow_mut().button_layout_changed.take();
        if let Some(mut fun) = callback {
            fun();
            self.callbacks.borrow_mut().button_layout_changed = Some(fun);
        }
    }

    pub fn primary_output_scale(&self) -> i32 {
        self.state.borrow_mut().primary_output_scale()
    }
}

fn extract_states<'a, S: TryFrom<u32> + 'a>(states: &'a [u8]) -> impl Iterator<Item = S> + 'a
where
    <S as TryFrom<u32>>::Error: 'a,
{
    states
        .chunks_exact(4)
        .flat_map(TryInto::<[u8; 4]>::try_into)
        .map(u32::from_ne_bytes)
        .flat_map(S::try_from)
}

impl rwh::HasWindowHandle for WaylandWindow {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let surface = self.0.surface().id().as_ptr() as *mut libc::c_void;
        let c_ptr = NonNull::new(surface).ok_or(rwh::HandleError::Unavailable)?;
        let handle = rwh::WaylandWindowHandle::new(c_ptr);
        let raw_handle = rwh::RawWindowHandle::Wayland(handle);
        Ok(unsafe { rwh::WindowHandle::borrow_raw(raw_handle) })
    }
}

impl rwh::HasDisplayHandle for WaylandWindow {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let display = self
            .0
            .surface()
            .backend()
            .upgrade()
            .ok_or(rwh::HandleError::Unavailable)?
            .display_ptr() as *mut libc::c_void;

        let c_ptr = NonNull::new(display).ok_or(rwh::HandleError::Unavailable)?;
        let handle = rwh::WaylandDisplayHandle::new(c_ptr);
        let raw_handle = rwh::RawDisplayHandle::Wayland(handle);
        Ok(unsafe { rwh::DisplayHandle::borrow_raw(raw_handle) })
    }
}

impl PlatformWindow for WaylandWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        self.borrow().bounds
    }

    fn is_maximized(&self) -> bool {
        self.borrow().maximized
    }

    fn window_bounds(&self) -> WindowBounds {
        let state = self.borrow();
        if state.fullscreen {
            WindowBounds::Fullscreen(state.window_bounds)
        } else if state.maximized {
            WindowBounds::Maximized(state.window_bounds)
        } else {
            drop(state);
            WindowBounds::Windowed(self.bounds())
        }
    }

    fn inner_window_bounds(&self) -> WindowBounds {
        let state = self.borrow();
        if state.fullscreen {
            WindowBounds::Fullscreen(state.window_bounds)
        } else if state.maximized {
            WindowBounds::Maximized(state.window_bounds)
        } else {
            let inset = state.inset();
            drop(state);
            WindowBounds::Windowed(self.bounds().inset(inset))
        }
    }

    fn content_size(&self) -> Size<Pixels> {
        self.borrow().bounds.size
    }

    fn resize(&mut self, size: Size<Pixels>) {
        let state = self.borrow();
        let state_ptr = self.0.clone();

        // 弹窗的位置由合成器决定，调整大小时重新运行 positioner 并通过
        // configure 回复驱动缓冲区调整。在第一次 configure 之前弹窗尚未映射，
        // 不能 reposition，但初始 positioner 已经携带了大小信息。
        if matches!(state.surface_state, WaylandSurfaceState::Popup(_)) {
            if state.acknowledged_first_configure {
                let parent_geometry = state
                    .parent
                    .as_ref()
                    .map(|parent| parent.window_geometry())
                    .unwrap_or_default();
                state
                    .surface_state
                    .reposition_popup(&state.globals, size, parent_geometry);
            }
            return;
        }

        // 保持窗口几何形状与配置处理一致。在 Wayland 上，窗口几何形状是
        // 表面局部的：调整大小不应尝试移动窗口；合成器
        // 控制位置。我们还考虑了客户端装饰 inset 和平铺。
        let window_geometry = inset_by_tiling(
            Bounds {
                origin: Point::default(),
                size,
            },
            state.inset(),
            state.tiling,
        )
        .map(|v| f32::from(v) as i32)
        .map_size(|v| if v <= 0 { 1 } else { v });

        state.surface_state.set_geometry(
            window_geometry.origin.x,
            window_geometry.origin.y,
            window_geometry.size.width,
            window_geometry.size.height,
        );

        state
            .globals
            .executor
            .spawn(async move { state_ptr.resize(size) })
            .detach();
    }

    fn set_position(&mut self, position: Point<Pixels>) {
        // 在 Wayland 上，窗口位置由合成器控制
        // 这里我们只更新内部状态，实际位置可能不会被合成器尊重
        let state = self.borrow();
        let size = state.bounds.size;
        let _state_ptr = self.0.clone();

        let window_geometry = inset_by_tiling(
            Bounds {
                origin: position,
                size,
            },
            state.inset(),
            state.tiling,
        )
        .map(|v| f32::from(v) as i32)
        .map_size(|v| if v <= 0 { 1 } else { v });

        state.surface_state.set_geometry(
            window_geometry.origin.x,
            window_geometry.origin.y,
            window_geometry.size.width,
            window_geometry.size.height,
        );
    }

    fn scale_factor(&self) -> f32 {
        self.borrow().scale
    }

    fn appearance(&self) -> WindowAppearance {
        self.borrow().appearance
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        let state = self.borrow();
        state.display.as_ref().map(|(id, display)| {
            Rc::new(WaylandDisplay {
                id: id.clone(),
                name: display.name.clone(),
                bounds: display.bounds.to_pixels(state.scale),
            }) as Rc<dyn PlatformDisplay>
        })
    }

    fn mouse_position(&self) -> Point<Pixels> {
        self.borrow()
            .client
            .get_client()
            .borrow()
            .mouse_location
            .unwrap_or_default()
    }

    fn modifiers(&self) -> Modifiers {
        self.borrow().client.get_client().borrow().modifiers
    }

    fn capslock(&self) -> Capslock {
        self.borrow().client.get_client().borrow().capslock
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        self.borrow_mut().input_handler = Some(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.borrow_mut().input_handler.take()
    }

    fn prompt(
        &self,
        _level: PromptLevel,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[PromptButton],
    ) -> Option<Receiver<usize>> {
        None
    }

    fn activate(&self) {
        // 尝试请求激活令牌。即使激活很可能被拒绝，
        // KWin 和 Mutter 也可以使用 app_id 以视觉方式指示我们正在请求关注
        self.0.request_activation();
    }

    fn is_active(&self) -> bool {
        self.borrow().active
    }

    fn is_hovered(&self) -> bool {
        self.borrow().hovered
    }

    fn set_title(&mut self, title: &str) {
        self.borrow_mut().title = title.to_owned();
        if let Some(toplevel) = self.borrow().surface_state.toplevel() {
            toplevel.set_title(title.to_string());
        }
    }

    fn get_title(&self) -> String {
        self.borrow().title.clone()
    }

    fn set_app_id(&mut self, app_id: &str) {
        let mut state = self.borrow_mut();
        if let Some(toplevel) = state.surface_state.toplevel() {
            toplevel.set_app_id(app_id.to_owned());
        }
        state.app_id = Some(app_id.to_owned());
    }

    fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance) {
        let mut state = self.borrow_mut();
        state.background_appearance = background_appearance;
        update_window(state);
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.borrow().background_appearance
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        let client = self.borrow().client.get_client();
        let state = client.borrow();
        state
            .gpu_context
            .borrow()
            .as_ref()
            .is_some_and(|ctx| ctx.supports_dual_source_blending())
    }

    fn minimize(&self) {
        self.0.minimize_window();
    }

    fn hide(&self) {
        self.0.hide_window();
    }

    fn zoom(&self) {
        let state = self.borrow();
        if let Some(toplevel) = state.surface_state.toplevel() {
            if !state.maximized {
                toplevel.set_maximized();
            } else {
                toplevel.unset_maximized();
            }
        }
    }

    fn toggle_fullscreen(&self) {
        let state = self.borrow();
        if let Some(toplevel) = state.surface_state.toplevel() {
            if !state.fullscreen {
                toplevel.set_fullscreen(None);
            } else {
                toplevel.unset_fullscreen();
            }
        }
    }

    fn is_fullscreen(&self) -> bool {
        self.borrow().fullscreen
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.0.callbacks.borrow_mut().request_frame = Some(callback);
    }

    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> rgpui::DispatchEventResult>) {
        self.0.callbacks.borrow_mut().input = Some(callback);
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0.callbacks.borrow_mut().active_status_change = Some(callback);
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0.callbacks.borrow_mut().hover_status_change = Some(callback);
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.0.callbacks.borrow_mut().resize = Some(callback);
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        self.0.callbacks.borrow_mut().moved = Some(callback);
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.0.callbacks.borrow_mut().should_close = Some(callback);
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.0.callbacks.borrow_mut().close = Some(callback);
    }

    fn on_hit_test_window_control(&self, _callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        // 核心只在 Windows / Web 注册该回调（见 rgpui/src/window.rs 的注册处）。
        // Wayland 是客户端装饰，拖拽与窗口按钮由 TitleBar 的显式鼠标事件 +
        // start_window_move / zoom_window 接管，这里保留 trait 契约的空实现。
    }

    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.callbacks.borrow_mut().appearance_changed = Some(callback);
    }

    fn on_button_layout_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.callbacks.borrow_mut().button_layout_changed = Some(callback);
    }

    fn draw(&self, scene: &Scene) {
        let mut state = self.borrow_mut();
        if !state.mapping.can_render() || !state.acknowledged_first_configure {
            return;
        }

        if state.renderer.device_lost() {
            let raw_window = RawWindow {
                window: state.surface.id().as_ptr().cast::<std::ffi::c_void>(),
                display: state
                    .surface
                    .backend()
                    .upgrade()
                    .unwrap()
                    .display_ptr()
                    .cast::<std::ffi::c_void>(),
            };
            match state.renderer.recover(&raw_window) {
                Ok(()) => {}
                Err(err) => {
                    log::warn!("GPU recovery failed, will retry on next frame: {err}");
                }
            }

            state.force_render_after_recovery = true;
            return;
        }

        state.renderer_presented = state.renderer.draw(scene);

        if state.renderer.needs_redraw() {
            state.force_render_after_recovery = true;
        }
    }

    fn completed_frame(&self) {
        let mut state = self.borrow_mut();
        if !state.mapping.can_render() || !state.acknowledged_first_configure {
            return;
        }

        // 解决旧版本 wlroots 中的一个问题：在未附加缓冲区的情况下提交
        // 可能导致无效同步，从而导致图形损坏
        if !state.renderer_presented {
            state.surface.commit();
        }

        let restored = state.renderer_presented && state.mapping.presented();
        state.renderer_presented = false;
        if !restored && !state.mapping.is_visible() && !state.remap_retry_scheduled {
            // 尚未映射的窗口可能收不到 frame 回调；首帧获取缓冲区失败时主动重试。
            state.remap_retry_scheduled = true;
            let generation = state.mapping.generation();
            let weak_state = Rc::downgrade(&self.0.state);
            let weak_callbacks = Rc::downgrade(&self.0.callbacks);
            state
                .globals
                .executor
                .spawn(async move {
                    smol::Timer::after(std::time::Duration::from_millis(16)).await;
                    let (Some(state), Some(callbacks)) =
                        (weak_state.upgrade(), weak_callbacks.upgrade())
                    else {
                        return;
                    };
                    {
                        let mut state = state.borrow_mut();
                        if state.mapping.generation() != generation {
                            return;
                        }
                        state.remap_retry_scheduled = false;
                        if state.mapping.is_visible() || !state.mapping.can_render() {
                            return;
                        }
                        state.force_render_after_recovery = true;
                    }
                    WaylandWindowStatePtr { state, callbacks }.frame();
                })
                .detach();
        }
        if restored {
            let token = state.restore_activation_token.take();
            let children = std::mem::take(&mut state.hidden_children);
            let client = state.client.get_client();
            drop(state);
            // 缓冲区已经提交，合成器才有可激活的窗口；此前不得消费单次令牌。
            if let Some(token) = token {
                self.0.activate_with_token(token);
            } else {
                self.0.request_activation();
            }
            for id in children {
                let child = get_window(&mut client.borrow_mut(), &id);
                if let Some(child) = child {
                    child.request_activation();
                }
            }
            log::debug!("Wayland window remapped and first frame presented");
        }
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        let state = self.borrow();
        state.renderer.sprite_atlas().clone()
    }

    /// 把当前帧的场景离屏渲染并回读像素（仅测试用途，不呈现）。
    ///
    /// 与 X11 走的是 `WgpuRenderer` 里同一条编码路径；尺寸取当前 surface 配置，
    /// 所以 surface 尚未配置（窗口还没拿到首帧尺寸）时会直接返回错误。
    #[cfg(feature = "test-support")]
    fn render_to_image(&self, scene: &Scene) -> anyhow::Result<image::RgbaImage> {
        let mut state = self.borrow_mut();
        let (width, height, pixels) = state.renderer.render_scene_to_pixels(scene)?;
        image::RgbaImage::from_raw(width, height, pixels)
            .ok_or_else(|| anyhow::anyhow!("回读像素与 {width}x{height} 尺寸不符"))
    }

    fn show_window_menu(&self, position: Point<Pixels>) {
        let state = self.borrow();
        let serial = state.client.get_serial(SerialKind::MousePress);
        if let Some(toplevel) = state.surface_state.toplevel() {
            toplevel.show_window_menu(
                &state.globals.seat,
                serial,
                f32::from(position.x) as i32,
                f32::from(position.y) as i32,
            );
        }
    }

    fn start_window_move(&self) {
        let state = self.borrow();
        let serial = state.client.get_serial(SerialKind::MousePress);
        if let Some(toplevel) = state.surface_state.toplevel() {
            toplevel._move(&state.globals.seat, serial);
        }
    }

    fn start_window_resize(&self, edge: rgpui::ResizeEdge) {
        let state = self.borrow();
        if let Some(toplevel) = state.surface_state.toplevel() {
            toplevel.resize(
                &state.globals.seat,
                state.client.get_serial(SerialKind::MousePress),
                edge.to_xdg(),
            )
        }
    }

    /// 鼠标穿透：Wayland 没有 X Shape 那样的独立扩展，
    /// 「空输入区域」就是通用做法 —— 窗口照常合成显示，事件落到下层
    fn set_mouse_passthrough(&self, passthrough: bool) {
        if passthrough {
            self.set_input_region(Some(&[]));
        } else {
            self.set_input_region(None);
        }
    }

    fn set_input_region(&self, region: Option<&[rgpui::Bounds<rgpui::Pixels>]>) {
        let state = self.borrow();
        match region {
            None => state.surface.set_input_region(None),
            Some(rects) => {
                let wl_region = state
                    .globals
                    .compositor
                    .create_region(&state.globals.qh, ());
                for rect in rects {
                    let rect = rect.map(|pixels| f32::from(pixels) as i32);
                    wl_region.add(
                        rect.origin.x,
                        rect.origin.y,
                        rect.size.width,
                        rect.size.height,
                    );
                }
                state.surface.set_input_region(Some(&wl_region));
                wl_region.destroy();
            }
        }
        state.surface.commit();
    }

    /// 运行时改 layer-shell 的 `exclusive_zone`：只对 layer-shell 窗口有意义，
    /// 其他窗口类型没有该协议对象，直接跳过
    fn set_exclusive_zone(&self, zone: Pixels) {
        let state = self.borrow();
        let Some(layer_surface) = state.surface_state.layer_surface() else {
            log::debug!("独占区域只对 layer-shell 窗口有效，当前窗口不是该类型");
            return;
        };
        layer_surface.set_exclusive_zone(f32::from(zone) as i32);
        state.surface.commit();
    }

    /// 独占区域作用的边缘。合成器自己校验「必须是单一边缘」，
    /// 非法值由合成器忽略，这里只负责把请求发出去并提交
    fn set_exclusive_edge(&self, edge: Anchor) {
        let state = self.borrow();
        let Some(layer_surface) = state.surface_state.layer_surface() else {
            log::debug!("独占边缘只对 layer-shell 窗口有效，当前窗口不是该类型");
            return;
        };
        layer_surface.set_exclusive_edge(super::layer_shell::wayland_anchor(edge));
        state.surface.commit();
    }

    fn window_decorations(&self) -> Decorations {
        let state = self.borrow();
        match state.decorations {
            WindowDecorations::Server => Decorations::Server,
            WindowDecorations::Client => Decorations::Client {
                tiling: state.tiling,
            },
        }
    }

    fn request_decorations(&self, decorations: WindowDecorations) {
        let mut state = self.borrow_mut();
        match state.surface_state.decoration().as_ref() {
            Some(decoration) => {
                decoration.set_mode(decorations.to_xdg());
                state.decorations = decorations;
                update_window(state);
            }
            None => {
                if matches!(decorations, WindowDecorations::Server) {
                    log::info!(
                        "Server-side decorations requested, but the Wayland server does not support them. Falling back to client-side decorations."
                    );
                }
                state.decorations = WindowDecorations::Client;
                update_window(state);
            }
        }
    }

    fn window_controls(&self) -> WindowControls {
        let state = self.borrow();
        WindowControls {
            maximize: state.window_controls.maximize && state.is_resizable,
            minimize: state.window_controls.minimize && state.is_minimizable,
            ..state.window_controls
        }
    }

    fn set_client_inset(&self, inset: Pixels) {
        let mut state = self.borrow_mut();
        if Some(inset) != state.client_inset {
            state.client_inset = Some(inset);
            update_window(state);
        }
    }

    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        let state = self.borrow();
        // 非激活窗口不更新 IME 位置，避免 Wayland 下误触发输入法候选框移动
        if !state.active {
            return;
        }
        state.client.update_ime_position(bounds);
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.borrow().renderer.gpu_specs().into()
    }

    fn play_system_bell(&self) {
        let state = self.borrow();
        let surface = if state.surface_state.toplevel().is_some() {
            Some(&state.surface)
        } else {
            None
        };
        if let Some(bell) = state.globals.system_bell.as_ref() {
            bell.ring(surface);
        }
    }

    fn a11y_init(&self, callbacks: rgpui::A11yCallbacks) {
        let activation_handler = TrivialActivationHandler {
            callback: callbacks.activation,
        };
        let action_handler = TrivialActionHandler(callbacks.action);
        let deactivation_handler = TrivialDeactivationHandler {
            callback: callbacks.deactivation,
        };

        let adapter =
            accesskit_unix::Adapter::new(activation_handler, action_handler, deactivation_handler);

        self.borrow_mut().accesskit_adapter = Some(adapter);
    }

    fn a11y_tree_update(&self, tree_update: accesskit::TreeUpdate) {
        let mut state = self.borrow_mut();
        if let Some(adapter) = state.accesskit_adapter.as_mut() {
            adapter.update_if_active(|| tree_update);
        }
    }
}

fn update_window(mut state: RefMut<WaylandWindowState>) {
    let opaque = !state.is_transparent();

    state.renderer.update_transparency(!opaque);
    let opaque_area = state.window_bounds.map(|v| f32::from(v) as i32);
    opaque_area.inset(f32::from(state.inset()) as i32);

    let region = state
        .globals
        .compositor
        .create_region(&state.globals.qh, ());
    region.add(
        opaque_area.origin.x,
        opaque_area.origin.y,
        opaque_area.size.width,
        opaque_area.size.height,
    );

    // 注意：圆角使这个矩形 API 难以使用
    // 由于使用 CSD 时这很常见，我们直接禁用此 API
    if state.background_appearance == WindowBackgroundAppearance::Opaque
        && state.decorations == WindowDecorations::Server
    {
        // 向合成器承诺窗口表面的这个区域
        // 不包含透明像素。这允许合成器跳过
        // 更新表面后面的内容以获得更好的性能
        state.surface.set_opaque_region(Some(&region));
    } else {
        state.surface.set_opaque_region(None);
    }

    if let Some(ref blur_manager) = state.globals.blur_manager {
        if state.background_appearance == WindowBackgroundAppearance::Blurred {
            if state.blur.is_none() {
                let blur = blur_manager.create(&state.surface, &state.globals.qh, ());
                state.blur = Some(blur);
            }
            state.blur.as_ref().unwrap().commit();
        } else {
            // 为不透明窗口清除模糊效果可能也无妨
            blur_manager.unset(&state.surface);
            if let Some(b) = state.blur.take() {
                b.release()
            }
        }
    }

    region.destroy();
}

pub(crate) trait WindowDecorationsExt {
    fn to_xdg(self) -> zxdg_toplevel_decoration_v1::Mode;
}

impl WindowDecorationsExt for WindowDecorations {
    fn to_xdg(self) -> zxdg_toplevel_decoration_v1::Mode {
        match self {
            WindowDecorations::Client => zxdg_toplevel_decoration_v1::Mode::ClientSide,
            WindowDecorations::Server => zxdg_toplevel_decoration_v1::Mode::ServerSide,
        }
    }
}

pub(crate) trait ResizeEdgeWaylandExt {
    fn to_xdg(self) -> xdg_toplevel::ResizeEdge;
}

impl ResizeEdgeWaylandExt for ResizeEdge {
    fn to_xdg(self) -> xdg_toplevel::ResizeEdge {
        match self {
            ResizeEdge::Top => xdg_toplevel::ResizeEdge::Top,
            ResizeEdge::TopRight => xdg_toplevel::ResizeEdge::TopRight,
            ResizeEdge::Right => xdg_toplevel::ResizeEdge::Right,
            ResizeEdge::BottomRight => xdg_toplevel::ResizeEdge::BottomRight,
            ResizeEdge::Bottom => xdg_toplevel::ResizeEdge::Bottom,
            ResizeEdge::BottomLeft => xdg_toplevel::ResizeEdge::BottomLeft,
            ResizeEdge::Left => xdg_toplevel::ResizeEdge::Left,
            ResizeEdge::TopLeft => xdg_toplevel::ResizeEdge::TopLeft,
        }
    }
}

/// 配置事件是基于窗口几何形状的，我们不断更新它以考虑客户端装饰
/// 但由于我们的侵入式 CSD，这不是我们要渲染的区域
/// 因此，这里我们通过重新加入 inset 来计算"实际"大小
fn compute_outer_size(
    inset: Pixels,
    new_size: Option<Size<Pixels>>,
    tiling: Tiling,
) -> Option<Size<Pixels>> {
    new_size.map(|mut new_size| {
        if !tiling.top {
            new_size.height += inset;
        }
        if !tiling.bottom {
            new_size.height += inset;
        }
        if !tiling.left {
            new_size.width += inset;
        }
        if !tiling.right {
            new_size.width += inset;
        }

        new_size
    })
}

fn inset_by_tiling(mut bounds: Bounds<Pixels>, inset: Pixels, tiling: Tiling) -> Bounds<Pixels> {
    if !tiling.top {
        bounds.origin.y += inset;
        bounds.size.height -= inset;
    }
    if !tiling.bottom {
        bounds.size.height -= inset;
    }
    if !tiling.left {
        bounds.origin.x += inset;
        bounds.size.width -= inset;
    }
    if !tiling.right {
        bounds.size.width -= inset;
    }

    bounds
}

struct TrivialActivationHandler {
    callback: Box<dyn Fn() -> Option<accesskit::TreeUpdate> + Send + 'static>,
}

impl accesskit::ActivationHandler for TrivialActivationHandler {
    fn request_initial_tree(&mut self) -> Option<accesskit::TreeUpdate> {
        (self.callback)()
    }
}

struct TrivialActionHandler(Box<dyn Fn(accesskit::ActionRequest) + Send + 'static>);

impl accesskit::ActionHandler for TrivialActionHandler {
    fn do_action(&mut self, request: accesskit::ActionRequest) {
        (self.0)(request);
    }
}

struct TrivialDeactivationHandler {
    callback: Box<dyn Fn() + Send + 'static>,
}

impl accesskit::DeactivationHandler for TrivialDeactivationHandler {
    fn deactivate_accessibility(&mut self) {
        (self.callback)();
    }
}
