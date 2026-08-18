use crate::{AnyElement, App, Context, IntoElement, ParentElement as _, Styled as _, Task, Window};

use crate::{
    ActiveTheme as _, Icon, IconName, IndexPath, Selectable, h_flex,
    list::{ListState, loading::Loading},
};

/// List 的代理 trait。
#[allow(unused)]
pub trait ListDelegate: Sized + 'static {
    /// 列表条目的类型，需实现 [`Selectable`] 与 [`IntoElement`]。
    type Item: Selectable + IntoElement;

    /// 当查询输入变化时调用，可在此执行搜索。
    fn perform_search(
        &mut self,
        query: &str,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        Task::ready(())
    }

    /// 返回列表中的节数量，默认为 1。
    ///
    /// 最小值为 1。
    fn sections_count(&self, cx: &App) -> usize {
        1
    }

    /// 返回指定节中的条目数量。
    ///
    /// 注意：只有条目数量 > 0 的节才会渲染。如果节有 0 个条目，
    /// 节头和节尾也会被跳过。
    fn items_count(&self, section: usize, cx: &App) -> usize;

    /// 渲染指定索引的条目。
    ///
    /// 返回 None 将跳过该条目。
    ///
    /// 注意：每个条目应有相同的高度。
    fn render_item(
        &mut self,
        ix: IndexPath,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item>;

    /// 渲染指定索引的节头，默认为 None。
    ///
    /// 注意：每个节头应有相同的高度。
    fn render_section_header(
        &mut self,
        section: usize,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        None::<AnyElement>
    }

    /// 渲染指定索引的节尾，默认为 None。
    ///
    /// 注意：每个节尾应有相同的高度。
    fn render_section_footer(
        &mut self,
        section: usize,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        None::<AnyElement>
    }

    /// 返回列表为空时要显示的元素。
    fn render_empty(
        &mut self,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        h_flex()
            .size_full()
            .justify_center()
            .text_color(cx.theme().muted_foreground.opacity(0.6))
            .child(Icon::new(IconName::Inbox).size_12())
            .into_any_element()
    }

    /// 返回 Some(AnyElement) 以渲染列表的初始状态。
    ///
    /// 这可用于在用户与列表交互之前显示一个视图。
    ///
    /// 例如：上次搜索结果，或上次选中的条目。
    ///
    /// 默认为 None，表示没有初始状态。
    fn render_initial(
        &mut self,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<AnyElement> {
        None
    }

    /// 返回加载状态以显示加载视图。
    fn loading(&self, cx: &App) -> bool {
        false
    }

    /// 返回加载时要显示的元素，默认为内置的 Skeleton 加载视图。
    fn render_loading(
        &mut self,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        Loading
    }

    /// 设置选中的索引，仅存储 ix，不执行确认。
    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    );

    /// 设置被右键点击的条目索引。
    fn set_right_clicked_index(
        &mut self,
        ix: Option<IndexPath>,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
    }

    /// 设置确认并给出选中的索引，表示用户点击了条目或按下了回车。
    ///
    /// 在 confirm 之前总是会调用 `set_selected_index`。
    fn confirm(&mut self, secondary: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
    }

    /// 取消选择，例如按下 ESC。
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {}

    /// 返回 true 以在滚动到底部时启用加载更多数据。
    ///
    /// 默认：false
    fn has_more(&self, cx: &App) -> bool {
        false
    }

    /// 返回一个阈值（n 个实体），当滚动到底部时，
    /// 剩余行数触发 `load_more`。
    ///
    /// 该值应小于首次加载的行总数。
    ///
    /// 默认：20 个实体（节头、节尾和行）
    fn load_more_threshold(&self) -> usize {
        20
    }

    /// 当列表滚动到底部时加载更多数据。
    ///
    /// 这将在后台任务中执行。
    ///
    /// 当列表接近底部时总是被调用，
    /// 因此必须检查是否还有更多数据要加载或锁定加载状态。
    fn load_more(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {}
}
