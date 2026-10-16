//! 复选框组件，支持选中、未选中和半选状态的开关控件。

use std::{sync::Arc, time::Duration};

use crate::prelude::FluentBuilder as _;
use crate::{
    ActiveTheme, Animation, AnimationExt, AnyElement, App, ComponentText, Disableable, ElementId,
    ElementSize, FocusableExt as _, IconNamed, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, Role, Selectable, SharedString, Sizable, StatefulInteractiveElement, Styled,
    StyledExt as _, Svg, Toggled, Window, div, px, relative, rems, svg,
};

/// 复选框（Checkbox）元素。
#[derive(IntoElement)]
pub struct Checkbox {
    /// 元素 ID
    id: ElementId,
    /// 基础 Div 元素
    base: crate::Div,
    /// 样式精炼
    style: crate::StyleRefinement,
    /// 标签文本
    label: Option<ComponentText>,
    /// 子元素
    children: Vec<AnyElement>,
    /// 是否选中
    checked: bool,
    /// 是否禁用
    disabled: bool,
    /// 尺寸
    size: ElementSize,
    /// 是否为 Tab 停靠点
    tab_stop: bool,
    /// Tab 索引
    tab_index: isize,
    /// 值变更回调（参数为变更后的选中状态，按值传递）
    on_change: Option<Arc<dyn Fn(bool, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 提示文本（当前简化存储，暂不渲染）
    tooltip: Option<SharedString>,
}

impl Checkbox {
    /// 创建带有指定 id 的新复选框。
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            base: div(),
            style: crate::StyleRefinement::default(),
            label: None,
            children: Vec::new(),
            checked: false,
            disabled: false,
            size: ElementSize::default(),
            on_change: None,
            tab_stop: true,
            tab_index: 0,
            tooltip: None,
        }
    }

    /// 设置复选框的提示文本。
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// 设置复选框的标签。
    pub fn label(mut self, label: impl Into<ComponentText>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 设置复选框的选中状态。
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// 设置复选框的值变更回调。
    ///
    /// `bool` 参数表示点击后的新选中状态（按值传递）。
    pub fn on_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }

    /// 设置复选框是否为 Tab 停靠点，默认为 true。
    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self
    }

    /// 设置复选框的 Tab 索引，默认为 0。
    pub fn tab_index(mut self, tab_index: isize) -> Self {
        self.tab_index = tab_index;
        self
    }

    fn handle_change(
        on_change: &Option<Arc<dyn Fn(bool, &mut Window, &mut App) + Send + Sync + 'static>>,
        checked: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        let new_checked = !checked;
        if let Some(f) = on_change {
            (f)(new_checked, window, cx);
        }
    }
}

impl InteractiveElement for Checkbox {
    fn interactivity(&mut self) -> &mut crate::Interactivity {
        self.base.interactivity()
    }
}
impl StatefulInteractiveElement for Checkbox {}

impl Styled for Checkbox {
    fn style(&mut self) -> &mut crate::StyleRefinement {
        &mut self.style
    }
}

impl Disableable for Checkbox {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Selectable for Checkbox {
    fn selected(self, selected: bool) -> Self {
        self.checked(selected)
    }

    fn is_selected(&self) -> bool {
        self.checked
    }
}

impl ParentElement for Checkbox {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Sizable for Checkbox {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

/// 构建选中态对勾 Svg（纯构建部分，可单测）。
///
/// 对勾使用编译期嵌入字节，不依赖宿主 App 的 `AssetSource`。
fn check_svg(size: ElementSize, checked: bool, disabled: bool, cx: &App) -> Svg {
    let color = if disabled {
        cx.theme().primary_foreground.opacity(0.5)
    } else {
        cx.theme().primary_foreground
    };

    svg()
        .absolute()
        .top_px()
        .left_px()
        .map(|this| match size {
            ElementSize::XSmall => this.size_2(),
            ElementSize::Small => this.size_2p5(),
            ElementSize::Medium => this.size_3(),
            ElementSize::Large => this.size_3p5(),
            _ => this.size_3(),
        })
        .text_color(color)
        .map(|this| match checked {
            // 内置对勾必须附带编译期嵌入字节，不依赖宿主 App 的 AssetSource；
            // 否则未配置资源的窗口中对勾画不出，选中态只剩纯色底（黑框）。
            true => this
                .path(crate::IconName::Check.path())
                .data(crate::IconName::Check.bytes()),
            _ => this,
        })
}

pub(crate) fn checkbox_check_icon(
    id: ElementId,
    size: ElementSize,
    checked: bool,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let toggle_state = window.use_keyed_state(id, cx, |_, _| checked);

    check_svg(size, checked, disabled, cx).map(|this| {
        if !disabled && checked != *toggle_state.read(cx) {
            let duration = Duration::from_secs_f64(0.25);
            cx.spawn({
                let toggle_state = toggle_state.clone();
                async move |cx| {
                    cx.background_executor().timer(duration).await;
                    _ = toggle_state.update(cx, |this, _| *this = checked);
                }
            })
            .detach();

            this.with_animation(
                ElementId::NamedInteger("toggle".into(), checked as u64),
                Animation::new(Duration::from_secs_f64(0.25)),
                move |this, delta| this.opacity(if checked { 1.0 * delta } else { 1.0 - delta }),
            )
            .into_any_element()
        } else {
            this.into_any_element()
        }
    })
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        div().child(self.render_box(window, cx))
    }
}

impl Checkbox {
    /// 复选框主体（带无障碍标注的内层盒子）。
    ///
    /// 抽成独立方法以便单测直断 role/label/toggled，不经过外层包裹 `div`；
    /// 渲染树与原来完全一致（外层仍包一层 `div`）。
    fn render_box(self, window: &mut Window, cx: &mut App) -> crate::Stateful<crate::Div> {
        let checked = self.checked;

        let focus_handle = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let is_focused = focus_handle.is_focused(window);

        let border_color = if checked {
            cx.theme().primary
        } else {
            cx.theme().input
        };
        let color = if self.disabled {
            border_color.opacity(0.5)
        } else {
            border_color
        };
        let radius = cx.theme().radius.min(px(4.));

        self.base
            .id(self.id.clone())
            .role(Role::CheckBox)
            .aria_toggled(if checked {
                Toggled::True
            } else {
                Toggled::False
            })
            .when_some(
                self.label.as_ref().map(|l| l.get_text(cx)),
                |this, label| this.aria_label(label),
            )
            .when(!self.disabled, |this| {
                this.track_focus(
                    &focus_handle
                        .tab_stop(self.tab_stop)
                        .tab_index(self.tab_index),
                )
            })
            .h_flex()
            .gap_2()
            .items_start()
            .line_height(relative(1.))
            .text_color(cx.theme().foreground)
            .map(|this| match self.size {
                ElementSize::XSmall => this.text_xs(),
                ElementSize::Small => this.text_sm(),
                ElementSize::Medium => this.text_base(),
                ElementSize::Large => this.text_lg(),
                _ => this,
            })
            .when(self.disabled, |this| {
                this.text_color(cx.theme().muted_foreground)
            })
            .rounded(cx.theme().radius * 0.5)
            .focus_ring(is_focused, px(2.), window, cx)
            .refine_style(&self.style)
            .child(
                div()
                    .relative()
                    .map(|this| match self.size {
                        ElementSize::XSmall => this.size_3(),
                        ElementSize::Small => this.size_3p5(),
                        ElementSize::Medium => this.size_4(),
                        ElementSize::Large => this.size(rems(1.125)),
                        _ => this.size_4(),
                    })
                    .flex_shrink_0()
                    .border_1()
                    .border_color(color)
                    .rounded(radius)
                    .when(cx.theme().shadow && !self.disabled, |this| this.shadow_xs())
                    .map(|this| match checked {
                        false => this.bg(cx.theme().input_background()),
                        true if self.disabled => this.bg(color),
                        true => this.bg(cx.theme().tokens.primary),
                    })
                    .child(checkbox_check_icon(
                        self.id,
                        self.size,
                        checked,
                        self.disabled,
                        window,
                        cx,
                    )),
            )
            .when(self.label.is_some() || !self.children.is_empty(), |this| {
                this.child(
                    crate::v_flex()
                        .flex_1()
                        .overflow_hidden()
                        .line_height(relative(1.2))
                        .gap_1()
                        .map(|this| {
                            if let Some(label) = self.label {
                                this.child(
                                    div()
                                        .size_full()
                                        .text_color(cx.theme().foreground)
                                        .when(self.disabled, |this| {
                                            this.text_color(cx.theme().muted_foreground)
                                        })
                                        .line_height(relative(1.))
                                        .child(label),
                                )
                            } else {
                                this
                            }
                        })
                        .children(self.children),
                )
            })
            .on_mouse_down(crate::MouseButton::Left, |_, window, _| {
                // 避免在鼠标按下时获得焦点
                window.prevent_default();
            })
            .when(!self.disabled, |this| {
                this.on_click({
                    let on_change = self.on_change.clone();
                    move |_, window, cx| {
                        window.prevent_default();
                        Self::handle_change(&on_change, checked, window, cx);
                    }
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TestAppContext;

    /// 测试复选框基本构造与标签
    #[test]
    fn test_checkbox_build() {
        let cb = Checkbox::new("test-checkbox")
            .label("测试复选框")
            .checked(true)
            .tooltip("测试提示")
            .with_size(ElementSize::Medium);
        assert!(cb.checked);
        assert!(cb.label.is_some());
        assert_eq!(cb.tooltip, Some("测试提示".into()));
    }

    /// 测试复选框 disabled 状态
    #[test]
    fn test_checkbox_disabled() {
        let cb = Checkbox::new("test-checkbox-2")
            .disabled(true)
            .selected(true);
        assert!(cb.disabled);
        assert!(cb.is_selected());
    }

    /// 无障碍标注：role/label/toggled/点击动作（gpui-kit 同款口径）。
    #[rgpui::test]
    fn checkbox_a11y_role_label_and_toggle(cx: &mut TestAppContext) {
        use crate::{Element as _, IntoElement as _, Render, canvas};
        use std::sync::{Arc, Mutex};

        type Captured = Arc<Mutex<Option<[accesskit::Node; 3]>>>;
        struct Probe {
            captured: Captured,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut crate::Context<Self>) -> impl IntoElement {
                let captured = self.captured.clone();
                canvas(
                    move |_, window, cx| {
                        let mut info = |checkbox: Checkbox| {
                            let el = checkbox.render_box(window, cx).into_element();
                            assert_eq!(el.a11y_role(), Some(Role::CheckBox));
                            let mut node = accesskit::Node::new(Role::CheckBox);
                            el.write_a11y_info(&mut node);
                            node
                        };
                        *captured.lock().unwrap() = Some([
                            info(Checkbox::new("unchecked").label("记住我")),
                            info(Checkbox::new("checked").checked(true)),
                            info(Checkbox::new("disabled").disabled(true)),
                        ]);
                    },
                    |_, _, _, _| {},
                )
            }
        }

        cx.update(crate::theme::init);
        let captured: Captured = Arc::new(Mutex::new(None));
        let result = captured.clone();
        let (_, cx) = cx.add_window_view(move |_, _| Probe { captured });
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        let [unchecked, checked, disabled] = result.lock().unwrap().take().unwrap();
        assert_eq!(unchecked.label(), Some("记住我"));
        assert_eq!(unchecked.toggled(), Some(Toggled::False));
        assert!(unchecked.supports_action(accesskit::Action::Click));
        assert_eq!(checked.toggled(), Some(Toggled::True));
        assert!(checked.supports_action(accesskit::Action::Click));
        // 禁用态不挂点击：辅助技术不提供激活动作。
        assert!(!disabled.supports_action(accesskit::Action::Click));
    }

    /// 选中态对勾必须携带嵌入 SVG 字节，不依赖宿主 AssetSource。
    ///
    /// 回归测试：此前对勾仅设置 `path`，在未配置资源的窗口中解析失败，
    /// 选中后只剩纯色底（黑框），对勾画不出。
    #[rgpui::test]
    fn checked_icon_embeds_svg_data(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::theme::init(cx);
            assert!(!crate::IconName::Check.bytes().is_empty());

            let checked = check_svg(ElementSize::Medium, true, false, cx);
            assert!(checked.has_embedded_data());

            let unchecked = check_svg(ElementSize::Medium, false, false, cx);
            assert!(!unchecked.has_embedded_data());
        });
    }
}
