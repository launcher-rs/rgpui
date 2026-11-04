//! 开关组件，支持开/关状态切换的滑动开关控件。

use std::{sync::Arc, time::Duration};

use crate::prelude::FluentBuilder as _;
use crate::{
    ActiveTheme, Animation, AnimationExt as _, App, Background, ComponentText, Disableable,
    ElementId, ElementSize, FocusableExt as _, Hsla, InteractiveElement, IntoElement,
    ManagedTooltipExt as _, ParentElement as _, RenderOnce, Role, SharedString, Side, Sizable,
    StatefulInteractiveElement as _, StyleRefinement, Styled, StyledExt as _, Toggled, Tooltip,
    Window, div, h_flex, px,
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
    /// 值变更回调（参数为变更后的选中状态，按值传递）
    on_change: Option<Arc<dyn Fn(bool, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 尺寸
    size: ElementSize,
    /// 选中时的背景色
    color: Option<Hsla>,
    /// 提示文本（悬停时以 `Tooltip` 浮层显示）
    tooltip: Option<SharedString>,
    /// 是否参与 Tab 焦点链
    tab_stop: bool,
    /// 焦点顺序（负值表示不进 Tab 链）
    tab_index: isize,
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
            on_change: None,
            label_side: Side::Right,
            size: ElementSize::Medium,
            color: None,
            tooltip: None,
            tab_stop: true,
            tab_index: 0,
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

    /// 添加 Switch 的值变更回调（参数为变更后的选中状态，按值传递）。
    pub fn on_change<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_change = Some(Arc::new(handler));
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

    /// 设置是否参与 Tab 焦点链，默认：true。
    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self
    }

    /// 设置焦点顺序，默认：0。
    pub fn tab_index(mut self, tab_index: isize) -> Self {
        self.tab_index = tab_index;
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
        let on_change = self.on_change.clone();
        let toggle_state = window.use_keyed_state(self.id.clone(), cx, |_, _| checked);
        // 焦点句柄与动画状态同键不同型，各自独立存活。
        let focus_handle = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let is_focused = focus_handle.is_focused(window);

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
                .aria_disabled(self.disabled)
                .aria_toggled(if checked {
                    Toggled::True
                } else {
                    Toggled::False
                })
                .when_some(
                    self.label.as_ref().map(|l| l.get_text(cx)),
                    |this, label| this.aria_label(label),
                )
                // 禁用态不进焦点链：Tab 跳过，辅助技术也不提供激活动作。
                .when(!self.disabled, |this| {
                    this.track_focus(
                        &focus_handle
                            .tab_stop(self.tab_stop)
                            .tab_index(self.tab_index),
                    )
                })
                .when_some(self.tooltip.clone(), |this, tooltip| {
                    // 悬停提示：与 `Checkbox` / `Button` 同一套受管 tooltip
                    this.managed_tooltip(move |window, cx| {
                        Tooltip::new(tooltip.clone()).build(window, cx)
                    })
                })
                .focus_ring(is_focused, px(2.), window, cx)
                .gap_2()
                .items_start()
                .when(self.label_side.is_left(), |this| this.flex_row_reverse())
                .child(
                    // Switch 轨道（纯视觉：id 属于外层可聚焦节点，重复 id 会让两节点抢同一份元素状态）
                    div()
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
                    on_change
                        .as_ref()
                        .map(|c| c.clone())
                        .filter(|_| !self.disabled),
                    |this, on_change| {
                        let toggle_state = toggle_state.clone();
                        // 挂在 on_click 而非 on_mouse_down：框架会把聚焦元素上的
                        // Enter/Space 合成为点击，键盘用户因此才能切换开关。
                        this.on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            _ = toggle_state.update(cx, |this, _| *this = checked);
                            on_change(!checked, window, cx);
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

    /// 悬停 `.tooltip()` 必须真的弹出浮层。
    ///
    /// 回归测试：`tooltip` 字段此前只存不渲染，调用方设了提示什么也不发生。
    #[rgpui::test]
    fn switch_tooltip_shows_on_hover(cx: &mut crate::TestAppContext) {
        use crate::{AnyWindowHandle, AppContext as _, Render, Root, point};

        struct Probe;
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut crate::Context<Self>) -> impl IntoElement {
                div().size_full().child(
                    Switch::new("switch-tooltip")
                        .label("开启")
                        .tooltip("提示文本"),
                )
            }
        }

        cx.update(crate::theme::init);
        let window = cx.add_window(|_, _| Probe);
        let any_window = AnyWindowHandle::from(window);
        cx.update_window(any_window, |_, window, cx| {
            window.draw(cx).clear(cx);
            // 悬停判定发生在绘制阶段：先把鼠标放进开关轨道内，再绘制一次
            window.simulate_mouse_move(point(px(4.), px(4.)), cx);
        })
        .unwrap();
        cx.update_window(any_window, |_, window, cx| {
            window.draw(cx).clear(cx);
        })
        .unwrap();
        cx.executor().advance_clock(Duration::from_millis(600));
        cx.run_until_parked();

        let showing = cx
            .update_window(any_window, |_, window, cx| {
                Root::tooltip_overlay(window, cx)
                    .is_some_and(|overlay| overlay.read(cx).is_showing())
            })
            .unwrap();
        assert!(showing, "Switch 的 .tooltip() 应在悬停后显示浮层");
    }

    /// 键盘必须能切换开关：Tab 聚焦后按空格/回车等同于点击。
    ///
    /// 回归测试：`on_change` 原先挂在 `on_mouse_down` 上，且开关不进焦点链，
    /// 键盘用户既聚焦不了也切换不了。
    #[rgpui::test]
    fn switch_toggles_with_keyboard(cx: &mut crate::TestAppContext) {
        use crate::{
            AnyWindowHandle, AppContext as _, InputEvent as _, KeyDownEvent, KeyUpEvent, Keystroke,
            Render,
        };
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Arc, Mutex};

        struct Probe {
            flips: Arc<Mutex<Vec<bool>>>,
            checked: Arc<AtomicBool>,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut crate::Context<Self>) -> impl IntoElement {
                let flips = self.flips.clone();
                let checked = self.checked.clone();
                div().size_full().child(
                    Switch::new("switch-key")
                        .label("开启")
                        .checked(checked.load(Ordering::SeqCst))
                        .on_change(move |next, _, _| {
                            checked.store(next, Ordering::SeqCst);
                            flips.lock().unwrap().push(next);
                        }),
                )
            }
        }

        cx.update(crate::theme::init);
        let flips = Arc::new(Mutex::new(Vec::new()));
        let probe_flips = flips.clone();
        let window = cx.add_window(move |_, _| Probe {
            flips: probe_flips,
            checked: Arc::new(AtomicBool::new(false)),
        });
        let any_window = AnyWindowHandle::from(window);
        cx.update_window(any_window, |_, window, cx| {
            window.draw(cx).clear(cx);
            window.focus_next(cx);
            assert!(window.focused(cx).is_some(), "Tab 应落到开关上");
        })
        .unwrap();
        cx.update_window(any_window, |_, window, cx| {
            window.draw(cx).clear(cx);
        })
        .unwrap();

        let press = |cx: &mut crate::TestAppContext, key: &str| {
            let keystroke = Keystroke::parse(key).unwrap();
            cx.update_window(any_window, |_, window, cx| {
                window.dispatch_event(
                    KeyDownEvent {
                        keystroke: keystroke.clone(),
                        is_held: false,
                        prefer_character_input: false,
                    }
                    .to_platform_input(),
                    cx,
                );
                window.dispatch_event(KeyUpEvent { keystroke }.to_platform_input(), cx);
                window.draw(cx).clear(cx);
            })
            .unwrap();
        };

        press(cx, "space");
        press(cx, "enter");
        assert_eq!(
            *flips.lock().unwrap(),
            vec![true, false],
            "空格与回车各切换一次"
        );
    }
}
