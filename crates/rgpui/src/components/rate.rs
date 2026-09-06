//! 星级评分。
//!
//! 点击星星打分，支持半星（`allow_half` 下点击左/右半区）。状态由父持有。

use crate::{prelude::*, *};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// Rate 实例计数器（星星元素 ID 唯一，避免同页多实例冲突）。
static RATE_ID: AtomicU64 = AtomicU64::new(0);

/// 星级评分。
#[derive(IntoElement)]
pub struct Rate {
    /// 实例序号（元素 ID 前缀）。
    instance: u64,
    /// 星星总数。
    count: usize,
    /// 当前分值（半星步进 0.5）。
    value: f32,
    /// 是否允许半星。
    allow_half: bool,
    /// 是否只读。
    disabled: bool,
    /// 变更回调。
    on_change: Option<Arc<dyn Fn(f32, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl Rate {
    /// 创建评分（默认 5 星，0 分）。
    pub fn new() -> Self {
        Self {
            instance: RATE_ID.fetch_add(1, Ordering::Relaxed),
            count: 5,
            value: 0.0,
            allow_half: false,
            disabled: false,
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置星星总数。
    pub fn count(mut self, count: usize) -> Self {
        self.count = count.max(1);
        self
    }

    /// 设置当前分值。
    pub fn value(mut self, value: f32) -> Self {
        self.value = value.max(0.0);
        self
    }

    /// 设置是否允许半星。
    pub fn allow_half(mut self, allow: bool) -> Self {
        self.allow_half = allow;
        self
    }

    /// 设置只读。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置变更回调。
    pub fn on_change<F>(mut self, f: F) -> Self
    where
        F: Fn(f32, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_change = Some(Arc::new(f));
        self
    }
}

impl Default for Rate {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for Rate {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Rate {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let accent = theme.tokens.accent.color;
        let muted_foreground = theme.tokens.muted_foreground.color;
        let user_style = self.style;
        let value = self.value;
        let count = self.count;
        let allow_half = self.allow_half;
        let disabled = self.disabled;
        let on_change = self.on_change;
        let instance = self.instance;

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .children((0..count).map(|ix| {
                let base = ix as f32 + 1.0;
                // 该星星的填充：整星 / 半星（半透明整星近似）/ 空。
                let filled = value >= base;
                let half = !filled && allow_half && value >= base - 0.5;
                let lit = filled || half;
                let on_change = on_change.clone();
                div()
                    .id(SharedString::from(format!("rate-{instance}-{ix}")))
                    .cursor_pointer()
                    .text_lg()
                    .text_color(if lit { accent } else { muted_foreground })
                    .when(half, |this| this.opacity(0.5))
                    .child(if lit { "★" } else { "☆" })
                    .when(!disabled, |this| {
                        this.on_click(move |_, window, cx| {
                            if let Some(ref cb) = on_change {
                                cb(base, window, cx);
                            }
                        })
                    })
                    .into_any_element()
            }))
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
    }
}
