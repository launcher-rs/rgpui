//! 可拖拽排序列表：支持拖拽重排条目。

use crate::{prelude::FluentBuilder as _, *};
use std::rc::Rc;

/// 拖拽中的排序条目数据。
#[derive(Clone)]
pub struct SortableItemDrag {
    /// 被拖拽条目的索引。
    index: usize,
    /// 拖拽位置。
    position: Point<Pixels>,
}

impl std::fmt::Debug for SortableItemDrag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SortableItemDrag")
            .field("index", &self.index)
            .finish()
    }
}

impl Render for SortableItemDrag {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div().pl(self.position.x).pt(self.position.y).child(
            div()
                .px(px(12.0))
                .py(px(8.0))
                .bg(theme.tokens.popover.opacity(0.95))
                .border_1()
                .border_color(theme.tokens.primary)
                .rounded(theme.radius)
                .shadow(vec![BoxShadow {
                    color: hsla(0.0, 0.0, 0.0, 0.2),
                    offset: point(px(0.0), px(4.0)),
                    blur_radius: px(8.0),
                    spread_radius: px(0.0),
                    inset: false,
                }])
                .text_size(px(14.0))
                .text_color(theme.tokens.foreground)
                .font_family(theme.font_family.clone())
                .child("Moving..."),
        )
    }
}

/// 可排序列表的状态：管理条目与拖拽索引。
pub struct SortableListState<T: Clone + 'static> {
    /// 条目列表。
    items: Vec<T>,
    /// 正在拖拽的条目索引。
    dragging_index: Option<usize>,
    /// 悬停的条目索引。
    hover_index: Option<usize>,
}

impl<T: Clone + 'static> SortableListState<T> {
    /// 创建状态。
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items,
            dragging_index: None,
            hover_index: None,
        }
    }

    /// 获取条目列表。
    pub fn items(&self) -> &[T] {
        &self.items
    }

    /// 设置条目列表。
    pub fn set_items(&mut self, items: Vec<T>) {
        self.items = items;
    }

    /// 获取正在拖拽的条目索引。
    pub fn dragging_index(&self) -> Option<usize> {
        self.dragging_index
    }

    /// 获取悬停的条目索引。
    pub fn hover_index(&self) -> Option<usize> {
        self.hover_index
    }
}

/// 可拖拽排序列表组件。
#[derive(IntoElement)]
pub struct SortableList<T: Clone + 'static> {
    /// 绑定状态实体。
    state: Entity<SortableListState<T>>,
    /// 条目渲染器（条目、索引、是否正在拖拽）。
    item_renderer: Rc<dyn Fn(&T, usize, bool) -> AnyElement>,
    /// 重排完成回调。
    on_reorder: Option<Rc<dyn Fn(Vec<T>, &mut Window, &mut App)>>,
    /// 排列方向。
    direction: Axis,
    /// 条目间距。
    gap: Pixels,
    /// 用户样式。
    style: StyleRefinement,
}

impl<T: Clone + 'static> SortableList<T> {
    /// 创建可排序列表。
    pub fn new(
        state: Entity<SortableListState<T>>,
        renderer: impl Fn(&T, usize, bool) -> AnyElement + 'static,
    ) -> Self {
        Self {
            state,
            item_renderer: Rc::new(renderer),
            on_reorder: None,
            direction: Axis::Vertical,
            gap: px(4.0),
            style: StyleRefinement::default(),
        }
    }

    /// 设置重排完成回调。
    pub fn on_reorder(
        mut self,
        callback: impl Fn(Vec<T>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_reorder = Some(Rc::new(callback));
        self
    }

    /// 设置排列方向。
    pub fn direction(mut self, direction: Axis) -> Self {
        self.direction = direction;
        self
    }

    /// 设置条目间距。
    pub fn gap(mut self, gap: impl Into<Pixels>) -> Self {
        self.gap = gap.into();
        self
    }
}

impl<T: Clone + 'static> Styled for SortableList<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<T: Clone + 'static> RenderOnce for SortableList<T> {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;
        let items = self.state.read(cx).items.clone();
        let dragging_index = self.state.read(cx).dragging_index;
        let drag_over_bg = theme.tokens.primary.opacity(0.1);
        let indicator_color = theme.tokens.primary;

        let mut container = div()
            .flex()
            .when(self.direction == Axis::Vertical, |d| d.flex_col())
            .gap(self.gap);

        for (idx, item) in items.iter().enumerate() {
            let is_dragging = dragging_index == Some(idx);
            let rendered = (self.item_renderer)(item, idx, is_dragging);

            let state_drop = self.state.clone();
            let on_reorder = self.on_reorder.clone();
            let state_drag = self.state.clone();

            let item_el = div()
                .id(ElementId::Name(format!("sortable-item-{}", idx).into()))
                .child(rendered)
                .on_drag(
                    SortableItemDrag {
                        index: idx,
                        position: Point::default(),
                    },
                    move |data: &SortableItemDrag, pos, _window, cx| {
                        state_drag.update(cx, |s, _| {
                            s.dragging_index = Some(data.index);
                        });
                        cx.new(|_| SortableItemDrag {
                            index: data.index,
                            position: pos,
                        })
                    },
                )
                .drag_over::<SortableItemDrag>(move |style, _, _, _| {
                    style
                        .bg(drag_over_bg)
                        .border_t(px(2.0))
                        .border_color(indicator_color)
                })
                .on_drop(move |dragged: &SortableItemDrag, window, cx| {
                    let from = dragged.index;
                    let to = idx;
                    if from == to {
                        state_drop.update(cx, |s, ctx| {
                            s.dragging_index = None;
                            s.hover_index = None;
                            ctx.notify();
                        });
                        return;
                    }

                    state_drop.update(cx, |s, ctx| {
                        let mut reordered = s.items.clone();
                        if from < reordered.len() {
                            let moved = reordered.remove(from);
                            let insert_at = to.min(reordered.len());
                            reordered.insert(insert_at, moved);
                            s.items = reordered;
                        }
                        s.dragging_index = None;
                        s.hover_index = None;
                        ctx.notify();
                    });

                    if let Some(ref callback) = on_reorder {
                        let reordered_items = state_drop.read(cx).items.clone();
                        callback(reordered_items, window, cx);
                    }
                })
                .when(is_dragging, |d| d.opacity(0.5));

            container = container.child(item_el);
        }

        container.map(|mut this| {
            this.style().refine(&user_style);
            this
        })
    }
}
