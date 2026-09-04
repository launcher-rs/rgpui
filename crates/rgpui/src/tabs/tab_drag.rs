//! Tab 拖拽排序支持 —— 允许拖拽 Tab 进行排序。
//!
//! # 示例
//!
//! ```rust,ignore
//! use rgpui::tabs::tab_drag::{TabDragDrop, TabDragState};
//!
//! let drag_state = cx.new(|_| TabDragState::default());
//! TabDragDrop::new(drag_state)
//!     .on_reorder(|tabs, window, cx| { /* 处理排序完成 */ })
//! ```

use crate::{
    App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, Window, div, px, prelude::FluentBuilder,
    theme::ActiveTheme,
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

/// 拖拽中的 Tab 数据。
#[derive(Clone, Debug)]
pub struct TabDragData {
    /// Tab 索引。
    pub index: usize,
    /// Tab 标题。
    pub title: String,
    /// 拖拽位置。
    pub position: crate::Point<Pixels>,
}

impl crate::Render for TabDragData {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        crate::div()
            .pl(self.position.x)
            .pt(self.position.y)
            .child(
                crate::div()
                    .px(px(12.0))
                    .py(px(6.0))
                    .bg(theme.tokens.popover.opacity(0.95))
                    .border_1()
                    .border_color(theme.tokens.primary)
                    .rounded(theme.radius)
                    .shadow(vec![crate::BoxShadow {
                        color: crate::hsla(0.0, 0.0, 0.0, 0.25),
                        offset: crate::point(px(0.0), px(4.0)),
                        blur_radius: px(12.0),
                        spread_radius: px(0.0),
                        inset: false,
                    }])
                    .text_size(px(13.0))
                    .text_color(theme.tokens.foreground)
                    .font_family(theme.font_family.clone())
                    .child(self.title.clone()),
            )
    }
}

impl std::fmt::Debug for TabDragState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TabDragState")
            .field("dragging_index", &self.dragging_index)
            .field("tabs_count", &self.tabs.len())
            .finish()
    }
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
            let tab_width = 120.0;
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

/// Tab 拖拽组件 —— 渲染 Tab 栏并支持拖拽排序。
#[derive(IntoElement)]
pub struct TabDragDrop {
    /// 绑定状态实体。
    state: Entity<TabDragState>,
    /// 重排完成回调。
    on_reorder: Option<std::rc::Rc<dyn Fn(Vec<TabItem>, &mut Window, &mut App)>>,
    /// Tab 样式。
    tab_style: TabStyle,
}

/// Tab 样式配置。
#[derive(Clone, Debug)]
pub struct TabStyle {
    /// 普通背景色。
    pub normal_bg: crate::Hsla,
    /// 悬停背景色。
    pub hover_bg: crate::Hsla,
    /// 拖拽中背景色（被拖拽的 Tab）。
    pub dragging_bg: crate::Hsla,
    /// 拖拽中背景色（悬停目标）。
    pub drop_target_bg: crate::Hsla,
    /// 拖拽指示线颜色。
    pub indicator_color: crate::Hsla,
    /// Tab 间距。
    pub gap: Pixels,
    /// Tab 内边距。
    pub padding: (Pixels, Pixels),
    /// Tab 圆角。
    pub radius: Pixels,
}

impl Default for TabStyle {
    fn default() -> Self {
        Self {
            normal_bg: crate::gray_100(),
            hover_bg: crate::gray_200(),
            dragging_bg: crate::gray_300(),
            drop_target_bg: crate::blue_50(),
            indicator_color: crate::blue_500(),
            gap: px(2.0),
            padding: (px(12.0), px(6.0)),
            radius: px(6.0),
        }
    }
}

impl TabDragDrop {
    /// 创建新的 Tab 拖拽组件。
    pub fn new(state: Entity<TabDragState>) -> Self {
        Self {
            state,
            on_reorder: None,
            tab_style: TabStyle::default(),
        }
    }

    /// 设置重排完成回调。
    pub fn on_reorder(
        mut self,
        callback: impl Fn(Vec<TabItem>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_reorder = Some(std::rc::Rc::new(callback));
        self
    }

    /// 设置 Tab 样式。
    pub fn tab_style(mut self, style: TabStyle) -> Self {
        self.tab_style = style;
        self
    }
}

impl crate::RenderOnce for TabDragDrop {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);

        if !state.enabled {
            return div().into_any_element();
        }

        let tabs = state.tabs.clone();
        let dragging_index = state.dragging_index;
        let style = self.tab_style.clone();
        let state_clone = self.state.clone();
        let on_reorder = self.on_reorder.clone();

        // 渲染 Tab 栏
        let tab_bar = div()
            .id("tab-drag-bar")
            .flex()
            .items_center()
            .gap(style.gap);

        let mut container = tab_bar;

        for (idx, tab) in tabs.iter().enumerate() {
            let is_dragging = dragging_index == Some(idx);
            let tab = tab.clone();
            let state_drag = state_clone.clone();
            let state_drop = state_clone.clone();
            let on_reorder = on_reorder.clone();
            let style_clone = style.clone();
            let tab_title = tab.title.clone();

            let tab_element = div()
                .id(format!("tab-{}", idx))
                .flex()
                .items_center()
                .gap(px(6.0))
                .px(style_clone.padding.0)
                .py(style_clone.padding.1)
                .rounded(style_clone.radius)
                .text_sm()
                // 正常状态背景
                .when(!is_dragging, |el| el.bg(style_clone.normal_bg))
                // 拖拽中的 Tab 半透明 + 虚线边框
                .when(is_dragging, |el| {
                    el.bg(style_clone.dragging_bg)
                        .opacity(0.4)
                        .border_1()
                        .border_dashed()
                        .border_color(style_clone.indicator_color)
                })
                .child(tab.title.clone())
                .children(if tab.closable {
                    Some(
                        div()
                            .text_xs()
                            .text_color(crate::gray_500())
                            .hover(|el| el.text_color(crate::gray_700()))
                            .child("x"),
                    )
                } else {
                    None
                })
                .on_drag(
                    TabDragData {
                        index: idx,
                        title: tab_title,
                        position: crate::Point::default(),
                    },
                    move |data: &TabDragData, pos, _window, cx| {
                        state_drag.update(cx, |s, _| {
                            s.dragging_index = Some(data.index);
                            s.drag_current = Some(pos);
                        });
                        cx.new(|_| TabDragData {
                            index: data.index,
                            title: data.title.clone(),
                            position: pos,
                        })
                    },
                )
                .drag_over::<TabDragData>(move |style, _, _, _| {
                    style
                        .bg(style_clone.drop_target_bg)
                        .border_l(px(3.0))
                        .border_color(style_clone.indicator_color)
                })
                .on_drop(move |dragged: &TabDragData, _window, cx| {
                    let from = dragged.index;
                    let to = idx;
                    if from == to {
                        state_drop.update(cx, |s, ctx| {
                            s.dragging_index = None;
                            s.drag_current = None;
                            ctx.notify();
                        });
                        return;
                    }

                    state_drop.update(cx, |s, ctx| {
                        if from < s.tabs.len() && to < s.tabs.len() {
                            let moved = s.tabs.remove(from);
                            let insert_at = to.min(s.tabs.len());
                            s.tabs.insert(insert_at, moved);
                        }
                        s.dragging_index = None;
                        s.drag_current = None;
                        ctx.notify();
                    });

                    if let Some(ref callback) = on_reorder {
                        let reordered = state_drop.read(cx).tabs.clone();
                        callback(reordered, _window, cx);
                    }
                });

            container = container.child(tab_element);
        }

        container.into_any_element()
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
