//! 内置图标总览：全部 `IconName` 变体 + 三种图标来源用法。
//!
//! 三种用法：
//! 1. 内置命名图标 `Icon::new(IconName::Check)`——SVG 字节编译期嵌入，不依赖运行时资源目录；
//! 2. 自定义路径 + 数据 `Icon::default().path(..).data(..)`——自备 SVG 字节；
//! 3. 自定义 `IconNamed` 实现——任意类型都能当图标用。
//!
//! 网格按变体名排序，每格显示图标与变体名。`cargo test -p rgpui --test guards`
//! 会校验本文件与 `crates/rgpui/assets/icons` 的清单一致（漏一个都会失败）。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, Hsla, Icon, IconName, IconNamed, Render, ScrollHandle, SharedString,
    Window, WindowBounds, WindowOptions, div, h_flex, prelude::*, px, size, v_flex,
};
use rgpui_platform::application;

/// 自定义 `IconNamed` 示例：把任意 SVG 当成图标使用。
struct StarGlyph;

impl IconNamed for StarGlyph {
    /// 资源路径（仅作标识与缓存键，真正渲染用 [`Self::bytes`]）。
    fn path(&self) -> SharedString {
        "icons/star.svg".into()
    }

    /// 编译期嵌入的 SVG 原始字节。
    fn bytes(&self) -> &'static [u8] {
        include_bytes!("../../../../crates/rgpui/assets/icons/star.svg")
    }
}

/// 自定义路径 + 数据用法演示的 SVG 字节。
const STAR_SVG: &[u8] = include_bytes!("../../../../crates/rgpui/assets/icons/star.svg");

/// 全部内置图标：枚举值与变体名（同名），按变体名排序。
const ICONS: &[(IconName, &str)] = &[
    (IconName::ALargeSmall, "ALargeSmall"),
    (IconName::ArrowDown, "ArrowDown"),
    (IconName::ArrowLeft, "ArrowLeft"),
    (IconName::ArrowRight, "ArrowRight"),
    (IconName::ArrowUp, "ArrowUp"),
    (IconName::Asterisk, "Asterisk"),
    (IconName::Battery, "Battery"),
    (IconName::BatteryCharging, "BatteryCharging"),
    (IconName::BatteryFull, "BatteryFull"),
    (IconName::BatteryLow, "BatteryLow"),
    (IconName::BatteryMedium, "BatteryMedium"),
    (IconName::BatteryWarning, "BatteryWarning"),
    (IconName::Bell, "Bell"),
    (IconName::BookOpen, "BookOpen"),
    (IconName::Bot, "Bot"),
    (IconName::Building2, "Building2"),
    (IconName::Calendar, "Calendar"),
    (IconName::CaseSensitive, "CaseSensitive"),
    (IconName::ChartPie, "ChartPie"),
    (IconName::Check, "Check"),
    (IconName::ChevronDown, "ChevronDown"),
    (IconName::ChevronLeft, "ChevronLeft"),
    (IconName::ChevronRight, "ChevronRight"),
    (IconName::ChevronUp, "ChevronUp"),
    (IconName::ChevronsUpDown, "ChevronsUpDown"),
    (IconName::CircleCheck, "CircleCheck"),
    (IconName::CircleUser, "CircleUser"),
    (IconName::CircleX, "CircleX"),
    (IconName::Close, "Close"),
    (IconName::Copy, "Copy"),
    (IconName::Cpu, "Cpu"),
    (IconName::Dash, "Dash"),
    (IconName::Delete, "Delete"),
    (IconName::Ellipsis, "Ellipsis"),
    (IconName::EllipsisVertical, "EllipsisVertical"),
    (IconName::ExternalLink, "ExternalLink"),
    (IconName::Eye, "Eye"),
    (IconName::EyeOff, "EyeOff"),
    (IconName::File, "File"),
    (IconName::Folder, "Folder"),
    (IconName::FolderClosed, "FolderClosed"),
    (IconName::FolderOpen, "FolderOpen"),
    (IconName::Frame, "Frame"),
    (IconName::GalleryVerticalEnd, "GalleryVerticalEnd"),
    (IconName::Github, "Github"),
    (IconName::Globe, "Globe"),
    (IconName::HardDrive, "HardDrive"),
    (IconName::Heart, "Heart"),
    (IconName::HeartOff, "HeartOff"),
    (IconName::Inbox, "Inbox"),
    (IconName::Info, "Info"),
    (IconName::Inspector, "Inspector"),
    (IconName::LayoutDashboard, "LayoutDashboard"),
    (IconName::Loader, "Loader"),
    (IconName::LoaderCircle, "LoaderCircle"),
    (IconName::Map, "Map"),
    (IconName::Maximize, "Maximize"),
    (IconName::MemoryStick, "MemoryStick"),
    (IconName::Menu, "Menu"),
    (IconName::Minimize, "Minimize"),
    (IconName::Minus, "Minus"),
    (IconName::Moon, "Moon"),
    (IconName::Network, "Network"),
    (IconName::Palette, "Palette"),
    (IconName::PanelBottom, "PanelBottom"),
    (IconName::PanelBottomOpen, "PanelBottomOpen"),
    (IconName::PanelLeft, "PanelLeft"),
    (IconName::PanelLeftClose, "PanelLeftClose"),
    (IconName::PanelLeftOpen, "PanelLeftOpen"),
    (IconName::PanelRight, "PanelRight"),
    (IconName::PanelRightClose, "PanelRightClose"),
    (IconName::PanelRightOpen, "PanelRightOpen"),
    (IconName::Pause, "Pause"),
    (IconName::Play, "Play"),
    (IconName::Plus, "Plus"),
    (IconName::Redo, "Redo"),
    (IconName::Redo2, "Redo2"),
    (IconName::Replace, "Replace"),
    (IconName::ResizeCorner, "ResizeCorner"),
    (IconName::Search, "Search"),
    (IconName::Settings, "Settings"),
    (IconName::Settings2, "Settings2"),
    (IconName::SortAscending, "SortAscending"),
    (IconName::SortDescending, "SortDescending"),
    (IconName::SquareTerminal, "SquareTerminal"),
    (IconName::Star, "Star"),
    (IconName::StarFill, "StarFill"),
    (IconName::StarOff, "StarOff"),
    (IconName::Sun, "Sun"),
    (IconName::ThumbsDown, "ThumbsDown"),
    (IconName::ThumbsUp, "ThumbsUp"),
    (IconName::TriangleAlert, "TriangleAlert"),
    (IconName::Undo, "Undo"),
    (IconName::Undo2, "Undo2"),
    (IconName::User, "User"),
    (IconName::WindowClose, "WindowClose"),
    (IconName::WindowMaximize, "WindowMaximize"),
    (IconName::WindowMinimize, "WindowMinimize"),
    (IconName::WindowRestore, "WindowRestore"),
];

/// 图标网格演示视图。
struct IconsDemo {
    /// 整页滚动句柄。
    scroll: ScrollHandle,
}

impl IconsDemo {
    /// 创建演示视图。
    fn new() -> Self {
        Self {
            scroll: ScrollHandle::new(),
        }
    }
}

/// 单个图标单元格：图标 + 变体名。
fn icon_cell(
    icon: IconName,
    name: &'static str,
    foreground: Hsla,
    muted: Hsla,
    border: Hsla,
) -> impl IntoElement {
    v_flex()
        .id(name)
        .w(px(136.0))
        .items_center()
        .justify_center()
        .gap_2()
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(border)
        .child(Icon::new(icon).with_size(px(28.0)).text_color(foreground))
        .child(div().text_xs().text_color(muted).child(name))
}

/// 用法卡片：一个示例图标 + 标题 + 调用代码。
fn usage_card(
    icon: Icon,
    title: &'static str,
    code: &'static str,
    muted: Hsla,
    border: Hsla,
) -> impl IntoElement {
    v_flex()
        .flex_1()
        .gap_2()
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(border)
        .child(icon.with_size(px(28.0)))
        .child(div().text_sm().child(title))
        .child(div().text_xs().text_color(muted).child(code))
}

impl Render for IconsDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let foreground: Hsla = theme.tokens.foreground.into();
        let muted: Hsla = theme.tokens.muted_foreground.into();
        let border: Hsla = theme.tokens.border.into();
        let scroll = self.scroll.clone();

        div()
            .id("icons-page")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&scroll)
            .bg(theme.tokens.background)
            .child(
                v_flex()
                    .gap_6()
                    .p(px(24.0))
                    .child(div().text_xl().child("图标（Icon）"))
                    .child(div().text_sm().text_color(muted).child(format!(
                        "内置 IconName 共 {} 个，下面按变体名排序逐个渲染。",
                        ICONS.len()
                    )))
                    .child(h_flex().gap_4().children([
                        usage_card(
                            Icon::new(IconName::Check),
                            "内置命名图标",
                            "Icon::new(IconName::Check)",
                            muted,
                            border,
                        ),
                        usage_card(
                            Icon::default().path("icons/star.svg").data(STAR_SVG),
                            "自定义路径 + 数据",
                            "Icon::default().path(..).data(include_bytes!(..))",
                            muted,
                            border,
                        ),
                        usage_card(
                            Icon::new(StarGlyph),
                            "自定义 IconNamed",
                            "impl IconNamed for MyIcon { path, bytes }",
                            muted,
                            border,
                        ),
                    ]))
                    .child(
                        h_flex().flex_wrap().gap_3().children(
                            ICONS.iter().cloned().map(|(icon, name)| {
                                icon_cell(icon, name, foreground, muted, border)
                            }),
                        ),
                    ),
            )
    }
}

/// 启动演示窗口。
fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(980.0), px(760.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_window, cx| cx.new(|_| IconsDemo::new()),
        )
        .expect("图标示例：窗口创建失败");
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
/// WASM 启动入口。
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
