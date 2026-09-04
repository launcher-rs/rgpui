//! 虚拟滚动增强 —— 支持大量数据的高效渲染。
//!
//! # 示例
//!
//! ```rust,ignore
//! use rgpui::virtual_scroll::{VirtualScroll, VirtualScrollState};
//!
//! let state = cx.new(|_| VirtualScrollState::new(10000, 30.0));
//! VirtualScroll::new(state)
//! ```

use crate::{Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, px};

/// 虚拟滚动状态。
pub struct VirtualScrollState {
    /// 总项目数。
    pub total_items: usize,
    /// 每项高度（像素）。
    pub item_height: f32,
    /// 当前滚动偏移。
    pub scroll_offset: f32,
    /// 可见区域高度。
    pub viewport_height: f32,
    /// 缓冲区大小（额外渲染的项目数）。
    pub buffer_size: usize,
}

impl VirtualScrollState {
    /// 创建新的虚拟滚动状态。
    pub fn new(total_items: usize, item_height: f32) -> Self {
        Self {
            total_items,
            item_height,
            scroll_offset: 0.0,
            viewport_height: 0.0,
            buffer_size: 5,
        }
    }

    /// 设置缓冲区大小。
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// 更新滚动偏移。
    pub fn set_scroll_offset(&mut self, offset: f32) {
        let max_offset =
            (self.total_items as f32 * self.item_height - self.viewport_height).max(0.0);
        self.scroll_offset = offset.clamp(0.0, max_offset);
    }

    /// 设置视口高度。
    pub fn set_viewport_height(&mut self, height: f32) {
        self.viewport_height = height;
    }

    /// 获取可见项目的起始索引。
    pub fn visible_start_index(&self) -> usize {
        let start = (self.scroll_offset / self.item_height) as usize;
        start.saturating_sub(self.buffer_size)
    }

    /// 获取可见项目的结束索引。
    pub fn visible_end_index(&self) -> usize {
        let end = ((self.scroll_offset + self.viewport_height) / self.item_height) as usize;
        (end + self.buffer_size).min(self.total_items)
    }

    /// 获取可见项目的数量。
    pub fn visible_count(&self) -> usize {
        self.visible_end_index() - self.visible_start_index()
    }

    /// 获取总内容高度。
    pub fn total_height(&self) -> f32 {
        self.total_items as f32 * self.item_height
    }

    /// 获取指定项目的顶部位置。
    pub fn item_top(&self, index: usize) -> f32 {
        index as f32 * self.item_height
    }

    /// 获取指定项目的底部位置。
    pub fn item_bottom(&self, index: usize) -> f32 {
        (index + 1) as f32 * self.item_height
    }

    /// 滚动到指定项目。
    pub fn scroll_to_item(&mut self, index: usize) {
        let target_offset = index as f32 * self.item_height;
        if target_offset < self.scroll_offset {
            self.scroll_offset = target_offset;
        } else if target_offset + self.item_height > self.scroll_offset + self.viewport_height {
            self.scroll_offset = target_offset + self.item_height - self.viewport_height;
        }
    }
}

/// 虚拟滚动组件。
pub struct VirtualScroll {
    state: Entity<VirtualScrollState>,
}

impl VirtualScroll {
    /// 创建新的虚拟滚动组件。
    pub fn new(state: Entity<VirtualScrollState>) -> Self {
        Self { state }
    }
}

impl Render for VirtualScroll {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .relative()
            .h(px(state.total_height()))
            .child(div().absolute().top(px(state.scroll_offset)).w_full())
    }
}

/// 虚拟滚动项目渲染器。
pub trait VirtualItemRenderer {
    /// 项目类型。
    type Item;

    /// 渲染单个项目。
    fn render_item(&self, item: &Self::Item, index: usize) -> impl IntoElement;

    /// 获取项目高度。
    fn item_height(&self, item: &Self::Item) -> f32;
}

/// 虚拟滚动列表 —— 自动管理可见项目。
pub struct VirtualScrollList<I: Clone> {
    state: Entity<VirtualScrollState>,
    items: Vec<I>,
    renderer: Box<dyn Fn(&I, usize) -> crate::AnyElement>,
}

impl<I: Clone + 'static> VirtualScrollList<I> {
    /// 创建新的虚拟滚动列表。
    pub fn new(
        state: Entity<VirtualScrollState>,
        items: Vec<I>,
        renderer: impl Fn(&I, usize) -> crate::AnyElement + 'static,
    ) -> Self {
        Self {
            state,
            items,
            renderer: Box::new(renderer),
        }
    }

    /// 获取可见项目。
    pub fn visible_items(&self, cx: &Context<Self>) -> Vec<(usize, &I)> {
        let state = self.state.read(cx);
        let start = state.visible_start_index();
        let end = state.visible_end_index();

        self.items[start..end]
            .iter()
            .enumerate()
            .map(|(i, item)| (start + i, item))
            .collect()
    }
}

impl<I: Clone + 'static> Render for VirtualScrollList<I> {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visible = self.visible_items(cx);
        let state = self.state.read(cx);

        div()
            .relative()
            .h(px(state.total_height()))
            .children(visible.into_iter().map(|(index, item)| {
                let y = state.item_top(index);
                div()
                    .absolute()
                    .top(px(y))
                    .w_full()
                    .child((self.renderer)(item, index))
            }))
    }
}

/// 虚拟滚动方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualScrollDirection {
    /// 垂直滚动。
    Vertical,
    /// 水平滚动。
    Horizontal,
}

/// 虚拟滚动配置。
#[derive(Debug, Clone)]
pub struct VirtualScrollConfig {
    /// 滚动方向。
    pub direction: VirtualScrollDirection,
    /// 缓冲区大小。
    pub buffer_size: usize,
    /// 估算项目高度（用于初始渲染）。
    pub estimated_item_height: f32,
    /// 是否启用回收（重用已离开视口的元素）。
    pub enable_recycling: bool,
}

impl Default for VirtualScrollConfig {
    fn default() -> Self {
        Self {
            direction: VirtualScrollDirection::Vertical,
            buffer_size: 5,
            estimated_item_height: 30.0,
            enable_recycling: true,
        }
    }
}
