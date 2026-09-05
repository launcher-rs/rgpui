#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Context, Global, Menu, MenuItem, SharedString, SystemMenuType, Window, WindowOptions,
    actions, div, prelude::*,
};
use rgpui_platform::application;

struct SetMenus;

impl Render for SetMenus {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 将已存储的菜单读回并展示：原生菜单栏仅在 macOS 显示为系统全局菜单；
        // 在 Windows/Linux 上 `set_menus` 只存储数据（可通过 `get_menus` 读回），
        // 不会渲染原生菜单栏，因此这里把存储内容直接画出来作为反馈。
        let stored = cx.get_menus().unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .bg(rgpui::white())
            .size_full()
            .p_8()
            .gap_4()
            .text_color(rgpui::black())
            .child(div().text_xl().child("Set Menus Example"))
            .child(
                div()
                    .text_sm()
                    .child("原生应用菜单栏仅在 macOS 显示为系统全局菜单；在 Windows/Linux 上 set_menus 仅存储数据（可用 get_menus 读回），不会渲染原生菜单栏。",
                    ),
            )
            .child(
                div()
                    .id("toggle-view-mode")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .bg(rgpui::black())
                    .text_color(rgpui::white())
                    .text_sm()
                    .child("切换视图模式（List / Grid，将刷新下方读回结果）")
                    .on_click(cx.listener(|_, _, _, cx| {
                        toggle_check(&ToggleCheck, cx);
                        cx.refresh_windows();
                    })),
            )
            .child(div().text_base().child(format!(
                "已存储的顶层菜单（get_menus 读回，共 {} 个）：",
                stored.len()
            )))
            .children(stored.iter().map(|menu| {
                div().text_sm().child(format!(
                    "· {}{}（{} 项）",
                    menu.name,
                    if menu.disabled { " [禁用]" } else { "" },
                    menu.items.len()
                ))
            }))
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        cx.set_global(AppState::new());

        // Bring the menu bar to the foreground (so you can see the menu bar)
        cx.activate(true);
        // Register the `quit` function so it can be referenced
        // by the `MenuItem::action` in the menu bar
        cx.on_action(quit);
        cx.on_action(toggle_check);
        // Add menu items
        set_app_menus(cx);
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| SetMenus {}))
            .unwrap();
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

#[derive(PartialEq)]
enum ViewMode {
    List,
    Grid,
}

impl ViewMode {
    fn toggle(&mut self) {
        *self = match self {
            ViewMode::List => ViewMode::Grid,
            ViewMode::Grid => ViewMode::List,
        }
    }
}

impl From<ViewMode> for SharedString {
    fn from(val: ViewMode) -> Self {
        match val {
            ViewMode::List => "List",
            ViewMode::Grid => "Grid",
        }
        .into()
    }
}

struct AppState {
    view_mode: ViewMode,
}

impl AppState {
    fn new() -> Self {
        Self {
            view_mode: ViewMode::List,
        }
    }
}

impl Global for AppState {}

fn set_app_menus(cx: &mut App) {
    let app_state = cx.global::<AppState>();
    cx.set_menus([Menu::new("set_menus").items([
        MenuItem::os_submenu("Services", SystemMenuType::Services),
        MenuItem::separator(),
        MenuItem::action("Disabled Item", rgpui::NoAction).disabled(true),
        MenuItem::submenu(Menu::new("Disabled Submenu").disabled(true)),
        MenuItem::separator(),
        MenuItem::action("List Mode", ToggleCheck).checked(app_state.view_mode == ViewMode::List),
        MenuItem::submenu(
            Menu::new("Mode").items([
                MenuItem::action(ViewMode::List, ToggleCheck)
                    .checked(app_state.view_mode == ViewMode::List),
                MenuItem::action(ViewMode::Grid, ToggleCheck)
                    .checked(app_state.view_mode == ViewMode::Grid),
            ]),
        ),
        MenuItem::separator(),
        MenuItem::action("Quit", Quit),
    ])]);
}

// Associate actions using the `actions!` macro (or `Action` derive macro)
actions!(set_menus, [Quit, ToggleCheck]);

// Define the quit function that is registered with the App
fn quit(_: &Quit, cx: &mut App) {
    println!("Gracefully quitting the application...");
    cx.quit();
}

fn toggle_check(_: &ToggleCheck, cx: &mut App) {
    let app_state = cx.global_mut::<AppState>();
    app_state.view_mode.toggle();
    set_app_menus(cx);
}
