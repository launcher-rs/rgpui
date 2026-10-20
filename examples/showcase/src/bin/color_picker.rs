//! 取色器增强示例
//!
//! 演示 [`ColorPickerState`] 的主题色板 / 最近颜色快捷色块，以及可独立复用的
//! [`ColorPalette`]：
//! - 取色器（三滑块 + hex + 主题色板 + 最近颜色）：点击色块即应用并记录；
//! - hex 输入框回车把当前颜色记入最近颜色；
//! - 独立色板：把命名颜色色阶（`ColorName::all()` 的 500 色阶）铺成一格一格，
//!   点击直接应用到取色器；
//! - 「记录当前颜色 / 清空最近」按钮演示程序化维护最近颜色。
//!
//! 运行：`cargo run -p showcase --bin color_picker`

use rgpui::{
    App, Bounds, Button, Context, Entity, Hsla, IntoElement, ParentElement, Render, SharedString,
    Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions,
    components::{ColorPalette, ColorPickerState, color_name_palette},
    div, hsla,
    prelude::*,
    px, size,
};
use rgpui_platform::application;

/// 取色器增强示例应用。
struct ColorPickerApp {
    /// 指向自身的句柄。
    handle: Entity<Self>,
    /// 取色器（含主题色板 / 最近颜色）。
    picker: Entity<ColorPickerState>,
    /// 当前颜色。
    color: Hsla,
    /// 状态提示。
    status: SharedString,
}

impl ColorPickerApp {
    /// 创建示例应用。
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let handle = cx.entity();
        let initial = hsla(210.0 / 360.0, 0.6, 0.5, 1.0);
        let picker = cx.new(|cx| {
            ColorPickerState::new(window, cx).color(initial).on_change({
                let handle = handle.clone();
                move |color, _window, cx| {
                    handle.update(cx, |this, cx| {
                        this.color = color;
                        cx.notify();
                    });
                }
            })
        });
        Self {
            handle,
            picker,
            color: initial,
            status: "点击色板色块、拖动滑块或输入 hex（回车提交）".into(),
        }
    }

    /// 把取色器当前颜色记入最近颜色。
    fn remember(&mut self, cx: &mut Context<Self>) {
        let color = self.picker.read(cx).color_value();
        let changed = self
            .picker
            .update(cx, |picker, cx| picker.push_recent(color, cx));
        self.status = if changed {
            "已记入最近颜色".into()
        } else {
            "该颜色已排在最近颜色首位".into()
        };
        cx.notify();
    }

    /// 清空最近颜色。
    fn clear(&mut self, cx: &mut Context<Self>) {
        self.picker
            .update(cx, |picker, cx| picker.clear_recents(cx));
        self.status = "已清空最近颜色".into();
        cx.notify();
    }
}

impl Render for ColorPickerApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let handle = self.handle.clone();

        // 独立色板：命名颜色 500 色阶，点击直接应用到取色器（复用 `ColorPalette`）。
        let picker_for_scale = self.picker.clone();
        let scale_palette = ColorPalette::new("scale-500")
            .colors(color_name_palette(500))
            .selected(Some(self.color))
            .swatch_size(20.0)
            .gap(6.0)
            .on_select(move |color, window, cx| {
                picker_for_scale.update(cx, |picker, cx| picker.set_color(color, window, cx));
            });

        let hex = hex_string(self.color);

        div()
            .flex()
            .flex_row()
            .gap_6()
            .p_6()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(div().text_xl().child("取色器增强示例"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.status.clone()),
                    )
                    .child(self.picker.clone())
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(Button::new("remember").label("记录当前颜色").on_click({
                                let handle = handle.clone();
                                move |_event, _window, cx| {
                                    handle.update(cx, |this, cx| this.remember(cx))
                                }
                            }))
                            .child(Button::new("clear").label("清空最近").on_click({
                                move |_event, _window, cx| {
                                    handle.update(cx, |this, cx| this.clear(cx))
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .flex_1()
                    .child(div().text_lg().child("预览"))
                    .child(
                        div()
                            .p_4()
                            .rounded_lg()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(self.color)
                            .child(
                                div()
                                    .text_color(contrast_foreground(self.color))
                                    .child(format!("当前颜色 {hex}")),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("命名色阶色板（500）—— 点击直接应用"),
                    )
                    .child(scale_palette),
            )
    }
}

/// 颜色转 `#rrggbb` 文本。
fn hex_string(color: Hsla) -> String {
    let rgba = color.to_rgb();
    let channel = |value: f32| (value * 255.0).round().clamp(0.0, 255.0) as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(rgba.r),
        channel(rgba.g),
        channel(rgba.b)
    )
}

/// 依亮度选前景色（浅色背景用深色文字，反之用白色）。
fn contrast_foreground(color: Hsla) -> Hsla {
    if color.l > 0.55 {
        hsla(0.0, 0.0, 0.05, 1.0)
    } else {
        hsla(0.0, 0.0, 1.0, 1.0)
    }
}

fn main() {
    rgpui::init_logging();
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(880.), px(620.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Color Picker Example".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| ColorPickerApp::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}
