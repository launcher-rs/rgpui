//! 发送输入框（对标 AntDX Sender）。
//!
//! 多行输入 + 发送按钮组合：回车发送（Shift+回车换行）、清空输入框。
//! 状态由实体持有，父组件经 `cx.new(|cx| SenderState::new(window, cx))` 创建。

use crate::{
    input_ui::{Input, InputEvent, InputState},
    prelude::*,
    *,
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// Sender 实例计数器（发送按钮 ID 唯一，避免同页多实例冲突）。
static SENDER_ID: AtomicU64 = AtomicU64::new(0);

/// 发送输入框状态实体。
pub struct SenderState {
    /// 实例序号（元素 ID 前缀）。
    instance: u64,
    /// 输入框。
    input: Entity<InputState>,
    /// 发送回调（文本）。
    on_send: Option<Arc<dyn Fn(SharedString, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 有待触发的发送（回车时无 Window，延后到 render 触发）。
    pending_send: bool,
}

impl SenderState {
    /// 创建发送输入框（`Context<SenderState>` 内调用，父组件经 `cx.new` 间接调用）。
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("输入消息…"));

        cx.subscribe(&input, |this, _input, event, cx| match event {
            InputEvent::PressEnter { shift, .. } => {
                if !shift {
                    this.pending_send = true;
                    cx.notify();
                }
            }
            _ => {}
        })
        .detach();

        Self {
            instance: SENDER_ID.fetch_add(1, Ordering::Relaxed),
            input,
            on_send: None,
            pending_send: false,
        }
    }

    /// 设置发送回调。
    pub fn on_send<F>(mut self, f: F) -> Self
    where
        F: Fn(SharedString, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_send = Some(Arc::new(f));
        self
    }

    /// 输入框实体（供父组件聚焦等）。
    pub fn input(&self) -> &Entity<InputState> {
        &self.input
    }
}

impl Render for SenderState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 延后的发送：取出文本、清空输入框、触发回调。
        if self.pending_send {
            self.pending_send = false;
            let text = self.input.read(cx).text().to_string();
            if !text.trim().is_empty() {
                let text: SharedString = text.into();
                self.input.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                });
                if let Some(ref cb) = self.on_send.clone() {
                    cb(text, window, cx);
                }
            }
        }

        let input = self.input.clone();
        let panel = cx.entity();
        let instance = self.instance;
        div()
            .flex()
            .flex_row()
            .items_end()
            .gap(px(8.0))
            .w_full()
            .child(div().flex_1().child(Input::new(&input).w_full()))
            .child(
                Button::new(SharedString::from(format!("sender-{instance}-send")))
                    .label("发送")
                    .on_click(move |_, _, cx| {
                        panel.update(cx, |this, cx| {
                            this.pending_send = true;
                            cx.notify();
                        });
                    }),
            )
    }
}
