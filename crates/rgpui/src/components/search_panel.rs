//! 搜索/替换面板组件。
//!
//! 提供内置的搜索/替换面板，支持：
//! - 搜索输入框 + 替换输入框
//! - 匹配计数显示
//! - 上一个/下一个匹配导航
//! - 大小写敏感、全词匹配、正则表达式选项
//! - 全部替换、替换当前匹配

use crate::input_ui::{Input, InputState, TextDecoration, TextDecorationCollection};
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

/// 搜索匹配标黄器：持有装饰集合句柄（增量刷新不泄漏）+ 可配色。
///
/// 默认黄底、文字色不变；查询变化时由调用方调 [`Self::mark`] 重标即可
/// （通常跟在 `SearchPanelState::set_source` 后面）。
#[derive(Clone)]
pub struct SearchHighlight {
    collection: Option<TextDecorationCollection>,
    background: Hsla,
    foreground: Option<Hsla>,
}

impl Default for SearchHighlight {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchHighlight {
    /// 创建标黄器（默认黄底）。
    pub fn new() -> Self {
        Self {
            collection: None,
            background: yellow(),
            foreground: None,
        }
    }

    /// 设置标黄配色（背景 + 可选文字色），下次 [`Self::mark`] 生效。
    pub fn set_colors(&mut self, background: Hsla, foreground: Option<Hsla>) {
        self.background = background;
        self.foreground = foreground;
    }

    /// 当前背景色。
    pub fn background(&self) -> Hsla {
        self.background
    }

    /// 当前文字色（`None` 表示保持原文颜色）。
    pub fn foreground(&self) -> Option<Hsla> {
        self.foreground
    }

    /// 标黄全部匹配。
    ///
    /// `source` 须与算出 `matches` 的是同一份文本（否则偏移错位）；
    /// 已有集合时增量 `set` 刷新，无泄漏。
    pub fn mark(
        &mut self,
        text: &Entity<InputState>,
        source: &str,
        matches: &[SearchMatch],
        cx: &mut App,
    ) {
        // 行首字节偏移表（钳制到行内，避免错位 ranges）。
        let mut line_starts = Vec::new();
        let mut offset = 0;
        for part in source.split('\n') {
            line_starts.push((offset, part.len()));
            offset += part.len() + 1;
        }
        let decorations: Vec<TextDecoration> = matches
            .iter()
            .map(|m| {
                let (base, len) = line_starts
                    .get(m.line)
                    .copied()
                    .unwrap_or((source.len(), 0));
                let start = base + m.start_col.min(len);
                let end = base + m.end_col.min(len).max(start - base);
                TextDecoration::new(
                    start..end,
                    HighlightStyle {
                        background_color: Some(self.background),
                        color: self.foreground,
                        ..Default::default()
                    },
                )
            })
            .collect();
        match self.collection.take() {
            Some(collection) => {
                collection.set(decorations, cx);
                self.collection = Some(collection);
            }
            None => {
                self.collection = Some(text.update(cx, |state, cx| {
                    state.create_decorations_collection(decorations, cx)
                }));
            }
        }
    }

    /// 清除标黄。
    pub fn clear(&mut self, cx: &mut App) {
        if let Some(collection) = self.collection.take() {
            collection.clear(cx);
        }
    }
}

/// 行列（字节列）转全文 UTF-8 字节偏移（钳制到行内，防错位）。
fn byte_offset_of(source: &str, line: usize, col: usize) -> usize {
    let mut offset = 0;
    for (ix, part) in source.split('\n').enumerate() {
        if ix == line {
            return offset + col.min(part.len());
        }
        offset += part.len() + 1;
    }
    offset
}

/// 可嵌入的搜索面板实体（`Render` 版）。
///
/// 父组件在自己的 `Context` 里用 `cx.new(|cx| SearchPanelState::new(window, cx))`
/// 创建一次存成 `Entity`，`render` 里直接 `child(panel.clone())`。
///
/// 查询/选项变化时用调用方经 [`Self::set_source`] 推送的全文重算匹配。
pub struct SearchPanelState {
    /// 搜索状态实体。
    state: Entity<SearchState>,
    /// 搜索输入框。
    search_input: Entity<InputState>,
    /// 替换输入框（可选）。
    replace_input: Option<Entity<InputState>>,
    /// 是否显示替换区域。
    show_replace: bool,
    /// 待搜索全文（`set_source` 推送）。
    source: String,
    /// 绑定的编辑器（`attach_editor` 设置，用于标黄与默认跳转）。
    attached_editor: Option<Entity<InputState>>,
    /// 绑定编辑器的匹配标黄器（延迟创建，配色可调）。
    highlight: Option<SearchHighlight>,
    /// 有待触发的导航（Enter/上下按钮只改索引，真正的回调在 render 里拿 Window 触发）。
    pending_navigate: bool,
    /// 有待触发的替换（替换框回车时无 Window，延后到 render 里触发）。
    pending_replace: bool,
    /// 匹配导航回调（行号, 起始列, 结束列）。
    on_navigate: Option<Rc<dyn Fn(usize, usize, usize, &mut Window, &mut App)>>,
    /// 替换回调（查询, 替换文本）。
    on_replace: Option<Rc<dyn Fn(String, String, &mut Window, &mut App)>>,
    /// 全部替换回调。
    on_replace_all: Option<Rc<dyn Fn(String, String, &mut Window, &mut App)>>,
    /// 关闭回调（面板自身不画关闭按钮，由父组件消费，如标题栏的 ×）。
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    /// 焦点句柄。
    focus_handle: FocusHandle,
    /// 用户样式。
    style: StyleRefinement,
}

impl SearchPanelState {
    /// 创建搜索/替换面板实体状态（`Context<SearchPanelState>` 内调用，父组件经 `cx.new` 间接调用）。
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_| SearchState::new());
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search..."));
        let replace_input = cx.new(|cx| InputState::new(window, cx).placeholder("Replace..."));
        let focus_handle = cx.focus_handle();

        // 跟随搜索状态（查询/选项/匹配变化即重渲染，计数标签实时更新）。
        cx.observe(&state, |_, _, cx| cx.notify()).detach();

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
            source: String::new(),
            attached_editor: None,
            highlight: None,
            pending_navigate: false,
            pending_replace: false,
            on_navigate: None,
            on_replace: None,
            on_replace_all: None,
            on_close: None,
            focus_handle,
            style: StyleRefinement::default(),
        }
    }

    /// 创建仅搜索（无替换）面板实体状态。
    pub fn search_only(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_| SearchState::new());
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search..."));
        let focus_handle = cx.focus_handle();

        // 跟随搜索状态（查询/选项/匹配变化即重渲染，计数标签实时更新）。
        cx.observe(&state, |_, _, cx| cx.notify()).detach();

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
            source: String::new(),
            attached_editor: None,
            highlight: None,
            pending_navigate: false,
            pending_replace: false,
            on_navigate: None,
            on_replace: None,
            on_replace_all: None,
            on_close: None,
            focus_handle,
            style: StyleRefinement::default(),
        }
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

    /// 设置关闭回调（面板自身不画关闭按钮，由父组件在需要时调用）。
    pub fn on_close<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_close = Some(Rc::new(handler));
        self
    }

    /// 设置替换回调（构建后追加/覆盖，与 builder 版 `on_replace` 等价）。
    pub fn set_on_replace<F>(&mut self, handler: F)
    where
        F: Fn(String, String, &mut Window, &mut App) + 'static,
    {
        self.on_replace = Some(Rc::new(handler));
    }

    /// 设置全部替换回调（构建后追加/覆盖，与 builder 版 `on_replace_all` 等价）。
    pub fn set_on_replace_all<F>(&mut self, handler: F)
    where
        F: Fn(String, String, &mut Window, &mut App) + 'static,
    {
        self.on_replace_all = Some(Rc::new(handler));
    }

    /// 推送待搜索全文并用当前查询重算匹配。
    pub fn set_source(&mut self, source: String, cx: &mut App) {
        let query = self.state.read(cx).query().to_string();
        self.source = source;
        let source = self.source.clone();
        self.state.update(cx, |state, cx| {
            state.set_query(query, &source);
            cx.notify();
        });
    }

    /// 绑定编辑器：一行接通“文本同步 + 匹配标黄 + 跳转导航”。
    ///
    /// 与 `v1_2_showcase --bin search` 的手工接线等价：
    /// - 编辑器文本变化 → 自动 `set_source` 推送全文；
    /// - 匹配变化 → 内部标黄器重标（默认黄底，可调 [`Self::set_highlight_colors`]）；
    /// - 导航回调未设置时给默认跳转（选区 + 只读滚动），已有自定义不覆盖。
    ///
    /// 替换回调（`on_replace`/`on_replace_all`）仍由调用方按需设置，
    /// 面板只负责搜索，替换执行权留给外部（文本归属不同，框架不代劳）。
    pub fn attach_editor(&mut self, editor: &Entity<InputState>, cx: &mut Context<Self>) {
        self.attached_editor = Some(editor.clone());
        // 默认跳转：已有自定义导航回调则保留。
        if self.on_navigate.is_none() {
            let editor_handle = editor.clone();
            self.on_navigate = Some(Rc::new(move |line, start, end, _, cx| {
                let full = editor_handle.read_with(cx, |state, _| state.text().to_string());
                let start = byte_offset_of(&full, line, start);
                let end = byte_offset_of(&full, line, end).max(start);
                editor_handle.update(cx, |state, cx| {
                    state.set_selected_range(start..end, cx);
                    state.reveal_offset(start, cx);
                });
            }));
        }
        // 文本一改就同步 source（否则匹配/标黄按旧文本算，全错位）。
        let editor_handle = editor.clone();
        cx.subscribe(editor, move |this, _editor, event, cx| {
            if !matches!(event, crate::input_ui::InputEvent::Change) {
                return;
            }
            let full = editor_handle.read_with(cx, |state, _| state.text().to_string());
            this.set_source(full, cx);
        })
        .detach();
        // 查询/匹配一变就重标。
        cx.observe(&self.state.clone(), |this, _, cx| {
            this.mark_attached(cx);
        })
        .detach();
        // 初始全文 + 初始标黄。
        let full = editor.read_with(cx, |state, _| state.text().to_string());
        self.set_source(full, cx);
        self.mark_attached(cx);
    }

    /// 设置标黄配色（绑定编辑器后调，下次重标生效并立即重标一次）。
    pub fn set_highlight_colors(
        &mut self,
        background: Hsla,
        foreground: Option<Hsla>,
        cx: &mut App,
    ) {
        self.highlight
            .get_or_insert_with(SearchHighlight::new)
            .set_colors(background, foreground);
        self.mark_attached(cx);
    }

    /// 聚焦搜索输入框（`Ctrl+F` 接线用：一行调用）。
    pub fn focus_search_input(&self, window: &mut Window, cx: &mut App) {
        self.search_input.update(cx, |state, cx| {
            state.focus(window, cx);
        });
    }

    /// 设置匹配导航回调（构建后追加/覆盖，与 builder 版 `on_replace` 类似）。
    pub fn set_on_navigate<F>(&mut self, handler: F)
    where
        F: Fn(usize, usize, usize, &mut Window, &mut App) + 'static,
    {
        self.on_navigate = Some(Rc::new(handler));
    }

    /// 设置是否显示替换区域（`Ctrl+F` 关/`Ctrl+R` 开，弹窗模式用）。
    pub fn set_show_replace(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_replace = show;
        // 无替换输入框的 `search_only` 构造下强制隐藏（字段与构造一致）。
        if self.replace_input.is_none() {
            self.show_replace = false;
        }
        cx.notify();
    }

    /// 是否显示替换区域。
    pub fn show_replace(&self) -> bool {
        self.show_replace
    }

    /// 按当前匹配重标绑定的编辑器（无绑定时空操作）。
    fn mark_attached(&mut self, cx: &mut App) {
        let Some(editor) = self.attached_editor.clone() else {
            return;
        };
        let matches = self.state.read(cx).matches().to_vec();
        let source = self.source.clone();
        self.highlight
            .get_or_insert_with(SearchHighlight::new)
            .mark(&editor, &source, &matches, cx);
    }

    /// 搜索输入框实体（供父组件聚焦等）。
    pub fn search_input(&self) -> &Entity<InputState> {
        &self.search_input
    }

    /// 搜索状态实体（供父组件读取匹配）。
    pub fn state(&self) -> &Entity<SearchState> {
        &self.state
    }
    /// 用当前存的全文重算匹配。
    fn update_search(&mut self, query: &str, cx: &mut App) {
        let source = self.source.clone();
        self.state.update(cx, |state, cx| {
            state.set_query(query.to_string(), &source);
            cx.notify();
        });
    }

    /// 导航到下一个匹配（回调延后到 render 里触发，需要 Window）。
    fn navigate_next(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.next_match();
            cx.notify();
        });
        self.pending_navigate = true;
        cx.notify();
    }

    /// 导航到上一个匹配（回调延后到 render 里触发，需要 Window）。
    fn navigate_prev(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.prev_match();
            cx.notify();
        });
        self.pending_navigate = true;
        cx.notify();
    }

    /// 替换当前匹配：记延后标记，render 里拿 Window 触发 `on_replace`，实际替换由外部处理。
    fn replace_current(&mut self, cx: &mut Context<Self>) {
        self.pending_replace = true;
        cx.notify();
    }
}

impl Styled for SearchPanelState {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Focusable for SearchPanelState {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SearchPanelState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 延后的导航回调：有 Window 后真正触发。
        if self.pending_navigate {
            self.pending_navigate = false;
            if let (Some(cb), Some(m)) = (
                self.on_navigate.clone(),
                self.state.read(cx).current_match().cloned(),
            ) {
                cb(m.line, m.start_col, m.end_col, window, cx);
            }
        }

        // 延后的替换回调：替换框回车时无 Window，在此触发。
        if self.pending_replace {
            self.pending_replace = false;
            let (query, replacement) = self.state.read_with(cx, |state, _| {
                (state.query().to_string(), state.replacement().to_string())
            });
            if let Some(ref cb) = self.on_replace.clone() {
                cb(query, replacement, window, cx);
            }
        }

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

        let panel = cx.entity();
        let state_entity = self.state.clone();
        let search_input = self.search_input.clone();
        let replace_input = self.replace_input.clone();
        let on_replace = self.on_replace.clone();
        let on_replace_all = self.on_replace_all.clone();
        let source = self.source.clone();

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
                    // 上一个（走实体方法，顺带置位导航延后标记）
                    .child({
                        let navigate = panel.clone();
                        Button::new("prev-match")
                            .ghost()
                            .small()
                            .icon(IconName::ChevronUp)
                            .disabled(!has_matches)
                            .on_click(move |_, _, cx| {
                                navigate.update(cx, |this, cx| this.navigate_prev(cx));
                            })
                    })
                    // 下一个（走实体方法，顺带置位导航延后标记）
                    .child({
                        Button::new("next-match")
                            .ghost()
                            .small()
                            .icon(IconName::ChevronDown)
                            .disabled(!has_matches)
                            .on_click(move |_, _, cx| {
                                panel.update(cx, |this, cx| this.navigate_next(cx));
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
                            let on_replace = on_replace.clone();
                            let state_entity = state_entity.clone();
                            Button::new("replace-current")
                                .ghost()
                                .small()
                                .label("Replace")
                                .disabled(!has_matches)
                                .on_click(move |_, window, cx| {
                                    let (query, replacement) =
                                        state_entity.read_with(cx, |state, _| {
                                            (
                                                state.query().to_string(),
                                                state.replacement().to_string(),
                                            )
                                        });
                                    if let Some(ref cb) = on_replace {
                                        cb(query, replacement, window, cx);
                                    }
                                })
                        })
                        // 全部替换
                        .child({
                            let on_replace_all = on_replace_all.clone();
                            let state_entity = state_entity.clone();
                            Button::new("replace-all")
                                .ghost()
                                .small()
                                .label("All")
                                .disabled(!has_matches)
                                .on_click(move |_, window, cx| {
                                    let (query, replacement) =
                                        state_entity.read_with(cx, |state, _| {
                                            (
                                                state.query().to_string(),
                                                state.replacement().to_string(),
                                            )
                                        });
                                    if let Some(ref cb) = on_replace_all {
                                        cb(query, replacement, window, cx);
                                    }
                                })
                        }),
                )
            })
            .child(
                // 选项行（用存的全文重算，不再用空字符串占位）
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
                        let source = source.clone();
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
                        let source = source.clone();
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

#[cfg(test)]
mod tests {
    // 注意：不用 `use super::*`——父模块的 `use crate::*` 会把根导出的 `test`
    // 宏带进来，使展开后的裸 `#[test]` 解析到自身而无限递归。
    use super::{InputState, SearchPanelState};
    use crate::AppContext as _;
    use crate::{Context, Entity, IntoElement, Render, Window, div};

    /// 持有编辑器 + 搜索面板的测试宿主视图。
    struct Probe {
        text: Entity<InputState>,
        panel: Entity<SearchPanelState>,
    }

    impl Render for Probe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    /// 在搜索框输入查询（触发匹配重算 + 标黄）。
    fn type_query(
        panel: &Entity<SearchPanelState>,
        query: &str,
        cx: &mut crate::VisualTestContext,
    ) {
        cx.update(|window, cx| {
            let input = panel.read(cx).search_input().clone();
            input.update(cx, |state, cx| state.replace(query, window, cx));
        });
    }

    /// 替换行显隐可切换（`Ctrl+F` 关/`Ctrl+R` 开；`search_only` 构造恒关）。
    #[rgpui::test]
    fn show_replace_toggles(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let panel = cx.new(|cx| SearchPanelState::new(window, cx));
            let replace_only = cx.new(|cx| SearchPanelState::search_only(window, cx));
            ProbeReplace {
                panel,
                replace_only,
            }
        });
        let (panel, replace_only) = probe.read_with(cx, |probe, _| {
            (probe.panel.clone(), probe.replace_only.clone())
        });
        // `new` 构造默认显示替换行。
        assert!(panel.read_with(cx, |panel, _| panel.show_replace()));
        cx.update(|_, cx| {
            panel.update(cx, |panel, cx| {
                panel.set_show_replace(false, cx);
            });
        });
        assert!(!panel.read_with(cx, |panel, _| panel.show_replace()));
        // `search_only` 构造恒关（置 true 也压回 false）。
        assert!(!replace_only.read_with(cx, |panel, _| panel.show_replace()));
        cx.update(|_, cx| {
            replace_only.update(cx, |panel, cx| {
                panel.set_show_replace(true, cx);
            });
        });
        assert!(!replace_only.read_with(cx, |panel, _| panel.show_replace()));
    }

    /// 持有替换行显隐测试的面板视图。
    struct ProbeReplace {
        panel: Entity<SearchPanelState>,
        replace_only: Entity<SearchPanelState>,
    }

    impl Render for ProbeReplace {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    /// 读取（匹配数，标黄数）。
    fn match_stats(
        panel: &Entity<SearchPanelState>,
        cx: &mut crate::VisualTestContext,
    ) -> (usize, usize) {
        cx.update(|_, cx| {
            let panel_ref = panel.read(cx);
            let count = panel_ref.state().read(cx).match_count();
            let highlighted = panel_ref
                .highlight
                .as_ref()
                .and_then(|highlight| highlight.collection.as_ref())
                .map(|collection| collection.get_ranges(cx).len())
                .unwrap_or(0);
            (count, highlighted)
        })
    }

    #[rgpui::test]
    fn attach_editor_syncs_source_and_highlights(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let text = cx.new(|cx| {
                let mut state = InputState::new(window, cx).multi_line(true);
                state.replace("hello world\nhello rgpui", window, cx);
                state
            });
            let panel = cx.new(|cx| SearchPanelState::new(window, cx));
            panel.update(cx, |panel, cx| panel.attach_editor(&text, cx));
            Probe { text, panel }
        });
        let (text, panel) =
            probe.read_with(cx, |probe, _| (probe.text.clone(), probe.panel.clone()));
        // 在搜索框输入查询：匹配数 2，标黄 2 处。
        type_query(&panel, "hello", cx);
        assert_eq!(match_stats(&panel, cx), (2, 2));
        // 文本变化自动同步 source：改后匹配数跟上。
        cx.update(|window, cx| {
            text.update(cx, |state, cx| {
                state.replace_all("hello hello hello", window, cx)
            });
        });
        assert_eq!(match_stats(&panel, cx), (3, 3));
        // 默认导航：直接调回调，光标跳到第一处匹配。
        cx.update(|window, cx| {
            let navigate = panel
                .read(cx)
                .on_navigate
                .clone()
                .expect("attach 后应有默认导航");
            navigate(0, 0, 5, window, cx);
        });
        assert_eq!(text.read_with(cx, |state, _| state.selected_range()), 0..5);
    }
}
