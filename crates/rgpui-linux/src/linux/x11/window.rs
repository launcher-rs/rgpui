use anyhow::{Context as _, anyhow};
use x11rb::connection::RequestConnection;

use crate::linux::X11ClientStatePtr;
use rgpui::{
    AnyWindowHandle, Bounds, Decorations, DevicePixels, ForegroundExecutor, GpuSpecs, Modifiers,
    Pixels, PlatformAtlas, PlatformDisplay, PlatformInput, PlatformInputHandler, PlatformWindow,
    Point, PromptButton, PromptLevel, RequestFrameOptions, ResizeEdge, ScaledPixels, Scene, Size,
    Tiling, WindowAppearance, WindowBackgroundAppearance, WindowBounds, WindowControlArea,
    WindowDecorations, WindowKind, WindowParams, px,
};
use rgpui_wgpu::{CompositorGpuHint, WgpuRenderer, WgpuSurfaceConfig};

use raw_window_handle as rwh;
use rgpui::ResultExt;
use rgpui::collections::FxHashSet;
use rgpui::layer_shell::Anchor;
use rgpui::maybe;
use x11rb::{
    connection::Connection,
    cookie::{Cookie, VoidCookie},
    errors::ConnectionError,
    properties::WmSizeHints,
    protocol::{
        shape::{self, ConnectionExt as _},
        sync,
        xinput::{self, ConnectionExt as _},
        xproto::{self, ClientMessageEvent, ConnectionExt, TranslateCoordinatesReply},
    },
    wrapper::ConnectionExt as _,
    xcb_ffi::XCBConnection,
};

use std::{
    cell::RefCell, ffi::c_void, fmt::Display, num::NonZeroU32, ptr::NonNull, rc::Rc, sync::Arc,
};

use super::{X11Display, XINPUT_ALL_DEVICE_GROUPS, XINPUT_ALL_DEVICES};

x11rb::atom_manager! {
    pub XcbAtoms: AtomsCookie {
        XA_ATOM,
        XdndAware,
        XdndStatus,
        XdndEnter,
        XdndLeave,
        XdndPosition,
        XdndSelection,
        XdndDrop,
        XdndFinished,
        XdndTypeList,
        XdndActionCopy,
        TextUriList: b"text/uri-list",
        UTF8_STRING,
        TEXT,
        STRING,
        TEXT_PLAIN_UTF8: b"text/plain;charset=utf-8",
        TEXT_PLAIN: b"text/plain",
        XDND_DATA,
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
        WM_CHANGE_STATE,
        WM_TRANSIENT_FOR,
        _NET_WM_PID,
        _NET_WM_NAME,
        _NET_WM_ICON,
        _NET_WM_STATE,
        _NET_WM_STATE_MAXIMIZED_VERT,
        _NET_WM_STATE_MAXIMIZED_HORZ,
        _NET_WM_STATE_FULLSCREEN,
        _NET_WM_STATE_HIDDEN,
        _NET_WM_STATE_FOCUSED,
        _NET_WM_STATE_ABOVE,
        _NET_WM_STATE_MODAL,
        _NET_WM_STATE_SYNC,
        _NET_ACTIVE_WINDOW,
        _NET_WM_SYNC_REQUEST,
        _NET_WM_SYNC_REQUEST_COUNTER,
        _NET_WM_BYPASS_COMPOSITOR,
        _NET_WM_MOVERESIZE,
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_NOTIFICATION,
        _NET_WM_WINDOW_TYPE_DIALOG,
        _NET_WM_WINDOW_TYPE_DOCK,
        _NET_WM_STRUT,
        _NET_WM_STRUT_PARTIAL,
        _NET_WM_WINDOW_OPACITY,
        _NET_WM_SYNC,
        _NET_SUPPORTED,
        _MOTIF_WM_HINTS,
        _GTK_SHOW_WINDOW_MENU,
        _GTK_FRAME_EXTENTS,
        _GTK_EDGE_CONSTRAINTS,
        _NET_CLIENT_LIST_STACKING,
    }
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

fn query_render_extent(
    xcb: &Rc<XCBConnection>,
    x_window: xproto::Window,
) -> anyhow::Result<Size<DevicePixels>> {
    let reply = get_reply(|| "X11 GetGeometry failed.", xcb.get_geometry(x_window))?;
    Ok(Size {
        width: DevicePixels(reply.width as i32),
        height: DevicePixels(reply.height as i32),
    })
}

fn resize_edge_to_moveresize(edge: ResizeEdge) -> u32 {
    match edge {
        ResizeEdge::TopLeft => 0,
        ResizeEdge::Top => 1,
        ResizeEdge::TopRight => 2,
        ResizeEdge::Right => 3,
        ResizeEdge::BottomRight => 4,
        ResizeEdge::Bottom => 5,
        ResizeEdge::BottomLeft => 6,
        ResizeEdge::Left => 7,
    }
}

/// strut 只能保留一条边，而 `Anchor` 允许任意组合（例如左右都锚定的横向面板），
/// 组合起来就无从判断保留哪条，此时返回 `None`
fn single_edge(edge: Anchor) -> Option<Anchor> {
    (edge.bits().count_ones() == 1).then_some(edge)
}

/// 算出 EWMH `_NET_WM_STRUT_PARTIAL` 的 12 个值，顺序为 left、right、top、bottom、
/// left_start_y、left_end_y、right_start_y、right_end_y、top_start_x、top_end_x、
/// bottom_start_x、bottom_end_x，全部物理像素。
///
/// 口径与 Wayland layer-shell 一致：`zone` 是从指定的屏幕边缘起保留的条带宽度（含窗口
/// 自身），所以贴边面板直接传面板尺寸。`zone` 非正、或边缘不是单一边缘时返回 `None`
/// —— 由调用方删除属性。四个跨度（start/end）取窗口在垂直轴上的闭区间，多屏时
/// 只有窗口所在的那块屏幕会被让出空间。
fn strut_partial(
    edge: Anchor,
    origin: Point<i32>,
    size: Size<u32>,
    zone: u32,
) -> Option<[u32; 12]> {
    if zone == 0 {
        return None;
    }

    let x = u32::try_from(origin.x).unwrap_or(0);
    let y = u32::try_from(origin.y).unwrap_or(0);
    // 规范里跨度是闭区间的像素序号，所以末端要减 1
    let far_x = x.saturating_add(size.width).saturating_sub(1);
    let far_y = y.saturating_add(size.height).saturating_sub(1);

    let mut strut = [0; 12];
    if edge == Anchor::LEFT {
        strut[0] = zone;
        strut[4] = y;
        strut[5] = far_y;
    } else if edge == Anchor::RIGHT {
        strut[1] = zone;
        strut[6] = y;
        strut[7] = far_y;
    } else if edge == Anchor::TOP {
        strut[2] = zone;
        strut[8] = x;
        strut[9] = far_x;
    } else if edge == Anchor::BOTTOM {
        strut[3] = zone;
        strut[10] = x;
        strut[11] = far_x;
    } else {
        return None;
    }

    Some(strut)
}

#[derive(Debug)]
struct EdgeConstraints {
    top_tiled: bool,
    right_tiled: bool,
    bottom_tiled: bool,
    left_tiled: bool,
}

impl EdgeConstraints {
    fn from_atom(atom: u32) -> Self {
        EdgeConstraints {
            top_tiled: (atom & (1 << 0)) != 0,
            right_tiled: (atom & (1 << 2)) != 0,
            bottom_tiled: (atom & (1 << 4)) != 0,
            left_tiled: (atom & (1 << 6)) != 0,
        }
    }

    fn to_tiling(&self) -> Tiling {
        Tiling {
            top: self.top_tiled,
            right: self.right_tiled,
            bottom: self.bottom_tiled,
            left: self.left_tiled,
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct Visual {
    id: xproto::Visualid,
    colormap: u32,
    depth: u8,
}

struct VisualSet {
    inherit: Visual,
    opaque: Option<Visual>,
    transparent: Option<Visual>,
    root: u32,
    black_pixel: u32,
}

fn find_visuals(xcb: &XCBConnection, screen_index: usize) -> VisualSet {
    let screen = &xcb.setup().roots[screen_index];
    let mut set = VisualSet {
        inherit: Visual {
            id: screen.root_visual,
            colormap: screen.default_colormap,
            depth: screen.root_depth,
        },
        opaque: None,
        transparent: None,
        root: screen.root,
        black_pixel: screen.black_pixel,
    };

    for depth_info in screen.allowed_depths.iter() {
        for visual_type in depth_info.visuals.iter() {
            let visual = Visual {
                id: visual_type.visual_id,
                colormap: 0,
                depth: depth_info.depth,
            };
            log::debug!(
                "Visual id: {}, 类: {:?}, 深度: {}, 每值位数: {}, 掩码: 0x{:x} 0x{:x} 0x{:x}",
                visual_type.visual_id,
                visual_type.class,
                depth_info.depth,
                visual_type.bits_per_rgb_value,
                visual_type.red_mask,
                visual_type.green_mask,
                visual_type.blue_mask,
            );

            if (
                visual_type.red_mask,
                visual_type.green_mask,
                visual_type.blue_mask,
            ) != (0xFF0000, 0xFF00, 0xFF)
            {
                continue;
            }
            let color_mask = visual_type.red_mask | visual_type.green_mask | visual_type.blue_mask;
            let alpha_mask = color_mask as usize ^ ((1usize << depth_info.depth) - 1);

            if alpha_mask == 0 {
                if set.opaque.is_none() {
                    set.opaque = Some(visual);
                }
            } else {
                if set.transparent.is_none() {
                    set.transparent = Some(visual);
                }
            }
        }
    }

    set
}

#[derive(Debug, Clone, Copy)]
struct RawWindow {
    connection: *mut c_void,
    screen_id: usize,
    window_id: u32,
    visual_id: u32,
}

// 安全性：RawWindow 中的原始指针指向窗口生命周期内有效的 X11 连接
// 这些仅用于传递给 wgpu，后者需要 Send+Sync 来创建表面
unsafe impl Send for RawWindow {}
unsafe impl Sync for RawWindow {}

#[derive(Default)]
pub struct Callbacks {
    request_frame: Option<Box<dyn FnMut(RequestFrameOptions)>>,
    input: Option<Box<dyn FnMut(PlatformInput) -> rgpui::DispatchEventResult>>,
    active_status_change: Option<Box<dyn FnMut(bool)>>,
    hovered_status_change: Option<Box<dyn FnMut(bool)>>,
    resize: Option<Box<dyn FnMut(Size<Pixels>, f32)>>,
    moved: Option<Box<dyn FnMut()>>,
    should_close: Option<Box<dyn FnMut() -> bool>>,
    close: Option<Box<dyn FnOnce()>>,
    appearance_changed: Option<Box<dyn FnMut()>>,
    button_layout_changed: Option<Box<dyn FnMut()>>,
}

pub struct X11WindowState {
    pub destroyed: bool,
    parent: Option<X11WindowStatePtr>,
    children: FxHashSet<xproto::Window>,
    client: X11ClientStatePtr,
    executor: ForegroundExecutor,
    atoms: XcbAtoms,
    x_root_window: xproto::Window,
    x_screen_index: usize,
    visual_id: u32,
    pub(crate) counter_id: sync::Counter,
    pub(crate) last_sync_counter: Option<sync::Int64>,
    bounds: Bounds<Pixels>,
    scale_factor: f32,
    renderer: WgpuRenderer,
    display: Rc<dyn PlatformDisplay>,
    input_handler: Option<PlatformInputHandler>,
    appearance: WindowAppearance,
    background_appearance: WindowBackgroundAppearance,
    maximized_vertical: bool,
    maximized_horizontal: bool,
    hidden: bool,
    active: bool,
    hovered: bool,
    pub(crate) force_render_after_recovery: bool,
    fullscreen: bool,
    client_side_decorations_supported: bool,
    decorations: WindowDecorations,
    edge_constraints: Option<EdgeConstraints>,
    /// 独占区域宽度（逻辑像素），非正值表示不保留
    exclusive_zone: Pixels,
    /// 独占区域作用于哪条屏幕边缘；X11 strut 必须知道边，`None` 时不写属性
    exclusive_edge: Option<Anchor>,
    pub handle: AnyWindowHandle,
    last_insets: [u32; 4],
    accesskit_adapter: Option<accesskit_unix::Adapter>,
}

impl X11WindowState {
    fn is_transparent(&self) -> bool {
        self.background_appearance != WindowBackgroundAppearance::Opaque
    }
}

#[derive(Clone)]
pub(crate) struct X11WindowStatePtr {
    pub state: Rc<RefCell<X11WindowState>>,
    pub(crate) callbacks: Rc<RefCell<Callbacks>>,
    xcb: Rc<XCBConnection>,
    pub(crate) x_window: xproto::Window,
}

impl rwh::HasWindowHandle for RawWindow {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let Some(non_zero) = NonZeroU32::new(self.window_id) else {
            log::error!("RawWindow.window_id zero when getting window handle.");
            return Err(rwh::HandleError::Unavailable);
        };
        let mut handle = rwh::XcbWindowHandle::new(non_zero);
        handle.visual_id = NonZeroU32::new(self.visual_id);
        Ok(unsafe { rwh::WindowHandle::borrow_raw(handle.into()) })
    }
}
impl rwh::HasDisplayHandle for RawWindow {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let Some(non_zero) = NonNull::new(self.connection) else {
            log::error!("Null RawWindow.connection when getting display handle.");
            return Err(rwh::HandleError::Unavailable);
        };
        let handle = rwh::XcbDisplayHandle::new(Some(non_zero), self.screen_id as i32);
        Ok(unsafe { rwh::DisplayHandle::borrow_raw(handle.into()) })
    }
}

impl rwh::HasWindowHandle for X11Window {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let Some(non_zero) = NonZeroU32::new(self.0.x_window) else {
            return Err(rwh::HandleError::Unavailable);
        };
        let handle = rwh::XcbWindowHandle::new(non_zero);
        Ok(unsafe { rwh::WindowHandle::borrow_raw(handle.into()) })
    }
}

impl rwh::HasDisplayHandle for X11Window {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let connection =
            as_raw_xcb_connection::AsRawXcbConnection::as_raw_xcb_connection(&*self.0.xcb)
                as *mut _;
        let Some(non_zero) = NonNull::new(connection) else {
            return Err(rwh::HandleError::Unavailable);
        };
        let screen_id = {
            let state = self.0.state.borrow();
            u64::from(state.display.id()) as i32
        };
        let handle = rwh::XcbDisplayHandle::new(Some(non_zero), screen_id);
        Ok(unsafe { rwh::DisplayHandle::borrow_raw(handle.into()) })
    }
}

pub(crate) fn xcb_flush(xcb: &XCBConnection) {
    xcb.flush()
        .map_err(handle_connection_error)
        .context("X11 flush failed")
        .log_err();
}

pub(crate) fn check_reply<E, F, C>(
    failure_context: F,
    result: Result<VoidCookie<'_, C>, ConnectionError>,
) -> anyhow::Result<()>
where
    E: Display + Send + Sync + 'static,
    F: FnOnce() -> E,
    C: RequestConnection,
{
    result
        .map_err(handle_connection_error)
        .and_then(|response| response.check().map_err(|reply_error| anyhow!(reply_error)))
        .with_context(failure_context)
}

pub(crate) fn get_reply<E, F, C, O>(
    failure_context: F,
    result: Result<Cookie<'_, C, O>, ConnectionError>,
) -> anyhow::Result<O>
where
    E: Display + Send + Sync + 'static,
    F: FnOnce() -> E,
    C: RequestConnection,
    O: x11rb::x11_utils::TryParse,
{
    result
        .map_err(handle_connection_error)
        .and_then(|response| response.reply().map_err(|reply_error| anyhow!(reply_error)))
        .with_context(failure_context)
}

/// Convert X11 connection errors to `anyhow::Error` and panic for unrecoverable errors.
pub(crate) fn handle_connection_error(err: ConnectionError) -> anyhow::Error {
    match err {
        ConnectionError::UnknownError => anyhow!("X11 connection: Unknown error"),
        ConnectionError::UnsupportedExtension => anyhow!("X11 connection: Unsupported extension"),
        ConnectionError::MaximumRequestLengthExceeded => {
            anyhow!("X11 connection: Maximum request length exceeded")
        }
        ConnectionError::FdPassingFailed => {
            panic!("X11 connection: File descriptor passing failed")
        }
        ConnectionError::ParseError(parse_error) => {
            anyhow!(parse_error).context("Parse error in X11 response")
        }
        ConnectionError::InsufficientMemory => panic!("X11 connection: Insufficient memory"),
        ConnectionError::IoError(err) => anyhow!(err).context("X11 connection: IOError"),
        _ => anyhow!(err),
    }
}

impl X11WindowState {
    pub fn new(
        handle: AnyWindowHandle,
        client: X11ClientStatePtr,
        executor: ForegroundExecutor,
        gpu_context: rgpui_wgpu::GpuContext,
        compositor_gpu: Option<CompositorGpuHint>,
        params: WindowParams,
        xcb: &Rc<XCBConnection>,
        client_side_decorations_supported: bool,
        x_main_screen_index: usize,
        x_window: xproto::Window,
        atoms: &XcbAtoms,
        scale_factor: f32,
        appearance: WindowAppearance,
        parent_window: Option<X11WindowStatePtr>,
        supports_xinput_gestures: bool,
        is_bgr: bool,
    ) -> anyhow::Result<Self> {
        if let WindowKind::AnchoredPopup(_) = params.kind {
            return Err(rgpui::popup::PopupNotSupportedError.into());
        }

        let x_screen_index = params
            .display_id
            .map_or(x_main_screen_index, |did| u64::from(did) as usize);

        let visual_set = find_visuals(xcb, x_screen_index);

        // 只有需要透明合成的窗口才用 32 位 ARGB visual：不透明窗口用 ARGB 时交换链
        // 写入的 alpha 为 0，合成器会把整个窗口当作透明而看不到任何内容
        let needs_alpha = params.window_background != WindowBackgroundAppearance::Opaque
            || params.kind == WindowKind::Overlay;
        let visual = if needs_alpha {
            match visual_set.transparent {
                Some(visual) => visual,
                None => {
                    log::warn!("Unable to find a transparent visual",);
                    visual_set.inherit
                }
            }
        } else {
            visual_set.opaque.unwrap_or(visual_set.inherit)
        };
        log::info!("Using {:?}", visual);

        let colormap = if visual.colormap != 0 {
            visual.colormap
        } else {
            let id = xcb.generate_id()?;
            log::info!("Creating colormap {}", id);
            check_reply(
                || format!("X11 CreateColormap failed. id: {}", id),
                xcb.create_colormap(xproto::ColormapAlloc::NONE, id, visual_set.root, visual.id),
            )?;
            id
        };

        let win_aux = xproto::CreateWindowAux::new()
            // https://stackoverflow.com/questions/43218127/x11-xlib-xcb-creating-a-window-requires-border-pixel-if-specifying-colormap-wh
            .border_pixel(visual_set.black_pixel)
            .colormap(colormap)
            .override_redirect(
                matches!(params.kind, WindowKind::PopUp | WindowKind::Overlay) as u32,
            )
            .event_mask(
                xproto::EventMask::EXPOSURE
                    | xproto::EventMask::STRUCTURE_NOTIFY
                    | xproto::EventMask::FOCUS_CHANGE
                    | xproto::EventMask::KEY_PRESS
                    | xproto::EventMask::KEY_RELEASE
                    | xproto::EventMask::PROPERTY_CHANGE
                    | xproto::EventMask::VISIBILITY_CHANGE,
            );

        let mut bounds = params.bounds.to_device_pixels(scale_factor);
        if bounds.size.width.0 == 0 || bounds.size.height.0 == 0 {
            log::warn!(
                "窗口边界包含零值。height={}, width={}。回退到默认值",
                bounds.size.height.0,
                bounds.size.width.0
            );
            bounds.size.width = 800.into();
            bounds.size.height = 600.into();
        }

        check_reply(
            || {
                format!(
                    "X11 CreateWindow failed. depth: {}, x_window: {}, visual_set.root: {}, bounds.origin.x.0: {}, bounds.origin.y.0: {}, bounds.size.width.0: {}, bounds.size.height.0: {}",
                    visual.depth,
                    x_window,
                    visual_set.root,
                    bounds.origin.x.0 + 2,
                    bounds.origin.y.0,
                    bounds.size.width.0,
                    bounds.size.height.0
                )
            },
            xcb.create_window(
                visual.depth,
                x_window,
                visual_set.root,
                (bounds.origin.x.0 + 2) as i16,
                bounds.origin.y.0 as i16,
                bounds.size.width.0 as u16,
                bounds.size.height.0 as u16,
                0,
                xproto::WindowClass::INPUT_OUTPUT,
                visual.id,
                &win_aux,
            ),
        )?;

        // 收集设置过程中的错误，以便在失败时销毁窗口
        let setup_result = maybe!({
            let pid = std::process::id();
            check_reply(
                || "X11 ChangeProperty for _NET_WM_PID failed.",
                xcb.change_property32(
                    xproto::PropMode::REPLACE,
                    x_window,
                    atoms._NET_WM_PID,
                    xproto::AtomEnum::CARDINAL,
                    &[pid],
                ),
            )?;

            let reply = get_reply(|| "X11 GetGeometry failed.", xcb.get_geometry(x_window))?;
            if reply.x == 0 && reply.y == 0 {
                bounds.origin.x.0 += 2;
                // 解决一个 bug：当在默认位置打开时，
                // 我们渲染的内容出现在窗口边界外
                // （在 X + Gnome + Ubuntu 22 上为 14px, 49px）
                let x = bounds.origin.x.0;
                let y = bounds.origin.y.0;
                check_reply(
                    || format!("X11 ConfigureWindow failed. x: {}, y: {}", x, y),
                    xcb.configure_window(x_window, &xproto::ConfigureWindowAux::new().x(x).y(y)),
                )?;
            }
            if let Some(titlebar) = params.titlebar
                && let Some(title) = titlebar.title
            {
                check_reply(
                    || "X11 ChangeProperty8 on WM_NAME failed.",
                    xcb.change_property8(
                        xproto::PropMode::REPLACE,
                        x_window,
                        xproto::AtomEnum::WM_NAME,
                        xproto::AtomEnum::STRING,
                        title.as_bytes(),
                    ),
                )?;
                check_reply(
                    || "X11 ChangeProperty8 on _NET_WM_NAME failed.",
                    xcb.change_property8(
                        xproto::PropMode::REPLACE,
                        x_window,
                        atoms._NET_WM_NAME,
                        atoms.UTF8_STRING,
                        title.as_bytes(),
                    ),
                )?;
            }

            if params.kind == WindowKind::PopUp {
                check_reply(
                    || "X11 ChangeProperty32 setting window type for pop-up failed.",
                    xcb.change_property32(
                        xproto::PropMode::REPLACE,
                        x_window,
                        atoms._NET_WM_WINDOW_TYPE,
                        xproto::AtomEnum::ATOM,
                        &[atoms._NET_WM_WINDOW_TYPE_NOTIFICATION],
                    ),
                )?;
            }

            if params.kind == WindowKind::Floating || params.kind == WindowKind::Dialog {
                if let Some(parent_window) = parent_window.as_ref().map(|w| w.x_window) {
                    // WM_TRANSIENT_FOR 提示，表示主应用程序窗口。对于浮动窗口，我们设置
                    // 一个父窗口（WM_TRANSIENT_FOR），以便窗口管理器知道将浮动窗口
                    // 放置在主窗口的什么位置
                    // https://specifications.freedesktop.org/wm-spec/1.4/ar01s05.html
                    check_reply(
                        || "X11 ChangeProperty32 setting WM_TRANSIENT_FOR for floating window failed.",
                        xcb.change_property32(
                            xproto::PropMode::REPLACE,
                            x_window,
                            atoms.WM_TRANSIENT_FOR,
                            xproto::AtomEnum::WINDOW,
                            &[parent_window],
                        ),
                    )?;
                }
            }

            let parent = if params.kind == WindowKind::Dialog
                && let Some(parent) = parent_window
            {
                parent.add_child(x_window);

                Some(parent)
            } else {
                None
            };

            if params.kind == WindowKind::Dialog {
                // _NET_WM_WINDOW_TYPE_DIALOG 表示这是一个对话框（浮动）窗口
                // https://specifications.freedesktop.org/wm-spec/1.4/ar01s05.html
                check_reply(
                    || "X11 ChangeProperty32 setting window type for dialog window failed.",
                    xcb.change_property32(
                        xproto::PropMode::REPLACE,
                        x_window,
                        atoms._NET_WM_WINDOW_TYPE,
                        xproto::AtomEnum::ATOM,
                        &[atoms._NET_WM_WINDOW_TYPE_DIALOG],
                    ),
                )?;

                // 我们为对话框窗口设置模态状态，以便窗口管理器
                // 可以适当地处理它（例如，在对话框打开时防止与父窗口交互）
                check_reply(
                    || "X11 ChangeProperty32 setting modal state for dialog window failed.",
                    xcb.change_property32(
                        xproto::PropMode::REPLACE,
                        x_window,
                        atoms._NET_WM_STATE,
                        xproto::AtomEnum::ATOM,
                        &[atoms._NET_WM_STATE_MODAL],
                    ),
                )?;
            }

            // 处理 Overlay 与 LayerShell 窗口：X11 上面板类窗口的对应形态是 DOCK ——
            // 不进任务栏、常驻最上层。注意 DOCK 必须保持被 WM 接管（不能
            // override_redirect），否则 WM 收不到这个窗口，strut 也就无人执行
            if matches!(params.kind, WindowKind::Overlay | WindowKind::LayerShell(_)) {
                // 设置窗口类型为 DOCK（覆盖层）
                check_reply(
                    || "X11 ChangeProperty32 setting window type for overlay failed.",
                    xcb.change_property32(
                        xproto::PropMode::REPLACE,
                        x_window,
                        atoms._NET_WM_WINDOW_TYPE,
                        xproto::AtomEnum::ATOM,
                        &[atoms._NET_WM_WINDOW_TYPE_DOCK],
                    ),
                )?;

                // 设置始终置顶状态
                check_reply(
                    || "X11 ChangeProperty32 setting above state for overlay failed.",
                    xcb.change_property32(
                        xproto::PropMode::REPLACE,
                        x_window,
                        atoms._NET_WM_STATE,
                        xproto::AtomEnum::ATOM,
                        &[atoms._NET_WM_STATE_ABOVE],
                    ),
                )?;
            }

            check_reply(
                || "X11 ChangeProperty32 setting protocols failed.",
                xcb.change_property32(
                    xproto::PropMode::REPLACE,
                    x_window,
                    atoms.WM_PROTOCOLS,
                    xproto::AtomEnum::ATOM,
                    &[atoms.WM_DELETE_WINDOW, atoms._NET_WM_SYNC_REQUEST],
                ),
            )?;

            get_reply(
                || "X11 sync protocol initialize failed.",
                sync::initialize(xcb, 3, 1),
            )?;
            let sync_request_counter = xcb.generate_id()?;
            check_reply(
                || "X11 sync CreateCounter failed.",
                sync::create_counter(xcb, sync_request_counter, sync::Int64 { lo: 0, hi: 0 }),
            )?;

            check_reply(
                || "X11 ChangeProperty32 setting sync request counter failed.",
                xcb.change_property32(
                    xproto::PropMode::REPLACE,
                    x_window,
                    atoms._NET_WM_SYNC_REQUEST_COUNTER,
                    xproto::AtomEnum::CARDINAL,
                    &[sync_request_counter],
                ),
            )?;

            let mut xi_event_mask = xinput::XIEventMask::MOTION
                | xinput::XIEventMask::BUTTON_PRESS
                | xinput::XIEventMask::BUTTON_RELEASE
                | xinput::XIEventMask::ENTER
                | xinput::XIEventMask::LEAVE;
            if supports_xinput_gestures {
                // x11rb 0.13 没有为手势事件定义 XIEventMask 常量，
                // 因此我们从事件操作码构建它们（每个 XInput 事件类型 N 映射到掩码位 N）
                xi_event_mask |=
                    xinput::XIEventMask::from(1u32 << xinput::GESTURE_PINCH_BEGIN_EVENT)
                        | xinput::XIEventMask::from(1u32 << xinput::GESTURE_PINCH_UPDATE_EVENT)
                        | xinput::XIEventMask::from(1u32 << xinput::GESTURE_PINCH_END_EVENT);
            }
            check_reply(
                || "X11 XiSelectEvents failed.",
                xcb.xinput_xi_select_events(
                    x_window,
                    &[xinput::EventMask {
                        deviceid: XINPUT_ALL_DEVICE_GROUPS,
                        mask: vec![xi_event_mask],
                    }],
                ),
            )?;

            check_reply(
                || "X11 XiSelectEvents for device changes failed.",
                xcb.xinput_xi_select_events(
                    x_window,
                    &[xinput::EventMask {
                        deviceid: XINPUT_ALL_DEVICES,
                        mask: vec![
                            xinput::XIEventMask::HIERARCHY | xinput::XIEventMask::DEVICE_CHANGED,
                        ],
                    }],
                ),
            )?;

            xcb_flush(xcb);

            let mut renderer = {
                let raw_window = RawWindow {
                    connection: as_raw_xcb_connection::AsRawXcbConnection::as_raw_xcb_connection(
                        xcb,
                    ) as *mut _,
                    screen_id: x_screen_index,
                    window_id: x_window,
                    visual_id: visual.id,
                };
                let config = WgpuSurfaceConfig {
                    // 注意：这必须在 GPU 初始化之后完成，否则
                    // 尺寸会立即失效
                    size: query_render_extent(xcb, x_window)?,
                    // 我们将其设置为透明，即使我们有客户端装饰，
                    // 因为这些似乎在 X11 上即使没有 `true` 也能工作
                    // 如果窗口外观改变，那么渲染器也会更新
                    transparent: false,
                    preferred_present_mode: None,
                };
                WgpuRenderer::new(gpu_context, &raw_window, config, compositor_gpu)?
            };

            renderer.set_subpixel_layout(is_bgr);

            // Set max window size hints based on the GPU's maximum texture dimension.
            // This prevents the window from being resized larger than what the GPU can render.
            let max_texture_size = renderer.max_texture_size();
            let mut size_hints = WmSizeHints::new();
            if let Some(size) = params.window_min_size {
                size_hints.min_size =
                    Some((f32::from(size.width) as i32, f32::from(size.height) as i32));
            }
            size_hints.max_size = Some((max_texture_size as i32, max_texture_size as i32));
            check_reply(
                || {
                    format!(
                        "X11 change of WM_SIZE_HINTS failed. max_size: {:?}",
                        max_texture_size
                    )
                },
                size_hints.set_normal_hints(xcb, x_window),
            )?;

            if let Some(image) = params.icon {
                // https://specifications.freedesktop.org/wm-spec/1.4/ar01s05.html#id-1.6.13
                let property_size = 2 + (image.width() * image.height()) as usize;
                let mut property_data: Vec<u32> = Vec::with_capacity(property_size);
                property_data.push(image.width());
                property_data.push(image.height());
                property_data.extend(image.pixels().map(|px| {
                    let [r, g, b, a]: [u8; 4] = px.0;
                    u32::from_le_bytes([b, g, r, a])
                }));

                check_reply(
                    || "X11 ChangeProperty32 for _NET_ICON_NAME failed.",
                    xcb.change_property32(
                        xproto::PropMode::REPLACE,
                        x_window,
                        atoms._NET_WM_ICON,
                        xproto::AtomEnum::CARDINAL,
                        &property_data,
                    ),
                )?;
            }

            let display = Rc::new(X11Display::new(xcb, scale_factor, x_screen_index)?);

            // layer-shell 选项里的独占区域在 X11 上就是 strut；边缘没显式给出时
            // 从锚点推断（只有单 bit 锚点才推断得出来）。首次 ConfigureNotify
            // 会应用它，这里不必额外发请求
            let (exclusive_zone, exclusive_edge) = match &params.kind {
                WindowKind::LayerShell(options) => (
                    options.exclusive_zone.unwrap_or(px(0.)),
                    options
                        .exclusive_edge
                        .and_then(single_edge)
                        .or_else(|| single_edge(options.anchor)),
                ),
                _ => (px(0.), None),
            };

            Ok(Self {
                parent,
                children: FxHashSet::default(),
                client,
                executor,
                display,
                x_root_window: visual_set.root,
                x_screen_index,
                visual_id: visual.id,
                bounds: bounds.to_pixels(scale_factor),
                scale_factor,
                renderer,
                atoms: *atoms,
                input_handler: None,
                active: false,
                hovered: false,
                force_render_after_recovery: false,
                fullscreen: false,
                maximized_vertical: false,
                maximized_horizontal: false,
                hidden: false,
                appearance,
                handle,
                background_appearance: WindowBackgroundAppearance::Opaque,
                destroyed: false,
                client_side_decorations_supported,
                decorations: WindowDecorations::Server,
                last_insets: [0, 0, 0, 0],
                edge_constraints: None,
                exclusive_zone,
                exclusive_edge,
                counter_id: sync_request_counter,
                last_sync_counter: None,
                accesskit_adapter: None,
            })
        });

        if setup_result.is_err() {
            check_reply(
                || "X11 DestroyWindow failed while cleaning it up after setup failure.",
                xcb.destroy_window(x_window),
            )?;
            xcb_flush(xcb);
        }

        setup_result
    }

    fn content_size(&self) -> Size<Pixels> {
        // 迁移到 wgpu 后，X11WindowState::content_size() 返回逻辑像素
        // （bounds.size 已在 set_bounds 中除以 scale_factor），因此这里不需要进一步除法
        // 这与 Wayland 实现一致
        self.bounds.size
    }
}

pub(crate) struct X11Window(pub X11WindowStatePtr);

impl Drop for X11Window {
    fn drop(&mut self) {
        let mut state = self.0.state.borrow_mut();

        if let Some(parent) = state.parent.as_ref() {
            parent.state.borrow_mut().children.remove(&self.0.x_window);
        }

        state.renderer.destroy();

        let destroy_x_window = maybe!({
            check_reply(
                || "X11 DestroyWindow failure.",
                self.0.xcb.destroy_window(self.0.x_window),
            )?;
            xcb_flush(&self.0.xcb);

            anyhow::Ok(())
        })
        .log_err();

        if destroy_x_window.is_some() {
            state.destroyed = true;

            let this_ptr = self.0.clone();
            let client_ptr = state.client.clone();
            state
                .executor
                .spawn(async move {
                    this_ptr.close();
                    client_ptr.drop_window(this_ptr.x_window);
                })
                .detach();
        }

        drop(state);
    }
}

enum WmHintPropertyState {
    // Remove = 0,
    // Add = 1,
    Toggle = 2,
}

impl X11Window {
    pub fn new(
        handle: AnyWindowHandle,
        client: X11ClientStatePtr,
        executor: ForegroundExecutor,
        gpu_context: rgpui_wgpu::GpuContext,
        compositor_gpu: Option<CompositorGpuHint>,
        params: WindowParams,
        xcb: &Rc<XCBConnection>,
        client_side_decorations_supported: bool,
        x_main_screen_index: usize,
        x_window: xproto::Window,
        atoms: &XcbAtoms,
        scale_factor: f32,
        appearance: WindowAppearance,
        parent_window: Option<X11WindowStatePtr>,
        supports_xinput_gestures: bool,
        is_bgr: bool,
    ) -> anyhow::Result<Self> {
        // X11 没有「创建时即穿透」的窗口属性，只能建完窗立刻清空输入区域，
        // 所以这个意图要在 params 被移交给 X11WindowState 之前先取出来
        let mouse_passthrough = params.mouse_passthrough;
        let ptr = X11WindowStatePtr {
            state: Rc::new(RefCell::new(X11WindowState::new(
                handle,
                client,
                executor,
                gpu_context,
                compositor_gpu,
                params,
                xcb,
                client_side_decorations_supported,
                x_main_screen_index,
                x_window,
                atoms,
                scale_factor,
                appearance,
                parent_window,
                supports_xinput_gestures,
                is_bgr,
            )?)),
            callbacks: Rc::new(RefCell::new(Callbacks::default())),
            xcb: xcb.clone(),
            x_window,
        };

        let state = ptr.state.borrow_mut();
        ptr.set_wm_properties(state)?;

        let window = Self(ptr);
        if mouse_passthrough {
            window.set_mouse_passthrough(true);
        }

        Ok(window)
    }

    fn set_wm_hints<C: Display + Send + Sync + 'static, F: FnOnce() -> C>(
        &self,
        failure_context: F,
        wm_hint_property_state: WmHintPropertyState,
        prop1: u32,
        prop2: u32,
    ) -> anyhow::Result<()> {
        let state = self.0.state.borrow();
        let message = ClientMessageEvent::new(
            32,
            self.0.x_window,
            state.atoms._NET_WM_STATE,
            [wm_hint_property_state as u32, prop1, prop2, 1, 0],
        );
        check_reply(
            failure_context,
            self.0.xcb.send_event(
                false,
                state.x_root_window,
                xproto::EventMask::SUBSTRUCTURE_REDIRECT | xproto::EventMask::SUBSTRUCTURE_NOTIFY,
                message,
            ),
        )?;
        xcb_flush(&self.0.xcb);
        Ok(())
    }

    fn get_root_position(
        &self,
        position: Point<Pixels>,
    ) -> anyhow::Result<TranslateCoordinatesReply> {
        let state = self.0.state.borrow();
        get_reply(
            || "X11 TranslateCoordinates failed.",
            self.0.xcb.translate_coordinates(
                self.0.x_window,
                state.x_root_window,
                (f32::from(position.x) * state.scale_factor) as i16,
                (f32::from(position.y) * state.scale_factor) as i16,
            ),
        )
    }

    fn send_moveresize(&self, flag: u32) -> anyhow::Result<()> {
        let state = self.0.state.borrow();

        check_reply(
            || "X11 UngrabPointer before move/resize of window failed.",
            self.0.xcb.ungrab_pointer(x11rb::CURRENT_TIME),
        )?;

        let pointer = get_reply(
            || "X11 QueryPointer before move/resize of window failed.",
            self.0.xcb.query_pointer(self.0.x_window),
        )?;
        let message = ClientMessageEvent::new(
            32,
            self.0.x_window,
            state.atoms._NET_WM_MOVERESIZE,
            [
                pointer.root_x as u32,
                pointer.root_y as u32,
                flag,
                0, // Left mouse button
                0,
            ],
        );
        check_reply(
            || "X11 SendEvent to move/resize window failed.",
            self.0.xcb.send_event(
                false,
                state.x_root_window,
                xproto::EventMask::SUBSTRUCTURE_REDIRECT | xproto::EventMask::SUBSTRUCTURE_NOTIFY,
                message,
            ),
        )?;

        xcb_flush(&self.0.xcb);
        Ok(())
    }

    /// 读取窗口的一个字符串型原子属性，属性不存在或不是合法 UTF-8 时返回 `None`
    fn get_string_property(&self, property: u32, kind: u32) -> Option<String> {
        let reply = get_reply(
            || "X11 GetProperty for window title failed.",
            self.0
                .xcb
                .get_property(false, self.0.x_window, property, kind, 0, u32::MAX),
        )
        .log_err()?;

        if reply.value_len == 0 {
            return None;
        }

        String::from_utf8(reply.value).ok()
    }

    /// Shape 扩展在精简的 X server 上可能不存在；缺了就只能放弃穿透，
    /// 因为 ShapeReq 会被服务端以 BadMatch 拒绝
    fn has_shape_extension(&self) -> bool {
        self.0
            .xcb
            .extension_information(shape::X11_EXTENSION_NAME)
            .ok()
            .flatten()
            .is_some()
    }
}

impl X11WindowStatePtr {
    pub fn should_close(&self) -> bool {
        let mut cb = self.callbacks.borrow_mut();
        if let Some(mut should_close) = cb.should_close.take() {
            let result = (should_close)();
            cb.should_close = Some(should_close);
            result
        } else {
            true
        }
    }

    pub fn property_notify(&self, event: xproto::PropertyNotifyEvent) -> anyhow::Result<()> {
        let state = self.state.borrow_mut();
        if event.atom == state.atoms._NET_WM_STATE {
            self.set_wm_properties(state)?;
        } else if event.atom == state.atoms._GTK_EDGE_CONSTRAINTS {
            self.set_edge_constraints(state)?;
        }
        Ok(())
    }

    fn set_edge_constraints(
        &self,
        mut state: std::cell::RefMut<X11WindowState>,
    ) -> anyhow::Result<()> {
        let reply = get_reply(
            || "X11 GetProperty for _GTK_EDGE_CONSTRAINTS failed.",
            self.xcb.get_property(
                false,
                self.x_window,
                state.atoms._GTK_EDGE_CONSTRAINTS,
                xproto::AtomEnum::CARDINAL,
                0,
                4,
            ),
        )?;

        if reply.value_len != 0 {
            if let Ok(bytes) = reply.value[0..4].try_into() {
                let atom = u32::from_ne_bytes(bytes);
                let edge_constraints = EdgeConstraints::from_atom(atom);
                state.edge_constraints.replace(edge_constraints);
            } else {
                log::error!("Failed to parse GTK_EDGE_CONSTRAINTS");
            }
        }

        Ok(())
    }

    fn set_wm_properties(
        &self,
        mut state: std::cell::RefMut<X11WindowState>,
    ) -> anyhow::Result<()> {
        let reply = get_reply(
            || "X11 GetProperty for _NET_WM_STATE failed.",
            self.xcb.get_property(
                false,
                self.x_window,
                state.atoms._NET_WM_STATE,
                xproto::AtomEnum::ATOM,
                0,
                u32::MAX,
            ),
        )?;

        let atoms = reply
            .value
            .chunks_exact(4)
            .map(|chunk| u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));

        state.active = false;
        state.fullscreen = false;
        state.maximized_vertical = false;
        state.maximized_horizontal = false;
        state.hidden = false;

        for atom in atoms {
            if atom == state.atoms._NET_WM_STATE_FOCUSED {
                state.active = true;
            } else if atom == state.atoms._NET_WM_STATE_FULLSCREEN {
                state.fullscreen = true;
            } else if atom == state.atoms._NET_WM_STATE_MAXIMIZED_VERT {
                state.maximized_vertical = true;
            } else if atom == state.atoms._NET_WM_STATE_MAXIMIZED_HORZ {
                state.maximized_horizontal = true;
            } else if atom == state.atoms._NET_WM_STATE_HIDDEN {
                state.hidden = true;
            }
        }

        Ok(())
    }

    pub fn add_child(&self, child: xproto::Window) {
        let mut state = self.state.borrow_mut();
        state.children.insert(child);
    }

    pub fn is_blocked(&self) -> bool {
        let state = self.state.borrow();
        !state.children.is_empty()
    }

    pub fn close(&self) {
        let state = self.state.borrow();
        let client = state.client.clone();
        #[allow(clippy::mutable_key_type)]
        let children = state.children.clone();
        drop(state);

        if let Some(client) = client.get_client() {
            for child in children {
                if let Some(child_window) = client.get_window(child) {
                    child_window.close();
                }
            }
        }

        let mut callbacks = self.callbacks.borrow_mut();
        if let Some(fun) = callbacks.close.take() {
            fun()
        }
    }

    pub fn refresh(&self, request_frame_options: RequestFrameOptions) {
        let callback = self.callbacks.borrow_mut().request_frame.take();
        if let Some(mut fun) = callback {
            fun(request_frame_options);
            self.callbacks.borrow_mut().request_frame = Some(fun);
        }
    }

    pub fn handle_input(&self, input: PlatformInput) {
        if self.is_blocked() {
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
        if let PlatformInput::KeyDown(event) = input {
            // 插入文本时仅允许 shift 修饰键
            if event.keystroke.modifiers.is_subset_of(&Modifiers::shift()) {
                let mut state = self.state.borrow_mut();
                if let Some(mut input_handler) = state.input_handler.take() {
                    if let Some(key_char) = &event.keystroke.key_char {
                        drop(state);
                        input_handler.replace_text_in_range(None, key_char);
                        state = self.state.borrow_mut();
                    }
                    state.input_handler = Some(input_handler);
                }
            }
        }
    }

    pub fn handle_ime_commit(&self, text: String) {
        if self.is_blocked() {
            return;
        }
        let mut state = self.state.borrow_mut();
        if let Some(mut input_handler) = state.input_handler.take() {
            drop(state);
            input_handler.replace_text_in_range(None, &text);
            let mut state = self.state.borrow_mut();
            state.input_handler = Some(input_handler);
        }
    }

    pub fn handle_ime_preedit(&self, text: String) {
        if self.is_blocked() {
            return;
        }
        let mut state = self.state.borrow_mut();
        if let Some(mut input_handler) = state.input_handler.take() {
            drop(state);
            input_handler.replace_and_mark_text_in_range(None, &text, None);
            let mut state = self.state.borrow_mut();
            state.input_handler = Some(input_handler);
        }
    }

    pub fn handle_ime_unmark(&self) {
        if self.is_blocked() {
            return;
        }
        let mut state = self.state.borrow_mut();
        if let Some(mut input_handler) = state.input_handler.take() {
            drop(state);
            input_handler.unmark_text();
            let mut state = self.state.borrow_mut();
            state.input_handler = Some(input_handler);
        }
    }

    pub fn handle_ime_delete(&self) {
        if self.is_blocked() {
            return;
        }
        let mut state = self.state.borrow_mut();
        if let Some(mut input_handler) = state.input_handler.take() {
            drop(state);
            if let Some(marked) = input_handler.marked_text_range() {
                input_handler.replace_text_in_range(Some(marked), "");
            }
            let mut state = self.state.borrow_mut();
            state.input_handler = Some(input_handler);
        }
    }

    pub fn get_ime_area(&self) -> Option<Bounds<ScaledPixels>> {
        let mut state = self.state.borrow_mut();
        let scale_factor = state.scale_factor;
        let mut bounds: Option<Bounds<Pixels>> = None;
        if let Some(mut input_handler) = state.input_handler.take() {
            drop(state);
            if let Some(selection) = input_handler.selected_text_range(true) {
                bounds = input_handler.bounds_for_range(selection.range);
            }
            let mut state = self.state.borrow_mut();
            state.input_handler = Some(input_handler);
        };
        bounds.map(|b| b.scale(scale_factor))
    }

    /// 窗口左上角在 root 坐标系中的物理像素位置。WM 可能 reparent 窗口（加装饰框架），
    /// 此时 configure 事件里的 origin 是相对父窗口的，而 strut 按 root 坐标解释
    fn origin_in_root(&self) -> Option<Point<i32>> {
        let root = self.state.borrow().x_root_window;
        get_reply(
            || "X11 TranslateCoordinates for strut failed.",
            self.xcb.translate_coordinates(self.x_window, root, 0, 0),
        )
        .log_err()
        .map(|reply| Point {
            x: reply.dst_x.into(),
            y: reply.dst_y.into(),
        })
    }

    /// 写入或清除 strut 属性。两个都写：`_NET_WM_STRUT_PARTIAL` 带跨度（多屏才准），
    /// `_NET_WM_STRUT` 留给只认 4 个值的老 WM 兜底
    fn set_strut(&self, strut: Option<[u32; 12]>) {
        let state = self.state.borrow();
        match strut {
            Some(strut) => {
                check_reply(
                    || "X11 ChangeProperty32 for _NET_WM_STRUT failed.",
                    self.xcb.change_property32(
                        xproto::PropMode::REPLACE,
                        self.x_window,
                        state.atoms._NET_WM_STRUT,
                        xproto::AtomEnum::CARDINAL,
                        &strut[..4],
                    ),
                )
                .log_err();
                check_reply(
                    || "X11 ChangeProperty32 for _NET_WM_STRUT_PARTIAL failed.",
                    self.xcb.change_property32(
                        xproto::PropMode::REPLACE,
                        self.x_window,
                        state.atoms._NET_WM_STRUT_PARTIAL,
                        xproto::AtomEnum::CARDINAL,
                        &strut,
                    ),
                )
                .log_err();
            }
            None => {
                for (name, atom) in [
                    ("_NET_WM_STRUT", state.atoms._NET_WM_STRUT),
                    ("_NET_WM_STRUT_PARTIAL", state.atoms._NET_WM_STRUT_PARTIAL),
                ] {
                    check_reply(
                        || format!("X11 DeleteProperty for {name} failed."),
                        self.xcb.delete_property(self.x_window, atom),
                    )
                    .log_err();
                }
            }
        }

        xcb_flush(&self.xcb);
    }

    /// 按当前 `exclusive_zone` / `exclusive_edge` 刷新 strut。窗口移动或改变大小后
    /// 跨度就变了，必须重算，否则屏幕上会留下与窗口不对齐的保留区域
    fn apply_strut(&self) {
        let (zone, edge, scale_factor, size) = {
            let state = self.state.borrow();
            // 没确定边缘的窗口本来就没有 strut，不必发请求
            let Some(edge) = state.exclusive_edge else {
                return;
            };
            (
                state.exclusive_zone,
                edge,
                state.scale_factor,
                state.bounds.size,
            )
        };

        let Some(origin) = self.origin_in_root() else {
            return;
        };
        let zone = (f32::from(zone) * scale_factor).round().max(0.0) as u32;
        let size = Size {
            width: (f32::from(size.width) * scale_factor).round().max(0.0) as u32,
            height: (f32::from(size.height) * scale_factor).round().max(0.0) as u32,
        };

        self.set_strut(strut_partial(edge, origin, size, zone));
    }

    pub fn set_bounds(&self, bounds: Bounds<i32>) -> anyhow::Result<()> {
        let (is_resize, content_size, scale_factor) = {
            let mut state = self.state.borrow_mut();
            let bounds = bounds.map(|f| px(f as f32 / state.scale_factor));

            let is_resize = bounds.size.width != state.bounds.size.width
                || bounds.size.height != state.bounds.size.height;

            // 如果是调整大小事件（仅宽度/高度改变），我们忽略 `bounds.origin`
            // 因为它包含错误的值
            if is_resize {
                state.bounds.size = bounds.size;
            } else {
                state.bounds = bounds;
            }

            let gpu_size = query_render_extent(&self.xcb, self.x_window)?;
            state.renderer.update_drawable_size(gpu_size);
            let result = (is_resize, state.content_size(), state.scale_factor);
            if let Some(value) = state.last_sync_counter.take() {
                check_reply(
                    || "X11 sync SetCounter failed.",
                    sync::set_counter(&self.xcb, state.counter_id, value),
                )?;
            }
            result
        };

        let mut callbacks = self.callbacks.borrow_mut();
        if let Some(ref mut fun) = callbacks.resize {
            fun(content_size, scale_factor)
        }

        if !is_resize && let Some(ref mut fun) = callbacks.moved {
            fun();
        }
        drop(callbacks);

        // 面板类的 strut 依赖窗口位置与尺寸，configure 一到就要跟着刷新
        self.apply_strut();

        Ok(())
    }

    pub fn set_active(&self, focus: bool) {
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
        let callback = self.callbacks.borrow_mut().hovered_status_change.take();
        if let Some(mut fun) = callback {
            fun(focus);
            self.callbacks.borrow_mut().hovered_status_change = Some(fun);
        }
    }

    pub fn set_appearance(&mut self, appearance: WindowAppearance) {
        let mut state = self.state.borrow_mut();
        state.appearance = appearance;
        let is_transparent = state.is_transparent();
        state.renderer.update_transparency(is_transparent);
        state.appearance = appearance;
        drop(state);
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
}

impl PlatformWindow for X11Window {
    fn bounds(&self) -> Bounds<Pixels> {
        self.0.state.borrow().bounds
    }

    fn is_maximized(&self) -> bool {
        let state = self.0.state.borrow();

        // 最大化窗口被最小化后仍会保留其最大化状态
        !state.hidden && state.maximized_vertical && state.maximized_horizontal
    }

    fn window_bounds(&self) -> WindowBounds {
        let state = self.0.state.borrow();
        if self.is_maximized() {
            WindowBounds::Maximized(state.bounds)
        } else {
            WindowBounds::Windowed(state.bounds)
        }
    }

    fn inner_window_bounds(&self) -> WindowBounds {
        let state = self.0.state.borrow();
        if self.is_maximized() {
            WindowBounds::Maximized(state.bounds)
        } else {
            let mut bounds = state.bounds;
            let [left, right, top, bottom] = state.last_insets;

            let [left, right, top, bottom] = [
                px((left as f32) / state.scale_factor),
                px((right as f32) / state.scale_factor),
                px((top as f32) / state.scale_factor),
                px((bottom as f32) / state.scale_factor),
            ];

            bounds.origin.x += left;
            bounds.origin.y += top;
            bounds.size.width -= left + right;
            bounds.size.height -= top + bottom;

            WindowBounds::Windowed(bounds)
        }
    }

    fn content_size(&self) -> Size<Pixels> {
        // After the wgpu migration, X11WindowState::content_size() returns logical pixels
        // (bounds.size is already divided by scale_factor in set_bounds), so no further
        // division is needed here. This matches the Wayland implementation.
        self.0.state.borrow().content_size()
    }

    fn resize(&mut self, size: Size<Pixels>) {
        let state = self.0.state.borrow();
        let size = size.to_device_pixels(state.scale_factor);
        let width = size.width.0 as u32;
        let height = size.height.0 as u32;

        check_reply(
            || {
                format!(
                    "X11 ConfigureWindow failed. width: {}, height: {}",
                    width, height
                )
            },
            self.0.xcb.configure_window(
                self.0.x_window,
                &xproto::ConfigureWindowAux::new()
                    .width(width)
                    .height(height),
            ),
        )
        .log_err();
        xcb_flush(&self.0.xcb);
    }

    fn set_position(&mut self, position: Point<Pixels>) {
        let state = self.0.state.borrow();
        let x = (position.x.as_f32() * state.scale_factor).round() as i32;
        let y = (position.y.as_f32() * state.scale_factor).round() as i32;

        check_reply(
            || format!("X11 ConfigureWindow failed. x: {}, y: {}", x, y),
            self.0.xcb.configure_window(
                self.0.x_window,
                &xproto::ConfigureWindowAux::new().x(x).y(y),
            ),
        )
        .log_err();
        xcb_flush(&self.0.xcb);
    }

    fn scale_factor(&self) -> f32 {
        self.0.state.borrow().scale_factor
    }

    fn appearance(&self) -> WindowAppearance {
        self.0.state.borrow().appearance
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.0.state.borrow().display.clone())
    }

    fn mouse_position(&self) -> Point<Pixels> {
        get_reply(
            || "X11 QueryPointer failed.",
            self.0.xcb.query_pointer(self.0.x_window),
        )
        .log_err()
        .map_or(Point::new(Pixels::ZERO, Pixels::ZERO), |reply| {
            let scale_factor = self.0.state.borrow().scale_factor;
            Point::new(
                px(reply.win_x as f32 / scale_factor),
                px(reply.win_y as f32 / scale_factor),
            )
        })
    }

    fn modifiers(&self) -> Modifiers {
        self.0
            .state
            .borrow()
            .client
            .0
            .upgrade()
            .map(|ref_cell| ref_cell.borrow().modifiers)
            .unwrap_or_default()
    }

    fn capslock(&self) -> rgpui::Capslock {
        self.0
            .state
            .borrow()
            .client
            .0
            .upgrade()
            .map(|ref_cell| ref_cell.borrow().capslock)
            .unwrap_or_default()
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        self.0.state.borrow_mut().input_handler = Some(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.0.state.borrow_mut().input_handler.take()
    }

    fn prompt(
        &self,
        _level: PromptLevel,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[PromptButton],
    ) -> Option<futures::channel::oneshot::Receiver<usize>> {
        None
    }

    fn activate(&self) {
        // hide() 撤下的窗口 WM_STATE 会变成 Withdrawn，合成器不再管理它，
        // 单发 _NET_ACTIVE_WINDOW 会被忽略；先重新 map 才能恢复。窗口已可见时
        // MapWindow 是空操作。
        check_reply(
            || "X11 MapWindow on activate failed.",
            self.0.xcb.map_window(self.0.x_window),
        )
        .log_err();

        let data = [1, xproto::Time::CURRENT_TIME.into(), 0, 0, 0];
        let message = xproto::ClientMessageEvent::new(
            32,
            self.0.x_window,
            self.0.state.borrow().atoms._NET_ACTIVE_WINDOW,
            data,
        );
        self.0
            .xcb
            .send_event(
                false,
                self.0.state.borrow().x_root_window,
                xproto::EventMask::SUBSTRUCTURE_REDIRECT | xproto::EventMask::SUBSTRUCTURE_NOTIFY,
                message,
            )
            .log_err();
        self.0
            .xcb
            .set_input_focus(
                xproto::InputFocus::POINTER_ROOT,
                self.0.x_window,
                xproto::Time::CURRENT_TIME,
            )
            .log_err();
        xcb_flush(&self.0.xcb);
    }

    fn is_active(&self) -> bool {
        self.0.state.borrow().active
    }

    fn is_hovered(&self) -> bool {
        self.0.state.borrow().hovered
    }

    fn set_title(&mut self, title: &str) {
        check_reply(
            || "X11 ChangeProperty8 on WM_NAME failed.",
            self.0.xcb.change_property8(
                xproto::PropMode::REPLACE,
                self.0.x_window,
                xproto::AtomEnum::WM_NAME,
                xproto::AtomEnum::STRING,
                title.as_bytes(),
            ),
        )
        .log_err();

        check_reply(
            || "X11 ChangeProperty8 on _NET_WM_NAME failed.",
            self.0.xcb.change_property8(
                xproto::PropMode::REPLACE,
                self.0.x_window,
                self.0.state.borrow().atoms._NET_WM_NAME,
                self.0.state.borrow().atoms.UTF8_STRING,
                title.as_bytes(),
            ),
        )
        .log_err();
        xcb_flush(&self.0.xcb);
    }

    fn get_title(&self) -> String {
        // _NET_WM_NAME 是 EWMH 规定的 UTF-8 标题，优先读它；
        // 没有再回退到 ICCCM 的 WM_NAME（STRING）。
        let (net_wm_name, utf8_string) = {
            let state = self.0.state.borrow();
            (state.atoms._NET_WM_NAME, state.atoms.UTF8_STRING)
        };

        self.get_string_property(net_wm_name, utf8_string)
            .or_else(|| {
                self.get_string_property(
                    u32::from(xproto::AtomEnum::WM_NAME),
                    u32::from(xproto::AtomEnum::STRING),
                )
            })
            .unwrap_or_default()
    }

    fn request_attention(&self) {
        // ICCCM 的 WM_HINTS urgency 位是 X11 上「请求用户注意」的正规入口，
        // 行为与 `xdotool set_window --urgency 1` 一致（本机实测两者都能让
        // xprop 出现 "The urgency hint bit is set"）。
        // EWMH 的 _NET_WM_STATE_DEMANDS_ATTENTION 是「WM 设置、客户端只读」的状态，
        // 客户端自发 _NET_WM_STATE 客户端消息去要这个原子时 mutter 会直接忽略
        // （本机实测：_NET_WM_STATE 始终不出现该原子）。
        let mut hints = x11rb::properties::WmHints::new();
        hints.input = Some(true);
        hints.initial_state = Some(x11rb::properties::WmHintsState::Normal);
        hints.urgent = true;

        check_reply(
            || "X11 ChangeProperty on WM_HINTS for urgency failed.",
            hints.set(&self.0.xcb, self.0.x_window),
        )
        .log_err();
        xcb_flush(&self.0.xcb);
    }

    fn set_app_id(&mut self, app_id: &str) {
        let mut data = Vec::with_capacity(app_id.len() * 2 + 1);
        data.extend(app_id.bytes()); // instance https://unix.stackexchange.com/a/494170
        data.push(b'\0');
        data.extend(app_id.bytes()); // class

        check_reply(
            || "X11 ChangeProperty8 for WM_CLASS failed.",
            self.0.xcb.change_property8(
                xproto::PropMode::REPLACE,
                self.0.x_window,
                xproto::AtomEnum::WM_CLASS,
                xproto::AtomEnum::STRING,
                &data,
            ),
        )
        .log_err();
    }

    fn map_window(&mut self) -> anyhow::Result<()> {
        check_reply(
            || "X11 MapWindow failed.",
            self.0.xcb.map_window(self.0.x_window),
        )?;
        Ok(())
    }

    fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance) {
        let mut state = self.0.state.borrow_mut();
        state.background_appearance = background_appearance;
        let transparent = state.is_transparent();
        state.renderer.update_transparency(transparent);
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.0.state.borrow().background_appearance
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        self.0
            .state
            .borrow()
            .client
            .0
            .upgrade()
            .map(|ref_cell| {
                let state = ref_cell.borrow();
                state
                    .gpu_context
                    .borrow()
                    .as_ref()
                    .is_some_and(|ctx| ctx.supports_dual_source_blending())
            })
            .unwrap_or_default()
    }

    fn minimize(&self) {
        let state = self.0.state.borrow();
        const WINDOW_ICONIC_STATE: u32 = 3;
        let message = ClientMessageEvent::new(
            32,
            self.0.x_window,
            state.atoms.WM_CHANGE_STATE,
            [WINDOW_ICONIC_STATE, 0, 0, 0, 0],
        );
        check_reply(
            || "X11 SendEvent to minimize window failed.",
            self.0.xcb.send_event(
                false,
                state.x_root_window,
                xproto::EventMask::SUBSTRUCTURE_REDIRECT | xproto::EventMask::SUBSTRUCTURE_NOTIFY,
                message,
            ),
        )
        .log_err();
    }

    fn hide(&self) {
        check_reply(
            || "X11 UnmapWindow failed.",
            self.0.xcb.unmap_window(self.0.x_window),
        )
        .log_err();
    }

    /// 鼠标穿透：把 Shape 的输入区域设为空，窗口照常显示但事件落到下层窗口
    fn set_mouse_passthrough(&self, passthrough: bool) {
        if passthrough {
            self.set_input_region(Some(&[]));
        } else {
            self.set_input_region(None);
        }
    }

    /// `None` 恢复「整个窗口接收输入」（把输入区域重设为窗口外形），
    /// `Some(rects)` 只让这些矩形接收输入、其余区域穿透
    fn set_input_region(&self, region: Option<&[Bounds<Pixels>]>) {
        if !self.has_shape_extension() {
            log::debug!("X server 未提供 Shape 扩展，鼠标穿透与输入区域不可用");
            return;
        }

        match region {
            // 用 BOUNDING（窗口外形）重设，省掉一次 GetGeometry 往返去问窗口尺寸
            None => {
                check_reply(
                    || "X11 ShapeCombine on input region failed.",
                    self.0.xcb.shape_combine(
                        shape::SO::SET,
                        shape::SK::INPUT,
                        shape::SK::BOUNDING,
                        self.0.x_window,
                        0,
                        0,
                        self.0.x_window,
                    ),
                )
                .log_err();
            }
            Some(rects) => {
                let rects = rects
                    .iter()
                    .map(|item| {
                        let origin = item.origin;
                        let size = item.size;
                        xproto::Rectangle {
                            x: f32::from(origin.x)
                                .round()
                                .clamp(i16::MIN as f32, i16::MAX as f32)
                                as i16,
                            y: f32::from(origin.y)
                                .round()
                                .clamp(i16::MIN as f32, i16::MAX as f32)
                                as i16,
                            width: f32::from(size.width).round().clamp(0.0, u16::MAX as f32) as u16,
                            height: f32::from(size.height).round().clamp(0.0, u16::MAX as f32)
                                as u16,
                        }
                    })
                    .collect::<Vec<_>>();

                check_reply(
                    || "X11 ShapeRectangles on input region failed.",
                    self.0.xcb.shape_rectangles(
                        shape::SO::SET,
                        shape::SK::INPUT,
                        xproto::ClipOrdering::UNSORTED,
                        self.0.x_window,
                        0,
                        0,
                        &rects,
                    ),
                )
                .log_err();
            }
        }

        xcb_flush(&self.0.xcb);
    }

    /// X11 上没有 layer-shell 协议，独占区域用 EWMH strut 表达（见 `apply_strut`）
    fn set_exclusive_zone(&self, zone: Pixels) {
        self.0.state.borrow_mut().exclusive_zone = zone;
        self.0.apply_strut();
    }

    /// 边缘必须先于宽度确定：strut 得知道保留哪一条边。给的是组合锚点时不改动已有边缘
    fn set_exclusive_edge(&self, edge: Anchor) {
        let Some(single) = single_edge(edge) else {
            log::warn!("独占区域只能作用于单一边缘，{edge:?} 无法确定边，已忽略");
            return;
        };
        self.0.state.borrow_mut().exclusive_edge = Some(single);
        self.0.apply_strut();
    }

    fn zoom(&self) {
        let state = self.0.state.borrow();
        self.set_wm_hints(
            || "X11 SendEvent to maximize a window failed.",
            WmHintPropertyState::Toggle,
            state.atoms._NET_WM_STATE_MAXIMIZED_VERT,
            state.atoms._NET_WM_STATE_MAXIMIZED_HORZ,
        )
        .log_err();
    }

    fn toggle_fullscreen(&self) {
        let state = self.0.state.borrow();
        self.set_wm_hints(
            || "X11 SendEvent to fullscreen a window failed.",
            WmHintPropertyState::Toggle,
            state.atoms._NET_WM_STATE_FULLSCREEN,
            xproto::AtomEnum::NONE.into(),
        )
        .log_err();
    }

    fn is_fullscreen(&self) -> bool {
        self.0.state.borrow().fullscreen
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
        self.0.callbacks.borrow_mut().hovered_status_change = Some(callback);
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
    }

    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.callbacks.borrow_mut().appearance_changed = Some(callback);
    }

    fn on_button_layout_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.callbacks.borrow_mut().button_layout_changed = Some(callback);
    }

    fn draw(&self, scene: &Scene) {
        let mut inner = self.0.state.borrow_mut();

        if inner.renderer.device_lost() {
            let raw_window = RawWindow {
                connection: as_raw_xcb_connection::AsRawXcbConnection::as_raw_xcb_connection(
                    &*self.0.xcb,
                ) as *mut _,
                screen_id: inner.x_screen_index,
                window_id: self.0.x_window,
                visual_id: inner.visual_id,
            };
            match inner.renderer.recover(&raw_window) {
                Ok(()) => {}
                Err(err) => {
                    log::warn!("GPU recovery failed, will retry on next frame: {err}");
                }
            }

            inner.force_render_after_recovery = true;
            return;
        }

        inner.renderer.draw(scene);

        if inner.renderer.needs_redraw() {
            inner.force_render_after_recovery = true;
        }
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        let inner = self.0.state.borrow();
        inner.renderer.sprite_atlas().clone()
    }

    fn show_window_menu(&self, position: Point<Pixels>) {
        let state = self.0.state.borrow();

        check_reply(
            || "X11 UngrabPointer failed.",
            self.0.xcb.ungrab_pointer(x11rb::CURRENT_TIME),
        )
        .log_err();

        let Some(coords) = self.get_root_position(position).log_err() else {
            return;
        };
        let message = ClientMessageEvent::new(
            32,
            self.0.x_window,
            state.atoms._GTK_SHOW_WINDOW_MENU,
            [
                XINPUT_ALL_DEVICE_GROUPS as u32,
                coords.dst_x as u32,
                coords.dst_y as u32,
                0,
                0,
            ],
        );
        check_reply(
            || "X11 SendEvent to show window menu failed.",
            self.0.xcb.send_event(
                false,
                state.x_root_window,
                xproto::EventMask::SUBSTRUCTURE_REDIRECT | xproto::EventMask::SUBSTRUCTURE_NOTIFY,
                message,
            ),
        )
        .log_err();
    }

    fn start_window_move(&self) {
        const MOVERESIZE_MOVE: u32 = 8;
        self.send_moveresize(MOVERESIZE_MOVE).log_err();
    }

    fn start_window_resize(&self, edge: ResizeEdge) {
        self.send_moveresize(resize_edge_to_moveresize(edge))
            .log_err();
    }

    fn window_decorations(&self) -> rgpui::Decorations {
        let state = self.0.state.borrow();

        // 客户端窗口装饰需要合成器支持
        if !state.client_side_decorations_supported {
            return Decorations::Server;
        }

        match state.decorations {
            WindowDecorations::Server => Decorations::Server,
            WindowDecorations::Client => {
                let tiling = if state.fullscreen {
                    Tiling::tiled()
                } else if let Some(edge_constraints) = &state.edge_constraints {
                    edge_constraints.to_tiling()
                } else {
                    // https://source.chromium.org/chromium/chromium/src/+/main:ui/ozone/platform/x11/x11_window.cc;l=2519;drc=1f14cc876cc5bf899d13284a12c451498219bb2d
                    Tiling {
                        top: state.maximized_vertical,
                        bottom: state.maximized_vertical,
                        left: state.maximized_horizontal,
                        right: state.maximized_horizontal,
                    }
                };
                Decorations::Client { tiling }
            }
        }
    }

    fn set_client_inset(&self, inset: Pixels) {
        let mut state = self.0.state.borrow_mut();

        let dp = (f32::from(inset) * state.scale_factor) as u32;

        let insets = if state.fullscreen {
            [0, 0, 0, 0]
        } else if let Some(edge_constraints) = &state.edge_constraints {
            let left = if edge_constraints.left_tiled { 0 } else { dp };
            let top = if edge_constraints.top_tiled { 0 } else { dp };
            let right = if edge_constraints.right_tiled { 0 } else { dp };
            let bottom = if edge_constraints.bottom_tiled { 0 } else { dp };

            [left, right, top, bottom]
        } else {
            let (left, right) = if state.maximized_horizontal {
                (0, 0)
            } else {
                (dp, dp)
            };
            let (top, bottom) = if state.maximized_vertical {
                (0, 0)
            } else {
                (dp, dp)
            };
            [left, right, top, bottom]
        };

        if state.last_insets != insets {
            state.last_insets = insets;

            check_reply(
                || "X11 ChangeProperty for _GTK_FRAME_EXTENTS failed.",
                self.0.xcb.change_property(
                    xproto::PropMode::REPLACE,
                    self.0.x_window,
                    state.atoms._GTK_FRAME_EXTENTS,
                    xproto::AtomEnum::CARDINAL,
                    size_of::<u32>() as u8 * 8,
                    4,
                    bytemuck::cast_slice::<u32, u8>(&insets),
                ),
            )
            .log_err();
        }
    }

    fn request_decorations(&self, mut decorations: rgpui::WindowDecorations) {
        let mut state = self.0.state.borrow_mut();

        if matches!(decorations, rgpui::WindowDecorations::Client)
            && !state.client_side_decorations_supported
        {
            log::info!(
                "x11: no compositor present, falling back to server-side window decorations"
            );
            decorations = rgpui::WindowDecorations::Server;
        }

        // https://github.com/rust-windowing/winit/blob/master/src/platform_impl/linux/x11/util/hint.rs#L53-L87
        let hints_data: [u32; 5] = match decorations {
            WindowDecorations::Server => [1 << 1, 0, 1, 0, 0],
            WindowDecorations::Client => [1 << 1, 0, 0, 0, 0],
        };

        let success = check_reply(
            || "X11 ChangeProperty for _MOTIF_WM_HINTS failed.",
            self.0.xcb.change_property(
                xproto::PropMode::REPLACE,
                self.0.x_window,
                state.atoms._MOTIF_WM_HINTS,
                state.atoms._MOTIF_WM_HINTS,
                size_of::<u32>() as u8 * 8,
                5,
                bytemuck::cast_slice::<u32, u8>(&hints_data),
            ),
        )
        .log_err();

        let Some(()) = success else {
            return;
        };

        match decorations {
            WindowDecorations::Server => {
                state.decorations = WindowDecorations::Server;
                let is_transparent = state.is_transparent();
                state.renderer.update_transparency(is_transparent);
            }
            WindowDecorations::Client => {
                state.decorations = WindowDecorations::Client;
                let is_transparent = state.is_transparent();
                state.renderer.update_transparency(is_transparent);
            }
        }

        drop(state);
        let mut callbacks = self.0.callbacks.borrow_mut();
        if let Some(appearance_changed) = callbacks.appearance_changed.as_mut() {
            appearance_changed();
        }
    }

    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        let state = self.0.state.borrow();
        let client = state.client.clone();
        drop(state);
        client.update_ime_position(bounds);
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.0.state.borrow().renderer.gpu_specs().into()
    }

    fn play_system_bell(&self) {
        // 音量 0% 表示不从系统音量增加或减少
        let _ = self.0.xcb.bell(0);
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

        self.0.state.borrow_mut().accesskit_adapter = Some(adapter);
    }

    fn a11y_tree_update(&self, tree_update: accesskit::TreeUpdate) {
        let mut state = self.0.state.borrow_mut();
        if let Some(adapter) = state.accesskit_adapter.as_mut() {
            adapter.update_if_active(|| tree_update);
        }
    }

    fn a11y_update_window_bounds(&self) {
        let mut state = self.0.state.borrow_mut();
        let scale = state.scale_factor;
        let bounds = state.bounds;
        let [left, right, top, bottom] = state.last_insets;

        let x = f32::from(bounds.origin.x);
        let y = f32::from(bounds.origin.y);
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);

        let outer = accesskit::Rect {
            x0: (x * scale) as f64,
            y0: (y * scale) as f64,
            x1: ((x + width) * scale) as f64,
            y1: ((y + height) * scale) as f64,
        };

        let inner = accesskit::Rect {
            x0: (x * scale) as f64 + left as f64,
            y0: (y * scale) as f64 + top as f64,
            x1: ((x + width) * scale) as f64 - right as f64,
            y1: ((y + height) * scale) as f64 - bottom as f64,
        };

        if let Some(adapter) = state.accesskit_adapter.as_mut() {
            adapter.set_root_window_bounds(outer, inner);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(width: u32, height: u32) -> Size<u32> {
        Size { width, height }
    }

    #[test]
    fn bottom_strut_reserves_zone_and_spans_window_width() {
        let strut =
            strut_partial(Anchor::BOTTOM, Point { x: 0, y: 800 }, size(1024, 100), 100).unwrap();

        // left、right、top 不该被碰到；bottom 从屏幕下边缘起算
        assert_eq!(&strut[..3], &[0, 0, 0]);
        assert_eq!(strut[3], 100);
        // 跨度是闭区间的像素序号
        assert_eq!(&strut[4..10], &[0; 6]);
        assert_eq!(&strut[10..], &[0, 1023]);
    }

    #[test]
    fn left_strut_spans_window_height() {
        let strut = strut_partial(Anchor::LEFT, Point { x: 0, y: 100 }, size(60, 400), 60).unwrap();

        assert_eq!(strut[0], 60);
        assert_eq!(&strut[4..6], &[100, 499]);
    }

    #[test]
    fn negative_or_multi_edge_yields_no_strut() {
        // zone 非正：调用方据此删除属性
        assert!(strut_partial(Anchor::TOP, Point { x: 0, y: 0 }, size(10, 10), 0).is_none());
        // 左右都锚定时判断不出保留哪条边
        assert!(
            strut_partial(
                Anchor::LEFT | Anchor::RIGHT,
                Point { x: 0, y: 0 },
                size(10, 10),
                10
            )
            .is_none()
        );
    }

    #[test]
    fn strut_edge_must_be_single_bit() {
        assert_eq!(single_edge(Anchor::TOP), Some(Anchor::TOP));
        assert_eq!(single_edge(Anchor::TOP | Anchor::LEFT), None);
        assert_eq!(single_edge(Anchor::empty()), None);
    }
}
