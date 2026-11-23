//! Terminal view component for rgpui-term.
//!
//! This module provides the TerminalView struct which is the main rgpui entity
//! that handles input events and renders the terminal using TerminalElement.
//!
//! # Architecture
//!
//! TerminalView acts as the glue between rgpui's event system and the Terminal entity.
//! It:
//! - Receives keyboard events and forwards them to the terminal via try_keystroke
//! - Handles mouse events (down, up, move, scroll) for selection and mouse reporting
//! - Provides copy/paste/clear/select_all actions bound to keyboard shortcuts
//! - Subscribes to terminal events (Wakeup, Bell, TitleChanged, etc.) to update UI
//!
//! # Example
//!
//! ```ignore
//! let terminal = TerminalBuilder::new(...)?.subscribe(cx);
//! let terminal_entity = cx.new(|_| terminal);
//! let view = cx.new(|cx| TerminalView::new(terminal_entity, cx));
//! ```

use rgpui::{
    App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Render,
    ScrollWheelEvent, Styled, Window, actions, div, px,
};

use crate::{Event, Terminal, TerminalElement, TextStyle, ThemeManager};

const SCROLLBAR_WIDTH: f32 = 8.0;
const SCROLLBAR_THUMB_MIN_HEIGHT: f32 = 20.0;

/// Convert pointer movement into a scroll offset using the geometry captured when
/// the thumb was grabbed, so incoming output cannot change the drag sensitivity.
fn scrollbar_drag_offset(
    start_offset: usize,
    history_size: usize,
    dy: f32,
    scrollable_track: f32,
) -> f32 {
    if history_size == 0 || scrollable_track <= 0.0 {
        return 0.0;
    }

    // 保留小数行：整数部分驱动 alacritty 的 display_offset，
    // 小数部分交给渲染层做亚像素平移。若在这里 round 成整数，
    // 鼠标移动不足「一行对应的像素」时画面完全不动，跨过门槛又跳一行——
    // 历史很长时一行 = 十几像素，正是「停住一下再跳」的由来。
    let target = start_offset as f64 - (dy as f64 / scrollable_track as f64) * history_size as f64;
    target.clamp(0.0, history_size as f64) as f32
}

actions!(
    terminal,
    [
        Copy,
        Paste,
        Clear,
        SelectAll,
        ScrollLineUp,
        ScrollLineDown,
        ScrollPageUp,
        ScrollPageDown,
        SearchNext,
        SearchPrevious,
        ClearSearch,
        ChangeTheme
    ]
);

/// Main terminal view component that handles input and coordinates rendering.
///
/// This struct wraps a Terminal entity and provides:
/// - Focus management for keyboard input routing
/// - Event handlers for keyboard and mouse input
/// - Action handlers for clipboard operations and scrolling
/// - Subscription to terminal events for UI updates
pub struct TerminalView {
    terminal: Entity<Terminal>,
    focus_handle: FocusHandle,
    has_bell: bool,
    text_style: crate::TextStyle,
    is_dragging_scrollbar: bool,
    scrollbar_hovered: bool,
    drag_start_y: Pixels,
    drag_start_offset: usize,
    drag_history_size: usize,
    drag_scrollable_track: f32,
    drag_last_offset: f32,
}

impl TerminalView {
    /// Creates a new TerminalView wrapping the given Terminal entity.
    ///
    /// Sets up event subscriptions and focus handling for the terminal.
    pub fn new(terminal: Entity<Terminal>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_with_style(terminal, crate::TextStyle::default(), window, cx)
    }

    /// Creates a new TerminalView with a custom text style.
    pub fn new_with_style(
        terminal: Entity<Terminal>,
        text_style: crate::TextStyle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();

        cx.subscribe(&terminal, |this, _, event: &Event, cx| {
            this.handle_terminal_event(event, cx);
        })
        .detach();

        cx.on_focus_in(&focus_handle, window, |this: &mut Self, _window, cx| {
            this.terminal.update(cx, |terminal, _| {
                terminal.focus_in();
            });
        })
        .detach();

        cx.on_focus_out(
            &focus_handle,
            window,
            |this: &mut Self, _event, _window, cx| {
                this.terminal.update(cx, |terminal, _| {
                    terminal.focus_out();
                });
            },
        )
        .detach();

        Self {
            terminal,
            focus_handle,
            has_bell: false,
            text_style,
            is_dragging_scrollbar: false,
            scrollbar_hovered: false,
            drag_start_y: px(0.),
            drag_start_offset: 0,
            drag_history_size: 0,
            drag_scrollable_track: 0.0,
            drag_last_offset: 0.0,
        }
    }

    /// Returns a reference to the underlying Terminal entity.
    pub fn terminal(&self) -> &Entity<Terminal> {
        &self.terminal
    }

    /// Returns whether the terminal bell has sounded since last cleared.
    pub fn has_bell(&self) -> bool {
        self.has_bell
    }

    /// Clears the bell indicator.
    pub fn clear_bell(&mut self, cx: &mut Context<Self>) {
        self.has_bell = false;
        cx.notify();
    }

    /// Updates the theme by applying a new theme from the ThemeManager.
    ///
    /// This updates the text_style with the new theme colors while preserving
    /// font settings.
    pub fn set_theme(&mut self, theme_name: &str, cx: &mut Context<Self>) {
        if let Some(theme_manager) = cx.try_global::<ThemeManager>()
            && let Some(theme) = theme_manager.get_theme(theme_name)
        {
            self.text_style.theme = theme.clone();
            self.text_style.foreground = theme.foreground;
            self.text_style.background = theme.background;
            cx.notify();
        }
    }

    /// Returns a mutable reference to the text style for external updates.
    pub fn text_style_mut(&mut self) -> &mut TextStyle {
        &mut self.text_style
    }

    fn handle_terminal_event(&mut self, event: &Event, cx: &mut Context<Self>) {
        match event {
            Event::Wakeup => cx.notify(),
            Event::Bell => {
                self.has_bell = true;
                cx.notify();
            }
            Event::TitleChanged => cx.notify(),
            Event::BlinkChanged(_) => cx.notify(),
            Event::SelectionsChanged => cx.notify(),
            Event::CloseTerminal => {
                cx.notify();
            }
            // 超链接点击由业务层订阅 Terminal 事件处理（如打开浏览器），视图无需重绘
            Event::HyperlinkClick(_) => {}
            // 搜索状态变化要重绘（高亮可能新增或消失）；具体状态由业务层
            // 订阅 Terminal 的 SearchUpdated 事件或查询 search_status
            Event::SearchUpdated(_) => cx.notify(),
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.clear_bell(cx);

        // IME 组合期间按键归输入法（候选上下翻、确认、取消），不能直送 PTY；
        // 上屏文本由 `TerminalInputHandler::replace_text_in_range` 写入。
        if self.terminal.read(cx).ime_composition().is_some() {
            return;
        }

        let handled = self.terminal.update(cx, |terminal, _| {
            terminal.try_keystroke(&event.keystroke, false)
        });

        if handled {
            cx.stop_propagation();
        }
    }

    fn is_mouse_in_scrollbar(
        &self,
        position: &rgpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let content = &self.terminal.read(cx).last_content;
        if content.history_size == 0 {
            return false;
        }
        let bounds = content.terminal_bounds.bounds;
        let scrollbar_left = bounds.origin.x + bounds.size.width - px(SCROLLBAR_WIDTH);
        position.x >= scrollbar_left
            && position.x <= bounds.origin.x + bounds.size.width
            && position.y >= bounds.origin.y
            && position.y <= bounds.origin.y + bounds.size.height
    }

    fn is_in_scrollbar_area(&self, event: &MouseDownEvent, cx: &mut Context<Self>) -> bool {
        self.is_mouse_in_scrollbar(&event.position, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 外部松开兜底在 TerminalElement 的 paint 阶段清除了解析标志；若松开
        // 发生在终端外，这里在下一次按下时同步清理视图侧的拖动状态。
        self.is_dragging_scrollbar = false;
        self.terminal
            .update(cx, |terminal, _| terminal.end_scrollbar_drag());

        window.focus(&self.focus_handle, cx);
        window.prevent_default();

        if event.button == MouseButton::Right {
            let mouse_mode = self.terminal.read(cx).mouse_mode(event.modifiers.shift);
            if !mouse_mode {
                if let Some(item) = cx.read_from_clipboard()
                    && let Some(text) = item.text()
                {
                    self.terminal.update(cx, |terminal, _| {
                        terminal.paste(&text);
                    });
                }
                cx.notify();
                return;
            }
        }

        // Check if click is in the scrollbar area
        if event.button == MouseButton::Left && self.is_in_scrollbar_area(event, cx) {
            cx.stop_propagation();
            self.handle_scrollbar_click(event, cx);
            return;
        }

        self.terminal.update(cx, |terminal, cx| {
            terminal.mouse_down(event, cx);
        });
        cx.notify();
    }

    fn handle_scrollbar_click(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let content = &self.terminal.read(cx).last_content;
        let history_size = content.history_size;
        let visible_lines = content.terminal_bounds.num_lines();
        let display_offset = content.display_offset;
        let track_height = f32::from(content.terminal_bounds.height());

        if history_size == 0 || track_height <= 0.0 {
            return;
        }

        let total_lines = history_size + visible_lines;
        let thumb_height = (visible_lines as f32 / total_lines as f32 * track_height)
            .max(SCROLLBAR_THUMB_MIN_HEIGHT);
        let scrollable_track = track_height - thumb_height;
        let thumb_top = if history_size > 0 {
            (1.0 - display_offset as f32 / history_size as f32) * scrollable_track
        } else {
            0.0
        };

        let click_y = f32::from(event.position.y - content.terminal_bounds.bounds.origin.y);

        // If click is on the thumb, start dragging
        if click_y >= thumb_top && click_y <= thumb_top + thumb_height {
            self.is_dragging_scrollbar = true;
            self.drag_start_y = event.position.y;
            self.drag_start_offset = display_offset;
            self.drag_history_size = history_size;
            self.drag_scrollable_track = scrollable_track;
            self.drag_last_offset = display_offset as f32;
            // 拖动期间让解析线程暂停：拖动帧要独占数据锁重排网格，
            // 否则和刷屏解析反复抢锁，表现成「动一段、卡一段」。
            // 同时把几何存进 Terminal，供窗口级鼠标兜底在界外继续驱动。
            self.terminal.update(cx, |terminal, _| {
                terminal.set_ui_dragging(true);
                terminal.begin_scrollbar_drag(crate::ScrollbarDragGeometry {
                    start_y: event.position.y,
                    start_offset: display_offset,
                    history_size,
                    scrollable_track,
                });
            });
        } else {
            // Click on track: scroll page up/down
            let thumb_center = thumb_top + thumb_height / 2.0;
            if click_y < thumb_center {
                self.terminal.update(cx, |terminal, _| {
                    terminal.scroll_page_up();
                });
            } else {
                self.terminal.update(cx, |terminal, _| {
                    terminal.scroll_page_down();
                });
            }
        }
        cx.notify();
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.is_dragging_scrollbar {
            self.is_dragging_scrollbar = false;
            self.terminal.update(cx, |terminal, _| {
                terminal.end_scrollbar_drag();
            });
            cx.notify();
            return;
        }

        self.terminal.update(cx, |terminal, cx| {
            terminal.mouse_up(event, cx);
        });
        cx.notify();
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let was_hovered = self.scrollbar_hovered;
        self.scrollbar_hovered = self.is_mouse_in_scrollbar(&event.position, cx);
        if was_hovered != self.scrollbar_hovered {
            cx.notify();
        }

        self.terminal.update(cx, |terminal, cx| {
            terminal.mouse_move(event, cx);
        });
    }

    fn on_mouse_drag(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_dragging_scrollbar {
            self.handle_scrollbar_drag(event, cx);
            return;
        }

        let bounds = self.terminal.read(cx).last_content.terminal_bounds.bounds;
        self.terminal.update(cx, |terminal, cx| {
            terminal.mouse_drag(event, bounds, cx);
        });
    }

    fn handle_scrollbar_drag(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.drag_history_size == 0 || self.drag_scrollable_track <= 0.0 {
            return;
        }

        let dy = f32::from(event.position.y - self.drag_start_y);
        // Moving thumb down means scrolling towards bottom (decreasing offset)
        let target_offset = scrollbar_drag_offset(
            self.drag_start_offset,
            self.drag_history_size,
            dy,
            self.drag_scrollable_track,
        );

        // 目标已是浮点行：亚像素变化也要响应，画面才能逐像素跟随。
        // 只吞掉完全没有变化的事件（含浮点），避免无谓重绘。
        let delta_rows = (target_offset - self.drag_last_offset).abs();
        if delta_rows < 0.001 {
            return;
        }
        self.drag_last_offset = target_offset;

        self.terminal.update(cx, |terminal, _| {
            terminal.scroll_to_offset_fractional(target_offset);
        });
        cx.notify();
    }

    fn on_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 滚轮连滚同样要和刷屏解析错开数据锁，否则一帧一帧地丢。
        self.terminal.read(cx).note_ui_scroll();
        self.terminal.update(cx, |terminal, _| {
            terminal.scroll_wheel(event, 1.0);
        });
        cx.notify();
    }

    fn copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
        // Terminal::copy 的 Copy 分支内部已把选区文本写入剪贴板，不再从
        // last_content.selection_text 取（那会导致每帧重算选区文本，拖选卡顿）。
        self.terminal.update(cx, |terminal, _| {
            terminal.copy(Some(true));
        });
        cx.notify();
    }

    fn paste(&mut self, _: &Paste, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(item) = cx.read_from_clipboard()
            && let Some(text) = item.text()
        {
            self.terminal.update(cx, |terminal, _| {
                terminal.paste(&text);
            });
        }
    }

    fn clear(&mut self, _: &Clear, _window: &mut Window, cx: &mut Context<Self>) {
        self.terminal.update(cx, |terminal, _| {
            terminal.clear();
        });
        cx.notify();
    }

    fn select_all(&mut self, _: &SelectAll, _window: &mut Window, cx: &mut Context<Self>) {
        self.terminal.update(cx, |terminal, _| {
            terminal.select_all();
        });
        cx.notify();
    }

    fn scroll_line_up(&mut self, _: &ScrollLineUp, _window: &mut Window, cx: &mut Context<Self>) {
        self.terminal.update(cx, |terminal, _| {
            terminal.scroll_line_up();
        });
        cx.notify();
    }

    fn scroll_line_down(
        &mut self,
        _: &ScrollLineDown,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.terminal.update(cx, |terminal, _| {
            terminal.scroll_line_down();
        });
        cx.notify();
    }

    fn scroll_page_up(&mut self, _: &ScrollPageUp, _window: &mut Window, cx: &mut Context<Self>) {
        self.terminal.update(cx, |terminal, _| {
            terminal.scroll_page_up();
        });
        cx.notify();
    }

    fn scroll_page_down(
        &mut self,
        _: &ScrollPageDown,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.terminal.update(cx, |terminal, _| {
            terminal.scroll_page_down();
        });
        cx.notify();
    }

    /// 跳到下一个搜索匹配（由 [`SearchNext`] 动作驱动）。
    fn search_next(&mut self, _: &SearchNext, _window: &mut Window, cx: &mut Context<Self>) {
        self.terminal.update(cx, |terminal, _| {
            terminal.search_next();
        });
        cx.notify();
    }

    /// 跳到上一个搜索匹配（由 [`SearchPrevious`] 动作驱动）。
    fn search_previous(
        &mut self,
        _: &SearchPrevious,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.terminal.update(cx, |terminal, _| {
            terminal.search_previous();
        });
        cx.notify();
    }

    /// 清除搜索高亮与状态（由 [`ClearSearch`] 动作驱动）。
    fn clear_search(&mut self, _: &ClearSearch, _window: &mut Window, cx: &mut Context<Self>) {
        self.terminal.update(cx, |terminal, _| {
            terminal.clear_search();
        });
        cx.notify();
    }

    /// 按正则设置搜索关键词（转给内部 [`Terminal`] 实体，排队到下次 `sync` 处理）。
    ///
    /// 不在此处 notify：入队后 `sync` 会 emit [`Event::Wakeup`]，视图靠该事件刷新。
    pub fn search(&self, query: &str, cx: &mut App) {
        self.terminal.update(cx, |terminal, _| {
            terminal.search(query);
        });
    }

    /// 按字面量设置搜索关键词：元字符被转义，所见即所搜。
    ///
    /// 搜索条这类直接吃用户输入的入口应当用它，避免 `.` `(` `[` 被当成正则。
    pub fn search_literal(&self, query: &str, cx: &mut App) {
        self.terminal.update(cx, |terminal, _| {
            terminal.search_literal(query);
        });
    }

    /// 当前搜索匹配状态（非法 / 无匹配 / 命中）。
    ///
    /// 搜索是排队执行的，状态在 [`Terminal::sync`] 之后才准确；
    /// 实时跟进请订阅 [`Event::SearchUpdated`]。
    pub fn search_status(&self, cx: &App) -> crate::SearchStatus {
        self.terminal
            .read_with(cx, |terminal, _| terminal.search_status())
    }

    /// Synchronizes terminal state with the window for rendering.
    ///
    /// Should be called before rendering to ensure the terminal content is up to date.
    pub fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.terminal.update(cx, |terminal, cx| {
            terminal.sync(window, cx);
        });
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 鼠标按住期间即使短时没有新位移事件，也继续提交下一帧；真实拖动中
        // 平台可能合并/暂缓指针事件，单靠事件触发会让视口长时间不刷新。
        // Paint 阶段的窗口级 mouse-up 兜底会清掉共享标志，随后动画自然停止。
        if self.is_dragging_scrollbar {
            if self.terminal.read(cx).is_ui_dragging() {
                window.request_animation_frame();
            } else {
                self.is_dragging_scrollbar = false;
            }
        }

        let terminal = self.terminal.clone();
        let focus_handle = self.focus_handle.clone();
        let is_focused = self.focus_handle.is_focused(window);

        div()
            .id("terminal-view")
            .size_full()
            // Transparent background to allow blur effect from parent window
            .bg(rgpui::transparent_black())
            .track_focus(&focus_handle)
            .key_context("Terminal")
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if event.dragging() {
                    this.on_mouse_drag(event, window, cx);
                } else {
                    this.on_mouse_move(event, window, cx);
                }
            }))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::clear))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::scroll_line_up))
            .on_action(cx.listener(Self::scroll_line_down))
            .on_action(cx.listener(Self::scroll_page_up))
            .on_action(cx.listener(Self::scroll_page_down))
            .on_action(cx.listener(Self::search_next))
            .on_action(cx.listener(Self::search_previous))
            .on_action(cx.listener(Self::clear_search))
            .child(TerminalElement::new(
                terminal,
                focus_handle.clone(),
                is_focused,
                true,
                self.text_style.clone(),
                self.scrollbar_hovered || self.is_dragging_scrollbar,
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::scrollbar_drag_offset;

    #[test]
    fn scrollbar_drag_mapping_uses_fixed_history_geometry() {
        let start_offset = 300_000;
        let history_at_grab = 400_000;
        let scrollable_track = 600.0;

        // 半程向下应稳定映射到一半历史偏移；后续日志增长不能改动这一比例。
        assert_eq!(
            scrollbar_drag_offset(start_offset, history_at_grab, 300.0, scrollable_track,),
            100_000.0,
        );
        assert_eq!(
            scrollbar_drag_offset(start_offset, history_at_grab, 900.0, scrollable_track,),
            0.0,
        );
        assert_eq!(
            scrollbar_drag_offset(start_offset, history_at_grab, -900.0, scrollable_track,),
            history_at_grab as f32,
        );
    }

    #[test]
    fn scrollbar_drag_keeps_subline_precision() {
        // 拖动 1 像素应产生小数行的变化，而不是被 round 成整行吞掉。
        // 历史 10000 行、轨道 730px 时，1px ≈ 13.7 行——这里用同样的比例
        // 验证返回值确实带小数，供渲染层做亚像素平移。
        let start_offset = 0;
        let history_at_grab = 10_000;
        let scrollable_track = 730.0;
        let one_px = scrollbar_drag_offset(start_offset, history_at_grab, -1.0, scrollable_track);
        assert!(
            one_px > 0.0 && one_px.fract().abs() > 0.001,
            "1px 拖动应产生带小数的行偏移，实际 {one_px}"
        );
    }
}
