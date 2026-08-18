//! 可折叠面板：内容高度与透明度平滑展开/收起。

use std::rc::Rc;
use std::time::Duration;

use crate::{prelude::FluentBuilder as _, *};

/// 可折叠面板组件：触发区 + 平滑展开/收起的内容区。
#[derive(IntoElement)]
pub struct AnimatedCollapsible {
    trigger: Option<AnyElement>,
    content: Option<AnyElement>,
    is_open: bool,
    disabled: bool,
    show_icon: bool,
    duration: Duration,
    on_toggle: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    style: StyleRefinement,
}

impl AnimatedCollapsible {
    /// 创建可折叠面板，默认收起、250ms 动画、显示展开图标。
    pub fn new() -> Self {
        Self {
            trigger: None,
            content: None,
            is_open: false,
            disabled: false,
            show_icon: true,
            duration: Duration::from_millis(250),
            on_toggle: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置触发区内容。
    pub fn trigger(mut self, trigger: impl IntoElement) -> Self {
        self.trigger = Some(trigger.into_any_element());
        self
    }

    /// 设置内容区。
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    /// 设置初始展开状态。
    pub fn open(mut self, open: bool) -> Self {
        self.is_open = open;
        self
    }

    /// 禁用交互。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 是否显示展开/收起图标。
    pub fn show_icon(mut self, show: bool) -> Self {
        self.show_icon = show;
        self
    }

    /// 设置展开/收起动画时长。
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// 设置切换回调（参数为展开目标状态）。
    pub fn on_toggle<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle = Some(Rc::new(handler));
        self
    }
}

impl Default for AnimatedCollapsible {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for AnimatedCollapsible {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for AnimatedCollapsible {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;
        let is_open = self.is_open;
        let disabled = self.disabled;
        let show_icon = self.show_icon;
        let on_toggle = self.on_toggle;
        let duration = self.duration;

        let anim_id: SharedString = if is_open {
            "animated-collapsible-open".into()
        } else {
            "animated-collapsible-close".into()
        };

        let mut root = div().flex().flex_col().w_full();

        if let Some(trigger) = self.trigger {
            let trigger_row = div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .cursor(if disabled {
                    CursorStyle::Arrow
                } else {
                    CursorStyle::PointingHand
                })
                .when(!disabled, |this: Div| {
                    this.hover(|style| style.bg(theme.tokens.muted.opacity(0.5)))
                })
                .when(disabled, |this: Div| this.opacity(0.5))
                .when(!disabled && on_toggle.is_some(), |this: Div| {
                    let handler = on_toggle.clone().unwrap();
                    this.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        handler(!is_open, window, cx);
                    })
                })
                .when(show_icon, |this: Div| {
                    this.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(16.0))
                            .text_color(theme.tokens.muted_foreground)
                            .when(is_open, |this: Div| this.child("▼"))
                            .when(!is_open, |this: Div| this.child("▶")),
                    )
                })
                .child(div().flex_1().child(trigger));

            root = root.child(trigger_row);
        }

        if let Some(content) = self.content {
            let content_wrapper = div().overflow_hidden().child(content).with_animation(
                ElementId::Name(anim_id),
                Animation::new(duration).with_easing(ease_out_cubic),
                move |el, delta| {
                    if is_open {
                        el.max_h(px(1500.0 * delta)).opacity(delta)
                    } else {
                        el.max_h(px(1500.0 * (1.0 - delta))).opacity(1.0 - delta)
                    }
                },
            );

            root = root.child(content_wrapper);
        }

        root.style().refine(&user_style);
        root
    }
}
