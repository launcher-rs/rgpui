//! 剪贴板组件：一个把给定值写入系统剪贴板的图标按钮，复制成功后短暂显示对勾。
//!
//! 用于代码块、密钥、命令等「一键复制」场景。点击后经 `App::write_to_clipboard`
//! 写入，2 秒后图标自动恢复。

use std::{sync::Arc, time::Duration};

use crate::{prelude::FluentBuilder as _, *};

/// 复制按钮。
#[derive(IntoElement)]
pub struct Clipboard {
    /// 元素 ID（同时用作内部状态键）。
    id: ElementId,
    /// 直接复制的值。
    value: SharedString,
    /// 动态取值回调，优先级高于 [`value`](Self::value)。
    value_fn: Option<Arc<dyn Fn(&mut Window, &mut App) -> SharedString + Send + Sync>>,
    /// 复制成功后的回调。
    on_copied: Option<Arc<dyn Fn(SharedString, &mut Window, &mut App) + Send + Sync>>,
    /// tooltip 文本。
    tooltip_text: Option<SharedString>,
    /// 无障碍名称，默认 `Copy`。
    aria_label: Option<SharedString>,
    /// 尺寸。
    size: ElementSize,
}

impl Clipboard {
    /// 创建复制按钮。
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: SharedString::default(),
            value_fn: None,
            on_copied: None,
            tooltip_text: None,
            aria_label: None,
            size: ElementSize::XSmall,
        }
    }

    /// 设置 tooltip 文本。
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip_text = Some(tooltip.into());
        self
    }

    /// 设置无障碍名称，默认 `Copy`。
    ///
    /// 按钮只显示图标，没有可见标签；tooltip 是提示而非名称，故单独提供此项。
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// 设置要复制的值，默认空字符串。
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = value.into();
        self
    }

    /// 用回调动态计算要复制的值（优先级高于 [`value`](Self::value)）。
    pub fn value_fn(
        mut self,
        value: impl Fn(&mut Window, &mut App) -> SharedString + Send + Sync + 'static,
    ) -> Self {
        self.value_fn = Some(Arc::new(value));
        self
    }

    /// 设置复制成功后的回调。
    pub fn on_copied<F>(mut self, handler: F) -> Self
    where
        F: Fn(SharedString, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_copied = Some(Arc::new(handler));
        self
    }
}

impl Sizable for Clipboard {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for Clipboard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| ClipboardState::default());

        let copied = state.read(cx).copied;
        let value = self.value;
        let value_fn = self.value_fn;
        let on_copied = self.on_copied;
        let aria_label = self.aria_label.unwrap_or_else(|| "Copy".into());

        Button::new(self.id)
            .icon(if copied {
                IconName::Check
            } else {
                IconName::Copy
            })
            .ghost()
            .with_size(self.size)
            .aria_label(aria_label)
            .when_some(self.tooltip_text, |this, text| this.tooltip(text))
            // 已复制时暂时不响应再次点击（图标此时是对勾）。
            .when(!copied, |this| {
                this.on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    let value = value_fn
                        .as_ref()
                        .map(|f| f(window, cx))
                        .unwrap_or_else(|| value.clone());
                    cx.write_to_clipboard(ClipboardItem::new_string(value.to_string()));
                    state.update(cx, |state, cx| {
                        state.copied = true;
                        cx.notify();
                    });

                    // 2 秒后恢复图标。
                    let state = state.clone();
                    cx.spawn(async move |cx| {
                        cx.background_executor().timer(Duration::from_secs(2)).await;
                        let _ = state.update(cx, |state, cx| {
                            state.copied = false;
                            cx.notify();
                        });
                    })
                    .detach();

                    if let Some(on_copied) = &on_copied {
                        on_copied(value, window, cx);
                    }
                })
            })
    }
}

/// 复制按钮的内部状态：标记当前是否处于「已复制」窗口期。
#[derive(Default)]
struct ClipboardState {
    /// 是否处于已复制状态。
    copied: bool,
}
