//! 基础组件示例：按钮、复选、单选、开关、滑块、加载、骨架屏、徽标、标签、分隔线、快捷键提示。

use rgpui::prelude::*;
use rgpui::{
    Badge, Button, ButtonVariants, Checkbox, Disableable, Icon, IconName, IntoElement, Kbd,
    ParentElement, Radio, Separator, Sizable, Skeleton, Slider, SliderState, Spinner, Styled,
    Switch, Tag, TagVariant, Window, div, px, v_flex,
};

use super::StoryItem;

/// 基础组件故事注册表。
pub fn stories() -> Vec<StoryItem> {
    vec![
        StoryItem {
            title: "按钮",
            build: |window, cx| cx.new(|cx| ButtonStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "选择类控件",
            build: |window, cx| cx.new(|cx| SelectionStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "滑块",
            build: |window, cx| cx.new(|cx| SliderStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "加载与占位",
            build: |_, cx| cx.new(|cx| LoadingStory::new(cx)).into(),
        },
        StoryItem {
            title: "徽标与标签",
            build: |_, cx| cx.new(|cx| BadgeTagStory::new(cx)).into(),
        },
        StoryItem {
            title: "分隔线与快捷键",
            build: |_, cx| cx.new(|cx| KbdSeparatorStory::new(cx)).into(),
        },
    ]
}

/// 按钮示例视图。
struct ButtonStory;

impl ButtonStory {
    fn new(_window: &mut Window, _cx: &mut rgpui::Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for ButtonStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut rgpui::Context<Self>) -> impl IntoElement {
        v_flex()
            .id("button-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("变体"))
            .child(
                v_flex().id("button-variants").gap(px(8.0)).child(
                    v_flex()
                        .gap(px(8.0))
                        .child(Button::new("btn-default").label("默认按钮"))
                        .child(Button::new("btn-primary").label("主要按钮").primary())
                        .child(Button::new("btn-secondary").label("次要按钮").secondary())
                        .child(Button::new("btn-danger").label("危险按钮").danger())
                        .child(Button::new("btn-success").label("成功按钮").success())
                        .child(Button::new("btn-warning").label("警告按钮").warning())
                        .child(Button::new("btn-info").label("信息按钮").info())
                        .child(Button::new("btn-ghost").label("幽灵按钮").ghost())
                        .child(Button::new("btn-link").label("链接按钮").link())
                        .child(Button::new("btn-text").label("文本按钮").text()),
                ),
            )
            .child(div().h(px(16.0)))
            .child(section_title("图标与状态"))
            .child(
                v_flex()
                    .id("button-icon-state")
                    .gap(px(8.0))
                    .child(Button::new("btn-icon").icon(IconName::Search).label("搜索"))
                    .child(Button::new("btn-loading").label("加载中").loading(true))
                    .child(Button::new("btn-disabled").label("禁用").disabled(true))
                    .child(
                        Button::new("btn-outline")
                            .label("描边按钮")
                            .outline()
                            .rounded(rgpui::ButtonRounded::Medium),
                    ),
            )
            .child(div().h(px(16.0)))
            .child(section_title("Tooltip 提示"))
            .child(
                div().child(
                    Button::new("btn-tooltip")
                        .label("悬停我")
                        .tooltip("这是一个工具提示"),
                ),
            )
    }
}

/// 选择类控件（复选/单选/开关）示例视图。
struct SelectionStory;

impl SelectionStory {
    fn new(_window: &mut Window, _cx: &mut rgpui::Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for SelectionStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut rgpui::Context<Self>) -> impl IntoElement {
        v_flex()
            .id("selection-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("复选（Checkbox）"))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(Checkbox::new("cb-1").label("选中").checked(true))
                    .child(Checkbox::new("cb-2").label("未选中"))
                    .child(Checkbox::new("cb-3").label("禁用").disabled(true)),
            )
            .child(div().h(px(16.0)))
            .child(section_title("单选（Radio）"))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(Radio::new("radio-1").label("选项一").checked(true))
                    .child(Radio::new("radio-2").label("选项二"))
                    .child(Radio::new("radio-3").label("禁用").disabled(true)),
            )
            .child(div().h(px(16.0)))
            .child(section_title("开关（Switch）"))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(Switch::new("sw-1").label("已开启").checked(true))
                    .child(Switch::new("sw-2").label("已关闭"))
                    .child(Switch::new("sw-3").label("禁用").disabled(true)),
            )
    }
}

/// 滑块示例视图（需持有 SliderState 实体）。
struct SliderStory {
    state: rgpui::Entity<SliderState>,
}

impl SliderStory {
    fn new(_window: &mut Window, cx: &mut rgpui::Context<Self>) -> Self {
        let state = cx.new(|_cx| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(1.0)
                .default_value(40.0)
        });
        Self { state }
    }
}

impl rgpui::Render for SliderStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut rgpui::Context<Self>) -> impl IntoElement {
        v_flex()
            .id("slider-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("水平滑块"))
            .child(
                v_flex()
                    .w(px(320.0))
                    .child(Slider::new(&self.state).horizontal()),
            )
            .child(div().h(px(16.0)))
            .child(section_title("垂直滑块"))
            .child(
                div()
                    .h(px(160.0))
                    .child(Slider::new(&self.state).vertical()),
            )
    }
}

/// 加载与占位示例视图。
struct LoadingStory;

impl LoadingStory {
    fn new(_cx: &mut rgpui::Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for LoadingStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut rgpui::Context<Self>) -> impl IntoElement {
        v_flex()
            .id("loading-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("加载中（Spinner）"))
            .child(
                v_flex().gap(px(8.0)).child(Spinner::new()).child(
                    v_flex()
                        .gap(px(8.0))
                        .child(Spinner::new().small())
                        .child(Spinner::new().large()),
                ),
            )
            .child(div().h(px(16.0)))
            .child(section_title("骨架屏（Skeleton）"))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(Skeleton::new())
                    .child(Skeleton::new().secondary())
                    .child(v_flex().w(px(300.0)).child(Skeleton::new().h(px(16.0)))),
            )
            .child(div().h(px(16.0)))
            .child(section_title("图标（Icon）"))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(Icon::new(IconName::Check).size(px(24.0)))
                    .child(Icon::new(IconName::Search).size(px(24.0)))
                    .child(Icon::new(IconName::Settings).size(px(24.0))),
            )
    }
}

/// 徽标与标签示例视图。
struct BadgeTagStory;

impl BadgeTagStory {
    fn new(_cx: &mut rgpui::Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for BadgeTagStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut rgpui::Context<Self>) -> impl IntoElement {
        v_flex()
            .id("badge-tag-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("徽标（Badge）"))
            .child(
                v_flex().gap(px(8.0)).child(
                    v_flex()
                        .gap(px(8.0))
                        .child(Badge::new().count(3))
                        .child(Badge::new().count(100))
                        .child(Badge::new().count(5).dot()),
                ),
            )
            .child(div().h(px(16.0)))
            .child(section_title("标签（Tag）"))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(Tag::new().child("默认标签"))
                    .child(
                        Tag::new()
                            .with_variant(TagVariant::Primary)
                            .child("主要标签"),
                    )
                    .child(
                        Tag::new()
                            .with_variant(TagVariant::Danger)
                            .child("危险标签"),
                    )
                    .child(
                        Tag::new()
                            .with_variant(TagVariant::Success)
                            .child("成功标签"),
                    ),
            )
    }
}

/// 分隔线与快捷键示例视图。
struct KbdSeparatorStory;

impl KbdSeparatorStory {
    fn new(_cx: &mut rgpui::Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for KbdSeparatorStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut rgpui::Context<Self>) -> impl IntoElement {
        v_flex()
            .id("kbd-separator-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("分隔线（Separator）"))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(Separator::horizontal().label("水平分隔"))
                    .child(Separator::horizontal_dashed().label("虚线分隔"))
                    .child(
                        div()
                            .h(px(40.0))
                            .child(Separator::vertical().label("垂直分隔")),
                    ),
            )
            .child(div().h(px(16.0)))
            .child(section_title("快捷键提示（Kbd）"))
            .child(
                v_flex().gap(px(8.0)).child(
                    v_flex()
                        .gap(px(8.0))
                        .child(Kbd::new(rgpui::Keystroke {
                            key: "s".into(),
                            modifiers: rgpui::Modifiers::default(),
                            key_char: None,
                        }))
                        .child(Kbd::new(rgpui::Keystroke {
                            key: "Enter".into(),
                            modifiers: rgpui::Modifiers::default(),
                            key_char: None,
                        }))
                        .child(Kbd::new(rgpui::Keystroke {
                            key: "s".into(),
                            modifiers: rgpui::Modifiers {
                                control: true,
                                ..Default::default()
                            },
                            key_char: None,
                        })),
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
