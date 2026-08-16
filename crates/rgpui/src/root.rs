use crate::elements::TooltipOverlay;
use crate::{
    ActiveTheme, AnyView, App, AppContext, Context, Entity, InteractiveElement, IntoElement,
    ParentElement as _, Render, StyleRefinement, Styled, StyledExt as _, Window, div,
};

/// Root 是一个用于 App 窗口顶层视图的视图（必须是窗口中的第一个视图）。
///
/// 当前精简版仅用于管理 TooltipOverlay 覆盖层。
pub struct Root {
    /// 样式精炼
    style: StyleRefinement,
    /// 窗口主体视图
    view: AnyView,
    /// 工具提示覆盖层实体
    pub(crate) tooltip_overlay: Entity<TooltipOverlay>,
}

impl Root {
    /// 创建新的 Root 视图。
    pub fn new(view: impl Into<AnyView>, cx: &mut Context<Self>) -> Self {
        Self {
            style: StyleRefinement::default(),
            view: view.into(),
            tooltip_overlay: cx.new(|_| TooltipOverlay::new()),
        }
    }

    /// 在窗口中更新 Root 视图。
    pub fn update<F, R>(window: &mut Window, cx: &mut App, f: F) -> R
    where
        F: FnOnce(&mut Self, &mut Window, &mut Context<Self>) -> R,
    {
        let root = window
            .root::<Root>()
            .flatten()
            .expect("BUG: window first layer should be a rgpui::Root.");

        root.update(cx, |root, cx| f(root, window, cx))
    }

    /// 只读访问窗口中的 Root 视图。
    pub fn read<'a>(window: &'a Window, cx: &'a App) -> &'a Self {
        &window
            .root::<Root>()
            .expect("The window root view should be of type `rgpui::Root`.")
            .unwrap()
            .read(cx)
    }

    /// 获取此窗口的工具提示覆盖层实体。
    pub(crate) fn tooltip_overlay(window: &Window, cx: &App) -> Option<Entity<TooltipOverlay>> {
        let root = window.root::<Root>()??;
        Some(root.read(cx).tooltip_overlay.clone())
    }

    /// 返回 Root 的根视图。
    pub fn view(&self) -> &AnyView {
        &self.view
    }
}

impl Styled for Root {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(cx.theme().font_size);

        div()
            .id("root")
            .relative()
            .size_full()
            .font_family(cx.theme().font_family.clone())
            .bg(cx.theme().tokens.background)
            .text_color(cx.theme().foreground)
            .refine_style(&self.style)
            .child(self.view.clone())
            .child(self.tooltip_overlay.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    /// 测试 Root 创建
    #[test]
    fn test_root_creation() {
        // 在无窗口上下文中验证类型可用性
        let _ = std::any::type_name::<Root>;
        let _ = std::any::type_name::<TestView>;
    }
}
