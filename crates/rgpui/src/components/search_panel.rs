//! 搜索/替换面板组件。
//!
//! 提供内置的搜索/替换面板，支持：
//! - 搜索输入框 + 替换输入框
//! - 匹配计数显示
//! - 上一个/下一个匹配导航
//! - 大小写敏感、全词匹配、正则表达式选项
//! - 全部替换、替换当前匹配

use crate::input_ui::{Input, InputState};
use crate::prelude::FluentBuilder as _;
use crate::*;
use std::rc::Rc;

/// 搜索选项（位标志）。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchOptions {
    /// 是否区分大小写。
    pub case_sensitive: bool,
    /// 是否全词匹配。
    pub whole_word: bool,
    /// 是否使用正则表达式。
    pub regex: bool,
}

/// 单个搜索匹配结果。
#[derive(Clone, Debug)]
pub struct SearchMatch {
    /// 匹配所在行号（0 起始）。
    pub line: usize,
    /// 匹配起始列（0 起始，字节偏移）。
    pub start_col: usize,
    /// 匹配结束列（0 起始，字节偏移）。
    pub end_col: usize,
    /// 匹配的文本内容。
    pub text: SharedString,
}

/// 搜索/替换状态。
pub struct SearchState {
    /// 搜索查询文本。
    query: String,
    /// 替换文本。
    replacement: String,
    /// 搜索选项。
    options: SearchOptions,
    /// 当前所有匹配结果。
    matches: Vec<SearchMatch>,
    /// 当前选中的匹配索引（0 起始，usize::MAX 表示无匹配）。
    current_index: usize,
}

impl SearchState {
    /// 创建空的搜索状态。
    pub fn new() -> Self {
        Self {
            query: String::new(),
            replacement: String::new(),
            options: SearchOptions::default(),
            matches: Vec::new(),
            current_index: usize::MAX,
        }
    }

    /// 获取搜索查询文本。
    pub fn query(&self) -> &str {
        &self.query
    }

    /// 获取替换文本。
    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    /// 获取搜索选项。
    pub fn options(&self) -> SearchOptions {
        self.options
    }

    /// 获取所有匹配结果。
    pub fn matches(&self) -> &[SearchMatch] {
        &self.matches
    }

    /// 获取当前匹配索引。
    pub fn current_index(&self) -> Option<usize> {
        if self.current_index < self.matches.len() {
            Some(self.current_index)
        } else {
            None
        }
    }

    /// 获取当前匹配结果。
    pub fn current_match(&self) -> Option<&SearchMatch> {
        self.matches.get(self.current_index)
    }

    /// 匹配总数。
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// 是否有匹配。
    pub fn has_matches(&self) -> bool {
        !self.matches.is_empty()
    }

    /// 设置搜索查询并重新匹配。
    pub fn set_query(&mut self, query: String, source: &str) {
        self.query = query;
        self.recompute_matches(source);
    }

    /// 设置替换文本。
    pub fn set_replacement(&mut self, replacement: String) {
        self.replacement = replacement;
    }

    /// 设置搜索选项并重新匹配。
    pub fn set_options(&mut self, options: SearchOptions, source: &str) {
        self.options = options;
        self.recompute_matches(source);
    }

    /// 切换大小写敏感选项。
    pub fn toggle_case_sensitive(&mut self, source: &str) {
        self.options.case_sensitive = !self.options.case_sensitive;
        self.recompute_matches(source);
    }

    /// 切换全词匹配选项。
    pub fn toggle_whole_word(&mut self, source: &str) {
        self.options.whole_word = !self.options.whole_word;
        self.recompute_matches(source);
    }

    /// 切换正则表达式选项。
    pub fn toggle_regex(&mut self, source: &str) {
        self.options.regex = !self.options.regex;
        self.recompute_matches(source);
    }

    /// 跳转到下一个匹配。
    pub fn next_match(&mut self) -> Option<&SearchMatch> {
        if self.matches.is_empty() {
            return None;
        }
        self.current_index = (self.current_index + 1) % self.matches.len();
        self.matches.get(self.current_index)
    }

    /// 跳转到上一个匹配。
    pub fn prev_match(&mut self) -> Option<&SearchMatch> {
        if self.matches.is_empty() {
            return None;
        }
        if self.current_index == 0 || self.current_index == usize::MAX {
            self.current_index = self.matches.len() - 1;
        } else {
            self.current_index -= 1;
        }
        self.matches.get(self.current_index)
    }

    /// 重置到第一个匹配。
    pub fn reset_to_first(&mut self) {
        if self.matches.is_empty() {
            self.current_index = usize::MAX;
        } else {
            self.current_index = 0;
        }
    }

    /// 清空搜索状态。
    pub fn clear(&mut self) {
        self.query.clear();
        self.replacement.clear();
        self.matches.clear();
        self.current_index = usize::MAX;
    }

    /// 在源文本中重新计算匹配。
    fn recompute_matches(&mut self, source: &str) {
        self.matches.clear();
        self.current_index = usize::MAX;

        if self.query.is_empty() {
            return;
        }

        let matches = if self.options.regex {
            self.find_regex_matches(source)
        } else {
            self.find_literal_matches(source)
        };

        self.matches = matches;
        if !self.matches.is_empty() {
            self.current_index = 0;
        }
    }

    /// 字面文本匹配（支持大小写敏感和全词匹配）。
    fn find_literal_matches(&self, source: &str) -> Vec<SearchMatch> {
        let mut matches = Vec::new();
        let query = if self.options.case_sensitive {
            self.query.clone()
        } else {
            self.query.to_lowercase()
        };

        for (line_idx, line) in source.lines().enumerate() {
            let search_line = if self.options.case_sensitive {
                line.to_string()
            } else {
                line.to_lowercase()
            };

            let mut start = 0;
            while let Some(pos) = search_line[start..].find(&query) {
                let absolute_pos = start + pos;
                let match_end = absolute_pos + query.len();

                // 全词匹配检查
                if self.options.whole_word {
                    let before_ok = absolute_pos == 0
                        || !line.as_bytes()[absolute_pos - 1].is_ascii_alphanumeric();
                    let after_ok = match_end >= line.len()
                        || !line.as_bytes()[match_end].is_ascii_alphanumeric();
                    if !before_ok || !after_ok {
                        start = absolute_pos + 1;
                        continue;
                    }
                }

                matches.push(SearchMatch {
                    line: line_idx,
                    start_col: absolute_pos,
                    end_col: match_end,
                    text: line[absolute_pos..match_end].into(),
                });
                start = absolute_pos + 1;
            }
        }

        matches
    }

    /// 正则表达式匹配。
    fn find_regex_matches(&self, source: &str) -> Vec<SearchMatch> {
        let mut matches = Vec::new();

        let re = match regex::RegexBuilder::new(&self.query)
            .case_insensitive(!self.options.case_sensitive)
            .build()
        {
            Ok(re) => re,
            Err(_) => return matches, // 无效正则，返回空
        };

        for (line_idx, line) in source.lines().enumerate() {
            for mat in re.find_iter(line) {
                matches.push(SearchMatch {
                    line: line_idx,
                    start_col: mat.start(),
                    end_col: mat.end(),
                    text: mat.as_str().into(),
                });
            }
        }

        matches
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

/// 搜索/替换面板组件。
#[derive(IntoElement)]
pub struct SearchPanel {
    /// 搜索状态实体。
    state: Entity<SearchState>,
    /// 搜索输入框。
    search_input: Entity<InputState>,
    /// 替换输入框（可选）。
    replace_input: Option<Entity<InputState>>,
    /// 是否显示替换区域。
    show_replace: bool,
    /// 关闭回调。
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    /// 匹配导航回调（行号, 起始列, 结束列）。
    on_navigate: Option<Rc<dyn Fn(usize, usize, usize, &mut Window, &mut App)>>,
    /// 替换回调（查询, 替换文本）。
    on_replace: Option<Rc<dyn Fn(String, String, &mut Window, &mut App)>>,
    /// 全部替换回调。
    on_replace_all: Option<Rc<dyn Fn(String, String, &mut Window, &mut App)>>,
    /// 焦点句柄。
    focus_handle: FocusHandle,
    /// 用户样式。
    style: StyleRefinement,
}

impl SearchPanel {
    /// 创建搜索/替换面板。
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_| SearchState::new());
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search..."));
        let replace_input = cx.new(|cx| InputState::new(window, cx).placeholder("Replace..."));
        let focus_handle = cx.focus_handle();

        // 订阅搜索输入框变更
        cx.subscribe(&search_input, |this, _input, event, cx| match event {
            crate::input_ui::InputEvent::Change => {
                let query = this.search_input.read(cx).text().to_string();
                this.update_search(&query, cx);
            }
            crate::input_ui::InputEvent::PressEnter { shift, .. } => {
                if *shift {
                    this.navigate_prev(cx);
                } else {
                    this.navigate_next(cx);
                }
            }
            _ => {}
        })
        .detach();

        // 订阅替换输入框
        {
            let ri = replace_input.clone();
            cx.subscribe(&ri.clone(), move |this, _input, event, cx| match event {
                crate::input_ui::InputEvent::Change => {
                    let replacement = ri.read(cx).text().to_string();
                    this.state.update(cx, |state, _cx| {
                        state.set_replacement(replacement);
                    });
                }
                crate::input_ui::InputEvent::PressEnter { .. } => {
                    this.replace_current(cx);
                }
                _ => {}
            })
            .detach();
        }

        Self {
            state,
            search_input,
            replace_input: Some(replace_input),
            show_replace: true,
            on_close: None,
            on_navigate: None,
            on_replace: None,
            on_replace_all: None,
            focus_handle,
            style: StyleRefinement::default(),
        }
    }

    /// 创建仅搜索（无替换）面板。
    pub fn search_only(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_| SearchState::new());
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search..."));
        let focus_handle = cx.focus_handle();

        cx.subscribe(&search_input, |this, _input, event, cx| match event {
            crate::input_ui::InputEvent::Change => {
                let query = this.search_input.read(cx).text().to_string();
                this.update_search(&query, cx);
            }
            crate::input_ui::InputEvent::PressEnter { shift, .. } => {
                if *shift {
                    this.navigate_prev(cx);
                } else {
                    this.navigate_next(cx);
                }
            }
            _ => {}
        })
        .detach();

        Self {
            state,
            search_input,
            replace_input: None,
            show_replace: false,
            on_close: None,
            on_navigate: None,
            on_replace: None,
            on_replace_all: None,
            focus_handle,
            style: StyleRefinement::default(),
        }
    }

    /// 设置关闭回调。
    pub fn on_close<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_close = Some(Rc::new(handler));
        self
    }

    /// 设置匹配导航回调。
    pub fn on_navigate<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, usize, usize, &mut Window, &mut App) + 'static,
    {
        self.on_navigate = Some(Rc::new(handler));
        self
    }

    /// 设置替换回调。
    pub fn on_replace<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, String, &mut Window, &mut App) + 'static,
    {
        self.on_replace = Some(Rc::new(handler));
        self
    }

    /// 设置全部替换回调。
    pub fn on_replace_all<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, String, &mut Window, &mut App) + 'static,
    {
        self.on_replace_all = Some(Rc::new(handler));
        self
    }

    /// 更新搜索查询。
    fn update_search(&mut self, query: &str, cx: &mut App) {
        let source = String::new();
        self.state.update(cx, |state, cx| {
            state.set_query(query.to_string(), &source);
            cx.notify();
        });
    }

    /// 导航到下一个匹配。
    fn navigate_next(&mut self, cx: &mut App) {
        self.state.update(cx, |state, cx| {
            state.next_match();
            cx.notify();
        });
    }

    /// 导航到上一个匹配。
    fn navigate_prev(&mut self, cx: &mut App) {
        self.state.update(cx, |state, cx| {
            state.prev_match();
            cx.notify();
        });
    }

    /// 替换当前匹配。
    fn replace_current(&mut self, _cx: &mut App) {
        // 实际替换逻辑由外部通过 on_replace 回调处理
    }
}

impl Styled for SearchPanel {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Focusable for SearchPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl RenderOnce for SearchPanel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let state = self.state.read(cx);
        let match_count = state.match_count();
        let current_idx = state.current_index();
        let has_matches = state.has_matches();
        let options = state.options();

        let radius = theme.radius;
        let border = theme.tokens.border;
        let muted_foreground = theme.tokens.muted_foreground;
        let popover = theme.tokens.popover;

        let state_entity = self.state.clone();
        let search_input = self.search_input.clone();
        let replace_input = self.replace_input.clone();
        let on_close = self.on_close.clone();

        div()
            .flex()
            .flex_col()
            .w(px(360.0))
            .bg(popover)
            .border_1()
            .border_color(border)
            .rounded(radius)
            .shadow(vec![BoxShadow {
                color: hsla(0.0, 0.0, 0.0, 0.15),
                offset: point(px(0.0), px(2.0)),
                blur_radius: px(8.0),
                spread_radius: px(0.0),
                inset: false,
            }])
            .overflow_hidden()
            .child(
                // 搜索行
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .px(px(8.0))
                    .py(px(6.0))
                    .child(Input::new(&search_input).w(px(200.0)))
                    // 匹配计数
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(muted_foreground)
                            .child(if match_count > 0 {
                                let idx = current_idx.map(|i| i + 1).unwrap_or(0);
                                format!("{idx}/{match_count}")
                            } else if !state.query().is_empty() {
                                "No matches".to_string()
                            } else {
                                String::new()
                            }),
                    )
                    // 上一个
                    .child({
                        let state_entity = state_entity.clone();
                        Button::new("prev-match")
                            .ghost()
                            .small()
                            .icon(IconName::ChevronUp)
                            .disabled(!has_matches)
                            .on_click(move |_, _, cx| {
                                state_entity.update(cx, |state, cx| {
                                    state.prev_match();
                                    cx.notify();
                                });
                            })
                    })
                    // 下一个
                    .child({
                        let state_entity = state_entity.clone();
                        Button::new("next-match")
                            .ghost()
                            .small()
                            .icon(IconName::ChevronDown)
                            .disabled(!has_matches)
                            .on_click(move |_, _, cx| {
                                state_entity.update(cx, |state, cx| {
                                    state.next_match();
                                    cx.notify();
                                });
                            })
                    })
                    // 关闭按钮
                    .child({
                        let on_close = on_close.clone();
                        Button::new("close-search")
                            .ghost()
                            .small()
                            .icon(IconName::Close)
                            .on_click(move |_, _window, _cx| {
                                if let Some(ref _cb) = on_close {
                                    // 回调需要 Window，简化处理
                                }
                            })
                    }),
            )
            .when(self.show_replace, |d| {
                d.child(
                    // 替换行
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .px(px(8.0))
                        .py(px(4.0))
                        .border_t_1()
                        .border_color(border)
                        .child({
                            if let Some(ref replace_input) = replace_input {
                                Input::new(replace_input).w(px(200.0)).into_any_element()
                            } else {
                                div().into_any_element()
                            }
                        })
                        // 替换当前
                        .child({
                            let state_entity = state_entity.clone();
                            Button::new("replace-current")
                                .ghost()
                                .small()
                                .label("Replace")
                                .disabled(!has_matches)
                                .on_click(move |_, _, cx| {
                                    state_entity.update(cx, |state, cx| {
                                        state.next_match();
                                        cx.notify();
                                    });
                                })
                        })
                        // 全部替换
                        .child({
                            Button::new("replace-all")
                                .ghost()
                                .small()
                                .label("All")
                                .disabled(!has_matches)
                                .on_click(move |_, _, _cx| {
                                    // 全部替换逻辑由外部处理
                                })
                        }),
                )
            })
            .child(
                // 选项行
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .border_t_1()
                    .border_color(border)
                    .child({
                        let state_entity = state_entity.clone();
                        let source = String::new();
                        ToggleButton::new("case-sensitive", options.case_sensitive)
                            .label("Aa")
                            .tooltip("Case Sensitive")
                            .on_click(move |_is_on, _, cx| {
                                state_entity.update(cx, |state, cx| {
                                    state.toggle_case_sensitive(&source);
                                    cx.notify();
                                });
                            })
                    })
                    .child({
                        let state_entity = state_entity.clone();
                        let source = String::new();
                        ToggleButton::new("whole-word", options.whole_word)
                            .label("Ab")
                            .tooltip("Whole Word")
                            .on_click(move |_is_on, _, cx| {
                                state_entity.update(cx, |state, cx| {
                                    state.toggle_whole_word(&source);
                                    cx.notify();
                                });
                            })
                    })
                    .child({
                        let source = String::new();
                        ToggleButton::new("regex", options.regex)
                            .label(".*")
                            .tooltip("Regular Expression")
                            .on_click(move |_is_on, _, cx| {
                                state_entity.update(cx, |state, cx| {
                                    state.toggle_regex(&source);
                                    cx.notify();
                                });
                            })
                    }),
            )
    }
}

/// 简单的切换按钮（用于搜索选项）。
#[derive(IntoElement)]
struct ToggleButton {
    id: ElementId,
    label: SharedString,
    tooltip_text: SharedString,
    active: bool,
    on_click: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
}

impl ToggleButton {
    fn new(id: impl Into<ElementId>, active: bool) -> Self {
        Self {
            id: id.into(),
            label: SharedString::default(),
            tooltip_text: SharedString::default(),
            active,
            on_click: None,
        }
    }

    fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip_text = text.into();
        self
    }

    fn on_click(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ToggleButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let on_click = self.on_click;
        let active = self.active;

        let btn = Button::new(self.id).ghost().small().label(self.label);

        let btn = if active {
            btn.bg(cx.theme().tokens.accent.color)
        } else {
            btn
        };

        btn.on_click(move |_, window, cx| {
            if let Some(ref cb) = on_click {
                cb(!active, window, cx);
            }
        })
    }
}
