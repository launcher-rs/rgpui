//! 切换按钮。
//!
//! 按下态保持的按钮（与 momentary 的 `Button` 区分），附 `ToggleGroup` 单选组。

use crate::{prelude::*, *};
use std::sync::Arc;

/// 切换按钮。
#[derive(IntoElement)]
pub struct Toggle {
    /// 标签。
    label: SharedString,
    /// 是否按下。
    pressed: bool,
    /// 切换回调（参数为切换后的状态）。
    on_change: Option<Arc<dyn Fn(bool, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl Toggle {
    /// 创建切换按钮。
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            pressed: false,
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置按下状态（状态由父持有）。
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }

    /// 设置切换回调。
    pub fn on_change<F>(mut self, f: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_change = Some(Arc::new(f));
        self
    }
}

impl Styled for Toggle {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Toggle {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let accent = cx.theme().tokens.accent.color;
        let pressed = self.pressed;
        let user_style = self.style;

        Button::new(self.label.clone())
            .label(self.label)
            .map(|mut this| {
                if pressed {
                    this.style().refine(&StyleRefinement::default().bg(accent.opacity(0.15)));
                }
                this.style().refine(&user_style);
                this
            })
            .on_click(move |_, window, cx| {
                if let Some(ref cb) = self.on_change {
                    cb(!pressed, window, cx);
                }
            })
    }
}

/// 单选切换组（选项互斥，选中态由父持有）。
#[derive(IntoElement)]
pub struct ToggleGroup {
    /// 选项标签。
    options: Vec<SharedString>,
    /// 选中下标。
    selected: Option<usize>,
    /// 变更回调。
    on_change: Option<Arc<dyn Fn(usize, &mut Window, &mut App) + Send + Sync + 'static>>,
}

impl ToggleGroup {
    /// 创建切换组。
    pub fn new(options: Vec<SharedString>) -> Self {
        Self {
            options,
            selected: None,
            on_change: None,
        }
    }

    /// 设置选中下标。
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
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

impl RenderOnce for ToggleGroup {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let selected = self.selected;
        let on_change = self.on_change;
        div()
            .flex()
            .flex_row()
            .gap(px(4.0))
            .children(self.options.into_iter().enumerate().map(|(ix, label)| {
                let on_change = on_change.clone();
                Toggle::new(label)
                    .pressed(Some(ix) == selected)
                    .on_change(move |_, window, cx| {
                        if let Some(ref cb) = on_change {
                            cb(ix, window, cx);
                        }
                    })
                    .into_any_element()
            }))
    }
}
