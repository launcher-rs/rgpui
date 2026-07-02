use rgpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, div, relative,
};

use crate::{
    ActiveTheme as _, StyledExt as _,
    dialog::{CancelDialog, ConfirmDialog},
    h_flex,
};

/// 对话框底部区域组件
///
/// 通常包含操作按钮（确认、取消等），位于对话框最下方。
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
    /// 样式 refinement
    style: StyleRefinement,
    /// 子元素列表
    children: Vec<AnyElement>,
}

impl DialogFooter {
    /// 创建新的对话框底部区域组件
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

impl ParentElement for DialogFooter {
    /// 向底部区域中添加子元素
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for DialogFooter {
    /// 获取可修改的样式引用
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DialogFooter {
    /// 渲染对话框底部区域
    ///
    /// 使用水平弹性布局，子元素靠右对齐，带底部圆角。
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

/// 对话框底部按钮 trait
///
/// 用于标识按钮是取消类型还是操作类型。
pub trait DialogFooterButton {
    /// 判断是否为取消按钮
    fn is_cancel(&self) -> bool {
        false
    }

    /// 判断是否为操作按钮
    fn is_action(&self) -> bool {
        false
    }
}

/// 对话框关闭按钮包装组件
///
/// 点击时触发 CancelDialog 动作，用于包裹需要关闭对话框的按钮。
#[derive(IntoElement)]
pub struct DialogClose {
    /// 子元素列表
    children: Vec<AnyElement>,
}

impl DialogClose {
    /// 创建新的对话框关闭按钮包装组件
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }
}

impl ParentElement for DialogClose {
    /// 向关闭按钮中添加子元素
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for DialogClose {
    /// 渲染对话框关闭按钮
    ///
    /// 点击时派发 CancelDialog 动作以关闭对话框。
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .size_full()
            .id("dialog-close")
            .on_click(move |_, window, cx| window.dispatch_action(Box::new(CancelDialog), cx))
            .children(self.children)
    }
}

/// 对话框确认操作按钮包装组件
///
/// 点击时触发 ConfirmDialog 动作，用于包裹需要确认操作的按钮。
#[derive(IntoElement)]
pub struct DialogAction {
    /// 子元素列表
    children: Vec<AnyElement>,
}

impl DialogAction {
    /// 创建新的对话框确认操作按钮包装组件
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }
}

impl ParentElement for DialogAction {
    /// 向操作按钮中添加子元素
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for DialogAction {
    /// 渲染对话框确认操作按钮
    ///
    /// 点击时派发 ConfirmDialog 动作以确认操作。
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .size_full()
            .id("dialog-action")
            .on_click(move |_, window, cx| window.dispatch_action(Box::new(ConfirmDialog), cx))
            .children(self.children)
    }
}
