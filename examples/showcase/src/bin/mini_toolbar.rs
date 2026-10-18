//! 迷你工具栏（MiniToolbar）演示：在「文档」上拖动选择后就近浮出紧凑命令条，
//! 按钮与 QAT 同源（`QatItem`，按命令 id 派发）。
//!
//! 对应 `rgpui` 的 `ribbon` feature（`rgpui::mini_toolbar` + `rgpui::qat`）。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::mini_toolbar::MiniToolbar;
use rgpui::qat::QatItem;
use rgpui::{
    App, Bounds, CommandDescriptor, Context, FocusHandle, IconName, MouseButton, Pixels, Render,
    SharedString, Window, WindowBounds, WindowOptions, actions, div, point, prelude::*, px,
    register_command, rgb, size, v_flex,
};
use rgpui_platform::application;

/// 页面顶部说明条高度（此区域不参与选择）。
const HEADER_H: Pixels = px(56.0);

actions!(minitoolbar, [Bold, Italic, Underline, Highlight, Copy]);

/// 「加粗」命令描述符。
fn cmd_bold() -> CommandDescriptor {
    CommandDescriptor::new("minitoolbar::Bold", "加粗")
        .with_category("格式")
        .with_description("切换选区加粗")
}

/// 「斜体」命令描述符。
fn cmd_italic() -> CommandDescriptor {
    CommandDescriptor::new("minitoolbar::Italic", "斜体").with_category("格式")
}

/// 「下划线」命令描述符。
fn cmd_underline() -> CommandDescriptor {
    CommandDescriptor::new("minitoolbar::Underline", "下划线").with_category("格式")
}

/// 「高亮」命令描述符。
fn cmd_highlight() -> CommandDescriptor {
    CommandDescriptor::new("minitoolbar::Highlight", "高亮").with_category("格式")
}

/// 「复制」命令描述符。
fn cmd_copy() -> CommandDescriptor {
    CommandDescriptor::new("minitoolbar::Copy", "复制")
        .with_category("剪贴板")
        .with_keystroke("ctrl-c")
}

register_command!(cmd_bold);
register_command!(cmd_italic);
register_command!(cmd_underline);
register_command!(cmd_highlight);
register_command!(cmd_copy);

/// 选区（起点 → 终点，窗口坐标）。
type Selection = (rgpui::Point<Pixels>, rgpui::Point<Pixels>);

/// 演示视图：假文档 + 拖动选择 + 就近浮出的迷你工具栏。
struct MiniToolbarDemo {
    focus_handle: FocusHandle,
    dragging: bool,
    selection: Option<Selection>,
    bold: bool,
    italic: bool,
    underline: bool,
    highlight: bool,
    status: SharedString,
}

impl MiniToolbarDemo {
    /// 创建视图并聚焦自身。
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        Self {
            focus_handle,
            dragging: false,
            selection: None,
            bold: false,
            italic: false,
            underline: false,
            highlight: false,
            status: SharedString::from("在下方文档区按住左键拖动即可选中。"),
        }
    }

    /// 设置状态栏文本。
    fn set_status(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.status = text.into();
        cx.notify();
    }

    fn on_bold(&mut self, _: &Bold, _window: &mut Window, cx: &mut Context<Self>) {
        self.bold = !self.bold;
        let state = if self.bold { "开" } else { "关" };
        self.set_status(format!("加粗：{state}"), cx);
    }

    fn on_italic(&mut self, _: &Italic, _window: &mut Window, cx: &mut Context<Self>) {
        self.italic = !self.italic;
        let state = if self.italic { "开" } else { "关" };
        self.set_status(format!("斜体：{state}"), cx);
    }

    fn on_underline(&mut self, _: &Underline, _window: &mut Window, cx: &mut Context<Self>) {
        self.underline = !self.underline;
        let state = if self.underline { "开" } else { "关" };
        self.set_status(format!("下划线：{state}"), cx);
    }

    fn on_highlight(&mut self, _: &Highlight, _window: &mut Window, cx: &mut Context<Self>) {
        self.highlight = !self.highlight;
        let state = if self.highlight { "开" } else { "关" };
        self.set_status(format!("高亮：{state}"), cx);
    }

    fn on_copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
        self.set_status("已复制选区（演示）", cx);
    }

    /// 当前格式状态的文字摘要。
    fn format_summary(&self) -> String {
        let mark = |on: bool| if on { "开" } else { "关" };
        format!(
            "加粗 {} ｜ 斜体 {} ｜ 下划线 {} ｜ 高亮 {}",
            mark(self.bold),
            mark(self.italic),
            mark(self.underline),
            mark(self.highlight)
        )
    }

    /// 迷你工具栏的落点：选区终点右下偏移，避免遮住选区。
    fn toolbar_at(selection: Selection) -> rgpui::Point<Pixels> {
        point(selection.1.x + px(4.0), selection.1.y + px(18.0))
    }
}

impl Render for MiniToolbarDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selection = self.selection;
        let summary = self.format_summary();
        let status = self.status.clone();

        // 选区高亮矩形（根元素原点即窗口原点，坐标可直接用）。
        let selection_rect = selection.map(|(a, b)| {
            let origin = point(a.x.min(b.x), a.y.min(b.y));
            let extent = size((a.x - b.x).abs(), (a.y - b.y).abs());
            div()
                .absolute()
                .left(origin.x)
                .top(origin.y)
                .w(extent.width)
                .h(extent.height)
                .bg(rgb(0x60a5fa).opacity(0.25))
        });

        div()
            .id("mini-toolbar-demo")
            .relative()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_bold))
            .on_action(cx.listener(Self::on_italic))
            .on_action(cx.listener(Self::on_underline))
            .on_action(cx.listener(Self::on_highlight))
            .on_action(cx.listener(Self::on_copy))
            .on_key_down(
                cx.listener(|this, event: &rgpui::KeyDownEvent, _window, cx| {
                    if event.keystroke.key == "escape" && this.selection.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &rgpui::MouseDownEvent, _window, cx| {
                    if event.position.y < HEADER_H {
                        return;
                    }
                    this.dragging = true;
                    this.selection = Some((event.position, event.position));
                    cx.notify();
                }),
            )
            .on_mouse_move(
                cx.listener(|this, event: &rgpui::MouseMoveEvent, _window, cx| {
                    if let Some((start, _)) = this.selection
                        && this.dragging
                    {
                        this.selection = Some((start, event.position));
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _event: &rgpui::MouseUpEvent, _window, cx| {
                    this.dragging = false;
                    cx.notify();
                }),
            )
            .size_full()
            .bg(rgb(0xf8fafc))
            // 说明条（覆盖在顶部，不参与选择）。
            .child(
                v_flex()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .h(HEADER_H)
                    .justify_center()
                    .gap(px(2.0))
                    .px(px(16.0))
                    .bg(rgb(0xf1f5f9))
                    .border_b_1()
                    .border_color(rgb(0xe2e8f0))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(rgpui::FontWeight::BOLD)
                            .child("迷你工具栏（MiniToolbar）demo"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x64748b))
                            .child("在文档区按住左键拖动 → 就近浮出命令条；Esc 取消选择。"),
                    ),
            )
            // 假文档内容。
            .child(
                v_flex()
                    .absolute()
                    .top(HEADER_H)
                    .left_0()
                    .w_full()
                    .p(px(24.0))
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x334155))
                            .child("rgpui 的迷你工具栏与快速访问工具栏共用 QatItem："),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x334155))
                            .child("按钮按命令 id 绑定，点击经 App::dispatch_command 派发。"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x334155))
                            .child("选择这段文字试试：松开鼠标后面板会停在选区旁边。"),
                    ),
            )
            // 选区高亮。
            .children(selection_rect)
            // 迷你工具栏（就近浮出）。
            .children(selection.map(|selection| {
                MiniToolbar::new()
                    .at(Self::toolbar_at(selection))
                    .item(QatItem::button("minitoolbar::Bold").label("B"))
                    .item(QatItem::button("minitoolbar::Italic").label("I"))
                    .item(QatItem::button("minitoolbar::Underline").label("U"))
                    .item(QatItem::separator())
                    .item(QatItem::button("minitoolbar::Highlight").icon(IconName::Palette))
                    .item(QatItem::button("minitoolbar::Copy").icon(IconName::Copy))
            }))
            // 底部状态条。
            .child(
                v_flex()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .w_full()
                    .gap(px(2.0))
                    .px(px(16.0))
                    .py(px(6.0))
                    .bg(rgb(0xf1f5f9))
                    .border_t_1()
                    .border_color(rgb(0xe2e8f0))
                    .child(div().text_xs().child(status))
                    .child(div().text_xs().text_color(rgb(0x64748b)).child(summary)),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(640.0), px(420.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| MiniToolbarDemo::new(window, cx)),
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
