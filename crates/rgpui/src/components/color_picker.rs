//! 颜色选择器。
//!
//! 色相/饱和度/明度三滑块 + hex 输入 + 预览色块，变更经 `on_change(Hsla)` 回调。
//! 状态由实体持有，父组件经 `cx.new(|cx| ColorPickerState::new(window, cx))` 创建。

use crate::{
    input_ui::{Input, InputState},
    prelude::*,
    *,
};
use std::sync::Arc;

/// 颜色选择器状态实体。
pub struct ColorPickerState {
    /// 色相滑块（0–360）。
    hue: Entity<SliderState>,
    /// 饱和度滑块（0–100）。
    saturation: Entity<SliderState>,
    /// 明度滑块（0–100）。
    lightness: Entity<SliderState>,
    /// hex 输入框。
    hex_input: Entity<InputState>,
    /// 当前颜色。
    color: Hsla,
    /// 变更回调。
    on_change: Option<Arc<dyn Fn(Hsla, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 有待触发的变更回调（滑块/输入事件无 Window，延后到 render 触发）。
    pending_emit: bool,
    /// hex 输入框待同步（`set_value` 需要 Window，延后到 render 执行）。
    hex_dirty: bool,
}

impl ColorPickerState {
    /// 创建颜色选择器（`Context<ColorPickerState>` 内调用，父组件经 `cx.new` 间接调用）。
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let hue = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(360.0)
                .step(1.0)
                .default_value(210.0)
        });
        let saturation = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(1.0)
                .default_value(60.0)
        });
        let lightness = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(1.0)
                .default_value(50.0)
        });
        let hex_input = cx.new(|cx| InputState::new(window, cx).placeholder("#rrggbb"));

        for slider in [&hue, &saturation, &lightness] {
            cx.subscribe(slider, |this, _slider, event, cx| match event {
                SliderEvent::Change(_) => {
                    this.recompute_from_sliders(cx);
                }
                _ => {}
            })
            .detach();
        }

        cx.subscribe(&hex_input, |this, _input, event, cx| match event {
            crate::input_ui::InputEvent::Change => {
                this.recompute_from_hex(cx);
            }
            _ => {}
        })
        .detach();

        Self {
            hue,
            saturation,
            lightness,
            hex_input,
            color: hsla(210.0 / 360.0, 0.6, 0.5, 1.0),
            on_change: None,
            pending_emit: false,
            hex_dirty: true,
        }
    }

    /// 设置初始颜色。
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// 设置变更回调。
    pub fn on_change<F>(mut self, f: F) -> Self
    where
        F: Fn(Hsla, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_change = Some(Arc::new(f));
        self
    }

    /// 当前颜色。
    pub fn color_value(&self) -> Hsla {
        self.color
    }

    /// 从三滑块重算颜色。
    fn recompute_from_sliders(&mut self, cx: &mut Context<Self>) {
        let h = slider_value(&self.hue, cx);
        let s = slider_value(&self.saturation, cx);
        let l = slider_value(&self.lightness, cx);
        self.color = hsla(h / 360.0, s / 100.0, l / 100.0, 1.0);
        self.hex_dirty = true;
        self.pending_emit = true;
        cx.notify();
    }

    /// 从 hex 输入解析颜色（解析失败则忽略）。
    fn recompute_from_hex(&mut self, cx: &mut Context<Self>) {
        let text = self.hex_input.read(cx).text().to_string();
        let hex = text.trim().trim_start_matches('#');
        if hex.len() != 6 {
            return;
        }
        let rgb = u32::from_str_radix(hex, 16).ok();
        if let Some(rgb) = rgb {
            let r = ((rgb >> 16) & 0xff) as f32 / 255.0;
            let g = ((rgb >> 8) & 0xff) as f32 / 255.0;
            let b = (rgb & 0xff) as f32 / 255.0;
            self.color = rgb_to_hsla(r, g, b);
            self.pending_emit = true;
            cx.notify();
        }
    }
}

/// 读滑块单值（非单值时取 0）。
fn slider_value(state: &Entity<SliderState>, cx: &App) -> f32 {
    match state.read(cx).value() {
        SliderValue::Single(v) => v,
        SliderValue::Range(_, end) => end,
    }
}

/// RGB 转 Hsla（h/s/l 均为 0–1，h 为色相环比例）。
fn rgb_to_hsla(r: f32, g: f32, b: f32) -> Hsla {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < f32::EPSILON {
        return hsla(0.0, 0.0, l, 1.0);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - r).abs() < f32::EPSILON {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    hsla(h, s, l, 1.0)
}

/// Hsla 转 RGB（各通道 0–1）。
fn hsla_to_rgb(color: Hsla) -> (f32, f32, f32) {
    // Hsla 字段为公开的 h/s/l/a（0–1）。
    let (h, s, l) = (color.h, color.s, color.l);
    if s.abs() < f32::EPSILON {
        return (l, l, l);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let channel = |t: f32| {
        let t = if t < 0.0 {
            t + 1.0
        } else if t > 1.0 {
            t - 1.0
        } else {
            t
        };
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    (channel(h + 1.0 / 3.0), channel(h), channel(h - 1.0 / 3.0))
}

impl Render for ColorPickerState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 延后的 hex 同步：set_value 需要 Window。
        if self.hex_dirty {
            self.hex_dirty = false;
            let (r, g, b) = hsla_to_rgb(self.color);
            let text: SharedString = format!(
                "#{:02x}{:02x}{:02x}",
                (r * 255.0).round() as u8,
                (g * 255.0).round() as u8,
                (b * 255.0).round() as u8
            )
            .into();
            self.hex_input.update(cx, |input, cx| {
                input.set_value(text, window, cx);
            });
        }

        // 延后的变更回调：render 有 Window 后真正触发。
        if self.pending_emit {
            self.pending_emit = false;
            if let Some(ref cb) = self.on_change.clone() {
                cb(self.color, window, cx);
            }
        }

        let theme = cx.theme();
        let border = theme.tokens.border;
        let popover = theme.tokens.popover;
        let muted_foreground = theme.tokens.muted_foreground.color;

        let hue = self.hue.clone();
        let saturation = self.saturation.clone();
        let lightness = self.lightness.clone();
        let hex_input = self.hex_input.clone();

        div()
            .flex()
            .flex_col()
            .w(px(260.0))
            .bg(popover)
            .border_1()
            .border_color(border)
            .rounded_md()
            .p(px(12.0))
            .gap(px(8.0))
            // 预览色块
            .child(div().w_full().h(px(48.0)).rounded_sm().bg(self.color))
            .child(slider_row("H", &hue, muted_foreground))
            .child(slider_row("S", &saturation, muted_foreground))
            .child(slider_row("L", &lightness, muted_foreground))
            .child(Input::new(&hex_input).w_full())
    }
}

/// 单行滑块（标签 + 滑块）。
fn slider_row(
    label: &'static str,
    state: &Entity<SliderState>,
    muted_foreground: Hsla,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .w(px(16.0))
                .text_sm()
                .text_color(muted_foreground)
                .child(label),
        )
        .child(div().flex_1().child(Slider::new(state).horizontal()))
}
