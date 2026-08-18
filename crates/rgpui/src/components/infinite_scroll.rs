//! 无限滚动组件：滚动到底部自动加载更多内容。

use crate::{prelude::FluentBuilder as _, *};
use std::rc::Rc;

/// 加载状态。
#[derive(Clone, Debug, PartialEq)]
pub enum LoadingState {
    /// 空闲（可加载）。
    Idle,
    /// 正在加载。
    Loading,
    /// 加载完成。
    Loaded,
    /// 加载出错（携带错误信息）。
    Error(SharedString),
    /// 已加载到底。
    EndReached,
}

/// 无限滚动状态：管理加载状态、页码与滚动句柄。
pub struct InfiniteScrollState {
    /// 加载状态。
    loading_state: LoadingState,
    /// 当前页码。
    page: usize,
    /// 是否还有更多数据。
    has_more: bool,
    /// 滚动句柄（用于判断滚动位置）。
    scroll_handle: ScrollHandle,
}

impl InfiniteScrollState {
    /// 创建状态实体。
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            loading_state: LoadingState::Idle,
            page: 0,
            has_more: true,
            scroll_handle: ScrollHandle::new(),
        })
    }

    /// 获取加载状态。
    pub fn loading_state(&self) -> &LoadingState {
        &self.loading_state
    }

    /// 获取当前页码。
    pub fn page(&self) -> usize {
        self.page
    }

    /// 标记开始加载。
    pub fn set_loading(&mut self) {
        self.loading_state = LoadingState::Loading;
    }

    /// 标记加载完成。
    pub fn set_loaded(&mut self) {
        self.page += 1;
        self.has_more = true;
        self.loading_state = LoadingState::Idle;
    }

    /// 标记加载出错。
    pub fn set_error(&mut self, msg: impl Into<SharedString>) {
        self.loading_state = LoadingState::Error(msg.into());
    }

    /// 标记已到达底部。
    pub fn set_end_reached(&mut self) {
        self.has_more = false;
        self.loading_state = LoadingState::EndReached;
    }

    /// 重置状态。
    pub fn reset(&mut self) {
        self.page = 0;
        self.has_more = true;
        self.loading_state = LoadingState::Idle;
    }

    /// 获取滚动句柄。
    pub fn scroll_handle(&self) -> &ScrollHandle {
        &self.scroll_handle
    }
}

/// 无限滚动组件。
#[derive(IntoElement)]
pub struct InfiniteScroll {
    /// 元素 ID（默认由调用位置生成）。
    id: ElementId,
    /// 绑定状态实体。
    state: Entity<InfiniteScrollState>,
    /// 触发加载的滚动比例阈值（0~1）。
    threshold: f32,
    /// 加载更多回调（携带页码）。
    on_load_more: Option<Rc<dyn Fn(usize, &mut Window, &mut App)>>,
    /// 自定义加载指示器。
    loading_indicator: Option<AnyElement>,
    /// 自定义底部提示。
    end_indicator: Option<AnyElement>,
    /// 子元素列表。
    children: Vec<AnyElement>,
    /// 用户样式。
    style: StyleRefinement,
}

impl InfiniteScroll {
    /// 创建无限滚动组件（ID 由调用位置自动生成）。
    #[track_caller]
    pub fn new(state: Entity<InfiniteScrollState>) -> Self {
        let location = std::panic::Location::caller();
        Self {
            id: ElementId::Name(
                format!(
                    "infinite-scroll:{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
                .into(),
            ),
            state,
            threshold: 0.8,
            on_load_more: None,
            loading_indicator: None,
            end_indicator: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// 设置触发加载的滚动阈值。
    pub fn threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// 设置加载更多回调。
    pub fn on_load_more(
        mut self,
        callback: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_load_more = Some(Rc::new(callback));
        self
    }

    /// 设置自定义加载指示器。
    pub fn loading_indicator(mut self, indicator: impl IntoElement) -> Self {
        self.loading_indicator = Some(indicator.into_any_element());
        self
    }

    /// 设置自定义底部提示。
    pub fn end_indicator(mut self, indicator: impl IntoElement) -> Self {
        self.end_indicator = Some(indicator.into_any_element());
        self
    }
}

impl ParentElement for InfiniteScroll {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for InfiniteScroll {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for InfiniteScroll {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;
        let font_family = theme.font_family.clone();
        let muted_foreground = theme.tokens.muted_foreground;
        let error_color = theme.highlight_theme.style.status.error_border(cx);
        let (loading_state, scroll_handle) = {
            let s = self.state.read(cx);
            (s.loading_state.clone(), s.scroll_handle.clone())
        };

        let mut container = div()
            .id(self.id)
            .overflow_y_scroll()
            .track_scroll(&scroll_handle)
            .flex()
            .flex_col()
            .size_full();

        if let Some(callback) = self.on_load_more {
            let state_c = self.state.clone();
            let threshold = self.threshold;
            container = container.on_scroll_wheel(move |_event, window, cx| {
                let (should_load, page) = {
                    let s = state_c.read(cx);
                    if s.loading_state != LoadingState::Idle || !s.has_more {
                        return;
                    }
                    let handle = &s.scroll_handle;
                    let offset_y = (-handle.offset().y).max(px(0.0));
                    let max_y = handle.max_offset().y;
                    if max_y > px(0.0) && offset_y >= max_y * threshold {
                        (true, s.page)
                    } else {
                        return;
                    }
                };
                if should_load {
                    callback(page, window, cx);
                }
            });
        }

        container = container.children(self.children);

        match loading_state {
            LoadingState::Loading => {
                let indicator = self.loading_indicator.unwrap_or_else(|| {
                    div()
                        .flex()
                        .justify_center()
                        .py(px(16.0))
                        .child(Spinner::new())
                        .into_any_element()
                });
                container = container.child(indicator);
            }
            LoadingState::EndReached => {
                let indicator = self.end_indicator.unwrap_or_else(|| {
                    div()
                        .flex()
                        .justify_center()
                        .py(px(16.0))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .text_color(muted_foreground)
                                .font_family(font_family.clone())
                                .child("No more items"),
                        )
                        .into_any_element()
                });
                container = container.child(indicator);
            }
            LoadingState::Error(ref msg) => {
                container = container.child(
                    div().flex().justify_center().py(px(16.0)).child(
                        div()
                            .text_size(px(14.0))
                            .text_color(error_color)
                            .font_family(font_family)
                            .child(msg.clone()),
                    ),
                );
            }
            _ => {}
        }

        container.map(|mut this| {
            this.style().refine(&user_style);
            this
        })
    }
}
