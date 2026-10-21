//! 颜色选择器。
//!
//! 色相/饱和度/明度三滑块 + hex 输入 + 预览色块，外加**主题色板**与**最近颜色**
//! 两组快捷色块（[`ColorPalette`]），变更经 `on_change(Hsla)` 回调。
//! 状态由实体持有，父组件经 `cx.new(|cx| ColorPickerState::new(window, cx))` 创建。
//!
//! 最近颜色的记录时机：点击色板色块、在 hex 输入框按下回车，或应用显式调用
//! [`ColorPickerState::push_recent`]（如对话框点「确定」时）。

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
    /// 最近使用的颜色（去重置顶，上限 `recent_limit`）。
    recents: Vec<Hsla>,
    /// 最近颜色上限。
    recent_limit: usize,
    /// 是否显示主题色板与最近颜色。
    show_palette: bool,
    /// 吸管待应用的颜色（取色回调运行在浮层上下文，没有来源窗口的 `&mut Window`，
    /// 延后到 render 时经 [`Self::set_color`] 应用，与 `hex_dirty` 同一套延后机制）。
    #[cfg(feature = "screen-capture")]
    pending_pick: Option<Hsla>,
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
            // 回车视为「提交当前颜色」，记入最近颜色。
            crate::input_ui::InputEvent::PressEnter { .. } => {
                let color = this.color;
                this.push_recent(color, cx);
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
            recents: Vec::new(),
            recent_limit: RECENT_COLORS_LIMIT,
            show_palette: true,
            #[cfg(feature = "screen-capture")]
            pending_pick: None,
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

    /// 设置是否显示主题色板与最近颜色（默认显示）。
    pub fn show_palette(mut self, show: bool) -> Self {
        self.show_palette = show;
        self
    }

    /// 设置最近颜色上限（最小 1，默认 [`RECENT_COLORS_LIMIT`]）。
    pub fn recent_limit(mut self, limit: usize) -> Self {
        self.recent_limit = limit.max(1);
        self
    }

    /// 当前颜色。
    pub fn color_value(&self) -> Hsla {
        self.color
    }

    /// 最近使用的颜色（最新的在前）。
    pub fn recents(&self) -> &[Hsla] {
        &self.recents
    }

    /// 把颜色记入最近颜色（去重置顶 + 截断），返回列表是否发生变化。
    pub fn push_recent(&mut self, color: Hsla, cx: &mut Context<Self>) -> bool {
        let changed = push_recent(&mut self.recents, color, self.recent_limit);
        if changed {
            cx.notify();
        }
        changed
    }

    /// 清空最近颜色。
    pub fn clear_recents(&mut self, cx: &mut Context<Self>) {
        if !self.recents.is_empty() {
            self.recents.clear();
            cx.notify();
        }
    }

    /// 启动屏幕取色（吸管，`screen-capture` feature 门控）：抓主屏首帧打开
    /// 全屏冻结浮层，选中颜色写回 `pending_pick`，由下一次 render 应用
    /// （见字段注释）。
    #[cfg(feature = "screen-capture")]
    pub fn begin_eyedropper(&mut self, cx: &mut Context<Self>) {
        let picker = cx.entity().downgrade();
        crate::eyedropper::open_eyedropper(cx, move |result, cx| {
            if let crate::eyedropper::EyedropperResult::Picked(color) = result {
                // 来源选择器已被释放（弹层已关 / 应用退出）时丢弃结果即可。
                let _ = picker.update(cx, |state, cx| {
                    state.pending_pick = Some(color);
                    cx.notify();
                });
            }
        });
    }

    /// 直接应用一个颜色：同步三滑块与 hex，并触发 `on_change`。
    ///
    /// 滑块步长为 1，只作近似指示；颜色以传入值为准（避免色阶量化改变取值）。
    pub fn set_color(&mut self, color: Hsla, window: &mut Window, cx: &mut Context<Self>) {
        self.hue.update(cx, |slider, cx| {
            slider.set_value(color.h * 360.0, window, cx)
        });
        self.saturation.update(cx, |slider, cx| {
            slider.set_value(color.s * 100.0, window, cx)
        });
        self.lightness.update(cx, |slider, cx| {
            slider.set_value(color.l * 100.0, window, cx)
        });
        self.color = color;
        self.hex_dirty = true;
        self.pending_emit = true;
        cx.notify();
    }

    /// 点击色板色块：应用颜色并记入最近颜色。
    fn apply_swatch(&mut self, color: Hsla, window: &mut Window, cx: &mut Context<Self>) {
        self.set_color(color, window, cx);
        self.push_recent(color, cx);
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
        // 延后的吸管颜色：取色回调没有来源窗口，延后到本次 render 应用（此时窗口可用）。
        // set_color 内部会置 pending_emit / hex_dirty，由下方两个延后块接着处理。
        #[cfg(feature = "screen-capture")]
        if let Some(color) = self.pending_pick.take() {
            self.set_color(color, window, cx);
            self.push_recent(color, cx);
        }

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

        // 主题色板 + 最近颜色：共用同一个回调（应用颜色 + 记入最近）。
        let palette_block = if self.show_palette {
            let handler: Arc<dyn Fn(Hsla, &mut Window, &mut App) + 'static> = {
                let picker = cx.entity();
                Arc::new(move |color, window, cx| {
                    picker.update(cx, |picker, cx| picker.apply_swatch(color, window, cx));
                })
            };
            let current = self.color;
            let mut block = div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(section_label("主题色", muted_foreground))
                .child(
                    ColorPalette::new("color-picker-theme")
                        .colors(theme_palette(cx))
                        .swatch_size(16.0)
                        .selected(Some(current))
                        .on_select_arc(handler.clone()),
                );
            if !self.recents.is_empty() {
                block = block.child(section_label("最近", muted_foreground)).child(
                    ColorPalette::new("color-picker-recent")
                        .colors(self.recents.clone())
                        .swatch_size(16.0)
                        .selected(Some(current))
                        .on_select_arc(handler),
                );
            }
            Some(block.into_any_element())
        } else {
            None
        };

        // 吸管：启动屏幕取色流程（`screen-capture` feature 门控；未启用时不渲染该行）。
        #[cfg(feature = "screen-capture")]
        let eyedropper_row = {
            let picker = cx.entity();
            Some(
                Button::new("color-picker-eyedropper")
                    .label("屏幕取色")
                    .small()
                    .on_click(move |_, _, cx| {
                        picker.update(cx, |picker, cx| picker.begin_eyedropper(cx));
                    })
                    .into_any_element(),
            )
        };
        #[cfg(not(feature = "screen-capture"))]
        let eyedropper_row: Option<AnyElement> = None;

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
            .when_some(eyedropper_row, |this, row| this.child(row))
            .when_some(palette_block, |this, block| this.child(block))
    }
}

/// 小节标签（小号灰色文本）。
fn section_label(label: &'static str, muted_foreground: Hsla) -> impl IntoElement {
    div().text_xs().text_color(muted_foreground).child(label)
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

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::ColorPickerState;
    use crate::{
        AppContext as _, Context, Entity, IntoElement, Render, TestAppContext, Window, div, hsla,
    };

    /// 取色器测试宿主（仅用于拿到带 Window 的测试上下文）。
    struct Probe;

    impl Render for Probe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    /// 最近颜色去重置顶，`set_color` 同步当前颜色，`recent_limit` 生效。
    #[rgpui::test]
    fn recents_and_set_color(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let (_probe, cx) = cx.add_window_view(|_window, _cx| Probe);
        let picker: Entity<ColorPickerState> =
            cx.update(|window, cx| cx.new(|cx| ColorPickerState::new(window, cx)));

        // 初始无最近颜色，默认显示色板。
        assert!(picker.read_with(cx, |state, _| state.recents().is_empty()));

        let a = hsla(0.0, 0.5, 0.5, 1.0);
        let b = hsla(0.6, 0.5, 0.5, 1.0);
        cx.update(|_, cx| {
            picker.update(cx, |state, cx| {
                assert!(state.push_recent(a, cx));
                assert!(state.push_recent(b, cx));
                // 已在首位：无变化。
                assert!(!state.push_recent(b, cx));
            });
        });
        assert_eq!(
            picker.read_with(cx, |state, _| state.recents().to_vec()),
            vec![b, a]
        );

        // 应用颜色：当前颜色同步，滑块与 hex 延后到 render 同步（此处只查颜色）。
        cx.update(|window, cx| {
            picker.update(cx, |state, cx| state.set_color(a, window, cx));
        });
        assert_eq!(picker.read_with(cx, |state, _| state.color_value()), a);

        // 清空最近颜色。
        cx.update(|_, cx| picker.update(cx, |state, cx| state.clear_recents(cx)));
        assert!(picker.read_with(cx, |state, _| state.recents().is_empty()));

        // 上限最小为 1（传 0 按 1 处理），并可通过 builder 关闭色板。
        let limited: Entity<ColorPickerState> = cx.update(|window, cx| {
            cx.new(|cx| {
                ColorPickerState::new(window, cx)
                    .recent_limit(0)
                    .show_palette(false)
            })
        });
        cx.update(|_, cx| {
            limited.update(cx, |state, cx| {
                state.push_recent(a, cx);
                state.push_recent(b, cx);
            });
        });
        assert_eq!(
            limited.read_with(cx, |state, _| state.recents().to_vec()),
            vec![b]
        );
    }
}
