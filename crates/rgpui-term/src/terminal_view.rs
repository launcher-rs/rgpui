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
    App, ClipboardItem, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
    Render, ScrollWheelEvent, Styled, Window, actions, div, px,
};

use crate::{Event, Terminal, TerminalElement, TextStyle, ThemeManager};

const SCROLLBAR_WIDTH: f32 = 8.0;
const SCROLLBAR_THUMB_MIN_HEIGHT: f32 = 20.0;

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
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.clear_bell(cx);

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
        let content = &self.terminal.read(cx).last_content;
        let history_size = content.history_size;
        let visible_lines = content.terminal_bounds.num_lines();
        let track_height = f32::from(content.terminal_bounds.height());

        if history_size == 0 || track_height <= 0.0 {
            return;
        }

        let total_lines = history_size + visible_lines;
        let thumb_height = (visible_lines as f32 / total_lines as f32 * track_height)
            .max(SCROLLBAR_THUMB_MIN_HEIGHT);
        let scrollable_track = track_height - thumb_height;

        if scrollable_track <= 0.0 {
            return;
        }

        let dy = f32::from(event.position.y - self.drag_start_y);
        let ratio = dy / scrollable_track;
        // Moving thumb down means scrolling towards bottom (decreasing offset)
        let target_offset =
            (self.drag_start_offset as f32 - ratio * history_size as f32).round() as i32;
        let target_offset = target_offset.clamp(0, history_size as i32) as usize;

        self.terminal.update(cx, |terminal, _| {
            terminal.scroll_to_offset(target_offset);
        });
        cx.notify();
    }

    fn on_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.terminal.update(cx, |terminal, _| {
            terminal.scroll_wheel(event, 1.0);
        });
        cx.notify();
    }

    fn copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
        self.terminal.update(cx, |terminal, _| {
            terminal.copy(Some(true));
        });

        if let Some(text) = self.terminal.read(cx).last_content.selection_text.clone() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
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
