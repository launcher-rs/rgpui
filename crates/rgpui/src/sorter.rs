//! 缩略图排序器 —— 可拖拽重排的缩略图网格（幻灯片 / 页面 / 图片）。
//!
//! 面向「一堆等尺寸缩略图 + 拖拽重排」的场景：自动换行网格、单选 / 范围选择 /
//! 多选、缩放、键盘导航，以及**拖拽重排**（拖拽过程中在插入位置画一条指示线）。
//! 框架只负责控件与交互，缩略图内容与项目顺序由应用通过 [`SorterDelegate`] 提供；
//! 重排只抛出 [`SorterEvent::Reorder`]（多选拖动时是 [`SorterEvent::ReorderMany`]），
//! 真正移动数据由应用完成（见下方示例）。拖到视口上下边缘的自动滚动由
//! [`SorterState::drag_autoscroll`] 提供逻辑，应用按帧驱动即可。
//!
//! 与 [`DataTable`](crate::DataTable) / [`VirtualList`](crate::VirtualList) 的区别：
//! 后两者面向「有序行」且不提供重排，本控件面向「二维缩略图墙」并提供拖拽重排、
//! 缩放与多选。
//!
//! ## 用法
//!
//! ```ignore
//! use rgpui::sorter::{Sorter, SorterDelegate, SorterEvent, SorterState};
//!
//! struct Slides {
//!     items: Vec<String>,
//! }
//!
//! impl SorterDelegate for Slides {
//!     fn count(&self, _: &App) -> usize {
//!         self.items.len()
//!     }
//!
//!     fn render_item(
//!         &mut self,
//!         index: usize,
//!         _selected: bool,
//!         _window: &mut Window,
//!         _cx: &mut Context<SorterState<Self>>,
//!     ) -> impl IntoElement {
//!         div().size_full().child(self.items[index].clone())
//!     }
//! }
//!
//! let state = cx.new(|cx| SorterState::new(Slides { items }, cx));
//! cx.subscribe_in(&state, window, |this, state, event: &SorterEvent, window, cx| {
//!     if let SorterEvent::Reorder { from, to } = event {
//!         state.update(cx, |state, cx| {
//!             let item = state.delegate_mut().items.remove(*from);
//!             state.delegate_mut().items.insert(*to, item);
//!             cx.notify();
//!         });
//!     }
//! })
//! .detach();
//!
//! // 视图树中：
//! Sorter::new(&state)
//! ```

use std::collections::BTreeSet;

use crate::{
    ActiveTheme as _, AnyElement, App, Bounds, Context, ElementExt as _, Entity, EventEmitter,
    FocusHandle, Focusable, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point, Render,
    RenderOnce, ScrollHandle, Scrollbar, Size, StatefulInteractiveElement as _, Styled as _,
    Window, div, px, size,
};

/// 缩略图基准宽度（缩放系数为 1.0 时的像素宽度）。
pub const DEFAULT_CELL_WIDTH: Pixels = px(160.0);
/// 缩略图之间以及网格四周的默认间距（像素）。
pub const DEFAULT_GAP: Pixels = px(12.0);
/// 最小缩放系数。
pub const MIN_ZOOM: f32 = 0.4;
/// 最大缩放系数。
pub const MAX_ZOOM: f32 = 2.5;
/// 默认单元格宽高比（16:9）。
pub const DEFAULT_ASPECT_RATIO: f32 = 16.0 / 9.0;
/// 拖拽自动滚动的触边距离（距视口上下边缘多少像素内开始滚动）。
pub const AUTOSCROLL_EDGE: Pixels = px(48.0);
/// 拖拽自动滚动的最快速度（每帧滚动多少像素，贴着边缘时取到）。
pub const AUTOSCROLL_MAX_SPEED: Pixels = px(18.0);

/// 把缩放系数夹取到 [`MIN_ZOOM`]..=[`MAX_ZOOM`]，非有限值回落为 `1.0`。
pub fn clamp_zoom(zoom: f32) -> f32 {
    if zoom.is_finite() {
        zoom.clamp(MIN_ZOOM, MAX_ZOOM)
    } else {
        1.0
    }
}

/// 归一化宽高比：非有限值或非正数回落为 [`DEFAULT_ASPECT_RATIO`]。
pub fn normalize_aspect_ratio(aspect_ratio: f32) -> f32 {
    if aspect_ratio.is_finite() && aspect_ratio > 0.0 {
        aspect_ratio
    } else {
        DEFAULT_ASPECT_RATIO
    }
}

/// 缩略图单元格尺寸：宽 = 基准宽 × 夹取后的缩放，高 = 宽 ÷ 宽高比。
pub fn cell_size(base_width: Pixels, zoom: f32, aspect_ratio: f32) -> Size<Pixels> {
    let width = base_width * clamp_zoom(zoom);
    size(width, width * (1.0 / normalize_aspect_ratio(aspect_ratio)))
}

/// 一行能放下的缩略图数量（至少 1）。
///
/// 按 `floor((可用宽 + 间距) / (格宽 + 间距))` 计算，保证与 `flex-wrap` 的换行一致。
pub fn grid_columns(available_width: Pixels, cell_width: Pixels, gap: Pixels) -> usize {
    let step = (cell_width + gap).as_f32();
    if step <= 0.0 {
        return 1;
    }
    let columns = ((available_width.as_f32() + gap.as_f32()) / step).floor();
    if columns.is_finite() && columns >= 1.0 {
        columns as usize
    } else {
        1
    }
}

/// 根据指针位置与各单元格边界求插入槽（取值 `0..=count`）。
///
/// 命中某个单元格时：指针在其水平中线右侧返回 `index + 1`，否则返回 `index`；
/// 未命中任何单元格（如落在网格空白处）返回 `cells.len()`（排到末尾）。
pub fn drop_slot(cells: &[Bounds<Pixels>], pointer: Point<Pixels>) -> usize {
    for (index, bounds) in cells.iter().enumerate() {
        if bounds.contains(&pointer) {
            let center_x = bounds.origin.x + bounds.size.width * 0.5;
            return if pointer.x > center_x {
                index + 1
            } else {
                index
            };
        }
    }
    cells.len()
}

/// 把「拖拽源索引 + 插入槽」换算成移动后的目标索引；无需移动时返回 `None`。
///
/// 插入槽等于源位置本身或紧随其后时表示没有实际位移。
pub fn move_dest(from: usize, slot: usize) -> Option<usize> {
    if slot == from || slot == from + 1 {
        None
    } else if slot > from {
        Some(slot - 1)
    } else {
        Some(slot)
    }
}

/// 拖拽到视口上下边缘时的自动滚动速度（每帧像素，正数向下、负数向上）。
///
/// 指针进入距边缘 `edge` 的带状区域后开始滚动，越贴近边缘越快，贴边时取到
/// `max_speed`；不在边缘区域时返回 0。自动滚动由应用按帧重复调用
/// [`SorterState::drag_autoscroll`] 驱动，组件本身不持有定时器。
pub fn autoscroll_velocity(
    pointer_y: Pixels,
    viewport: Bounds<Pixels>,
    edge: Pixels,
    max_speed: Pixels,
) -> Pixels {
    let top = viewport.origin.y.as_f32();
    let bottom = top + viewport.size.height.as_f32();
    let edge = edge.as_f32().max(1.0);
    let max_speed = max_speed.as_f32();
    let y = pointer_y.as_f32();

    if y < top + edge {
        let intensity = ((top + edge - y) / edge).clamp(0.0, 1.0);
        px(-max_speed * intensity)
    } else if y > bottom - edge {
        let intensity = ((y - (bottom - edge)) / edge).clamp(0.0, 1.0);
        px(max_speed * intensity)
    } else {
        px(0.0)
    }
}

/// 多选整组拖动：把插入槽换算成「整组落位后的起始索引」。
///
/// `selected` 必须升序且非空。槽位落在选中集合首项之后、末项及其后一位之前时返回
/// `None`：整组拖回自己身上没有意义。返回值是**抽走选中项之后**的列表里，整组第一项
/// 应处的索引。
pub fn move_group_dest(selected: &[usize], slot: usize) -> Option<usize> {
    let first = *selected.first()?;
    let last = *selected.last()?;
    if slot == first || (slot > first && slot <= last + 1) {
        return None;
    }

    // 目标槽前面的选中项会被抽走，落位索引要相应前移
    let removed_before = selected.iter().filter(|&&index| index < slot).count();
    Some(slot - removed_before)
}

/// 排序器抛出的事件（应用经 `subscribe` 接收）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SorterEvent {
    /// 选中集合变化（升序索引列表）。
    SelectionChanged(Vec<usize>),
    /// 请求重排：把 `from` 处的项目移动到 `to`（移动后的索引）。
    Reorder {
        /// 拖拽源索引。
        from: usize,
        /// 移动后的目标索引。
        to: usize,
    },
    /// 请求整组重排：把 `from` 里的项目（升序）整体移动到 `to` 起始的位置。
    ///
    /// 应用按「先按索引升序抽出这些项目，再插入到 `to`」的方式重排数据。
    ReorderMany {
        /// 整组项目的原索引，升序。
        from: Vec<usize>,
        /// 抽走后的列表中，整组第一项的目标索引。
        to: usize,
    },
    /// 激活某一项（双击 / `Enter` / `Space`）。
    Activate(usize),
    /// 请求删除选中项（`Delete` / `Backspace`）。
    Delete(Vec<usize>),
}

/// 为排序器提供项目数量、宽高比与缩略图渲染的代理 trait。
pub trait SorterDelegate: Sized + 'static {
    /// 项目数量。
    fn count(&self, cx: &App) -> usize;

    /// 单元格宽高比（宽 ÷ 高），默认 [`DEFAULT_ASPECT_RATIO`]。
    fn aspect_ratio(&self, _cx: &App) -> f32 {
        DEFAULT_ASPECT_RATIO
    }

    /// 渲染第 `index` 个缩略图内容（填充单元格，`selected` 为选中态）。
    ///
    /// 默认渲染项目序号。
    fn render_item(
        &mut self,
        index: usize,
        _selected: bool,
        _window: &mut Window,
        _cx: &mut Context<SorterState<Self>>,
    ) -> impl IntoElement {
        div().size_full().child((index + 1).to_string())
    }
}

/// 排序器状态：持有委托、选中集合、缩放、拖拽与滚动。
pub struct SorterState<D: SorterDelegate> {
    /// 数据 / 渲染委托。
    delegate: D,
    /// 选中项索引（升序）。
    selected: BTreeSet<usize>,
    /// 范围选择的锚点。
    anchor: Option<usize>,
    /// 缩放系数（见 [`MIN_ZOOM`] / [`MAX_ZOOM`]）。
    zoom: f32,
    /// 是否在缩略图左上角显示序号。
    show_index: bool,
    /// 正在拖拽的项目索引。
    dragging: Option<usize>,
    /// 拖拽期间的插入槽。
    hover_slot: Option<usize>,
    /// 拖拽期间的指针位置（窗口坐标；拖拽预览用）。
    drag_position: Option<Point<Pixels>>,
    /// 最近一次预绘制的各单元格边界（窗口坐标，随滚动更新）。
    cell_bounds: Vec<Bounds<Pixels>>,
    /// 最近一次预绘制的根元素边界（即视口）。
    bounds: Bounds<Pixels>,
    /// 每行列数（最近一次渲染缓存，键盘导航用）。
    columns: usize,
    /// 单元格基准宽。
    base_width: Pixels,
    /// 单元格间距（同时用作网格内边距）。
    gap: Pixels,
    /// 网格滚动句柄。
    scroll: ScrollHandle,
    /// 焦点句柄。
    focus_handle: FocusHandle,
}

impl<D: SorterDelegate> SorterState<D> {
    /// 创建排序器状态。
    pub fn new(delegate: D, cx: &mut Context<Self>) -> Self {
        Self {
            delegate,
            selected: BTreeSet::new(),
            anchor: None,
            zoom: 1.0,
            show_index: true,
            dragging: None,
            hover_slot: None,
            drag_position: None,
            cell_bounds: Vec::new(),
            bounds: Bounds::default(),
            columns: 1,
            base_width: DEFAULT_CELL_WIDTH,
            gap: DEFAULT_GAP,
            scroll: ScrollHandle::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    /// 数据委托。
    pub fn delegate(&self) -> &D {
        &self.delegate
    }

    /// 数据委托（可变）。
    pub fn delegate_mut(&mut self) -> &mut D {
        &mut self.delegate
    }

    /// 项目数量（实时向委托查询）。
    pub fn count(&self, cx: &App) -> usize {
        self.delegate.count(cx)
    }

    /// 选中项索引（升序）。
    pub fn selected(&self) -> &BTreeSet<usize> {
        &self.selected
    }

    /// 主选中项（最小索引）。
    pub fn primary(&self) -> Option<usize> {
        self.selected.iter().next().copied()
    }

    /// 某项是否被选中。
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected.contains(&index)
    }

    /// 选中项数量。
    pub fn selection_len(&self) -> usize {
        self.selected.len()
    }

    /// 当前缩放系数。
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// 设置缩放系数（自动夹取到 [`MIN_ZOOM`]..=[`MAX_ZOOM`]）。
    pub fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        let zoom = clamp_zoom(zoom);
        if (self.zoom - zoom).abs() > f32::EPSILON {
            self.zoom = zoom;
            cx.notify();
        }
    }

    /// 按 `delta` 增减缩放系数。
    pub fn zoom_by(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.set_zoom(self.zoom + delta, cx);
    }

    /// 是否显示序号角标。
    pub fn show_index(&self) -> bool {
        self.show_index
    }

    /// 设置是否显示序号角标。
    pub fn set_show_index(&mut self, show: bool, cx: &mut Context<Self>) {
        if self.show_index != show {
            self.show_index = show;
            cx.notify();
        }
    }

    /// 单元格基准宽。
    pub fn base_width(&self) -> Pixels {
        self.base_width
    }

    /// 设置单元格基准宽。
    pub fn set_base_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        if self.base_width != width {
            self.base_width = width;
            cx.notify();
        }
    }

    /// 单元格间距。
    pub fn gap(&self) -> Pixels {
        self.gap
    }

    /// 设置单元格间距（同时用作网格内边距）。
    pub fn set_gap(&mut self, gap: Pixels, cx: &mut Context<Self>) {
        if self.gap != gap {
            self.gap = gap;
            cx.notify();
        }
    }

    /// 每行列数（最近一次渲染缓存）。
    pub fn columns(&self) -> usize {
        self.columns
    }

    /// 是否正在拖拽。
    pub fn is_dragging(&self) -> bool {
        self.dragging.is_some()
    }

    /// 聚焦排序器。
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
    }

    /// 单选某一项（越界忽略）。
    pub fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.delegate.count(cx) {
            return;
        }
        self.anchor = Some(index);
        if self.selected.len() == 1 && self.selected.contains(&index) {
            return;
        }
        self.selected.clear();
        self.selected.insert(index);
        self.emit_selection(cx);
    }

    /// 切换某项的选中态（多选）。
    pub fn toggle(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.delegate.count(cx) {
            return;
        }
        if !self.selected.remove(&index) {
            self.selected.insert(index);
        }
        self.anchor = Some(index);
        self.emit_selection(cx);
    }

    /// 以 `anchor`..=`cursor` 的范围选中（顺序无关）。
    pub fn select_range(&mut self, anchor: usize, cursor: usize, cx: &mut Context<Self>) {
        let count = self.delegate.count(cx);
        if count == 0 {
            return;
        }
        let last = count - 1;
        let (start, end) = (anchor.min(cursor).min(last), anchor.max(cursor).min(last));
        let next: BTreeSet<usize> = (start..=end).collect();
        self.anchor = Some(anchor.min(last));
        if next != self.selected {
            self.selected = next;
            self.emit_selection(cx);
        }
    }

    /// 全选。
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        let count = self.delegate.count(cx);
        let next: BTreeSet<usize> = (0..count).collect();
        if next != self.selected {
            self.selected = next;
            self.emit_selection(cx);
        }
    }

    /// 清空选择。
    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            return;
        }
        self.selected.clear();
        self.emit_selection(cx);
    }

    /// 开始拖拽某一项。
    pub fn begin_drag(&mut self, index: usize, cx: &mut Context<Self>) {
        self.dragging = Some(index);
        self.hover_slot = Some(index);
        cx.notify();
    }

    /// 设置拖拽插入槽（应用可直接按槽位驱动，也可用 [`Self::update_drag`] 由指针推算）。
    pub fn set_drop_slot(&mut self, slot: usize, cx: &mut Context<Self>) {
        if self.dragging.is_none() {
            return;
        }
        if self.hover_slot != Some(slot) {
            self.hover_slot = Some(slot);
            cx.notify();
        }
    }

    /// 按指针位置更新插入槽（拖拽过程中由鼠标移动驱动）。
    pub fn update_drag(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if self.dragging.is_none() {
            return;
        }
        self.drag_position = Some(position);
        let slot = drop_slot(&self.cell_bounds, position);
        self.set_drop_slot(slot, cx);
    }

    /// 拖拽边缘自动滚动：按指针位置滚动视口，返回本帧的滚动增量。
    ///
    /// 应用在拖拽期间用前台定时器每帧调用一次；返回 [`px(0.0)`] 表示指针不在边缘
    /// 区域，可以停掉定时器。滚动后顺带重算插入槽。
    pub fn drag_autoscroll(&mut self, pointer: Point<Pixels>, cx: &mut Context<Self>) -> Pixels {
        if self.dragging.is_none() {
            return px(0.0);
        }

        let delta = autoscroll_velocity(
            pointer.y,
            self.bounds,
            AUTOSCROLL_EDGE,
            AUTOSCROLL_MAX_SPEED,
        );
        if delta == px(0.0) {
            return delta;
        }

        let mut offset = self.scroll.offset();
        offset.y = offset.y + delta;
        self.scroll.set_offset(offset);
        self.update_drag(pointer, cx);
        cx.notify();
        delta
    }

    /// 结束拖拽：位移有效时抛出 [`SorterEvent::Reorder`]；拖的是多选中的一项时
    /// 抛 [`SorterEvent::ReorderMany`]，整组一起移动。
    pub fn finish_drag(&mut self, cx: &mut Context<Self>) {
        let Some(from) = self.dragging.take() else {
            return;
        };
        self.drag_position = None;
        let slot = self.hover_slot.take().unwrap_or(from);

        if self.selected.len() > 1 && self.selected.contains(&from) {
            let indices: Vec<usize> = self.selected.iter().copied().collect();
            if let Some(to) = move_group_dest(&indices, slot) {
                cx.emit(SorterEvent::ReorderMany { from: indices, to });
            }
        } else if let Some(to) = move_dest(from, slot) {
            cx.emit(SorterEvent::Reorder { from, to });
        }
        cx.notify();
    }

    /// 取消拖拽（不抛出事件）。
    pub fn cancel_drag(&mut self, cx: &mut Context<Self>) {
        self.drag_position = None;
        if self.dragging.take().is_some() || self.hover_slot.take().is_some() {
            cx.notify();
        }
    }

    /// 滚动到指定项使其可见。
    pub fn scroll_to_item(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(item) = self.cell_bounds.get(index).copied() else {
            return;
        };
        let viewport_height = self.bounds.size.height.as_f32();
        if viewport_height <= 0.0 {
            return;
        }
        let rel_top = item.origin.y.as_f32() - self.bounds.origin.y.as_f32();
        let rel_bottom = rel_top + item.size.height.as_f32();
        let mut offset = self.scroll.offset();
        if rel_top < 0.0 {
            offset.y = offset.y + px(rel_top);
        } else if rel_bottom > viewport_height {
            offset.y = offset.y - px(rel_bottom - viewport_height);
        }
        self.scroll.set_offset(offset);
        cx.notify();
    }

    /// 抛出选中变化事件并重绘。
    fn emit_selection(&mut self, cx: &mut Context<Self>) {
        let indices: Vec<usize> = self.selected.iter().copied().collect();
        cx.emit(SorterEvent::SelectionChanged(indices));
        cx.notify();
    }

    /// 单元格鼠标按下：选择 + 双击激活 + 进入拖拽。
    fn on_item_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus(window, cx);
        let command = event.modifiers.control || event.modifiers.platform;

        if event.click_count >= 2 {
            self.select(index, cx);
            cx.emit(SorterEvent::Activate(index));
            return;
        }
        if event.modifiers.shift {
            let anchor = self.anchor.unwrap_or(index);
            self.select_range(anchor, index, cx);
        } else if command {
            self.toggle(index, cx);
        } else {
            self.select(index, cx);
        }
        self.begin_drag(index, cx);
    }

    /// 鼠标移动：拖拽期间更新插入槽。
    fn on_pointer_move(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.update_drag(position, cx);
    }

    /// 鼠标释放：结束拖拽（位移有效则抛出重排事件）。
    fn on_pointer_up(&mut self, cx: &mut Context<Self>) {
        self.finish_drag(cx);
    }

    /// 键盘导航 / 激活 / 删除。
    fn on_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let count = self.delegate.count(cx);
        if count == 0 {
            return;
        }
        let key = event.keystroke.key.to_ascii_lowercase();
        let modifiers = event.keystroke.modifiers;
        let extend = modifiers.shift;
        let command = modifiers.control || modifiers.platform;
        let columns = self.columns.max(1) as i64;
        let current = self.primary().unwrap_or(0);

        match key.as_str() {
            "left" => self.move_selection(current, -1, extend, count, cx),
            "right" => self.move_selection(current, 1, extend, count, cx),
            "up" => self.move_selection(current, -columns, extend, count, cx),
            "down" => self.move_selection(current, columns, extend, count, cx),
            "home" => self.move_to(0, extend, cx),
            "end" => self.move_to(count - 1, extend, cx),
            "a" if command => self.select_all(cx),
            "enter" | "space" => {
                if self.selected.is_empty() {
                    self.select(current, cx);
                }
                cx.emit(SorterEvent::Activate(current));
            }
            "delete" | "backspace" => {
                if !self.selected.is_empty() {
                    let indices: Vec<usize> = self.selected.iter().copied().collect();
                    cx.emit(SorterEvent::Delete(indices));
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    /// 按步长移动选中项。
    fn move_selection(
        &mut self,
        current: usize,
        delta: i64,
        extend: bool,
        count: usize,
        cx: &mut Context<Self>,
    ) {
        let next = (current as i64 + delta).clamp(0, count as i64 - 1) as usize;
        self.move_to(next, extend, cx);
    }

    /// 移动选中项到 `index`（`extend` 为真时从锚点扩展范围）。
    fn move_to(&mut self, index: usize, extend: bool, cx: &mut Context<Self>) {
        if extend {
            let anchor = self.anchor.unwrap_or(index);
            self.select_range(anchor, index, cx);
        } else {
            self.select(index, cx);
        }
        self.scroll_to_item(index, cx);
    }

    /// 渲染单元格的序号角标。
    fn render_index_badge(&self, index: usize, bg: Hsla, fg: Hsla) -> AnyElement {
        div()
            .absolute()
            .top_1()
            .left_1()
            .px_1()
            .rounded_sm()
            .bg(bg)
            .text_color(fg)
            .text_xs()
            .child((index + 1).to_string())
            .into_any_element()
    }
}

impl<D: SorterDelegate> EventEmitter<SorterEvent> for SorterState<D> {}

impl<D: SorterDelegate> Focusable for SorterState<D> {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl<D: SorterDelegate> Render for SorterState<D> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 同步数量并夹取选择 / 拖拽 / 边界缓存。
        let count = self.delegate.count(cx);
        self.selected.retain(|index| *index < count);
        if self.anchor.is_some_and(|anchor| anchor >= count) {
            self.anchor = None;
        }
        if self.dragging.is_some_and(|index| index >= count) {
            self.dragging = None;
            self.hover_slot = None;
        }
        self.cell_bounds.truncate(count);

        let cell = cell_size(self.base_width, self.zoom, self.delegate.aspect_ratio(cx));
        let gap = self.gap;
        let padding = gap * 2.0;
        self.columns = grid_columns(
            (self.bounds.size.width - padding).max(px(0.0)),
            cell.width,
            gap,
        );

        // 先把主题色拷出来，避免与后续 `render_item` 的 `&mut cx` 借用冲突。
        let theme = cx.theme();
        let border = theme.border;
        let selected_border = theme.primary;
        let index_bg = theme.muted;
        let index_fg = theme.muted_foreground;

        let selected = self.selected.clone();
        let dragging = self.dragging;
        let show_index = self.show_index;

        let mut cells: Vec<AnyElement> = Vec::with_capacity(count);
        for index in 0..count {
            let is_selected = selected.contains(&index);
            let content = self
                .delegate
                .render_item(index, is_selected, window, cx)
                .into_any_element();

            let mut item = div()
                .relative()
                .overflow_hidden()
                .rounded_md()
                .border_2()
                .border_color(if is_selected { selected_border } else { border })
                .w(cell.width)
                .h(cell.height)
                .on_prepaint({
                    let state = cx.entity();
                    move |bounds, _window, cx| {
                        state.update(cx, |state, _| {
                            if state.cell_bounds.len() <= index {
                                state.cell_bounds.resize(index + 1, Bounds::default());
                            }
                            state.cell_bounds[index] = bounds;
                        });
                    }
                })
                .id(("sorter-item", index))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |state, event, window, cx| {
                        state.on_item_mouse_down(event, index, window, cx);
                    }),
                )
                .on_mouse_move(
                    cx.listener(move |state, event: &MouseMoveEvent, _window, cx| {
                        state.on_pointer_move(event.position, cx);
                    }),
                );

            if dragging == Some(index) {
                item = item.opacity(0.4);
            }
            if show_index {
                item = item.child(self.render_index_badge(index, index_bg, index_fg));
            }
            cells.push(item.child(content).into_any_element());
        }

        // 拖拽预览：跟随指针的半透明副本（原格保留半透明占位）。
        let ghost = match (dragging, self.drag_position) {
            (Some(index), Some(position)) => {
                let content = self
                    .delegate
                    .render_item(index, false, window, cx)
                    .into_any_element();
                Some(
                    div()
                        .absolute()
                        .left(position.x - self.bounds.origin.x - cell.width / 2.0)
                        .top(position.y - self.bounds.origin.y - cell.height / 2.0)
                        .w(cell.width)
                        .h(cell.height)
                        .rounded_md()
                        .border_2()
                        .border_color(selected_border)
                        .opacity(0.85)
                        .overflow_hidden()
                        .child(content)
                        .into_any_element(),
                )
            }
            _ => None,
        };

        // 插入指示线：按插入槽落在对应单元格的左缘（末尾落在最后一格右缘）。
        let insertion = self.hover_slot.and_then(|slot| {
            let (x, y, height) = if slot < self.cell_bounds.len() {
                let bounds = self.cell_bounds[slot];
                (bounds.origin.x, bounds.origin.y, bounds.size.height)
            } else {
                let bounds = *self.cell_bounds.last()?;
                (
                    bounds.origin.x + bounds.size.width,
                    bounds.origin.y,
                    bounds.size.height,
                )
            };
            Some((x - self.bounds.origin.x, y - self.bounds.origin.y, height))
        });

        let scroll = self.scroll.clone();
        div()
            .id(("sorter-body", cx.entity().entity_id()))
            .size_full()
            .relative()
            .p(padding)
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .on_prepaint({
                let state = cx.entity();
                move |bounds, _window, cx| {
                    state.update(cx, |state, _| state.bounds = bounds);
                }
            })
            .on_mouse_move(cx.listener(|state, event: &MouseMoveEvent, _window, cx| {
                state.on_pointer_move(event.position, cx);
            }))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(gap)
                    .content_start()
                    .children(cells),
            )
            .children(ghost)
            .children(insertion.map(|(x, y, height)| {
                div()
                    .absolute()
                    .left(x)
                    .top(y)
                    .w(px(3.0))
                    .h(height)
                    .rounded_full()
                    .bg(selected_border)
                    .into_any_element()
            }))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .w(Scrollbar::width())
                    .child(Scrollbar::vertical(&scroll)),
            )
    }
}

/// 排序器元素（薄包装：按键上下文、焦点与键盘 / 释放事件）。
#[derive(IntoElement)]
pub struct Sorter<D: SorterDelegate> {
    state: Entity<SorterState<D>>,
}

impl<D: SorterDelegate> Sorter<D> {
    /// 使用给定 [`SorterState`] 创建排序器元素。
    pub fn new(state: &Entity<SorterState<D>>) -> Self {
        Self {
            state: state.clone(),
        }
    }
}

impl<D: SorterDelegate> RenderOnce for Sorter<D> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus_handle = self.state.read(cx).focus_handle(cx);
        let entity_id = self.state.entity_id();

        div()
            .id(("sorter", entity_id))
            .key_context("Sorter")
            .track_focus(&focus_handle)
            .size_full()
            .on_key_down(window.listener_for(
                &self.state,
                |state, event: &KeyDownEvent, _window, cx| {
                    state.on_key_down(event, cx);
                },
            ))
            .capture_any_mouse_up(window.listener_for(
                &self.state,
                |state, _event: &MouseUpEvent, _window, cx| {
                    state.on_pointer_up(cx);
                },
            ))
            .child(self.state)
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{
        DEFAULT_ASPECT_RATIO, MAX_ZOOM, MIN_ZOOM, Sorter, SorterDelegate, SorterEvent, SorterState,
        autoscroll_velocity, cell_size, clamp_zoom, drop_slot, grid_columns, move_dest,
        move_group_dest, normalize_aspect_ratio,
    };
    use crate::{
        App, AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement as _, Render,
        SharedString, Styled as _, Window, div, point, px, size,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    /// 固定 5 项的测试委托。
    struct TestSorter;

    impl SorterDelegate for TestSorter {
        fn count(&self, _cx: &App) -> usize {
            5
        }

        fn render_item(
            &mut self,
            index: usize,
            _selected: bool,
            _window: &mut Window,
            _cx: &mut Context<SorterState<Self>>,
        ) -> impl IntoElement {
            div()
                .size_full()
                .child(SharedString::from(index.to_string()))
        }
    }

    /// 渲染排序器的测试宿主。
    struct Probe {
        state: Entity<SorterState<TestSorter>>,
    }

    impl Render for Probe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(Sorter::new(&self.state))
        }
    }

    /// 构建宿主并收集排序器事件。
    fn setup(
        cx: &mut crate::TestAppContext,
    ) -> (
        Entity<SorterState<TestSorter>>,
        Rc<RefCell<Vec<SorterEvent>>>,
        &mut crate::VisualTestContext,
    ) {
        let events: Rc<RefCell<Vec<SorterEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(move |window, cx| {
            let state = cx.new(|cx| SorterState::new(TestSorter, cx));
            cx.subscribe_in(
                &state,
                window,
                move |_this, _state, event: &SorterEvent, _window, _cx| {
                    sink.borrow_mut().push(event.clone());
                },
            )
            .detach();
            Probe { state }
        });
        let state = probe.read_with(&*cx, |probe, _| probe.state.clone());
        (state, events, cx)
    }

    /// 缩放夹取与宽高比归一化。
    #[test]
    fn zoom_and_aspect_are_clamped() {
        assert_eq!(clamp_zoom(1.0), 1.0);
        assert_eq!(clamp_zoom(0.1), MIN_ZOOM);
        assert_eq!(clamp_zoom(99.0), MAX_ZOOM);
        assert_eq!(clamp_zoom(f32::NAN), 1.0);

        assert_eq!(normalize_aspect_ratio(2.0), 2.0);
        assert_eq!(normalize_aspect_ratio(0.0), DEFAULT_ASPECT_RATIO);
        assert_eq!(normalize_aspect_ratio(f32::INFINITY), DEFAULT_ASPECT_RATIO);
    }

    /// 单元格尺寸随缩放线性变化，并按宽高比换算高度。
    #[test]
    fn cell_size_scales_and_follows_ratio() {
        let base = cell_size(px(160.0), 1.0, 2.0);
        assert_eq!(base.width, px(160.0));
        assert_eq!(base.height, px(80.0));

        let doubled = cell_size(px(160.0), 2.0, 2.0);
        assert_eq!(doubled.width, px(320.0));
        assert_eq!(doubled.height, px(160.0));

        // 越界缩放被夹取。
        assert_eq!(cell_size(px(100.0), 10.0, 1.0).width, px(100.0) * MAX_ZOOM);
    }

    /// 每行列数按可用宽度换算，且至少为 1。
    #[test]
    fn columns_wrap_by_available_width() {
        // 间距 10、格宽 100：宽 310 恰好放 3 列（100*3 + 10*2 = 320 > 310）。
        assert_eq!(grid_columns(px(310.0), px(100.0), px(10.0)), 2);
        assert_eq!(grid_columns(px(320.0), px(100.0), px(10.0)), 3);
        assert_eq!(grid_columns(px(0.0), px(100.0), px(10.0)), 1);
    }

    /// 插入槽按指针落在单元格中线左右决定。
    #[test]
    fn drop_slot_uses_cell_midpoint() {
        let cells = [
            Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(100.0), px(50.0)),
            },
            Bounds {
                origin: point(px(100.0), px(0.0)),
                size: size(px(100.0), px(50.0)),
            },
        ];
        // 第一格左半 → 槽 0；右半 → 槽 1。
        assert_eq!(drop_slot(&cells, point(px(10.0), px(20.0))), 0);
        assert_eq!(drop_slot(&cells, point(px(80.0), px(20.0))), 1);
        // 第二格右半 → 槽 2。
        assert_eq!(drop_slot(&cells, point(px(180.0), px(20.0))), 2);
        // 落在网格外 → 排到末尾。
        assert_eq!(drop_slot(&cells, point(px(400.0), px(400.0))), 2);
    }

    /// 移动目标换算：源位置自身或紧随其后视为无位移。
    #[test]
    fn move_dest_skips_noop_slots() {
        assert_eq!(move_dest(2, 2), None);
        assert_eq!(move_dest(2, 3), None);
        assert_eq!(move_dest(2, 5), Some(4));
        assert_eq!(move_dest(4, 1), Some(1));
        assert_eq!(move_dest(0, 4), Some(3));
    }

    /// 拖到视口上下边缘才自动滚动，越贴边越快。
    #[test]
    fn autoscroll_only_near_edges() {
        let viewport = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(400.0), px(300.0)),
        };

        // 视口中间：不滚
        assert_eq!(
            autoscroll_velocity(px(150.0), viewport, px(48.0), px(18.0)),
            px(0.0)
        );
        // 贴上边缘：向上滚，越贴越快，贴边取到上限
        let near_top = autoscroll_velocity(px(10.0), viewport, px(48.0), px(18.0));
        let above_top = autoscroll_velocity(px(-20.0), viewport, px(48.0), px(18.0));
        assert!(near_top < px(0.0));
        assert!(near_top > above_top);
        assert_eq!(above_top, px(-18.0));
        // 贴下边缘：向下滚
        assert_eq!(
            autoscroll_velocity(px(320.0), viewport, px(48.0), px(18.0)),
            px(18.0)
        );
    }

    /// 整组落位：抽走选中项后索引前移，拖回自己身上不产生位移。
    #[test]
    fn group_dest_compacts_before_slot() {
        // [1, 2] 拖到槽 4：抽走两项后落位索引变成 2
        assert_eq!(move_group_dest(&[1, 2], 4), Some(2));
        // 拖到最前
        assert_eq!(move_group_dest(&[3, 4], 0), Some(0));
        // 拖回自己区间：无位移
        assert_eq!(move_group_dest(&[1, 2], 1), None);
        assert_eq!(move_group_dest(&[1, 2], 2), None);
        assert_eq!(move_group_dest(&[1, 2], 3), None);
        // 空选择没有落位
        assert_eq!(move_group_dest(&[], 0), None);
    }

    /// 单选 / 范围 / 多选 / 全选都抛出 `SelectionChanged`。
    #[rgpui::test]
    fn selection_variants_emit_changes(cx: &mut crate::TestAppContext) {
        let (state, events, mut cx) = setup(cx);

        cx.update(|_, cx| state.update(cx, |state, cx| state.select(2, cx)));
        assert_eq!(state.read_with(cx, |state, _| state.primary()), Some(2));
        assert_eq!(
            events.borrow().as_slice(),
            [SorterEvent::SelectionChanged(vec![2])]
        );

        events.borrow_mut().clear();
        cx.update(|_, cx| state.update(cx, |state, cx| state.select_range(2, 4, cx)));
        assert_eq!(
            state.read_with(cx, |state, _| state
                .selected()
                .iter()
                .copied()
                .collect::<Vec<_>>()),
            vec![2, 3, 4]
        );
        assert_eq!(
            events.borrow().as_slice(),
            [SorterEvent::SelectionChanged(vec![2, 3, 4])]
        );

        events.borrow_mut().clear();
        cx.update(|_, cx| state.update(cx, |state, cx| state.toggle(0, cx)));
        assert_eq!(state.read_with(cx, |state, _| state.selection_len()), 4);
        assert_eq!(
            events.borrow().as_slice(),
            [SorterEvent::SelectionChanged(vec![0, 2, 3, 4])]
        );

        // 越界索引被忽略，不抛出事件。
        events.borrow_mut().clear();
        cx.update(|_, cx| state.update(cx, |state, cx| state.select(99, cx)));
        assert!(events.borrow().is_empty());

        events.borrow_mut().clear();
        cx.update(|_, cx| state.update(cx, |state, cx| state.select_all(cx)));
        assert_eq!(state.read_with(cx, |state, _| state.selection_len()), 5);
    }

    /// 拖拽预览：拖拽期间记录指针位置，结束后清空（预览据此定位）。
    #[rgpui::test]
    fn drag_records_pointer_position(cx: &mut crate::TestAppContext) {
        let (state, _events, mut cx) = setup(cx);
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.begin_drag(0, cx);
                state.update_drag(crate::point(crate::px(120.0), crate::px(64.0)), cx);
            })
        });
        let position = state.read_with(cx, |state, _| state.drag_position);
        assert!(position.is_some(), "拖拽期间应记录指针位置");
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.cancel_drag(cx);
            })
        });
        assert!(
            state
                .read_with(cx, |state, _| state.drag_position)
                .is_none()
        );
    }
    /// 拖拽到有效槽位抛出重排事件，落到相邻槽位则视为无位移。
    #[rgpui::test]
    fn drag_reorder_emits_event(cx: &mut crate::TestAppContext) {
        let (state, events, mut cx) = setup(cx);

        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.begin_drag(0, cx);
                state.set_drop_slot(3, cx);
                state.finish_drag(cx);
            })
        });
        assert_eq!(
            events.borrow().as_slice(),
            [SorterEvent::Reorder { from: 0, to: 2 }]
        );
        assert!(!state.read_with(cx, |state, _| state.is_dragging()));

        // 槽位等于源位置 / 紧随源位置：无位移，不抛出事件。
        events.borrow_mut().clear();
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.begin_drag(2, cx);
                state.set_drop_slot(2, cx);
                state.finish_drag(cx);
            })
        });
        assert!(events.borrow().is_empty());

        // 取消拖拽不抛出事件。
        events.borrow_mut().clear();
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.begin_drag(1, cx);
                state.set_drop_slot(4, cx);
                state.cancel_drag(cx);
            })
        });
        assert!(events.borrow().is_empty());
    }

    /// 拖的是多选中的一项时整组移动，抛 `ReorderMany`。
    #[rgpui::test]
    fn drag_multi_selection_emits_reorder_many(cx: &mut crate::TestAppContext) {
        let (state, events, mut cx) = setup(cx);

        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.toggle(1, cx);
                state.toggle(2, cx);
            })
        });
        events.borrow_mut().clear();

        // 抓取选中项之一拖到末尾：整组 [1, 2] 落到抽走两项后的索引 3
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.begin_drag(2, cx);
                state.set_drop_slot(5, cx);
                state.finish_drag(cx);
            })
        });
        assert_eq!(
            events.borrow().as_slice(),
            [SorterEvent::ReorderMany {
                from: vec![1, 2],
                to: 3
            }]
        );
    }

    /// 缩放 / 序号显示 / 间距的可配置性。
    #[rgpui::test]
    fn options_are_configurable(cx: &mut crate::TestAppContext) {
        let (state, _events, mut cx) = setup(cx);

        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.set_zoom(99.0, cx);
                state.set_show_index(false, cx);
                state.set_gap(px(4.0), cx);
                state.set_base_width(px(200.0), cx);
            })
        });
        assert_eq!(state.read_with(cx, |state, _| state.zoom()), MAX_ZOOM);
        assert!(!state.read_with(cx, |state, _| state.show_index()));
        assert_eq!(state.read_with(cx, |state, _| state.gap()), px(4.0));
        assert_eq!(
            state.read_with(cx, |state, _| state.base_width()),
            px(200.0)
        );
    }
}
