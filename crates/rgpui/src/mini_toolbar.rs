//! 迷你工具栏（Mini toolbar）—— 选中文本/对象时就近浮出的紧凑命令条
//! （feature `ribbon`）。
//!
//! 与 [`QuickAccessToolbar`](crate::qat::QuickAccessToolbar) 复用同一套
//! [`QatItem`](crate::qat::QatItem)：按钮按**命令 id** 绑定，点击经
//! [`App::dispatch_command`](crate::App::dispatch_command) 派发。
//!
//! 迷你工具栏本身不决定「何时/在哪」出现，由调用方控制：用
//! [`MiniToolbar::at`] 以绝对定位钉在父容器坐标上（父容器需 `relative()`），
//! 拖动选择 / 右键 / 悬浮时按需渲染，四周留出边距避免遮住选区。
//!
//! ```ignore
//! use rgpui::mini_toolbar::MiniToolbar;
//! use rgpui::qat::QatItem;
//! use rgpui::{IconName, point, px};
//!
//! div().relative().size_full().child(
//!     MiniToolbar::new()
//!         .at(point(px(120.0), px(64.0)))
//!         .item(QatItem::button("app::Bold").icon(IconName::Asterisk))
//!         .item(QatItem::button("app::Italic").label("I")),
//! )
//! ```

use crate::qat::{QatItem, render_item};
use crate::{
    App, IntoElement, ParentElement, Pixels, Point, RenderOnce, Styled, StyledExt as _, Window,
    h_flex, px,
};

/// 迷你工具栏：选中后就近浮出的紧凑命令条。
#[derive(IntoElement)]
pub struct MiniToolbar {
    items: Vec<QatItem>,
    at: Option<Point<Pixels>>,
}

impl MiniToolbar {
    /// 创建空的迷你工具栏。
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            at: None,
        }
    }

    /// 追加一个项。
    pub fn item(mut self, item: impl Into<QatItem>) -> Self {
        self.items.push(item.into());
        self
    }

    /// 批量追加项（每一项都需可转换为 [`QatItem`]）。
    pub fn items<I>(mut self, items: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<QatItem>,
    {
        self.items.extend(items.into_iter().map(Into::into));
        self
    }

    /// 以绝对定位放置在父容器坐标 `at` 处（父容器需为 `relative()`）。
    ///
    /// 不调用时按普通流式元素渲染，交给调用方自行布局。
    pub fn at(mut self, at: Point<Pixels>) -> Self {
        self.at = Some(at);
        self
    }
}

impl RenderOnce for MiniToolbar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let bar = h_flex()
            .gap(px(1.0))
            .items_center()
            .p(px(3.0))
            .popover_style(cx)
            .children(self.items.iter().map(|item| render_item(item, cx)));

        match self.at {
            Some(at) => bar.absolute().left(at.x).top(at.y).into_any_element(),
            None => bar.into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::MiniToolbar;
    use crate::qat::QatItem;
    use crate::{IntoElement, ParentElement as _, Render, Styled as _, Window, div, point, px};

    /// 流式渲染（不指定位置）冒烟测试。
    #[rgpui::test]
    fn renders_in_flow(cx: &mut crate::TestAppContext) {
        struct Probe;
        impl Render for Probe {
            fn render(
                &mut self,
                _window: &mut Window,
                _cx: &mut crate::Context<Self>,
            ) -> impl IntoElement {
                MiniToolbar::new()
                    .item(QatItem::button("rgpui_test::Open"))
                    .item(QatItem::separator())
            }
        }

        let (_view, cx) = cx.add_window_view(|_window, _cx| Probe);
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    }

    /// 绝对定位渲染冒烟测试。
    #[rgpui::test]
    fn renders_positioned(cx: &mut crate::TestAppContext) {
        struct Probe;
        impl Render for Probe {
            fn render(
                &mut self,
                _window: &mut Window,
                _cx: &mut crate::Context<Self>,
            ) -> impl IntoElement {
                div().relative().size_full().child(
                    MiniToolbar::new()
                        .at(point(px(40.0), px(24.0)))
                        .item(QatItem::button("rgpui_test::Open")),
                )
            }
        }

        let (_view, cx) = cx.add_window_view(|_window, _cx| Probe);
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    }
}
