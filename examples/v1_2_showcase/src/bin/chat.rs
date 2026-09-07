//! 聊天三件演示：Bubble 气泡 + MessageScroller 吸底/未读 + Marker 分隔线。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Avatar, Bounds, Context, Render, Window, WindowBounds, WindowOptions,
    chat::{
        Bubble, BubbleSide, Marker, Message, MessageGroup, MessageScroller, MessageScrollerState,
    },
    div, h_flex,
    prelude::*,
    px, size, v_flex,
};
use rgpui_platform::application;

struct ChatDemo {
    scroller: rgpui::Entity<MessageScrollerState>,
    groups: Vec<MessageGroup>,
    counter: usize,
}

impl ChatDemo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let scroller = cx.new(|_| MessageScrollerState::new());
        let _ = window;
        Self {
            scroller,
            groups: vec![MessageGroup {
                sender: "助手".into(),
                messages: vec![Message::text("你好，有什么可以帮你？")],
            }],
            counter: 0,
        }
    }

    fn push(&mut self, sender: &str, mine: bool, cx: &mut Context<Self>) {
        self.counter += 1;
        let text = if mine {
            format!("我的第 {} 条消息", self.counter)
        } else {
            format!("助手的第 {} 条回复", self.counter)
        };
        match self.groups.last_mut() {
            Some(group) if group.sender.as_str() == sender => {
                group.messages.push(Message::text(text));
            }
            _ => self.groups.push(MessageGroup {
                sender: sender.into(),
                messages: vec![Message::text(text)],
            }),
        }
        cx.notify();
    }
}

impl Render for ChatDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let groups = self.groups.clone();
        v_flex()
            .size_full()
            .child(
                h_flex()
                    .gap(px(8.0))
                    .p(px(8.0))
                    .child(
                        rgpui::Button::new("chat-push-theirs")
                            .label("追加助手消息")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.push("助手", false, cx);
                            })),
                    )
                    .child(
                        rgpui::Button::new("chat-push-mine")
                            .label("追加我的消息")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.push("我", true, cx);
                            })),
                    )
                    .child(
                        rgpui::Button::new("chat-clear")
                            .label("清空")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.groups.clear();
                                this.counter = 0;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div().flex_1().p(px(12.0)).child(
                    MessageScroller::new(self.scroller.clone()).child(
                        v_flex()
                            .gap(px(4.0))
                            .child(Marker::new("今天 09:00"))
                            .children(groups.into_iter().map(|group| {
                                let mine = group.sender.as_str() == "我";
                                let sender = group.sender.clone();
                                let initial: String =
                                    sender.chars().next().unwrap_or('?').to_string();
                                v_flex()
                                    .gap(px(2.0))
                                    .child(div().text_xs().px(px(40.0)).child(sender))
                                    .children(group.messages.into_iter().map(move |msg| {
                                        let text = match msg.content {
                                            MessageType::Text(text) => text,
                                            MessageType::CodeBlock { code, .. } => code,
                                        };
                                        Bubble::new()
                                            .side(if mine {
                                                BubbleSide::Right
                                            } else {
                                                BubbleSide::Left
                                            })
                                            .avatar(Avatar::new(initial.clone()).size(px(28.0)))
                                            .child(div().text_sm().child(text))
                                    }))
                            })),
                    ),
                ),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(560.0), px(680.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| ChatDemo::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
