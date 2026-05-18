//! Agent Term - A GPUI terminal with a floating sidebar overlay.
//!
//! This example focuses on reproducing Agent Term's current visual layout:
//! - Floating, rounded sidebar with inset + shadow
//! - Main terminal content padded to avoid the sidebar
//! - Transparent/blurred window background
//! - Multi-session terminal tabs in the title bar

use rgpui_component::button::{Button, ButtonVariants};
use rgpui_component::resizable::{h_resizable, resizable_panel, v_resizable};
use rgpui_component::tab::{Tab, TabBar};
use rgpui_component::{ActiveTheme, AxisExt, IconName, Sizable, h_flex, v_flex};
use rgpui_platform::application;
use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Mutex};

use rgpui::{
    AnyElement, App, AppContext, Axis, BoxShadow, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Pixels, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Window, WindowBackgroundAppearance, WindowOptions, actions,
    div, hsla, point, prelude::*, px, rgb, rgba,
};
use rgpui_term::{
    Clear, Copy, Event, InputOrigin, Paste, SelectAll, Terminal, TerminalBuilder, TerminalConfig,
    TerminalContent, TerminalMiddleware, TerminalView, TextStyle, ThemeManager,
};

mod persistence;
mod sidebar_state;
mod title_bar;
use sidebar_state::{
    FavoriteEntry, FavoritesData, RecentData, SavedTab, SidebarSection, WorkspaceLayout,
};
use title_bar::TitleBar;
actions!(
    agent_term,
    [
        Quit,
        ToggleSidebar,
        NewTab,
        CloseTab,
        NextTab,
        PreviousTab,
        SplitRight,
        SplitDown,
        SaveWorkspace,
        RestoreWorkspace,
        ToggleSearch,
        DumpText
    ]
);

// Layout (mirrors the Tauri UI tokens / App.tsx layout math)
const SIDEBAR_INSET: f32 = 8.0;
const SIDEBAR_GAP: f32 = 16.0;
const SIDEBAR_MIN_WIDTH: f32 = 200.0;
const SIDEBAR_MAX_WIDTH: f32 = 420.0;
const SIDEBAR_HEADER_LEFT_PADDING: f32 = 68.0;

// Colors (approximate the current Agent Term Tauri tokens)
const TEXT_PRIMARY: u32 = 0xd8d8d8;
const TEXT_SUBTLE: u32 = 0xa6a6a6;
const TEXT_FAINT: u32 = 0x5a5a5a;

const SURFACE_ROOT: u32 = 0x000000;
const SURFACE_SIDEBAR: u32 = 0x202020;
const BORDER_SOFT: u32 = 0x3a3a3a;

const SURFACE_ROOT_ALPHA: f32 = 0.12;
const SURFACE_SIDEBAR_ALPHA: f32 = 0.4;
const BORDER_SOFT_ALPHA: f32 = 0.50;

const ENABLE_BLUR: bool = false;

struct LoggingMiddleware {
    last_output: Mutex<Option<String>>,
}

impl LoggingMiddleware {
    fn new() -> Self {
        Self {
            last_output: Mutex::new(None),
        }
    }
}

fn format_input_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x1b => out.push_str("\\x1b"),
            0x20..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\x{:02x}", b)),
        }
    }
    out
}

fn content_to_string(content: &TerminalContent) -> String {
    let rows = content.terminal_bounds.num_lines();
    let cols = content.terminal_bounds.num_columns();
    let mut grid = vec![vec![' '; cols]; rows];

    for cell in &content.cells {
        let row = cell.point.line.0;
        let col = cell.point.column.0;
        if row >= 0 {
            let row = row as usize;
            let col = col;
            if row < rows && col < cols {
                grid[row][col] = cell.c;
            }
        }
    }

    let mut lines: Vec<String> = grid
        .into_iter()
        .map(|line| {
            let mut s: String = line.into_iter().collect();
            while s.ends_with(' ') {
                s.pop();
            }
            s
        })
        .collect();

    while matches!(lines.last(), Some(line) if line.is_empty()) {
        lines.pop();
    }

    lines.join("\n")
}

impl TerminalMiddleware for LoggingMiddleware {
    fn on_input(
        &self,
        input: Cow<'static, [u8]>,
        origin: InputOrigin,
    ) -> Option<Cow<'static, [u8]>> {
        let display = format_input_bytes(&input);
        eprintln!("[middleware] input origin={:?} {}", origin, display);
        Some(input)
    }

    fn on_event(&self, event: &Event) {
        eprintln!("[middleware] event {:?}", event);
    }

    fn on_output(&self, content: &TerminalContent) {
        let output = content_to_string(content);
        let mut guard = match self.last_output.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        if guard.as_ref() != Some(&output) {
            eprintln!("[middleware] output\n{}", output);
            *guard = Some(output);
        }
    }
}

fn platform_keybindings() -> Vec<KeyBinding> {
    let mut bindings = vec![
        KeyBinding::new("ctrl-shift-c", Copy, Some("Terminal")),
        KeyBinding::new("ctrl-shift-v", Paste, Some("Terminal")),
    ];

    #[cfg(target_os = "macos")]
    {
        bindings.extend([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-b", ToggleSidebar, None),
            KeyBinding::new("cmd-c", Copy, Some("Terminal")),
            KeyBinding::new("cmd-v", Paste, Some("Terminal")),
            KeyBinding::new("cmd-a", SelectAll, Some("Terminal")),
            KeyBinding::new("cmd-k", Clear, Some("Terminal")),
            KeyBinding::new("cmd-t", NewTab, None),
            KeyBinding::new("cmd-w", CloseTab, None),
            KeyBinding::new("ctrl-tab", NextTab, None),
            KeyBinding::new("ctrl-shift-tab", PreviousTab, None),
            KeyBinding::new("cmd-d", SplitRight, None),
            KeyBinding::new("cmd-shift-d", SplitDown, None),
        ]);
    }

    #[cfg(not(target_os = "macos"))]
    {
        bindings.extend([
            KeyBinding::new("ctrl-shift-q", Quit, None),
            KeyBinding::new("ctrl-shift-b", ToggleSidebar, None),
            KeyBinding::new("ctrl-shift-a", SelectAll, Some("Terminal")),
            KeyBinding::new("ctrl-shift-k", Clear, Some("Terminal")),
            KeyBinding::new("ctrl-shift-t", NewTab, None),
            KeyBinding::new("ctrl-shift-w", CloseTab, None),
            KeyBinding::new("ctrl-tab", NextTab, None),
            KeyBinding::new("ctrl-shift-tab", PreviousTab, None),
            KeyBinding::new("ctrl-shift-d", SplitRight, None),
            KeyBinding::new("ctrl-alt-d", SplitDown, None),
            KeyBinding::new("ctrl-shift-g", DumpText, None),
        ]);
    }

    bindings
}

#[cfg(windows)]
fn find_on_path(executable: &str) -> Option<String> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(executable);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

#[cfg(windows)]
fn find_pwsh() -> Option<String> {
    if let Some(path) = find_on_path("pwsh.exe") {
        return Some(path);
    }

    let roots = ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"];
    for key in roots {
        if let Ok(root) = env::var(key) {
            for suffix in ["PowerShell\\7\\pwsh.exe", "PowerShell\\7-preview\\pwsh.exe"] {
                let path = std::path::PathBuf::from(&root).join(suffix);
                if path.is_file() {
                    return Some(path.to_string_lossy().into_owned());
                }
            }
        }
    }

    if let Ok(root) = env::var("LOCALAPPDATA") {
        for suffix in [
            "Microsoft\\PowerShell\\7\\pwsh.exe",
            "Microsoft\\PowerShell\\7-preview\\pwsh.exe",
        ] {
            let path = std::path::PathBuf::from(&root).join(suffix);
            if path.is_file() {
                return Some(path.to_string_lossy().into_owned());
            }
        }
    }

    None
}

fn platform_shell() -> Option<String> {
    #[cfg(windows)]
    {
        if let Ok(shell) = env::var("SHELL") {
            return Some(shell);
        }

        if let Some(pwsh) = find_pwsh() {
            return Some(pwsh);
        }

        if let Ok(root) = env::var("SystemRoot") {
            let mut path = std::path::PathBuf::from(root);
            path.push("System32");
            path.push("WindowsPowerShell");
            path.push("v1.0");
            path.push("powershell.exe");
            return Some(path.to_string_lossy().into_owned());
        }

        return Some("powershell".to_string());
    }

    #[cfg(not(windows))]
    {
        if let Ok(shell) = env::var("SHELL") {
            return Some(shell);
        }
        if std::path::Path::new("/bin/zsh").exists() {
            Some("/bin/zsh".to_string())
        } else {
            Some("/bin/bash".to_string())
        }
    }
}

fn rgba_u32(rgb: u32, alpha: f32) -> u32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    (rgb << 8) | a
}

fn main() {
    let app = application().with_assets(rgpui_component_assets::Assets);

    app.run(|cx: &mut App| {
        rgpui_component::init(cx);
        cx.bind_keys(platform_keybindings());

        cx.on_action(|_: &Quit, cx| cx.quit());

        // Initialize theme manager with config
        let terminal_config =
            TerminalConfig::load_or_create().unwrap_or_else(|_| TerminalConfig::default());
        let theme_manager = ThemeManager::new(Some(terminal_config.theme.clone()));
        cx.set_global(theme_manager);

        let background_appearance = if ENABLE_BLUR {
            WindowBackgroundAppearance::Blurred
        } else {
            WindowBackgroundAppearance::Opaque
        };

        let window_options = WindowOptions {
            titlebar: Some(rgpui::TitlebarOptions {
                title: Some("Agent Term".into()),
                appears_transparent: true,
                traffic_light_position: Some(rgpui::point(px(16.0), px(16.0))),
                ..Default::default()
            }),
            window_background: background_appearance,
            ..Default::default()
        };

        cx.open_window(window_options, |window, cx| {
            window.set_background_appearance(background_appearance);

            let terminal_config =
                TerminalConfig::load_or_create().unwrap_or_else(|_| TerminalConfig::default());
            let text_style = TextStyle::from_config(&terminal_config);

            let view = cx.new(|cx| {
                let focus_handle = cx.focus_handle();
                focus_handle.focus(window, cx);

                // Load persistent state
                let favorites = persistence::load_favorites();
                let recent_dirs = persistence::load_recent();
                let saved_workspaces = persistence::load_workspaces();

                let mut section_expanded = HashMap::new();
                section_expanded.insert(SidebarSection::Favorites, true);
                section_expanded.insert(SidebarSection::Projects, true);
                section_expanded.insert(SidebarSection::RecentDirs, true);
                section_expanded.insert(SidebarSection::Workspaces, false);

                AgentTermApp {
                    tabs: Vec::new(),
                    active_tab_index: 0,
                    next_tab_id: 0,
                    next_pane_id: 0,
                    active_pane_id: 0,
                    focus_handle,
                    tab_bar_scroll_handle: ScrollHandle::new(),
                    sidebar_visible: true,
                    sidebar_width: 250.0,
                    resizing_sidebar: false,
                    resize_start_x: Pixels::ZERO,
                    resize_start_width: 250.0,
                    text_style,
                    projects: vec![
                        Project {
                            name: "Agent Term".into(),
                            expanded: true,
                            sessions: vec![TerminalSession {
                                name: "adityasharma@Ma...".into(),
                                active: true,
                            }],
                        },
                        Project {
                            name: "Frontier OS".into(),
                            expanded: true,
                            sessions: vec![],
                        },
                    ],
                    show_theme_menu: false,
                    selected_theme: "One Dark".into(),
                    search_visible: false,
                    search_query: String::new(),
                    favorites,
                    recent_dirs,
                    saved_workspaces,
                    section_expanded,
                }
            });

            // Create the initial tab
            let view_clone = view.downgrade();
            let window_handle = window.window_handle();
            cx.spawn(async move |cx| {
                let _ = cx.update_window(window_handle, |_, window, cx| {
                    let _ = view_clone.update(cx, |app, cx| {
                        app.create_new_tab(window, cx);
                    });
                });
            })
            .detach();

            view
        })
        .unwrap();
    });
}

#[derive(Clone)]
struct Project {
    name: SharedString,
    expanded: bool,
    sessions: Vec<TerminalSession>,
}

#[derive(Clone)]
struct TerminalSession {
    name: SharedString,
    active: bool,
}

/// Represents the pane layout within a tab
enum PaneNode {
    Leaf {
        pane_id: usize,
        terminal: Entity<Terminal>,
        terminal_view: Entity<TerminalView>,
    },
    Split {
        axis: Axis,
        children: Vec<PaneNode>,
    },
}

impl PaneNode {
    fn first_pane_id(&self) -> usize {
        match self {
            PaneNode::Leaf { pane_id, .. } => *pane_id,
            PaneNode::Split { children, .. } => children[0].first_pane_id(),
        }
    }

    fn first_terminal_view(&self) -> Entity<TerminalView> {
        match self {
            PaneNode::Leaf { terminal_view, .. } => terminal_view.clone(),
            PaneNode::Split { children, .. } => children[0].first_terminal_view(),
        }
    }

    fn split_pane(
        &mut self,
        target_pane_id: usize,
        axis: Axis,
        new_pane_id: usize,
        new_terminal: Entity<Terminal>,
        new_terminal_view: Entity<TerminalView>,
    ) -> bool {
        match self {
            PaneNode::Leaf { pane_id, .. } if *pane_id == target_pane_id => {
                let old = std::mem::replace(
                    self,
                    PaneNode::Split {
                        axis,
                        children: Vec::new(),
                    },
                );
                if let PaneNode::Split { children, .. } = self {
                    children.push(old);
                    children.push(PaneNode::Leaf {
                        pane_id: new_pane_id,
                        terminal: new_terminal,
                        terminal_view: new_terminal_view,
                    });
                }
                true
            }
            PaneNode::Split { children, .. } => {
                for child in children.iter_mut() {
                    if child.split_pane(
                        target_pane_id,
                        axis,
                        new_pane_id,
                        new_terminal.clone(),
                        new_terminal_view.clone(),
                    ) {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn remove_pane(&mut self, target_pane_id: usize) -> bool {
        match self {
            PaneNode::Leaf { pane_id, .. } => *pane_id == target_pane_id,
            PaneNode::Split { children, .. } => {
                let mut removed_idx = None;
                for (i, child) in children.iter_mut().enumerate() {
                    if child.remove_pane(target_pane_id) {
                        removed_idx = Some(i);
                        break;
                    }
                }
                if let Some(idx) = removed_idx {
                    children.remove(idx);
                }
                if children.len() == 1 {
                    let only_child = children.remove(0);
                    *self = only_child;
                    return false;
                }
                children.is_empty()
            }
        }
    }

    fn collect_terminal_views(&self, views: &mut Vec<Entity<TerminalView>>) {
        match self {
            PaneNode::Leaf { terminal_view, .. } => {
                views.push(terminal_view.clone());
            }
            PaneNode::Split { children, .. } => {
                for child in children {
                    child.collect_terminal_views(views);
                }
            }
        }
    }

    fn find_terminal(&self, target_pane_id: usize) -> Option<&Entity<Terminal>> {
        match self {
            PaneNode::Leaf {
                pane_id, terminal, ..
            } if *pane_id == target_pane_id => Some(terminal),
            PaneNode::Split { children, .. } => children
                .iter()
                .find_map(|c| c.find_terminal(target_pane_id)),
            _ => None,
        }
    }
}

/// Represents a single terminal tab session
struct TerminalTab {
    id: usize,
    root: PaneNode,
    title: SharedString,
}

struct AgentTermApp {
    tabs: Vec<TerminalTab>,
    active_tab_index: usize,
    next_tab_id: usize,
    next_pane_id: usize,
    active_pane_id: usize,
    focus_handle: FocusHandle,
    tab_bar_scroll_handle: ScrollHandle,
    sidebar_visible: bool,
    sidebar_width: f32,
    resizing_sidebar: bool,
    resize_start_x: Pixels,
    resize_start_width: f32,
    text_style: TextStyle,
    projects: Vec<Project>,
    show_theme_menu: bool,
    selected_theme: SharedString,
    // Search
    search_visible: bool,
    search_query: String,
    // Favorites
    favorites: FavoritesData,
    // Recent directories
    recent_dirs: RecentData,
    // Saved workspaces
    saved_workspaces: Vec<WorkspaceLayout>,
    // Section collapse state
    section_expanded: HashMap<SidebarSection, bool>,
}

impl AgentTermApp {
    fn create_new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        eprintln!(
            "[DEBUG] create_new_tab called, current tabs: {}",
            self.tabs.len()
        );
        let shell = platform_shell();
        let mut env_vars: HashMap<String, String> = env::vars().collect();
        env_vars.insert("TERM".to_string(), "xterm-256color".to_string());
        env_vars.insert("COLORTERM".to_string(), "truecolor".to_string());

        let window_id = window.window_handle().window_id().as_u64();

        let working_dir = match env::current_dir() {
            Ok(dir) if dir.exists() => Some(dir),
            _ => None,
        };

        // Record in recent directories
        if let Some(ref dir) = working_dir {
            self.recent_dirs.touch(dir.clone());
            persistence::save_recent(&self.recent_dirs);
        }

        let terminal_task = TerminalBuilder::new(working_dir, shell, env_vars, None, window_id, cx);

        let text_style = self.text_style.clone();
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;

        let window_handle = window.window_handle();

        cx.spawn(async move |view_handle, cx| {
            let builder = match terminal_task.await {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Failed to create terminal: {e}");
                    return;
                }
            };

            let _ = cx.update_window(window_handle, |_, window, cx| {
                let _ = view_handle.update(cx, |app, cx| {
                    let terminal = cx.new(|cx| builder.subscribe(cx));
                    terminal.update(cx, |terminal, _| {
                        terminal.add_middleware(Arc::new(LoggingMiddleware::new()));
                    });

                    let terminal_view = cx.new(|cx| {
                        TerminalView::new_with_style(terminal.clone(), text_style, window, cx)
                    });

                    let pane_id = app.next_pane_id;
                    app.next_pane_id += 1;

                    // Subscribe to terminal events for title changes and close
                    app.subscribe_to_terminal_events(tab_id, pane_id, &terminal, cx);

                    let title: SharedString = format!("Terminal {}", tab_id + 1).into();

                    let tab = TerminalTab {
                        id: tab_id,
                        root: PaneNode::Leaf {
                            pane_id,
                            terminal: terminal.clone(),
                            terminal_view: terminal_view.clone(),
                        },
                        title,
                    };

                    app.tabs.push(tab);
                    app.active_tab_index = app.tabs.len() - 1;
                    app.active_pane_id = pane_id;
                    app.tab_bar_scroll_handle
                        .scroll_to_item(app.active_tab_index);

                    // Focus the new terminal
                    let focus_handle = terminal_view.read(cx).focus_handle(cx);
                    focus_handle.focus(window, cx);

                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn subscribe_to_terminal_events(
        &mut self,
        tab_id: usize,
        pane_id: usize,
        terminal: &Entity<Terminal>,
        cx: &mut Context<Self>,
    ) {
        cx.subscribe(
            terminal,
            move |this, terminal, event: &Event, cx| match event {
                Event::TitleChanged => {
                    let title = terminal.read(cx).breadcrumb_text.clone();
                    if let Some(tab) = this.tabs.iter_mut().find(|t| t.id == tab_id) {
                        if tab.root.first_pane_id() == pane_id && !title.is_empty() {
                            tab.title = title.into();
                        }
                    }
                    cx.notify();
                }
                Event::CloseTerminal => {
                    if let Some(tab) = this.tabs.iter_mut().find(|t| t.id == tab_id) {
                        let should_remove = tab.root.remove_pane(pane_id);
                        if should_remove {
                            this.close_tab_by_id(tab_id, cx);
                            return;
                        }
                    }
                    cx.notify();
                }
                _ => {}
            },
        )
        .detach();
    }

    fn switch_to_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index < self.tabs.len() {
            self.active_tab_index = index;
            self.tab_bar_scroll_handle.scroll_to_item(index);
            if let Some(tab) = self.tabs.get(index) {
                let tv = tab.root.first_terminal_view();
                self.active_pane_id = tab.root.first_pane_id();
                let focus_handle = tv.read(cx).focus_handle(cx);
                focus_handle.focus(window, cx);
            }
            cx.notify();
        }
    }

    fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            // Don't close the last tab, create a new one instead
            self.create_new_tab(window, cx);
            if !self.tabs.is_empty() {
                self.tabs.remove(0);
            }
            return;
        }

        if index < self.tabs.len() {
            self.tabs.remove(index);

            // Adjust active index
            if self.active_tab_index >= self.tabs.len() {
                self.active_tab_index = self.tabs.len().saturating_sub(1);
            } else if self.active_tab_index > index {
                self.active_tab_index = self.active_tab_index.saturating_sub(1);
            }
            if !self.tabs.is_empty() {
                self.tab_bar_scroll_handle
                    .scroll_to_item(self.active_tab_index);
            }

            // Focus the new active terminal
            if let Some(tab) = self.tabs.get(self.active_tab_index) {
                let tv = tab.root.first_terminal_view();
                self.active_pane_id = tab.root.first_pane_id();
                let focus_handle = tv.read(cx).focus_handle(cx);
                focus_handle.focus(window, cx);
            }

            cx.notify();
        }
    }

    fn close_tab_by_id(&mut self, tab_id: usize, cx: &mut Context<Self>) {
        if let Some(index) = self.tabs.iter().position(|t| t.id == tab_id) {
            if self.tabs.len() <= 1 {
                // Keep a placeholder - new tab will be created on next render
                cx.notify();
                return;
            }

            self.tabs.remove(index);

            if self.active_tab_index >= self.tabs.len() {
                self.active_tab_index = self.tabs.len().saturating_sub(1);
            } else if self.active_tab_index > index {
                self.active_tab_index = self.active_tab_index.saturating_sub(1);
            }
            if !self.tabs.is_empty() {
                self.tab_bar_scroll_handle
                    .scroll_to_item(self.active_tab_index);
                if let Some(tab) = self.tabs.get(self.active_tab_index) {
                    self.active_pane_id = tab.root.first_pane_id();
                }
            }

            cx.notify();
        }
    }

    fn has_multiple_panes(&self) -> bool {
        if let Some(tab) = self.tabs.get(self.active_tab_index) {
            matches!(&tab.root, PaneNode::Split { .. })
        } else {
            false
        }
    }

    fn close_pane(&mut self, pane_id: usize, cx: &mut Context<Self>) {
        let should_close_tab = if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            let should_close = tab.root.remove_pane(pane_id);
            if !should_close {
                // Update active pane to the first remaining pane
                self.active_pane_id = tab.root.first_pane_id();
            }
            should_close
        } else {
            false
        };

        if should_close_tab {
            if let Some(tab) = self.tabs.get(self.active_tab_index) {
                let tab_id = tab.id;
                self.close_tab_by_id(tab_id, cx);
                return;
            }
        }
        cx.notify();
    }

    fn on_new_tab(&mut self, _: &NewTab, window: &mut Window, cx: &mut Context<Self>) {
        self.create_new_tab(window, cx);
    }

    fn on_close_tab(&mut self, _: &CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        self.close_tab(self.active_tab_index, window, cx);
    }

    fn on_next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<Self>) {
        if !self.tabs.is_empty() {
            let next = (self.active_tab_index + 1) % self.tabs.len();
            self.switch_to_tab(next, window, cx);
        }
    }

    fn on_previous_tab(&mut self, _: &PreviousTab, window: &mut Window, cx: &mut Context<Self>) {
        if !self.tabs.is_empty() {
            let prev = if self.active_tab_index == 0 {
                self.tabs.len() - 1
            } else {
                self.active_tab_index - 1
            };
            self.switch_to_tab(prev, window, cx);
        }
    }

    fn toggle_sidebar(&mut self, _: &ToggleSidebar, _window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }

    fn toggle_theme_menu(&mut self, cx: &mut Context<Self>) {
        self.show_theme_menu = !self.show_theme_menu;
        cx.notify();
    }

    fn on_split_right(&mut self, _: &SplitRight, window: &mut Window, cx: &mut Context<Self>) {
        self.split_active_pane(Axis::Horizontal, window, cx);
    }

    fn on_split_down(&mut self, _: &SplitDown, window: &mut Window, cx: &mut Context<Self>) {
        self.split_active_pane(Axis::Vertical, window, cx);
    }

    fn on_dump_text(&mut self, _: &DumpText, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(self.active_tab_index) else {
            return;
        };
        let Some(terminal) = tab.root.find_terminal(self.active_pane_id) else {
            return;
        };
        let terminal = terminal.read(cx);

        // Dump blocks
        let blocks = terminal.blocks();
        eprintln!("=== Terminal Blocks ({}) ===", blocks.len());
        for block in blocks {
            eprintln!("[Block #{}] command: {:?}", block.id, block.command);
            eprintln!("  detection: {:?}", block.detection);
            if let Some(code) = block.exit_code {
                eprintln!("  exit_code: {}", code);
            }
            eprintln!("  output:");
            for line in block.output.lines() {
                eprintln!("    {}", line);
            }
            eprintln!("---");
        }
        if terminal.block_running() {
            eprintln!("[Block tracker: command running]");
        }

        // Also dump full text
        let text = terminal.get_all_text();
        eprintln!("=== Terminal Full Text ===\n{}\n=== End ===", text);
    }

    fn split_active_pane(&mut self, axis: Axis, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(self.active_tab_index) else {
            return;
        };
        let target_pane_id = self.active_pane_id;
        let tab_id = tab.id;

        let shell = platform_shell();
        let mut env_vars: HashMap<String, String> = env::vars().collect();
        env_vars.insert("TERM".to_string(), "xterm-256color".to_string());
        env_vars.insert("COLORTERM".to_string(), "truecolor".to_string());
        let window_id = window.window_handle().window_id().as_u64();
        let working_dir = env::current_dir().ok().filter(|d| d.exists());
        let terminal_task = TerminalBuilder::new(working_dir, shell, env_vars, None, window_id, cx);

        let text_style = self.text_style.clone();
        let new_pane_id = self.next_pane_id;
        self.next_pane_id += 1;

        let window_handle = window.window_handle();

        cx.spawn(async move |view_handle, cx| {
            let builder = match terminal_task.await {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Failed to create terminal for split: {e}");
                    return;
                }
            };

            let _ = cx.update_window(window_handle, |_, window, cx| {
                let _ = view_handle.update(cx, |app, cx| {
                    let terminal = cx.new(|cx| builder.subscribe(cx));
                    terminal.update(cx, |terminal, _| {
                        terminal.add_middleware(Arc::new(LoggingMiddleware::new()));
                    });

                    let terminal_view = cx.new(|cx| {
                        TerminalView::new_with_style(terminal.clone(), text_style, window, cx)
                    });

                    app.subscribe_to_terminal_events(tab_id, new_pane_id, &terminal, cx);

                    if let Some(tab) = app.tabs.iter_mut().find(|t| t.id == tab_id) {
                        tab.root.split_pane(
                            target_pane_id,
                            axis,
                            new_pane_id,
                            terminal.clone(),
                            terminal_view.clone(),
                        );
                    }

                    app.active_pane_id = new_pane_id;
                    let focus_handle = terminal_view.read(cx).focus_handle(cx);
                    focus_handle.focus(window, cx);

                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn render_pane_node(
        &self,
        node: &PaneNode,
        id_prefix: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let active_border_color = theme.primary;
        let hover_border_color = theme.border;
        let transparent = rgba(0x00000000);

        match node {
            PaneNode::Leaf {
                terminal_view,
                pane_id,
                ..
            } => {
                let pane_id = *pane_id;
                let is_active = self.active_pane_id == pane_id;
                let terminal_view_clone = terminal_view.clone();
                let has_multiple_panes = self.has_multiple_panes();

                div()
                    .id(SharedString::from(format!("pane-{}", pane_id)))
                    .group(SharedString::from(format!("pane-group-{}", pane_id)))
                    .size_full()
                    .overflow_hidden()
                    .relative()
                    .border_t_1()
                    .when(is_active, |el| el.border_color(active_border_color))
                    .when(!is_active, |el| el.border_color(transparent))
                    .hover(|el| {
                        if is_active {
                            el.border_color(active_border_color)
                        } else {
                            el.border_color(hover_border_color)
                        }
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.active_pane_id = pane_id;
                            let focus_handle = terminal_view_clone.read(cx).focus_handle(cx);
                            focus_handle.focus(window, cx);
                            cx.notify();
                        }),
                    )
                    .child(terminal_view.clone())
                    .when(!is_active && has_multiple_panes, |el| {
                        el.child(
                            div()
                                .id(SharedString::from(format!("close-pane-{}", pane_id)))
                                .absolute()
                                .top_1()
                                .right_1()
                                .w(px(20.0))
                                .h(px(20.0))
                                .rounded_md()
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xs()
                                .text_color(rgb(TEXT_SUBTLE))
                                .bg(rgba(0x00000080))
                                .opacity(0.0)
                                .group_hover(
                                    SharedString::from(format!("pane-group-{}", pane_id)),
                                    |s| s.opacity(1.0),
                                )
                                .hover(|s| s.bg(rgba(0xef444480)).text_color(rgb(0xffffff)))
                                .child("×")
                                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation();
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.close_pane(pane_id, cx);
                                })),
                        )
                    })
                    .into_any_element()
            }
            PaneNode::Split { axis, children } => {
                let id = SharedString::from(format!("{}-split", id_prefix));
                let mut group = if axis.is_horizontal() {
                    h_resizable(id)
                } else {
                    v_resizable(id)
                };
                for (i, child) in children.iter().enumerate() {
                    let child_id = format!("{}-{}", id_prefix, i);
                    let child_el = self.render_pane_node(child, &child_id, window, cx);
                    group = group.child(resizable_panel().child(child_el));
                }
                group.into_any_element()
            }
        }
    }

    fn tab_scroll_step(&self) -> Pixels {
        let viewport_width = self.tab_bar_scroll_handle.bounds().size.width;
        if viewport_width == Pixels::ZERO {
            px(160.0)
        } else {
            viewport_width * 0.6
        }
    }

    fn scroll_tab_bar_by(&mut self, delta: Pixels) {
        let max_offset = self.tab_bar_scroll_handle.max_offset();
        if max_offset.x == Pixels::ZERO {
            return;
        }

        let mut offset = self.tab_bar_scroll_handle.offset();
        let next_x = (offset.x + delta).clamp(-max_offset.x, px(0.));
        if next_x != offset.x {
            offset.x = next_x;
            self.tab_bar_scroll_handle.set_offset(offset);
        }
    }

    fn scroll_tab_bar_left(&mut self) {
        let step = self.tab_scroll_step();
        self.scroll_tab_bar_by(step);
    }

    fn scroll_tab_bar_right(&mut self) {
        let step = self.tab_scroll_step();
        self.scroll_tab_bar_by(-step);
    }

    fn switch_theme(&mut self, theme_name: SharedString, cx: &mut Context<Self>) {
        self.selected_theme = theme_name.clone();
        self.show_theme_menu = false;

        // Update the theme in ThemeManager
        ThemeManager::global_mut(cx).set_theme(&theme_name);

        // Update all terminal views' themes
        let mut views = Vec::new();
        for tab in &self.tabs {
            tab.root.collect_terminal_views(&mut views);
        }
        for view in views {
            view.update(cx, |v, cx| {
                v.set_theme(&theme_name, cx);
            });
        }

        cx.notify();
    }

    fn start_sidebar_resize(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.resizing_sidebar = true;
        self.resize_start_x = event.position.x;
        self.resize_start_width = self.sidebar_width;
        cx.notify();
    }

    fn stop_sidebar_resize(
        &mut self,
        _event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.resizing_sidebar {
            self.resizing_sidebar = false;
            cx.notify();
        }
    }

    fn update_sidebar_resize(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.resizing_sidebar || !event.dragging() {
            return;
        }

        let delta = event.position.x - self.resize_start_x;
        let next_width =
            (self.resize_start_width + delta / px(1.0)).clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH);
        if (next_width - self.sidebar_width).abs() > 0.1 {
            self.sidebar_width = next_width;
            cx.notify();
        }
    }

    fn sidebar_shadow() -> Vec<BoxShadow> {
        vec![
            BoxShadow {
                color: hsla(0., 0., 0., 0.25),
                offset: point(px(0.0), px(18.0)),
                blur_radius: px(45.0),
                spread_radius: px(0.0),
            },
            BoxShadow {
                color: hsla(0., 0., 0., 0.15),
                offset: point(px(0.0), px(6.0)),
                blur_radius: px(18.0),
                spread_radius: px(0.0),
            },
        ]
    }

    fn render_sidebar_shell(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("sidebar-shell")
            .absolute()
            .left(px(SIDEBAR_INSET))
            .top(px(SIDEBAR_INSET))
            .bottom(px(SIDEBAR_INSET))
            .w(px(self.sidebar_width))
            .relative()
            .child(
                div()
                    .id("sidebar-wrapper")
                    .size_full()
                    .rounded(px(16.0))
                    .overflow_hidden()
                    .bg(rgba(rgba_u32(SURFACE_SIDEBAR, SURFACE_SIDEBAR_ALPHA)))
                    .border_1()
                    .border_color(rgba(rgba_u32(BORDER_SOFT, BORDER_SOFT_ALPHA)))
                    .shadow(Self::sidebar_shadow())
                    .child(self.render_sidebar_content(cx)),
            )
            .when(self.show_theme_menu, |el| {
                el.child(self.render_theme_menu(cx))
            })
            .child(
                div()
                    .id("sidebar-resizer")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(self.sidebar_width - 3.0))
                    .w(px(6.0))
                    .rounded(px(999.0))
                    .bg(rgpui::transparent_black())
                    .cursor_col_resize()
                    .hover(|s| s.bg(rgba(rgba_u32(TEXT_PRIMARY, 0.20))))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::start_sidebar_resize))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::stop_sidebar_resize))
                    .on_mouse_move(cx.listener(Self::update_sidebar_resize)),
            )
    }

    fn render_sidebar_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("sidebar-content")
            .size_full()
            .flex()
            .flex_col()
            .child(self.render_sidebar_header(cx))
            .when(self.search_visible, |el| {
                el.child(self.render_search_bar(cx))
            })
            .child(self.render_quick_actions(cx))
            .child(
                div()
                    .id("sidebar-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(self.render_favorites_section(cx))
                    .child(self.render_project_tree(cx))
                    .child(self.render_recent_dirs_section(cx))
                    .child(self.render_workspaces_section(cx)),
            )
            .child(self.render_sidebar_footer())
    }

    fn render_sidebar_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h(px(44.0))
            .pl(px(SIDEBAR_HEADER_LEFT_PADDING))
            .pr(px(16.0))
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgba(rgba_u32(BORDER_SOFT, BORDER_SOFT_ALPHA)))
            .child(
                div()
                    .text_sm()
                    .font_weight(rgpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(TEXT_PRIMARY))
                    .child("AGENT TERM"),
            )
            .child(
                div()
                    .flex()
                    .gap(px(4.0))
                    .child(
                        Button::new("nav-back")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowLeft),
                    )
                    .child(
                        Button::new("nav-forward")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowRight),
                    )
                    .child(
                        Button::new("search-toggle")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Search)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.search_visible = !this.search_visible;
                                if !this.search_visible {
                                    this.search_query.clear();
                                }
                                cx.notify();
                            })),
                    )
                    .child(self.render_theme_button(cx)),
            )
    }

    fn render_search_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.search_query.clone();
        div()
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(rgba(rgba_u32(BORDER_SOFT, BORDER_SOFT_ALPHA)))
            .child(
                div()
                    .id("search-input")
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(px(6.0))
                    .bg(rgba(0xffffff08))
                    .border_1()
                    .border_color(rgba(rgba_u32(BORDER_SOFT, 0.3)))
                    .child(div().text_xs().text_color(rgb(TEXT_FAINT)).child("⌕"))
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(if query.is_empty() {
                                rgb(TEXT_FAINT)
                            } else {
                                rgb(TEXT_PRIMARY)
                            })
                            .child(if query.is_empty() {
                                "Search sessions...".to_string()
                            } else {
                                query
                            }),
                    )
                    .when(!self.search_query.is_empty(), |el| {
                        el.child(
                            div()
                                .id("search-clear")
                                .text_xs()
                                .text_color(rgb(TEXT_SUBTLE))
                                .cursor_pointer()
                                .hover(|s| s.text_color(rgb(TEXT_PRIMARY)))
                                .child("×")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.search_query.clear();
                                        cx.notify();
                                    }),
                                ),
                        )
                    }),
            )
    }

    fn render_quick_actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(12.0))
            .py(px(8.0))
            .flex()
            .gap(px(6.0))
            .border_b_1()
            .border_color(rgba(rgba_u32(BORDER_SOFT, BORDER_SOFT_ALPHA)))
            .child(
                Button::new("new-terminal")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Plus)
                    .label("New")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.create_new_tab(window, cx);
                    })),
            )
            .child(
                Button::new("ssh-connect")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Globe)
                    .label("SSH"),
            )
            .child(
                Button::new("clone-session")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Copy)
                    .label("Clone"),
            )
    }

    fn render_section_header(
        &self,
        section: SidebarSection,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let expanded = self.section_expanded.get(&section).copied().unwrap_or(true);
        div()
            .id(SharedString::from(format!("section-{:?}", section)))
            .px(px(12.0))
            .py(px(6.0))
            .flex()
            .items_center()
            .gap(px(6.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(0xffffff08)))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(TEXT_FAINT))
                    .child(if expanded { "▼" } else { "▶" }),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(rgpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(TEXT_SUBTLE))
                    .child(section.title().to_uppercase()),
            )
            .on_mouse_down(MouseButton::Left, {
                cx.listener(move |this, _, _, cx| {
                    let expanded = this.section_expanded.entry(section).or_insert(true);
                    *expanded = !*expanded;
                    cx.notify();
                })
            })
    }

    fn is_section_expanded(&self, section: SidebarSection) -> bool {
        self.section_expanded.get(&section).copied().unwrap_or(true)
    }

    fn render_favorites_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let expanded = self.is_section_expanded(SidebarSection::Favorites);
        let mut container = div().child(self.render_section_header(SidebarSection::Favorites, cx));

        if expanded {
            if self.favorites.entries.is_empty() {
                container = container.child(
                    div()
                        .px(px(16.0))
                        .py(px(4.0))
                        .text_sm()
                        .text_color(rgb(TEXT_FAINT))
                        .child("No favorites yet"),
                );
            } else {
                for (i, entry) in self.favorites.entries.iter().enumerate() {
                    let display = entry.display_name();
                    let path = entry.path.clone();
                    let matches_search = self.search_query.is_empty()
                        || display
                            .to_lowercase()
                            .contains(&self.search_query.to_lowercase());
                    if matches_search {
                        container = container.child(
                            div()
                                .id(SharedString::from(format!("fav-{}", i)))
                                .group(SharedString::from(format!("fav-group-{}", i)))
                                .px(px(16.0))
                                .py(px(4.0))
                                .flex()
                                .items_center()
                                .justify_between()
                                .rounded(px(4.0))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(0xffffff10)))
                                .on_mouse_down(MouseButton::Left, {
                                    cx.listener(move |this, _, window, cx| {
                                        // Open terminal in this directory
                                        // For now just create a new tab
                                        this.create_new_tab(window, cx);
                                    })
                                })
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.0))
                                        .child(
                                            div().text_xs().text_color(rgb(TEXT_SUBTLE)).child("★"),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(rgb(TEXT_PRIMARY))
                                                .overflow_x_hidden()
                                                .max_w(px(120.0))
                                                .child(display),
                                        ),
                                )
                                .child(
                                    div()
                                        .id(SharedString::from(format!("fav-rm-{}", i)))
                                        .text_xs()
                                        .text_color(rgb(TEXT_SUBTLE))
                                        .cursor_pointer()
                                        .opacity(0.0)
                                        .group_hover(
                                            SharedString::from(format!("fav-group-{}", i)),
                                            |s| s.opacity(1.0),
                                        )
                                        .hover(|s| s.text_color(rgb(TEXT_PRIMARY)))
                                        .child("×")
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_click({
                                            cx.listener(move |this, _, _, cx| {
                                                this.favorites.remove(&path);
                                                persistence::save_favorites(&this.favorites);
                                                cx.notify();
                                            })
                                        }),
                                ),
                        );
                    }
                }
            }

            // Add favorite button
            container = container.child(
                div()
                    .id("add-favorite")
                    .px(px(16.0))
                    .py(px(4.0))
                    .text_sm()
                    .text_color(rgb(TEXT_FAINT))
                    .cursor_pointer()
                    .hover(|s| s.text_color(rgb(TEXT_SUBTLE)))
                    .child("+ Add current dir")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if let Ok(dir) = env::current_dir() {
                                this.favorites.add(FavoriteEntry::new(dir));
                                persistence::save_favorites(&this.favorites);
                                cx.notify();
                            }
                        }),
                    ),
            );
        }

        container
    }

    fn render_project_tree(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let expanded = self.is_section_expanded(SidebarSection::Projects);
        let mut container = div().child(self.render_section_header(SidebarSection::Projects, cx));

        if expanded {
            container = container.child(self.render_add_project());

            let mut tree = div().px(px(8.0));
            for project in &self.projects {
                tree = tree.child(self.render_project(project));
            }
            container = container.child(tree);
        }

        container
    }

    fn render_theme_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(20.0))
            .h(px(20.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .cursor_pointer()
            .text_color(rgb(TEXT_SUBTLE))
            .hover(|s| s.text_color(rgb(TEXT_PRIMARY)).bg(rgba(0xffffff10)))
            .child("◐")
            .when(self.show_theme_menu, |s| s.bg(rgba(0xffffff20)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.toggle_theme_menu(cx);
                }),
            )
    }

    fn render_theme_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme_manager = ThemeManager::global(cx);
        let available_themes = theme_manager.available_themes();

        div()
            .absolute()
            .top(px(52.0))
            .right(px(16.0))
            .min_w(px(180.0))
            .max_w(px(220.0))
            .rounded(px(8.0))
            .bg(rgba(rgba_u32(SURFACE_SIDEBAR, 0.95)))
            .border_1()
            .border_color(rgba(rgba_u32(BORDER_SOFT, BORDER_SOFT_ALPHA)))
            .shadow(Self::sidebar_shadow())
            .py(px(4.0))
            .child({
                let mut menu = div();
                for theme_name in available_themes {
                    let is_selected = theme_name == self.selected_theme.as_ref();
                    let theme_name_shared: SharedString = theme_name.clone().into();
                    menu = menu.child(
                        div()
                            .px(px(12.0))
                            .py(px(6.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(0xffffff10)))
                            .when(is_selected, |s| s.bg(rgba(0xffffff15)))
                            .on_mouse_down(MouseButton::Left, {
                                let theme_name = theme_name_shared.clone();
                                cx.listener(move |this, _, _, cx| {
                                    this.switch_theme(theme_name.clone(), cx);
                                })
                            })
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(if is_selected {
                                        rgb(TEXT_PRIMARY)
                                    } else {
                                        rgb(TEXT_SUBTLE)
                                    })
                                    .when(is_selected, |s| {
                                        s.font_weight(rgpui::FontWeight::SEMIBOLD)
                                    })
                                    .child(theme_name.clone()),
                            )
                            .when(is_selected, |el| {
                                el.child(div().text_xs().text_color(rgb(TEXT_PRIMARY)).child("✓"))
                            }),
                    );
                }
                menu
            })
    }

    fn render_add_project(&self) -> impl IntoElement {
        div().px(px(16.0)).py(px(12.0)).child(
            div()
                .text_sm()
                .text_color(rgb(TEXT_SUBTLE))
                .cursor_pointer()
                .hover(|s| s.text_color(rgb(TEXT_PRIMARY)))
                .child("+ Add Project"),
        )
    }

    fn render_project(&self, project: &Project) -> impl IntoElement {
        div()
            .py(px(4.0))
            .child(
                // Project header
                div()
                    .px(px(8.0))
                    .py(px(6.0))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .cursor_pointer()
                    .rounded(px(4.0))
                    .hover(|s| s.bg(rgba(0xffffff10)))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(TEXT_SUBTLE))
                            .child(if project.expanded { "▼" } else { "▶" }),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(rgpui::FontWeight::MEDIUM)
                            .text_color(rgb(TEXT_PRIMARY))
                            .child(project.name.clone()),
                    ),
            )
            .when(project.expanded, |el| {
                let mut sessions_container = div().pl(px(16.0));

                if project.sessions.is_empty() {
                    sessions_container = sessions_container.child(
                        div()
                            .px(px(8.0))
                            .py(px(4.0))
                            .text_sm()
                            .text_color(rgb(TEXT_FAINT))
                            .child("No terminals"),
                    );
                } else {
                    for session in &project.sessions {
                        sessions_container = sessions_container.child(self.render_session(session));
                    }
                }

                el.child(sessions_container)
            })
    }

    fn render_session(&self, session: &TerminalSession) -> impl IntoElement {
        // Add status indicator color (green = running, gray = exited)
        let status_color = if session.active {
            rgb(0x98C379) // green
        } else {
            rgb(TEXT_FAINT) // gray
        };

        div()
            .px(px(8.0))
            .py(px(4.0))
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(4.0))
            .cursor_pointer()
            .when(session.active, |s| s.bg(rgba(0xffffff10)))
            .hover(|s| s.bg(rgba(0xffffff15)))
            .group("session")
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        // Status indicator dot
                        div().w(px(6.0)).h(px(6.0)).rounded_full().bg(status_color),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(TEXT_PRIMARY))
                            .max_w(px(140.0))
                            .overflow_x_hidden()
                            .child(session.name.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .opacity(0.0)
                    .group_hover("session", |s| s.opacity(1.0))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(TEXT_SUBTLE))
                            .px(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.text_color(rgb(TEXT_PRIMARY)))
                            .child("···"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(TEXT_SUBTLE))
                            .px(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.text_color(rgb(TEXT_PRIMARY)))
                            .child("×"),
                    ),
            )
    }

    fn render_recent_dirs_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let expanded = self.is_section_expanded(SidebarSection::RecentDirs);
        let mut container = div().child(self.render_section_header(SidebarSection::RecentDirs, cx));

        if expanded {
            if self.recent_dirs.entries.is_empty() {
                container = container.child(
                    div()
                        .px(px(16.0))
                        .py(px(4.0))
                        .text_sm()
                        .text_color(rgb(TEXT_FAINT))
                        .child("No recent directories"),
                );
            } else {
                for (i, entry) in self.recent_dirs.entries.iter().take(10).enumerate() {
                    let display = entry.display_name();
                    let _path = entry.path.clone();
                    let matches_search = self.search_query.is_empty()
                        || display
                            .to_lowercase()
                            .contains(&self.search_query.to_lowercase());
                    if matches_search {
                        container = container.child(
                            div()
                                .id(SharedString::from(format!("recent-{}", i)))
                                .px(px(16.0))
                                .py(px(4.0))
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .rounded(px(4.0))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(0xffffff10)))
                                .on_mouse_down(MouseButton::Left, {
                                    cx.listener(move |this, _, window, cx| {
                                        this.create_new_tab(window, cx);
                                    })
                                })
                                .child(div().text_xs().text_color(rgb(TEXT_FAINT)).child("📁"))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(TEXT_PRIMARY))
                                        .overflow_x_hidden()
                                        .max_w(px(140.0))
                                        .child(display),
                                ),
                        );
                    }
                }
            }
        }

        container
    }

    fn render_workspaces_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let expanded = self.is_section_expanded(SidebarSection::Workspaces);
        let mut container = div().child(self.render_section_header(SidebarSection::Workspaces, cx));

        if expanded {
            if self.saved_workspaces.is_empty() {
                container = container.child(
                    div()
                        .px(px(16.0))
                        .py(px(4.0))
                        .text_sm()
                        .text_color(rgb(TEXT_FAINT))
                        .child("No saved workspaces"),
                );
            } else {
                for (i, ws) in self.saved_workspaces.iter().enumerate() {
                    let name = ws.name.clone();
                    let matches_search = self.search_query.is_empty()
                        || name
                            .to_lowercase()
                            .contains(&self.search_query.to_lowercase());
                    if matches_search {
                        container = container.child(
                            div()
                                .id(SharedString::from(format!("ws-{}", i)))
                                .group(SharedString::from(format!("ws-group-{}", i)))
                                .px(px(16.0))
                                .py(px(4.0))
                                .flex()
                                .items_center()
                                .justify_between()
                                .rounded(px(4.0))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(0xffffff10)))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.0))
                                        .child(
                                            div().text_xs().text_color(rgb(TEXT_FAINT)).child("▢"),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(rgb(TEXT_PRIMARY))
                                                .overflow_x_hidden()
                                                .max_w(px(120.0))
                                                .child(name.clone()),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(TEXT_FAINT))
                                        .child(format!("{} tabs", ws.tabs.len())),
                                ),
                        );
                    }
                }
            }

            // Save current workspace button
            container = container.child(
                div()
                    .id("save-workspace")
                    .px(px(16.0))
                    .py(px(4.0))
                    .text_sm()
                    .text_color(rgb(TEXT_FAINT))
                    .cursor_pointer()
                    .hover(|s| s.text_color(rgb(TEXT_SUBTLE)))
                    .child("+ Save current workspace")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.save_current_workspace(cx);
                        }),
                    ),
            );
        }

        container
    }

    fn render_sidebar_footer(&self) -> impl IntoElement {
        let session_count = self.tabs.len();
        div()
            .h(px(32.0))
            .px(px(16.0))
            .flex()
            .items_center()
            .border_t_1()
            .border_color(rgba(rgba_u32(BORDER_SOFT, BORDER_SOFT_ALPHA)))
            .child(div().text_xs().text_color(rgb(TEXT_FAINT)).child(format!(
                "{} active session{}",
                session_count,
                if session_count == 1 { "" } else { "s" }
            )))
    }

    fn save_current_workspace(&mut self, cx: &mut Context<Self>) {
        let tabs: Vec<SavedTab> = self
            .tabs
            .iter()
            .map(|tab| SavedTab {
                title: tab.title.to_string(),
                working_dir: None, // TODO: Track working dir per tab
            })
            .collect();

        let workspace = WorkspaceLayout {
            name: format!("Workspace {}", self.saved_workspaces.len() + 1),
            tabs,
            active_tab_index: self.active_tab_index,
            sidebar_width: self.sidebar_width,
        };

        persistence::save_workspace(&workspace);
        self.saved_workspaces.push(workspace);
        cx.notify();
    }

    fn render_terminal_container(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let content_left = if self.sidebar_visible {
            self.sidebar_width + SIDEBAR_INSET + SIDEBAR_GAP
        } else {
            0.0
        };

        let pane_element: Option<AnyElement> =
            if let Some(tab) = self.tabs.get(self.active_tab_index) {
                Some(self.render_pane_node(&tab.root, "root", window, cx))
            } else {
                None
            };

        div()
            .id("terminal-container")
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .left(px(content_left))
            .flex()
            .flex_col()
            .when_some(pane_element, |el, pane_el| {
                el.child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .py(px(16.0))
                        .px(px(8.0))
                        .child(pane_el),
                )
            })
            .when(self.tabs.is_empty(), |el| {
                el.flex()
                    .items_center()
                    .justify_center()
                    .child(div().text_color(rgb(TEXT_FAINT)).child("Loading terminal…"))
            })
    }
}

impl Render for AgentTermApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab_index = self.active_tab_index;

        v_flex()
            .size_full()
            .child(
                TitleBar::new().child(
                    h_flex().flex_1().min_w(px(0.0)).overflow_x_hidden().child(
                        TabBar::new("terminal-tabs")
                            .flex_1()
                            .min_w(px(0.0))
                            .track_scroll(&self.tab_bar_scroll_handle)
                            .prefix(
                                h_flex()
                                    .mx_1()
                                    .child(
                                        Button::new("back")
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::ArrowLeft)
                                            .on_click(cx.listener(|this, _, _, _| {
                                                this.scroll_tab_bar_left();
                                            })),
                                    )
                                    .child(
                                        Button::new("forward")
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::ArrowRight)
                                            .on_click(cx.listener(|this, _, _, _| {
                                                this.scroll_tab_bar_right();
                                            })),
                                    ),
                            )
                            .menu(true)
                            .selected_index(active_tab_index)
                            .on_click(cx.listener(|this, ix: &usize, window, cx| {
                                this.switch_to_tab(*ix, window, cx);
                            }))
                            .children(self.tabs.iter().enumerate().map(|(ix, tab)| {
                                Tab::new().label(tab.title.clone()).suffix(
                                    div()
                                        .id(("close-tab", ix))
                                        .px(px(4.0))
                                        .cursor_pointer()
                                        .text_color(rgb(TEXT_SUBTLE))
                                        .hover(|s| s.text_color(rgb(TEXT_PRIMARY)))
                                        .child("×")
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation()
                                        })
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.close_tab(ix, window, cx);
                                        })),
                                )
                            }))
                            .suffix(
                                div()
                                    .id("add-tab")
                                    .px(px(8.0))
                                    .py(px(4.0))
                                    .cursor_pointer()
                                    .text_color(rgb(TEXT_SUBTLE))
                                    .hover(|s| s.text_color(rgb(TEXT_PRIMARY)).bg(rgba(0xffffff10)))
                                    .child("+")
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation();
                                    })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.create_new_tab(window, cx);
                                    })),
                            ),
                    ),
                ),
            )
            .child(
                div()
                    .id("agent-term-app")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .relative()
                    // .bg(rgba(rgba_u32(SURFACE_ROOT, SURFACE_ROOT_ALPHA)))
                    .track_focus(&self.focus_handle)
                    .on_action(cx.listener(Self::toggle_sidebar))
                    .on_action(cx.listener(Self::on_new_tab))
                    .on_action(cx.listener(Self::on_close_tab))
                    .on_action(cx.listener(Self::on_next_tab))
                    .on_action(cx.listener(Self::on_previous_tab))
                    .on_action(cx.listener(Self::on_split_right))
                    .on_action(cx.listener(Self::on_split_down))
                    .on_action(cx.listener(Self::on_dump_text))
                    .on_mouse_move(cx.listener(Self::update_sidebar_resize))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::stop_sidebar_resize))
                    .child(self.render_terminal_container(window, cx))
                    .when(self.sidebar_visible, |el| {
                        el.child(self.render_sidebar_shell(cx))
                    }),
            )
    }
}

impl Focusable for AgentTermApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
