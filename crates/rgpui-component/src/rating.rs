use crate::theme::ActiveTheme;
use crate::{Disableable, Icon, IconName, Sizable, Size, StyledExt, h_flex};
use std::rc::Rc;

use rgpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, StyleRefinement,
    Styled, Window, div, prelude::FluentBuilder as _,
};
use rgpui::{ClickEvent, Hsla, StatefulInteractiveElement};

/// 星级评分组件
#[derive(IntoElement)]
pub struct Rating {
    /// 元素 ID
    id: ElementId,
    /// 样式引用
    style: StyleRefinement,
    /// 组件尺寸
    size: Size,
    /// 是否禁用
    disabled: bool,
    /// 当前评分值
    value: usize,
    /// 最大评分值
    max: usize,
    /// 激活状态颜色
    color: Option<Hsla>,
    /// 评分变化时的回调
    on_click: Option<Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>>,
}

impl Rating {
    /// 创建新的星级评分组件
    ///
    /// # 参数
    ///
    /// * `id` - 元素 ID
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            size: Size::Medium,
            disabled: false,
            value: 0,
            max: 5,
            color: None,
            on_click: None,
        }
    }

    /// 设置星星尺寸
    pub fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }

    /// 禁用交互
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置激活颜色，默认使用主题色中的 `yellow`
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// 设置初始评分值（0..=max）
    pub fn value(mut self, value: usize) -> Self {
        self.value = value;
        if self.value > self.max {
            self.value = self.max;
        }
        self
    }

    /// 设置最大星星数量
    pub fn max(mut self, max: usize) -> Self {
        self.max = max;
        if self.value > self.max {
            self.value = self.max;
        }
        self
    }

    /// 添加评分变化时的回调
    ///
    /// `&usize` 参数为新的评分值
    pub fn on_click(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Styled for Rating {
    fn style(&mut self) -> &mut rgpui::StyleRefinement {
        &mut self.style
    }
}

impl Sizable for Rating {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl Disableable for Rating {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// 评分状态
struct RaingState {
    /// 保存初始化时的默认值，用于检测外部值变化
    default_value: usize,
    /// 当前选中的评分值
    value: usize,
    /// 当前悬停的评分值
    hovered_value: usize,
}

impl RenderOnce for Rating {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id;
        let size = self.size;
        let disabled = self.disabled;
        let max = self.max;
        let default_value = self.value;
        let active_color = self.color.unwrap_or(cx.theme().yellow);
        let on_click = self.on_click.clone();

        let state = window.use_keyed_state(id.clone(), cx, |_, _| RaingState {
            default_value,
            value: default_value,
            hovered_value: 0,
        });

        // 如果外部改变了 value 属性，则重置状态
        if state.read(cx).default_value != default_value {
            state.update(cx, |state, _| {
                state.default_value = default_value;
                state.value = default_value;
            });
        }
        let value = state.read(cx).value;

        h_flex()
            .id(id)
            .flex_nowrap()
            .refine_style(&self.style)
            .on_hover(window.listener_for(&state, move |state, hovered, _, cx| {
                if !hovered {
                    state.hovered_value = 0;
                    cx.notify();
                }
            }))
            .map(|mut this| {
                for ix in 1..=max {
                    let filled = ix <= value;
                    let hovered = state.read(cx).hovered_value >= ix;

                    this = this.child(
                        div()
                            .id(ix)
                            .p_0p5()
                            .flex_none()
                            .flex_shrink_0()
                            .when(filled || hovered, |this| this.text_color(active_color))
                            .child(
                                Icon::new(if filled {
                                    IconName::StarFill
                                } else {
                                    IconName::Star
                                })
                                .with_size(size),
                            )
                            .when(!disabled, |this| {
                                this.on_mouse_move(window.listener_for(
                                    &state,
                                    move |state, _, _, cx| {
                                        state.hovered_value = ix;
                                        cx.notify();
                                    },
                                ))
                                .on_click({
                                    let state = state.clone();
                                    let on_click = on_click.clone();
                                    move |_: &ClickEvent, window, cx| {
                                        let new = if value >= ix {
                                            ix.saturating_sub(1)
                                        } else {
                                            ix
                                        };

                                        state.update(cx, |state, cx| {
                                            state.value = new;
                                            cx.notify();
                                        });

                                        if let Some(on_click) = &on_click {
                                            on_click(&new, window, cx);
                                        }
                                    }
                                })
                            }),
                    );
                }

                this
            })
    }
}
