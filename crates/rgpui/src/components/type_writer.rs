//! 打字机：文本逐字打出的动画状态与组件。

use std::time::Duration;

use crate::{prelude::FluentBuilder as _, *};

/// 打字机状态：保存完整文本与当前可见字符数，负责逐字计时推进。
pub struct TypeWriterState {
    full_text: SharedString,
    visible_count: usize,
    is_typing: bool,
    _cursor_visible: bool,
    version: usize,
    speed: Duration,
    on_complete: Option<Box<dyn Fn(&mut App)>>,
}

impl TypeWriterState {
    /// 创建打字机状态。
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            full_text: text.into(),
            visible_count: 0,
            is_typing: false,
            _cursor_visible: true,
            version: 0,
            speed: Duration::from_millis(50),
            on_complete: None,
        }
    }

    /// 设置每字符间隔时长。
    pub fn with_speed(mut self, speed: Duration) -> Self {
        self.speed = speed;
        self
    }

    /// 开始打字动画（逐字显示文本）。
    pub fn start(&mut self, cx: &mut Context<Self>) {
        self.visible_count = 0;
        self.is_typing = true;
        self.version += 1;
        let version = self.version;
        let total_chars = self.full_text.chars().count();
        let speed = self.speed;

        cx.spawn(async move |this, cx| {
            for i in 1..=total_chars {
                cx.background_executor().timer(speed).await;
                let should_stop = this
                    .update(cx, |state, cx| {
                        if state.version != version {
                            return true;
                        }
                        state.visible_count = i;
                        cx.notify();
                        false
                    })
                    .unwrap_or(true);

                if should_stop {
                    return;
                }
            }

            _ = this.update(cx, |state, cx| {
                if state.version != version {
                    return;
                }
                state.is_typing = false;
                if let Some(cb) = state.on_complete.take() {
                    cb(cx);
                }
                cx.notify();
            });
        })
        .detach();

        cx.notify();
    }

    /// 重置打字进度。
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.version += 1;
        self.visible_count = 0;
        self.is_typing = false;
        cx.notify();
    }

    /// 更换文本并重置进度。
    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.version += 1;
        self.full_text = text.into();
        self.visible_count = 0;
        self.is_typing = false;
        cx.notify();
    }

    /// 设置打字完成回调。
    pub fn on_complete(mut self, handler: impl Fn(&mut App) + 'static) -> Self {
        self.on_complete = Some(Box::new(handler));
        self
    }

    /// 返回当前可见文本。
    pub fn visible_text(&self) -> &str {
        let byte_end = self
            .full_text
            .char_indices()
            .nth(self.visible_count)
            .map(|(i, _)| i)
            .unwrap_or(self.full_text.len());
        &self.full_text[..byte_end]
    }

    /// 判断是否正在打字。
    pub fn is_typing(&self) -> bool {
        self.is_typing
    }

    /// 判断是否已打完。
    pub fn is_complete(&self) -> bool {
        self.visible_count >= self.full_text.chars().count()
    }
}

/// 打字机组件：显示打字机状态的内容与闪烁光标。
#[derive(IntoElement)]
pub struct TypeWriter {
    id: ElementId,
    base: Div,
    state: Entity<TypeWriterState>,
    show_cursor: bool,
    cursor_char: char,
}

impl TypeWriter {
    /// 创建打字机组件。
    pub fn new(id: impl Into<ElementId>, state: Entity<TypeWriterState>) -> Self {
        Self {
            id: id.into(),
            base: div(),
            state,
            show_cursor: true,
            cursor_char: '|',
        }
    }

    /// 是否显示闪烁光标。
    pub fn cursor(mut self, show: bool) -> Self {
        self.show_cursor = show;
        self
    }

    /// 设置光标字符。
    pub fn cursor_char(mut self, ch: char) -> Self {
        self.cursor_char = ch;
        self
    }
}

impl Styled for TypeWriter {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for TypeWriter {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let state = self.state.read(cx);
        let text = String::from(state.visible_text());
        let is_typing = state.is_typing();
        let cursor_str = SharedString::from(String::from(self.cursor_char));

        self.base
            .flex()
            .flex_row()
            .text_color(theme.tokens.foreground)
            .child(text)
            .when(
                self.show_cursor && (is_typing || !state.is_complete()),
                |el| {
                    el.child(
                        div().id(self.id).child(cursor_str).with_animation(
                            "cursor-blink",
                            Animation::new(Duration::from_millis(530))
                                .repeat()
                                .with_easing(linear),
                            |el, delta| {
                                if delta < 0.5 {
                                    el.opacity(1.0)
                                } else {
                                    el.opacity(0.0)
                                }
                            },
                        ),
                    )
                },
            )
    }
}
