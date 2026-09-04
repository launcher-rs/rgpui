//! Tab 拖拽排序支持 —— 允许拖拽 Tab 进行排序。
//!
//! # 示例
//!
//! ```rust,ignore
//! use rgpui::tabs::tab_drag::{TabDragDrop, TabDragState};
//!
//! let drag_state = cx.new(|_| TabDragState::default());
//! TabDragDrop::new(drag_state)
//! ```

use crate::{
    Context, Entity, IntoElement, ParentElement, Pixels, Render, Styled,
    Window, div, px,
};

/// Tab 拖拽状态。
#[derive(Default)]
pub struct TabDragState {
    /// 当前正在拖拽的 Tab 索引。
    pub dragging_index: Option<usize>,
    /// 拖拽起始位置。
    pub drag_start: Option<crate::Point<Pixels>>,
    /// 拖拽当前位置。
    pub drag_current: Option<crate::Point<Pixels>>,
    /// 是否启用拖拽。
    pub enabled: bool,
    /// Tab 列表。
    pub tabs: Vec<TabItem>,
}

/// Tab 项目。
#[derive(Debug, Clone)]
pub struct TabItem {
    /// Tab 标题。
    pub title: String,
    /// Tab ID。
    pub id: String,
    /// 是否可关闭。
    pub closable: bool,
}

impl TabDragState {
    /// 开始拖拽。
    pub fn start_drag(&mut self, index: usize, position: crate::Point<Pixels>) {
        if self.enabled {
            self.dragging_index = Some(index);
            self.drag_start = Some(position);
            self.drag_current = Some(position);
        }
    }

    /// 更新拖拽位置。
    pub fn update_drag(&mut self, position: crate::Point<Pixels>) {
        if self.dragging_index.is_some() {
            self.drag_current = Some(position);
        }
    }

    /// 结束拖拽，返回目标索引。
    pub fn end_drag(&mut self) -> Option<(usize, usize)> {
        if let Some(from) = self.dragging_index.take() {
            let to = self.drag_target_index();
            self.drag_start = None;
            self.drag_current = None;
            to.map(|to| (from, to))
        } else {
            None
        }
    }

    /// 取消拖拽。
    pub fn cancel_drag(&mut self) {
        self.dragging_index = None;
        self.drag_start = None;
        self.drag_current = None;
    }

    /// 获取拖拽目标索引。
    pub fn drag_target_index(&self) -> Option<usize> {
        if let (Some(_from), Some(current)) = (self.dragging_index, self.drag_current) {
            // 简化实现：根据水平位置计算目标索引
            let tab_width = 120.0; // 假设每个 Tab 宽度 120px
            let target = (current.x.0 / tab_width) as usize;
            Some(target.min(self.tabs.len().saturating_sub(1)))
        } else {
            None
        }
    }

    /// 是否正在拖拽。
    pub fn is_dragging(&self) -> bool {
        self.dragging_index.is_some()
    }

    /// 移动 Tab。
    pub fn move_tab(&mut self, from: usize, to: usize) {
        if from < self.tabs.len() && to < self.tabs.len() && from != to {
            let tab = self.tabs.remove(from);
            self.tabs.insert(to, tab);
        }
    }
}

/// Tab 拖拽组件。
pub struct TabDragDrop {
    state: Entity<TabDragState>,
}

impl TabDragDrop {
    /// 创建新的 Tab 拖拽组件。
    pub fn new(state: Entity<TabDragState>) -> Self {
        Self { state }
    }
}

impl Render for TabDragDrop {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        if !state.enabled {
            return div().into_element();
        }

        // 渲染拖拽预览（如果正在拖拽）
        if let (Some(from), Some(current)) = (state.dragging_index, state.drag_current) {
            if let Some(tab) = state.tabs.get(from) {
                return div()
                    .absolute()
                    .left(current.x - px(60.0))
                    .top(current.y - px(15.0))
                    .w(px(120.))
                    .h(px(30.))
                    .bg(crate::gray_700())
                    .rounded_md()
                    .shadow_lg()
                    .opacity(0.8)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .child(tab.title.clone());
            }
        }

        div().into_element()
    }
}

/// Tab 拖拽事件。
#[derive(Debug, Clone)]
pub enum TabDragEvent {
    /// 开始拖拽。
    DragStart {
        /// Tab 索引。
        index: usize,
    },
    /// 拖拽中。
    Dragging {
        /// 当前索引。
        from: usize,
        /// 目标索引。
        to: usize,
    },
    /// 拖拽结束。
    DragEnd {
        /// 旧索引。
        from: usize,
        /// 新索引。
        to: usize,
    },
    /// 拖拽取消。
    DragCancelled,
}
