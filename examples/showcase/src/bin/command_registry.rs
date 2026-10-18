//! command registry 演示：`register_command!` 登记命令元数据 → `CommandRegistry` 读取 →
//! 点击按 id 派发（`App::dispatch_command`）→ 视图 `on_action` 处理。
//!
//! 展示了套件类应用「一切皆命令」的骨架：UI、快捷键、命令面板、CLI、MCP 共用同一份
//! 命令表，按 id 分发，与具体前端解耦。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, CommandDescriptor, CommandRegistry, Context, FocusHandle, Render, Window,
    WindowBounds, WindowOptions, actions, div, h_flex, prelude::*, px, register_command, size,
    v_flex,
};
use rgpui_platform::application;

actions!(demo, [Increment, Decrement, Reset]);

/// 「增加」命令的描述符。
fn cmd_increment() -> CommandDescriptor {
    CommandDescriptor::new("demo::Increment", "增加")
        .with_category("计数")
        .with_description("把计数加一")
        .with_keystroke("ctrl-i")
}

/// 「减少」命令的描述符。
fn cmd_decrement() -> CommandDescriptor {
    CommandDescriptor::new("demo::Decrement", "减少")
        .with_category("计数")
        .with_description("把计数减一")
        .with_keystroke("ctrl-d")
}

/// 「重置」命令的描述符。
fn cmd_reset() -> CommandDescriptor {
    CommandDescriptor::new("demo::Reset", "重置")
        .with_category("计数")
        .with_description("把计数清零")
}

register_command!(cmd_increment);
register_command!(cmd_decrement);
register_command!(cmd_reset);

/// 演示视图：显示计数，并列出注册表中的全部命令。
struct CommandRegistryDemo {
    count: i32,
    focus_handle: FocusHandle,
}

impl CommandRegistryDemo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        Self {
            count: 0,
            focus_handle,
        }
    }

    fn on_increment(&mut self, _: &Increment, _window: &mut Window, cx: &mut Context<Self>) {
        self.count += 1;
        cx.notify();
    }

    fn on_decrement(&mut self, _: &Decrement, _window: &mut Window, cx: &mut Context<Self>) {
        self.count -= 1;
        cx.notify();
    }

    fn on_reset(&mut self, _: &Reset, _window: &mut Window, cx: &mut Context<Self>) {
        self.count = 0;
        cx.notify();
    }
}

impl Render for CommandRegistryDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let registry = CommandRegistry::global();

        let rows = registry.iter().map(|descriptor| {
            let id = descriptor.id().clone();
            let shortcut = descriptor
                .default_keystrokes()
                .first()
                .map(|key| key.to_string())
                .unwrap_or_default();
            let label = format!("{}（{}）{}", descriptor.label(), descriptor.id(), shortcut);
            div()
                .id(descriptor.id().clone())
                .px(px(10.0))
                .py(px(6.0))
                .rounded_md()
                .bg(rgpui::rgb(0xeef2ff))
                .cursor_pointer()
                .text_sm()
                .hover(|style| style.bg(rgpui::rgb(0xdfe6ff)))
                .child(label)
                .on_click(move |_, _window, cx| {
                    if let Err(error) = cx.dispatch_command(&id, None) {
                        log::warn!("派发命令 `{id}` 失败：{error}");
                    }
                })
        });

        let summary = format!(
            "计数：{}  ｜  已登记 {} 条命令，分类：{}",
            self.count,
            registry.len(),
            registry
                .categories()
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" / ")
        );

        v_flex()
            .id("command-registry-demo")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_increment))
            .on_action(cx.listener(Self::on_decrement))
            .on_action(cx.listener(Self::on_reset))
            .size_full()
            .gap(px(10.0))
            .p(px(16.0))
            .bg(rgpui::rgb(0xf8fafc))
            .child(
                div()
                    .text_lg()
                    .font_weight(rgpui::FontWeight::BOLD)
                    .child("命令注册表 demo"),
            )
            .child(div().text_sm().child(summary))
            .child(
                div()
                    .text_xs()
                    .text_color(rgpui::rgb(0x64748b))
                    .child("点击任意命令按 id 派发；也可用 Ctrl+I / Ctrl+D（需先加载快捷键）。"),
            )
            .child(
                h_flex()
                    .gap(px(8.0))
                    .items_center()
                    .child(
                        div()
                            .id("cmd-increment")
                            .px(px(10.0))
                            .py(px(4.0))
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .child("增加（demo::Increment）")
                            .on_click(|_, _window, cx| {
                                let _ = cx.dispatch_command("demo::Increment", None);
                            }),
                    )
                    .child(
                        div()
                            .id("cmd-decrement")
                            .px(px(10.0))
                            .py(px(4.0))
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .child("减少（demo::Decrement）")
                            .on_click(|_, _window, cx| {
                                let _ = cx.dispatch_command("demo::Decrement", None);
                            }),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(4.0))
                    .flex_1()
                    .overflow_hidden()
                    .children(rows),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(560.0), px(480.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| CommandRegistryDemo::new(window, cx)),
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
