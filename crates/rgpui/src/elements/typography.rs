//! 排印辅助。
//!
//! 标题/正文/链接的统一样式入口，避免各处手写字号颜色。

use crate::{prelude::*, *};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// Link 实例计数器（元素 ID 唯一，避免同页多实例冲突）。
static LINK_ID: AtomicU64 = AtomicU64::new(0);

/// 标题级别。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TitleLevel {
    /// H1（最大）。
    H1,
    /// H2。
    #[default]
    H2,
    /// H3。
    H3,
    /// H4。
    H4,
    /// H5（最小）。
    H5,
}

impl TitleLevel {
    /// 对应字号。
    fn size(&self) -> Pixels {
        match self {
            TitleLevel::H1 => px(24.0),
            TitleLevel::H2 => px(20.0),
            TitleLevel::H3 => px(16.0),
            TitleLevel::H4 => px(14.0),
            TitleLevel::H5 => px(12.0),
        }
    }
}

/// 标题。
#[derive(IntoElement)]
pub struct Title {
    /// 级别。
    level: TitleLevel,
    /// 文本。
    text: SharedString,
}

impl Title {
    /// 创建标题（默认 H2）。
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            level: TitleLevel::H2,
            text: text.into(),
        }
    }

    /// 设置级别。
    pub fn level(mut self, level: TitleLevel) -> Self {
        self.level = level;
        self
    }
}

impl RenderOnce for Title {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .text_size(self.level.size())
            .text_color(cx.theme().tokens.foreground.color)
            .child(self.text)
    }
}

/// 正文变体。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextVariant {
    /// 默认正文。
    #[default]
    Body,
    /// 次要（灰）。
    Secondary,
    /// 危险（红）。
    Danger,
    /// 警告（黄）。
    Warning,
}

/// 正文段落。
#[derive(IntoElement)]
pub struct Paragraph {
    /// 变体。
    variant: TextVariant,
    /// 文本。
    text: SharedString,
}

impl Paragraph {
    /// 创建正文段落。
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            variant: TextVariant::Body,
            text: text.into(),
        }
    }

    /// 设置变体。
    pub fn variant(mut self, variant: TextVariant) -> Self {
        self.variant = variant;
        self
    }
}

impl RenderOnce for Paragraph {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let tokens = &cx.theme().tokens;
        let color = match self.variant {
            TextVariant::Body => tokens.foreground.color,
            TextVariant::Secondary => tokens.muted_foreground.color,
            TextVariant::Danger => red(),
            TextVariant::Warning => yellow(),
        };
        div().text_sm().text_color(color).child(self.text)
    }
}

/// 链接（下划线 + 点击回调）。
#[derive(IntoElement)]
pub struct Link {
    /// 元素 ID（默认唯一生成）。
    id: SharedString,
    /// 文本。
    text: SharedString,
    /// 点击回调。
    on_click: Option<Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static>>,
}

impl Link {
    /// 创建链接。
    pub fn new(text: impl Into<SharedString>) -> Self {
        let id = LINK_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            id: SharedString::from(format!("link-{id}")),
            text: text.into(),
            on_click: None,
        }
    }

    /// 设置点击回调。
    pub fn on_click<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_click = Some(Arc::new(f));
        self
    }
}

impl RenderOnce for Link {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let accent = cx.theme().tokens.accent.color;
        div()
            .id(self.id.clone())
            .text_sm()
            .text_color(accent)
            .text_decoration_1()
            .cursor_pointer()
            .child(self.text)
            .when_some(self.on_click, |this, cb| {
                this.on_click(move |_, window, cx| cb(window, cx))
            })
    }
}
