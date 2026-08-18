//! 文本逐段揭示：按词/行/字符交错淡入滑入。

use std::time::Duration;

use rgpui::*;

/// 文本揭示的分段模式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RevealMode {
    /// 按单词分段。
    ByWord,
    /// 按行分段。
    ByLine,
    /// 按字符分段。
    ByCharacter,
}

/// 文本逐段揭示组件：分段依次淡入并向上偏移，产生交错出现效果。
#[derive(IntoElement)]
pub struct TextReveal {
    id: ElementId,
    text: SharedString,
    mode: RevealMode,
    stagger: Duration,
    duration: Duration,
    easing: fn(f32) -> f32,
    text_size: Option<Pixels>,
    font_weight: Option<FontWeight>,
    text_color: Option<Hsla>,
    style: StyleRefinement,
}

impl TextReveal {
    /// 创建文本揭示组件，默认按词分段、每段 400ms、交错 50ms。
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            mode: RevealMode::ByWord,
            stagger: Duration::from_millis(50),
            duration: Duration::from_millis(400),
            easing: ease_out_cubic,
            text_size: None,
            font_weight: None,
            text_color: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置分段模式。
    pub fn mode(mut self, mode: RevealMode) -> Self {
        self.mode = mode;
        self
    }

    /// 设置分段交错延迟。
    pub fn stagger(mut self, stagger: Duration) -> Self {
        self.stagger = stagger;
        self
    }

    /// 设置每段动画时长。
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// 设置缓动函数。
    pub fn easing(mut self, easing: fn(f32) -> f32) -> Self {
        self.easing = easing;
        self
    }

    /// 设置字号。
    pub fn text_size(mut self, size: Pixels) -> Self {
        self.text_size = Some(size);
        self
    }

    /// 设置字重。
    pub fn font_weight(mut self, weight: FontWeight) -> Self {
        self.font_weight = Some(weight);
        self
    }

    /// 设置文本颜色。
    pub fn text_color(mut self, color: Hsla) -> Self {
        self.text_color = Some(color);
        self
    }
}

impl Styled for TextReveal {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TextReveal {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let segments: Vec<String> = match self.mode {
            RevealMode::ByWord => self
                .text
                .split_whitespace()
                .map(|s| s.to_string())
                .collect(),
            RevealMode::ByLine => self.text.lines().map(|s| s.to_string()).collect(),
            RevealMode::ByCharacter => self.text.chars().map(|c| c.to_string()).collect(),
        };

        let is_line_mode = self.mode == RevealMode::ByLine;
        let mode = self.mode;
        let stagger = self.stagger;
        let duration = self.duration;
        let easing = self.easing;
        let user_style = self.style;

        let mut container = if is_line_mode {
            div().flex().flex_col()
        } else {
            div().flex().flex_row().flex_wrap()
        };

        container.style().refine(&user_style);

        if let Some(size) = self.text_size {
            container = container.text_size(size);
        }
        if let Some(weight) = self.font_weight {
            container = container.font_weight(weight);
        }
        if let Some(color) = self.text_color {
            container = container.text_color(color);
        }

        for (i, segment) in segments.into_iter().enumerate() {
            let seg_id: ElementId = ElementId::Name(format!("{}-seg-{}", self.id, i).into());
            let delay = stagger.as_millis() as u64 * i as u64;
            let total_duration = Duration::from_millis(duration.as_millis() as u64 + delay);

            let display_text: SharedString = if mode == RevealMode::ByWord && i > 0 {
                format!(" {}", segment).into()
            } else {
                segment.into()
            };

            let seg_el = div().id(seg_id.clone()).child(display_text).with_animation(
                seg_id,
                Animation::new(total_duration).with_easing(easing),
                move |el, delta| {
                    let seg_t = if total_duration.as_millis() > 0 {
                        let delay_fraction = delay as f32 / total_duration.as_millis() as f32;
                        ((delta - delay_fraction) / (1.0 - delay_fraction)).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };

                    let offset = (1.0 - seg_t) * 8.0;
                    el.opacity(seg_t).mt(px(offset))
                },
            );

            container = container.child(seg_el);
        }

        container
    }
}
