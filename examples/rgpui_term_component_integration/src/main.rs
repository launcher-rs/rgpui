//! gpui-component Theme Integration Example
//!
//! This example demonstrates how to integrate gpui-term with gpui-component's
//! theme system, showing automatic theme synchronization and switching.

mod theme_adapter;

use std::collections::HashMap;
use std::env;
use std::sync::Arc;

use rgpui::{
    App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyBinding,
    ParentElement, Render, SharedString, Styled, Window, WindowOptions, actions, div, prelude::*,
    px,
};

use rgpui_component::{
    ActiveTheme, Sizable, StyledExt, Theme as ComponentTheme, ThemeMode, ThemeRegistry,
    button::{Button, ButtonVariants},
    h_flex,
    scroll::ScrollableElement,
    v_flex,
};

use rgpui_term::{
    Clear, Copy, Event, InputOrigin, Paste, SelectAll, Terminal, TerminalBuilder, TerminalContent,
    TerminalMiddleware, TerminalView,
};

use rgpui_platform::application;
use theme_adapter::ComponentThemeExt;

actions!(component_integration, [Quit, ToggleDarkMode]);

// Simple logging middleware to show terminal I/O
struct SimpleLogger;

impl TerminalMiddleware for SimpleLogger {
    fn on_input(
        &self,
        input: std::borrow::Cow<'static, [u8]>,
        origin: InputOrigin,
    ) -> Option<std::borrow::Cow<'static, [u8]>> {
        log::debug!(
            "Terminal input ({:?}): {:?}",
            origin,
            String::from_utf8_lossy(&input)
        );
        Some(input)
    }

    fn on_event(&self, event: &Event) {
        log::debug!("Terminal event: {:?}", event);
    }

    fn on_output(&self, _content: &TerminalContent) {
        // Uncomment to see terminal output
        // log::debug!("Terminal output updated");
    }
}

// Platform-specific shell detection
fn platform_shell() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        env::var("COMSPEC")
            .ok()
            .or_else(|| Some("cmd.exe".to_string()))
    }

    #[cfg(not(target_os = "windows"))]
    {
        env::var("SHELL").ok().or_else(|| {
            if std::path::Path::new("/bin/zsh").exists() {
                Some("/bin/zsh".to_string())
            } else {
                Some("/bin/bash".to_string())
            }
        })
    }
}

// Keybindings
fn keybindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("ctrl-shift-c", Copy, Some("Terminal")),
        KeyBinding::new("ctrl-shift-v", Paste, Some("Terminal")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-q", Quit, None),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-c", Copy, Some("Terminal")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-v", Paste, Some("Terminal")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-a", SelectAll, Some("Terminal")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-k", Clear, Some("Terminal")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-d", ToggleDarkMode, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-q", Quit, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-a", SelectAll, Some("Terminal")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-k", Clear, Some("Terminal")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-d", ToggleDarkMode, None),
    ]
}

struct ComponentIntegrationApp {
    terminal: Option<Entity<Terminal>>,
    terminal_view: Option<Entity<TerminalView>>,
    focus_handle: FocusHandle,
    current_theme: SharedString,
}

impl ComponentIntegrationApp {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            terminal: None,
            terminal_view: None,
            focus_handle: cx.focus_handle(),
            current_theme: "One Dark".into(),
        }
    }

    fn set_terminal(
        &mut self,
        terminal: Entity<Terminal>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Create terminal view with gpui-component theme
        let terminal_view = cx.new(|cx| {
            let mut view = TerminalView::new(terminal.clone(), window, cx);

            // ⭐ Apply gpui-component theme and set up auto-sync
            view.apply_component_theme(cx);
            view.observe_component_theme(cx);

            view
        });

        let focus_handle = terminal_view.read(cx).focus_handle(cx);
        focus_handle.focus(window, cx);

        self.terminal = Some(terminal);
        self.terminal_view = Some(terminal_view);
        cx.notify();
    }

    fn toggle_dark_mode(
        &mut self,
        _: &ToggleDarkMode,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current_mode = ComponentTheme::global(cx).mode;
        let new_mode = match current_mode {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
        };

        ComponentTheme::change(new_mode, None, cx);
        cx.notify();
    }

    fn switch_theme(&mut self, theme_name: SharedString, cx: &mut Context<Self>) {
        if let Some(theme_config) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
            ComponentTheme::global_mut(cx).apply_config(&theme_config);
            self.current_theme = theme_name;
            cx.notify();
        }
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        h_flex()
            .w_full()
            .h(px(60.0))
            .items_center()
            .justify_between()
            .px_6()
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.background)
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .text_xl()
                            .font_semibold()
                            .text_color(theme.foreground)
                            .child("gpui-component Integration Demo"),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(theme.accent)
                            .text_xs()
                            .text_color(theme.accent_foreground)
                            .child(format!("Mode: {:?}", theme.mode)),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("toggle-mode")
                            .primary()
                            .small()
                            .label(if theme.mode == ThemeMode::Dark {
                                "Switch to Light"
                            } else {
                                "Switch to Dark"
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_dark_mode(&ToggleDarkMode, window, cx);
                            })),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(format!("Current: {}", self.current_theme)),
                    ),
            )
    }

    fn render_theme_palette(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let available_themes = ThemeRegistry::global(cx).sorted_themes();

        v_flex()
            .w(px(250.0))
            .h_full()
            .border_r_1()
            .border_color(theme.border)
            .bg(theme.background)
            .child(
                div()
                    .w_full()
                    .h(px(50.0))
                    .flex()
                    .items_center()
                    .px_4()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(theme.foreground)
                            .child(format!("Themes ({})", available_themes.len())),
                    ),
            )
            .child(div().flex_1().overflow_y_scrollbar().child({
                let mut list = v_flex().gap_1().p_2();

                for (idx, theme_config) in available_themes.iter().enumerate() {
                    let theme_name: SharedString = theme_config.name.clone();
                    let is_selected = theme_name == self.current_theme;

                    list = list.child(
                        div().w_full().child(
                            Button::new(("theme-btn", idx))
                                .ghost()
                                .small()
                                .label(theme_name.clone())
                                .when(is_selected, |btn: Button| btn.primary())
                                .on_click({
                                    let theme_name = theme_name.clone();
                                    cx.listener(move |this, _, _, cx| {
                                        this.switch_theme(theme_name.clone(), cx);
                                    })
                                }),
                        ),
                    );
                }

                list
            }))
            .child(self.render_color_info(cx))
    }

    fn render_color_info(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .w_full()
            .border_t_1()
            .border_color(theme.border)
            .p_3()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .font_semibold()
                    .text_color(theme.muted_foreground)
                    .child("Terminal Colors"),
            )
            .child(self.render_color_swatch(cx, "Foreground".to_string(), theme.foreground))
            .child(self.render_color_swatch(cx, "Background".to_string(), theme.background))
            .child(self.render_color_swatch(cx, "Red".to_string(), theme.red))
            .child(self.render_color_swatch(cx, "Green".to_string(), theme.green))
            .child(self.render_color_swatch(cx, "Blue".to_string(), theme.blue))
            .child(self.render_color_swatch(cx, "Yellow".to_string(), theme.yellow))
    }

    fn render_color_swatch(&self, cx: &App, label: String, color: rgpui::Hsla) -> impl IntoElement {
        let theme = ComponentTheme::global(cx);

        h_flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .w(px(20.0))
                    .h(px(20.0))
                    .rounded_sm()
                    .border_1()
                    .border_color(theme.border)
                    .bg(color),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(label),
            )
    }

    fn render_terminal_area(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .bg(theme.background)
            .when_some(self.terminal_view.as_ref(), |el, tv| {
                el.child(div().flex_1().p_4().child(tv.clone()))
            })
            .when(self.terminal_view.is_none(), |el| {
                el.flex().items_center().justify_center().child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child("Initializing terminal..."),
                )
            })
    }
}

impl Focusable for ComponentIntegrationApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ComponentIntegrationApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .track_focus(&self.focus_handle)
            .child(self.render_header(cx))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .child(self.render_theme_palette(cx))
                    .child(self.render_terminal_area(cx)),
            )
    }
}

fn main() {
    env_logger::init();

    application().run(|cx: &mut App| {
        // Initialize gpui-component theme system
        rgpui_component::init(cx);

        cx.bind_keys(keybindings());
        cx.on_action(|_: &Quit, cx| cx.quit());

        let window_options = WindowOptions {
            titlebar: Some(rgpui::TitlebarOptions {
                title: Some("gpui-component Integration".into()),
                appears_transparent: false,
                ..Default::default()
            }),
            window_bounds: Some(rgpui::WindowBounds::Windowed(rgpui::Bounds {
                origin: rgpui::Point {
                    x: px(100.0),
                    y: px(100.0),
                },
                size: rgpui::Size {
                    width: px(1200.0),
                    height: px(800.0),
                },
            })),
            ..Default::default()
        };

        cx.open_window(window_options, |window, cx| {
            let view = cx.new(ComponentIntegrationApp::new);

            // Create terminal asynchronously
            let shell = platform_shell();
            let mut env_vars: HashMap<String, String> = env::vars().collect();
            env_vars.insert("TERM".to_string(), "xterm-256color".to_string());
            env_vars.insert("COLORTERM".to_string(), "truecolor".to_string());

            let window_id = window.window_handle().window_id().as_u64();
            let terminal_task = TerminalBuilder::new(
                env::current_dir().ok(),
                shell,
                env_vars,
                None,
                window_id,
                cx,
            );

            let view_clone = view.downgrade();
            let window_handle = window.window_handle();

            cx.spawn(async move |cx| {
                let builder = match terminal_task.await {
                    Ok(b) => b,
                    Err(e) => {
                        log::error!("Failed to create terminal: {}", e);
                        return;
                    }
                };

                let _ = cx.update_window(window_handle, |_, window, cx| {
                    let _ = view_clone.update(cx, |app, cx| {
                        let terminal = cx.new(|cx| builder.subscribe(cx));

                        // Add logging middleware
                        terminal.update(cx, |terminal, _| {
                            terminal.add_middleware(Arc::new(SimpleLogger));
                        });

                        app.set_terminal(terminal, window, cx);

                        log::info!("Terminal initialized with gpui-component theme");
                    });
                });
            })
            .detach();

            view
        })
        .unwrap();
    });
}
