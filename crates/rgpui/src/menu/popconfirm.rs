//! 气泡确认框。
//!
//! 点击触发器弹出小浮层二次确认（确定/取消），比 `AlertDialog` 更轻。
//! 开关状态由 `Popover` 内部管理，确认/取消后自动关闭。

use crate::{prelude::*, *};
use std::sync::Arc;

/// 气泡确认框。
#[derive(IntoElement)]
pub struct Popconfirm {
    /// 实例 ID（元素 ID 前缀，跨帧稳定；默认调用点生成）。
    instance: SharedString,
    /// 触发器按钮文本。
    trigger_label: SharedString,
    /// 确认提示文本。
    message: SharedString,
    /// 确认回调。
    on_confirm: Option<Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static>>,
    /// 取消回调。
    on_cancel: Option<Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl Popconfirm {
    /// 创建气泡确认框（默认 ID 取调用点，跨帧稳定，弹层状态存得住；
    /// 循环内多实例必须显式 `.id()`）。
    #[track_caller]
    pub fn new(trigger_label: impl Into<SharedString>, message: impl Into<SharedString>) -> Self {
        Self {
            instance: crate::caller_element_id("popconfirm"),
            trigger_label: trigger_label.into(),
            message: message.into(),
            on_confirm: None,
            on_cancel: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置实例 ID（默认调用点生成；循环内多实例必须显式设置）。
    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.instance = id.into();
        self
    }

    /// 设置确认回调。
    pub fn on_confirm<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_confirm = Some(Arc::new(f));
        self
    }

    /// 设置取消回调。
    pub fn on_cancel<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_cancel = Some(Arc::new(f));
        self
    }
}

impl Styled for Popconfirm {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Popconfirm {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let border = theme.tokens.border;
        let popover_bg = theme.tokens.popover;
        let muted_foreground = theme.tokens.muted_foreground.color;
        let user_style = self.style;

        let instance = self.instance;
        let message = self.message;
        let on_confirm = self.on_confirm;
        let on_cancel = self.on_cancel;

        Popover::new(SharedString::from(format!("popconfirm-{instance}")))
            .trigger(
                Button::new(SharedString::from(format!("popconfirm-{instance}-trigger")))
                    .label(self.trigger_label),
            )
            .content(move |_, _window, cx| {
                let popover = cx.entity();
                let on_confirm = on_confirm.clone();
                let on_cancel = on_cancel.clone();
                div()
                    .flex()
                    .flex_col()
                    .w(px(220.0))
                    .bg(popover_bg)
                    .border_1()
                    .border_color(border)
                    .rounded_md()
                    .p(px(12.0))
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted_foreground)
                            .child(message.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap(px(8.0))
                            .child({
                                let popover = popover.clone();
                                Button::new(SharedString::from(format!(
                                    "popconfirm-{instance}-cancel"
                                )))
                                .ghost()
                                .small()
                                .label("取消")
                                .on_click(
                                    move |_, window, cx| {
                                        let popover = popover.clone();
                                        popover.update(cx, |state, cx| {
                                            state.dismiss(window, cx);
                                        });
                                        if let Some(ref cb) = on_cancel {
                                            cb(window, cx);
                                        }
                                    },
                                )
                            })
                            .child(
                                Button::new(SharedString::from(format!(
                                    "popconfirm-{instance}-ok"
                                )))
                                .small()
                                .label("确定")
                                .on_click(
                                    move |_, window, cx| {
                                        popover.update(cx, |state, cx| {
                                            state.dismiss(window, cx);
                                        });
                                        if let Some(ref cb) = on_confirm {
                                            cb(window, cx);
                                        }
                                    },
                                ),
                            ),
                    )
            })
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
    }
}
