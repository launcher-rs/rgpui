//! 标签输入：可添加/删除多个标签的文本输入。

use std::rc::Rc;

use crate::input_ui::{Input, InputState};
use crate::{prelude::FluentBuilder as _, *};

/// 标签输入状态。
pub struct TagInputState {
    /// 已添加的标签。
    tags: Vec<SharedString>,
    /// 输入框状态实体（复用 `input_ui` 输入状态，支持完整键盘与输入法处理）。
    input: Entity<InputState>,
    /// 标签数量上限。
    max_tags: Option<usize>,
}

impl TagInputState {
    /// 创建标签输入状态。
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx));
        Self {
            tags: Vec::new(),
            input,
            max_tags: None,
        }
    }

    /// 以初始标签列表创建状态。
    pub fn with_tags(
        window: &mut Window,
        cx: &mut Context<Self>,
        tags: Vec<impl Into<SharedString>>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx));
        Self {
            tags: tags.into_iter().map(|t| t.into()).collect(),
            input,
            max_tags: None,
        }
    }

    /// 获取已添加的标签。
    pub fn tags(&self) -> &[SharedString] {
        &self.tags
    }

    /// 替换标签列表。
    pub fn set_tags(&mut self, tags: Vec<impl Into<SharedString>>, cx: &mut Context<Self>) {
        self.tags = tags.into_iter().map(|t| t.into()).collect();
        cx.notify();
    }

    /// 添加标签，去重且受数量上限约束；成功返回 true。
    pub fn add_tag(&mut self, tag: impl Into<SharedString>, cx: &mut Context<Self>) -> bool {
        let tag = tag.into();
        if tag.is_empty() {
            return false;
        }
        if self.tags.iter().any(|t| t.as_ref() == tag.as_ref()) {
            return false;
        }
        if let Some(max) = self.max_tags {
            if self.tags.len() >= max {
                return false;
            }
        }
        self.tags.push(tag);
        cx.notify();
        true
    }

    /// 按下标移除标签。
    pub fn remove_tag(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.tags.len() {
            self.tags.remove(index);
            cx.notify();
        }
    }

    /// 移除最后一个标签。
    pub fn remove_last_tag(&mut self, cx: &mut Context<Self>) {
        if !self.tags.is_empty() {
            self.tags.pop();
            cx.notify();
        }
    }

    /// 清空所有标签。
    pub fn clear_tags(&mut self, cx: &mut Context<Self>) {
        self.tags.clear();
        cx.notify();
    }

    /// 获取输入框文本。
    pub fn input_value(&self, cx: &App) -> String {
        self.input.read(cx).text().to_string()
    }

    /// 设置输入框文本。
    pub fn set_input_value(
        &mut self,
        value: impl Into<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value: String = value.into();
        self.input.update(cx, |input, cx| {
            input.set_value(value, window, cx);
        });
        cx.notify();
    }

    /// 获取标签数量上限。
    pub fn max_tags(&self) -> Option<usize> {
        self.max_tags
    }

    /// 设置标签数量上限。
    pub fn set_max_tags(&mut self, max: Option<usize>, cx: &mut Context<Self>) {
        self.max_tags = max;
        cx.notify();
    }

    /// 提交当前输入为标签，成功时清空输入。
    pub fn commit_input(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let value = self.input.read(cx).text().to_string().trim().to_string();
        if self.add_tag(value, cx) {
            self.input.update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
            true
        } else {
            false
        }
    }
}

impl Render for TagInputState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// 标签输入组件。
#[derive(IntoElement)]
pub struct TagInput {
    /// 绑定状态实体。
    state: Entity<TagInputState>,
    /// 占位文本。
    placeholder: SharedString,
    /// 是否禁用。
    disabled: bool,
    /// 建议标签列表。
    suggestions: Vec<SharedString>,
    /// 标签变化回调。
    on_change: Option<Rc<dyn Fn(&[SharedString], &mut Window, &mut App)>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl TagInput {
    /// 创建标签输入，默认占位 "Add tag..."。
    pub fn new(state: Entity<TagInputState>) -> Self {
        Self {
            state,
            placeholder: "Add tag...".into(),
            disabled: false,
            suggestions: Vec::new(),
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置是否禁用。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置建议标签列表。
    pub fn suggestions(mut self, suggestions: Vec<impl Into<SharedString>>) -> Self {
        self.suggestions = suggestions.into_iter().map(|s| s.into()).collect();
        self
    }

    /// 设置标签变化回调。
    pub fn on_change(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for TagInput {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TagInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let user_style = self.style;
        let placeholder = self.placeholder;
        let disabled = self.disabled;
        let state = self.state.clone();

        // 将占位文本同步到输入状态（Input 组件从输入状态读取占位文本）。
        state.update(cx, |s, cx| {
            s.input.update(cx, |input, cx| {
                input.set_placeholder(placeholder.clone(), window, cx);
            });
        });

        let theme = cx.theme();
        let state_data = self.state.read(cx);
        let tags = state_data.tags.clone();
        let input_state = state_data.input.clone();
        let is_focused = state_data.input.read(cx).is_focused(window);

        let mut root = div();
        root.style().refine(&user_style);

        root = root.child(
            div()
                .id("tag-input-container")
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(6.0))
                .min_h(px(40.0))
                .px(px(10.0))
                .py(px(6.0))
                .bg(theme.tokens.background)
                .border_1()
                .border_color(if is_focused {
                    theme.tokens.ring
                } else {
                    theme.tokens.input
                })
                .rounded(theme.radius)
                .when(disabled, |d| d.opacity(0.5))
                .when(!disabled, |d| {
                    // 点击容器任意位置都聚焦输入框。
                    let state = state.clone();
                    d.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        state.update(cx, |s, cx| {
                            s.input.update(cx, |input, cx| input.focus(window, cx));
                        });
                    })
                })
                .children(tags.iter().enumerate().map(|(idx, tag)| {
                    let state_for_remove = state.clone();
                    let on_change = self.on_change.clone();
                    let disabled = disabled;

                    div()
                        .id(SharedString::from(format!("tag-{}", idx)))
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .px(px(8.0))
                        .py(px(4.0))
                        .bg(theme.tokens.primary.opacity(0.1))
                        .text_color(theme.tokens.primary)
                        .rounded(px(4.0))
                        .text_size(px(13.0))
                        .font_family(theme.font_family.clone())
                        .child(tag.clone())
                        .when(!disabled, |d| {
                            d.child(
                                div()
                                    .id(SharedString::from(format!("tag-remove-{}", idx)))
                                    .ml(px(2.0))
                                    .cursor_pointer()
                                    .rounded(px(2.0))
                                    .hover(|s| s.bg(theme.tokens.primary.opacity(0.2)))
                                    .text_size(px(12.0))
                                    .on_click(move |_, window, cx| {
                                        state_for_remove.update(cx, |s, cx| {
                                            s.remove_tag(idx, cx);
                                            if let Some(ref handler) = on_change {
                                                handler(&s.tags, window, &mut **cx);
                                            }
                                        });
                                    })
                                    .child("×"),
                            )
                        })
                }))
                .when(!disabled, {
                    let state_for_input = state;
                    let input_state = input_state;
                    let on_change = self.on_change.clone();

                    move |container| {
                        container.child(
                            div().flex_1().min_w(px(80.0)).child(
                                div()
                                    .id("tag-input-field")
                                    .min_w(px(60.0))
                                    .on_key_down({
                                        let state = state_for_input.clone();
                                        let on_change = on_change.clone();
                                        move |event, window, cx| match event.keystroke.key.as_str()
                                        {
                                            "enter" => {
                                                state.update(cx, |s, cx| {
                                                    if s.commit_input(window, cx) {
                                                        if let Some(ref handler) = on_change {
                                                            handler(&s.tags, window, &mut **cx);
                                                        }
                                                    }
                                                });
                                                cx.stop_propagation();
                                            }
                                            "backspace" => {
                                                state.update(cx, |s, cx| {
                                                    if s.input.read(cx).text().len() == 0
                                                        && !s.tags.is_empty()
                                                    {
                                                        s.remove_last_tag(cx);
                                                        if let Some(ref handler) = on_change {
                                                            handler(&s.tags, window, &mut **cx);
                                                        }
                                                        cx.stop_propagation();
                                                    }
                                                });
                                            }
                                            _ => {}
                                        }
                                    })
                                    .child(
                                        Input::new(&input_state)
                                            .appearance(false)
                                            .bordered(false)
                                            .small(),
                                    ),
                            ),
                        )
                    }
                }),
        );

        root
    }
}
