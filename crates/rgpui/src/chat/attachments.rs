//! 输入附件（对标 AntDX Attachments）+ 文件卡片。
//!
//! 发送前挂载的文件列表：文件名 + 大小 + 类型图标 + 移除按钮。

use crate::{prelude::*, *};
use std::sync::Arc;

/// 单个附件。
#[derive(Clone)]
pub struct AttachmentItem {
    /// 文件名。
    pub name: SharedString,
    /// 大小描述（如 "12 KB"，可选）。
    pub size: Option<SharedString>,
}

impl AttachmentItem {
    /// 创建附件。
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            size: None,
        }
    }

    /// 设置大小描述。
    pub fn size(mut self, size: impl Into<SharedString>) -> Self {
        self.size = Some(size.into());
        self
    }
}

/// 文件卡片（单个附件展示 + 移除按钮）。
#[derive(IntoElement)]
pub struct FileCard {
    /// 附件。
    item: AttachmentItem,
    /// 移除回调。
    on_remove: Option<Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl FileCard {
    /// 创建文件卡片。
    pub fn new(item: AttachmentItem) -> Self {
        Self {
            item,
            on_remove: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置移除回调。
    pub fn on_remove<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_remove = Some(Arc::new(f));
        self
    }
}

impl Styled for FileCard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for FileCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let border = theme.tokens.border.color;
        let popover = theme.tokens.popover;
        let muted_foreground = theme.tokens.muted_foreground.color;
        let user_style = self.style;

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .px(px(10.0))
            .py(px(8.0))
            .rounded_md()
            .border_1()
            .border_color(border)
            .bg(popover)
            .child(div().text_color(muted_foreground).child(IconName::File))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap(px(2.0))
                    .child(div().text_sm().child(self.item.name))
                    .when_some(self.item.size, |this, size| {
                        this.child(div().text_xs().text_color(muted_foreground).child(size))
                    }),
            )
            .when_some(self.on_remove, |this, cb| {
                this.child(
                    Button::new("file-card-remove")
                        .ghost()
                        .small()
                        .icon(IconName::Close)
                        .on_click(move |_, window, cx| cb(window, cx)),
                )
            })
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
    }
}

/// 输入附件列表。
#[derive(IntoElement)]
pub struct Attachments {
    /// 附件列表。
    items: Vec<AttachmentItem>,
    /// 移除回调（下标）。
    on_remove: Option<Arc<dyn Fn(usize, &mut Window, &mut App) + Send + Sync + 'static>>,
}

impl Attachments {
    /// 创建附件列表。
    pub fn new(items: Vec<AttachmentItem>) -> Self {
        Self {
            items,
            on_remove: None,
        }
    }

    /// 设置移除回调。
    pub fn on_remove<F>(mut self, f: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_remove = Some(Arc::new(f));
        self
    }
}

impl RenderOnce for Attachments {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let on_remove = self.on_remove;
        div().flex().flex_row().flex_wrap().gap(px(6.0)).children(
            self.items.into_iter().enumerate().map(|(ix, item)| {
                let mut card = FileCard::new(item);
                if let Some(ref cb) = on_remove {
                    let cb = cb.clone();
                    card = card.on_remove(move |window, cx| cb(ix, window, cx));
                }
                card.into_any_element()
            }),
        )
    }
}
