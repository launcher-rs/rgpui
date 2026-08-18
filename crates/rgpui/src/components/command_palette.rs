//! 命令面板组件：带模糊搜索的命令列表，支持键盘导航与快捷执行。

use crate::{
    input_ui::{Input, InputEvent, InputState},
    prelude::FluentBuilder as _,
    *,
};
use std::rc::Rc;

actions!(
    command_palette,
    [NavigateUp, NavigateDown, SelectCommand, CloseCommand]
);

/// 单条命令。
#[derive(Clone)]
pub struct Command {
    /// 命令 ID。
    pub id: SharedString,
    /// 命令名称。
    pub name: SharedString,
    /// 命令描述。
    pub description: Option<SharedString>,
    /// 命令图标。
    pub icon: Option<IconName>,
    /// 命令分类。
    pub category: Option<SharedString>,
    /// 快捷键提示文本。
    pub shortcut: Option<SharedString>,
    /// 选中执行的回调。
    pub on_select: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    /// 搜索文本（小写，名称 + 描述）。
    search_text: String,
}

impl Command {
    /// 创建命令。
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        let id = id.into();
        let name = name.into();
        let search_text = name.to_string().to_lowercase();

        Self {
            id,
            name,
            description: None,
            icon: None,
            category: None,
            shortcut: None,
            on_select: None,
            search_text,
        }
    }

    /// 设置命令描述（描述也会参与搜索匹配）。
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        let desc = description.into();
        self.search_text = format!("{} {}", self.name, desc).to_lowercase();
        self.description = Some(desc);
        self
    }

    /// 设置命令图标。
    pub fn icon(mut self, icon: impl Into<IconName>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 设置命令分类。
    pub fn category(mut self, category: impl Into<SharedString>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// 设置快捷键提示文本。
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// 设置选中执行的回调。
    pub fn on_select<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// 判断命令是否匹配查询。
    pub fn matches(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }

        let query = query.to_lowercase();
        self.search_text.contains(&query)
    }

    /// 计算匹配得分（用于排序，分值越高越靠前）。
    pub fn match_score(&self, query: &str) -> i32 {
        if query.is_empty() {
            return 0;
        }

        let query = query.to_lowercase();
        let name_lower = self.name.to_string().to_lowercase();

        if name_lower == query {
            return 1000;
        }

        if name_lower.starts_with(&query) {
            return 500;
        }

        if name_lower.contains(&query) {
            return 100;
        }

        if self.search_text.contains(&query) {
            return 50;
        }

        0
    }
}

/// 命令面板状态。
pub struct CommandPaletteState {
    /// 全部命令。
    commands: Vec<Command>,
    /// 当前搜索关键字。
    search_query: String,
    /// 过滤后的命令列表。
    filtered_commands: Vec<Command>,
    /// 当前选中索引。
    selected_index: usize,
    /// 最近执行的命令 ID 列表（最多保留 10 条）。
    recent_commands: Vec<SharedString>,
}

impl CommandPaletteState {
    /// 创建状态。
    pub fn new(commands: Vec<Command>) -> Self {
        let filtered_commands = commands.clone();

        Self {
            commands,
            search_query: String::new(),
            filtered_commands,
            selected_index: 0,
            recent_commands: Vec::new(),
        }
    }

    /// 更新搜索关键字并重新过滤命令。
    pub fn update_search(&mut self, query: String) {
        self.search_query = query.clone();

        if query.is_empty() {
            self.filtered_commands = self.commands.clone();
        } else {
            let mut matches: Vec<(Command, i32)> = self
                .commands
                .iter()
                .filter(|cmd| cmd.matches(&query))
                .map(|cmd| (cmd.clone(), cmd.match_score(&query)))
                .collect();

            matches.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
            self.filtered_commands = matches.into_iter().map(|(cmd, _)| cmd).collect();
        }

        self.selected_index = 0;
    }

    /// 选择上一条命令。
    pub fn select_previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// 选择下一条命令。
    pub fn select_next(&mut self) {
        if self.selected_index < self.filtered_commands.len().saturating_sub(1) {
            self.selected_index += 1;
        }
    }

    /// 执行当前选中的命令，并记录到最近命令。
    pub fn execute_selected(&mut self, window: &mut Window, cx: &mut App) -> bool {
        if let Some(command) = self.filtered_commands.get(self.selected_index) {
            if let Some(handler) = &command.on_select {
                handler(window, cx);
                self.recent_commands.push(command.id.clone());
                if self.recent_commands.len() > 10 {
                    self.recent_commands.remove(0);
                }
                return true;
            }
        }
        false
    }

    /// 获取过滤后的命令列表。
    pub fn filtered_commands(&self) -> &[Command] {
        &self.filtered_commands
    }

    /// 获取当前选中索引。
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }
}

/// 命令面板组件。
#[derive(IntoElement)]
pub struct CommandPalette {
    /// 命令状态实体。
    state: Entity<CommandPaletteState>,
    /// 搜索输入框状态实体。
    search_input: Entity<InputState>,
    /// 关闭回调。
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    /// 焦点句柄。
    focus_handle: FocusHandle,
    /// 用户样式。
    style: StyleRefinement,
}

impl CommandPalette {
    /// 创建命令面板。
    pub fn new(window: &mut Window, cx: &mut Context<Self>, commands: Vec<Command>) -> Self {
        let state = cx.new(|_| CommandPaletteState::new(commands));
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Type a command or search..."));
        let focus_handle = cx.focus_handle();

        // 订阅搜索输入框的变更事件，实时过滤命令。
        cx.subscribe(&search_input, |this, _input, event, cx| match event {
            InputEvent::Change => {
                let query = this.search_input.read(cx).text().to_string();
                this.state.update(cx, |state, cx| {
                    state.update_search(query);
                    cx.notify();
                });
            }
            _ => {}
        })
        .detach();

        Self {
            state,
            search_input,
            on_close: None,
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
}

impl Styled for CommandPalette {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Focusable for CommandPalette {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl RenderOnce for CommandPalette {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let radius_lg = theme.radius_lg;
        let font_size = theme.font_size;
        let popover = theme.tokens.popover;
        let border = theme.tokens.border;
        let muted = theme.tokens.muted;
        let muted_foreground = theme.tokens.muted_foreground;

        let state = self.state.read(cx);
        let filtered = state.filtered_commands();
        let selected_idx = state.selected_index();
        let user_style = self.style;
        let search_input = self.search_input;
        let focus_handle = self.focus_handle;

        let state_for_nav_up = self.state.clone();
        let state_for_nav_down = self.state.clone();
        let state_for_select = self.state.clone();
        let on_close_for_backdrop = self.on_close.clone();
        let on_close_for_select = self.on_close.clone();
        let on_close_for_esc = self.on_close.clone();

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(crate::rgba(0x00000088))
            .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                if let Some(handler) = &on_close_for_backdrop {
                    handler(window, cx);
                }
            })
            .on_scroll_wheel(|_, _, _| {})
            .key_context("CommandPalette")
            .track_focus(&focus_handle)
            .on_action(move |_: &NavigateUp, _window, cx| {
                state_for_nav_up.update(cx, |state, cx| {
                    state.select_previous();
                    cx.notify();
                });
            })
            .on_action(move |_: &NavigateDown, _window, cx| {
                state_for_nav_down.update(cx, |state, cx| {
                    state.select_next();
                    cx.notify();
                });
            })
            .on_action(move |_: &SelectCommand, window, cx| {
                let executed = state_for_select
                    .update(cx, |state, app_cx| state.execute_selected(window, app_cx));
                if executed {
                    if let Some(handler) = &on_close_for_select {
                        handler(window, cx);
                    }
                }
            })
            .on_action(move |_: &CloseCommand, window, cx| {
                if let Some(handler) = &on_close_for_esc {
                    handler(window, cx);
                }
            })
            .child(
                div()
                    .w(px(600.0))
                    .max_h(px(500.0))
                    .flex()
                    .flex_col()
                    .bg(popover)
                    .border_1()
                    .border_color(border)
                    .rounded(radius_lg)
                    .shadow_lg()
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Left, |_event, _window, _cx| {})
                    .map(|this| {
                        let mut div = this;
                        div.style().refine(&user_style);
                        div
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .px(px(16.0))
                            .py(px(12.0))
                            .border_b_1()
                            .border_color(border)
                            .child(Input::new(&search_input)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .overflow_y_scrollbar()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .p(px(8.0))
                                    .children(filtered.is_empty().then(|| {
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .h(px(200.0))
                                            .child(
                                                div()
                                                    .text_size(font_size * 0.85)
                                                    .text_color(muted_foreground)
                                                    .child("No commands found"),
                                            )
                                            .into_any_element()
                                    }))
                                    .children(filtered.iter().enumerate().map(|(idx, command)| {
                                        let is_selected = idx == selected_idx;
                                        render_command_item(command.clone(), is_selected, theme)
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(16.0))
                            .py(px(8.0))
                            .border_t_1()
                            .border_color(border)
                            .bg(muted.opacity(0.3))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(16.0))
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(muted_foreground)
                                            .child("↑↓ Navigate"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(muted_foreground)
                                            .child("↵ Select"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(muted_foreground)
                                            .child("Esc Close"),
                                    ),
                            ),
                    ),
            )
    }
}

/// 渲染单条命令项。
fn render_command_item(command: Command, selected: bool, theme: &crate::Theme) -> impl IntoElement {
    let radius = theme.radius;
    let font_size = theme.font_size;
    let font_family = theme.font_family.clone();
    let accent = theme.tokens.accent;
    let accent_foreground = theme.tokens.accent_foreground;
    let foreground = theme.tokens.foreground;
    let muted = theme.tokens.muted;
    let muted_foreground = theme.tokens.muted_foreground;

    div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .px(px(12.0))
        .py(px(10.0))
        .rounded(radius)
        .cursor(CursorStyle::PointingHand)
        .when(selected, |div| div.bg(accent))
        .when(!selected, |div| div.hover(|style| style.bg(muted)))
        .when_some(command.on_select, |div, handler| {
            div.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                handler(window, cx);
            })
        })
        .when_some(command.icon, |div, icon| {
            div.child(Icon::new(icon).with_size(px(18.0)).text_color(if selected {
                accent_foreground
            } else {
                foreground
            }))
        })
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(font_size)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if selected {
                            accent_foreground
                        } else {
                            foreground
                        })
                        .font_family(font_family.clone())
                        .child(command.name),
                )
                .when_some(command.description, |d, desc| {
                    d.child(
                        div()
                            .text_size(font_size * 0.85)
                            .text_color(if selected {
                                accent_foreground.opacity(0.8)
                            } else {
                                muted_foreground.color
                            })
                            .font_family(font_family.clone())
                            .child(desc),
                    )
                }),
        )
        .children(command.shortcut.map(|shortcut| {
            div()
                .px(px(8.0))
                .py(px(4.0))
                .rounded(radius)
                .bg(if selected {
                    accent_foreground.opacity(0.2)
                } else {
                    muted.color
                })
                .child(
                    div()
                        .text_size(font_size * 0.85)
                        .text_color(if selected {
                            accent_foreground
                        } else {
                            muted_foreground
                        })
                        .font_family(font_family)
                        .child(shortcut),
                )
                .into_any_element()
        }))
}
