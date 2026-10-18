//! 快速访问工具栏（QAT）演示：一排按**命令 id** 绑定的紧凑图标按钮，
//! 通常置于 Ribbon / 标题栏之上；点击经 `App::dispatch_command` 派发，
//! 与 Ribbon、菜单、命令面板共用同一份命令表。
//!
//! 本例对应 `rgpui` 的 `ribbon` feature（`QatItem` / `QuickAccessToolbar`）。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::qat::{QatItem, QuickAccessToolbar};
use rgpui::{
    App, Bounds, CommandDescriptor, Context, FocusHandle, IconName, Render, SharedString, Window,
    WindowBounds, WindowOptions, actions, div, h_flex, prelude::*, px, register_command, rgb, size,
    v_flex,
};
use rgpui_platform::application;

actions!(qat, [Save, Undo, Redo, Settings, Search]);

/// 「保存」命令描述符。
fn cmd_save() -> CommandDescriptor {
    CommandDescriptor::new("qat::Save", "保存")
        .with_category("快速访问")
        .with_keystroke("ctrl-s")
}

/// 「撤销」命令描述符。
fn cmd_undo() -> CommandDescriptor {
    CommandDescriptor::new("qat::Undo", "撤销")
        .with_category("快速访问")
        .with_keystroke("ctrl-z")
}

/// 「重做」命令描述符。
fn cmd_redo() -> CommandDescriptor {
    CommandDescriptor::new("qat::Redo", "重做")
        .with_category("快速访问")
        .with_keystroke("ctrl-y")
}

/// 「设置」命令描述符。
fn cmd_settings() -> CommandDescriptor {
    CommandDescriptor::new("qat::Settings", "设置")
        .with_category("快速访问")
        .with_description("打开应用设置")
}

/// 「查找」命令描述符。
fn cmd_search() -> CommandDescriptor {
    CommandDescriptor::new("qat::Search", "查找").with_category("快速访问")
}

register_command!(cmd_save);
register_command!(cmd_undo);
register_command!(cmd_redo);
register_command!(cmd_settings);
register_command!(cmd_search);

/// 演示视图：顶部一条 QAT，下方是等价的普通按钮与最近一次命令回显。
struct QatDemo {
    last: Option<SharedString>,
    focus_handle: FocusHandle,
}

impl QatDemo {
    /// 创建视图并聚焦自身。
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        Self {
            last: None,
            focus_handle,
        }
    }

    /// 记录最近一次被派发的命令。
    fn record(&mut self, label: &str, cx: &mut Context<Self>) {
        self.last = Some(label.into());
        cx.notify();
    }

    fn on_save(&mut self, _: &Save, _window: &mut Window, cx: &mut Context<Self>) {
        self.record("保存", cx);
    }

    fn on_undo(&mut self, _: &Undo, _window: &mut Window, cx: &mut Context<Self>) {
        self.record("撤销", cx);
    }

    fn on_redo(&mut self, _: &Redo, _window: &mut Window, cx: &mut Context<Self>) {
        self.record("重做", cx);
    }

    fn on_settings(&mut self, _: &Settings, _window: &mut Window, cx: &mut Context<Self>) {
        self.record("设置", cx);
    }

    fn on_search(&mut self, _: &Search, _window: &mut Window, cx: &mut Context<Self>) {
        self.record("查找", cx);
    }
}

impl Render for QatDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let last = self
            .last
            .clone()
            .unwrap_or_else(|| SharedString::from("（尚未派发）"));

        v_flex()
            .id("qat-demo")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_save))
            .on_action(cx.listener(Self::on_undo))
            .on_action(cx.listener(Self::on_redo))
            .on_action(cx.listener(Self::on_settings))
            .on_action(cx.listener(Self::on_search))
            .size_full()
            .bg(rgb(0xf8fafc))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .px(px(8.0))
                    .py(px(4.0))
                    .bg(rgb(0xf1f5f9))
                    .border_b_1()
                    .border_color(rgb(0xe2e8f0))
                    .child(
                        QuickAccessToolbar::new()
                            .item(QatItem::button("qat::Save").icon(IconName::Check))
                            .item(QatItem::button("qat::Undo").icon(IconName::Undo))
                            .item(QatItem::button("qat::Redo").icon(IconName::Redo))
                            .item(QatItem::separator())
                            .item(QatItem::button("qat::Search").label("查找"))
                            .item(QatItem::button("qat::Settings").icon(IconName::Settings)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x64748b))
                            .child(format!("最近派发：{last}")),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(10.0))
                    .p(px(16.0))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(rgpui::FontWeight::BOLD)
                            .child("快速访问工具栏（QAT）demo"),
                    )
                    .child(
                        div().text_sm().text_color(rgb(0x475569)).child(
                            "顶部一条 QAT 按命令 id 派发；下方按钮派发同一条命令，效果一致。",
                        ),
                    )
                    .child(div().text_sm().child(format!("最近派发：{last}")))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .items_center()
                            .child(
                                div()
                                    .id("plain-save")
                                    .px(px(10.0))
                                    .py(px(4.0))
                                    .rounded_md()
                                    .bg(rgb(0xeef2ff))
                                    .cursor_pointer()
                                    .text_xs()
                                    .child("普通按钮：保存（qat::Save）")
                                    .on_click(|_, _window, cx| {
                                        let _ = cx.dispatch_command("qat::Save", None);
                                    }),
                            )
                            .child(
                                div()
                                    .id("plain-settings")
                                    .px(px(10.0))
                                    .py(px(4.0))
                                    .rounded_md()
                                    .bg(rgb(0xeef2ff))
                                    .cursor_pointer()
                                    .text_xs()
                                    .child("普通按钮：设置（qat::Settings）")
                                    .on_click(|_, _window, cx| {
                                        let _ = cx.dispatch_command("qat::Settings", None);
                                    }),
                            ),
                    ),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(620.0), px(340.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| QatDemo::new(window, cx)),
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
