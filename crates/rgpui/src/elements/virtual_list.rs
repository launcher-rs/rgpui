//! 虚拟列表 - 用于渲染大量尺寸不同的行/列。
//!
//! > 注意：必须保证每列宽度或行高度一致（同组）。
//!
//! 出于性能考虑，只渲染可见范围。
//!
//! 参考 `rgpui::uniform_list`。
//! https://github.com/zed-industries/zed/blob/0ae1603610ab6b265bdfbee7b8dbc23c5ab06edc/crates/rgpui/src/elements/uniform_list.rs
//!
//! 与 `uniform_list` 不同，这里的每个条目可以有不同的大小。
//!
//! 这适用于更复杂的布局，例如不同行高的表格。
use std::{
    cell::RefCell,
    cmp,
    ops::{Deref, Range},
    rc::Rc,
};

use crate::{
    Along, AnyElement, App, AvailableSpace, Axis, AxisExt, Bounds, ContentMask, Context,
    DeferredScrollToItem, Div, Element, ElementId, Entity, GlobalElementId, Half, Hitbox,
    InteractiveElement, IntoElement, IsZero as _, ListSizingBehavior, Pixels, Point, Render,
    ScrollHandle, ScrollStrategy, Size, Stateful, StatefulInteractiveElement, StyleRefinement,
    Styled, Window, div, elements::scroll::ScrollbarHandle, point, px, size,
};
use smallvec::SmallVec;

struct VirtualListScrollHandleState {
    axis: Axis,
    items_count: usize,
    deferred_scroll_to_item: Option<DeferredScrollToItem>,
}

/// [`VirtualList`] 的滚动句柄。
///
/// 参见 [`ScrollHandle`]。
#[derive(Clone)]
pub struct VirtualListScrollHandle {
    state: Rc<RefCell<VirtualListScrollHandleState>>,
    base_handle: ScrollHandle,
}

impl From<ScrollHandle> for VirtualListScrollHandle {
    fn from(handle: ScrollHandle) -> Self {
        let mut this = VirtualListScrollHandle::new();
        this.base_handle = handle;
        this
    }
}

impl AsRef<ScrollHandle> for VirtualListScrollHandle {
    fn as_ref(&self) -> &ScrollHandle {
        &self.base_handle
    }
}

impl ScrollbarHandle for VirtualListScrollHandle {
    fn offset(&self) -> Point<Pixels> {
        self.base_handle.offset()
    }

    fn set_offset(&self, offset: Point<Pixels>) {
        self.base_handle.set_offset(offset);
    }

    fn content_size(&self) -> Size<Pixels> {
        self.base_handle.content_size()
    }
}

impl Deref for VirtualListScrollHandle {
    type Target = ScrollHandle;

    fn deref(&self) -> &Self::Target {
        &self.base_handle
    }
}

impl VirtualListScrollHandle {
    /// 创建新的 VirtualListScrollHandle。
    pub fn new() -> Self {
        VirtualListScrollHandle {
            state: Rc::new(RefCell::new(VirtualListScrollHandleState {
                axis: Axis::Vertical,
                items_count: 0,
                deferred_scroll_to_item: None,
            })),
            base_handle: ScrollHandle::default(),
        }
    }

    /// 获取基础滚动句柄。
    pub fn base_handle(&self) -> &ScrollHandle {
        &self.base_handle
    }

    /// 滚动到指定索引的条目。
    pub fn scroll_to_item(&self, ix: usize, strategy: ScrollStrategy) {
        self.scroll_to_item_with_offset(ix, strategy, 0);
    }

    /// 滚动到指定索引的条目，并附加条目偏移量。
    fn scroll_to_item_with_offset(&self, ix: usize, strategy: ScrollStrategy, offset: usize) {
        let mut state = self.state.borrow_mut();
        state.deferred_scroll_to_item = Some(DeferredScrollToItem {
            item_index: ix,
            strategy,
            offset,
            scroll_strict: false,
        });
    }

    /// 滚动到列表底部。
    pub fn scroll_to_bottom(&self) {
        let items_count = self.state.borrow().items_count;
        self.scroll_to_item(items_count.saturating_sub(1), ScrollStrategy::Top);
    }
}

/// 创建垂直方向的 [`VirtualList`]。
///
/// 这类似于 RGPUI 的 `uniform_list`，但支持两个轴。
///
/// `item_sizes` 是每行的大小。只使用 `height`；`width` 通过测量
/// [`VirtualList::with_item_to_measure_index`] 选中的条目来推断，默认为第一个条目。
///
/// 参见 [`h_virtual_list`]。
#[inline]
pub fn v_virtual_list<R, V>(
    view: Entity<V>,
    id: impl Into<ElementId>,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    f: impl 'static + Fn(&mut V, Range<usize>, &mut Window, &mut Context<V>) -> Vec<R>,
) -> VirtualList
where
    R: IntoElement,
    V: Render,
{
    virtual_list(view, id, Axis::Vertical, item_sizes, f)
}

/// 创建水平方向的 [`VirtualList`]。
///
/// `item_sizes` 是每列的大小。只使用 `width`；`height` 通过测量
/// [`VirtualList::with_item_to_measure_index`] 选中的条目来推断，默认为第一个条目。
///
/// 参见 [`v_virtual_list`]。
#[inline]
pub fn h_virtual_list<R, V>(
    view: Entity<V>,
    id: impl Into<ElementId>,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    f: impl 'static + Fn(&mut V, Range<usize>, &mut Window, &mut Context<V>) -> Vec<R>,
) -> VirtualList
where
    R: IntoElement,
    V: Render,
{
    virtual_list(view, id, Axis::Horizontal, item_sizes, f)
}

pub(crate) fn virtual_list<R, V>(
    view: Entity<V>,
    id: impl Into<ElementId>,
    axis: Axis,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    f: impl 'static + Fn(&mut V, Range<usize>, &mut Window, &mut Context<V>) -> Vec<R>,
) -> VirtualList
where
    R: IntoElement,
    V: Render,
{
    let id: ElementId = id.into();
    let scroll_handle = VirtualListScrollHandle::new();
    let render_range = move |visible_range, window: &mut Window, cx: &mut App| {
        view.update(cx, |this, cx| {
            f(this, visible_range, window, cx)
                .into_iter()
                .map(|component| component.into_any_element())
                .collect()
        })
    };

    VirtualList {
        id: id.clone(),
        axis,
        base: div()
            .id(id)
            .size_full()
            .overflow_scroll()
            .restrict_scroll_to_axis()
            .track_scroll(&scroll_handle),
        scroll_handle,
        items_count: item_sizes.len(),
        item_sizes,
        render_items: Box::new(render_range),
        sizing_behavior: ListSizingBehavior::default(),
        item_to_measure_index: 0,
    }
}

/// 用于渲染大量不同尺寸条目的虚拟列表组件。
pub struct VirtualList {
    id: ElementId,
    axis: Axis,
    base: Stateful<Div>,
    scroll_handle: VirtualListScrollHandle,
    items_count: usize,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    render_items: Box<
        dyn for<'a> Fn(Range<usize>, &'a mut Window, &'a mut App) -> SmallVec<[AnyElement; 64]>,
    >,
    sizing_behavior: ListSizingBehavior,
    item_to_measure_index: usize,
}

impl Styled for VirtualList {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl VirtualList {
    /// 将滚动句柄与虚拟列表绑定。
    pub fn track_scroll(mut self, scroll_handle: &VirtualListScrollHandle) -> Self {
        self.base = self.base.track_scroll(&scroll_handle);
        self.scroll_handle = scroll_handle.clone();
        self
    }

    /// 设置列表的尺寸计算行为。
    pub fn with_sizing_behavior(mut self, behavior: ListSizingBehavior) -> Self {
        self.sizing_behavior = behavior;
        self
    }

    /// 设置用于推断列表交叉轴尺寸的条目索引。
    pub fn with_item_to_measure_index(mut self, index: usize) -> Self {
        self.item_to_measure_index = index;
        self
    }

    /// 为表格指定滚动句柄。
    ///
    /// 表格比较特殊，因为 `scroll_handle` 基于表格头部（不是虚拟列表）。
    pub fn with_scroll_handle(mut self, scroll_handle: &VirtualListScrollHandle) -> Self {
        self.base = div().id(self.id.clone()).size_full();
        self.scroll_handle = scroll_handle.clone();
        self
    }

    fn scroll_to_deferred_item(
        &self,
        scroll_offset: Point<Pixels>,
        items_bounds: &[Bounds<Pixels>],
        content_bounds: &Bounds<Pixels>,
        scroll_to_item: DeferredScrollToItem,
    ) -> Point<Pixels> {
        let Some(bounds) = items_bounds
            .get(scroll_to_item.item_index + scroll_to_item.offset)
            .cloned()
        else {
            return scroll_offset;
        };

        let mut scroll_offset = scroll_offset;
        match scroll_to_item.strategy {
            ScrollStrategy::Center => {
                if self.axis.is_vertical() {
                    scroll_offset.y = content_bounds.top() + content_bounds.size.height.half()
                        - bounds.top()
                        - bounds.size.height.half()
                } else {
                    scroll_offset.x = content_bounds.left() + content_bounds.size.width.half()
                        - bounds.left()
                        - bounds.size.width.half()
                }
            }
            _ => {
                // Ref: https://github.com/zed-industries/zed/blob/0d145289e0867a8d5d63e5e1397a5ca69c9d49c3/crates/rgpui/src/elements/div.rs#L3026
                if self.axis.is_vertical() {
                    if bounds.top() + scroll_offset.y < content_bounds.top() {
                        scroll_offset.y = content_bounds.top() - bounds.top()
                    } else if bounds.bottom() + scroll_offset.y > content_bounds.bottom() {
                        scroll_offset.y = content_bounds.bottom() - bounds.bottom();
                    }
                } else {
                    if bounds.left() + scroll_offset.x < content_bounds.left() {
                        scroll_offset.x = content_bounds.left() - bounds.left();
                    } else if bounds.right() + scroll_offset.x > content_bounds.right() {
                        scroll_offset.x = content_bounds.right() - bounds.right();
                    }
                }
            }
        }
        self.scroll_handle.set_offset(scroll_offset);
        scroll_offset
    }

    /// 参考自：https://github.com/zed-industries/zed/blob/83f9f9d9e3f5914392cab9a09e3472711a1d7b38/crates/rgpui/src/elements/uniform_list.rs#L660
    fn measure_item(
        &self,
        list_width: Option<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Size<Pixels> {
        if self.items_count == 0 {
            return Size::default();
        }

        let item_ix = self.item_to_measure_index.min(self.items_count - 1);
        let mut items = (self.render_items)(item_ix..item_ix + 1, window, cx);
        let Some(mut item_to_measure) = items.pop() else {
            return Size::default();
        };
        let available_space = size(
            list_width.map_or(AvailableSpace::MinContent, |width| {
                AvailableSpace::Definite(width)
            }),
            AvailableSpace::MinContent,
        );
        item_to_measure.layout_as_root(available_space, window, cx)
    }
}

/// [VirtualItem] 使用的帧状态。
pub struct VirtualListFrameState {
    /// 要绘制的可见条目。
    items: SmallVec<[AnyElement; 32]>,
    size_layout: ItemSizeLayout,
}

/// 虚拟列表的尺寸布局缓存。
///
/// 缓存每条条目的尺寸与原点位置，供虚拟列表渲染与滚动计算使用。
#[derive(Default, Clone)]
pub struct ItemSizeLayout {
    items_sizes: Rc<Vec<Size<Pixels>>>,
    content_size: Size<Pixels>,
    sizes: Vec<Pixels>,
    origins: Vec<Pixels>,
    last_layout_bounds: Bounds<Pixels>,
}

impl IntoElement for VirtualList {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for VirtualList {
    type RequestLayoutState = VirtualListFrameState;
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&crate::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (crate::LayoutId, Self::RequestLayoutState) {
        let rem_size = window.rem_size();
        let font_size = window.text_style().font_size.to_pixels(rem_size);
        let mut size_layout = ItemSizeLayout::default();
        let longest_item_size = self.measure_item(None, window, cx);

        let layout_id = self.base.interactivity().request_layout(
            global_id,
            inspector_id,
            window,
            cx,
            |style, window, cx| {
                size_layout = window.with_element_state(
                    global_id.unwrap(),
                    |state: Option<ItemSizeLayout>, _window| {
                        let mut state = state.unwrap_or(ItemSizeLayout::default());

                        // 包含条目之间的间距以计算条目尺寸
                        let gap = style
                            .gap
                            .along(self.axis)
                            .to_pixels(font_size.into(), rem_size);

                        if state.items_sizes != self.item_sizes {
                            state.items_sizes = self.item_sizes.clone();
                            // 按轴准备每个条目的尺寸
                            state.sizes = self
                                .item_sizes
                                .iter()
                                .enumerate()
                                .map(|(i, size)| {
                                    let size = size.along(self.axis);
                                    if i + 1 == self.items_count {
                                        size
                                    } else {
                                        size + gap
                                    }
                                })
                                .collect::<Vec<_>>();

                            // 按轴准备每个条目的原点
                            state.origins = state
                                .sizes
                                .iter()
                                .scan(px(0.), |cumulative, size| match self.axis {
                                    Axis::Horizontal => {
                                        let x = *cumulative;
                                        *cumulative += *size;
                                        Some(x)
                                    }
                                    Axis::Vertical => {
                                        let y = *cumulative;
                                        *cumulative += *size;
                                        Some(y)
                                    }
                                })
                                .collect::<Vec<_>>();

                            if self.axis.is_horizontal() {
                                state.content_size.width =
                                    px(state.sizes.iter().map(|size| size.as_f32()).sum::<f32>());
                            } else {
                                state.content_size.height =
                                    px(state.sizes.iter().map(|size| size.as_f32()).sum::<f32>());
                            }
                        }

                        if self.axis.is_horizontal() {
                            state.content_size.height = longest_item_size.height;
                        } else {
                            state.content_size.width = longest_item_size.width;
                        }

                        (state.clone(), state)
                    },
                );

                let axis = self.axis;
                let layout_id =
                    match self.sizing_behavior {
                        ListSizingBehavior::Infer => {
                            window.with_text_style(style.text_style().cloned(), |window| {
                                let size_layout = size_layout.clone();

                                window.request_measured_layout(style, {
                                    move |known_dimensions, available_space, _, _| {
                                        let mut size = Size::default();
                                        if axis.is_horizontal() {
                                            size.width = known_dimensions.width.unwrap_or(
                                                match available_space.width {
                                                    AvailableSpace::Definite(x) => x,
                                                    AvailableSpace::MinContent
                                                    | AvailableSpace::MaxContent => {
                                                        size_layout.content_size.width
                                                    }
                                                },
                                            );
                                            size.height = known_dimensions.width.unwrap_or(
                                                match available_space.height {
                                                    AvailableSpace::Definite(x) => x,
                                                    AvailableSpace::MinContent
                                                    | AvailableSpace::MaxContent => {
                                                        size_layout.content_size.height
                                                    }
                                                },
                                            );
                                        } else {
                                            size.width = known_dimensions.width.unwrap_or(
                                                match available_space.width {
                                                    AvailableSpace::Definite(x) => x,
                                                    AvailableSpace::MinContent
                                                    | AvailableSpace::MaxContent => {
                                                        size_layout.content_size.width
                                                    }
                                                },
                                            );
                                            size.height = known_dimensions.height.unwrap_or(
                                                match available_space.height {
                                                    AvailableSpace::Definite(x) => x,
                                                    AvailableSpace::MinContent
                                                    | AvailableSpace::MaxContent => {
                                                        size_layout.content_size.height
                                                    }
                                                },
                                            );
                                        }

                                        size
                                    }
                                })
                            })
                        }
                        ListSizingBehavior::Auto => window
                            .with_text_style(style.text_style().cloned(), |window| {
                                window.request_layout(style, None, cx)
                            }),
                    };

                layout_id
            },
        );

        (
            layout_id,
            VirtualListFrameState {
                items: SmallVec::new(),
                size_layout,
            },
        )
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&crate::InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        layout.size_layout.last_layout_bounds = bounds;

        let style = self
            .base
            .interactivity()
            .compute_style(global_id, None, window, cx);
        let border_widths = style.border_widths.to_pixels(window.rem_size());
        let paddings = style
            .padding
            .to_pixels(bounds.size.into(), window.rem_size());

        let item_sizes = &layout.size_layout.sizes;
        let item_origins = &layout.size_layout.origins;

        let content_bounds = Bounds::from_corners(
            bounds.origin
                + point(
                    border_widths.left + paddings.left,
                    border_widths.top + paddings.top,
                ),
            bounds.bottom_right()
                - point(
                    border_widths.right + paddings.right,
                    border_widths.bottom + paddings.bottom,
                ),
        );

        // 使用条目边界更新滚动句柄
        let items_bounds = item_origins
            .iter()
            .enumerate()
            .map(|(i, &origin)| {
                let item_size = item_sizes[i];

                Bounds {
                    origin: match self.axis {
                        Axis::Horizontal => point(content_bounds.left() + origin, px(0.)),
                        Axis::Vertical => point(px(0.), content_bounds.top() + origin),
                    },
                    size: match self.axis {
                        Axis::Horizontal => size(item_size, content_bounds.size.height),
                        Axis::Vertical => size(content_bounds.size.width, item_size),
                    },
                }
            })
            .collect::<Vec<_>>();

        let axis = self.axis;

        let mut scroll_state = self.scroll_handle.state.borrow_mut();
        scroll_state.axis = axis;
        scroll_state.items_count = self.items_count;

        let mut scroll_offset = self.scroll_handle.offset();
        if let Some(scroll_to_item) = scroll_state.deferred_scroll_to_item.take() {
            scroll_offset = self.scroll_to_deferred_item(
                scroll_offset,
                &items_bounds,
                &content_bounds,
                scroll_to_item,
            );
        }

        scroll_offset = scroll_offset
            .max(&point(
                content_bounds.size.width - layout.size_layout.content_size.width,
                content_bounds.size.height - layout.size_layout.content_size.height,
            ))
            .min(&point(px(0.), px(0.)));
        if scroll_offset != self.scroll_handle.offset() {
            self.scroll_handle.set_offset(scroll_offset);
        }

        self.base.interactivity().prepaint(
            global_id,
            inspector_id,
            bounds,
            layout.size_layout.content_size,
            window,
            cx,
            |_style, _, hitbox, window, cx| {
                if self.items_count > 0 {
                    let min_scroll_offset = content_bounds.size.along(self.axis)
                        - layout.size_layout.content_size.along(self.axis);

                    let is_scrolled = !scroll_offset.along(self.axis).is_zero();
                    if is_scrolled {
                        match self.axis {
                            Axis::Horizontal if scroll_offset.x < min_scroll_offset => {
                                scroll_offset.x = min_scroll_offset;
                                self.scroll_handle.set_offset(scroll_offset);
                            }
                            Axis::Vertical if scroll_offset.y < min_scroll_offset => {
                                scroll_offset.y = min_scroll_offset;
                                self.scroll_handle.set_offset(scroll_offset);
                            }
                            _ => {}
                        }
                    }

                    let (first_visible_element_ix, last_visible_element_ix) = match self.axis {
                        Axis::Horizontal => {
                            let mut cumulative_size = px(0.);
                            let mut first_visible_element_ix = 0;
                            for (i, &size) in item_sizes.iter().enumerate() {
                                cumulative_size += size;
                                if cumulative_size > -(scroll_offset.x + paddings.left) {
                                    first_visible_element_ix = i;
                                    break;
                                }
                            }

                            cumulative_size = px(0.);
                            let mut last_visible_element_ix = 0;
                            for (i, &size) in item_sizes.iter().enumerate() {
                                cumulative_size += size;
                                if cumulative_size > (-scroll_offset.x + content_bounds.size.width)
                                {
                                    last_visible_element_ix = i + 1;
                                    break;
                                }
                            }
                            if last_visible_element_ix == 0 {
                                last_visible_element_ix = self.items_count;
                            } else {
                                last_visible_element_ix += 1;
                            }
                            (first_visible_element_ix, last_visible_element_ix)
                        }
                        Axis::Vertical => {
                            let mut cumulative_size = px(0.);
                            let mut first_visible_element_ix = 0;
                            for (i, &size) in item_sizes.iter().enumerate() {
                                cumulative_size += size;
                                if cumulative_size > -(scroll_offset.y + paddings.top) {
                                    first_visible_element_ix = i;
                                    break;
                                }
                            }

                            cumulative_size = px(0.);
                            let mut last_visible_element_ix = 0;
                            for (i, &size) in item_sizes.iter().enumerate() {
                                cumulative_size += size;
                                if cumulative_size > (-scroll_offset.y + content_bounds.size.height)
                                {
                                    last_visible_element_ix = i + 1;
                                    break;
                                }
                            }
                            if last_visible_element_ix == 0 {
                                last_visible_element_ix = self.items_count;
                            } else {
                                last_visible_element_ix += 1;
                            }
                            (first_visible_element_ix, last_visible_element_ix)
                        }
                    };

                    let visible_range = first_visible_element_ix
                        ..cmp::min(last_visible_element_ix, self.items_count);

                    let items = (self.render_items)(visible_range.clone(), window, cx);

                    let content_mask = ContentMask { bounds };
                    window.with_content_mask(Some(content_mask), |window| {
                        for (mut item, ix) in items.into_iter().zip(visible_range.clone()) {
                            let item_origin = match self.axis {
                                Axis::Horizontal => {
                                    content_bounds.origin
                                        + point(item_origins[ix] + scroll_offset.x, scroll_offset.y)
                                }
                                Axis::Vertical => {
                                    content_bounds.origin
                                        + point(scroll_offset.x, item_origins[ix] + scroll_offset.y)
                                }
                            };

                            let available_space = match self.axis {
                                Axis::Horizontal => size(
                                    AvailableSpace::Definite(item_sizes[ix]),
                                    AvailableSpace::Definite(content_bounds.size.height),
                                ),
                                Axis::Vertical => size(
                                    AvailableSpace::Definite(content_bounds.size.width),
                                    AvailableSpace::Definite(item_sizes[ix]),
                                ),
                            };

                            item.layout_as_root(available_space, window, cx);
                            item.prepaint_at(item_origin, window, cx);
                            layout.items.push(item);
                        }
                    });
                }

                hitbox
            },
        )
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&crate::InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.base.interactivity().paint(
            global_id,
            inspector_id,
            bounds,
            hitbox.as_ref(),
            window,
            cx,
            |_, window, cx| {
                for item in &mut layout.items {
                    item.paint(window, cx);
                }
            },
        )
    }
}
