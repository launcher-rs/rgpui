use std::{rc::Rc, time::Duration};

use crate::prelude::FluentBuilder as _;
use crate::{
    ActiveTheme, Animation, AnimationExt as _, App, Background, ComponentText, Disableable,
    ElementId, ElementSize, Hsla, InteractiveElement, IntoElement, ParentElement as _, RenderOnce,
    Role, SharedString, Side, Sizable, StatefulInteractiveElement as _, StyleRefinement, Styled,
    StyledExt as _, Toggled, Window, div, h_flex, px,
};

/// 可切换开/关的开关（Switch）元素。
#[derive(IntoElement)]
pub struct Switch {
    /// 元素 ID
    id: ElementId,
    /// 样式精炼
    style: StyleRefinement,
    /// 是否选中
    checked: bool,
    /// 是否禁用
    disabled: bool,
    /// 标签文本
    label: Option<ComponentText>,
    /// 标签所在侧
    label_side: Side,
    /// 点击回调
    on_click: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,
    /// 尺寸
    size: ElementSize,
    /// 选中时的背景色
    color: Option<Hsla>,
    /// 提示文本（当前简化存储，暂不渲染）
    tooltip: Option<SharedString>,
}

impl Switch {
    /// 创建新的 Switch 元素。
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id: ElementId = id.into();
        Self {
            id: id,
            style: StyleRefinement::default(),
            checked: false,
            disabled: false,
            label: None,
            on_click: None,
            label_side: Side::Right,
            size: ElementSize::Medium,
            color: None,
            tooltip: None,
        }
    }

    /// 设置 Switch 的选中状态。
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// 设置 Switch 的标签。
    pub fn label(mut self, label: impl Into<ComponentText>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 添加 Switch 的点击回调。
    pub fn on_click<F>(mut self, handler: F) -> Self
    where
        F: Fn(&bool, &mut Window, &mut App) + 'static,
    {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// 设置选中时 Switch 的背景色，默认为 `cx.theme().primary`。
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// 设置 Switch 的提示文本。
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }
}

impl Styled for Switch {
    fn style(&mut self) -> &mut crate::StyleRefinement {
        &mut self.style
    }
}

impl Sizable for Switch {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl Disableable for Switch {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let checked = self.checked;
        let on_click = self.on_click.clone();
        let toggle_state = window.use_keyed_state(self.id.clone(), cx, |_, _| checked);

        let checked_bg = self
            .color
            .map(Background::from)
            .unwrap_or(cx.theme().tokens.primary.into());
        let (bg, toggle_bg): (Background, Background) = match checked {
            true => (checked_bg, cx.theme().tokens.switch_thumb.into()),
            false => (
                cx.theme().tokens.switch.into(),
                cx.theme().tokens.switch_thumb.into(),
            ),
        };

        let (bg, toggle_bg) = if self.disabled {
            (
                if checked { bg.opacity(0.5) } else { bg },
                toggle_bg.opacity(0.35),
            )
        } else {
            (bg, toggle_bg)
        };

        let (bg_width, bg_height) = match self.size {
            ElementSize::XSmall | ElementSize::Small => (px(28.), px(16.)),
            _ => (px(36.), px(20.)),
        };
        let bar_width = match self.size {
            ElementSize::XSmall | ElementSize::Small => px(12.),
            _ => px(16.),
        };
        let inset = px(2.);
        let radius = if cx.theme().radius >= px(4.) {
            bg_height
        } else {
            cx.theme().radius
        };

        div().refine_style(&self.style).child(
            h_flex()
                .id(self.id.clone())
                .role(Role::Switch)
                .aria_toggled(if checked {
                    Toggled::True
                } else {
                    Toggled::False
                })
                .when_some(
                    self.label.as_ref().map(|l| l.get_text(cx)),
                    |this, label| this.aria_label(label),
                )
                .gap_2()
                .items_start()
                .when(self.label_side.is_left(), |this| this.flex_row_reverse())
                .child(
                    // Switch 轨道
                    div()
                        .id(self.id.clone())
                        .w(bg_width)
                        .h(bg_height)
                        .rounded(radius)
                        .flex()
                        .items_center()
                        .border(inset)
                        .border_color(cx.theme().transparent)
                        .bg(bg)
                        .child(
                            // Switch 滑块
                            div()
                                .rounded(radius)
                                .bg(toggle_bg)
                                .size(bar_width)
                                .map(|this| {
                                    let prev_checked = toggle_state.read(cx);
                                    if !self.disabled && *prev_checked != checked {
                                        let duration = Duration::from_secs_f64(0.15);
                                        cx.spawn({
                                            let toggle_state = toggle_state.clone();
                                            async move |cx| {
                                                cx.background_executor().timer(duration).await;
                                                _ = toggle_state.update(cx, |this, _| {
                                                    *this = checked;
                                                });
                                            }
                                        })
                                        .detach();

                                        this.with_animation(
                                            ElementId::NamedInteger("move".into(), checked as u64),
                                            Animation::new(duration),
                                            move |this, delta| {
                                                let max_x = bg_width - bar_width - inset * 2;
                                                let x = if checked {
                                                    max_x * delta
                                                } else {
                                                    max_x - max_x * delta
                                                };
                                                this.left(x)
                                            },
                                        )
                                        .into_any_element()
                                    } else {
                                        let max_x = bg_width - bar_width - inset * 2;
                                        let x = if checked { max_x } else { px(0.) };
                                        this.left(x).into_any_element()
                                    }
                                }),
                        ),
                )
                .when_some(self.label, |this, label| {
                    this.child(div().line_height(bg_height).child(label).map(
                        |this| match self.size {
                            ElementSize::XSmall | ElementSize::Small => this.text_sm(),
                            _ => this.text_base(),
                        },
                    ))
                })
                .when_some(
                    on_click
                        .as_ref()
                        .map(|c| c.clone())
                        .filter(|_| !self.disabled),
                    |this, on_click| {
                        let toggle_state = toggle_state.clone();
                        this.on_mouse_down(crate::MouseButton::Left, move |_, window, cx| {
                            cx.stop_propagation();
                            _ = toggle_state.update(cx, |this, _| *this = checked);
                            on_click(&!checked, window, cx);
                        })
                    },
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 Switch 基本构造
    #[test]
    fn test_switch_build() {
        let s = Switch::new("test-switch")
            .label("开启")
            .checked(true)
            .with_size(ElementSize::Small)
            .color(crate::red_500());
        assert!(s.checked);
        assert!(s.label.is_some());
        assert_eq!(s.size, ElementSize::Small);
        assert!(s.color.is_some());
    }

    /// 测试 Switch 标签侧
    #[test]
    fn test_switch_label_side() {
        let s = Switch::new("test-switch-2").label("标签");
        assert!(s.label_side.is_right());
    }
}
