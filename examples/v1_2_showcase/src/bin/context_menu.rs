//! 右键菜单演示（§H）：默认菜单 + 追加自定义 + 完全接管 + 总开关。
//!
//! 三个标签页各放一个全屏多行编辑器：
//! - 默认菜单：零配置，右键即有（剪切/复制/粘贴/全选/撤销/重做）。
//! - 追加自定义：`context_menu_extra` 追加转大写/转小写/清空 + 总开关。
//! - 完全接管：`context_menu_override`（自定义头 + 复用默认项 + 清空）。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, KeyBinding, PopupMenuItem, Render, Toggle, Window, WindowBounds,
    WindowOptions, div,
    input_ui::{
        Backspace, Copy, Cut, Delete, Enter, Escape, Input, InputState, MoveDown, MoveEnd,
        MoveHome, MoveLeft, MoveRight, MoveUp, Paste, Redo, SelectAll, Undo,
    },
    prelude::*,
    px, size,
    tabs::{Tab, TabBar},
    v_flex,
};
use rgpui_platform::application;

const DEFAULT_SAMPLE: &str = "Right-click anywhere in this editor to open the default context menu.\n\
    \n\
    Cut / Copy / Paste / Select All / Undo / Redo are enabled automatically\n\
    based on the current state (selection, undo history, disabled).\n\
    \n\
    Try this: select some text first, then right-click.";

const EXTRA_SAMPLE: &str = "Select a WORD below, right-click, and choose UPPERCASE or lowercase.\n\
    \n\
    hello rgpui context menu\n\
    The quick brown fox jumps over the lazy dog\n\
    \n\
    The toggle above turns the whole menu on and off.";

const OVERRIDE_SAMPLE: &str = "This editor takes over the menu completely.\n\
    \n\
    The custom header and the Clear item are hand-built,\n\
    while the middle section reuses the default menu builder.\n\
    \n\
    Right-click to inspect: hello override world";

struct ContextMenuDemo {
    selected_tab: usize,
    default_input: rgpui::Entity<InputState>,
    extra_input: rgpui::Entity<InputState>,
    override_input: rgpui::Entity<InputState>,
    menu_enabled: bool,
}

impl ContextMenuDemo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let default_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx).multi_line(true);
            state.replace(DEFAULT_SAMPLE, window, cx);
            state
        });
        let extra_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx).multi_line(true);
            state.replace(EXTRA_SAMPLE, window, cx);
            state
        });
        let override_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx).multi_line(true);
            state.replace(OVERRIDE_SAMPLE, window, cx);
            state
        });
        Self {
            selected_tab: 0,
            default_input,
            extra_input,
            override_input,
            menu_enabled: true,
        }
    }

    /// 追加页编辑器：默认项保留，自定义项跟在分隔符后。
    fn render_extra_input(&self) -> impl IntoElement {
        let state = self.extra_input.clone();
        let upper_state = state.clone();
        let lower_state = state.clone();
        let clear_state = state.clone();
        Input::new(&self.extra_input)
            .flex_1()
            .show_context_menu(self.menu_enabled)
            .context_menu_extra(move |menu, _, _, _| {
                let upper_state = upper_state.clone();
                let lower_state = lower_state.clone();
                let clear_state = clear_state.clone();
                menu.item(PopupMenuItem::new("转为大写 UPPERCASE").on_click(
                    move |_, window, cx| {
                        upper_state.update(cx, |state, cx| {
                            let selected = state.selected_value().to_string();
                            if !selected.is_empty() {
                                state.replace(selected.to_uppercase(), window, cx);
                            }
                        });
                    },
                ))
                .item(
                    PopupMenuItem::new("转为小写 lowercase").on_click(move |_, window, cx| {
                        lower_state.update(cx, |state, cx| {
                            let selected = state.selected_value().to_string();
                            if !selected.is_empty() {
                                state.replace(selected.to_lowercase(), window, cx);
                            }
                        });
                    }),
                )
                .item(PopupMenuItem::new("清空").on_click(
                    move |_, window, cx| {
                        clear_state.update(cx, |state, cx| state.replace_all("", window, cx));
                    },
                ))
            })
    }

    /// 接管页编辑器：默认项不要，全部自己说了算（这里复用默认项再加一段）。
    fn render_override_input(&self) -> impl IntoElement {
        let clear_state = self.override_input.clone();
        Input::new(&self.override_input)
            .flex_1()
            .context_menu_override(move |menu, state, _, cx| {
                let menu = menu.label("自定义菜单（接管模式）");
                let menu = InputState::build_default_context_menu(menu, &state, cx);
                let clear_state = clear_state.clone();
                menu.separator()
                    .item(PopupMenuItem::new("清空").on_click(move |_, window, cx| {
                        clear_state.update(cx, |state, cx| state.replace_all("", window, cx));
                    }))
            })
    }
}

impl Render for ContextMenuDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let demo = cx.entity();
        v_flex()
            .size_full()
            .p(px(12.0))
            .gap(px(8.0))
            .child(
                TabBar::new("context-menu-demo-tabs")
                    .selected_index(self.selected_tab)
                    .on_click(cx.listener(|this, ix: &usize, _, cx| {
                        this.selected_tab = *ix;
                        cx.notify();
                    }))
                    .child(Tab::new().label("默认菜单"))
                    .child(Tab::new().label("追加自定义"))
                    .child(Tab::new().label("完全接管")),
            )
            .child(match self.selected_tab {
                0 => v_flex()
                    .flex_1()
                    .gap(px(8.0))
                    .child(div().text_sm().child("零配置：右键即有默认菜单"))
                    .child(Input::new(&self.default_input).flex_1())
                    .into_any_element(),
                1 => v_flex()
                    .flex_1()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_sm()
                                    .child("默认项 + 转大写/转小写/清空（先选中英文单词再右键）"),
                            )
                            .child(
                                Toggle::new("启用右键菜单")
                                    .pressed(self.menu_enabled)
                                    .on_change(move |pressed, _, cx| {
                                        demo.update(cx, |this, cx| {
                                            this.menu_enabled = pressed;
                                            cx.notify();
                                        });
                                    }),
                            ),
                    )
                    .child(self.render_extra_input())
                    .into_any_element(),
                _ => v_flex()
                    .flex_1()
                    .gap(px(8.0))
                    .child(div().text_sm().child("自定义头 + 复用默认项 + 清空"))
                    .child(self.render_override_input())
                    .into_any_element(),
            })
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        // 输入框编辑键位（应用级注册；secondary = macOS Cmd / 其他平台 Ctrl）。
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, None),
            KeyBinding::new("delete", Delete, None),
            KeyBinding::new("left", MoveLeft, None),
            KeyBinding::new("right", MoveRight, None),
            KeyBinding::new("up", MoveUp, None),
            KeyBinding::new("down", MoveDown, None),
            KeyBinding::new("home", MoveHome, None),
            KeyBinding::new("end", MoveEnd, None),
            KeyBinding::new(
                "enter",
                Enter {
                    secondary: false,
                    shift: false,
                },
                None,
            ),
            KeyBinding::new("escape", Escape, None),
            KeyBinding::new("secondary-a", SelectAll, None),
            KeyBinding::new("secondary-c", Copy, None),
            KeyBinding::new("secondary-x", Cut, None),
            KeyBinding::new("secondary-v", Paste, None),
            KeyBinding::new("secondary-z", Undo, None),
            KeyBinding::new("secondary-shift-z", Redo, None),
        ]);
        let bounds = Bounds::centered(None, size(px(860.0), px(620.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| ContextMenuDemo::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
