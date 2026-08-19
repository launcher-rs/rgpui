//! 扩展组件示例：并入核心的 rgpui-ui 组件（分隔面板、标签输入、OTP、热键输入、行内编辑、
//! 打字机、动画计数器、波形、迷你走势图、SVG 渲染）。

use rgpui::components::{
    AnimatedCounter, AnimatedCounterState, HotkeyInput, HotkeyInputState, InlineEdit,
    InlineEditState, OTPInput, OTPInputSize, OTPState, SVGRenderer, Sparkline, SplitPane,
    SplitPaneState, TagInput, TagInputState, TypeWriter, TypeWriterState, Waveform,
};
use rgpui::prelude::*;
use rgpui::{Context, Entity, IntoElement, ParentElement, Styled, Window, div, hsla, px, v_flex};

use super::StoryItem;

/// 扩展组件故事注册表。
pub fn stories() -> Vec<StoryItem> {
    vec![
        StoryItem {
            title: "分隔面板",
            build: |window, cx| cx.new(|cx| SplitPaneStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "标签输入",
            build: |window, cx| cx.new(|cx| TagInputStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "一次性密码",
            build: |window, cx| cx.new(|cx| OtpStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "热键输入",
            build: |window, cx| cx.new(|cx| HotkeyStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "行内编辑",
            build: |window, cx| cx.new(|cx| InlineEditStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "打字机与计数器",
            build: |window, cx| cx.new(|cx| TypeWriterCounterStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "数据可视化",
            build: |_, cx| cx.new(|cx| DataVizStory::new(cx)).into(),
        },
    ]
}

/// 分隔面板示例视图。
struct SplitPaneStory {
    state: Entity<SplitPaneState>,
}

impl SplitPaneStory {
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| SplitPaneState::new(cx));
        Self { state }
    }
}

impl rgpui::Render for SplitPaneStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("split-pane-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("分隔面板（SplitPane）"))
            .child(
                SplitPane::horizontal(self.state.clone())
                    .show_collapse_buttons(true)
                    .first(div().p(px(16.0)).child("左侧面板"))
                    .second(div().p(px(16.0)).child("右侧面板"))
                    .h(px(200.0)),
            )
    }
}

/// 标签输入示例视图。
struct TagInputStory {
    state: Entity<TagInputState>,
}

impl TagInputStory {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state =
            cx.new(|cx| TagInputState::with_tags(window, cx, vec!["rgpui", "组件", "示例"]));
        Self { state }
    }
}

impl rgpui::Render for TagInputStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("tag-input-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("标签输入（TagInput）"))
            .child(v_flex().w(px(360.0)).child(
                TagInput::new(self.state.clone()).suggestions(vec!["动画", "手势", "图表", "特效"]),
            ))
    }
}

/// 一次性密码示例视图。
struct OtpStory {
    state: Entity<OTPState>,
}

impl OtpStory {
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| OTPState::new(cx, 6));
        Self { state }
    }
}

impl rgpui::Render for OtpStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("otp-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("一次性密码（OTPInput）"))
            .child(v_flex().child(OTPInput::new(&self.state).size(OTPInputSize::Md)))
    }
}

/// 热键输入示例视图。
struct HotkeyStory {
    state: Entity<HotkeyInputState>,
}

impl HotkeyStory {
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| HotkeyInputState::new(cx));
        Self { state }
    }
}

impl rgpui::Render for HotkeyStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("hotkey-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("热键输入（HotkeyInput）"))
            .child(
                v_flex()
                    .w(px(280.0))
                    .child(HotkeyInput::new(self.state.clone())),
            )
    }
}

/// 行内编辑示例视图。
struct InlineEditStory {
    state: Entity<InlineEditState>,
}

impl InlineEditStory {
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| InlineEditState::with_value(cx, "点击编辑这段文字"));
        Self { state }
    }
}

impl rgpui::Render for InlineEditStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("inline-edit-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("行内编辑（InlineEdit）"))
            .child(
                v_flex()
                    .w(px(360.0))
                    .child(InlineEdit::new(self.state.clone())),
            )
    }
}

/// 打字机与动画计数器示例视图。
struct TypeWriterCounterStory {
    writer_state: Entity<TypeWriterState>,
    counter_state: Entity<AnimatedCounterState>,
    /// 是否已在首次渲染时启动动画（避免在 `new` 中提前启动，导致切换到该页时动画早已结束）。
    started: bool,
}

impl TypeWriterCounterStory {
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let writer_state = cx.new(|_cx| TypeWriterState::new("rgpui 打字机动画与计数器演示"));
        let counter_state = cx.new(|_cx| AnimatedCounterState::new(0.0));
        Self {
            writer_state,
            counter_state,
            started: false,
        }
    }
}

impl rgpui::Render for TypeWriterCounterStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 视图首次渲染（即切换到本页）时启动打字机动画并触发计数滚动。
        if !self.started {
            self.started = true;
            self.writer_state.update(cx, |state, cx| state.start(cx));
            self.counter_state.update(cx, |state, cx| {
                state.set_value(12345.0, cx);
            });
        }

        v_flex()
            .id("type-writer-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("打字机（TypeWriter）"))
            .child(v_flex().child(TypeWriter::new("demo-writer", self.writer_state.clone())))
            .child(div().h(px(16.0)))
            .child(section_title("动画计数器（AnimatedCounter）"))
            .child(v_flex().child(AnimatedCounter::new(
                "demo-counter",
                self.counter_state.clone(),
            )))
    }
}

/// 数据可视化示例视图（波形、迷你走势图、SVG）。
struct DataVizStory;

impl DataVizStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for DataVizStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // 生成波形示例数据（正弦波）。
        let mut wave_data = Vec::new();
        for i in 0..64 {
            let v = ((i as f32 / 8.0) * std::f32::consts::PI).sin();
            wave_data.push(v * 0.5 + 0.5);
        }

        let line_data = vec![3.0, 7.0, 5.0, 9.0, 6.0, 12.0, 8.0, 15.0, 11.0, 18.0];
        let bar_data = vec![4.0, 6.0, 8.0, 5.0, 9.0, 7.0, 10.0];

        v_flex()
            .id("data-viz-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("音频波形（Waveform）"))
            .child(
                v_flex().w(px(360.0)).child(
                    Waveform::new()
                        .data(&wave_data)
                        .playback_position(0.35)
                        .color(hsla(0.62, 0.1, 0.5, 1.0))
                        .active_color(hsla(0.6, 0.8, 0.55, 1.0)),
                ),
            )
            .child(div().h(px(16.0)))
            .child(section_title("迷你走势图（Sparkline）"))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(
                        Sparkline::line(line_data.clone())
                            .width(px(240.0))
                            .height(px(40.0)),
                    )
                    .child(Sparkline::bar(bar_data).width(px(240.0)).height(px(40.0)))
                    .child(Sparkline::area(line_data).width(px(240.0)).height(px(40.0))),
            )
            .child(div().h(px(16.0)))
            .child(section_title("SVG 渲染（SVGRenderer）"))
            .child(
                v_flex().child(
                    SVGRenderer::new()
                        .path_data(
                            "M100,40 C100,20 70,10 55,30 C40,10 10,20 10,40 \
                                 C10,70 55,100 55,100 C55,100 100,70 100,40 Z",
                        )
                        .view_box(0.0, 0.0, 110.0, 110.0)
                        .fill(hsla(0.0, 0.75, 0.55, 1.0))
                        .w(px(80.0))
                        .h(px(80.0)),
                ),
            )
    }
}

/// 章节标题辅助函数。
fn section_title(text: impl Into<rgpui::SharedString> + IntoElement) -> impl IntoElement {
    div()
        .text_size(px(14.0))
        .text_color(rgpui::hsla(0.0, 0.0, 0.55, 1.0))
        .child(text)
}
