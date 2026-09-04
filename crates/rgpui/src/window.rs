//! 窗口管理：窗口创建、事件分发、渲染管线与输入处理。

#[cfg(any(feature = "inspector", debug_assertions))]
use crate::Inspector;
use crate::{
    Action, AnyDrag, AnyElement, AnyImageCache, AnyTooltip, AnyView, App, AppContext, Arena, Asset,
    AsyncWindowContext, AvailableSpace, Background, BorderStyle, Bounds, BoxShadow, Capslock,
    Context, Corners, CursorHideMode, CursorStyle, Decorations, DevicePixels,
    DispatchActionListener, DispatchNodeId, DispatchTree, DisplayId, Edges, Effect, Entity,
    EntityId, EventEmitter, FileDropEvent, FontId, Global, GlobalElementId, GlyphId, GpuSpecs,
    Hsla, InputHandler, IsZero, KeyBinding, KeyContext, KeyDownEvent, KeyEvent, Keystroke,
    KeystrokeEvent, LayoutId, LineLayoutIndex, Modifiers, ModifiersChangedEvent, MonochromeSprite,
    MouseButton, MouseEvent, MouseMoveEvent, MouseUpEvent, Path, Pixels, PlatformAtlas,
    PlatformDisplay, PlatformInput, PlatformInputHandler, PlatformWindow, Point, PolychromeSprite,
    Priority, PromptButton, PromptLevel, Quad, Render, RenderGlyphParams, RenderImage,
    RenderImageParams, RenderSvgParams, Replay, ResizeEdge, SMOOTH_SVG_SCALE_FACTOR,
    SUBPIXEL_VARIANTS_X, SUBPIXEL_VARIANTS_Y, ScaledPixels, Scene, Shadow, SharedString, Size,
    StrikethroughStyle, Style, SubpixelSprite, SubscriberSet, Subscription, SystemWindowTab,
    SystemWindowTabController, TabStopMap, TaffyLayoutEngine, Task, TextRenderingMode, TextStyle,
    TextStyleRefinement, ThermalState, TransformationMatrix, Underline, UnderlineStyle,
    WindowAppearance, WindowBackgroundAppearance, WindowBounds, WindowControls, WindowDecorations,
    WindowOptions, WindowParams, WindowTextSystem, point, prelude::*, profiler, px, rems,
    root::Root, size, transparent_black,
};

use crate::collections::{FxHashMap, FxHashSet};
use crate::refineable::Refineable;
use crate::rgpui_util::post_inc;
use crate::rgpui_util::{ResultExt, measure};
use crate::scheduler::Instant;
use anyhow::{Context as _, Result, anyhow};
#[cfg(target_os = "macos")]
use core_video::pixel_buffer::CVPixelBuffer;
use derive_more::{Deref, DerefMut};
use futures::FutureExt;
use futures::channel::oneshot;
#[cfg(feature = "input-latency-histogram")]
use hdrhistogram::Histogram;
use itertools::FoldWhile::{Continue, Done};
use itertools::Itertools;
use parking_lot::RwLock;
use raw_window_handle::{HandleError, HasDisplayHandle, HasWindowHandle};
use slotmap::SlotMap;
use smallvec::SmallVec;
use std::{
    any::{Any, TypeId},
    borrow::Cow,
    cell::{Cell, RefCell},
    cmp,
    fmt::{Debug, Display},
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem,
    ops::{DerefMut, Range},
    rc::Rc,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
    time::Duration,
};
use uuid::Uuid;

pub(crate) mod a11y;
mod prompts;

pub use a11y::A11ySubtreeBuilder;

use self::a11y::A11y;
#[cfg(not(target_family = "wasm"))]
use self::a11y::ROOT_NODE_ID;
use crate::util::{
    atomic_incr_if_not_zero, ceil_to_device_pixel, floor_to_device_pixel, round_half_toward_zero,
    round_half_toward_zero_f64, round_stroke_to_device_pixel, round_to_device_pixel,
};
pub use prompts::*;

/// 未指定窗口大小时使用的默认尺寸。
pub const DEFAULT_WINDOW_SIZE: Size<Pixels> = size(px(1536.), px(1095.));

/// 6:5 宽高比的最小窗口尺寸，用于功能性附加窗口（如设置和规则库窗口）。
pub const DEFAULT_ADDITIONAL_WINDOW_SIZE: Size<Pixels> = Size {
    width: Pixels(900.),
    height: Pixels(750.),
};

/// 表示事件分发时的两个不同阶段。
#[derive(Default, Copy, Clone, Debug, Eq, PartialEq)]
pub enum DispatchPhase {
    /// 在捕获阶段之后是冒泡阶段，此时鼠标事件监听器从前向后调用，
    /// 键盘事件监听器从焦点元素向元素树根调用。注册事件监听器时通常使用此阶段。
    #[default]
    Bubble,
    /// 在初始捕获阶段，鼠标事件监听器从后向前调用，键盘监听器从树根向下朝焦点元素调用。
    /// 此阶段用于特殊目的，如清除点击事件的"按下"状态。如果在此阶段停止事件传播，
    /// 你需要清楚自己在做什么。直接区域外的处理程序可能依赖于在此阶段检测非本地事件。
    Capture,
}

impl DispatchPhase {
    /// 返回 `true` 表示这是"冒泡"阶段。
    #[inline]
    pub fn bubble(self) -> bool {
        self == DispatchPhase::Bubble
    }

    /// 返回 `true` 表示这是"捕获"阶段。
    #[inline]
    pub fn capture(self) -> bool {
        self == DispatchPhase::Capture
    }
}

struct WindowInvalidatorInner {
    pub dirty: bool,
    pub draw_phase: DrawPhase,
    pub dirty_views: FxHashSet<EntityId>,
    pub update_count: usize,
    pub frame_dirty: FrameDirtyAccumulator,
}

/// 每帧失效记录簿记，在绘制时清空并发送到帧分析器。
/// 跟踪当前帧首次变脏的时间以及合并了多少次失效。
/// 仅在启用 `profiler::frame_trace_enabled()` 时填充。
#[derive(Default)]
struct FrameDirtyAccumulator {
    dirty_at: Option<Instant>,
    invalidations: u64,
}

#[derive(Clone)]
pub(crate) struct WindowInvalidator {
    inner: Rc<RefCell<WindowInvalidatorInner>>,
}

impl WindowInvalidator {
    pub fn new() -> Self {
        WindowInvalidator {
            inner: Rc::new(RefCell::new(WindowInvalidatorInner {
                dirty: true,
                draw_phase: DrawPhase::None,
                dirty_views: FxHashSet::default(),
                update_count: 0,
                frame_dirty: FrameDirtyAccumulator::default(),
            })),
        }
    }

    pub fn invalidate_view(&self, entity: EntityId, cx: &mut App) -> bool {
        let mut inner = self.inner.borrow_mut();
        inner.update_count += 1;
        inner.dirty_views.insert(entity);
        if inner.draw_phase == DrawPhase::None {
            Self::record_frame_dirty(&mut inner);
            inner.dirty = true;
            cx.push_effect(Effect::Notify { emitter: entity });
            true
        } else {
            false
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.inner.borrow().dirty
    }

    pub fn set_dirty(&self, dirty: bool) {
        let mut inner = self.inner.borrow_mut();
        inner.dirty = dirty;
        if dirty {
            inner.update_count += 1;
            Self::record_frame_dirty(&mut inner);
        }
    }

    pub fn set_phase(&self, phase: DrawPhase) {
        self.inner.borrow_mut().draw_phase = phase
    }

    pub fn update_count(&self) -> usize {
        self.inner.borrow().update_count
    }

    fn record_frame_dirty(inner: &mut WindowInvalidatorInner) {
        if profiler::frame_trace_enabled() {
            inner.frame_dirty.dirty_at.get_or_insert_with(Instant::now);
            inner.frame_dirty.invalidations += 1;
        }
    }

    fn take_frame_dirty(&self) -> FrameDirtyAccumulator {
        mem::take(&mut self.inner.borrow_mut().frame_dirty)
    }

    pub fn take_views(&self) -> FxHashSet<EntityId> {
        mem::take(&mut self.inner.borrow_mut().dirty_views)
    }

    pub fn replace_views(&self, views: FxHashSet<EntityId>) {
        self.inner.borrow_mut().dirty_views = views;
    }

    pub fn not_drawing(&self) -> bool {
        self.inner.borrow().draw_phase == DrawPhase::None
    }

    #[track_caller]
    pub fn debug_assert_paint(&self) {
        debug_assert!(
            matches!(self.inner.borrow().draw_phase, DrawPhase::Paint),
            "this method can only be called during paint"
        );
    }

    #[track_caller]
    pub fn debug_assert_prepaint(&self) {
        debug_assert!(
            matches!(self.inner.borrow().draw_phase, DrawPhase::Prepaint),
            "this method can only be called during request_layout, or prepaint"
        );
    }

    #[track_caller]
    pub fn debug_assert_paint_or_prepaint(&self) {
        debug_assert!(
            matches!(
                self.inner.borrow().draw_phase,
                DrawPhase::Paint | DrawPhase::Prepaint
            ),
            "this method can only be called during request_layout, prepaint, or paint"
        );
    }
}

type AnyObserver = Box<dyn FnMut(&mut Window, &mut App) -> bool + 'static>;

pub(crate) type AnyWindowFocusListener =
    Box<dyn FnMut(&WindowFocusEvent, &mut Window, &mut App) -> bool + 'static>;

pub(crate) struct WindowFocusEvent {
    pub(crate) previous_focus_path: SmallVec<[FocusId; 8]>,
    pub(crate) current_focus_path: SmallVec<[FocusId; 8]>,
}

impl WindowFocusEvent {
    pub fn is_focus_in(&self, focus_id: FocusId) -> bool {
        !self.previous_focus_path.contains(&focus_id) && self.current_focus_path.contains(&focus_id)
    }

    pub fn is_focus_out(&self, focus_id: FocusId) -> bool {
        self.previous_focus_path.contains(&focus_id) && !self.current_focus_path.contains(&focus_id)
    }
}

/// 订阅 `Context::on_focus_out` 事件时提供。
pub struct FocusOutEvent {
    /// 表示失去焦点的弱焦点句柄。
    pub blurred: WeakFocusHandle,
}

slotmap::new_key_type! {
    /// 可聚焦元素的全局唯一标识符。
    pub struct FocusId;
}

thread_local! {
    /// 没有应用专用 arena 时使用的后备 arena。
    /// 在生产环境中，每次窗口绘制都会将 CURRENT_ELEMENT_ARENA 设为应用的 arena。
    pub(crate) static ELEMENT_ARENA: RefCell<Arena> = RefCell::new(Arena::new(1024 * 1024));

    /// 指向当前 App 的元素 arena（绘制期间）。
    /// 允许多个测试 App 拥有独立的 arena，防止调度器交错任务时发生跨会话损坏。
    static CURRENT_ELEMENT_ARENA: Cell<Option<*const RefCell<Arena>>> = const { Cell::new(None) };
}

/// 此线程上当前是否正在进行窗口绘制。
///
/// 仅在 `ElementArenaScope` 活跃时为 `true`：嵌套作用域会恢复之前的
/// （仍已设置的）arena 指针，因此 `CURRENT_ELEMENT_ARENA` 从最外层绘制
/// 开始到结束期间都是 `Some`。
///
/// `on_request_frame` 回调使用此函数来延迟在绘制已在栈上时重入到达的
/// 绘制请求（例如通过 Windows 窗口过程中的嵌套消息泵），
/// 而不是运行嵌套绘制或在已借用的 App 上 panic。
fn draw_in_progress() -> bool {
    CURRENT_ELEMENT_ARENA.with(|current| current.get().is_some())
}

/// 在当前 arena 中分配元素。如果有活动的应用专用 arena（绘制期间），
/// 则使用该 arena，否则回退到线程局部的 ELEMENT_ARENA。
pub(crate) fn with_element_arena<R>(f: impl FnOnce(&mut Arena) -> R) -> R {
    CURRENT_ELEMENT_ARENA.with(|current| {
        if let Some(arena_ptr) = current.get() {
            // SAFETY: The pointer is valid for the duration of the draw operation
            // that set it, and we're being called during that same draw.
            let arena_cell = unsafe { &*arena_ptr };
            f(&mut arena_cell.borrow_mut())
        } else {
            ELEMENT_ARENA.with_borrow_mut(f)
        }
    })
}

/// 作用域守卫，在绘制操作期间设置 CURRENT_ELEMENT_ARENA 并跟踪 arena 的
/// 作用域深度，以便嵌套绘制的 `ArenaClearNeeded::clear` 被延迟，而不是释放
/// 外层绘制仍引用的内存（参见 `Arena::clear`）。
///
/// 使用进入时的相同 arena 调用 [`ElementArenaScope::exit`] 以获取绘制当前
/// 欠的 [`ArenaClearNeeded`] token；要求 `exit` 使得在作用域结束前无法
/// 请求清理。作用域的拆卸——恢复线程局部变量并用 `end_scope` 平衡
/// `begin_scope`——在 `Drop` 中发生，因此 arena 的作用域深度在每条路径上
/// 都保持平衡，包括 panic 在 `exit` 被到达之前展开绘制的情况。
/// （如果拆卸仅存在于 `exit` 中，这样的 panic 会使作用域深度永久升高，
/// 延迟每次未来的清理，导致内存无界泄漏。）
pub(crate) struct ElementArenaScope {
    /// 进入的 arena：在 `exit` 中与参数比较，在 `Drop` 中解引用以结束其作用域
    /// （参见 SAFETY 说明）。
    entered: *const RefCell<Arena>,
    previous: Option<*const RefCell<Arena>>,
    exited: bool,
}

impl ElementArenaScope {
    /// 进入一个元素分配使用给定 arena 的作用域。
    pub(crate) fn enter(arena: &RefCell<Arena>) -> Self {
        arena.borrow_mut().begin_scope();
        let previous = CURRENT_ELEMENT_ARENA.with(|current| {
            let prev = current.get();
            current.set(Some(arena as *const RefCell<Arena>));
            prev
        });
        Self {
            entered: arena as *const RefCell<Arena>,
            previous,
            exited: false,
        }
    }

    /// 结束作用域：恢复之前活动的 arena 并结束 arena 的清理延迟作用域。
    /// 返回绘制当前欠的 arena 清理 token；在此处生成使得在作用域结束前
    /// 无法请求清理（否则将被永远静默延迟）。
    ///
    /// 如果传入的 arena 与进入时不同则 panic：结束错误 arena 的作用域
    /// 会使两个 arena 的作用域深度失衡，允许其中一个在绘制仍引用其内存时
    /// 进行清理。
    pub(crate) fn exit(mut self, arena: &RefCell<Arena>) -> ArenaClearNeeded {
        assert!(
            std::ptr::eq(self.entered, arena),
            "ElementArenaScope::exit called with a different arena than was entered"
        );
        self.exited = true;
        // Teardown (restoring the thread-local and ending the arena's
        // clear-deferral scope) runs in `Drop`, which fires both here  — `self`
        // is dropped as `exit` returns, before the token reaches the caller  —         // and when a panic unwinds the draw before `exit` is reached.
        ArenaClearNeeded::new(arena)
    }
}

impl Drop for ElementArenaScope {
    fn drop(&mut self) {
        // Teardown lives here (rather than in `exit`) so it runs exactly once on
        // every path: `exit` consumes and drops the guard on the normal path,
        // and unwinding drops it on the panic path. Balancing `begin_scope` here
        // keeps the arena's scope depth correct even when a draw panics; if this
        // only happened in `exit`, a panic between `enter` and `exit` would leave
        // the depth elevated and defer every future clear.
        CURRENT_ELEMENT_ARENA.with(|current| {
            current.set(self.previous);
        });
        // SAFETY: `entered` came from a `&RefCell<Arena>` in `enter`, and the
        // arena (owned by the `App` being drawn) outlives this guard on both the
        // normal and unwinding paths, since the guard is a local of the draw.
        unsafe { &*self.entered }.borrow_mut().end_scope();
        if !self.exited && !std::thread::panicking() {
            debug_assert!(false, "ElementArenaScope dropped without calling exit()");
            log::error!(
                "ElementArenaScope dropped without calling exit(); \
                 the arena clear for this draw was never requested"
            );
        }
    }
}

/// 当元素 arena 已被使用时返回，因此必须在下次绘制前清除。
#[must_use]
pub struct ArenaClearNeeded {
    /// 被绘制的 arena 的标识符。仅在 `clear` 中与另一个指针比较，从不解引用。
    arena: *const RefCell<Arena>,
}

impl ArenaClearNeeded {
    /// 为被绘制的 App 创建新的 ArenaClearNeeded token。私有方法：获取它的唯一
    /// 方式是 [`ElementArenaScope::exit`]。
    fn new(arena: &RefCell<Arena>) -> Self {
        Self {
            arena: arena as *const RefCell<Arena>,
        }
    }

    /// 清除绘制所针对的 App 的元素 arena。如果外层绘制仍在进行中
    /// （此绘制嵌套在其内部），则清理被延迟到外层绘制自身的 `ArenaClearNeeded`，
    /// 以确保其活动分配不会被释放。
    ///
    /// 如果传入的 App 与绘制所针对的不同则 panic，因为清理另一个 App 的
    /// arena 可能会释放其绘制仍引用的内存。
    pub fn clear(self, cx: &mut App) {
        assert!(
            std::ptr::eq(self.arena, &cx.element_arena),
            "ArenaClearNeeded::clear called with a different App than the draw ran against"
        );
        cx.element_arena.borrow_mut().clear();
    }
}

pub(crate) type FocusMap = RwLock<SlotMap<FocusId, FocusRef>>;
pub(crate) struct FocusRef {
    pub(crate) ref_count: AtomicUsize,
    pub(crate) tab_index: isize,
    pub(crate) tab_stop: bool,
}

impl FocusId {
    /// 获取与此句柄关联的元素是否当前具有焦点。
    pub fn is_focused(&self, window: &Window) -> bool {
        window.focus == Some(*self)
    }

    /// 获取与此句柄关联的元素是否包含焦点元素或其自身是否具有焦点。
    pub fn contains_focused(&self, window: &Window, cx: &App) -> bool {
        window
            .focused(cx)
            .is_some_and(|focused| self.contains(focused.id, window))
    }

    /// 获取与此句柄关联的元素是否被包含在焦点元素内或其自身是否具有焦点。
    pub fn within_focused(&self, window: &Window, cx: &App) -> bool {
        let focused = window.focused(cx);
        focused.is_some_and(|focused| focused.id.contains(*self, window))
    }

    /// 获取此句柄在最近渲染帧中是否包含给定句柄。
    pub(crate) fn contains(&self, other: Self, window: &Window) -> bool {
        window
            .rendered_frame
            .dispatch_tree
            .focus_contains(*self, other)
    }
}

/// 用于跟踪和操作窗口中焦点元素的句柄。
pub struct FocusHandle {
    pub(crate) id: FocusId,
    handles: Arc<FocusMap>,
    /// 此元素在 tab 顺序中的索引。
    pub tab_index: isize,
    /// 此元素是否可通过 tab 导航获得焦点。
    pub tab_stop: bool,
}

impl std::fmt::Debug for FocusHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("FocusHandle({:?})", self.id))
    }
}

impl FocusHandle {
    pub(crate) fn new(handles: &Arc<FocusMap>) -> Self {
        let id = handles.write().insert(FocusRef {
            ref_count: AtomicUsize::new(1),
            tab_index: 0,
            tab_stop: false,
        });

        Self {
            id,
            tab_index: 0,
            tab_stop: false,
            handles: handles.clone(),
        }
    }

    pub(crate) fn for_id(id: FocusId, handles: &Arc<FocusMap>) -> Option<Self> {
        let lock = handles.read();
        let focus = lock.get(id)?;
        if atomic_incr_if_not_zero(&focus.ref_count) == 0 {
            return None;
        }
        Some(Self {
            id,
            tab_index: focus.tab_index,
            tab_stop: focus.tab_stop,
            handles: handles.clone(),
        })
    }

    /// 设置与此句柄关联的元素的 tab 索引。
    pub fn tab_index(mut self, index: isize) -> Self {
        self.tab_index = index;
        if let Some(focus) = self.handles.write().get_mut(self.id) {
            focus.tab_index = index;
        }
        self
    }

    /// 设置与此句柄关联的元素是否为 tab 停靠点。
    ///
    /// 当为 `false` 时，该元素不会包含在 tab 顺序中。
    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        if let Some(focus) = self.handles.write().get_mut(self.id) {
            focus.tab_stop = tab_stop;
        }
        self
    }

    /// 将此焦点句柄转换为弱引用变体，不会阻止其被释放。
    pub fn downgrade(&self) -> WeakFocusHandle {
        WeakFocusHandle {
            id: self.id,
            handles: Arc::downgrade(&self.handles),
        }
    }

    /// 将焦点移动到与此句柄关联的元素。
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(self, cx)
    }

    /// 获取与此句柄关联的元素是否当前具有焦点。
    pub fn is_focused(&self, window: &Window) -> bool {
        self.id.is_focused(window)
    }

    /// 获取与此句柄关联的元素是否包含焦点元素或其自身是否具有焦点。
    pub fn contains_focused(&self, window: &Window, cx: &App) -> bool {
        self.id.contains_focused(window, cx)
    }

    /// 获取与此句柄关联的元素是否被包含在焦点元素内或其自身是否具有焦点。
    pub fn within_focused(&self, window: &Window, cx: &mut App) -> bool {
        self.id.within_focused(window, cx)
    }

    /// 获取此句柄在最近渲染帧中是否包含给定句柄。
    pub fn contains(&self, other: &Self, window: &Window) -> bool {
        self.id.contains(other.id, window)
    }

    /// 在渲染此焦点句柄的元素上分发操作。
    pub fn dispatch_action(&self, action: &dyn Action, window: &mut Window, cx: &mut App) {
        if let Some(node_id) = window
            .rendered_frame
            .dispatch_tree
            .focusable_node_id(self.id)
        {
            window.dispatch_action_on_node(node_id, action, cx)
        }
    }
}

impl Clone for FocusHandle {
    fn clone(&self) -> Self {
        Self::for_id(self.id, &self.handles).unwrap()
    }
}

impl PartialEq for FocusHandle {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for FocusHandle {}

impl Drop for FocusHandle {
    fn drop(&mut self) {
        self.handles
            .read()
            .get(self.id)
            .unwrap()
            .ref_count
            .fetch_sub(1, SeqCst);
    }
}

/// 焦点句柄的弱引用。
#[derive(Clone, Debug)]
pub struct WeakFocusHandle {
    pub(crate) id: FocusId,
    pub(crate) handles: Weak<FocusMap>,
}

impl WeakFocusHandle {
    /// 尝试将 [WeakFocusHandle] 升级为 [FocusHandle]。
    pub fn upgrade(&self) -> Option<FocusHandle> {
        let handles = self.handles.upgrade()?;
        FocusHandle::for_id(self.id, &handles)
    }
}

impl PartialEq for WeakFocusHandle {
    fn eq(&self, other: &WeakFocusHandle) -> bool {
        self.id == other.id
    }
}

impl Eq for WeakFocusHandle {}

impl PartialEq<FocusHandle> for WeakFocusHandle {
    fn eq(&self, other: &FocusHandle) -> bool {
        self.id == other.id
    }
}

impl PartialEq<WeakFocusHandle> for FocusHandle {
    fn eq(&self, other: &WeakFocusHandle) -> bool {
        self.id == other.id
    }
}

/// Focusable 允许视图的用户轻松聚焦它
/// （使用 window.focus_view(cx, view)）
pub trait Focusable: 'static {
    /// 返回与此视图关联的焦点句柄。
    fn focus_handle(&self, cx: &App) -> FocusHandle;
}

impl<V: Focusable> Focusable for Entity<V> {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.read(cx).focus_handle(cx)
    }
}

/// ManagedView 是一种视图（如 Modal、Popover、Menu 等），
/// 其生命周期由另一个视图管理。
pub trait ManagedView: Focusable + EventEmitter<DismissEvent> + Render {}

impl<M: Focusable + EventEmitter<DismissEvent> + Render> ManagedView for M {}

/// 由 [`ManagedView`] 的实现者发出，表示视图应被关闭，例如当视图以模态方式呈现时。
pub struct DismissEvent;

type FrameCallback = Box<dyn FnOnce(&mut Window, &mut App)>;

pub(crate) type AnyMouseListener =
    Box<dyn FnMut(&dyn Any, DispatchPhase, &mut Window, &mut App) + 'static>;

#[derive(Clone)]
pub(crate) struct CursorStyleRequest {
    pub(crate) hitbox_id: Option<HitboxId>,
    pub(crate) style: CursorStyle,
}

#[derive(Default, Eq, PartialEq)]
pub(crate) struct HitTest {
    pub(crate) ids: SmallVec<[HitboxId; 8]>,
    pub(crate) hover_hitbox_count: usize,
}

/// 对应平台窗口的窗口控制区域类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowControlArea {
    /// 允许拖动平台窗口的区域。
    Drag,
    /// 允许关闭平台窗口的区域。
    Close,
    /// 允许最大化平台窗口的区域。
    Max,
    /// 允许最小化平台窗口的区域。
    Min,
}

/// 包含 [HitboxBehavior] 的 [Hitbox] 标识符。
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct HitboxId(u64);

#[cfg(feature = "test-support")]
impl HitboxId {
    /// 专用于集成测试 API 的占位 HitboxId，这些 API 需要 hitbox 但其值无关紧要。
    /// 替代方案是将 hitbox 设为 Optional，但这会使实现复杂化。
    pub const fn placeholder() -> Self {
        Self(0)
    }
}

impl HitboxId {
    /// 检查此 ID 的 hitbox 当前是否被悬停。在键盘输入模式期间返回 `false`，
    /// 以便键盘导航抑制悬停高亮。除了处理 `ScrollWheelEvent` 时，这通常是
    /// 确定是否处理鼠标事件或绘制悬停样式时所需的方法。
    ///
    /// 详见 [`Hitbox::is_hovered`]。
    pub fn is_hovered(self, window: &Window) -> bool {
        // If this hitbox has captured the pointer, it's always considered hovered
        if window.captured_hitbox == Some(self) {
            return true;
        }
        if window.last_input_was_keyboard() {
            return false;
        }
        self.hit_test(window)
    }

    /// 检查此 ID 的 hitbox 当前是否被悬停，无论上次使用的输入模式如何。
    ///
    /// 详见 [`HitboxId::is_hovered`]。
    pub(crate) fn is_hovered_ignoring_last_input(self, window: &Window) -> bool {
        // If this hitbox has captured the pointer, it's always considered hovered
        if window.captured_hitbox == Some(self) {
            return true;
        }
        self.hit_test(window)
    }

    fn hit_test(self, window: &Window) -> bool {
        let hit_test = &window.mouse_hit_test;
        for id in hit_test.ids.iter().take(hit_test.hover_hitbox_count) {
            if self == *id {
                return true;
            }
        }
        false
    }

    /// 检查此 ID 的 hitbox 是否包含鼠标并应处理滚动事件。
    /// 通常仅在处理 `ScrollWheelEvent` 时使用，其他情况应使用 `is_hovered`。
    /// 详见 `Hitbox::is_hovered` 文档中关于此区别的说明。
    pub fn should_handle_scroll(self, window: &Window) -> bool {
        window.mouse_hit_test.ids.contains(&self)
    }

    fn next(mut self) -> HitboxId {
        HitboxId(self.0.wrapping_add(1))
    }
}

/// 一个可能阻挡先前插入的 hitbox 的矩形区域。
/// 详见 [Window::insert_hitbox]。
#[derive(Clone, Debug, Deref)]
pub struct Hitbox {
    /// hitbox 的唯一标识符。
    pub id: HitboxId,
    /// hitbox 的边界。
    #[deref]
    pub bounds: Bounds<Pixels>,
    /// 插入 hitbox 时的内容遮罩。
    pub content_mask: ContentMask<Pixels>,
    /// 指定 hitbox 行为的标志。
    pub behavior: HitboxBehavior,
}

impl Hitbox {
    /// 检查 hitbox 当前是否被悬停。在键盘输入模式期间返回 `false`，
    /// 以便键盘导航抑制悬停高亮。除了处理 `ScrollWheelEvent` 时，这通常是
    /// 确定是否处理鼠标事件或绘制悬停样式时所需的方法。
    ///
    /// 即使 hitbox 包含鼠标，如果其前面的 hitbox 设置了
    /// `HitboxBehavior::BlockMouse`（`InteractiveElement::occlude`）或
    /// `HitboxBehavior::BlockMouseExceptScroll`（`InteractiveElement::block_mouse_except_scroll`），
    /// 或当前输入模式为键盘（参见 [`Window::last_input_was_keyboard`]），
    /// 也可能返回 `false`。
    ///
    /// 处理 `ScrollWheelEvent` 时通常应改用 `should_handle_scroll`。
    /// 具体来说，这是由于诸如覆盖层之类的用例，它们使下方元素不可交互的同时
    /// 仍允许滚动。更抽象地说，这是因为 `is_hovered` 关于鼠标正下方的元素交互
    /// ——鼠标移动、点击、悬停样式等。相比之下，滚动关于找到当前最外层的可滚动容器。
    pub fn is_hovered(&self, window: &Window) -> bool {
        self.id.is_hovered(window)
    }

    /// 检查 hitbox 是否包含鼠标并应处理滚动事件。通常仅在处理 `ScrollWheelEvent`
    /// 时使用，其他情况应使用 `is_hovered`。详见 `Hitbox::is_hovered` 文档。
    ///
    /// 即使 hitbox 包含鼠标，如果其前面的 hitbox 设置了
    /// `HitboxBehavior::BlockMouse`（`InteractiveElement::occlude`），也可能返回 `false`。
    pub fn should_handle_scroll(&self, window: &Window) -> bool {
        self.id.should_handle_scroll(window)
    }
}

/// hitbox 如何影响鼠标行为。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum HitboxBehavior {
    /// 正常的 hitbox 鼠标行为，不影响其他 hitbox 的鼠标处理。
    #[default]
    Normal,

    /// 此 hitbox 后面的所有 hitbox 将被忽略，因此 `hitbox.is_hovered() == false`
    /// 且 `hitbox.should_handle_scroll() == false`。对于元素来说，这通常导致
    /// 跳过所有鼠标事件、悬停样式和工具提示。此标志由
    /// [`InteractiveElement::occlude`] 设置。
    ///
    /// 对于检查这些 hitbox 的鼠标处理程序，这与为每种鼠标事件类型注册冒泡阶段
    /// 处理程序行为相同：
    ///
    /// ```ignore
    /// window.on_mouse_event(move |_: &EveryMouseEventTypeHere, phase, window, cx| {
    ///     if phase == DispatchPhase::Capture && hitbox.is_hovered(window) {
    ///         cx.stop_propagation();
    ///     }
    /// })
    /// ```
    ///
    /// 这对事件处理之外也有影响——任何使用 hitbox 检查的地方，如悬停样式和
    /// 工具提示。这些其他行为是此机制的主要目的。替代方案可能是不影响鼠标事件处理
    /// ——但这会允许不一致的 UI，其中点击和移动与不被认为悬停的元素交互。
    BlockMouse,

    /// 此 hitbox 后面的所有 hitbox 将有 `hitbox.is_hovered() == false`，
    /// 即使 `hitbox.should_handle_scroll() == true`。对于元素来说，这通常导致
    /// 忽略除滚动事件之外的所有鼠标交互——详见 [`Hitbox::is_hovered`] 文档。
    /// 此标志由 [`InteractiveElement::block_mouse_except_scroll`] 设置。
    ///
    /// 对于检查这些 hitbox 的鼠标处理程序，这与为每种鼠标事件类型
    /// **除了** `ScrollWheelEvent` 注册冒泡阶段处理程序行为相同：
    ///
    /// ```ignore
    /// window.on_mouse_event(move |_: &EveryMouseEventTypeExceptScroll, phase, window, cx| {
    ///     if phase == DispatchPhase::Bubble && hitbox.should_handle_scroll(window) {
    ///         cx.stop_propagation();
    ///     }
    /// })
    /// ```
    ///
    /// 详见 [`Hitbox::is_hovered`] 文档中关于 `ScrollWheelEvent` 为何与其他
    /// 鼠标事件处理不同的说明。如果还需要阻止这些滚动事件，可以使用类似上面的
    /// `cx.stop_propagation()` 处理程序。
    ///
    /// 这对事件处理之外也有影响——影响任何使用 `is_hovered` 的地方，如悬停样式和
    /// 工具提示。这些其他行为是此机制的主要目的。替代方案可能是不影响鼠标事件处理
    /// ——但这会允许不一致的 UI，其中点击和移动与不被认为悬停的元素交互。
    BlockMouseExceptScroll,
}

/// 工具提示的标识符。
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct TooltipId(usize);

impl TooltipId {
    /// 检查工具提示当前是否被悬停。
    pub fn is_hovered(&self, window: &Window) -> bool {
        window
            .tooltip_bounds
            .as_ref()
            .is_some_and(|tooltip_bounds| {
                tooltip_bounds.id == *self
                    && tooltip_bounds.bounds.contains(&window.mouse_position())
            })
    }
}

pub(crate) struct TooltipBounds {
    id: TooltipId,
    bounds: Bounds<Pixels>,
}

#[derive(Clone)]
pub(crate) struct TooltipRequest {
    id: TooltipId,
    tooltip: AnyTooltip,
}

pub(crate) struct DeferredDraw {
    current_view: EntityId,
    priority: usize,
    parent_node: DispatchNodeId,
    element_id_stack: SmallVec<[ElementId; 32]>,
    text_style_stack: Vec<TextStyleRefinement>,
    content_mask: Option<ContentMask<Pixels>>,
    rem_size: Pixels,
    element: Option<AnyElement>,
    absolute_offset: Point<Pixels>,
    prepaint_range: Range<PrepaintStateIndex>,
    paint_range: Range<PaintIndex>,
}

pub(crate) struct Frame {
    pub(crate) focus: Option<FocusId>,
    pub(crate) window_active: bool,
    pub(crate) element_states: FxHashMap<(GlobalElementId, TypeId), ElementStateBox>,
    accessed_element_states: Vec<(GlobalElementId, TypeId)>,
    pub(crate) mouse_listeners: Vec<Option<AnyMouseListener>>,
    pub(crate) dispatch_tree: DispatchTree,
    pub(crate) scene: Scene,
    pub(crate) hitboxes: Vec<Hitbox>,
    pub(crate) window_control_hitboxes: Vec<(WindowControlArea, Hitbox)>,
    pub(crate) deferred_draws: Vec<DeferredDraw>,
    pub(crate) input_handlers: Vec<Option<PlatformInputHandler>>,
    pub(crate) tooltip_requests: Vec<Option<TooltipRequest>>,
    pub(crate) cursor_styles: Vec<CursorStyleRequest>,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) debug_bounds: FxHashMap<String, Bounds<Pixels>>,
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(crate) next_inspector_instance_ids: FxHashMap<Rc<crate::InspectorElementPath>, usize>,
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(crate) inspector_hitboxes: FxHashMap<HitboxId, crate::InspectorElementId>,
    #[cfg(feature = "dom-backend")]
    pub(crate) dom_key_hitboxes: FxHashMap<crate::DomNodeKey, Vec<HitboxId>>,
    /// DOM 模式下可滚动容器（`overflow: scroll` 的 div）的 key → `ScrollHandle` 映射。
    /// 浏览器原生滚动时由 `dispatch_dom_scroll` 按 key 反查并更新 `ScrollHandle` 的偏移。
    #[cfg(feature = "dom-backend")]
    pub(crate) dom_scroll_handles: FxHashMap<crate::DomNodeKey, crate::ScrollHandle>,
    pub(crate) tab_stops: TabStopMap,
}

#[derive(Clone, Default)]
pub(crate) struct PrepaintStateIndex {
    hitboxes_index: usize,
    tooltips_index: usize,
    deferred_draws_index: usize,
    dispatch_tree_index: usize,
    accessed_element_states_index: usize,
    line_layout_index: LineLayoutIndex,
}

#[derive(Clone, Default)]
pub(crate) struct PaintIndex {
    scene_index: usize,
    mouse_listeners_index: usize,
    input_handlers_index: usize,
    cursor_styles_index: usize,
    accessed_element_states_index: usize,
    tab_handle_index: usize,
    line_layout_index: LineLayoutIndex,
}

impl Frame {
    pub(crate) fn new(dispatch_tree: DispatchTree) -> Self {
        Frame {
            focus: None,
            window_active: false,
            element_states: FxHashMap::default(),
            accessed_element_states: Vec::new(),
            mouse_listeners: Vec::new(),
            dispatch_tree,
            scene: Scene::default(),
            hitboxes: Vec::new(),
            window_control_hitboxes: Vec::new(),
            deferred_draws: Vec::new(),
            input_handlers: Vec::new(),
            tooltip_requests: Vec::new(),
            cursor_styles: Vec::new(),

            #[cfg(any(test, feature = "test-support"))]
            debug_bounds: FxHashMap::default(),

            #[cfg(any(feature = "inspector", debug_assertions))]
            next_inspector_instance_ids: FxHashMap::default(),

            #[cfg(any(feature = "inspector", debug_assertions))]
            inspector_hitboxes: FxHashMap::default(),
            #[cfg(feature = "dom-backend")]
            dom_key_hitboxes: FxHashMap::default(),
            #[cfg(feature = "dom-backend")]
            dom_scroll_handles: FxHashMap::default(),
            tab_stops: TabStopMap::default(),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.element_states.clear();
        self.accessed_element_states.clear();
        self.mouse_listeners.clear();
        self.dispatch_tree.clear();
        self.scene.clear();
        self.input_handlers.clear();
        self.tooltip_requests.clear();
        self.cursor_styles.clear();
        self.hitboxes.clear();
        self.window_control_hitboxes.clear();
        self.deferred_draws.clear();
        self.tab_stops.clear();
        self.focus = None;

        #[cfg(any(test, feature = "test-support"))]
        {
            self.debug_bounds.clear();
        }

        #[cfg(any(feature = "inspector", debug_assertions))]
        {
            self.next_inspector_instance_ids.clear();
            self.inspector_hitboxes.clear();
        }

        #[cfg(feature = "dom-backend")]
        {
            self.dom_key_hitboxes.clear();
            self.dom_scroll_handles.clear();
        }
    }

    pub(crate) fn cursor_style(&self, window: &Window) -> Option<CursorStyle> {
        self.cursor_styles
            .iter()
            .rev()
            .fold_while(None, |style, request| match request.hitbox_id {
                None => Done(Some(request.style)),
                Some(hitbox_id) => Continue(style.or_else(|| {
                    hitbox_id
                        .is_hovered_ignoring_last_input(window)
                        .then_some(request.style)
                })),
            })
            .into_inner()
    }

    pub(crate) fn hit_test(&self, position: Point<Pixels>) -> HitTest {
        let mut set_hover_hitbox_count = false;
        let mut hit_test = HitTest::default();
        for hitbox in self.hitboxes.iter().rev() {
            let bounds = hitbox.bounds.intersect(&hitbox.content_mask.bounds);
            if bounds.contains(&position) {
                hit_test.ids.push(hitbox.id);
                if !set_hover_hitbox_count
                    && hitbox.behavior == HitboxBehavior::BlockMouseExceptScroll
                {
                    hit_test.hover_hitbox_count = hit_test.ids.len();
                    set_hover_hitbox_count = true;
                }
                if hitbox.behavior == HitboxBehavior::BlockMouse {
                    break;
                }
            }
        }
        if !set_hover_hitbox_count {
            hit_test.hover_hitbox_count = hit_test.ids.len();
        }
        hit_test
    }

    pub(crate) fn focus_path(&self) -> SmallVec<[FocusId; 8]> {
        self.focus
            .map(|focus_id| self.dispatch_tree.focus_path(focus_id))
            .unwrap_or_default()
    }

    pub(crate) fn finish(&mut self, prev_frame: &mut Self) {
        for element_state_key in &self.accessed_element_states {
            if let Some((element_state_key, element_state)) =
                prev_frame.element_states.remove_entry(element_state_key)
            {
                self.element_states.insert(element_state_key, element_state);
            }
        }

        self.scene.finish();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
enum InputModality {
    Mouse,
    Keyboard,
    Touch,
}

/// 保存特定窗口的状态。
pub struct Window {
    pub(crate) handle: AnyWindowHandle,
    pub(crate) invalidator: WindowInvalidator,
    pub(crate) removed: bool,
    pub(crate) platform_window: Box<dyn PlatformWindow>,
    display_id: Option<DisplayId>,
    is_resizable: bool,
    is_minimizable: bool,
    sprite_atlas: Arc<dyn PlatformAtlas>,
    text_system: Arc<WindowTextSystem>,
    text_rendering_mode: Rc<Cell<TextRenderingMode>>,
    rem_size: Pixels,
    /// 窗口 rem 大小的覆盖值栈。
    ///
    /// `with_rem_size` 使用此栈允许以给定的 rem 大小渲染元素树。
    rem_size_override_stack: SmallVec<[Pixels; 8]>,
    pub(crate) viewport_size: Size<Pixels>,
    layout_engine: Option<TaffyLayoutEngine>,
    pub(crate) root: Option<AnyView>,
    pub(crate) element_id_stack: SmallVec<[ElementId; 32]>,
    pub(crate) text_style_stack: Vec<TextStyleRefinement>,
    pub(crate) rendered_entity_stack: Vec<EntityId>,
    pub(crate) element_offset_stack: Vec<Point<Pixels>>,
    pub(crate) element_opacity: f32,
    pub(crate) content_mask_stack: Vec<ContentMask<Pixels>>,
    pub(crate) requested_autoscroll: Option<Bounds<Pixels>>,
    pub(crate) image_cache_stack: Vec<AnyImageCache>,
    pub(crate) rendered_frame: Frame,
    pub(crate) next_frame: Frame,
    next_hitbox_id: HitboxId,
    pub(crate) next_tooltip_id: TooltipId,
    pub(crate) tooltip_bounds: Option<TooltipBounds>,
    next_frame_callbacks: Rc<RefCell<Vec<FrameCallback>>>,
    pub(crate) dirty_views: FxHashSet<EntityId>,
    focus_listeners: SubscriberSet<(), AnyWindowFocusListener>,
    pub(crate) focus_lost_listeners: SubscriberSet<(), AnyObserver>,
    default_prevented: bool,
    mouse_position: Point<Pixels>,
    mouse_hit_test: HitTest,
    modifiers: Modifiers,
    capslock: Capslock,
    scale_factor: f32,
    pub(crate) bounds_observers: SubscriberSet<(), AnyObserver>,
    appearance: WindowAppearance,
    pub(crate) appearance_observers: SubscriberSet<(), AnyObserver>,
    pub(crate) button_layout_observers: SubscriberSet<(), AnyObserver>,
    active: Rc<Cell<bool>>,
    hovered: Rc<Cell<bool>>,
    pub(crate) needs_present: Rc<Cell<bool>>,
    /// 跟踪最近的输入事件时间戳以确定输入是否以高速率到达。
    /// 仅在输入速率超过 60fps 时选择性启用 VRR 优化。
    pub(crate) input_rate_tracker: Rc<RefCell<InputRateTracker>>,
    #[cfg(feature = "input-latency-histogram")]
    input_latency_tracker: InputLatencyTracker,
    last_input_modality: InputModality,
    pub(crate) refreshing: bool,
    pub(crate) activation_observers: SubscriberSet<(), AnyObserver>,
    pub(crate) focus: Option<FocusId>,
    focus_enabled: bool,
    /// 每次焦点移动时递增。用于在焦点变化时使待处理的键盘激活状态失效。
    pub(crate) focus_generation: u64,
    pending_input: Option<PendingInput>,
    pending_modifier: ModifierState,
    pub(crate) pending_input_observers: SubscriberSet<(), AnyObserver>,
    prompt: Option<RenderablePromptHandle>,
    pub(crate) client_inset: Option<Pixels>,
    /// 已捕获指针的 hitbox（如果有）。被捕获时，鼠标事件会路由到此 hitbox，
    /// 无论命中测试结果如何。
    captured_hitbox: Option<HitboxId>,
    #[cfg(any(feature = "inspector", debug_assertions))]
    inspector: Option<Entity<Inspector>>,
    pub(crate) a11y: A11y,
    /// Web DOM 后端构建器：当平台窗口声明 `supports_dom()` 时创建，
    /// 每帧在 paint 阶段收集 DOM 节点，帧末交付给平台窗口。
    #[cfg(feature = "dom-backend")]
    pub(crate) dom_builder: Option<crate::DomTreeBuilder>,
}

#[derive(Clone, Debug, Default)]
struct ModifierState {
    modifiers: Modifiers,
    saw_other_input: bool,
}

/// 跟踪输入事件时间戳以确定输入是否以高速率到达。
/// 用于选择性 VRR（可变刷新率）优化。
#[derive(Clone, Debug)]
pub(crate) struct InputRateTracker {
    timestamps: Vec<Instant>,
    window: Duration,
    inputs_per_second: u32,
    sustain_until: Instant,
    sustain_duration: Duration,
}

impl Default for InputRateTracker {
    fn default() -> Self {
        Self {
            timestamps: Vec::new(),
            window: Duration::from_millis(100),
            inputs_per_second: 60,
            sustain_until: Instant::now(),
            sustain_duration: Duration::from_secs(1),
        }
    }
}

impl InputRateTracker {
    pub fn record_input(&mut self) {
        let now = Instant::now();
        self.timestamps.push(now);
        self.prune_old_timestamps(now);

        let min_events = self.inputs_per_second as u128 * self.window.as_millis() / 1000;
        if self.timestamps.len() as u128 >= min_events {
            self.sustain_until = now + self.sustain_duration;
        }
    }

    pub fn is_high_rate(&self) -> bool {
        Instant::now() < self.sustain_until
    }

    fn prune_old_timestamps(&mut self, now: Instant) {
        self.timestamps
            .retain(|&t| now.duration_since(t) <= self.window);
    }
}

/// 窗口输入延迟直方图的时间点快照，适用于外部格式化。
#[cfg(feature = "input-latency-histogram")]
pub struct InputLatencySnapshot {
    /// 输入到帧延迟样本的直方图，单位为纳秒。
    pub latency_histogram: Histogram<u64>,
    /// 每个渲染帧合并的输入事件直方图。
    pub events_per_frame_histogram: Histogram<u64>,
    /// 在绘制期间到达并被排除在延迟记录之外的输入事件数量。
    pub mid_draw_events_dropped: u64,
}

/// 记录帧中第一个输入事件被分发到生成的帧被呈现之间的时间，
/// 在多个事件合并为单个帧时捕获最坏情况延迟。
#[cfg(feature = "input-latency-histogram")]
struct InputLatencyTracker {
    /// 当前帧中第一个未渲染输入事件的时间戳；在帧呈现时清除。
    first_input_at: Option<Instant>,
    /// 自上次帧呈现以来收到的输入事件数量。
    pending_input_count: u64,
    /// 输入到帧延迟样本的直方图，单位为纳秒。
    latency_histogram: Histogram<u64>,
    /// 每个渲染帧合并的输入事件直方图。
    events_per_frame_histogram: Histogram<u64>,
    /// 在绘制期间到达并被排除在延迟记录之外的输入事件数量，
    /// 因为其效果要到下一帧才会显示。
    mid_draw_events_dropped: u64,
}

#[cfg(feature = "input-latency-histogram")]
impl InputLatencyTracker {
    fn new() -> Result<Self> {
        Ok(Self {
            first_input_at: None,
            pending_input_count: 0,
            latency_histogram: Histogram::new(3)
                .map_err(|e| anyhow!("Failed to create input latency histogram: {e}"))?,
            events_per_frame_histogram: Histogram::new(3)
                .map_err(|e| anyhow!("Failed to create events per frame histogram: {e}"))?,
            mid_draw_events_dropped: 0,
        })
    }

    /// 记录在给定时间分发了一个输入事件。
    /// 每帧仅保留第一个事件的时间戳（最坏情况延迟）。
    fn record_input(&mut self, dispatch_time: Instant) {
        self.first_input_at.get_or_insert(dispatch_time);
        self.pending_input_count += 1;
    }

    /// 记录在绘制阶段期间到达并被排除在延迟跟踪之外的输入事件。
    fn record_mid_draw_input(&mut self) {
        self.mid_draw_events_dropped += 1;
    }

    /// 记录帧已呈现，刷新待处理的延迟和合并样本。
    fn record_frame_presented(&mut self) {
        if let Some(first_input_at) = self.first_input_at.take() {
            let latency_nanos = first_input_at.elapsed().as_nanos() as u64;
            self.latency_histogram.record(latency_nanos).ok();
        }
        if self.pending_input_count > 0 {
            self.events_per_frame_histogram
                .record(self.pending_input_count)
                .ok();
            self.pending_input_count = 0;
        }
    }

    fn snapshot(&self) -> InputLatencySnapshot {
        InputLatencySnapshot {
            latency_histogram: self.latency_histogram.clone(),
            events_per_frame_histogram: self.events_per_frame_histogram.clone(),
            mid_draw_events_dropped: self.mid_draw_events_dropped,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DrawPhase {
    None,
    Prepaint,
    Paint,
    Focus,
}

#[derive(Default, Debug)]
struct PendingInput {
    keystrokes: SmallVec<[Keystroke; 1]>,
    focus: Option<FocusId>,
    timer: Option<Task<()>>,
    needs_timeout: bool,
}

pub(crate) struct ElementStateBox {
    pub(crate) inner: Box<dyn Any>,
    #[cfg(debug_assertions)]
    pub(crate) type_name: &'static str,
}

fn default_bounds(display_id: Option<DisplayId>, cx: &mut App) -> WindowBounds {
    // 获取当前活动窗口的初始边界（不含最大化/全屏状态），用于级联定位新窗口
    let active_window_bounds = cx
        .active_window()
        .and_then(|w| {
            w.update(cx, |_, window, _| {
                // 优先使用初始边界，避免最大化/全屏状态影响级联位置
                match window.window_bounds() {
                    WindowBounds::Maximized(_) | WindowBounds::Fullscreen(_) => None,
                    bounds => Some(bounds),
                }
            })
            .ok()
        })
        .flatten();

    const CASCADE_OFFSET: f32 = 25.0;

    let display = display_id
        .map(|id| cx.find_display(id))
        .unwrap_or_else(|| cx.primary_display());

    let default_placement = || Bounds::new(point(px(0.), px(0.)), DEFAULT_WINDOW_SIZE);

    // Use visible_bounds to exclude taskbar/dock areas
    let display_bounds = display
        .as_ref()
        .map(|d| d.visible_bounds())
        .unwrap_or_else(default_placement);

    let (
        Bounds {
            origin: base_origin,
            size: base_size,
        },
        window_bounds_ctor,
    ): (_, fn(Bounds<Pixels>) -> WindowBounds) = match active_window_bounds {
        Some(bounds) => match bounds {
            WindowBounds::Windowed(bounds) => (bounds, WindowBounds::Windowed),
            WindowBounds::Maximized(bounds) => (bounds, WindowBounds::Maximized),
            WindowBounds::Fullscreen(bounds) => (bounds, WindowBounds::Fullscreen),
        },
        None => (
            display
                .as_ref()
                .map(|d| d.default_bounds())
                .unwrap_or_else(default_placement),
            WindowBounds::Windowed,
        ),
    };

    let cascade_offset = point(px(CASCADE_OFFSET), px(CASCADE_OFFSET));
    let proposed_origin = base_origin + cascade_offset;
    let proposed_bounds = Bounds::new(proposed_origin, base_size);

    let display_right = display_bounds.origin.x + display_bounds.size.width;
    let display_bottom = display_bounds.origin.y + display_bounds.size.height;
    let window_right = proposed_bounds.origin.x + proposed_bounds.size.width;
    let window_bottom = proposed_bounds.origin.y + proposed_bounds.size.height;

    let fits_horizontally = window_right <= display_right;
    let fits_vertically = window_bottom <= display_bottom;

    let final_origin = match (fits_horizontally, fits_vertically) {
        (true, true) => proposed_origin,
        (false, true) => point(display_bounds.origin.x, base_origin.y),
        (true, false) => point(base_origin.x, display_bounds.origin.y),
        (false, false) => display_bounds.origin,
    };
    window_bounds_ctor(Bounds::new(final_origin, base_size))
}

impl Window {
    pub(crate) fn new(
        handle: AnyWindowHandle,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<Self> {
        let WindowOptions {
            window_bounds,
            titlebar,
            focus,
            show,
            kind,
            is_movable,
            app_owns_titlebar_drag,
            mouse_passthrough,
            is_resizable,
            is_minimizable,
            display_id,
            window_background,
            app_id,
            window_min_size,
            window_decorations,
            #[cfg_attr(
                not(any(target_os = "linux", target_os = "freebsd")),
                allow(unused_variables)
            )]
            icon,
            #[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
            tabbing_identifier,
        } = options;

        let initial_window_title = titlebar
            .as_ref()
            .and_then(|titlebar| titlebar.title.clone());

        let window_bounds = window_bounds.unwrap_or_else(|| default_bounds(display_id, cx));
        let mut platform_window = cx.platform.open_window(
            handle,
            WindowParams {
                bounds: window_bounds.get_bounds(),
                titlebar,
                kind,
                is_movable,
                app_owns_titlebar_drag,
                mouse_passthrough,
                is_resizable,
                is_minimizable,
                focus,
                show,
                display_id,
                window_min_size,
                app_id: app_id.clone(),
                icon,
                #[cfg(target_os = "macos")]
                tabbing_identifier,
            },
        )?;

        let tab_bar_visible = platform_window.tab_bar_visible();
        SystemWindowTabController::init_visible(cx, tab_bar_visible);
        if let Some(tabs) = platform_window.tabbed_windows() {
            SystemWindowTabController::add_tab(cx, handle.window_id(), tabs);
        }

        let display_id = platform_window.display().map(|display| display.id());
        let sprite_atlas = platform_window.sprite_atlas();
        let mouse_position = platform_window.mouse_position();
        let modifiers = platform_window.modifiers();
        let capslock = platform_window.capslock();
        let content_size = platform_window.content_size();
        let scale_factor = platform_window.scale_factor();
        let appearance = platform_window.appearance();
        let text_system = Arc::new(WindowTextSystem::new(cx.text_system().clone()));
        let invalidator = WindowInvalidator::new();
        let active = Rc::new(Cell::new(platform_window.is_active()));
        let hovered = Rc::new(Cell::new(platform_window.is_hovered()));
        let needs_present = Rc::new(Cell::new(false));
        let next_frame_callbacks: Rc<RefCell<Vec<FrameCallback>>> = Default::default();
        let input_rate_tracker = Rc::new(RefCell::new(InputRateTracker::default()));
        let last_frame_time = Rc::new(Cell::new(None));

        platform_window
            .request_decorations(window_decorations.unwrap_or(WindowDecorations::Server));
        platform_window.set_background_appearance(window_background);

        match window_bounds {
            WindowBounds::Fullscreen(_) => platform_window.toggle_fullscreen(),
            WindowBounds::Maximized(_) => platform_window.zoom(),
            WindowBounds::Windowed(_) => {}
        }

        let accessibility_force_disabled = cx.accessibility_force_disabled;
        let a11y_active_flag = Arc::new(AtomicBool::new(false));

        #[cfg(not(target_family = "wasm"))]
        if !accessibility_force_disabled {
            let mut initial_root_node = accesskit::Node::new(accesskit::Role::Window);
            if let Some(title) = &initial_window_title {
                initial_root_node.set_label(title.to_string());
            }
            let initial_tree = accesskit::TreeUpdate {
                nodes: vec![(ROOT_NODE_ID, initial_root_node)],
                tree: Some(accesskit::Tree::new(ROOT_NODE_ID)),
                tree_id: accesskit::TreeId::ROOT,
                focus: ROOT_NODE_ID,
            };
            let (activation_sender, activation_receiver) = async_channel::unbounded::<()>();
            let (deactivation_sender, deactivation_receiver) = async_channel::unbounded::<()>();
            let (action_sender, action_receiver) =
                async_channel::unbounded::<accesskit::ActionRequest>();

            platform_window.a11y_init(crate::A11yCallbacks {
                activation: {
                    let active_flag = a11y_active_flag.clone();
                    Box::new(move || {
                        log::info!("Accessibility activated");
                        active_flag.store(true, SeqCst);
                        activation_sender.send_blocking(()).log_err();
                        Some(initial_tree.clone())
                    })
                },
                action: Box::new(move |request| {
                    action_sender.send_blocking(request).log_err();
                }),
                deactivation: {
                    let active_flag = a11y_active_flag.clone();
                    Box::new(move || {
                        log::info!("Accessibility deactivated");
                        active_flag.store(false, SeqCst);
                        deactivation_sender.send_blocking(()).log_err();
                    })
                },
            });

            // A11y can be activated at any time, and so we cannot compute a
            // correct `TreeUpdate` on-demand. When this happens, we return a
            // default empty `TreeUpdate`.
            //
            // So we force a new frame, which will then send a correct `TreeUpdate`.
            let mut async_cx = cx.to_async();
            cx.foreground_executor()
                .spawn(async move {
                    while activation_receiver.recv().await.is_ok() {
                        handle
                            .update(&mut async_cx, |_, window, _| window.refresh())
                            .log_err();
                    }
                })
                .detach();

            let mut async_cx = cx.to_async();
            cx.foreground_executor()
                .spawn(async move {
                    while deactivation_receiver.recv().await.is_ok() {
                        handle
                            .update(&mut async_cx, |_, window, _| window.refresh())
                            .log_err();
                    }
                })
                .detach();

            let mut async_cx = cx.to_async();
            cx.foreground_executor()
                .spawn(async move {
                    while let Ok(request) = action_receiver.recv().await {
                        handle
                            .update(&mut async_cx, |_, window, cx| {
                                window.handle_a11y_action(request, cx);
                            })
                            .log_err();
                    }
                })
                .detach();
        }

        platform_window.on_close(Box::new({
            let window_id = handle.window_id();
            let mut cx = cx.to_async();
            move || {
                let _ = handle.update(&mut cx, |_, window, _| window.remove_window());
                let _ = cx.update(|cx| {
                    SystemWindowTabController::remove_tab(cx, window_id);
                });
            }
        }));
        platform_window.on_request_frame(Box::new({
            let mut cx = cx.to_async();
            let invalidator = invalidator.clone();
            let active = active.clone();
            let needs_present = needs_present.clone();
            let next_frame_callbacks = next_frame_callbacks.clone();
            let input_rate_tracker = input_rate_tracker.clone();
            let mut deferred_force_render = false;
            move |request_frame_options| {
                // This must be checked before anything else: if this request
                // arrived re-entrantly while a draw is on this thread's stack
                // (e.g. via a nested message pump in the Windows window
                // procedure), drawing would nest draws, and even touching the
                // App would panic on its already-mutable borrow. Skip instead;
                // the platform leaves the window invalidated (or re-invalidates
                // it), so a fresh request arrives once the in-progress draw
                // unwinds. Remember force_render so the deferred frame still
                // bypasses the view cache.
                //
                // Returning here skips `complete_frame`, which on Wayland would
                // stall the window's frame callbacks (no `surface.commit()`)  —                 // but calling it would hit the App borrow panic above, and this
                // branch is unreachable there in practice: only Windows pumps
                // platform events (and thus requests frames) mid-draw.
                if draw_in_progress() {
                    log::debug!("deferring re-entrant window draw request");
                    deferred_force_render |= request_frame_options.force_render;
                    return;
                }
                // Take the deferred flag first: `||` short-circuits, and leaving
                // the flag set when this request already forces a render would
                // force a second, redundant render on the next frame.
                let force_render =
                    mem::take(&mut deferred_force_render) || request_frame_options.force_render;

                let thermal_state = handle
                    .update(&mut cx, |_, _, cx| cx.thermal_state())
                    .log_err();

                // Throttle frame rate based on conditions:
                // - Thermal pressure (Serious/Critical): cap to ~60fps
                // - Inactive window (not focused): cap to ~30fps to save energy
                let min_frame_interval = if request_frame_options.require_presentation
                    || (!request_frame_options.force_render
                        && next_frame_callbacks.borrow().is_empty())
                {
                    None
                } else if !active.get() && !input_rate_tracker.borrow_mut().is_high_rate() {
                    Some(Duration::from_micros(33333))
                } else if let Some(ThermalState::Critical | ThermalState::Serious) = thermal_state {
                    Some(Duration::from_micros(16667))
                } else {
                    None
                };

                let now = Instant::now();
                if let Some(min_interval) = min_frame_interval {
                    if let Some(last_frame) = last_frame_time.get()
                        && now.duration_since(last_frame) < min_interval
                    {
                        // Don't lose a pending forced render to throttling.
                        deferred_force_render |= force_render;
                        // Must still complete the frame on platforms that require it.
                        // On Wayland, `surface.frame()` was already called to request the
                        // next frame callback, so we must call `surface.commit()` (via
                        // `complete_frame`) or the compositor won't send another callback.
                        handle
                            .update(&mut cx, |_, window, _| window.complete_frame())
                            .log_err();
                        return;
                    }
                }
                last_frame_time.set(Some(now));

                let next_frame_callbacks = next_frame_callbacks.take();
                if !next_frame_callbacks.is_empty() {
                    handle
                        .update(&mut cx, |_, window, cx| {
                            for callback in next_frame_callbacks {
                                callback(window, cx);
                            }
                        })
                        .log_err();
                }

                // Keep presenting if input was recently arriving at a high rate (>= 60fps).
                // Once high-rate input is detected, we sustain presentation for 1 second
                // to prevent display underclocking during active input.
                let needs_present = request_frame_options.require_presentation
                    || needs_present.get()
                    || input_rate_tracker.borrow_mut().is_high_rate();

                if invalidator.is_dirty() || force_render {
                    measure("frame duration", || {
                        handle
                            .update(&mut cx, |_, window, cx| {
                                if force_render {
                                    // Bypass cached view reuse so we don't replay stale
                                    // atlas tile references after a GPU device recovery.
                                    window.refresh();
                                }
                                let arena_clear_needed = window.draw(cx);
                                window.present();
                                arena_clear_needed.clear(cx);
                            })
                            .log_err();
                    })
                } else if needs_present {
                    handle
                        .update(&mut cx, |_, window, _| window.present())
                        .log_err();
                }

                handle
                    .update(&mut cx, |_, window, _| {
                        window.complete_frame();
                    })
                    .log_err();
            }
        }));
        platform_window.on_resize(Box::new({
            let mut cx = cx.to_async();
            move |_, _| {
                handle
                    .update(&mut cx, |_, window, cx| window.bounds_changed(cx))
                    .log_err();
            }
        }));
        platform_window.on_moved(Box::new({
            let mut cx = cx.to_async();
            move || {
                handle
                    .update(&mut cx, |_, window, cx| window.bounds_changed(cx))
                    .log_err();
            }
        }));
        platform_window.on_appearance_changed(Box::new({
            let cx = cx.to_async();
            let foreground_executor = cx.foreground_executor().clone();
            move || {
                let mut cx = cx.clone();
                // 延迟更新：修改 AppKit 外观时可能同步触发此回调，而此刻 App 可能已被借用。
                foreground_executor
                    .spawn(async move {
                        handle
                            .update(&mut cx, |_, window, cx| window.appearance_changed(cx))
                            .log_err();
                    })
                    .detach();
            }
        }));
        platform_window.on_button_layout_changed(Box::new({
            let mut cx = cx.to_async();
            move || {
                handle
                    .update(&mut cx, |_, window, cx| window.button_layout_changed(cx))
                    .log_err();
            }
        }));
        platform_window.on_active_status_change(Box::new({
            let mut cx = cx.to_async();
            move |active| {
                handle
                    .update(&mut cx, |_, window, cx| {
                        window.active.set(active);
                        window.modifiers = window.platform_window.modifiers();
                        window.capslock = window.platform_window.capslock();
                        window
                            .activation_observers
                            .clone()
                            .retain(&(), |callback| callback(window, cx));

                        window.bounds_changed(cx);
                        window.refresh();

                        SystemWindowTabController::update_last_active(cx, window.handle.id);
                    })
                    .log_err();
            }
        }));
        platform_window.on_hover_status_change(Box::new({
            let mut cx = cx.to_async();
            move |active| {
                handle
                    .update(&mut cx, |_, window, _| {
                        window.hovered.set(active);
                        window.refresh();
                    })
                    .log_err();
            }
        }));
        platform_window.on_input({
            let mut cx = cx.to_async();
            Box::new(move |event| {
                handle
                    .update(&mut cx, |_, window, cx| window.dispatch_event(event, cx))
                    .log_err()
                    .unwrap_or(DispatchEventResult::default())
            })
        });
        #[cfg(feature = "dom-backend")]
        platform_window.on_dom_event({
            let mut cx = cx.to_async();
            Box::new(move |keys, event| {
                handle
                    .update(&mut cx, |_, window, cx| {
                        window.dispatch_event_for_dom(keys, event, cx)
                    })
                    .log_err()
                    .unwrap_or(DispatchEventResult::default())
            })
        });
        #[cfg(feature = "dom-backend")]
        platform_window.on_dom_scroll({
            let mut cx = cx.to_async();
            Box::new(move |keys, left, top| {
                let _ = handle
                    .update(&mut cx, |_, window, _cx| {
                        window.dispatch_dom_scroll(keys, left, top);
                    })
                    .log_err();
            })
        });
        platform_window.on_hit_test_window_control({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, window, _cx| {
                        for (area, hitbox) in &window.rendered_frame.window_control_hitboxes {
                            if window.mouse_hit_test.ids.contains(&hitbox.id) {
                                return Some(*area);
                            }
                        }
                        None
                    })
                    .log_err()
                    .unwrap_or(None)
            })
        });
        platform_window.on_move_tab_to_new_window({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, _window, cx| {
                        SystemWindowTabController::move_tab_to_new_window(cx, handle.window_id());
                    })
                    .log_err();
            })
        });
        platform_window.on_merge_all_windows({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, _window, cx| {
                        SystemWindowTabController::merge_all_windows(cx, handle.window_id());
                    })
                    .log_err();
            })
        });
        platform_window.on_select_next_tab({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, _window, cx| {
                        SystemWindowTabController::select_next_tab(cx, handle.window_id());
                    })
                    .log_err();
            })
        });
        platform_window.on_select_previous_tab({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, _window, cx| {
                        SystemWindowTabController::select_previous_tab(cx, handle.window_id())
                    })
                    .log_err();
            })
        });
        platform_window.on_toggle_tab_bar({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, window, cx| {
                        let tab_bar_visible = window.platform_window.tab_bar_visible();
                        SystemWindowTabController::set_visible(cx, tab_bar_visible);
                    })
                    .log_err();
            })
        });

        if let Some(app_id) = app_id {
            platform_window.set_app_id(&app_id);
        }

        platform_window.map_window().unwrap();

        #[cfg(feature = "dom-backend")]
        let supports_dom = platform_window.supports_dom();

        Ok(Window {
            handle,
            invalidator,
            removed: false,
            platform_window,
            display_id,
            is_resizable,
            is_minimizable,
            sprite_atlas,
            text_system,
            text_rendering_mode: cx.text_rendering_mode.clone(),
            rem_size: px(16.),
            rem_size_override_stack: SmallVec::new(),
            viewport_size: content_size,
            layout_engine: Some(TaffyLayoutEngine::new()),
            root: None,
            element_id_stack: SmallVec::default(),
            text_style_stack: Vec::new(),
            rendered_entity_stack: Vec::new(),
            element_offset_stack: Vec::new(),
            content_mask_stack: Vec::new(),
            element_opacity: 1.0,
            requested_autoscroll: None,
            rendered_frame: Frame::new(DispatchTree::new(cx.keymap.clone(), cx.actions.clone())),
            next_frame: Frame::new(DispatchTree::new(cx.keymap.clone(), cx.actions.clone())),
            next_frame_callbacks,
            next_hitbox_id: HitboxId(0),
            next_tooltip_id: TooltipId::default(),
            tooltip_bounds: None,
            dirty_views: FxHashSet::default(),
            focus_listeners: SubscriberSet::new(),
            focus_lost_listeners: SubscriberSet::new(),
            default_prevented: true,
            mouse_position,
            mouse_hit_test: HitTest::default(),
            modifiers,
            capslock,
            scale_factor,
            bounds_observers: SubscriberSet::new(),
            appearance,
            appearance_observers: SubscriberSet::new(),
            button_layout_observers: SubscriberSet::new(),
            active,
            hovered,
            needs_present,
            input_rate_tracker,
            #[cfg(feature = "input-latency-histogram")]
            input_latency_tracker: InputLatencyTracker::new()?,
            last_input_modality: InputModality::Mouse,
            refreshing: false,
            activation_observers: SubscriberSet::new(),
            focus: None,
            focus_enabled: true,
            focus_generation: 0,
            pending_input: None,
            pending_modifier: ModifierState::default(),
            pending_input_observers: SubscriberSet::new(),
            prompt: None,
            client_inset: None,
            image_cache_stack: Vec::new(),
            captured_hitbox: None,
            #[cfg(any(feature = "inspector", debug_assertions))]
            inspector: None,
            a11y: A11y::new(
                a11y_active_flag,
                accessibility_force_disabled,
                initial_window_title,
            ),
            #[cfg(feature = "dom-backend")]
            dom_builder: supports_dom.then(crate::DomTreeBuilder::new),
        })
    }

    pub(crate) fn new_focus_listener(
        &self,
        value: AnyWindowFocusListener,
    ) -> (Subscription, impl FnOnce() + use<>) {
        self.focus_listeners.insert((), value)
    }
}

/// 事件分发结果 — 指示事件是否继续传播以及默认行为是否被阻止。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DispatchEventResult {
    /// 事件是否继续向父元素传播。
    pub propagate: bool,
    /// 默认行为是否已被阻止。
    pub default_prevented: bool,
}

/// 表示窗口的哪个区域是可见的。超出此遮罩的内容将不会被渲染。
/// 目前仅支持矩形内容遮罩，但我们为遮罩定义了独立类型，以便未来支持更复杂的形状。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct ContentMask<P: Clone + Debug + Default + PartialEq> {
    /// 边界
    pub bounds: Bounds<P>,
}

impl ContentMask<Pixels> {
    /// 按给定缩放因子缩放内容遮罩的像素单位。
    pub fn scale(&self, factor: f32) -> ContentMask<ScaledPixels> {
        ContentMask {
            bounds: self.bounds.scale(factor),
        }
    }

    /// 将此内容遮罩与给定内容遮罩取交集。
    pub fn intersect(&self, other: &Self) -> Self {
        let bounds = self.bounds.intersect(&other.bounds);
        ContentMask { bounds }
    }
}

impl Window {
    fn mark_view_dirty(&mut self, view_id: EntityId) {
        // Mark ancestor views as dirty. If already in the `dirty_views` set, then all its ancestors
        // should already be dirty.
        for view_id in self
            .rendered_frame
            .dispatch_tree
            .view_path_reversed(view_id)
        {
            if !self.dirty_views.insert(view_id) {
                break;
            }
        }
    }

    /// 注册窗口外观变化时调用的回调。
    pub fn observe_window_appearance(
        &self,
        mut callback: impl FnMut(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.appearance_observers.insert(
            (),
            Box::new(move |window, cx| {
                callback(window, cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// 注册窗口按钮布局变化时调用的回调。
    pub fn observe_button_layout_changed(
        &self,
        mut callback: impl FnMut(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.button_layout_observers.insert(
            (),
            Box::new(move |window, cx| {
                callback(window, cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// 用新的根实体替换窗口的根实体。
    pub fn replace_root<E>(
        &mut self,
        cx: &mut App,
        build_view: impl FnOnce(&mut Window, &mut Context<E>) -> E,
    ) -> Entity<E>
    where
        E: 'static + Render,
    {
        let view = cx.new(|cx| build_view(self, cx));
        self.root = Some(view.clone().into());
        self.refresh();
        view
    }

    /// 返回窗口的根实体（如果有）。
    pub fn root<E>(&self) -> Option<Option<Entity<E>>>
    where
        E: 'static + Render,
    {
        self.root
            .as_ref()
            .map(|view| view.clone().downcast::<E>().ok())
    }

    /// 获取属于此上下文的窗口句柄。
    pub fn window_handle(&self) -> AnyWindowHandle {
        self.handle
    }

    /// 将窗口标记为脏，安排在下一帧重绘。
    /// 由 DOM 后端在浏览器原生滚动（可滚动容器的 `scroll` 事件）后回调：
    /// 根据事件链反查到对应的可滚动容器，把浏览器滚动位置同步回其 [`ScrollHandle`]。
    ///
    /// `left`/`top` 为浏览器滚动视口的 `scrollLeft`/`scrollTop`（向下/向右为正），
    /// 与 Rust 的滚动偏移（向下/向右为负）符号相反，这里取反后写入。
    #[cfg(feature = "dom-backend")]
    pub(crate) fn dispatch_dom_scroll(
        &mut self,
        keys: Vec<crate::DomNodeKey>,
        left: f64,
        top: f64,
    ) {
        let handle = keys
            .iter()
            .rev()
            .find_map(|k| self.rendered_frame.dom_scroll_handles.get(k).cloned());
        let Some(handle) = handle else { return };
        let new_offset = Point::new(px(-left as f32), px(-top as f32));
        // 仅在滚动位置真正变化时刷新，避免原生滚动回写引发的重绘循环。
        if handle.offset() != new_offset {
            handle.set_offset(new_offset);
            self.refresh();
        }
    }

    /// 标记窗口需要重绘（立即调度下一帧），用于滚动/输入等交互后刷新视图。
    pub fn refresh(&mut self) {
        if self.invalidator.not_drawing() {
            self.refreshing = true;
            self.invalidator.set_dirty(true);
        }
    }

    /// 关闭此窗口。
    pub fn remove_window(&mut self) {
        self.removed = true;
    }

    /// 获取当前聚焦的 [`FocusHandle`]。如果没有元素聚焦，返回 `None`。
    pub fn focused(&self, cx: &App) -> Option<FocusHandle> {
        self.focus
            .and_then(|id| FocusHandle::for_id(id, &cx.focus_handles))
    }

    /// 将焦点移动到与给定 [`FocusHandle`] 关联的元素。
    pub fn focus(&mut self, handle: &FocusHandle, cx: &mut App) {
        if !self.focus_enabled || self.focus == Some(handle.id) {
            return;
        }

        self.focus = Some(handle.id);
        self.focus_generation = self.focus_generation.wrapping_add(1);
        self.clear_pending_keystrokes();

        // Avoid re-entrant entity updates by deferring observer notifications to the end of the
        // current effect cycle, and only for this window.
        let window_handle = self.handle;
        cx.defer(move |cx| {
            window_handle
                .update(cx, |_, window, cx| {
                    window.pending_input_changed(cx);
                })
                .ok();
        });

        self.refresh();
    }

    /// 移除此上下文窗口中所有元素的焦点。
    pub fn blur(&mut self) {
        if !self.focus_enabled {
            return;
        }

        if self.focus.is_some() {
            self.focus_generation = self.focus_generation.wrapping_add(1);
        }
        self.focus = None;
        self.refresh();
    }

    /// 使窗口失焦并不允许其中任何元素再次获得焦点。
    pub fn disable_focus(&mut self) {
        self.blur();
        self.focus_enabled = false;
    }

    /// 将焦点移动到下一个 tab 停靠点。
    pub fn focus_next(&mut self, cx: &mut App) {
        if !self.focus_enabled {
            return;
        }

        if let Some(handle) = self.rendered_frame.tab_stops.next(self.focus.as_ref()) {
            self.focus(&handle, cx)
        }
    }

    /// 将焦点移动到上一个 tab 停靠点。
    pub fn focus_prev(&mut self, cx: &mut App) {
        if !self.focus_enabled {
            return;
        }

        if let Some(handle) = self.rendered_frame.tab_stops.prev(self.focus.as_ref()) {
            self.focus(&handle, cx)
        }
    }

    /// 获取文本系统的访问器。
    pub fn text_system(&self) -> &Arc<WindowTextSystem> {
        &self.text_system
    }

    /// 当前文本样式。由提供给 `with_text_style` 的所有样式细化组合而成。
    pub fn text_style(&self) -> TextStyle {
        let mut style = TextStyle::default();
        for refinement in &self.text_style_stack {
            style.refine(refinement);
        }
        style
    }

    /// 检查平台窗口是否已最大化。
    ///
    /// 在某些平台（如 Windows）上，这与边界为显示器大小不同。
    pub fn is_maximized(&self) -> bool {
        self.platform_window.is_maximized()
    }

    /// 请求特定的窗口装饰（Wayland）
    pub fn request_decorations(&self, decorations: WindowDecorations) {
        self.platform_window.request_decorations(decorations);
    }

    /// 设置 layer-shell 表面的独占区域：它保留多少屏幕空间
    /// 以使其他表面避免遮挡它（例如面板保留空间）。
    /// 正值从锚定边缘保留该距离，0 允许
    /// 表面被移出其他独占区域，-1 忽略保留
    /// 空间并可能延伸到其他表面下方。（仅限 Wayland layer-shell 窗口）
    pub fn set_exclusive_zone(&self, zone: Pixels) {
        self.platform_window.set_exclusive_zone(zone);
    }

    /// 设置 layer-shell 表面独占区域适用的锚定边缘。
    /// 仅在角锚定表面时需要此选项；否则
    /// 边缘从锚点推断。边缘必须是表面锚定的
    /// 单一边缘，否则将被忽略。（仅限 Wayland layer-shell 窗口）
    #[cfg(all(target_os = "linux", feature = "wayland"))]
    pub fn set_exclusive_edge(&self, edge: crate::layer_shell::Anchor) {
        self.platform_window.set_exclusive_edge(edge);
    }

    /// 如果此窗口可调整大小，则启动交互式窗口调整大小操作。
    pub fn start_window_resize(&self, edge: ResizeEdge) {
        if self.is_resizable {
            self.platform_window.start_window_resize(edge);
        }
    }

    /// 仅限 Linux（wayland）：设置窗口的输入区域，即接收指针
    /// 和触摸输入的区域。其外部的事件将传递到窗口下方的内容。
    ///
    /// - `Some(rects)` 将输入限制为 `rects` 的并集（窗口坐标）。
    /// - `Some(&[])` 表示空区域，窗口不接收任何指针或触摸输入。
    /// - `None` 将区域重置为默认值，整个窗口重新接收输入。
    pub fn set_input_region(&self, region: Option<&[Bounds<Pixels>]>) {
        self.platform_window.set_input_region(region);
    }

    /// 返回 `WindowBounds` 以指示窗口关闭后应如何
    /// 重新打开
    pub fn window_bounds(&self) -> WindowBounds {
        self.platform_window.window_bounds()
    }

    /// 返回不包含内边距的 `WindowBounds`（Wayland 和 X11）
    pub fn inner_window_bounds(&self) -> WindowBounds {
        self.platform_window.inner_window_bounds()
    }

    /// 在当前聚焦的元素上分发给定操作。
    pub fn dispatch_action(&mut self, action: Box<dyn Action>, cx: &mut App) {
        let focus_id = self.focused(cx).map(|handle| handle.id);

        let window = self.handle;
        cx.defer(move |cx| {
            window
                .update(cx, |_, window, cx| {
                    let node_id = window.focus_node_id_in_rendered_frame(focus_id);
                    window.dispatch_action_on_node(node_id, action.as_ref(), cx);
                })
                .log_err();
        })
    }

    pub(crate) fn dispatch_keystroke_observers(
        &mut self,
        event: &dyn Any,
        action: Option<Box<dyn Action>>,
        context_stack: Vec<KeyContext>,
        cx: &mut App,
    ) {
        let Some(key_down_event) = event.downcast_ref::<KeyDownEvent>() else {
            return;
        };

        cx.keystroke_observers.clone().retain(&(), move |callback| {
            (callback)(
                &KeystrokeEvent {
                    keystroke: key_down_event.keystroke.clone(),
                    action: action.as_ref().map(|action| action.boxed_clone()),
                    context_stack: context_stack.clone(),
                },
                self,
                cx,
            )
        });
    }

    pub(crate) fn dispatch_keystroke_interceptors(
        &mut self,
        event: &dyn Any,
        context_stack: Vec<KeyContext>,
        cx: &mut App,
    ) {
        let Some(key_down_event) = event.downcast_ref::<KeyDownEvent>() else {
            return;
        };

        cx.keystroke_interceptors
            .clone()
            .retain(&(), move |callback| {
                (callback)(
                    &KeystrokeEvent {
                        keystroke: key_down_event.keystroke.clone(),
                        action: None,
                        context_stack: context_stack.clone(),
                    },
                    self,
                    cx,
                )
            });
    }

    /// 安排给定函数在当前效果周期结束时运行，允许当前在栈上的
    /// 实体返回到应用。
    pub fn defer(&self, cx: &mut App, f: impl FnOnce(&mut Window, &mut App) + 'static) {
        let handle = self.handle;
        cx.defer(move |cx| {
            handle.update(cx, |_, window, cx| f(window, cx)).ok();
        });
    }

    /// 订阅实体发出的事件。
    /// 你订阅的实体必须实现 [`EventEmitter`] trait。
    /// 回调将传入发出实体的句柄、事件和当前窗口的窗口上下文。
    pub fn observe<T: 'static>(
        &mut self,
        observed: &Entity<T>,
        cx: &mut App,
        mut on_notify: impl FnMut(Entity<T>, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        let entity_id = observed.entity_id();
        let observed = observed.downgrade();
        let window_handle = self.handle;
        cx.new_observer(
            entity_id,
            Box::new(move |cx| {
                window_handle
                    .update(cx, |_, window, cx| {
                        if let Some(handle) = observed.upgrade() {
                            on_notify(handle, window, cx);
                            true
                        } else {
                            false
                        }
                    })
                    .unwrap_or(false)
            }),
        )
    }

    /// 订阅实体发出的事件。
    /// 你订阅的实体必须实现 [`EventEmitter`] trait。
    /// 回调将传入发出实体的句柄、事件和当前窗口的窗口上下文。
    pub fn subscribe<Emitter, Evt>(
        &mut self,
        entity: &Entity<Emitter>,
        cx: &mut App,
        mut on_event: impl FnMut(Entity<Emitter>, &Evt, &mut Window, &mut App) + 'static,
    ) -> Subscription
    where
        Emitter: EventEmitter<Evt>,
        Evt: 'static,
    {
        let entity_id = entity.entity_id();
        let handle = entity.downgrade();
        let window_handle = self.handle;
        cx.new_subscription(
            entity_id,
            (
                TypeId::of::<Evt>(),
                Box::new(move |event, cx| {
                    window_handle
                        .update(cx, |_, window, cx| {
                            if let Some(entity) = handle.upgrade() {
                                let event = event.downcast_ref().expect("invalid event type");
                                on_event(entity, event, window, cx);
                                true
                            } else {
                                false
                            }
                        })
                        .unwrap_or(false)
                }),
            ),
        )
    }

    /// 注册一个回调，在给定 `Entity` 被释放时调用。
    pub fn observe_release<T>(
        &self,
        entity: &Entity<T>,
        cx: &mut App,
        mut on_release: impl FnOnce(&mut T, &mut Window, &mut App) + 'static,
    ) -> Subscription
    where
        T: 'static,
    {
        let entity_id = entity.entity_id();
        let window_handle = self.handle;
        let (subscription, activate) = cx.release_listeners.insert(
            entity_id,
            Box::new(move |entity, cx| {
                let entity = entity.downcast_mut().expect("invalid entity type");
                let _ = window_handle.update(cx, |_, window, cx| on_release(entity, window, cx));
            }),
        );
        activate();
        subscription
    }

    /// 创建一个 [`AsyncWindowContext`]，它具有静态生命周期，可以跨
    /// 异步代码中的 await 点持有。
    pub fn to_async(&self, cx: &App) -> AsyncWindowContext {
        AsyncWindowContext::new_context(cx.to_async(), self.handle)
    }

    /// 安排给定闭包在当前帧渲染后直接运行。
    pub fn on_next_frame(&self, callback: impl FnOnce(&mut Window, &mut App) + 'static) {
        RefCell::borrow_mut(&self.next_frame_callbacks).push(Box::new(callback));
    }

    /// 返回窗口左上角在屏幕坐标中的位置。
    pub fn position(&self) -> Point<Pixels> {
        self.bounds().origin
    }

    /// 返回窗口所在屏幕的大小，或主屏幕大小（如果不可用）。
    pub fn screen_size(&self, cx: &App) -> Option<Size<Pixels>> {
        self.display(cx)
            .or_else(|| cx.primary_display())
            .map(|d| d.bounds().size)
    }

    /// 安排在下一动画帧绘制一帧。
    ///
    /// 这对于需要持续动画的元素很有用，例如视频播放器或动画 GIF。
    /// 即使没有其他变化，它也会导致窗口在下一帧重绘。
    ///
    /// 如果从视图内部调用，它将在下一帧通知该视图。否则，它将刷新整个窗口。
    ///
    /// 驱动纯装饰动画（旋转器、脉冲等）的调用者应优先使用
    /// [`AnimationExt::with_animation`](crate::AnimationExt::with_animation)，
    /// 它会自动遵循 [`App::reduce_motion`]。直接使用此
    /// 方法进行装饰性动画时，应检查 [`App::reduce_motion`]
    /// 并在设置时跳过帧请求。
    pub fn request_animation_frame(&self) {
        let entity = self.current_view();
        self.on_next_frame(move |_, cx| cx.notify(entity));
    }

    /// 运行通过 [`Self::on_next_frame`] 安排的所有回调，返回运行的数量。
    ///
    /// 测试没有平台帧循环，因此这模拟了
    /// 下一帧的交付。
    #[cfg(any(test, feature = "test-support"))]
    pub fn simulate_next_frame(&mut self, cx: &mut App) -> usize {
        let callbacks = self.next_frame_callbacks.take();
        let count = callbacks.len();
        for callback in callbacks {
            callback(self, cx);
        }
        count
    }

    /// 在应用程序线程池上生成给定闭包返回的 future。
    /// 闭包会获得当前窗口的句柄和一个 `AsyncWindowContext` 供
    /// 在你的 future 中使用。
    #[track_caller]
    pub fn spawn<AsyncFn, R>(&self, cx: &App, f: AsyncFn) -> Task<R>
    where
        R: 'static,
        AsyncFn: AsyncFnOnce(&mut AsyncWindowContext) -> R + 'static,
    {
        let handle = self.handle;
        cx.spawn(async move |app| {
            let mut async_window_cx = AsyncWindowContext::new_context(app.clone(), handle);
            f(&mut async_window_cx).await
        })
    }

    /// 在应用程序线程池上生成给定闭包返回的 future，
    /// 使用给定的优先级。闭包会获得当前窗口的
    /// 句柄和一个 `AsyncWindowContext` 供在你的 future 中使用。
    #[track_caller]
    pub fn spawn_with_priority<AsyncFn, R>(
        &self,
        priority: Priority,
        cx: &App,
        f: AsyncFn,
    ) -> Task<R>
    where
        R: 'static,
        AsyncFn: AsyncFnOnce(&mut AsyncWindowContext) -> R + 'static,
    {
        let handle = self.handle;
        cx.spawn_with_priority(priority, async move |app| {
            let mut async_window_cx = AsyncWindowContext::new_context(app.clone(), handle);
            f(&mut async_window_cx).await
        })
    }

    /// 通知窗口其边界已更改。
    ///
    /// 这会从平台窗口更新内部状态，如 `viewport_size` 和 `scale_factor`，
    /// 然后通知观察者。通常由平台的
    /// 调整大小回调自动调用，但公开暴露以用于测试基础设施。
    pub fn bounds_changed(&mut self, cx: &mut App) {
        self.scale_factor = self.platform_window.scale_factor();
        self.viewport_size = self.platform_window.content_size();
        self.display_id = self.platform_window.display().map(|display| display.id());
        self.mouse_position = self.platform_window.mouse_position();

        self.refresh();

        self.bounds_observers
            .clone()
            .retain(&(), |callback| callback(self, cx));
    }

    /// 返回当前窗口在全局坐标空间中的边界，可能跨多个显示器。
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.platform_window.bounds()
    }

    /// 将当前帧的场景渲染到纹理并返回 RGBA 格式的像素数据。
    /// 这不会将帧呈现到屏幕——用于我们想要
    /// 在不显示或要求窗口可见的情况下捕获渲染内容的视觉测试。
    #[cfg(any(test, feature = "test-support"))]
    pub fn render_to_image(&self) -> anyhow::Result<image::RgbaImage> {
        self.platform_window
            .render_to_image(&self.rendered_frame.scene)
    }

    /// 设置窗口的内容大小。
    pub fn resize(&mut self, size: Size<Pixels>) {
        self.platform_window.resize(size);
    }

    /// 返回窗口当前是否为全屏状态
    pub fn is_fullscreen(&self) -> bool {
        self.platform_window.is_fullscreen()
    }

    pub(crate) fn appearance_changed(&mut self, cx: &mut App) {
        self.appearance = self.platform_window.appearance();

        self.appearance_observers
            .clone()
            .retain(&(), |callback| callback(self, cx));
    }

    pub(crate) fn button_layout_changed(&mut self, cx: &mut App) {
        self.button_layout_observers
            .clone()
            .retain(&(), |callback| callback(self, cx));
    }

    /// 返回当前窗口的外观。
    pub fn appearance(&self) -> WindowAppearance {
        self.appearance
    }

    /// 返回窗口内可绘制区域的大小。
    pub fn viewport_size(&self) -> Size<Pixels> {
        self.viewport_size
    }

    /// 返回此窗口是否被操作系统聚焦（接收按键事件）。
    pub fn is_window_active(&self) -> bool {
        self.active.get()
    }

    /// 返回此窗口是否被认为是
    /// 当前拥有鼠标光标的窗口。
    /// 在 Mac 上，这等同于 `is_window_active`。
    pub fn is_window_hovered(&self) -> bool {
        if cfg!(any(
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd"
        )) {
            self.hovered.get()
        } else {
            self.is_window_active()
        }
    }

    /// 切换窗口的缩放状态。
    pub fn zoom_window(&self) {
        self.platform_window.zoom();
    }

    /// 打开原生标题栏上下文菜单，在实现客户端装饰时很有用（Wayland 和 X11）
    pub fn show_window_menu(&self, position: Point<Pixels>) {
        self.platform_window.show_window_menu(position)
    }

    /// 处理 Linux 和 macOS 的窗口移动。
    /// 告诉合成器控制窗口移动（Wayland 和 X11）
    ///
    /// 在移动操作期间可能不会接收到事件。
    pub fn start_window_move(&self) {
        self.platform_window.start_window_move()
    }

    /// 使用客户端装饰时，将此设置为不可见装饰的宽度（Wayland 和 X11）
    pub fn set_client_inset(&mut self, inset: Pixels) {
        self.client_inset = Some(inset);
        self.platform_window.set_client_inset(inset);
    }

    /// 返回 [`Self::set_client_inset`] 设置的 client_inset 值。
    pub fn client_inset(&self) -> Option<Pixels> {
        self.client_inset
    }

    /// 返回标题栏窗口控件是否需要由应用程序渲染（Wayland 和 X11）
    pub fn window_decorations(&self) -> Decorations {
        self.platform_window.window_decorations()
    }

    /// 返回此窗口是否可调整大小。
    pub fn is_resizable(&self) -> bool {
        self.is_resizable
    }

    /// 返回此窗口是否可最小化。
    pub fn is_minimizable(&self) -> bool {
        self.is_minimizable
    }

    /// 返回平台支持的控件。
    pub fn window_controls(&self) -> WindowControls {
        self.platform_window.window_controls()
    }

    /// 在平台级别更新窗口的标题。
    pub fn set_window_title(&mut self, title: &str) {
        self.platform_window.set_title(title);
        self.a11y.set_window_title(title.to_string());
    }

    /// 设置 macOS 红绿灯按钮的位置。
    #[cfg(target_os = "macos")]
    pub fn set_traffic_light_position(&self, position: Point<Pixels>) {
        self.platform_window.set_traffic_light_position(position);
    }

    /// 设置应用程序标识符。
    pub fn set_app_id(&mut self, app_id: &str) {
        self.platform_window.set_app_id(app_id);
    }

    /// 设置窗口背景外观。
    pub fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance) {
        self.platform_window
            .set_background_appearance(background_appearance);
    }

    /// 返回窗口背景外观。
    pub fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.platform_window.background_appearance()
    }

    /// 在平台级别将窗口标记为脏。
    pub fn set_window_edited(&mut self, edited: bool) {
        self.platform_window.set_edited(edited);
    }

    /// 设置此窗口代表的文件路径。
    /// 在 macOS 上，这设置窗口的辅助功能文档属性（AXDocument）。
    pub fn set_document_path(&self, path: Option<&std::path::Path>) {
        self.platform_window.set_document_path(path);
    }

    /// 确定窗口可见的显示器。
    pub fn display(&self, cx: &App) -> Option<Rc<dyn PlatformDisplay>> {
        cx.platform
            .displays()
            .into_iter()
            .find(|display| Some(display.id()) == self.display_id)
    }

    /// 显示平台字符调色板。
    pub fn show_character_palette(&self) {
        self.platform_window.show_character_palette();
    }

    /// 与窗口关联的显示器的缩放因子。例如，它可能
    /// 为"Retina"显示器返回 2.0，表示每个逻辑像素实际上
    /// 应渲染为屏幕上的两个像素。
    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    /// 为测试覆盖显示器缩放因子。
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        self.scale_factor = scale_factor;
        self.refresh();
    }

    /// 应用程序基础字体的 em 大小。调整此值允许
    /// UI 缩放，就像缩放网页一样。
    pub fn rem_size(&self) -> Pixels {
        self.rem_size_override_stack
            .last()
            .copied()
            .unwrap_or(self.rem_size)
    }

    /// 设置应用程序基础字体的 em 大小。调整此值允许
    /// UI 缩放，就像缩放网页一样。
    pub fn set_rem_size(&mut self, rem_size: impl Into<Pixels>) {
        self.rem_size = rem_size.into();
    }

    /// 为给定的 ElementId 获取全局唯一标识符。
    /// 仅在提供的闭包持续期间有效。
    pub fn with_global_id<R>(
        &mut self,
        element_id: ElementId,
        f: impl FnOnce(&GlobalElementId, &mut Self) -> R,
    ) -> R {
        self.with_id(element_id, |this| {
            let global_id = GlobalElementId(Arc::from(&*this.element_id_stack));

            f(&global_id, this)
        })
    }

    /// 使用压入栈的元素 ID 调用提供的闭包。
    #[inline]
    pub fn with_id<R>(
        &mut self,
        element_id: impl Into<ElementId>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.element_id_stack.push(element_id.into());
        let result = f(self);
        self.element_id_stack.pop();
        result
    }

    /// 使用指定的 rem 大小执行提供的函数。
    ///
    /// 此方法只能作为元素绘制的一部分来调用。
    // This function is called in a highly recursive manner in editor
    // prepainting, make sure its inlined to reduce the stack burden
    #[inline]
    pub fn with_rem_size<F, R>(&mut self, rem_size: Option<impl Into<Pixels>>, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.invalidator.debug_assert_paint_or_prepaint();

        if let Some(rem_size) = rem_size {
            self.rem_size_override_stack.push(rem_size.into());
            let result = f(self);
            self.rem_size_override_stack.pop();
            result
        } else {
            f(self)
        }
    }

    /// 与当前文本样式关联的行高。
    pub fn line_height(&self) -> Pixels {
        self.text_style().line_height_in_pixels(self.rem_size())
    }

    /// 将逻辑值四舍五入到最近的设备像素。
    #[inline]
    pub fn pixel_snap(&self, value: Pixels) -> Pixels {
        px(round_to_device_pixel(value.0, self.scale_factor()) / self.scale_factor())
    }

    /// [`Self::pixel_snap`] 的 f64 变体。
    #[inline]
    pub fn pixel_snap_f64(&self, value: f64) -> f64 {
        let scale_factor = f64::from(self.scale_factor());
        round_half_toward_zero_f64(value * scale_factor) / scale_factor
    }

    /// 将边界框的原点和大小对齐到最近的设备像素。
    #[inline]
    pub fn pixel_snap_bounds(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        bounds.map(|c| self.pixel_snap(c))
    }

    /// 将点的坐标对齐到最近的设备像素。
    #[inline]
    pub fn pixel_snap_point(&self, position: Point<Pixels>) -> Point<Pixels> {
        position.map(|c| self.pixel_snap(c))
    }

    #[inline]
    fn snap_bounds(&self, bounds: Bounds<Pixels>) -> Bounds<ScaledPixels> {
        let scale_factor = self.scale_factor();
        let left = round_to_device_pixel(bounds.left().0, scale_factor);
        let top = round_to_device_pixel(bounds.top().0, scale_factor);
        let right = round_to_device_pixel(bounds.right().0, scale_factor).max(left);
        let bottom = round_to_device_pixel(bounds.bottom().0, scale_factor).max(top);
        Bounds::from_corners(
            point(ScaledPixels(left), ScaledPixels(top)),
            point(ScaledPixels(right), ScaledPixels(bottom)),
        )
    }

    /// 向零舍入，但将任何非零输入钳制到至少 1 dp，以使细描边不会消失。
    #[inline]
    fn snap_stroke(&self, value: Pixels) -> ScaledPixels {
        ScaledPixels(round_stroke_to_device_pixel(value.0, self.scale_factor()))
    }

    #[inline]
    fn snap_border_widths(&self, edges: Edges<Pixels>) -> Edges<ScaledPixels> {
        edges.map(|e| self.snap_stroke(*e))
    }

    /// 近边缘向下取整，远边缘向上取整，产生原始区域的严格超集。
    #[inline]
    fn cover_bounds(&self, bounds: Bounds<Pixels>) -> Bounds<ScaledPixels> {
        let scale_factor = self.scale_factor();
        let left = floor_to_device_pixel(bounds.left().0, scale_factor);
        let top = floor_to_device_pixel(bounds.top().0, scale_factor);
        let right = ceil_to_device_pixel(bounds.right().0, scale_factor).max(left);
        let bottom = ceil_to_device_pixel(bounds.bottom().0, scale_factor).max(top);
        Bounds::from_corners(
            point(ScaledPixels(left), ScaledPixels(top)),
            point(ScaledPixels(right), ScaledPixels(bottom)),
        )
    }

    #[inline]
    fn snapped_content_mask(&self) -> ContentMask<ScaledPixels> {
        ContentMask {
            bounds: self.cover_bounds(self.content_mask().bounds),
        }
    }

    /// 调用以阻止事件的默认操作。目前仅用于阻止
    /// 父元素在鼠标按下时获得焦点。
    pub fn prevent_default(&mut self) {
        self.default_prevented = true;
    }

    /// 获取当前正在分发的事件的默认行为是否已被阻止。
    pub fn default_prevented(&self) -> bool {
        self.default_prevented
    }

    /// 确定给定操作在当前聚焦元素的分发路径上是否可用。
    pub fn is_action_available(&self, action: &dyn Action, cx: &App) -> bool {
        let node_id =
            self.focus_node_id_in_rendered_frame(self.focused(cx).map(|handle| handle.id));
        self.rendered_frame
            .dispatch_tree
            .is_action_available(action, node_id)
    }

    /// 确定给定操作在给定 focus_handle 的分发路径上是否可用。
    pub fn is_action_available_in(&self, action: &dyn Action, focus_handle: &FocusHandle) -> bool {
        let node_id = self.focus_node_id_in_rendered_frame(Some(focus_handle.id));
        self.rendered_frame
            .dispatch_tree
            .is_action_available(action, node_id)
    }

    /// 鼠标相对于窗口的位置。
    pub fn mouse_position(&self) -> Point<Pixels> {
        self.mouse_position
    }

    /// 为给定的 hitbox 捕获指针。捕获期间，所有鼠标移动和鼠标释放
    /// 事件将路由到检查此 hitbox 的 `is_hovered` 状态的监听器，
    /// 无论实际命中测试如何。这使得拖拽操作可以在
    /// 指针移出元素边界时继续。
    ///
    /// 捕获会在鼠标释放时自动释放。
    pub fn capture_pointer(&mut self, hitbox_id: HitboxId) {
        self.captured_hitbox = Some(hitbox_id);
    }

    /// 释放任何活动的指针捕获。
    pub fn release_pointer(&mut self) {
        self.captured_hitbox = None;
    }

    /// 返回已捕获指针的 hitbox（如果有）。
    pub fn captured_hitbox(&self) -> Option<HitboxId> {
        self.captured_hitbox
    }

    /// 键盘修饰键的当前状态
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    /// 如果最后一个输入事件是基于键盘的（按键、tab 导航等）则返回 true
    /// 用于焦点可见样式，仅为键盘导航显示焦点指示器。
    pub fn last_input_was_keyboard(&self) -> bool {
        self.last_input_modality == InputModality::Keyboard
    }

    /// 键盘大写锁定的当前状态
    pub fn capslock(&self) -> Capslock {
        self.capslock
    }

    fn complete_frame(&self) {
        self.platform_window.completed_frame();
    }

    /// 生成新帧并将其分配给 `rendered_frame`。要实际显示
    /// 新 [`Scene`] 的内容，请使用 [`Self::present`]。
    #[profiling::function]
    pub fn draw(&mut self, cx: &mut App) -> ArenaClearNeeded {
        // Drain unconditionally so a stale first-invalidation timestamp can't
        // leak into a later frame across enable/disable of frame tracing.
        let frame_dirty = self.invalidator.take_frame_dirty();
        let draw_started_at = profiler::frame_trace_enabled().then(Instant::now);

        // Set up the per-App arena for element allocation during this draw.
        // This ensures that multiple test Apps have isolated arenas.
        let arena_scope = ElementArenaScope::enter(&cx.element_arena);

        self.invalidate_entities();
        cx.entities.clear_accessed();
        debug_assert!(self.rendered_entity_stack.is_empty());
        self.invalidator.set_dirty(false);
        self.requested_autoscroll = None;

        // Restore the previously-used input handler.
        // Place it back into a None slot (left by a previous .take()) so that
        // cached paint_range indices in reuse_paint find the handler at the
        // expected position.
        if let Some(input_handler) = self.platform_window.take_input_handler() {
            if let Some(slot) = self
                .rendered_frame
                .input_handlers
                .iter_mut()
                .rev()
                .find(|h| h.is_none())
            {
                *slot = Some(input_handler);
            } else {
                self.rendered_frame.input_handlers.push(Some(input_handler));
            }
        }
        if !cx.mode.skip_drawing() {
            self.draw_roots(cx);
        }
        self.dirty_views.clear();
        self.next_frame.window_active = self.active.get();

        // Register requested input handler with the platform window.
        // Use .take() instead of .pop() to preserve Vec length, so that cached
        // paint_range indices remain valid for reuse_paint on the next frame.
        // Search backwards to find the last Some entry, since reuse_paint may
        // have copied None slots from the previous frame. (Fixes #50456)
        if let Some(input_handler) = self
            .next_frame
            .input_handlers
            .iter_mut()
            .rev()
            .find_map(|h| h.take())
        {
            self.platform_window.set_input_handler(input_handler);
        }

        self.layout_engine.as_mut().unwrap().clear();
        self.text_system().finish_frame();
        self.next_frame.finish(&mut self.rendered_frame);

        self.invalidator.set_phase(DrawPhase::Focus);
        let previous_focus_path = self.rendered_frame.focus_path();
        let previous_window_active = self.rendered_frame.window_active;
        mem::swap(&mut self.rendered_frame, &mut self.next_frame);
        self.next_frame.clear();
        let current_focus_path = self.rendered_frame.focus_path();
        let current_window_active = self.rendered_frame.window_active;
        let mut focus_before_listeners = self.focus;

        if previous_focus_path != current_focus_path
            || previous_window_active != current_window_active
        {
            if !previous_focus_path.is_empty() && current_focus_path.is_empty() {
                self.focus_lost_listeners
                    .clone()
                    .retain(&(), |listener| listener(self, cx));
                // The focus-lost fallback (e.g. a workspace refocusing itself) may target
                // an element that isn't part of the element tree, in which case scheduling
                // a redraw below would dispatch focus-lost again, looping forever. Only
                // track focus movement caused by the focus listeners.
                focus_before_listeners = self.focus;
            }

            let event = WindowFocusEvent {
                previous_focus_path: if previous_window_active {
                    previous_focus_path
                } else {
                    Default::default()
                },
                current_focus_path: if current_window_active {
                    current_focus_path
                } else {
                    Default::default()
                },
            };
            self.focus_listeners
                .clone()
                .retain(&(), |listener| listener(&event, self, cx));
        }

        debug_assert!(self.rendered_entity_stack.is_empty());
        self.record_entities_accessed(cx);
        self.reset_cursor_style(cx);
        self.refreshing = false;
        self.invalidator.set_phase(DrawPhase::None);
        // Focus listeners may move focus (e.g. a dock forwarding focus to its active
        // panel). `Window::focus` suppresses `refresh` while a draw is in progress, so
        // schedule another frame here to render the new focus state and dispatch the
        // resulting focus events.
        if self.focus != focus_before_listeners {
            self.refresh();
        }
        self.needs_present.set(true);

        if let Some(draw_start) = draw_started_at {
            profiler::record_frame_timing(profiler::FrameTiming {
                window_id: self.handle.window_id(),
                dirty_at: frame_dirty.dirty_at,
                invalidations: frame_dirty.invalidations,
                draw_start,
                draw_end: Instant::now(),
            });
        }

        // Exit the scope to obtain the arena-clear token this draw owes; the
        // scope's teardown itself happens in `ElementArenaScope::drop`.
        arena_scope.exit(&cx.element_arena)
    }

    fn record_entities_accessed(&mut self, cx: &mut App) {
        let mut entities_ref = cx.entities.accessed_entities.get_mut();
        let mut entities = mem::take(entities_ref.deref_mut());
        let handle = self.handle;
        cx.record_entities_accessed(
            handle,
            // Try moving window invalidator into the Window
            self.invalidator.clone(),
            &entities,
        );
        let mut entities_ref = cx.entities.accessed_entities.get_mut();
        mem::swap(&mut entities, entities_ref.deref_mut());
    }

    fn invalidate_entities(&mut self) {
        let mut views = self.invalidator.take_views();
        for entity in views.drain() {
            self.mark_view_dirty(entity);
        }
        self.invalidator.replace_views(views);
    }

    #[profiling::function]
    fn present(&mut self) {
        self.platform_window.draw(&self.rendered_frame.scene);
        #[cfg(feature = "input-latency-histogram")]
        self.input_latency_tracker.record_frame_presented();
        self.needs_present.set(false);
        profiling::finish_frame!();
    }

    /// 如果最近绘制的帧尚未呈现，则呈现它。
    ///
    /// 基准测试同步驱动绘制，而不是通过
    /// 帧请求循环，因此它们在每次测量更新后调用此方法，
    /// 就像生产环境呈现一样提交帧。
    #[cfg(feature = "bench")]
    pub fn present_if_needed(&mut self) {
        if self.needs_present.get() {
            self.present();
        }
    }

    /// 返回当前输入延迟直方图的快照。
    #[cfg(feature = "input-latency-histogram")]
    pub fn input_latency_snapshot(&self) -> InputLatencySnapshot {
        self.input_latency_tracker.snapshot()
    }

    fn draw_roots(&mut self, cx: &mut App) {
        self.invalidator.set_phase(DrawPhase::Prepaint);
        self.tooltip_bounds.take();

        self.a11y.sync_active_flag();
        if self.a11y.is_active() {
            self.a11y.begin_frame();
        }

        // DOM 后端：开始本帧的 DOM 树收集（仅当平台窗口声明支持时）。
        #[cfg(feature = "dom-backend")]
        if let Some(builder) = &mut self.dom_builder {
            builder.begin_frame();
        }

        let _inspector_width: Pixels = rems(30.0).to_pixels(self.rem_size());
        let root_size = {
            #[cfg(any(feature = "inspector", debug_assertions))]
            {
                if self.inspector.is_some() {
                    let mut size = self.viewport_size;
                    size.width = (size.width - _inspector_width).max(px(0.0));
                    size
                } else {
                    self.viewport_size
                }
            }
            #[cfg(not(any(feature = "inspector", debug_assertions)))]
            {
                self.viewport_size
            }
        };

        // Layout all root elements. Like the root element on the web, which
        // stretches to fill the viewport unless explicitly sized, window roots
        // fill the window when their size is `auto`.
        let scale_factor = self.scale_factor();
        let mut root_element = self.root.as_ref().unwrap().clone().into_any_element();
        let root_layout_id = root_element.request_layout(self, cx);
        self.layout_engine
            .as_mut()
            .unwrap()
            .stretch_auto_size_to_fill(root_layout_id, root_size, scale_factor);
        root_element.prepaint_as_root(Point::default(), root_size.into(), self, cx);

        #[cfg(any(feature = "inspector", debug_assertions))]
        let inspector_element = self.prepaint_inspector(_inspector_width, cx);

        self.prepaint_deferred_draws(cx);

        let mut prompt_element = None;
        let mut active_drag_element = None;
        let mut tooltip_element = None;
        if let Some(prompt) = self.prompt.take() {
            let mut element = prompt.view.any_view().into_any_element();
            let prompt_layout_id = element.request_layout(self, cx);
            self.layout_engine
                .as_mut()
                .unwrap()
                .stretch_auto_size_to_fill(prompt_layout_id, root_size, scale_factor);
            element.prepaint_as_root(Point::default(), root_size.into(), self, cx);
            prompt_element = Some(element);
            self.prompt = Some(prompt);
        } else if let Some(active_drag) = cx.active_drag.take() {
            let mut element = active_drag.view.clone().into_any_element();
            let offset = self.mouse_position() - active_drag.cursor_offset;
            element.prepaint_as_root(offset, AvailableSpace::min_size(), self, cx);
            active_drag_element = Some(element);
            cx.active_drag = Some(active_drag);
        } else {
            tooltip_element = self.prepaint_tooltip(cx);
        }

        self.mouse_hit_test = self.next_frame.hit_test(self.mouse_position);

        // Now actually paint the elements.
        self.invalidator.set_phase(DrawPhase::Paint);
        root_element.paint(self, cx);

        #[cfg(any(feature = "inspector", debug_assertions))]
        self.paint_inspector(inspector_element, cx);

        self.paint_deferred_draws(cx);

        if let Some(mut prompt_element) = prompt_element {
            prompt_element.paint(self, cx);
        } else if let Some(mut drag_element) = active_drag_element {
            drag_element.paint(self, cx);
        } else if let Some(mut tooltip_element) = tooltip_element {
            tooltip_element.paint(self, cx);
        }

        #[cfg(any(feature = "inspector", debug_assertions))]
        self.paint_inspector_hitbox(cx);

        // DOM 后端：结束本帧的 DOM 树收集，并把新鲜树交付给平台窗口。
        #[cfg(feature = "dom-backend")]
        if let Some(builder) = &mut self.dom_builder {
            static DOM_FRAME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let f = DOM_FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if f.is_multiple_of(20) {
                log::info!("DOM_FRAME {}", f);
            }
            let tree = builder.finish();
            self.platform_window.dom_tree_update(&tree);
        }

        // a11y may have been activated/deactivated halfway through the frame
        let a11y_active_start_of_frame = self.a11y.is_active();
        self.a11y.sync_active_flag();
        let a11y_active_end_of_frame = self.a11y.is_active();

        let should_send_a11y_update = a11y_active_start_of_frame && a11y_active_end_of_frame;

        if a11y_active_start_of_frame {
            // Harvest frame metadata for the debug dump while the live window
            // and frame are still in scope.
            let frame_info = crate::window::a11y::debug::FrameDebugInfo {
                viewport_size: self.viewport_size,
                scale_factor: self.scale_factor,
                tab_stop_count: self.next_frame.tab_stops.tab_stop_count(),
            };
            // clear the builder state regardless
            let tree_update = self.a11y.end_frame(frame_info);

            if should_send_a11y_update {
                log::debug!(
                    "Sending a11y tree update: {} nodes",
                    tree_update.nodes.len()
                );
                self.platform_window.a11y_tree_update(tree_update);
            }
        }
    }

    /// DOM 后端是否启用（即平台窗口声明支持 DOM 层）。
    #[cfg(feature = "dom-backend")]
    pub(crate) fn dom_builder_active(&self) -> bool {
        self.dom_builder.is_some()
    }

    /// 登记一个 DOM 节点并返回其跨帧稳定 key，同时把该 key 压入 DOM 父链。
    ///
    /// 由 [`crate::element::Drawable`] 在 paint 前后调用：
    /// - `node` 为 `None`（元素不参与 DOM 映射）时不登记、不压栈；
    /// - `is_keyed` 表示元素是否带 `.id()`（决定匿名兄弟序号归零）。
    ///
    /// 路径取当前 `element_id_stack`（此时已压入本元素自身的 id）。
    #[cfg(feature = "dom-backend")]
    pub(crate) fn dom_element(
        &mut self,
        node: Option<crate::DomNode>,
        is_keyed: bool,
    ) -> Option<crate::DomNodeKey> {
        let builder = self.dom_builder.as_mut()?;
        let node = node?;
        let path = &*self.element_id_stack;
        // 在 `register` 消费 `node` 之前取出可滚动容器的 `ScrollHandle`（若有）。
        #[cfg(feature = "dom-backend")]
        let scroll_handle = node.scroll_handle.clone();
        let key = builder.register(node, is_keyed, path);
        // DOM 模式：把可滚动容器的 `ScrollHandle` 登记进反查表，供浏览器原生滚动时
        // 由 `dispatch_dom_scroll` 反查并更新滚动偏移。
        #[cfg(feature = "dom-backend")]
        if let Some(handle) = scroll_handle {
            self.next_frame
                .dom_scroll_handles
                .insert(key.clone(), handle);
        }
        Some(key)
    }

    /// 结束当前元素的 DOM 栈帧（与 [`Self::dom_element`] 配对）。
    #[cfg(feature = "dom-backend")]
    pub(crate) fn dom_exit(&mut self) {
        if let Some(builder) = &mut self.dom_builder {
            builder.exit();
        }
    }

    /// 当前正在 prepaint 的元素的 DOM key（无 DOM 层或栈为空时为 `None`）。
    ///
    /// 由 `insert_hitbox` 使用，把 hitbox 关联到当前元素的 DOM key。
    /// 优先使用 `element_id_stack` 反查（精确，不受 builder 栈偏移影响），
    /// 回退到 builder 栈顶（快速路径）。
    #[cfg(feature = "dom-backend")]
    fn current_dom_key(&self) -> Option<crate::DomNodeKey> {
        let builder = self.dom_builder.as_ref()?;
        // 精确路径：通过 element_id_stack 反查（适用于 deferred/overlay 绘制，
        // 此时 builder 栈可能已偏移到祖先 key）。
        if let Some(key) = builder.key_for_element_id_stack(&self.element_id_stack) {
            return Some(key);
        }
        // 快速路径：builder 栈顶（正常 inline 绘制时栈与 element_id_stack 同步）。
        if builder.stack_len() == 0 {
            None
        } else {
            Some(builder.current_parent())
        }
    }

    fn prepaint_tooltip(&mut self, cx: &mut App) -> Option<AnyElement> {
        // Use indexing instead of iteration to avoid borrowing self for the duration of the loop.
        for tooltip_request_index in (0..self.next_frame.tooltip_requests.len()).rev() {
            let Some(Some(tooltip_request)) = self
                .next_frame
                .tooltip_requests
                .get(tooltip_request_index)
                .cloned()
            else {
                log::error!("Unexpectedly absent TooltipRequest");
                continue;
            };
            let mut element = tooltip_request.tooltip.view.clone().into_any_element();
            let mouse_position = tooltip_request.tooltip.mouse_position;
            let tooltip_size = element.layout_as_root(AvailableSpace::min_size(), self, cx);

            let mut tooltip_bounds =
                Bounds::new(mouse_position + point(px(1.), px(1.)), tooltip_size);
            let window_bounds = Bounds {
                origin: Point::default(),
                size: self.viewport_size(),
            };

            if tooltip_bounds.right() > window_bounds.right() {
                let new_x = mouse_position.x - tooltip_bounds.size.width - px(1.);
                if new_x >= Pixels::ZERO {
                    tooltip_bounds.origin.x = new_x;
                } else {
                    tooltip_bounds.origin.x = cmp::max(
                        Pixels::ZERO,
                        tooltip_bounds.origin.x - tooltip_bounds.right() - window_bounds.right(),
                    );
                }
            }

            if tooltip_bounds.bottom() > window_bounds.bottom() {
                let new_y = mouse_position.y - tooltip_bounds.size.height - px(1.);
                if new_y >= Pixels::ZERO {
                    tooltip_bounds.origin.y = new_y;
                } else {
                    tooltip_bounds.origin.y = cmp::max(
                        Pixels::ZERO,
                        tooltip_bounds.origin.y - tooltip_bounds.bottom() - window_bounds.bottom(),
                    );
                }
            }

            // It's possible for an element to have an active tooltip while not being painted (e.g.
            // via the `visible_on_hover` method). Since mouse listeners are not active in this
            // case, instead update the tooltip's visibility here.
            let is_visible =
                (tooltip_request.tooltip.check_visible_and_update)(tooltip_bounds, self, cx);
            if !is_visible {
                continue;
            }

            self.with_absolute_element_offset(tooltip_bounds.origin, |window| {
                element.prepaint(window, cx)
            });

            self.tooltip_bounds = Some(TooltipBounds {
                id: tooltip_request.id,
                bounds: tooltip_bounds,
            });
            return Some(element);
        }
        None
    }

    fn prepaint_deferred_draws(&mut self, cx: &mut App) {
        assert_eq!(self.element_id_stack.len(), 0);

        // Process deferred draws in multiple rounds to support nesting.
        // Each round processes all current deferred draws, which may push new ones.
        //
        // The draws are processed in place rather than being moved out of
        // `next_frame.deferred_draws`: `prepaint_index` snapshots that vector's
        // length, so any prepaint range recorded during a round (view caches,
        // nested deferred draws) must index the same vector `reuse_prepaint`
        // slices on the next frame. Moving the draws out and re-appending them
        // shifts the indices of nested draws, causing reused subtrees to graft
        // the wrong deferred draws and panic in the dispatch tree.
        let mut round_start = 0;
        let mut depth = 0;
        loop {
            let round_end = self.next_frame.deferred_draws.len();
            if round_start == round_end {
                break;
            }
            // Limit maximum nesting depth to prevent infinite loops.
            assert!(depth < 10, "Exceeded maximum (10) deferred depth");
            depth += 1;

            // Sort this round by priority.
            let mut traversal_order = (round_start..round_end).collect::<SmallVec<[usize; 8]>>();
            traversal_order.sort_by_key(|ix| self.next_frame.deferred_draws[*ix].priority);

            for deferred_draw_ix in traversal_order {
                let (element, parent_node, current_view, rem_size, absolute_offset, prepaint_range) = {
                    let deferred_draw = &mut self.next_frame.deferred_draws[deferred_draw_ix];
                    self.element_id_stack
                        .clone_from(&deferred_draw.element_id_stack);
                    self.text_style_stack
                        .clone_from(&deferred_draw.text_style_stack);
                    (
                        deferred_draw.element.take(),
                        deferred_draw.parent_node,
                        deferred_draw.current_view,
                        deferred_draw.rem_size,
                        deferred_draw.absolute_offset,
                        deferred_draw.prepaint_range.clone(),
                    )
                };
                self.next_frame.dispatch_tree.set_active_node(parent_node);

                let prepaint_start = self.prepaint_index();
                if let Some(mut element) = element {
                    self.with_rendered_view(current_view, |window| {
                        window.with_rem_size(Some(rem_size), |window| {
                            window.with_absolute_element_offset(absolute_offset, |window| {
                                element.prepaint(window, cx);
                            });
                        });
                    });
                    self.next_frame.deferred_draws[deferred_draw_ix].element = Some(element);
                } else {
                    self.reuse_prepaint(prepaint_range);
                }
                let prepaint_end = self.prepaint_index();
                self.next_frame.deferred_draws[deferred_draw_ix].prepaint_range =
                    prepaint_start..prepaint_end;
            }

            self.element_id_stack.clear();
            self.text_style_stack.clear();
            round_start = round_end;
        }
    }

    fn paint_deferred_draws(&mut self, cx: &mut App) {
        assert_eq!(self.element_id_stack.len(), 0);

        // Paint all deferred draws in priority order.
        // Since prepaint has already processed nested deferreds, we just paint them all.
        if self.next_frame.deferred_draws.len() == 0 {
            return;
        }

        let traversal_order = self.deferred_draw_traversal_order();
        let mut deferred_draws = mem::take(&mut self.next_frame.deferred_draws);
        for deferred_draw_ix in traversal_order {
            let mut deferred_draw = &mut deferred_draws[deferred_draw_ix];
            self.element_id_stack
                .clone_from(&deferred_draw.element_id_stack);
            self.next_frame
                .dispatch_tree
                .set_active_node(deferred_draw.parent_node);

            let paint_start = self.paint_index();
            let content_mask = deferred_draw.content_mask;
            if let Some(element) = deferred_draw.element.as_mut() {
                self.with_rendered_view(deferred_draw.current_view, |window| {
                    window.with_content_mask(content_mask, |window| {
                        window.with_rem_size(Some(deferred_draw.rem_size), |window| {
                            element.paint(window, cx);
                        });
                    })
                })
            } else {
                self.reuse_paint(deferred_draw.paint_range.clone());
            }
            let paint_end = self.paint_index();
            deferred_draw.paint_range = paint_start..paint_end;
        }
        self.next_frame.deferred_draws = deferred_draws;
        self.element_id_stack.clear();
    }

    fn deferred_draw_traversal_order(&mut self) -> SmallVec<[usize; 8]> {
        let deferred_count = self.next_frame.deferred_draws.len();
        let mut sorted_indices = (0..deferred_count).collect::<SmallVec<[_; 8]>>();
        sorted_indices.sort_by_key(|ix| self.next_frame.deferred_draws[*ix].priority);
        sorted_indices
    }

    pub(crate) fn prepaint_index(&self) -> PrepaintStateIndex {
        PrepaintStateIndex {
            hitboxes_index: self.next_frame.hitboxes.len(),
            tooltips_index: self.next_frame.tooltip_requests.len(),
            deferred_draws_index: self.next_frame.deferred_draws.len(),
            dispatch_tree_index: self.next_frame.dispatch_tree.len(),
            accessed_element_states_index: self.next_frame.accessed_element_states.len(),
            line_layout_index: self.text_system.layout_index(),
        }
    }

    pub(crate) fn reuse_prepaint(&mut self, range: Range<PrepaintStateIndex>) {
        self.next_frame.hitboxes.extend(
            self.rendered_frame.hitboxes[range.start.hitboxes_index..range.end.hitboxes_index]
                .iter()
                .cloned(),
        );
        self.next_frame.tooltip_requests.extend(
            self.rendered_frame.tooltip_requests
                [range.start.tooltips_index..range.end.tooltips_index]
                .iter_mut()
                .map(|request| request.take()),
        );
        self.next_frame.accessed_element_states.extend(
            self.rendered_frame.accessed_element_states[range.start.accessed_element_states_index
                ..range.end.accessed_element_states_index]
                .iter()
                .map(|(id, type_id)| (id.clone(), *type_id)),
        );
        self.text_system
            .reuse_layouts(range.start.line_layout_index..range.end.line_layout_index);

        let reused_subtree = self.next_frame.dispatch_tree.reuse_subtree(
            range.start.dispatch_tree_index..range.end.dispatch_tree_index,
            &mut self.rendered_frame.dispatch_tree,
            self.focus,
        );

        if reused_subtree.contains_focus() {
            self.next_frame.focus = self.focus;
        }

        self.next_frame.deferred_draws.extend(
            self.rendered_frame.deferred_draws
                [range.start.deferred_draws_index..range.end.deferred_draws_index]
                .iter()
                .map(|deferred_draw| DeferredDraw {
                    current_view: deferred_draw.current_view,
                    parent_node: reused_subtree.refresh_node_id(deferred_draw.parent_node),
                    element_id_stack: deferred_draw.element_id_stack.clone(),
                    text_style_stack: deferred_draw.text_style_stack.clone(),
                    content_mask: deferred_draw.content_mask,
                    rem_size: deferred_draw.rem_size,
                    priority: deferred_draw.priority,
                    element: None,
                    absolute_offset: deferred_draw.absolute_offset,
                    prepaint_range: deferred_draw.prepaint_range.clone(),
                    paint_range: deferred_draw.paint_range.clone(),
                }),
        );
    }

    pub(crate) fn paint_index(&self) -> PaintIndex {
        PaintIndex {
            scene_index: self.next_frame.scene.len(),
            mouse_listeners_index: self.next_frame.mouse_listeners.len(),
            input_handlers_index: self.next_frame.input_handlers.len(),
            cursor_styles_index: self.next_frame.cursor_styles.len(),
            accessed_element_states_index: self.next_frame.accessed_element_states.len(),
            tab_handle_index: self.next_frame.tab_stops.paint_index(),
            line_layout_index: self.text_system.layout_index(),
        }
    }

    pub(crate) fn reuse_paint(&mut self, range: Range<PaintIndex>) {
        self.next_frame.cursor_styles.extend(
            self.rendered_frame.cursor_styles
                [range.start.cursor_styles_index..range.end.cursor_styles_index]
                .iter()
                .cloned(),
        );
        self.next_frame.input_handlers.extend(
            self.rendered_frame.input_handlers
                [range.start.input_handlers_index..range.end.input_handlers_index]
                .iter_mut()
                .map(|handler| handler.take()),
        );
        self.next_frame.mouse_listeners.extend(
            self.rendered_frame.mouse_listeners
                [range.start.mouse_listeners_index..range.end.mouse_listeners_index]
                .iter_mut()
                .map(|listener| listener.take()),
        );
        self.next_frame.accessed_element_states.extend(
            self.rendered_frame.accessed_element_states[range.start.accessed_element_states_index
                ..range.end.accessed_element_states_index]
                .iter()
                .map(|(id, type_id)| (id.clone(), *type_id)),
        );
        self.next_frame.tab_stops.replay(
            &self.rendered_frame.tab_stops.insertion_history
                [range.start.tab_handle_index..range.end.tab_handle_index],
        );

        self.text_system
            .reuse_layouts(range.start.line_layout_index..range.end.line_layout_index);
        self.next_frame.scene.replay(
            range.start.scene_index..range.end.scene_index,
            &self.rendered_frame.scene,
        );
    }

    /// 将文本样式压入栈，并在该样式激活时调用函数。
    /// 使用 [`Window::text_style`] 获取当前组合的文本样式。此方法
    /// 只能作为元素绘制的一部分来调用。
    pub fn with_text_style<F, R>(&mut self, style: Option<TextStyleRefinement>, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.invalidator.debug_assert_paint_or_prepaint();
        if let Some(style) = style {
            self.text_style_stack.push(style);
            let result = f(self);
            self.text_style_stack.pop();
            result
        } else {
            f(self)
        }
    }

    /// 在平台级别更新光标样式。此方法只能
    /// 在元素绘制的绘制阶段调用。
    pub fn set_cursor_style(&mut self, style: CursorStyle, hitbox: &Hitbox) {
        self.invalidator.debug_assert_paint();
        self.next_frame.cursor_styles.push(CursorStyleRequest {
            hitbox_id: Some(hitbox.id),
            style,
        });
    }

    /// 在平台级别更新整个窗口的光标样式。使用此方法设置的
    /// 光标样式将优先于使用 `set_cursor_style` 设置的任何光标样式。
    /// 此方法只能在元素绘制的
    /// 绘制阶段调用。
    pub fn set_window_cursor_style(&mut self, style: CursorStyle) {
        self.invalidator.debug_assert_paint();
        self.next_frame.cursor_styles.push(CursorStyleRequest {
            hitbox_id: None,
            style,
        })
    }

    /// 设置要在下一帧渲染的工具提示。此方法只能
    /// 在元素绘制的绘制阶段调用。
    pub fn set_tooltip(&mut self, tooltip: AnyTooltip) -> TooltipId {
        self.invalidator.debug_assert_prepaint();
        let id = TooltipId(post_inc(&mut self.next_tooltip_id.0));
        self.next_frame
            .tooltip_requests
            .push(Some(TooltipRequest { id, tooltip }));
        id
    }

    /// 与当前遮罩取交集后，使用给定的内容遮罩调用给定函数。
    /// 此方法只能在元素绘制期间调用。
    // This function is called in a highly recursive manner in editor
    // prepainting, make sure its inlined to reduce the stack burden
    #[inline]
    pub fn with_content_mask<R>(
        &mut self,
        mask: Option<ContentMask<Pixels>>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint_or_prepaint();
        if let Some(mask) = mask {
            let mask = mask.intersect(&self.content_mask());
            self.content_mask_stack.push(mask);
            let result = f(self);
            self.content_mask_stack.pop();
            result
        } else {
            f(self)
        }
    }

    /// 更新相对于当前偏移量的全局元素偏移量。用于实现
    /// 滚动。此方法只能在元素绘制的预绘制阶段调用。
    pub fn with_element_offset<R>(
        &mut self,
        offset: Point<Pixels>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_prepaint();

        if offset.is_zero() {
            return f(self);
        };

        let abs_offset = self.element_offset() + offset;
        self.with_absolute_element_offset(abs_offset, f)
    }

    /// 根据给定偏移量更新全局元素偏移量。用于实现
    /// 拖拽手柄和其他元素的手动绘制。此方法只能在
    /// 元素绘制的预绘制阶段调用。
    pub fn with_absolute_element_offset<R>(
        &mut self,
        offset: Point<Pixels>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_prepaint();
        self.element_offset_stack.push(offset);
        let result = f(self);
        self.element_offset_stack.pop();
        result
    }

    pub(crate) fn with_element_opacity<R>(
        &mut self,
        opacity: Option<f32>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_paint_or_prepaint();

        let Some(opacity) = opacity else {
            return f(self);
        };

        let previous_opacity = self.element_opacity;
        self.element_opacity = previous_opacity * opacity;
        let result = f(self);
        self.element_opacity = previous_opacity;
        result
    }

    /// 以"可重试"方式对子元素执行预绘制，以便任何
    /// 预绘制的副作用可以在再次预绘制之前丢弃。用于支持自动滚动，
    /// 我们需要预绘制子元素以检测自动滚动边界，然后调整
    /// 元素偏移量并再次预绘制。参见 [`crate::List`] 获取示例。此方法只能
    /// 在元素绘制的预绘制阶段调用。
    pub fn transact<T, U>(&mut self, f: impl FnOnce(&mut Self) -> Result<T, U>) -> Result<T, U> {
        self.invalidator.debug_assert_prepaint();
        let index = self.prepaint_index();
        let result = f(self);
        if result.is_err() {
            self.next_frame.hitboxes.truncate(index.hitboxes_index);
            self.next_frame
                .tooltip_requests
                .truncate(index.tooltips_index);
            self.next_frame
                .deferred_draws
                .truncate(index.deferred_draws_index);
            self.next_frame
                .dispatch_tree
                .truncate(index.dispatch_tree_index);
            self.next_frame
                .accessed_element_states
                .truncate(index.accessed_element_states_index);
            self.text_system.truncate_layouts(index.line_layout_index);
        }
        result
    }

    /// 当你在 [`Element::prepaint`] 期间调用此方法时，包含元素将尝试
    /// 滚动以使指定边界可见。当它们决定自动滚动时，它们会用一组
    /// 新的边界再次调用 [`Element::prepaint`]。参见 [`crate::List`] 获取支持
    /// 在其包含的元素上调用此方法的元素示例。此方法只能
    /// 在元素绘制的预绘制阶段调用。
    pub fn request_autoscroll(&mut self, bounds: Bounds<Pixels>) {
        self.invalidator.debug_assert_prepaint();
        self.requested_autoscroll = Some(bounds);
    }

    /// 此方法可以从包含元素（如 [`crate::List`]）调用，以支持
    /// [`Self::request_autoscroll`] 中描述的自动滚动行为。
    pub fn take_autoscroll(&mut self) -> Option<Bounds<Pixels>> {
        self.invalidator.debug_assert_prepaint();
        self.requested_autoscroll.take()
    }

    /// 异步加载资源，如果资源尚未完成加载则返回 None。
    /// 资源完成加载后，你的视图将被重绘。
    ///
    /// 注意，多次调用此方法只会产生一次 `Asset::load` 调用
    /// 时间。
    pub fn use_asset<A: Asset>(&mut self, source: &A::Source, cx: &mut App) -> Option<A::Output> {
        let (task, is_first) = cx.fetch_asset::<A>(source);
        task.clone().now_or_never().or_else(|| {
            if is_first {
                let entity_id = self.current_view();
                self.spawn(cx, {
                    let task = task.clone();
                    async move |cx| {
                        task.await;

                        cx.on_next_frame(move |_, cx| {
                            cx.notify(entity_id);
                        });
                    }
                })
                .detach();
            }

            None
        })
    }

    /// 异步加载资源，如果资源尚未完成加载或不存在则返回 None。
    /// 资源完成加载后，你的视图不会被重绘。
    ///
    /// 注意，多次调用此方法只会产生一次 `Asset::load` 调用
    /// 时间。
    pub fn get_asset<A: Asset>(&mut self, source: &A::Source, cx: &mut App) -> Option<A::Output> {
        let (task, _) = cx.fetch_asset::<A>(source);
        task.now_or_never()
    }
    /// 获取当前元素偏移量。此方法只能在
    /// 元素绘制的预绘制阶段调用。
    pub fn element_offset(&self) -> Point<Pixels> {
        self.invalidator.debug_assert_prepaint();
        self.element_offset_stack
            .last()
            .copied()
            .unwrap_or_default()
    }

    /// 获取当前元素不透明度。此方法只能在
    /// 元素绘制的预绘制阶段调用。
    #[inline]
    pub(crate) fn element_opacity(&self) -> f32 {
        self.invalidator.debug_assert_paint_or_prepaint();
        self.element_opacity
    }

    /// 获取当前内容遮罩。此方法只能在元素绘制期间调用。
    pub fn content_mask(&self) -> ContentMask<Pixels> {
        self.invalidator.debug_assert_paint_or_prepaint();
        self.content_mask_stack
            .last()
            .cloned()
            .unwrap_or_else(|| ContentMask {
                bounds: Bounds {
                    origin: Point::default(),
                    size: self.viewport_size,
                },
            })
    }

    /// 为被调用函数中的元素提供新的命名空间，其标识符必须唯一。
    /// 可在自定义元素中使用，以区分多组子元素。
    pub fn with_element_namespace<R>(
        &mut self,
        element_id: impl Into<ElementId>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.element_id_stack.push(element_id.into());
        let result = f(self);
        self.element_id_stack.pop();
        result
    }

    /// 使用一个只要此元素在连续帧中被渲染就存在的状态。
    pub fn use_keyed_state<S: 'static>(
        &mut self,
        key: impl Into<ElementId>,
        cx: &mut App,
        init: impl FnOnce(&mut Self, &mut Context<S>) -> S,
    ) -> Entity<S> {
        let current_view = self.current_view();
        self.with_global_id(key.into(), |global_id, window| {
            window.with_element_state(global_id, |state: Option<Entity<S>>, window| {
                if let Some(state) = state {
                    (state.clone(), state)
                } else {
                    let new_state = cx.new(|cx| init(window, cx));
                    cx.observe(&new_state, move |_, cx| {
                        cx.notify(current_view);
                    })
                    .detach();
                    (new_state.clone(), new_state)
                }
            })
        })
    }

    /// 使用一个只要此元素在连续帧中被渲染就存在的状态，无需指定键
    ///
    /// 注意：此方法使用调用者的位置为此状态生成 ID。
    ///       如果此 ID 不足以标识你的状态（例如你正在渲染列表项），
    ///       可以使用 `use_keyed_state` 方法提供自定义 ElementID。
    #[track_caller]
    pub fn use_state<S: 'static>(
        &mut self,
        cx: &mut App,
        init: impl FnOnce(&mut Self, &mut Context<S>) -> S,
    ) -> Entity<S> {
        self.use_keyed_state(
            ElementId::CodeLocation(*core::panic::Location::caller()),
            cx,
            init,
        )
    }

    /// 更新或初始化具有给定 ID 的元素状态，该状态跨多个
    /// 帧存在。如果渲染帧中存在具有此 ID 的元素，其状态将被传递
    /// 给闭包。闭包返回的状态将被存储，以便在
    /// 绘制下一帧时引用。此方法只能作为元素绘制的一部分调用。
    pub fn with_element_state<S, R>(
        &mut self,
        global_id: &GlobalElementId,
        f: impl FnOnce(Option<S>, &mut Self) -> (R, S),
    ) -> R
    where
        S: 'static,
    {
        self.invalidator.debug_assert_paint_or_prepaint();

        let key = (global_id.clone(), TypeId::of::<S>());
        self.next_frame.accessed_element_states.push(key.clone());

        if let Some(any) = self
            .next_frame
            .element_states
            .remove(&key)
            .or_else(|| self.rendered_frame.element_states.remove(&key))
        {
            let ElementStateBox {
                inner,
                #[cfg(debug_assertions)]
                type_name,
            } = any;
            // Using the extra inner option to avoid needing to reallocate a new box.
            let mut state_box = inner
                .downcast::<Option<S>>()
                .map_err(|_| {
                    #[cfg(debug_assertions)]
                    {
                        anyhow::anyhow!(
                            "invalid element state type for id, requested {:?}, actual: {:?}",
                            std::any::type_name::<S>(),
                            type_name
                        )
                    }

                    #[cfg(not(debug_assertions))]
                    {
                        anyhow::anyhow!(
                            "invalid element state type for id, requested {:?}",
                            std::any::type_name::<S>(),
                        )
                    }
                })
                .unwrap();

            let state = state_box.take().expect(
                "reentrant call to with_element_state for the same state type and element id",
            );
            let (result, state) = f(Some(state), self);
            state_box.replace(state);
            self.next_frame.element_states.insert(
                key,
                ElementStateBox {
                    inner: state_box,
                    #[cfg(debug_assertions)]
                    type_name,
                },
            );
            result
        } else {
            let (result, state) = f(None, self);
            self.next_frame.element_states.insert(
                key,
                ElementStateBox {
                    inner: Box::new(Some(state)),
                    #[cfg(debug_assertions)]
                    type_name: std::any::type_name::<S>(),
                },
            );
            result
        }
    }

    /// `with_element_state` 的变体，允许元素的 id 为可选。这是一个
    /// 用于元素 id 可能被分配也可能不被分配的元素的便捷方法。优先使用 `with_element_state`
    /// 当元素保证有 id 时。
    ///
    /// 第一个选项表示"未提供 ID"
    /// 第二个选项表示"尚未初始化"
    pub fn with_optional_element_state<S, R>(
        &mut self,
        global_id: Option<&GlobalElementId>,
        f: impl FnOnce(Option<Option<S>>, &mut Self) -> (R, Option<S>),
    ) -> R
    where
        S: 'static,
    {
        self.invalidator.debug_assert_paint_or_prepaint();

        if let Some(global_id) = global_id {
            self.with_element_state(global_id, |state, cx| {
                let (result, state) = f(Some(state), cx);
                let state =
                    state.expect("you must return some state when you pass some element id");
                (result, state)
            })
        } else {
            let (result, state) = f(None, self);
            debug_assert!(
                state.is_none(),
                "you must not return an element state when passing None for the global id"
            );
            result
        }
    }

    /// 在 tab 组上下文中执行给定闭包。
    #[inline]
    pub fn with_tab_group<R>(&mut self, index: Option<isize>, f: impl FnOnce(&mut Self) -> R) -> R {
        if let Some(index) = index {
            self.next_frame.tab_stops.begin_group(index);
            let result = f(self);
            self.next_frame.tab_stops.end_group();
            result
        } else {
            f(self)
        }
    }

    /// 延迟绘制给定元素，安排其绘制在当前绘制的树之上
    /// 在稍后时间。`priority` 参数决定相对于其他延迟元素的绘制顺序，
    /// 较高的值绘制在上面。
    ///
    /// 当提供 `content_mask` 时，延迟元素将在
    /// 预绘制和绘制期间被裁剪到该区域。为 `None` 时不应用额外裁剪。
    ///
    /// 此方法只能作为元素绘制的预绘制阶段的一部分调用。
    pub fn defer_draw(
        &mut self,
        element: AnyElement,
        absolute_offset: Point<Pixels>,
        priority: usize,
        content_mask: Option<ContentMask<Pixels>>,
    ) {
        self.invalidator.debug_assert_prepaint();
        let parent_node = self.next_frame.dispatch_tree.active_node_id().unwrap();
        self.next_frame.deferred_draws.push(DeferredDraw {
            current_view: self.current_view(),
            parent_node,
            element_id_stack: self.element_id_stack.clone(),
            text_style_stack: self.text_style_stack.clone(),
            content_mask,
            rem_size: self.rem_size(),
            priority,
            element: Some(element),
            absolute_offset,
            prepaint_range: PrepaintStateIndex::default()..PrepaintStateIndex::default(),
            paint_range: PaintIndex::default()..PaintIndex::default(),
        });
    }

    /// 为指定边界创建新的绘制层。"图层"是一批
    /// 不重叠且具有相同绘制顺序的几何图形。通常出于
    /// 性能原因使用。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    pub fn paint_layer<R>(&mut self, bounds: Bounds<Pixels>, f: impl FnOnce(&mut Self) -> R) -> R {
        self.invalidator.debug_assert_paint();

        let content_mask = self.content_mask();
        let clipped_bounds = bounds.intersect(&content_mask.bounds);
        if !clipped_bounds.is_empty() {
            self.next_frame
                .scene
                .push_layer(self.cover_bounds(clipped_bounds));
        }

        let result = f(self);

        if !clipped_bounds.is_empty() {
            self.next_frame.scene.pop_layer();
        }

        result
    }

    /// 将 `shadows` 中的外阴影（非内嵌）绘制到当前
    /// z-index 的场景中。内嵌阴影会被跳过；使用 [`Self::paint_inset_shadows`] 绘制
    /// 在元素背景之后绘制，使其叠加在填充之上。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    pub fn paint_drop_shadows(
        &mut self,
        bounds: Bounds<Pixels>,
        corner_radii: Corners<Pixels>,
        shadows: &[BoxShadow],
    ) {
        self.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let content_mask = self.snapped_content_mask();
        let opacity = self.element_opacity();
        let element_bounds = self.cover_bounds(bounds);
        let element_corner_radii = corner_radii.scale(scale_factor);
        for shadow in shadows {
            if shadow.inset {
                continue;
            }
            let shadow_bounds = (bounds + shadow.offset).dilate(shadow.spread_radius);
            self.next_frame.scene.insert_primitive(Shadow {
                order: 0,
                blur_radius: shadow.blur_radius.scale(scale_factor),
                bounds: self.cover_bounds(shadow_bounds),
                content_mask,
                corner_radii: corner_radii.scale(scale_factor),
                color: shadow.color.opacity(opacity),
                element_bounds,
                element_corner_radii,
                inset: 0,
                pad: 0,
            });
        }
    }

    /// 将 `shadows` 中的内嵌阴影绘制到当前 z-index 的场景中。应在
    /// 元素背景之后调用，使阴影叠加在填充之上。
    /// 外阴影会被跳过；使用 [`Self::paint_drop_shadows`] 在背景之前绘制。
    pub fn paint_inset_shadows(
        &mut self,
        bounds: Bounds<Pixels>,
        corner_radii: Corners<Pixels>,
        shadows: &[BoxShadow],
    ) {
        self.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let content_mask = self.snapped_content_mask();
        let opacity = self.element_opacity();
        let element_bounds = self.cover_bounds(bounds);
        let element_corner_radii = corner_radii.scale(scale_factor);
        for shadow in shadows {
            if !shadow.inset {
                continue;
            }
            let hole = (bounds + shadow.offset).dilate(-shadow.spread_radius);
            // Clamp at zero so a large spread can't produce negative radii, which would
            // break the SDF in the shader.
            let zero = Pixels::ZERO;
            let hole_corner_radii = Corners {
                top_left: (corner_radii.top_left - shadow.spread_radius).max(zero),
                top_right: (corner_radii.top_right - shadow.spread_radius).max(zero),
                bottom_right: (corner_radii.bottom_right - shadow.spread_radius).max(zero),
                bottom_left: (corner_radii.bottom_left - shadow.spread_radius).max(zero),
            };
            self.next_frame.scene.insert_primitive(Shadow {
                order: 0,
                blur_radius: shadow.blur_radius.scale(scale_factor),
                bounds: self.cover_bounds(hole),
                content_mask,
                corner_radii: hole_corner_radii.scale(scale_factor),
                color: shadow.color.opacity(opacity),
                element_bounds,
                element_corner_radii,
                inset: 1,
                pad: 0,
            });
        }
    }

    /// 计算仅含边框的四边形内部的最大空白区域，用于把大片透明内部像素排除在渲染之外。
    fn largest_border_interior(quad: &Quad) -> Bounds<ScaledPixels> {
        let radii = &quad.corner_radii;
        let widths = &quad.border_widths;
        let edge_radii = Edges {
            top: radii.top_left.max(radii.top_right),
            right: radii.top_right.max(radii.bottom_right),
            bottom: radii.bottom_left.max(radii.bottom_right),
            left: radii.top_left.max(radii.bottom_left),
        };

        let antialias_inset = point(ScaledPixels(1.0), ScaledPixels(1.0));
        let inset_bounds = |top_left_inset, bottom_right_inset| {
            Bounds::from_corners(
                quad.bounds.origin + top_left_inset + antialias_inset,
                quad.bounds.bottom_right() - bottom_right_inset - antialias_inset,
            )
        };

        // 圆角只需在一个轴向上排除：两个候选带都为空（不含边框像素），
        // 因此选择面积更大者作为内部区域。
        let horizontal_band = inset_bounds(
            point(widths.left, widths.top.max(edge_radii.top)),
            point(widths.right, widths.bottom.max(edge_radii.bottom)),
        );
        let vertical_band = inset_bounds(
            point(widths.left.max(edge_radii.left), widths.top),
            point(widths.right.max(edge_radii.right), widths.bottom),
        );

        let area = |bounds: &Bounds<ScaledPixels>| {
            bounds.size.width.0.max(0.) * bounds.size.height.0.max(0.)
        };
        if area(&horizontal_band) >= area(&vertical_band) {
            horizontal_band
        } else {
            vertical_band
        }
    }

    /// 在当前层叠上下文中将一个或多个四边形绘制到下一帧的场景中。
    /// 四边形是带有可选背景、边框和圆角半径的着色矩形区域。
    /// 参见 [`fill`]、[`outline`] 和 [`quad`] 来构造此类型。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    ///
    /// 注意 `quad.corner_radii` 允许超出边界，在圆弧相交处
    /// 创建尖角。与虚线边框组合时显示效果不佳。
    /// 如果圆角应适合边界内，请使用 `Corners::clamp_radii_for_quad_size`。
    pub fn paint_quad(&mut self, quad: PaintQuad) {
        self.invalidator.debug_assert_paint();

        let opacity = self.element_opacity();
        let snapped_bounds = self.snap_bounds(quad.bounds);
        let snapped_border_widths = self.snap_border_widths(quad.border_widths);
        let quad = Quad {
            order: 0,
            bounds: snapped_bounds,
            content_mask: self.snapped_content_mask(),
            background: quad.background.opacity(opacity),
            border_color: quad.border_color.opacity(opacity),
            corner_radii: quad.corner_radii.scale(self.scale_factor()),
            border_widths: snapped_border_widths,
            border_style: quad.border_style,
            continuous_corners: 0,
            transform: TransformationMatrix::default(),
            blend_mode: 0,
            pad_quad: 0,
        };

        if !quad.background.is_transparent() {
            self.next_frame.scene.insert_primitive(quad);
            return;
        }

        // 仅带边框、无填充的四边形直接提交到场景会为每块透明的内部像素运行一次
        // 四边形着色器，当形状较大时开销尤其高。这里围绕其空白内部进行拆分。
        let outer_bounds = quad.bounds;
        let inner_bounds = Self::largest_border_interior(&quad);

        if inner_bounds.is_empty() {
            self.next_frame.scene.insert_primitive(quad);
            return;
        }

        let strips = [
            // Top
            Bounds::from_corners(
                outer_bounds.origin,
                point(outer_bounds.right(), inner_bounds.top()),
            ),
            // Bottom
            Bounds::from_corners(
                point(outer_bounds.left(), inner_bounds.bottom()),
                outer_bounds.bottom_right(),
            ),
            // Left
            Bounds::from_corners(
                point(outer_bounds.left(), inner_bounds.top()),
                inner_bounds.bottom_left(),
            ),
            // Right
            Bounds::from_corners(
                inner_bounds.top_right(),
                point(outer_bounds.right(), inner_bounds.bottom()),
            ),
        ];

        for strip in strips {
            let content_mask_bounds = quad.content_mask.bounds.intersect(&strip);
            if !content_mask_bounds.is_empty() {
                self.next_frame.scene.insert_primitive(Quad {
                    content_mask: ContentMask {
                        bounds: content_mask_bounds,
                    },
                    ..quad
                });
            }
        }
    }

    /// 在当前 z-index 将给定的 `Path` 绘制到下一帧的场景中。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    pub fn paint_path(&mut self, mut path: Path<Pixels>, color: impl Into<Background>) {
        self.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let content_mask = self.content_mask();
        let opacity = self.element_opacity();
        path.content_mask = content_mask;
        let color: Background = color.into();
        path.color = color.opacity(opacity);
        self.next_frame
            .scene
            .insert_primitive(path.scale(scale_factor));
    }

    /// 在当前 z-index 将下划线绘制到下一帧的场景中。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    pub fn paint_underline(
        &mut self,
        origin: Point<Pixels>,
        width: Pixels,
        style: &UnderlineStyle,
    ) {
        self.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let thickness = self.snap_stroke(style.thickness);
        let height = if style.wavy {
            ScaledPixels(thickness.0 * 3.)
        } else {
            thickness
        };
        let bounds = Bounds {
            origin: origin.map(|c| ScaledPixels(round_to_device_pixel(c.0, scale_factor))),
            size: size(self.snap_stroke(width), height),
        };
        let element_opacity = self.element_opacity();

        self.next_frame.scene.insert_primitive(Underline {
            order: 0,
            pad: 0,
            bounds,
            content_mask: self.snapped_content_mask(),
            color: style.color.unwrap_or_default().opacity(element_opacity),
            thickness,
            wavy: style.wavy.into(),
        });
    }

    /// 在当前 z-index 将删除线绘制到下一帧的场景中。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    pub fn paint_strikethrough(
        &mut self,
        origin: Point<Pixels>,
        width: Pixels,
        style: &StrikethroughStyle,
    ) {
        self.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let height = style.thickness;
        let bounds = Bounds {
            origin: origin.map(|c| ScaledPixels(round_to_device_pixel(c.0, scale_factor))),
            size: size(self.snap_stroke(width), self.snap_stroke(height)),
        };
        let opacity = self.element_opacity();

        self.next_frame.scene.insert_primitive(Underline {
            order: 0,
            pad: 0,
            bounds,
            content_mask: self.snapped_content_mask(),
            thickness: self.snap_stroke(style.thickness),
            color: style.color.unwrap_or_default().opacity(opacity),
            wavy: false.into(),
        });
    }

    /// 在当前 z-index 将单色（非表情符号）字形绘制到下一帧的场景中。
    ///
    /// 原点的 y 分量是字形的基线。
    /// 通常应优先使用 [`ShapedLine::paint`](crate::ShapedLine::paint) 或
    /// [`WrappedLine::paint`](crate::WrappedLine::paint)（[`TextSystem`](crate::TextSystem) 中的方法）。
    /// 此方法仅在你需要绘制已排版的单个字形时有用。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    pub fn paint_glyph(
        &mut self,
        origin: Point<Pixels>,
        font_id: FontId,
        glyph_id: GlyphId,
        font_size: Pixels,
        color: Hsla,
    ) -> Result<()> {
        self.invalidator.debug_assert_paint();

        let element_opacity = self.element_opacity();
        let scale_factor = self.scale_factor();
        let glyph_origin = origin.scale(scale_factor);

        let quantized_origin = Point::new(
            round_half_toward_zero(glyph_origin.x.0 * SUBPIXEL_VARIANTS_X as f32)
                / SUBPIXEL_VARIANTS_X as f32,
            round_half_toward_zero(glyph_origin.y.0 * SUBPIXEL_VARIANTS_Y as f32)
                / SUBPIXEL_VARIANTS_Y as f32,
        );
        let subpixel_variant = Point::new(
            (quantized_origin.x.fract() * SUBPIXEL_VARIANTS_X as f32) as u8,
            (quantized_origin.y.fract() * SUBPIXEL_VARIANTS_Y as f32) as u8,
        );
        let integer_origin = quantized_origin.map(|c| ScaledPixels(c.trunc()));
        let subpixel_rendering = self.should_use_subpixel_rendering(font_id, font_size);
        let dilation = self.text_system().glyph_dilation_for_color(color);
        let params = RenderGlyphParams {
            font_id,
            glyph_id,
            font_size,
            subpixel_variant,
            scale_factor,
            is_emoji: false,
            subpixel_rendering,
            dilation,
        };

        let raster_bounds = self.text_system().raster_bounds(&params)?;
        if !raster_bounds.is_zero() {
            let tile = self
                .sprite_atlas
                .get_or_insert_with(&params.clone().into(), &mut || {
                    let (size, bytes) = self.text_system().rasterize_glyph(&params)?;
                    Ok(Some((size, Cow::Owned(bytes))))
                })?
                .expect("Callback above only errors or returns Some");
            let bounds = Bounds {
                origin: integer_origin + raster_bounds.origin.map(Into::into),
                size: tile.bounds.size.map(Into::into),
            };
            let content_mask = self.snapped_content_mask();

            if subpixel_rendering {
                self.next_frame.scene.insert_primitive(SubpixelSprite {
                    order: 0,
                    pad: 0,
                    bounds,
                    content_mask,
                    color: color.opacity(element_opacity),
                    tile,
                    transformation: TransformationMatrix::unit(),
                });
            } else {
                self.next_frame.scene.insert_primitive(MonochromeSprite {
                    order: 0,
                    pad: 0,
                    bounds,
                    content_mask,
                    color: color.opacity(element_opacity),
                    tile,
                    transformation: TransformationMatrix::unit(),
                });
            }
        }
        Ok(())
    }

    fn should_use_subpixel_rendering(&self, font_id: FontId, font_size: Pixels) -> bool {
        if self.platform_window.background_appearance() != WindowBackgroundAppearance::Opaque {
            return false;
        }

        if !self.platform_window.is_subpixel_rendering_supported() {
            return false;
        }

        let mode = match self.text_rendering_mode.get() {
            TextRenderingMode::PlatformDefault => self
                .text_system()
                .recommended_rendering_mode(font_id, font_size),
            mode => mode,
        };

        mode == TextRenderingMode::Subpixel
    }

    /// 在当前 z-index 将表情符号字形绘制到下一帧的场景中。
    ///
    /// 原点的 y 分量是字形的基线。
    /// 通常应优先使用 [`ShapedLine::paint`](crate::ShapedLine::paint) 或
    /// [`WrappedLine::paint`](crate::WrappedLine::paint)（[`TextSystem`](crate::TextSystem) 中的方法）。
    /// 此方法仅在你需要绘制已排版的单个表情符号时有用。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    pub fn paint_emoji(
        &mut self,
        origin: Point<Pixels>,
        font_id: FontId,
        glyph_id: GlyphId,
        font_size: Pixels,
    ) -> Result<()> {
        self.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let glyph_origin = origin.scale(scale_factor);
        let integer_origin = glyph_origin.map(|c| ScaledPixels(round_half_toward_zero(c.0)));
        let params = RenderGlyphParams {
            font_id,
            glyph_id,
            font_size,
            subpixel_variant: Default::default(),
            scale_factor,
            is_emoji: true,
            subpixel_rendering: false,
            dilation: 0,
        };

        let raster_bounds = self.text_system().raster_bounds(&params)?;
        if !raster_bounds.is_zero() {
            let tile = self
                .sprite_atlas
                .get_or_insert_with(&params.clone().into(), &mut || {
                    let (size, bytes) = self.text_system().rasterize_glyph(&params)?;
                    Ok(Some((size, Cow::Owned(bytes))))
                })?
                .expect("Callback above only errors or returns Some");

            let bounds = Bounds {
                origin: integer_origin + raster_bounds.origin.map(Into::into),
                size: tile.bounds.size.map(Into::into),
            };
            let content_mask = self.snapped_content_mask();
            let opacity = self.element_opacity();

            self.next_frame.scene.insert_primitive(PolychromeSprite {
                order: 0,
                pad: 0,
                grayscale: false,
                bounds,
                corner_radii: Default::default(),
                content_mask,
                tile,
                opacity,
                transformation: TransformationMatrix::default(),
            });
        }
        Ok(())
    }

    /// 在当前层叠上下文中将单色 SVG 绘制到下一帧的场景中。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    pub fn paint_svg(
        &mut self,
        bounds: Bounds<Pixels>,
        path: SharedString,
        mut data: Option<&[u8]>,
        transformation: TransformationMatrix,
        color: Hsla,
        cx: &App,
    ) -> Result<()> {
        self.invalidator.debug_assert_paint();

        let element_opacity = self.element_opacity();
        let bounds = self.snap_bounds(bounds);

        let params = RenderSvgParams {
            path,
            size: bounds.size.map(|pixels| {
                DevicePixels::from((pixels.0 * SMOOTH_SVG_SCALE_FACTOR).ceil() as i32)
            }),
        };

        let Some(tile) =
            self.sprite_atlas
                .get_or_insert_with(&params.clone().into(), &mut || {
                    let Some((size, bytes)) = cx.svg_renderer.render_alpha_mask(&params, data)?
                    else {
                        return Ok(None);
                    };
                    Ok(Some((size, Cow::Owned(bytes))))
                })?
        else {
            return Ok(());
        };
        let content_mask = self.snapped_content_mask();
        let svg_bounds = Bounds {
            origin: bounds.center()
                - Point::new(
                    ScaledPixels(tile.bounds.size.width.0 as f32 / SMOOTH_SVG_SCALE_FACTOR / 2.),
                    ScaledPixels(tile.bounds.size.height.0 as f32 / SMOOTH_SVG_SCALE_FACTOR / 2.),
                ),
            size: tile
                .bounds
                .size
                .map(|value| ScaledPixels(value.0 as f32 / SMOOTH_SVG_SCALE_FACTOR)),
        };
        let final_bounds = svg_bounds
            .map_origin(|value| ScaledPixels(round_half_toward_zero(value.0)))
            .map_size(|size| size.ceil());

        self.next_frame.scene.insert_primitive(MonochromeSprite {
            order: 0,
            pad: 0,
            bounds: final_bounds,
            content_mask,
            color: color.opacity(element_opacity),
            tile,
            transformation,
        });

        Ok(())
    }

    /// 在当前 z-index 将图像绘制到下一帧的场景中。
    /// 如果 frame_index 无效，此方法将 panic
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    pub fn paint_image(
        &mut self,
        bounds: Bounds<Pixels>,
        corner_radii: Corners<Pixels>,
        data: Arc<RenderImage>,
        frame_index: usize,
        grayscale: bool,
    ) -> Result<()> {
        self.invalidator.debug_assert_paint();

        let bounds = self.snap_bounds(bounds);
        let params = RenderImageParams {
            image_id: data.id,
            frame_index,
        };

        let tile = self
            .sprite_atlas
            .get_or_insert_with(&params.into(), &mut || {
                Ok(Some((
                    data.size(frame_index),
                    Cow::Borrowed(
                        data.as_bytes(frame_index)
                            .expect("It's the caller's job to pass a valid frame index"),
                    ),
                )))
            })?
            .expect("Callback above only returns Some");
        let content_mask = self.snapped_content_mask();
        let corner_radii = corner_radii.scale(self.scale_factor());
        let opacity = self.element_opacity();

        self.next_frame.scene.insert_primitive(PolychromeSprite {
            order: 0,
            pad: 0,
            grayscale: grayscale,
            bounds,
            content_mask,
            corner_radii,
            tile,
            opacity,
            transformation: TransformationMatrix::default(),
        });
        Ok(())
    }

    /// 在当前 z-index 将表面绘制到下一帧的场景中。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    #[cfg(target_os = "macos")]
    pub fn paint_surface(&mut self, bounds: Bounds<Pixels>, image_buffer: CVPixelBuffer) {
        use crate::PaintSurface;

        self.invalidator.debug_assert_paint();

        let bounds = self.snap_bounds(bounds);
        let content_mask = self.snapped_content_mask();
        self.next_frame.scene.insert_primitive(PaintSurface {
            order: 0,
            bounds,
            content_mask,
            image_buffer,
        });
    }

    /// 从精灵图集中移除图像。
    pub fn drop_image(&mut self, data: Arc<RenderImage>) -> Result<()> {
        for frame_index in 0..data.frame_count() {
            let params = RenderImageParams {
                image_id: data.id,
                frame_index,
            };

            self.sprite_atlas.remove(&params.clone().into());
        }

        Ok(())
    }

    /// 返回图像的每一帧是否都存在于精灵图集中。
    #[cfg(any(test, feature = "test-support"))]
    pub fn has_image_atlas_entry(&self, data: &RenderImage) -> bool {
        data.frame_count() > 0
            && (0..data.frame_count()).all(|frame_index| {
                self.sprite_atlas.contains(
                    &RenderImageParams {
                        image_id: data.id,
                        frame_index,
                    }
                    .into(),
                )
            })
    }

    /// 为当前帧向布局树添加节点。接受请求布局的元素的 `Style`，
    /// 以及任何子元素的布局 id。此方法在
    /// 调用 [`Element::request_layout`] trait 方法时调用，使任何元素都能参与布局。
    ///
    /// 此方法只能作为元素绘制的 request_layout 或预绘制阶段的一部分调用。
    #[must_use]
    pub fn request_layout(
        &mut self,
        style: Style,
        children: impl IntoIterator<Item = LayoutId>,
        cx: &mut App,
    ) -> LayoutId {
        self.invalidator.debug_assert_prepaint();

        cx.layout_id_buffer.clear();
        cx.layout_id_buffer.extend(children);
        let rem_size = self.rem_size();
        let scale_factor = self.scale_factor();

        self.layout_engine.as_mut().unwrap().request_layout(
            style,
            rem_size,
            scale_factor,
            &cx.layout_id_buffer,
        )
    }

    /// 为当前帧向布局树添加节点。不接受 `Style` 和子元素，
    /// 此变体接受一个在布局期间调用的函数，以便你可以使用任意逻辑
    /// 来确定元素的大小。内部使用的一个地方是测量文本时。
    ///
    /// 给定的闭包在布局时使用已知尺寸和可用空间调用，
    /// 返回一个 `Size`。
    ///
    /// 此方法只能作为元素绘制的 request_layout 或预绘制阶段的一部分调用。
    pub fn request_measured_layout<F>(&mut self, style: Style, measure: F) -> LayoutId
    where
        F: Fn(Size<Option<Pixels>>, Size<AvailableSpace>, &mut Window, &mut App) -> Size<Pixels>
            + 'static,
    {
        self.invalidator.debug_assert_prepaint();

        let rem_size = self.rem_size();
        let scale_factor = self.scale_factor();
        self.layout_engine
            .as_mut()
            .unwrap()
            .request_measured_layout(style, rem_size, scale_factor, measure)
    }

    /// 在给定的可用空间内计算给定 id 的布局。
    /// 此方法因其副作用而被调用，通常由框架在绘制之前调用。
    /// 调用后，你可以请求给定布局节点 id 或其任何后代的边界。
    ///
    /// 此方法只能作为元素绘制的预绘制阶段的一部分调用。
    pub fn compute_layout(
        &mut self,
        layout_id: LayoutId,
        available_space: Size<AvailableSpace>,
        cx: &mut App,
    ) {
        self.invalidator.debug_assert_prepaint();

        let mut layout_engine = self.layout_engine.take().unwrap();
        layout_engine.compute_layout(layout_id, available_space, self, cx);
        self.layout_engine = Some(layout_engine);
    }

    /// 获取为给定 LayoutId 计算的相对于窗口的边界。此方法通常由
    /// RGPUI 自身自动调用，以便自动传递元素的 `Bounds`。
    ///
    /// 此方法只能作为元素绘制的一部分来调用。
    pub fn layout_bounds(&mut self, layout_id: LayoutId) -> Bounds<Pixels> {
        self.invalidator.debug_assert_prepaint();

        let scale_factor = self.scale_factor();
        let mut bounds = self
            .layout_engine
            .as_mut()
            .unwrap()
            .layout_bounds(layout_id, scale_factor)
            .map(Into::into);
        let snapped_offset = self.pixel_snap_point(self.element_offset());
        bounds.origin += snapped_offset;
        bounds
    }

    /// 此方法应在 `prepaint` 期间调用。你可以使用
    /// 返回的 [Hitbox] 在 `paint` 期间或事件处理程序中
    /// 来确定插入的 hitbox 是否是最顶层的。
    ///
    /// 此方法只能作为元素绘制的预绘制阶段的一部分调用。
    pub fn insert_hitbox(&mut self, bounds: Bounds<Pixels>, behavior: HitboxBehavior) -> Hitbox {
        self.invalidator.debug_assert_prepaint();

        let content_mask = self.content_mask();
        let mut id = self.next_hitbox_id;
        self.next_hitbox_id = self.next_hitbox_id.next();
        let hitbox = Hitbox {
            id,
            bounds,
            content_mask,
            behavior,
        };
        self.next_frame.hitboxes.push(hitbox.clone());

        // DOM 后端：把 hitbox 关联到当前 prepaint 元素的 DOM key（若有），
        // 供事件委托在点击 DOM 元素时按 key 链直接命中 hitbox。
        #[cfg(feature = "dom-backend")]
        if let Some(key) = self.current_dom_key() {
            self.next_frame
                .dom_key_hitboxes
                .entry(key)
                .or_default()
                .push(hitbox.id);
        }

        hitbox
    }

    /// 设置一个将作为平台窗口控制区域的 hitbox。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    pub fn insert_window_control_hitbox(&mut self, area: WindowControlArea, hitbox: Hitbox) {
        self.invalidator.debug_assert_paint();
        self.next_frame.window_control_hitboxes.push((area, hitbox));
    }

    /// 设置当前元素的按键上下文。此上下文将用于
    /// 将按键绑定转换为操作。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    pub fn set_key_context(&mut self, context: KeyContext) {
        self.invalidator.debug_assert_paint();
        self.next_frame.dispatch_tree.set_key_context(context);
    }

    /// 设置当前元素的焦点句柄。此句柄将用于管理焦点状态
    /// 和元素的键盘事件分发。
    ///
    /// 此方法只能作为元素绘制的预绘制阶段的一部分调用。
    pub fn set_focus_handle(&mut self, focus_handle: &FocusHandle, _: &App) {
        self.invalidator.debug_assert_prepaint();
        if focus_handle.is_focused(self) {
            self.next_frame.focus = Some(focus_handle.id);
        }
        self.next_frame.dispatch_tree.set_focus_id(focus_handle.id);
    }

    /// 设置当前元素的视图 id，用于管理视图缓存。
    ///
    /// 此方法只能作为元素预绘制的一部分调用。我们计划在未来
    /// 移除此方法，当我们解决一些需要直接构建编辑器元素
    /// 而不是总是通过视图使用编辑器的问题时。
    pub fn set_view_id(&mut self, view_id: EntityId) {
        self.invalidator.debug_assert_prepaint();
        self.next_frame.dispatch_tree.set_view_id(view_id);
    }

    /// 获取当前正在渲染的视图的实体 ID
    pub fn current_view(&self) -> EntityId {
        self.invalidator.debug_assert_paint_or_prepaint();
        self.rendered_entity_stack.last().copied().unwrap()
    }

    #[inline]
    pub(crate) fn with_rendered_view<R>(
        &mut self,
        id: EntityId,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.rendered_entity_stack.push(id);
        let result = f(self);
        self.rendered_entity_stack.pop();
        result
    }

    /// 使用指定的图像缓存执行提供的函数。
    pub fn with_image_cache<F, R>(&mut self, image_cache: Option<AnyImageCache>, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        if let Some(image_cache) = image_cache {
            self.image_cache_stack.push(image_cache);
            let result = f(self);
            self.image_cache_stack.pop();
            result
        } else {
            f(self)
        }
    }

    /// 设置输入处理器，例如 [`ElementInputHandler`][element_input_handler]，它与
    /// 平台接口以接收文本输入，并与 IME 交互等
    /// 关注点适当集成。此处理器将在即将到来的帧中处于活动状态，直到下一帧
    /// 渲染完成。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    ///
    /// [element_input_handler]: crate::ElementInputHandler
    pub fn handle_input(
        &mut self,
        focus_handle: &FocusHandle,
        input_handler: impl InputHandler,
        cx: &App,
    ) {
        self.invalidator.debug_assert_paint();

        if focus_handle.is_focused(self) {
            let cx = self.to_async(cx);
            self.next_frame
                .input_handlers
                .push(Some(PlatformInputHandler::new(cx, Box::new(input_handler))));
        }
    }

    /// 在窗口上注册下一帧的鼠标事件监听器。事件类型
    /// 由给定监听器的第一个参数决定。当下一帧渲染时
    /// 监听器将被清除。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    pub fn on_mouse_event<Event: MouseEvent>(
        &mut self,
        mut listener: impl FnMut(&Event, DispatchPhase, &mut Window, &mut App) + 'static,
    ) {
        self.invalidator.debug_assert_paint();

        self.next_frame.mouse_listeners.push(Some(Box::new(
            move |event: &dyn Any, phase: DispatchPhase, window: &mut Window, cx: &mut App| {
                if let Some(event) = event.downcast_ref() {
                    listener(event, phase, window, cx)
                }
            },
        )));
    }

    /// 在此节点上注册下一帧的按键事件监听器。事件类型
    /// 由给定监听器的第一个参数决定。当下一帧渲染时
    /// 监听器将被清除。
    ///
    /// 这是一个相当底层的方法，除非你有
    /// 自己注册监听器的特定需求，否则优先使用元素上的操作处理程序。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    pub fn on_key_event<Event: KeyEvent>(
        &mut self,
        listener: impl Fn(&Event, DispatchPhase, &mut Window, &mut App) + 'static,
    ) {
        self.invalidator.debug_assert_paint();

        self.next_frame.dispatch_tree.on_key_event(Rc::new(
            move |event: &dyn Any, phase, window: &mut Window, cx: &mut App| {
                if let Some(event) = event.downcast_ref::<Event>() {
                    listener(event, phase, window, cx)
                }
            },
        ));
    }

    /// 在窗口上注册下一帧的修饰键更改事件监听器。
    ///
    /// 这是一个相当底层的方法，除非你有
    /// 注册全局监听器的特定需求，否则优先使用元素上的事件处理程序。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    pub fn on_modifiers_changed(
        &mut self,
        listener: impl Fn(&ModifiersChangedEvent, &mut Window, &mut App) + 'static,
    ) {
        self.invalidator.debug_assert_paint();

        self.next_frame.dispatch_tree.on_modifiers_changed(Rc::new(
            move |event: &ModifiersChangedEvent, window: &mut Window, cx: &mut App| {
                listener(event, window, cx)
            },
        ));
    }

    /// 注册一个监听器，当给定的焦点句柄或其后代之一获得焦点时调用。
    /// 如果给定的焦点句柄或其后代之前已聚焦，则不会触发。
    /// 返回一个订阅，持续到订阅被丢弃。
    pub fn on_focus_in(
        &mut self,
        handle: &FocusHandle,
        cx: &mut App,
        mut listener: impl FnMut(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let focus_id = handle.id;
        let (subscription, activate) =
            self.new_focus_listener(Box::new(move |event, window, cx| {
                if event.is_focus_in(focus_id) {
                    listener(window, cx);
                }
                true
            }));
        cx.defer(move |_| activate());
        subscription
    }

    /// 注册一个监听器，当给定的焦点句柄或其后代之一失去焦点时调用。
    /// 返回一个订阅，持续到订阅被丢弃。
    pub fn on_focus_out(
        &mut self,
        handle: &FocusHandle,
        cx: &mut App,
        mut listener: impl FnMut(FocusOutEvent, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        let focus_id = handle.id;
        let (subscription, activate) =
            self.new_focus_listener(Box::new(move |event, window, cx| {
                if let Some(blurred_id) = event.previous_focus_path.last().copied()
                    && event.is_focus_out(focus_id)
                {
                    let event = FocusOutEvent {
                        blurred: WeakFocusHandle {
                            id: blurred_id,
                            handles: Arc::downgrade(&cx.focus_handles),
                        },
                    };
                    listener(event, window, cx)
                }
                true
            }));
        cx.defer(move |_| activate());
        subscription
    }

    fn reset_cursor_style(&self, cx: &mut App) {
        // Set the cursor only if we're the active window.
        if self.is_window_hovered() {
            let style = self
                .rendered_frame
                .cursor_style(self)
                .unwrap_or(CursorStyle::Arrow);
            cx.platform.set_cursor_style(style);
        }
    }

    /// 就像用户输入一样分发给定的按键。
    /// 你可以使用 Keystroke::parse("") 创建按键。
    pub fn dispatch_keystroke(&mut self, keystroke: Keystroke, cx: &mut App) -> bool {
        let keystroke = keystroke.with_simulated_ime();
        let result = self.dispatch_event(
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        );
        if !result.propagate {
            return true;
        }

        if let Some(input) = keystroke.key_char
            && let Some(mut input_handler) = self.platform_window.take_input_handler()
        {
            input_handler.dispatch_input(&input, self, cx);
            self.platform_window.set_input_handler(input_handler);
            return true;
        }

        false
    }

    /// 返回操作的按键绑定字符串，用于在 UI 中显示。使用最高优先级
    /// 的操作绑定（最后添加到键映射的绑定）。
    pub fn keystroke_text_for(&self, action: &dyn Action) -> String {
        self.highest_precedence_binding_for_action(action)
            .map(|binding| {
                binding
                    .keystrokes()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_else(|| action.name().to_string())
    }

    /// 在窗口上分发鼠标或键盘事件。
    #[profiling::function]
    pub fn dispatch_event(&mut self, event: PlatformInput, cx: &mut App) -> DispatchEventResult {
        self.dispatch_event_inner(event, cx, None)
    }

    /// 按 DOM key 链派发事件：跳过坐标 hit-test，直接命中这些 key 关联的 hitbox。
    ///
    /// 用于 Web 纯 DOM 模式的事件委托——点击 DOM 元素时浏览器报告的是元素身份而非
    /// 画布坐标，坐标 hit-test 在滚动/缩放下会错位，这里改按 DOM 树反查 key 链命中。
    #[cfg(feature = "dom-backend")]
    pub fn dispatch_event_for_dom(
        &mut self,
        keys: Vec<crate::DomNodeKey>,
        event: PlatformInput,
        cx: &mut App,
    ) -> DispatchEventResult {
        self.dispatch_event_inner(event, cx, Some(keys))
    }

    /// `dispatch_event` 的内部实现，支持可选的 DOM key 覆盖（事件委托时使用）。
    #[profiling::function]
    fn dispatch_event_inner(
        &mut self,
        event: PlatformInput,
        cx: &mut App,
        dom_keys: Option<Vec<crate::DomNodeKey>>,
    ) -> DispatchEventResult {
        #[cfg(feature = "input-latency-histogram")]
        let dispatch_time = Instant::now();
        let update_count_before = self.invalidator.update_count();
        // Track input modality for focus-visible styling and hover suppression.
        // Hover is suppressed during keyboard modality so that keyboard navigation
        // doesn't show hover highlights on the item under the mouse cursor.
        let old_modality = self.last_input_modality;
        self.last_input_modality = match &event {
            PlatformInput::KeyDown(_) => InputModality::Keyboard,
            PlatformInput::MouseMove(_) | PlatformInput::MouseDown(_) => InputModality::Mouse,
            PlatformInput::Touch(_) => InputModality::Touch,
            _ => self.last_input_modality,
        };
        if self.last_input_modality != old_modality {
            self.refresh();
        }

        // Handlers may set this to false by calling `stop_propagation`.
        cx.propagate_event = true;
        // Handlers may set this to true by calling `prevent_default`.
        self.default_prevented = false;

        let event = match event {
            // Track the mouse position with our own state, since accessing the platform
            // API for the mouse position can only occur on the main thread.
            PlatformInput::MouseMove(mouse_move) => {
                self.mouse_position = mouse_move.position;
                self.modifiers = mouse_move.modifiers;
                PlatformInput::MouseMove(mouse_move)
            }
            PlatformInput::MouseDown(mouse_down) => {
                self.mouse_position = mouse_down.position;
                self.modifiers = mouse_down.modifiers;
                PlatformInput::MouseDown(mouse_down)
            }
            PlatformInput::MouseUp(mouse_up) => {
                self.mouse_position = mouse_up.position;
                self.modifiers = mouse_up.modifiers;
                PlatformInput::MouseUp(mouse_up)
            }
            PlatformInput::MousePressure(mouse_pressure) => {
                PlatformInput::MousePressure(mouse_pressure)
            }
            PlatformInput::MouseExited(mouse_exited) => {
                self.modifiers = mouse_exited.modifiers;
                PlatformInput::MouseExited(mouse_exited)
            }
            PlatformInput::ModifiersChanged(modifiers_changed) => {
                self.modifiers = modifiers_changed.modifiers;
                self.capslock = modifiers_changed.capslock;
                PlatformInput::ModifiersChanged(modifiers_changed)
            }
            PlatformInput::ScrollWheel(scroll_wheel) => {
                self.mouse_position = scroll_wheel.position;
                self.modifiers = scroll_wheel.modifiers;
                PlatformInput::ScrollWheel(scroll_wheel)
            }
            PlatformInput::Pinch(pinch) => {
                self.mouse_position = pinch.position;
                self.modifiers = pinch.modifiers;
                PlatformInput::Pinch(pinch)
            }
            // Translate dragging and dropping of external files from the operating system
            // to internal drag and drop events.
            PlatformInput::FileDrop(file_drop) => match file_drop {
                FileDropEvent::Entered { position, paths } => {
                    self.mouse_position = position;
                    if cx.active_drag.is_none() {
                        cx.active_drag = Some(AnyDrag {
                            value: Arc::new(paths.clone()),
                            view: cx.new(|_| paths).into(),
                            cursor_offset: position,
                            cursor_style: None,
                        });
                    }
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position,
                        pressed_button: Some(MouseButton::Left),
                        modifiers: Modifiers::default(),
                    })
                }
                FileDropEvent::Pending { position } => {
                    self.mouse_position = position;
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position,
                        pressed_button: Some(MouseButton::Left),
                        modifiers: Modifiers::default(),
                    })
                }
                FileDropEvent::Submit { position } => {
                    cx.activate(true);
                    self.mouse_position = position;
                    PlatformInput::MouseUp(MouseUpEvent {
                        button: MouseButton::Left,
                        position,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                    })
                }
                FileDropEvent::Exited => {
                    cx.active_drag.take();
                    PlatformInput::FileDrop(FileDropEvent::Exited)
                }
            },
            PlatformInput::Touch(touch) => PlatformInput::Touch(touch),
            PlatformInput::KeyDown(_) | PlatformInput::KeyUp(_) => event,
        };

        if let Some(any_mouse_event) = event.mouse_event() {
            self.dispatch_mouse_event(any_mouse_event, cx, dom_keys.as_deref());
        } else if let Some(any_key_event) = event.keyboard_event() {
            self.dispatch_key_event(any_key_event, cx);
        }

        if self.invalidator.update_count() > update_count_before {
            self.input_rate_tracker.borrow_mut().record_input();
            #[cfg(feature = "input-latency-histogram")]
            if self.invalidator.not_drawing() {
                self.input_latency_tracker.record_input(dispatch_time);
            } else {
                self.input_latency_tracker.record_mid_draw_input();
            }
        }

        DispatchEventResult {
            propagate: cx.propagate_event,
            default_prevented: self.default_prevented,
        }
    }

    fn dispatch_mouse_event(
        &mut self,
        event: &dyn Any,
        cx: &mut App,
        dom_keys: Option<&[crate::DomNodeKey]>,
    ) {
        let hit_test = {
            #[cfg(feature = "dom-backend")]
            {
                match dom_keys {
                    Some(keys) => self.dom_keys_hit_test(keys),
                    None => self.rendered_frame.hit_test(self.mouse_position()),
                }
            }
            #[cfg(not(feature = "dom-backend"))]
            {
                let _ = dom_keys;
                self.rendered_frame.hit_test(self.mouse_position())
            }
        };
        if hit_test != self.mouse_hit_test {
            self.mouse_hit_test = hit_test;
            self.reset_cursor_style(cx);
        }

        #[cfg(any(feature = "inspector", debug_assertions))]
        if self.is_inspector_picking(cx) {
            self.handle_inspector_mouse_event(event, cx);
            // When inspector is picking, all other mouse handling is skipped.
            return;
        }

        let mut mouse_listeners = mem::take(&mut self.rendered_frame.mouse_listeners);

        // Capture phase, events bubble from back to front. Handlers for this phase are used for
        // special purposes, such as detecting events outside of a given Bounds.
        for listener in &mut mouse_listeners {
            let listener = listener.as_mut().unwrap();
            listener(event, DispatchPhase::Capture, self, cx);
            if !cx.propagate_event {
                break;
            }
        }

        // Bubble phase, where most normal handlers do their work.
        if cx.propagate_event {
            for listener in mouse_listeners.iter_mut().rev() {
                let listener = listener.as_mut().unwrap();
                listener(event, DispatchPhase::Bubble, self, cx);
                if !cx.propagate_event {
                    break;
                }
            }
        }

        self.rendered_frame.mouse_listeners = mouse_listeners;

        if cx.has_active_drag() {
            if event.is::<MouseMoveEvent>() {
                // If this was a mouse move event, redraw the window so that the
                // active drag can follow the mouse cursor.
                self.refresh();
            } else if event.is::<MouseUpEvent>() {
                // If this was a mouse up event, cancel the active drag and redraw
                // the window.
                cx.active_drag = None;
                self.refresh();
            }
        }

        // Auto-release pointer capture on mouse up
        if event.is::<MouseUpEvent>() && self.captured_hitbox.is_some() {
            self.captured_hitbox = None;
        }
    }

    /// 根据 DOM key 链收集对应的 hitbox，构造一个显式命中结果（事件委托用）。
    ///
    /// 链上所有 key 的 hitbox 都视为命中（hover_hitbox_count 等于命中总数），
    /// 从而绕过坐标 hit-test 的错位问题。
    #[cfg(feature = "dom-backend")]
    fn dom_keys_hit_test(&self, keys: &[crate::DomNodeKey]) -> HitTest {
        let mut ids = SmallVec::<[HitboxId; 8]>::new();
        for key in keys {
            if let Some(hitbox_ids) = self.rendered_frame.dom_key_hitboxes.get(key) {
                for hitbox_id in hitbox_ids {
                    if !ids.contains(hitbox_id) {
                        ids.push(*hitbox_id);
                    }
                }
            }
        }
        HitTest {
            hover_hitbox_count: ids.len(),
            ids,
        }
    }

    fn dispatch_key_event(&mut self, event: &dyn Any, cx: &mut App) {
        if self.invalidator.is_dirty() {
            self.draw(cx).clear(cx);
        }

        let node_id = self.focus_node_id_in_rendered_frame(self.focus);
        let dispatch_path = self.rendered_frame.dispatch_tree.dispatch_path(node_id);

        let mut keystroke: Option<Keystroke> = None;

        if let Some(event) = event.downcast_ref::<ModifiersChangedEvent>() {
            if event.modifiers.number_of_modifiers() == 0
                && self.pending_modifier.modifiers.number_of_modifiers() == 1
                && !self.pending_modifier.saw_other_input
            {
                let key = match self.pending_modifier.modifiers {
                    modifiers if modifiers.shift => Some("shift"),
                    modifiers if modifiers.control => Some("control"),
                    modifiers if modifiers.alt => Some("alt"),
                    modifiers if modifiers.platform => Some("platform"),
                    modifiers if modifiers.function => Some("function"),
                    _ => None,
                };
                if let Some(key) = key {
                    keystroke = Some(Keystroke {
                        key: key.to_string(),
                        key_char: None,
                        modifiers: Modifiers::default(),
                    });
                }
            }

            if self.pending_modifier.modifiers.number_of_modifiers() == 0
                && event.modifiers.number_of_modifiers() == 1
            {
                self.pending_modifier.saw_other_input = false
            } else if event.modifiers.number_of_modifiers() > 1 {
                self.pending_modifier.saw_other_input = true
            }
            self.pending_modifier.modifiers = event.modifiers
        } else if let Some(key_down_event) = event.downcast_ref::<KeyDownEvent>() {
            self.pending_modifier.saw_other_input = true;
            keystroke = Some(key_down_event.keystroke.clone());
            if key_down_event.keystroke.key_char.is_some()
                && matches!(
                    cx.cursor_hide_mode,
                    CursorHideMode::OnTyping | CursorHideMode::OnTypingAndAction
                )
            {
                cx.platform.hide_cursor_until_mouse_moves();
            }
        }

        let Some(keystroke) = keystroke else {
            self.finish_dispatch_key_event(event, dispatch_path, self.context_stack(), cx);
            return;
        };

        cx.propagate_event = true;
        self.dispatch_keystroke_interceptors(event, self.context_stack(), cx);
        if !cx.propagate_event {
            self.finish_dispatch_key_event(event, dispatch_path, self.context_stack(), cx);
            return;
        }

        let mut currently_pending = self.pending_input.take().unwrap_or_default();
        if currently_pending.focus.is_some() && currently_pending.focus != self.focus {
            currently_pending = PendingInput::default();
        }

        let match_result = self.rendered_frame.dispatch_tree.dispatch_key(
            currently_pending.keystrokes,
            keystroke,
            &dispatch_path,
        );

        if !match_result.to_replay.is_empty() {
            self.replay_pending_input(match_result.to_replay, cx);
            cx.propagate_event = true;
        }

        if !match_result.pending.is_empty() {
            currently_pending.timer.take();
            currently_pending.keystrokes = match_result.pending;
            currently_pending.focus = self.focus;

            let text_input_requires_timeout = event
                .downcast_ref::<KeyDownEvent>()
                .filter(|key_down| key_down.keystroke.key_char.is_some())
                .and_then(|_| self.platform_window.take_input_handler())
                .map_or(false, |mut input_handler| {
                    let accepts = input_handler.accepts_text_input(self, cx);
                    self.platform_window.set_input_handler(input_handler);
                    accepts
                });

            currently_pending.needs_timeout |=
                match_result.pending_has_binding || text_input_requires_timeout;

            if currently_pending.needs_timeout {
                currently_pending.timer = Some(self.spawn(cx, async move |cx| {
                    cx.background_executor.timer(Duration::from_secs(1)).await;
                    cx.update(move |window, cx| {
                        let Some(currently_pending) = window
                            .pending_input
                            .take()
                            .filter(|pending| pending.focus == window.focus)
                        else {
                            return;
                        };

                        let node_id = window.focus_node_id_in_rendered_frame(window.focus);
                        let dispatch_path =
                            window.rendered_frame.dispatch_tree.dispatch_path(node_id);

                        let to_replay = window
                            .rendered_frame
                            .dispatch_tree
                            .flush_dispatch(currently_pending.keystrokes, &dispatch_path);

                        window.pending_input_changed(cx);
                        window.replay_pending_input(to_replay, cx)
                    })
                    .log_err();
                }));
            } else {
                currently_pending.timer = None;
            }
            self.pending_input = Some(currently_pending);
            self.pending_input_changed(cx);
            cx.propagate_event = false;
            return;
        }

        let skip_bindings = event
            .downcast_ref::<KeyDownEvent>()
            .filter(|key_down_event| key_down_event.prefer_character_input)
            .map(|_| {
                self.platform_window
                    .take_input_handler()
                    .map_or(false, |mut input_handler| {
                        let accepts = input_handler.accepts_text_input(self, cx);
                        self.platform_window.set_input_handler(input_handler);
                        // If modifiers are not excessive (e.g. AltGr), and the input handler is accepting text input,
                        // we prefer the text input over bindings.
                        accepts
                    })
            })
            .unwrap_or(false);

        if !skip_bindings {
            for binding in match_result.bindings {
                self.dispatch_action_on_node(node_id, binding.action.as_ref(), cx);
                if !cx.propagate_event {
                    self.dispatch_keystroke_observers(
                        event,
                        Some(binding.action),
                        match_result.context_stack,
                        cx,
                    );
                    self.pending_input_changed(cx);
                    return;
                }
            }
        }

        self.finish_dispatch_key_event(event, dispatch_path, match_result.context_stack, cx);
        self.pending_input_changed(cx);
    }

    fn finish_dispatch_key_event(
        &mut self,
        event: &dyn Any,
        dispatch_path: SmallVec<[DispatchNodeId; 32]>,
        context_stack: Vec<KeyContext>,
        cx: &mut App,
    ) {
        self.dispatch_key_down_up_event(event, &dispatch_path, cx);
        if !cx.propagate_event {
            return;
        }

        self.dispatch_modifiers_changed_event(event, &dispatch_path, cx);
        if !cx.propagate_event {
            return;
        }

        self.dispatch_keystroke_observers(event, None, context_stack, cx);
    }

    pub(crate) fn pending_input_changed(&mut self, cx: &mut App) {
        self.pending_input_observers
            .clone()
            .retain(&(), |callback| callback(self, cx));
    }

    fn dispatch_key_down_up_event(
        &mut self,
        event: &dyn Any,
        dispatch_path: &SmallVec<[DispatchNodeId; 32]>,
        cx: &mut App,
    ) {
        // Capture phase
        for node_id in dispatch_path {
            let node = self.rendered_frame.dispatch_tree.node(*node_id);

            for key_listener in node.key_listeners.clone() {
                key_listener(event, DispatchPhase::Capture, self, cx);
                if !cx.propagate_event {
                    return;
                }
            }
        }

        // Bubble phase
        for node_id in dispatch_path.iter().rev() {
            // Handle low level key events
            let node = self.rendered_frame.dispatch_tree.node(*node_id);
            for key_listener in node.key_listeners.clone() {
                key_listener(event, DispatchPhase::Bubble, self, cx);
                if !cx.propagate_event {
                    return;
                }
            }
        }
    }

    fn dispatch_modifiers_changed_event(
        &mut self,
        event: &dyn Any,
        dispatch_path: &SmallVec<[DispatchNodeId; 32]>,
        cx: &mut App,
    ) {
        let Some(event) = event.downcast_ref::<ModifiersChangedEvent>() else {
            return;
        };
        for node_id in dispatch_path.iter().rev() {
            let node = self.rendered_frame.dispatch_tree.node(*node_id);
            for listener in node.modifiers_changed_listeners.clone() {
                listener(event, self, cx);
                if !cx.propagate_event {
                    return;
                }
            }
        }
    }

    /// 仍可完成绑定的待处理输入。上一焦点遗留的输入永远不会完成绑定。
    fn active_pending_input(&self) -> Option<&PendingInput> {
        self.pending_input
            .as_ref()
            .filter(|pending_input| pending_input.focus == self.focus)
    }

    /// 确定此窗口上是否正在进行潜在的多按键绑定。
    pub fn has_pending_keystrokes(&self) -> bool {
        self.active_pending_input().is_some()
    }

    pub(crate) fn clear_pending_keystrokes(&mut self) {
        self.pending_input.take();
    }

    /// 返回当前待处理的输入按键，这些按键可能导致多按键绑定。
    pub fn pending_input_keystrokes(&self) -> Option<&[Keystroke]> {
        self.active_pending_input()
            .map(|pending_input| pending_input.keystrokes.as_slice())
    }

    fn replay_pending_input(&mut self, replays: SmallVec<[Replay; 1]>, cx: &mut App) {
        let node_id = self.focus_node_id_in_rendered_frame(self.focus);
        let dispatch_path = self.rendered_frame.dispatch_tree.dispatch_path(node_id);

        'replay: for replay in replays {
            let event = KeyDownEvent {
                keystroke: replay.keystroke.clone(),
                is_held: false,
                prefer_character_input: true,
            };

            cx.propagate_event = true;
            for binding in replay.bindings {
                self.dispatch_action_on_node(node_id, binding.action.as_ref(), cx);
                if !cx.propagate_event {
                    self.dispatch_keystroke_observers(
                        &event,
                        Some(binding.action),
                        Vec::default(),
                        cx,
                    );
                    continue 'replay;
                }
            }

            self.dispatch_key_down_up_event(&event, &dispatch_path, cx);
            if !cx.propagate_event {
                continue 'replay;
            }
            if let Some(input) = replay.keystroke.key_char.as_ref().cloned()
                && let Some(mut input_handler) = self.platform_window.take_input_handler()
            {
                input_handler.dispatch_input(&input, self, cx);
                self.platform_window.set_input_handler(input_handler)
            }
        }
    }

    fn focus_node_id_in_rendered_frame(&self, focus_id: Option<FocusId>) -> DispatchNodeId {
        focus_id
            .and_then(|focus_id| {
                self.rendered_frame
                    .dispatch_tree
                    .focusable_node_id(focus_id)
            })
            .unwrap_or_else(|| self.rendered_frame.dispatch_tree.root_node_id())
    }

    fn dispatch_action_on_node(
        &mut self,
        node_id: DispatchNodeId,
        action: &dyn Action,
        cx: &mut App,
    ) {
        self.dispatch_action_on_node_inner(node_id, action, cx);

        if !cx.propagate_event
            && cx.cursor_hide_mode == CursorHideMode::OnTypingAndAction
            && self.last_input_was_keyboard()
        {
            cx.platform.hide_cursor_until_mouse_moves();
        }
    }

    fn dispatch_action_on_node_inner(
        &mut self,
        node_id: DispatchNodeId,
        action: &dyn Action,
        cx: &mut App,
    ) {
        let dispatch_path = self.rendered_frame.dispatch_tree.dispatch_path(node_id);

        // Capture phase for global actions.
        cx.propagate_event = true;
        if let Some(mut global_listeners) = cx
            .global_action_listeners
            .remove(&action.as_any().type_id())
        {
            for listener in &global_listeners {
                profiler::update_running_action(action, cx);
                listener(action.as_any(), DispatchPhase::Capture, cx);
                profiler::save_action_timing();
                if !cx.propagate_event {
                    break;
                }
            }

            global_listeners.extend(
                cx.global_action_listeners
                    .remove(&action.as_any().type_id())
                    .unwrap_or_default(),
            );

            cx.global_action_listeners
                .insert(action.as_any().type_id(), global_listeners);
        }

        if !cx.propagate_event {
            return;
        }

        // Capture phase for window actions.
        for node_id in &dispatch_path {
            let node = self.rendered_frame.dispatch_tree.node(*node_id);
            for DispatchActionListener {
                action_type,
                listener,
            } in node.action_listeners.clone()
            {
                let any_action = action.as_any();
                if action_type == any_action.type_id() {
                    profiler::update_running_action(action, cx);
                    listener(any_action, DispatchPhase::Capture, self, cx);
                    profiler::save_action_timing();

                    if !cx.propagate_event {
                        return;
                    }
                }
            }
        }

        // Bubble phase for window actions.
        for node_id in dispatch_path.iter().rev() {
            let node = self.rendered_frame.dispatch_tree.node(*node_id);
            for DispatchActionListener {
                action_type,
                listener,
            } in node.action_listeners.clone()
            {
                let any_action = action.as_any();
                if action_type == any_action.type_id() {
                    cx.propagate_event = false; // Actions stop propagation by default during the bubble phase
                    profiler::update_running_action(action, cx);
                    listener(any_action, DispatchPhase::Bubble, self, cx);
                    profiler::save_action_timing();

                    if !cx.propagate_event {
                        return;
                    }
                }
            }
        }

        // Bubble phase for global actions.
        if let Some(mut global_listeners) = cx
            .global_action_listeners
            .remove(&action.as_any().type_id())
        {
            for listener in global_listeners.iter().rev() {
                cx.propagate_event = false; // Actions stop propagation by default during the bubble phase

                profiler::update_running_action(action, cx);
                listener(action.as_any(), DispatchPhase::Bubble, cx);
                profiler::save_action_timing();
                if !cx.propagate_event {
                    break;
                }
            }

            global_listeners.extend(
                cx.global_action_listeners
                    .remove(&action.as_any().type_id())
                    .unwrap_or_default(),
            );

            cx.global_action_listeners
                .insert(action.as_any().type_id(), global_listeners);
        }
    }

    /// 注册给定的处理程序，每当给定类型的全局变量
    /// 更新时调用。
    pub fn observe_global<G: Global>(
        &mut self,
        cx: &mut App,
        f: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let window_handle = self.handle;
        let (subscription, activate) = cx.global_observers.insert(
            TypeId::of::<G>(),
            Box::new(move |cx| {
                window_handle
                    .update(cx, |_, window, cx| f(window, cx))
                    .is_ok()
            }),
        );
        cx.defer(move |_| activate());
        subscription
    }

    /// 聚焦当前窗口并在平台级别将其带到前台。
    pub fn activate_window(&self) {
        self.platform_window.activate();
    }

    /// 请求操作系统引起对此窗口的注意。
    pub fn request_attention(&self) {
        self.platform_window.request_attention();
    }

    /// 在平台级别最小化当前窗口。
    pub fn minimize_window(&self) {
        self.platform_window.minimize();
    }

    /// 从任务栏和屏幕隐藏窗口。
    pub fn hide_window(&self) {
        self.platform_window.hide();
    }

    /// 设置窗口是否允许鼠标事件传递到其后面的窗口。
    pub fn set_mouse_passthrough(&self, passthrough: bool) {
        self.platform_window.set_mouse_passthrough(passthrough);
    }

    /// 在屏幕坐标中设置窗口位置。
    pub fn set_position(&mut self, position: Point<Pixels>) {
        self.platform_window.set_position(position);
    }

    /// 在平台级别切换当前窗口的全屏状态。
    pub fn toggle_fullscreen(&self) {
        self.platform_window.toggle_fullscreen();
    }

    /// 更新 IME 面板位置建议，适用于日语、中文等语言。
    pub fn invalidate_character_coordinates(&self) {
        self.on_next_frame(|window, cx| {
            if let Some(mut input_handler) = window.platform_window.take_input_handler() {
                if let Some(bounds) = input_handler.selected_bounds(window, cx) {
                    window.platform_window.update_ime_position(bounds);
                }
                window.platform_window.set_input_handler(input_handler);
            }
        });
    }

    /// 呈现平台对话框。
    /// 将显示提供的消息以及每个答案的按钮。
    /// 当按钮被点击时，返回的 Receiver 将接收到被点击按钮的索引。
    pub fn prompt<T>(
        &mut self,
        level: PromptLevel,
        message: &str,
        detail: Option<&str>,
        answers: &[T],
        cx: &mut App,
    ) -> oneshot::Receiver<usize>
    where
        T: Clone + Into<PromptButton>,
    {
        let prompt_builder = cx.prompt_builder.take();
        let Some(prompt_builder) = prompt_builder else {
            unreachable!("Re-entrant window prompting is not supported by RGPUI");
        };

        let answers = answers
            .iter()
            .map(|answer| answer.clone().into())
            .collect::<Vec<_>>();

        let receiver = match &prompt_builder {
            PromptBuilder::Default => self
                .platform_window
                .prompt(level, message, detail, &answers)
                .unwrap_or_else(|| {
                    self.build_custom_prompt(&prompt_builder, level, message, detail, &answers, cx)
                }),
            PromptBuilder::Custom(_) => {
                self.build_custom_prompt(&prompt_builder, level, message, detail, &answers, cx)
            }
        };

        cx.prompt_builder = Some(prompt_builder);

        receiver
    }

    fn build_custom_prompt(
        &mut self,
        prompt_builder: &PromptBuilder,
        level: PromptLevel,
        message: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
        cx: &mut App,
    ) -> oneshot::Receiver<usize> {
        let (sender, receiver) = oneshot::channel();
        let handle = PromptHandle::new(sender);
        let handle = (prompt_builder)(level, message, detail, answers, handle, self, cx);
        self.prompt = Some(handle);
        receiver
    }

    /// 返回由 RGPUI 渲染的提示是否在此窗口中处于活动状态。
    ///
    /// 仅对在窗口中渲染的提示为 true（参见
    /// [`App::set_prompt_builder`])，不适用于平台原生提示对话框。
    pub fn has_active_prompt(&self) -> bool {
        self.prompt.is_some()
    }

    /// 返回当前上下文栈。
    pub fn context_stack(&self) -> Vec<KeyContext> {
        let node_id = self.focus_node_id_in_rendered_frame(self.focus);
        let dispatch_tree = &self.rendered_frame.dispatch_tree;
        dispatch_tree
            .dispatch_path(node_id)
            .iter()
            .filter_map(move |&node_id| dispatch_tree.node(node_id).context.clone())
            .collect()
    }

    /// 返回聚焦元素的所有可用操作。
    pub fn available_actions(&self, cx: &App) -> Vec<Box<dyn Action>> {
        let node_id = self.focus_node_id_in_rendered_frame(self.focus);
        let mut actions = self.rendered_frame.dispatch_tree.available_actions(node_id);
        for action_type in cx.global_action_listeners.keys() {
            if let Err(ix) = actions.binary_search_by_key(action_type, |a| a.as_any().type_id()) {
                let action = cx.actions.build_action_type(action_type).ok();
                if let Some(action) = action {
                    actions.insert(ix, action);
                }
            }
        }
        actions
    }

    /// 返回在当前聚焦元素上调用操作的按键绑定。绑定
    /// 按添加顺序返回。对于显示，最后一个绑定应优先。
    pub fn bindings_for_action(&self, action: &dyn Action) -> Vec<KeyBinding> {
        self.rendered_frame
            .dispatch_tree
            .bindings_for_action(action, &self.rendered_frame.dispatch_tree.context_stack)
    }

    /// 返回在当前聚焦元素上调用操作的最高优先级按键绑定。
    /// 这比获取 `bindings_for_action` 的最后一个结果更高效。
    pub fn highest_precedence_binding_for_action(&self, action: &dyn Action) -> Option<KeyBinding> {
        self.rendered_frame
            .dispatch_tree
            .highest_precedence_binding_for_action(
                action,
                &self.rendered_frame.dispatch_tree.context_stack,
            )
    }

    /// 返回上下文中操作的按键绑定。
    pub fn bindings_for_action_in_context(
        &self,
        action: &dyn Action,
        context: KeyContext,
    ) -> Vec<KeyBinding> {
        let dispatch_tree = &self.rendered_frame.dispatch_tree;
        dispatch_tree.bindings_for_action(action, &[context])
    }

    /// 返回上下文中操作的最高优先级按键绑定。这比
    /// 获取 `bindings_for_action_in_context` 的最后一个结果更高效。
    pub fn highest_precedence_binding_for_action_in_context(
        &self,
        action: &dyn Action,
        context: KeyContext,
    ) -> Option<KeyBinding> {
        let dispatch_tree = &self.rendered_frame.dispatch_tree;
        dispatch_tree.highest_precedence_binding_for_action(action, &[context])
    }

    /// 返回如果给定焦点句柄被聚焦时将调用操作的任何绑定。绑定
    /// 按添加顺序返回。对于显示，最后一个绑定
    /// 应优先。
    pub fn bindings_for_action_in(
        &self,
        action: &dyn Action,
        focus_handle: &FocusHandle,
    ) -> Vec<KeyBinding> {
        let dispatch_tree = &self.rendered_frame.dispatch_tree;
        let Some(context_stack) = self.context_stack_for_focus_handle(focus_handle) else {
            return vec![];
        };
        dispatch_tree.bindings_for_action(action, &context_stack)
    }

    /// 返回如果给定焦点句柄被聚焦时将调用操作的最高优先级
    /// 按键绑定。这比获取
    /// `bindings_for_action_in` 的最后一个结果更高效。
    pub fn highest_precedence_binding_for_action_in(
        &self,
        action: &dyn Action,
        focus_handle: &FocusHandle,
    ) -> Option<KeyBinding> {
        let dispatch_tree = &self.rendered_frame.dispatch_tree;
        let context_stack = self.context_stack_for_focus_handle(focus_handle)?;
        dispatch_tree.highest_precedence_binding_for_action(action, &context_stack)
    }

    /// 查找可以跟随当前上下文栈的当前输入序列的绑定。
    pub fn possible_bindings_for_input(&self, input: &[Keystroke]) -> Vec<KeyBinding> {
        self.rendered_frame
            .dispatch_tree
            .possible_next_bindings_for_input(input, &self.context_stack())
    }

    fn context_stack_for_focus_handle(
        &self,
        focus_handle: &FocusHandle,
    ) -> Option<Vec<KeyContext>> {
        let dispatch_tree = &self.rendered_frame.dispatch_tree;
        let node_id = dispatch_tree.focusable_node_id(focus_handle.id)?;
        let context_stack: Vec<_> = dispatch_tree
            .dispatch_path(node_id)
            .into_iter()
            .filter_map(|node_id| dispatch_tree.node(node_id).context.clone())
            .collect();
        Some(context_stack)
    }

    /// 返回一个通用事件监听器，使用与给定视图句柄关联的视图和上下文调用给定监听器。
    pub fn listener_for<T: 'static, E>(
        &self,
        view: &Entity<T>,
        f: impl Fn(&mut T, &E, &mut Window, &mut Context<T>) + 'static,
    ) -> impl Fn(&E, &mut Window, &mut App) + 'static {
        let view = view.downgrade();
        move |e: &E, window: &mut Window, cx: &mut App| {
            view.update(cx, |view, cx| f(view, e, window, cx)).ok();
        }
    }

    /// 返回一个通用处理程序，使用与给定视图句柄关联的视图和上下文调用给定处理程序。
    pub fn handler_for<E: 'static, Callback: Fn(&mut E, &mut Window, &mut Context<E>) + 'static>(
        &self,
        entity: &Entity<E>,
        f: Callback,
    ) -> impl Fn(&mut Window, &mut App) + 'static {
        let entity = entity.downgrade();
        move |window: &mut Window, cx: &mut App| {
            entity.update(cx, |entity, cx| f(entity, window, cx)).ok();
        }
    }

    /// 注册一个回调，可以根据返回的布尔值中断当前窗口的关闭。
    /// 如果回调返回 false，窗口将不会被关闭。
    pub fn on_window_should_close(
        &self,
        cx: &App,
        f: impl Fn(&mut Window, &mut App) -> bool + 'static,
    ) {
        let mut cx = self.to_async(cx);
        self.platform_window.on_should_close(Box::new(move || {
            cx.update(|window, cx| f(window, cx)).unwrap_or(true)
        }))
    }

    /// 在此节点上注册下一帧的操作监听器。操作类型
    /// 由给定监听器的第一个参数决定。当下一帧渲染时
    /// 监听器将被清除。
    ///
    /// 这是一个相当底层的方法，除非你有自己注册监听器的特定需求，否则优先使用元素上的操作处理程序。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    pub fn on_action(
        &mut self,
        action_type: TypeId,
        listener: impl Fn(&dyn Any, DispatchPhase, &mut Window, &mut App) + 'static,
    ) {
        self.invalidator.debug_assert_paint();

        self.next_frame
            .dispatch_tree
            .on_action(action_type, Rc::new(listener));
    }

    /// 如果条件为真，在此节点上注册下一帧的捕获操作监听器。
    /// 操作类型由给定监听器的第一个参数决定。当下一帧
    /// 渲染时监听器将被清除。
    ///
    /// 这是一个相当底层的方法，除非你有自己注册监听器的特定需求，否则优先使用元素上的操作处理程序。
    ///
    /// 此方法只能作为元素绘制的绘制阶段的一部分调用。
    pub fn on_action_when(
        &mut self,
        condition: bool,
        action_type: TypeId,
        listener: impl Fn(&dyn Any, DispatchPhase, &mut Window, &mut App) + 'static,
    ) {
        self.invalidator.debug_assert_paint();

        if condition {
            self.next_frame
                .dispatch_tree
                .on_action(action_type, Rc::new(listener));
        }
    }

    /// 读取支持此窗口的 GPU 的信息。
    /// 目前在 Mac 和 Windows 上返回 None。
    pub fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.platform_window.gpu_specs()
    }

    /// 执行标题栏双击操作。
    /// 这是 macOS 特定的。
    pub fn titlebar_double_click(&self) {
        self.platform_window
            .titlebar_double_click(self.is_resizable, self.is_minimizable);
    }

    /// 在平台级别获取窗口的标题。
    /// 这是 macOS 特定的。
    pub fn window_title(&self) -> String {
        self.platform_window.get_title()
    }

    /// 返回所有标签窗口及其标题的列表。
    /// 这是 macOS 特定的。
    pub fn tabbed_windows(&self) -> Option<Vec<SystemWindowTab>> {
        self.platform_window.tabbed_windows()
    }

    /// 返回标签栏的可见性。
    /// 这是 macOS 特定的。
    pub fn tab_bar_visible(&self) -> bool {
        self.platform_window.tab_bar_visible()
    }

    /// 将所有打开的窗口合并到一个标签窗口中。
    /// 这是 macOS 特定的。
    pub fn merge_all_windows(&self) {
        self.platform_window.merge_all_windows()
    }

    /// 将标签移动到新的包含窗口。
    /// 这是 macOS 特定的。
    pub fn move_tab_to_new_window(&self) {
        self.platform_window.move_tab_to_new_window()
    }

    /// 显示或隐藏窗口标签概览。
    /// 这是 macOS 特定的。
    pub fn toggle_window_tab_overview(&self) {
        self.platform_window.toggle_window_tab_overview()
    }

    /// 设置窗口的标签标识符。
    /// 这是 macOS 特定的。
    pub fn set_tabbing_identifier(&self, tabbing_identifier: Option<String>) {
        self.platform_window
            .set_tabbing_identifier(tabbing_identifier)
    }

    /// 请求操作系统播放警报声音。在某些平台上，这与
    /// 窗口关联，对于其他平台则只是一个简单的全局函数调用。
    pub fn play_system_bell(&self) {
        self.platform_window.play_system_bell()
    }

    /// 返回此帧的辅助功能是否处于活动状态，
    /// 即辅助技术（如屏幕阅读器）是否
    /// 已连接并正在构建辅助功能树。
    ///
    /// 使用此方法跳过渲染期间仅
    /// 通过辅助功能树可观察到的数据计算。当辅助功能
    /// 被激活时，会强制重绘，因此门控工作会在
    /// 下一次树更新发送到平台之前重新计算。
    ///
    /// 参见[辅助功能指南](crate::_accessibility)了解概述。
    pub fn is_a11y_active(&self) -> bool {
        self.a11y.is_active()
    }

    /// 上一帧辅助功能信息的调试表示。
    pub fn debug_a11y_tree_json(&self) -> Option<String> {
        self.a11y.debug_tree_json()
    }

    /// 为特定节点上的辅助功能操作注册监听器。
    /// 当屏幕阅读器请求给定操作时，将调用
    /// 由 `node_id` 标识的节点上的操作。
    ///
    /// 参见[辅助功能指南](crate::_accessibility)了解概述。
    pub fn on_a11y_action(
        &mut self,
        node_id: accesskit::NodeId,
        action: accesskit::Action,
        listener: impl FnMut(Option<&accesskit::ActionData>, &mut Window, &mut App) + 'static,
    ) {
        self.a11y
            .action_listeners
            .entry(node_id)
            .or_default()
            .push((action, Box::new(listener)));
    }

    #[cfg(not(target_family = "wasm"))]
    pub(crate) fn handle_a11y_action(&mut self, request: accesskit::ActionRequest, cx: &mut App) {
        // Take listeners out temporarily so the closures can borrow Window
        // mutably, then restore them afterward.
        if let Some(mut listeners) = self.a11y.action_listeners.remove(&request.target_node) {
            let extra_data = request.data.as_ref();
            let mut matched = false;
            for (action, listener) in &mut listeners {
                if *action == request.action {
                    listener(extra_data, self, cx);
                    matched = true;
                }
            }
            self.a11y
                .action_listeners
                .insert(request.target_node, listeners);
            if matched {
                return;
            }
        }

        // Fall back to built-in action handling.
        match request.action {
            accesskit::Action::Click => {
                if let Some(bounds) = self.a11y.node_bounds.get(&request.target_node).copied() {
                    let center = bounds.center();
                    let mouse_down = PlatformInput::MouseDown(crate::MouseDownEvent {
                        button: MouseButton::Left,
                        position: center,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                        first_mouse: false,
                    });
                    let mouse_up = PlatformInput::MouseUp(MouseUpEvent {
                        button: MouseButton::Left,
                        position: center,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                    });
                    self.dispatch_event(mouse_down, cx);
                    self.dispatch_event(mouse_up, cx);
                }
            }
            accesskit::Action::Focus => {
                if let Some(focus_id) = self.a11y.focus_ids.get(&request.target_node).copied()
                    && let Some(handle) = FocusHandle::for_id(focus_id, &cx.focus_handles)
                {
                    self.focus(&handle, cx);
                }
            }
            accesskit::Action::Blur => {
                self.blur();
            }
            _ => {
                log::debug!(
                    "Unhandled a11y action: {:?} on {:?}",
                    request.action,
                    request.target_node
                );
            }
        }
    }

    /// 切换此窗口的检查器模式。
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub fn toggle_inspector(&mut self, cx: &mut App) {
        self.inspector = match self.inspector {
            None => Some(cx.new(|_| Inspector::new())),
            Some(_) => None,
        };
        self.refresh();
    }

    /// 如果窗口处于检查器模式则返回 true。
    pub fn is_inspector_picking(&self, _cx: &App) -> bool {
        #[cfg(any(feature = "inspector", debug_assertions))]
        {
            if let Some(inspector) = &self.inspector {
                return inspector.read(_cx).is_picking();
            }
        }
        false
    }

    /// 使用对检查器状态的可变访问执行提供的函数。
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub fn with_inspector_state<T: 'static, R>(
        &mut self,
        _inspector_id: Option<&crate::InspectorElementId>,
        cx: &mut App,
        f: impl FnOnce(&mut Option<T>, &mut Self) -> R,
    ) -> R {
        if let Some(inspector_id) = _inspector_id
            && let Some(inspector) = &self.inspector
        {
            let inspector = inspector.clone();
            let active_element_id = inspector.read(cx).active_element_id();
            if Some(inspector_id) == active_element_id {
                return inspector.update(cx, |inspector, _cx| {
                    inspector.with_active_element_state(self, f)
                });
            }
        }
        f(&mut None, self)
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(crate) fn build_inspector_element_id(
        &mut self,
        path: crate::InspectorElementPath,
    ) -> crate::InspectorElementId {
        self.invalidator.debug_assert_paint_or_prepaint();
        let path = Rc::new(path);
        let next_instance_id = self
            .next_frame
            .next_inspector_instance_ids
            .entry(path.clone())
            .or_insert(0);
        let instance_id = *next_instance_id;
        *next_instance_id += 1;
        crate::InspectorElementId { path, instance_id }
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn prepaint_inspector(&mut self, inspector_width: Pixels, cx: &mut App) -> Option<AnyElement> {
        if let Some(inspector) = self.inspector.take() {
            let mut inspector_element = AnyView::from(inspector.clone()).into_any_element();
            inspector_element.prepaint_as_root(
                point(self.viewport_size.width - inspector_width, px(0.0)),
                size(inspector_width, self.viewport_size.height).into(),
                self,
                cx,
            );
            self.inspector = Some(inspector);
            Some(inspector_element)
        } else {
            None
        }
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn paint_inspector(&mut self, mut inspector_element: Option<AnyElement>, cx: &mut App) {
        if let Some(mut inspector_element) = inspector_element {
            inspector_element.paint(self, cx);
        };
    }

    /// 注册一个可用于检查器拾取模式的 hitbox，允许用户
    /// 通过点击来选择和检查 UI 元素。
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub fn insert_inspector_hitbox(
        &mut self,
        hitbox_id: HitboxId,
        inspector_id: Option<&crate::InspectorElementId>,
        cx: &App,
    ) {
        self.invalidator.debug_assert_paint_or_prepaint();
        if !self.is_inspector_picking(cx) {
            return;
        }
        if let Some(inspector_id) = inspector_id {
            self.next_frame
                .inspector_hitboxes
                .insert(hitbox_id, inspector_id.clone());
        }
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn paint_inspector_hitbox(&mut self, cx: &App) {
        if let Some(inspector) = self.inspector.as_ref() {
            let inspector = inspector.read(cx);
            if let Some((hitbox_id, _)) = self.hovered_inspector_hitbox(inspector, &self.next_frame)
                && let Some(hitbox) = self
                    .next_frame
                    .hitboxes
                    .iter()
                    .find(|hitbox| hitbox.id == hitbox_id)
            {
                self.paint_quad(crate::fill(hitbox.bounds, crate::rgba(0x61afef4d)));
            }
        }
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn handle_inspector_mouse_event(&mut self, event: &dyn Any, cx: &mut App) {
        let Some(inspector) = self.inspector.clone() else {
            return;
        };
        if event.downcast_ref::<MouseMoveEvent>().is_some() {
            inspector.update(cx, |inspector, _cx| {
                if let Some((_, inspector_id)) =
                    self.hovered_inspector_hitbox(inspector, &self.rendered_frame)
                {
                    inspector.hover(inspector_id, self);
                }
            });
        } else if event.downcast_ref::<crate::MouseDownEvent>().is_some() {
            inspector.update(cx, |inspector, _cx| {
                if let Some((_, inspector_id)) =
                    self.hovered_inspector_hitbox(inspector, &self.rendered_frame)
                {
                    inspector.select(inspector_id, self);
                }
            });
        } else if let Some(event) = event.downcast_ref::<crate::ScrollWheelEvent>() {
            // This should be kept in sync with SCROLL_LINES in x11 platform.
            const SCROLL_LINES: f32 = 3.0;
            const SCROLL_PIXELS_PER_LAYER: f32 = 36.0;
            let delta_y = event
                .delta
                .pixel_delta(px(SCROLL_PIXELS_PER_LAYER / SCROLL_LINES))
                .y;
            if let Some(inspector) = self.inspector.clone() {
                inspector.update(cx, |inspector, _cx| {
                    if let Some(depth) = inspector.pick_depth.as_mut() {
                        *depth += f32::from(delta_y) / SCROLL_PIXELS_PER_LAYER;
                        let max_depth = self.mouse_hit_test.ids.len() as f32 - 0.5;
                        if *depth < 0.0 {
                            *depth = 0.0;
                        } else if *depth > max_depth {
                            *depth = max_depth;
                        }
                        if let Some((_, inspector_id)) =
                            self.hovered_inspector_hitbox(inspector, &self.rendered_frame)
                        {
                            inspector.set_active_element_id(inspector_id, self);
                        }
                    }
                });
            }
        }
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn hovered_inspector_hitbox(
        &self,
        inspector: &Inspector,
        frame: &Frame,
    ) -> Option<(HitboxId, crate::InspectorElementId)> {
        if let Some(pick_depth) = inspector.pick_depth {
            let depth = (pick_depth as i64).try_into().unwrap_or(0);
            let max_skipped = self.mouse_hit_test.ids.len().saturating_sub(1);
            let skip_count = (depth as usize).min(max_skipped);
            for hitbox_id in self.mouse_hit_test.ids.iter().skip(skip_count) {
                if let Some(inspector_id) = frame.inspector_hitboxes.get(hitbox_id) {
                    return Some((*hitbox_id, inspector_id.clone()));
                }
            }
        }
        None
    }

    /// 用于测试：设置当前修饰键状态。
    /// 这不会生成任何事件。
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_modifiers(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
    }

    /// 用于测试：模拟鼠标移动事件到给定位置。
    /// 这通过正常的事件处理路径分发事件，
    /// 这将触发悬停状态和工具提示。
    #[cfg(any(test, feature = "test-support"))]
    pub fn simulate_mouse_move(&mut self, position: Point<Pixels>, cx: &mut App) {
        let event = PlatformInput::MouseMove(MouseMoveEvent {
            position,
            modifiers: self.modifiers,
            pressed_button: None,
        });
        let _ = self.dispatch_event(event, cx);
    }
}

// #[derive(Clone, Copy, Eq, PartialEq, Hash)]
slotmap::new_key_type! {
    /// 窗口的唯一标识符。
    pub struct WindowId;
}

impl WindowId {
    /// 将此窗口 ID 转换为 `u64`。
    pub fn as_u64(&self) -> u64 {
        self.0.as_ffi()
    }
}

impl From<u64> for WindowId {
    fn from(value: u64) -> Self {
        WindowId(slotmap::KeyData::from_ffi(value))
    }
}

/// 具有特定根视图类型的窗口句柄。
/// 注意这不会单独保持窗口存活。
#[derive(Deref, DerefMut)]
pub struct WindowHandle<V> {
    #[deref]
    #[deref_mut]
    pub(crate) any_handle: AnyWindowHandle,
    state_type: PhantomData<fn(V) -> V>,
}

impl<V> Debug for WindowHandle<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowHandle")
            .field("any_handle", &self.any_handle.id.as_u64())
            .finish()
    }
}

impl<V: 'static + Render> WindowHandle<V> {
    /// 从窗口 ID 创建新句柄。
    /// 这不会检查窗口的根类型是否为 `V`。
    pub fn new(id: WindowId) -> Self {
        WindowHandle {
            any_handle: AnyWindowHandle {
                id,
                state_type: TypeId::of::<V>(),
            },
            state_type: PhantomData,
        }
    }

    /// 从此窗口获取根视图。
    ///
    /// 如果窗口已关闭或根视图的类型与 `V` 不匹配，这将失败。
    #[cfg(any(test, feature = "test-support"))]
    pub fn root<C>(&self, cx: &mut C) -> Result<Entity<V>>
    where
        C: AppContext,
    {
        cx.update_window(self.any_handle, |root_view, _, cx| {
            Root::root_view_downcast::<V>(root_view, cx)
                .map_err(|_| anyhow!("the type of the window's root view has changed"))
        })?
    }

    /// 更新此窗口的根视图。
    ///
    /// 如果窗口已关闭或根视图的类型不匹配，这将失败
    pub fn update<C, R>(
        &self,
        cx: &mut C,
        update: impl FnOnce(&mut V, &mut Window, &mut Context<V>) -> R,
    ) -> Result<R>
    where
        C: AppContext,
    {
        cx.update_window(self.any_handle, |root_view, window, cx| {
            let view = Root::root_view_downcast::<V>(root_view, cx)
                .map_err(|_| anyhow!("the type of the window's root view has changed"))?;

            Ok(view.update(cx, |view, cx| update(view, window, cx)))
        })?
    }

    /// 从此窗口读取根视图。
    ///
    /// 如果窗口已关闭或根视图的类型与 `V` 不匹配，这将失败。
    pub fn read<'a>(&self, cx: &'a App) -> Result<&'a V> {
        let x = cx
            .windows
            .get(self.id)
            .and_then(|window| {
                window
                    .as_deref()
                    .and_then(|window| window.root.clone())
                    .map(|root_view| Root::root_view_downcast::<V>(root_view, cx))
            })
            .context("window not found")?
            .map_err(|_| anyhow!("the type of the window's root view has changed"))?;

        Ok(x.read(cx))
    }

    /// 通过回调从此窗口读取根视图
    ///
    /// 如果窗口已关闭或根视图的类型与 `V` 不匹配，这将失败。
    pub fn read_with<C, R>(&self, cx: &C, read_with: impl FnOnce(&V, &App) -> R) -> Result<R>
    where
        C: AppContext,
    {
        cx.read_window(self, |root_view, cx| read_with(root_view.read(cx), cx))
    }

    /// 从此窗口读取根视图指针。
    ///
    /// 如果窗口已关闭或根视图的类型与 `V` 不匹配，这将失败。
    pub fn entity<C>(&self, cx: &C) -> Result<Entity<V>>
    where
        C: AppContext,
    {
        cx.read_window(self, |root_view, _cx| root_view)
    }

    /// 检查此窗口是否为"活动"状态。
    ///
    /// 如果窗口已关闭或当前
    /// 被借用，将返回 `None`。
    pub fn is_active(&self, cx: &mut App) -> Option<bool> {
        cx.update_window(self.any_handle, |_, window, _| window.is_window_active())
            .ok()
    }
}

impl<V> Copy for WindowHandle<V> {}

impl<V> Clone for WindowHandle<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V> PartialEq for WindowHandle<V> {
    fn eq(&self, other: &Self) -> bool {
        self.any_handle == other.any_handle
    }
}

impl<V> Eq for WindowHandle<V> {}

impl<V> Hash for WindowHandle<V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.any_handle.hash(state);
    }
}

impl<V: 'static> From<WindowHandle<V>> for AnyWindowHandle {
    fn from(val: WindowHandle<V>) -> Self {
        val.any_handle
    }
}

/// 具有任意根视图类型的窗口句柄，可以向下转型为具有特定根视图类型的窗口。
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct AnyWindowHandle {
    pub(crate) id: WindowId,
    state_type: TypeId,
}

impl AnyWindowHandle {
    /// 获取此窗口的 ID。
    pub fn window_id(&self) -> WindowId {
        self.id
    }

    /// 尝试将此句柄转换为具有特定根视图类型的窗口句柄。
    /// 如果类型不匹配，将返回 `None`。
    pub fn downcast<T: 'static>(&self) -> Option<WindowHandle<T>> {
        if TypeId::of::<T>() == self.state_type {
            Some(WindowHandle {
                any_handle: *self,
                state_type: PhantomData,
            })
        } else {
            None
        }
    }

    /// 更新此窗口根视图的状态。
    ///
    /// 如果窗口已关闭，这将失败。
    pub fn update<C, R>(
        self,
        cx: &mut C,
        update: impl FnOnce(AnyView, &mut Window, &mut App) -> R,
    ) -> Result<R>
    where
        C: AppContext,
    {
        cx.update_window(self, update)
    }

    /// 读取此窗口根视图的状态。
    ///
    /// 如果窗口已关闭，这将失败。
    pub fn read<T, C, R>(self, cx: &C, read: impl FnOnce(Entity<T>, &App) -> R) -> Result<R>
    where
        C: AppContext,
        T: 'static,
    {
        let view = self
            .downcast::<T>()
            .context("the type of the window's root view has changed")?;

        cx.read_window(&view, read)
    }
}

impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        self.platform_window.window_handle()
    }
}

impl HasDisplayHandle for Window {
    fn display_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::DisplayHandle<'_>, HandleError> {
        self.platform_window.display_handle()
    }
}

/// [`Element`] 的标识符。
///
/// 可以用字符串、数字或两者构造，以及其他内部表示。
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ElementId {
    /// 视图元素的 ID
    View(EntityId),
    /// 整数 ID。
    Integer(u64),
    /// 基于字符串的 ID。
    Name(SharedString),
    /// UUID。
    Uuid(Uuid),
    /// 与焦点句柄等同的 ID。
    FocusHandle(FocusId),
    /// 名称和整数的组合。
    NamedInteger(SharedString, u64),
    /// 路径。
    Path(Arc<std::path::Path>),
    /// 代码位置。
    CodeLocation(core::panic::Location<'static>),
    /// 元素的带标签子元素。
    NamedChild(Arc<ElementId>, SharedString),
    /// 字节数组 ID（用于文本锚点）
    OpaqueId([u8; 20]),
}

impl ElementId {
    /// 从名称和 `usize` 构造 `ElementId::NamedInteger`。
    pub fn named_usize(name: impl Into<SharedString>, integer: usize) -> ElementId {
        Self::NamedInteger(name.into(), integer as u64)
    }
}

impl Display for ElementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElementId::View(entity_id) => write!(f, "view-{}", entity_id)?,
            ElementId::Integer(ix) => write!(f, "{}", ix)?,
            ElementId::Name(name) => write!(f, "{}", name)?,
            ElementId::FocusHandle(_) => write!(f, "FocusHandle")?,
            ElementId::NamedInteger(s, i) => write!(f, "{}-{}", s, i)?,
            ElementId::Uuid(uuid) => write!(f, "{}", uuid)?,
            ElementId::Path(path) => write!(f, "{}", path.display())?,
            ElementId::CodeLocation(location) => write!(f, "{}", location)?,
            ElementId::NamedChild(id, name) => write!(f, "{}-{}", id, name)?,
            ElementId::OpaqueId(opaque_id) => write!(f, "{:x?}", opaque_id)?,
        }

        Ok(())
    }
}

impl TryInto<SharedString> for ElementId {
    type Error = anyhow::Error;

    fn try_into(self) -> anyhow::Result<SharedString> {
        if let ElementId::Name(name) = self {
            Ok(name)
        } else {
            anyhow::bail!("element id is not string")
        }
    }
}

impl From<usize> for ElementId {
    fn from(id: usize) -> Self {
        ElementId::Integer(id as u64)
    }
}

impl From<i32> for ElementId {
    fn from(id: i32) -> Self {
        Self::Integer(id as u64)
    }
}

impl From<SharedString> for ElementId {
    fn from(name: SharedString) -> Self {
        ElementId::Name(name)
    }
}

impl From<String> for ElementId {
    fn from(name: String) -> Self {
        ElementId::Name(name.into())
    }
}

impl From<Arc<str>> for ElementId {
    fn from(name: Arc<str>) -> Self {
        ElementId::Name(name.into())
    }
}

impl From<Arc<std::path::Path>> for ElementId {
    fn from(path: Arc<std::path::Path>) -> Self {
        ElementId::Path(path)
    }
}

impl From<&'static str> for ElementId {
    fn from(name: &'static str) -> Self {
        ElementId::Name(SharedString::new_static(name))
    }
}

impl<'a> From<&'a FocusHandle> for ElementId {
    fn from(handle: &'a FocusHandle) -> Self {
        ElementId::FocusHandle(handle.id)
    }
}

impl From<(&'static str, EntityId)> for ElementId {
    fn from((name, id): (&'static str, EntityId)) -> Self {
        ElementId::NamedInteger(SharedString::new_static(name), id.as_u64())
    }
}

impl From<(&'static str, usize)> for ElementId {
    fn from((name, id): (&'static str, usize)) -> Self {
        ElementId::NamedInteger(SharedString::new_static(name), id as u64)
    }
}

impl From<(SharedString, usize)> for ElementId {
    fn from((name, id): (SharedString, usize)) -> Self {
        ElementId::NamedInteger(name, id as u64)
    }
}

impl From<(&'static str, u64)> for ElementId {
    fn from((name, id): (&'static str, u64)) -> Self {
        ElementId::NamedInteger(SharedString::new_static(name), id)
    }
}

impl From<Uuid> for ElementId {
    fn from(value: Uuid) -> Self {
        Self::Uuid(value)
    }
}

impl From<(&'static str, u32)> for ElementId {
    fn from((name, id): (&'static str, u32)) -> Self {
        ElementId::NamedInteger(SharedString::new_static(name), u64::from(id))
    }
}

impl<T: Into<SharedString>> From<(ElementId, T)> for ElementId {
    fn from((id, name): (ElementId, T)) -> Self {
        ElementId::NamedChild(Arc::new(id), name.into())
    }
}

impl From<&'static core::panic::Location<'static>> for ElementId {
    fn from(location: &'static core::panic::Location<'static>) -> Self {
        ElementId::CodeLocation(*location)
    }
}

impl From<[u8; 20]> for ElementId {
    fn from(opaque_id: [u8; 20]) -> Self {
        ElementId::OpaqueId(opaque_id)
    }
}

/// 在窗口中以给定位置和大小渲染的矩形。
/// 作为参数传递给 [`Window::paint_quad`]。
#[derive(Clone, Default)]
pub struct PaintQuad {
    /// 四边形在窗口中的边界。
    pub bounds: Bounds<Pixels>,
    /// 四边形的圆角半径。
    pub corner_radii: Corners<Pixels>,
    /// 四边形的背景颜色。
    pub background: Background,
    /// 四边形的边框宽度。
    pub border_widths: Edges<Pixels>,
    /// 四边形的边框颜色。
    pub border_color: Hsla,
    /// 四边形的边框样式。
    pub border_style: BorderStyle,
}

impl PaintQuad {
    /// 设置四边形的圆角半径。
    pub fn corner_radii(self, corner_radii: impl Into<Corners<Pixels>>) -> Self {
        PaintQuad {
            corner_radii: corner_radii.into(),
            ..self
        }
    }

    /// 设置四边形的边框宽度。
    pub fn border_widths(self, border_widths: impl Into<Edges<Pixels>>) -> Self {
        PaintQuad {
            border_widths: border_widths.into(),
            ..self
        }
    }

    /// 设置四边形的边框颜色。
    pub fn border_color(self, border_color: impl Into<Hsla>) -> Self {
        PaintQuad {
            border_color: border_color.into(),
            ..self
        }
    }

    /// 设置四边形的背景颜色。
    pub fn background(self, background: impl Into<Background>) -> Self {
        PaintQuad {
            background: background.into(),
            ..self
        }
    }
}

/// 使用给定参数创建四边形。
pub fn quad(
    bounds: Bounds<Pixels>,
    corner_radii: impl Into<Corners<Pixels>>,
    background: impl Into<Background>,
    border_widths: impl Into<Edges<Pixels>>,
    border_color: impl Into<Hsla>,
    border_style: BorderStyle,
) -> PaintQuad {
    PaintQuad {
        bounds,
        corner_radii: corner_radii.into(),
        background: background.into(),
        border_widths: border_widths.into(),
        border_color: border_color.into(),
        border_style,
    }
}

/// 使用给定边界和背景颜色创建填充四边形。
pub fn fill(bounds: impl Into<Bounds<Pixels>>, background: impl Into<Background>) -> PaintQuad {
    PaintQuad {
        bounds: bounds.into(),
        corner_radii: (0.).into(),
        background: background.into(),
        border_widths: (0.).into(),
        border_color: transparent_black(),
        border_style: BorderStyle::default(),
    }
}

/// 使用给定边界、边框颜色和 1px 边框宽度创建矩形轮廓
pub fn outline(
    bounds: impl Into<Bounds<Pixels>>,
    border_color: impl Into<Hsla>,
    border_style: BorderStyle,
) -> PaintQuad {
    PaintQuad {
        bounds: bounds.into(),
        corner_radii: (0.).into(),
        background: transparent_black().into(),
        border_widths: (1.).into(),
        border_color: border_color.into(),
        border_style,
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use crate::{
        AppContext as _, Bounds, Context, FocusHandle, InteractiveElement as _, IntoElement,
        ParentElement, Pixels, Render, Styled, TestAppContext, Window, WindowOptions, canvas, div,
        px, size,
    };

    struct EmptyView;

    impl Render for EmptyView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    struct OpensWindowOnPaint {
        opened: Rc<Cell<bool>>,
    }

    impl Render for OpensWindowOnPaint {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let opened = self.opened.clone();
            div()
                .size_full()
                .child(canvas(
                    |_, _, _| {},
                    move |_, _, _window, cx| {
                        if !opened.replace(true) {
                            cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| EmptyView))
                                .unwrap();
                        }
                    },
                ))
                // Siblings painted after the canvas: their elements were
                // allocated in the arena before the nested draw, so they detect
                // a mid-draw arena clear when painted afterwards.
                .child(div().child("after"))
        }
    }

    /// 打开窗口会同步绘制并请求一个元素 arena
    /// 清理。当在另一个窗口的绘制中发生时（此处：来自
    /// 元素的绘制），清理必须延迟到外层绘制
    /// 完成，否则外层绘制的 arena 分配的元素将被
    /// 释放。
    #[test]
    fn test_window_opened_during_draw_defers_arena_clear() {
        let mut cx = TestAppContext::single();

        let opened = Rc::new(Cell::new(false));
        // add_window draws once, which runs the nested open_window mid-draw.
        let window = cx.add_window({
            let opened = opened.clone();
            move |_, _| OpensWindowOnPaint { opened }
        });

        assert!(opened.get());
        assert_eq!(cx.windows().len(), 2);

        // The deferred clear must actually run once the outer draw unwinds:
        // subsequent draws of both windows work against a fresh arena.
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    }

    struct RootView {
        explicit_size: bool,
        child_bounds: Rc<Cell<Bounds<Pixels>>>,
    }

    impl Render for RootView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let child_bounds = self.child_bounds.clone();
            let root = div().flex().flex_col().child(
                canvas(
                    move |bounds, _, _| child_bounds.set(bounds),
                    |_, _, _, _| {},
                )
                .size_full(),
            );
            if self.explicit_size {
                root.w(px(300.)).h(px(200.))
            } else {
                root
            }
        }
    }

    #[test]
    fn auto_sized_window_root_fills_the_window() {
        let mut cx = TestAppContext::single();
        let child_bounds = Rc::new(Cell::new(Bounds::default()));
        let window = cx.add_window({
            let child_bounds = child_bounds.clone();
            move |_, _| RootView {
                explicit_size: false,
                child_bounds,
            }
        });

        let viewport_size = cx
            .update_window(window.into(), |_, window, cx| {
                window.draw(cx).clear(cx);
                window.viewport_size()
            })
            .unwrap();

        assert_eq!(child_bounds.get().size, viewport_size);
    }

    #[test]
    fn explicitly_sized_window_root_keeps_its_size() {
        let mut cx = TestAppContext::single();
        let child_bounds = Rc::new(Cell::new(Bounds::default()));
        let window = cx.add_window({
            let child_bounds = child_bounds.clone();
            move |_, _| RootView {
                explicit_size: true,
                child_bounds,
            }
        });

        cx.update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
        })
        .unwrap();

        assert_eq!(child_bounds.get().size, size(px(300.), px(200.)));
    }

    struct FocusForwarder {
        a: FocusHandle,
        b: FocusHandle,
    }

    impl Render for FocusForwarder {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(div().w(px(50.)).h(px(50.)).track_focus(&self.a))
                .child(div().w(px(50.)).h(px(50.)).track_focus(&self.b))
        }
    }

    /// 当焦点监听器再次移动焦点时（例如停靠栏将焦点转发到其
    /// 活动面板），产生的焦点事件必须在不等待
    /// 不相关的窗口重绘的情况下分发。
    #[rgpui::test]
    fn test_focus_moved_by_focus_listener_is_dispatched(cx: &mut TestAppContext) {
        let b_focus_count = Rc::new(Cell::new(0));
        let window = cx.add_window({
            let b_focus_count = b_focus_count.clone();
            move |window, cx| {
                let a = cx.focus_handle();
                let b = cx.focus_handle();
                cx.on_focus(&a, window, |this: &mut FocusForwarder, window, cx| {
                    let b = this.b.clone();
                    window.focus(&b, cx);
                })
                .detach();
                cx.on_focus(&b, window, move |_, _, _| {
                    b_focus_count.set(b_focus_count.get() + 1);
                })
                .detach();
                FocusForwarder { a, b }
            }
        });

        window
            .update(cx, |_, window, _| window.activate_window())
            .unwrap();
        cx.executor().run_until_parked();

        window
            .update(cx, |this, window, cx| {
                let a = this.a.clone();
                window.focus(&a, cx);
            })
            .unwrap();
        cx.executor().run_until_parked();

        window
            .update(cx, |this, window, _| {
                assert!(this.b.is_focused(window));
            })
            .unwrap();
        assert_eq!(b_focus_count.get(), 1);
    }
}
