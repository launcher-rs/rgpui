//! 等高可滚动元素列表，针对大列表优化。
//! 不使用完整的 taffy 布局系统，uniform_list 仅测量第一个元素，
//! 然后根据该测量值将所有剩余元素按行布局。这比完整布局系统快得多，
//! 但仅适用于等高元素。

use crate::{
    AnyElement, App, AvailableSpace, Bounds, ContentMask, Element, ElementId, Entity,
    GlobalElementId, Hitbox, InspectorElementId, InteractiveElement, Interactivity, IntoElement,
    IsZero, LayoutId, ListSizingBehavior, Overflow, Pixels, Point, ScrollHandle, Size,
    StyleRefinement, Styled, Window, point, px, size,
};
use smallvec::SmallVec;
use std::{cell::RefCell, cmp, ops::Range, rc::Rc, usize};

use super::ListHorizontalSizingBehavior;

/// uniform_list 为一组等高元素提供延迟渲染。
/// 当渲染到设置了 overflow-y: hidden 和固定（或最大）高度的容器中时，
/// uniform_list 只会渲染可见子集的元素。
#[track_caller]
pub fn uniform_list<R>(
    id: impl Into<ElementId>,
    item_count: usize,
    f: impl 'static + Fn(Range<usize>, &mut Window, &mut App) -> Vec<R>,
) -> UniformList
where
    R: IntoElement,
{
    let id = id.into();
    let mut base_style = StyleRefinement::default();
    base_style.overflow.y = Some(Overflow::Scroll);

    let render_range = move |range: Range<usize>, window: &mut Window, cx: &mut App| {
        f(range, window, cx)
            .into_iter()
            .map(|component| component.into_any_element())
            .collect()
    };

    UniformList {
        item_count,
        item_to_measure_index: 0,
        render_items: Box::new(render_range),
        decorations: Vec::new(),
        interactivity: Interactivity {
            element_id: Some(id),
            base_style: Box::new(base_style),
            ..Interactivity::new()
        },
        scroll_handle: None,
        sizing_behavior: ListSizingBehavior::default(),
        horizontal_sizing_behavior: ListHorizontalSizingBehavior::default(),
    }
}

/// 用于高效布局和显示等高元素列表的列表元素。
pub struct UniformList {
    item_count: usize,
    item_to_measure_index: usize,
    render_items: Box<
        dyn for<'a> Fn(Range<usize>, &'a mut Window, &'a mut App) -> SmallVec<[AnyElement; 64]>,
    >,
    decorations: Vec<Box<dyn UniformListDecoration>>,
    interactivity: Interactivity,
    scroll_handle: Option<UniformListScrollHandle>,
    sizing_behavior: ListSizingBehavior,
    horizontal_sizing_behavior: ListHorizontalSizingBehavior,
}

/// [UniformList] 使用的帧状态。
pub struct UniformListFrameState {
    items: SmallVec<[AnyElement; 32]>,
    decorations: SmallVec<[AnyElement; 2]>,
}

/// 用于控制均匀列表滚动位置的句柄。
/// 应将其存储在视图中，并在每帧传递给 uniform_list。
#[derive(Clone, Debug, Default)]
pub struct UniformListScrollHandle(pub Rc<RefCell<UniformListScrollState>>);

/// 滚动元素的放置位置。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollStrategy {
    /// 将元素放置在列表视口顶部。
    Top,
    /// 尝试将元素放置在列表视口中间。
    /// 如果滚动目标上方没有足够的列表项，则无法实现：
    /// 此时元素将放置在最近的可能位置。
    Center,
    /// 尝试将元素放置在列表视口底部。
    /// 如果滚动目标上方没有足够的列表项，则无法实现：
    /// 此时元素将放置在最近的可能位置。
    Bottom,
    /// 如果元素不可见，则尝试将其放置在：
    /// - 目标元素在当前可见元素上方时，放置在列表视口顶部。
    /// - 目标元素在当前可见元素下方时，放置在列表视口底部。
    Nearest,
}

/// 延迟滚动到指定项的参数。在下次布局时执行滚动。
#[derive(Clone, Copy, Debug)]
pub struct DeferredScrollToItem {
    /// 要滚动到的项索引。
    pub item_index: usize,
    /// 滚动策略。
    pub strategy: ScrollStrategy,
    /// 相对于目标项的偏移量（项数）。
    pub offset: usize,
    /// 是否严格按偏移量滚动（而非仅确保可见）。
    pub scroll_strict: bool,
}

/// 均匀列表的滚动状态，管理滚动句柄和延迟滚动参数。
#[derive(Clone, Debug, Default)]
pub struct UniformListScrollState {
    /// 底层滚动句柄。
    pub base_handle: ScrollHandle,
    /// 待执行的延迟滚动目标。
    pub deferred_scroll_to_item: Option<DeferredScrollToItem>,
    /// 上次布局时捕获的项大小。
    pub last_item_size: Option<ItemSize>,
    /// 上次布局时列表是否垂直翻转。
    pub y_flipped: bool,
}

#[derive(Copy, Clone, Debug, Default)]
/// 项及其内容的大小。
pub struct ItemSize {
    /// 项的大小。
    pub item: Size<Pixels>,
    /// 项内容的大小，当项受父元素约束时，可能大于项本身。
    pub contents: Size<Pixels>,
}

impl UniformListScrollHandle {
    /// 创建一个绑定到均匀列表的新滚动句柄。
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(UniformListScrollState {
            base_handle: ScrollHandle::new(),
            deferred_scroll_to_item: None,
            last_item_size: None,
            y_flipped: false,
        })))
    }

    /// 滚动列表使指定项索引可见。
    ///
    /// 使用非严格滚动：如果项已完全可见，则不执行滚动。
    /// 如果项超出视图，则按策略滚动最小距离使其进入视图。
    pub fn scroll_to_item(&self, ix: usize, strategy: ScrollStrategy) {
        self.0.borrow_mut().deferred_scroll_to_item = Some(DeferredScrollToItem {
            item_index: ix,
            strategy,
            offset: 0,
            scroll_strict: false,
        });
    }

    /// 滚动列表使指定项索引位于滚动策略位置。
    ///
    /// 使用严格滚动：即使项已可见，也会滚动到匹配的策略位置。
    /// 当需要精确定位时使用此方法。
    pub fn scroll_to_item_strict(&self, ix: usize, strategy: ScrollStrategy) {
        self.0.borrow_mut().deferred_scroll_to_item = Some(DeferredScrollToItem {
            item_index: ix,
            strategy,
            offset: 0,
            scroll_strict: true,
        });
    }

    /// 以项数偏移量滚动列表到指定项索引。
    ///
    /// 使用非严格滚动：如果项已在偏移区域内可见，则不执行滚动。
    ///
    /// 偏移参数从对应边缘缩小有效视口指定项数，然后在缩小的视口中应用滚动策略：
    /// - `ScrollStrategy::Top`：从顶部缩小，将项定位在新顶部
    /// - `ScrollStrategy::Center`：从顶部缩小，将项居中在缩小的视口中
    /// - `ScrollStrategy::Bottom`：从底部缩小，将项定位在新底部
    pub fn scroll_to_item_with_offset(&self, ix: usize, strategy: ScrollStrategy, offset: usize) {
        self.0.borrow_mut().deferred_scroll_to_item = Some(DeferredScrollToItem {
            item_index: ix,
            strategy,
            offset,
            scroll_strict: false,
        });
    }

    /// 滚动列表使指定项索引精确位于滚动策略位置，并带偏移量。
    ///
    /// 使用严格滚动：即使项已可见，也会滚动到匹配的策略位置。
    ///
    /// 偏移参数从对应边缘缩小有效视口指定项数，然后在缩小的视口中应用滚动策略：
    /// - `ScrollStrategy::Top`：从顶部缩小，将项定位在新顶部
    /// - `ScrollStrategy::Center`：从顶部缩小，将项居中在缩小的视口中
    /// - `ScrollStrategy::Bottom`：从底部缩小，将项定位在新底部
    pub fn scroll_to_item_strict_with_offset(
        &self,
        ix: usize,
        strategy: ScrollStrategy,
        offset: usize,
    ) {
        self.0.borrow_mut().deferred_scroll_to_item = Some(DeferredScrollToItem {
            item_index: ix,
            strategy,
            offset,
            scroll_strict: true,
        });
    }

    /// 检查列表是否垂直翻转。
    pub fn y_flipped(&self) -> bool {
        self.0.borrow().y_flipped
    }

    /// 获取最顶层可见子元素的索引。
    #[cfg(any(test, feature = "test-support"))]
    pub fn logical_scroll_top_index(&self) -> usize {
        let this = self.0.borrow();
        this.deferred_scroll_to_item
            .as_ref()
            .map(|deferred| deferred.item_index)
            .unwrap_or_else(|| this.base_handle.logical_scroll_top().0)
    }

    /// 检查列表是否可以垂直滚动。
    pub fn is_scrollable(&self) -> bool {
        if let Some(size) = self.0.borrow().last_item_size {
            size.contents.height > size.item.height
        } else {
            false
        }
    }

    /// 列表是否已滚动到底部，如果列表不可滚动则返回 `None`。
    pub fn is_scrolled_to_end(&self) -> Option<bool> {
        let state = self.0.borrow();
        let max_offset = state.base_handle.max_offset();
        if max_offset.y <= px(0.) {
            return None;
        }
        let offset = state.base_handle.offset();
        Some(-offset.y >= max_offset.y)
    }

    /// 滚动到列表底部。
    pub fn scroll_to_bottom(&self) {
        self.scroll_to_item(usize::MAX, ScrollStrategy::Bottom);
    }
}

impl Styled for UniformList {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.interactivity.base_style
    }
}

impl Element for UniformList {
    type RequestLayoutState = UniformListFrameState;
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        self.interactivity.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let max_items = self.item_count;
        let item_size = self.measure_item(None, window, cx);
        let layout_id = self.interactivity.request_layout(
            global_id,
            inspector_id,
            window,
            cx,
            |style, window, cx| match self.sizing_behavior {
                ListSizingBehavior::Infer => {
                    window.with_text_style(style.text_style().cloned(), |window| {
                        window.request_measured_layout(
                            style,
                            move |known_dimensions, available_space, _window, _cx| {
                                let desired_height = item_size.height * max_items;
                                let width = known_dimensions.width.unwrap_or(match available_space
                                    .width
                                {
                                    AvailableSpace::Definite(x) => x,
                                    AvailableSpace::MinContent | AvailableSpace::MaxContent => {
                                        item_size.width
                                    }
                                });
                                let height = match available_space.height {
                                    AvailableSpace::Definite(height) => desired_height.min(height),
                                    AvailableSpace::MinContent | AvailableSpace::MaxContent => {
                                        desired_height
                                    }
                                };
                                size(width, height)
                            },
                        )
                    })
                }
                ListSizingBehavior::Auto => window
                    .with_text_style(style.text_style().cloned(), |window| {
                        window.request_layout(style, None, cx)
                    }),
            },
        );

        (
            layout_id,
            UniformListFrameState {
                items: SmallVec::new(),
                decorations: SmallVec::new(),
            },
        )
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        frame_state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Hitbox> {
        let style = self
            .interactivity
            .compute_style(global_id, None, window, cx);
        let border = style.border_widths.to_pixels(window.rem_size());
        let padding = style
            .padding
            .to_pixels(bounds.size.into(), window.rem_size());

        let padded_bounds = Bounds::from_corners(
            bounds.origin + point(border.left + padding.left, border.top + padding.top),
            bounds.bottom_right()
                - point(border.right + padding.right, border.bottom + padding.bottom),
        );

        let can_scroll_horizontally = matches!(
            self.horizontal_sizing_behavior,
            ListHorizontalSizingBehavior::Unconstrained
        );

        let longest_item_size = self.measure_item(None, window, cx);
        let content_width = if can_scroll_horizontally {
            padded_bounds.size.width.max(longest_item_size.width)
        } else {
            padded_bounds.size.width
        };
        let content_size = Size {
            width: content_width,
            height: longest_item_size.height * self.item_count,
        };

        let shared_scroll_offset = self.interactivity.scroll_offset.clone().unwrap();
        let item_height = longest_item_size.height;
        let shared_scroll_to_item = self.scroll_handle.as_mut().and_then(|handle| {
            let mut handle = handle.0.borrow_mut();
            handle.last_item_size = Some(ItemSize {
                item: padded_bounds.size,
                contents: content_size,
            });
            handle.deferred_scroll_to_item.take()
        });

        self.interactivity.prepaint(
            global_id,
            inspector_id,
            bounds,
            content_size,
            window,
            cx,
            |_style, mut scroll_offset, hitbox, window, cx| {
                let y_flipped = if let Some(scroll_handle) = &self.scroll_handle {
                    let scroll_state = scroll_handle.0.borrow();
                    scroll_state.y_flipped
                } else {
                    false
                };

                if self.item_count > 0 {
                    let content_height = item_height * self.item_count;

                    let is_scrolled_vertically = !scroll_offset.y.is_zero();
                    let max_scroll_offset = padded_bounds.size.height - content_height;

                    if is_scrolled_vertically && scroll_offset.y < max_scroll_offset {
                        shared_scroll_offset.borrow_mut().y = max_scroll_offset;
                        scroll_offset.y = max_scroll_offset;
                    }

                    let content_width = content_size.width + padding.left + padding.right;
                    let is_scrolled_horizontally =
                        can_scroll_horizontally && !scroll_offset.x.is_zero();
                    if is_scrolled_horizontally && content_width <= padded_bounds.size.width {
                        shared_scroll_offset.borrow_mut().x = Pixels::ZERO;
                        scroll_offset.x = Pixels::ZERO;
                    }

                    if let Some(DeferredScrollToItem {
                        mut item_index,
                        mut strategy,
                        offset,
                        scroll_strict,
                    }) = shared_scroll_to_item
                    {
                        if y_flipped {
                            item_index = self.item_count.saturating_sub(item_index + 1);
                        }
                        let list_height = padded_bounds.size.height;
                        let mut updated_scroll_offset = shared_scroll_offset.borrow_mut();
                        let item_top = item_height * item_index;
                        let item_bottom = item_top + item_height;
                        let scroll_top = -updated_scroll_offset.y;
                        let offset_pixels = item_height * offset;

                        // is the selected item above/below currently visible items
                        let is_above = item_top < scroll_top + offset_pixels;
                        let is_below = item_bottom > scroll_top + list_height;

                        if scroll_strict || is_above || is_below {
                            if strategy == ScrollStrategy::Nearest {
                                if is_above {
                                    strategy = ScrollStrategy::Top;
                                } else if is_below {
                                    strategy = ScrollStrategy::Bottom;
                                }
                            }

                            let max_scroll_offset =
                                (content_height - list_height).max(Pixels::ZERO);
                            match strategy {
                                ScrollStrategy::Top => {
                                    updated_scroll_offset.y = -(item_top - offset_pixels)
                                        .clamp(Pixels::ZERO, max_scroll_offset);
                                }
                                ScrollStrategy::Center => {
                                    let item_center = item_top + item_height / 2.0;

                                    let viewport_height = list_height - offset_pixels;
                                    let viewport_center = offset_pixels + viewport_height / 2.0;
                                    let target_scroll_top = item_center - viewport_center;
                                    updated_scroll_offset.y =
                                        -target_scroll_top.clamp(Pixels::ZERO, max_scroll_offset);
                                }
                                ScrollStrategy::Bottom => {
                                    updated_scroll_offset.y = -(item_bottom - list_height)
                                        .clamp(Pixels::ZERO, max_scroll_offset);
                                }
                                ScrollStrategy::Nearest => {
                                    // Nearest, but the item is visible -> no scroll is required
                                }
                            }
                        }
                        scroll_offset = *updated_scroll_offset
                    }

                    let first_visible_element_ix =
                        (-(scroll_offset.y + padding.top) / item_height).floor() as usize;
                    let last_visible_element_ix = ((-scroll_offset.y + padded_bounds.size.height)
                        / item_height)
                        .ceil() as usize;

                    let visible_range = first_visible_element_ix
                        ..cmp::min(last_visible_element_ix, self.item_count);

                    let items = if y_flipped {
                        let flipped_range = self.item_count.saturating_sub(visible_range.end)
                            ..self.item_count.saturating_sub(visible_range.start);
                        let mut items = (self.render_items)(flipped_range, window, cx);
                        items.reverse();
                        items
                    } else {
                        (self.render_items)(visible_range.clone(), window, cx)
                    };

                    let content_mask = ContentMask { bounds };
                    window.with_content_mask(Some(content_mask), |window| {
                        for (mut item, ix) in items.into_iter().zip(visible_range.clone()) {
                            let item_origin = padded_bounds.origin
                                + scroll_offset
                                + point(Pixels::ZERO, item_height * ix);

                            let available_width = if can_scroll_horizontally {
                                padded_bounds.size.width + scroll_offset.x.abs()
                            } else {
                                padded_bounds.size.width
                            };
                            let available_space = size(
                                AvailableSpace::Definite(available_width),
                                AvailableSpace::Definite(item_height),
                            );
                            item.layout_as_root(available_space, window, cx);
                            item.prepaint_at(item_origin, window, cx);
                            frame_state.items.push(item);
                        }

                        let bounds =
                            Bounds::new(padded_bounds.origin + scroll_offset, padded_bounds.size);
                        for decoration in &self.decorations {
                            let mut decoration = decoration.as_ref().compute(
                                visible_range.clone(),
                                bounds,
                                scroll_offset,
                                item_height,
                                self.item_count,
                                window,
                                cx,
                            );
                            let available_space = size(
                                AvailableSpace::Definite(bounds.size.width),
                                AvailableSpace::Definite(bounds.size.height),
                            );
                            decoration.layout_as_root(available_space, window, cx);
                            decoration.prepaint_at(bounds.origin, window, cx);
                            frame_state.decorations.push(decoration);
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
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<crate::Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        hitbox: &mut Option<Hitbox>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.interactivity.paint(
            global_id,
            inspector_id,
            bounds,
            hitbox.as_ref(),
            window,
            cx,
            |_, window, cx| {
                for item in &mut request_layout.items {
                    item.paint(window, cx);
                }
                for decoration in &mut request_layout.decorations {
                    decoration.paint(window, cx);
                }
            },
        )
    }
}

impl IntoElement for UniformList {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// [`UniformList`] 的装饰物。可用于渲染缩进指南或其他视觉效果。
pub trait UniformListDecoration {
    /// 根据可见列表项范围、列表边界和每项高度计算装饰元素。
    fn compute(
        &self,
        visible_range: Range<usize>,
        bounds: Bounds<Pixels>,
        scroll_offset: Point<Pixels>,
        item_height: Pixels,
        item_count: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement;
}

impl<T: UniformListDecoration + 'static> UniformListDecoration for Entity<T> {
    fn compute(
        &self,
        visible_range: Range<usize>,
        bounds: Bounds<Pixels>,
        scroll_offset: Point<Pixels>,
        item_height: Pixels,
        item_count: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        self.update(cx, |inner, cx| {
            inner.compute(
                visible_range,
                bounds,
                scroll_offset,
                item_height,
                item_count,
                window,
                cx,
            )
        })
    }
}

impl UniformList {
    /// 选择用于测量宽度的特定列表项。
    pub fn with_width_from_item(mut self, item_index: Option<usize>) -> Self {
        self.item_to_measure_index = item_index.unwrap_or(0);
        self
    }

    /// 设置大小调整行为，类似于 `List` 元素。
    pub fn with_sizing_behavior(mut self, behavior: ListSizingBehavior) -> Self {
        self.sizing_behavior = behavior;
        self
    }

    /// 设置水平大小调整行为，控制列表项的水平布局方式。
    /// 使用 [`ListHorizontalSizingBehavior::Unconstrained`] 行为时，每项和列表本身将
    /// 具有最宽项的大小，并将 `end_slot` 推向右端。
    pub fn with_horizontal_sizing_behavior(
        mut self,
        behavior: ListHorizontalSizingBehavior,
    ) -> Self {
        self.horizontal_sizing_behavior = behavior;
        match behavior {
            ListHorizontalSizingBehavior::FitList => {
                self.interactivity.base_style.overflow.x = None;
            }
            ListHorizontalSizingBehavior::Unconstrained => {
                self.interactivity.base_style.overflow.x = Some(Overflow::Scroll);
            }
        }
        self
    }

    /// 向列表添加装饰元素。
    pub fn with_decoration(mut self, decoration: impl UniformListDecoration + 'static) -> Self {
        self.decorations.push(Box::new(decoration));
        self
    }

    fn measure_item(
        &self,
        list_width: Option<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Size<Pixels> {
        if self.item_count == 0 {
            return Size::default();
        }

        let item_ix = cmp::min(self.item_to_measure_index, self.item_count - 1);
        let mut items = (self.render_items)(item_ix..item_ix + 1, window, cx);
        let Some(mut item_to_measure) = items.pop() else {
            return Size::default();
        };
        let available_space = size(
            list_width.map_or(AvailableSpace::MaxContent, |width| {
                AvailableSpace::Definite(width)
            }),
            AvailableSpace::MinContent,
        );
        item_to_measure.layout_as_root(available_space, window, cx)
    }

    /// 跟踪并渲染此列表相对于给定滚动句柄的滚动状态。
    pub fn track_scroll(mut self, handle: &UniformListScrollHandle) -> Self {
        self.interactivity.tracked_scroll_handle = Some(handle.0.borrow().base_handle.clone());
        self.scroll_handle = Some(handle.clone());
        self
    }

    /// 设置列表是否垂直翻转，使第 0 项显示在底部。
    pub fn y_flipped(mut self, y_flipped: bool) -> Self {
        if let Some(ref scroll_handle) = self.scroll_handle {
            let mut scroll_state = scroll_handle.0.borrow_mut();
            let mut base_handle = &scroll_state.base_handle;
            let offset = base_handle.offset();
            match scroll_state.last_item_size {
                Some(last_size) if scroll_state.y_flipped != y_flipped => {
                    let new_y_offset =
                        -(offset.y + last_size.contents.height - last_size.item.height);
                    base_handle.set_offset(point(offset.x, new_y_offset));
                    scroll_state.y_flipped = y_flipped;
                }
                // Handle case where list is initially flipped.
                None if y_flipped => {
                    base_handle.set_offset(point(offset.x, Pixels::MIN));
                    scroll_state.y_flipped = y_flipped;
                }
                _ => {}
            }
        }
        self
    }
}

impl InteractiveElement for UniformList {
    fn interactivity(&mut self) -> &mut crate::Interactivity {
        &mut self.interactivity
    }
}

#[cfg(test)]
mod test {
    use crate::TestAppContext;

    #[rgpui::test]
    fn test_scroll_strategy_nearest(cx: &mut TestAppContext) {
        use crate::{
            Context, FocusHandle, ScrollStrategy, UniformListScrollHandle, Window, div, prelude::*,
            px, uniform_list,
        };
        use std::ops::Range;

        actions!(example, [SelectNext, SelectPrev]);

        struct TestView {
            index: usize,
            length: usize,
            scroll_handle: UniformListScrollHandle,
            focus_handle: FocusHandle,
            visible_range: Range<usize>,
        }

        impl TestView {
            pub fn select_next(
                &mut self,
                _: &SelectNext,
                window: &mut Window,
                _: &mut Context<Self>,
            ) {
                if self.index + 1 == self.length {
                    self.index = 0
                } else {
                    self.index += 1;
                }
                self.scroll_handle
                    .scroll_to_item(self.index, ScrollStrategy::Nearest);
                window.refresh();
            }

            pub fn select_previous(
                &mut self,
                _: &SelectPrev,
                window: &mut Window,
                _: &mut Context<Self>,
            ) {
                if self.index == 0 {
                    self.index = self.length - 1
                } else {
                    self.index -= 1;
                }
                self.scroll_handle
                    .scroll_to_item(self.index, ScrollStrategy::Nearest);
                window.refresh();
            }
        }

        impl Render for TestView {
            fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .id("list-example")
                    .track_focus(&self.focus_handle)
                    .on_action(cx.listener(Self::select_next))
                    .on_action(cx.listener(Self::select_previous))
                    .size_full()
                    .child(
                        uniform_list(
                            "entries",
                            self.length,
                            cx.processor(|this, range: Range<usize>, _window, _cx| {
                                this.visible_range = range.clone();
                                range
                                    .map(|ix| div().id(ix).h(px(20.0)).child(format!("Item {ix}")))
                                    .collect()
                            }),
                        )
                        .track_scroll(&self.scroll_handle)
                        .h(px(200.0)),
                    )
            }
        }

        let (view, cx) = cx.add_window_view(|window, cx| {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            TestView {
                scroll_handle: UniformListScrollHandle::new(),
                index: 0,
                focus_handle,
                length: 47,
                visible_range: 0..0,
            }
        });

        // 10 out of 47 items are visible

        // First 9 times selecting next item does not scroll
        for ix in 1..10 {
            cx.dispatch_action(SelectNext);
            view.read_with(cx, |view, _| {
                assert_eq!(view.index, ix);
                assert_eq!(view.visible_range, 0..10);
            })
        }

        // Now each time the list scrolls down by 1
        for ix in 10..47 {
            cx.dispatch_action(SelectNext);
            view.read_with(cx, |view, _| {
                assert_eq!(view.index, ix);
                assert_eq!(view.visible_range, ix - 9..ix + 1);
            })
        }

        // After the last item we move back to the start
        cx.dispatch_action(SelectNext);
        view.read_with(cx, |view, _| {
            assert_eq!(view.index, 0);
            assert_eq!(view.visible_range, 0..10);
        });

        // Return to the last element
        cx.dispatch_action(SelectPrev);
        view.read_with(cx, |view, _| {
            assert_eq!(view.index, 46);
            assert_eq!(view.visible_range, 37..47);
        });

        // First 9 times selecting previous does not scroll
        for ix in (37..46).rev() {
            cx.dispatch_action(SelectPrev);
            view.read_with(cx, |view, _| {
                assert_eq!(view.index, ix);
                assert_eq!(view.visible_range, 37..47);
            })
        }

        // Now each time the list scrolls up by 1
        for ix in (0..37).rev() {
            cx.dispatch_action(SelectPrev);
            view.read_with(cx, |view, _| {
                assert_eq!(view.index, ix);
                assert_eq!(view.visible_range, ix..ix + 10);
            })
        }
    }
}
