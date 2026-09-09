//! 页码导航。
//!
//! 列表分页：上一页/页码/下一页，页码过多时收起为省略号，当前页高亮。
//! 状态（当前页）由父持有。

use crate::{prelude::*, *};
use std::sync::Arc;

/// 页码导航。
#[derive(IntoElement)]
pub struct Pagination {
    /// 实例 ID（按钮 ID 前缀，跨帧稳定；默认调用点生成）。
    instance: SharedString,
    /// 当前页（1 起始）。
    current: usize,
    /// 总页数。
    total: usize,
    /// 两侧保留页码数。
    sibling_count: usize,
    /// 变更回调。
    on_change: Option<Arc<dyn Fn(usize, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl Pagination {
    /// 创建页码导航（默认 ID 取调用点，跨帧稳定；循环内多实例必须显式 `.id()`）。
    #[track_caller]
    pub fn new(current: usize, total: usize) -> Self {
        Self {
            instance: crate::caller_element_id("pg"),
            current: current.max(1),
            total: total.max(1),
            sibling_count: 1,
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置实例 ID（默认调用点生成；循环内多实例必须显式设置）。
    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.instance = id.into();
        self
    }

    /// 设置两侧保留页码数（默认 1）。
    pub fn sibling_count(mut self, count: usize) -> Self {
        self.sibling_count = count;
        self
    }

    /// 设置变更回调。
    pub fn on_change<F>(mut self, f: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_change = Some(Arc::new(f));
        self
    }
}

impl Styled for Pagination {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// 待渲染页码项：数字或省略号。
enum PageItem {
    Number(usize),
    Ellipsis,
}

impl RenderOnce for Pagination {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let border = theme.tokens.border.color;
        let accent = theme.tokens.accent.color;
        let muted_foreground = theme.tokens.muted_foreground.color;
        let user_style = self.style;

        // 组装页码序列：1 … current±s … total。
        let mut items = vec![PageItem::Number(1)];
        let lo = self.current.saturating_sub(self.sibling_count).max(2);
        let hi = (self.current + self.sibling_count).min(self.total.saturating_sub(1));
        if lo > 2 {
            items.push(PageItem::Ellipsis);
        }
        for p in lo..=hi {
            items.push(PageItem::Number(p));
        }
        if hi < self.total.saturating_sub(1) {
            items.push(PageItem::Ellipsis);
        }
        if self.total > 1 {
            items.push(PageItem::Number(self.total));
        }

        let instance = self.instance;
        let current = self.current;
        let total = self.total;
        let on_change = self.on_change;

        let mut row = div().flex().flex_row().items_center().gap(px(4.0));
        row = row.child(page_button(
            &format!("pg-{instance}-prev"),
            "‹",
            current <= 1,
            false,
            accent,
            muted_foreground,
            border,
            on_change.clone(),
            current.saturating_sub(1).max(1),
        ));
        for item in items {
            match item {
                PageItem::Ellipsis => {
                    row = row.child(div().text_sm().text_color(muted_foreground).child("…"));
                }
                PageItem::Number(p) => {
                    row = row.child(page_button(
                        &format!("pg-{instance}-{p}"),
                        &p.to_string(),
                        false,
                        p == current,
                        accent,
                        muted_foreground,
                        border,
                        on_change.clone(),
                        p,
                    ));
                }
            }
        }
        row = row.child(page_button(
            &format!("pg-{instance}-next"),
            "›",
            current >= total,
            false,
            accent,
            muted_foreground,
            border,
            on_change,
            (current + 1).min(total),
        ));
        row.map(|mut this| {
            this.style().refine(&user_style);
            this
        })
    }
}

/// 单个页码按钮（`selected` 为当前页时高亮）。
fn page_button(
    id: &str,
    label: &str,
    disabled: bool,
    selected: bool,
    accent: Hsla,
    muted_foreground: Hsla,
    border: Hsla,
    on_change: Option<Arc<dyn Fn(usize, &mut Window, &mut App) + Send + Sync + 'static>>,
    target: usize,
) -> impl IntoElement {
    div()
        .id(SharedString::from(id))
        .px(px(8.0))
        .py(px(4.0))
        .rounded_sm()
        .border_1()
        .border_color(if selected { accent } else { border })
        .text_sm()
        .text_color(if selected { accent } else { muted_foreground })
        .when(!disabled, |this| this.cursor_pointer())
        .when(disabled, |this| this.opacity(0.4))
        .child(label.to_string())
        .on_click(move |_, window, cx| {
            if !disabled {
                if let Some(ref cb) = on_change {
                    cb(target, window, cx);
                }
            }
        })
}
