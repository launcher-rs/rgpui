use crate::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, div, relative,
};

use crate::{
    ActiveTheme as _, StyledExt as _,
    dialog::{CancelDialog, ConfirmDialog},
    h_flex,
};

/// 对话框底部区域，通常包含操作按钮。
///
/// # 示例
///
/// ```ignore
/// DialogFooter::new()
///     .child(DialogClose::new().child(Button::new("cancel").label("取消")))
///     .child(Button::new("confirm").label("确认"))
/// ```
#[derive(IntoElement)]
pub struct DialogFooter {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl DialogFooter {
    /// 创建新的对话框底部元素。
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

impl ParentElement for DialogFooter {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for DialogFooter {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DialogFooter {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .gap_2()
            .justify_end()
            .line_height(relative(1.))
            .rounded_b(cx.theme().radius_lg)
            .refine_style(&self.style)
            .children(self.children)
    }
}

/// 对话框按钮辅助 trait，用于标记取消/确认按钮。
pub trait DialogFooterButton {
    /// 是否为取消按钮。
    fn is_cancel(&self) -> bool {
        false
    }

    /// 是否为确认按钮。
    fn is_action(&self) -> bool {
        false
    }
}

/// 对话框关闭包装元素，点击后触发取消对话框 action。
#[derive(IntoElement)]
pub struct DialogClose {
    children: Vec<AnyElement>,
}

impl DialogClose {
    /// 创建新的对话框关闭包装元素。
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }
}

impl ParentElement for DialogClose {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for DialogClose {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .size_full()
            .id("dialog-close")
            .on_click(move |_, window, cx| window.dispatch_action(Box::new(CancelDialog), cx))
            .children(self.children)
    }
}

/// 对话框确认包装元素，点击后触发确认对话框 action。
#[derive(IntoElement)]
pub struct DialogAction {
    children: Vec<AnyElement>,
}

impl DialogAction {
    /// 创建新的对话框确认包装元素。
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }
}

impl ParentElement for DialogAction {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for DialogAction {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .size_full()
            .id("dialog-action")
            .on_click(move |_, window, cx| window.dispatch_action(Box::new(ConfirmDialog), cx))
            .children(self.children)
    }
}
