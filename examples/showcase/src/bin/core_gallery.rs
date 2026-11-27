//! 基础 / 组件 / 绘制 / 调试 / 浮层 示例集合。
//!
//! 用法：cargo run -p showcase --bin core_gallery -- <demo>
//! 可用 demo：hello_world / a11y / theme / animation / daemon_app / icons / component_gallery / extended_components / scrollable / sidebar / tabs / fps_hud / inspector / inspector_custom / gradient / painting / pattern / anchor / popover
//! 不带参数时默认运行 hello_world，传 --list 打印清单。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    AnyView, App, AssetSource, Bounds, Button, ButtonVariants, Context, IntoElement, ParentElement,
    Render, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use rgpui_platform::application;

/// Gallery 资源来源：从文件系统加载示例所需的 SVG / 图片资源。
struct GalleryAssets;

impl AssetSource for GalleryAssets {
    fn load(&self, path: &str) -> rgpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        std::fs::read(path)
            .map(|bytes| Some(std::borrow::Cow::Owned(bytes)))
            .map_err(Into::into)
    }

    fn list(&self, path: &str) -> rgpui::Result<Vec<rgpui::SharedString>> {
        Ok(std::fs::read_dir(path)?
            .filter_map(|entry| {
                Some(rgpui::SharedString::from(
                    entry.ok()?.path().to_string_lossy().into_owned(),
                ))
            })
            .collect())
    }
}

mod hello_world {

    //! 最小可运行窗口：展示用 `application().run` 启动应用、`cx.open_window` 开窗口、
    //! `cx.new` 创建根视图、`Render` 渲染一个文本与方块的最基本流程。
    //! 用法：左侧 tab 选「hello_world」，窗口里显示问候语与一个方块。
    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::{
        App, Bounds, Context, SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions,
        div, prelude::*, px, rgb, size,
    };
    use rgpui_platform::application;

    struct HelloWorld {
        text: SharedString,
    }

    impl Render for HelloWorld {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .bg(rgb(0x505050))
                .size(px(500.0))
                .justify_center()
                .items_center()
                .shadow_lg()
                .border_1()
                .border_color(rgb(0x0000ff))
                .text_xl()
                .text_color(rgb(0xffffff))
                .child(format!("你好，{}！", self.text))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            div()
                                .size_8()
                                .bg(rgpui::red())
                                .border_1()
                                .border_dashed()
                                .rounded_md()
                                .border_color(rgpui::white()),
                        )
                        .child(
                            div()
                                .size_8()
                                .bg(rgpui::green())
                                .border_1()
                                .border_dashed()
                                .rounded_md()
                                .border_color(rgpui::white()),
                        )
                        .child(
                            div()
                                .size_8()
                                .bg(rgpui::blue())
                                .border_1()
                                .border_dashed()
                                .rounded_md()
                                .border_color(rgpui::white()),
                        )
                        .child(
                            div()
                                .size_8()
                                .bg(rgpui::yellow())
                                .border_1()
                                .border_dashed()
                                .rounded_md()
                                .border_color(rgpui::white()),
                        )
                        .child(
                            div()
                                .size_8()
                                .bg(rgpui::black())
                                .border_1()
                                .border_dashed()
                                .rounded_md()
                                .rounded_md()
                                .border_color(rgpui::white()),
                        )
                        .child(
                            div()
                                .size_8()
                                .bg(rgpui::white())
                                .border_1()
                                .border_dashed()
                                .rounded_md()
                                .border_color(rgpui::black()),
                        ),
                )
        }
    }

    pub fn run_example() {
        #[cfg(not(target_family = "wasm"))]
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    // 通过 titlebar 选项设置标题栏显示的名称（WM_NAME），
                    // Windows/macOS/Linux 各平台均生效
                    titlebar: Some(TitlebarOptions {
                        title: Some("你好，世界 - rgpui".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|_| HelloWorld {
                        text: "世界".into(),
                    })
                },
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| HelloWorld {
            text: "世界".into(),
        })
        .into()
    }
}

mod a11y {

    //! 无障碍（AccessKit）标注示例：展示 `id` / `role` / `aria_label` 等语义属性，
    //! 让辅助技术（读屏软件）能识别界面元素。
    //! 用法：左侧 tab 选「a11y」。窗口内有可聚焦的计数器（role=SpinButton）、重置按钮、
    //! 功能开关与一个待办列表；用 Tab / Shift+Tab 切换焦点，每项都有读屏标签。
    #![cfg_attr(target_family = "wasm", no_main)]

    //! Accessibility (AccessKit) demo app.
    //!
    //! Run with: `cargo run -p showcase --bin a11y`
    //!
    //! Or on Linux: `cargo run -p showcase --bin a11y`
    //!
    //! This app uses RGPUI's accessibility APIs to attach structured information to
    //! the element tree, which allows assistive technology to see and interact with
    //! the UI programmatically.
    //!
    //! The app behaves as follows:
    //! - It opens a single window.
    //! - The window's title is "rgpui 无障碍演示".
    //! - The window has a sequence of UI elements, stacked vertically:
    //!   - A heading with the text "无障碍演示".
    //!   - A row containing two elements:
    //!     - A spin button (role `SpinButton`) labelled "计数器：<n>", where
    //!       `<n>` is the current count. It supports `Increment` and `Decrement`
    //!       accessible actions, and also increments on click. The numeric value
    //!       is clamped to a minimum of 0.
    //!     - A button labelled "重置计数器" that resets the count to 0.
    //!   - A row containing two elements:
    //!     - A switch, that can be toggled, and starts disabled. Toggling the switch
    //!       does nothing.
    //!     - The text "启用功能".
    //!   - A "to-do" list, with three items, each represented with a `Text` element:
    //!     - "1. Write code"
    //!     - "2. Run tests"
    //!     - "3. Ship it"

    use rgpui::{
        AccessibleAction, App, Bounds, Context, FocusHandle, KeyBinding, Role, SharedString,
        Toggled, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
        text,
    };
    use rgpui_platform::application;

    actions!(a11y_example, [Tab, TabPrev]);

    struct A11yDemo {
        focus_handle: FocusHandle,
        count: i32,
        enabled: bool,
    }

    impl A11yDemo {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            Self {
                focus_handle,
                count: 0,
                enabled: false,
            }
        }
    }

    impl Render for A11yDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .id("root")
                .role(Role::Application)
                .aria_label("无障碍演示")
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(|_, _: &Tab, window, cx| window.focus_next(cx)))
                .on_action(cx.listener(|_, _: &TabPrev, window, cx| window.focus_prev(cx)))
                .size_full()
                .flex()
                .flex_col()
                .gap_4()
                .p_4()
                .bg(rgb(0x1e1e2e))
                .text_color(rgb(0xcdd6f4))
                // Heading
                .child(
                    div()
                        .id("heading")
                        .role(Role::Heading)
                        .aria_level(1)
                        .aria_label("无障碍演示")
                        .text_xl()
                        .font_weight(rgpui::FontWeight::BOLD)
                        .child(text!("无障碍演示")),
                )
                // Counter  — uses a SpinButton role with Increment/Decrement
                // actions so screen readers can adjust the value directly.
                // Click also works via the built-in handler.
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .id("counter")
                                .focusable()
                                .tab_stop(true)
                                .role(Role::SpinButton)
                                .aria_label(SharedString::from(format!("计数器：{}", self.count)))
                                .aria_numeric_value(self.count as f64)
                                .aria_min_numeric_value(0.0)
                                .on_a11y_action(AccessibleAction::Increment, {
                                    let this = cx.entity().downgrade();
                                    move |_, _, cx| {
                                        this.update(cx, |this, cx| {
                                            this.count += 1;
                                            cx.notify();
                                        })
                                        .ok();
                                    }
                                })
                                .on_a11y_action(AccessibleAction::Decrement, {
                                    let this = cx.entity().downgrade();
                                    move |_, _, cx| {
                                        this.update(cx, |this, cx| {
                                            this.count = (this.count - 1).max(0);
                                            cx.notify();
                                        })
                                        .ok();
                                    }
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.count += 1;
                                    cx.notify();
                                }))
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(0x89b4fa))
                                .text_color(rgb(0x1e1e2e))
                                .cursor_pointer()
                                .child(text!(format!("计数：{}", self.count))),
                        )
                        .child(
                            div()
                                .id("reset")
                                .focusable()
                                .tab_stop(true)
                                .role(Role::Button)
                                .aria_label("重置计数器")
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(0x585b70))
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.count = 0;
                                    cx.notify();
                                }))
                                .child(text!("重置")),
                        ),
                )
                // A toggle switch
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("toggle")
                                .focusable()
                                .tab_stop(true)
                                .role(Role::Switch)
                                .aria_label("启用功能")
                                .aria_toggled(if self.enabled {
                                    Toggled::True
                                } else {
                                    Toggled::False
                                })
                                .w(px(44.))
                                .h(px(24.))
                                .rounded_full()
                                .cursor_pointer()
                                .when(self.enabled, |el| el.bg(rgb(0x89b4fa)))
                                .when(!self.enabled, |el| el.bg(rgb(0x585b70)))
                                .child(
                                    div()
                                        .size(px(20.))
                                        .rounded_full()
                                        .bg(rgpui::white())
                                        .mt(px(2.))
                                        .when(self.enabled, |el| el.ml(px(22.)))
                                        .when(!self.enabled, |el| el.ml(px(2.))),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.enabled = !this.enabled;
                                    cx.notify();
                                })),
                        )
                        .child(text!("启用功能")),
                )
                // A short list
                .child(
                    div()
                        .id("task-list")
                        .role(Role::List)
                        .aria_label("任务")
                        .flex()
                        .flex_col()
                        .gap_1()
                        .children(["编写代码", "运行测试", "发布"].iter().enumerate().map(
                            |(i, label)| {
                                div()
                                    .id(("task", i))
                                    .role(Role::ListItem)
                                    .aria_label(SharedString::from(*label))
                                    .aria_position_in_set(i + 1)
                                    .aria_size_of_set(3)
                                    .py_1()
                                    .px_2()
                                    // Note: even though this `text!` macro
                                    // produces multiple elements, it doesn't
                                    // need its own unique ID because the parent
                                    // div has different IDs for each string.
                                    .child(text!(format!("{}. {}", i + 1, label)))
                            },
                        )),
                )
        }
    }

    pub fn run_example() {
        #[cfg(not(target_family = "wasm"))]
        env_logger::builder()
            .filter_level(log::LevelFilter::Warn)
            .filter_module("rgpui", log::LevelFilter::Info)
            .init();
        application().run(|cx: &mut App| {
            cx.bind_keys([
                KeyBinding::new("tab", Tab, None),
                KeyBinding::new("shift-tab", TabPrev, None),
            ]);

            let bounds = Bounds::centered(None, size(px(500.), px(400.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(rgpui::TitlebarOptions {
                        title: Some("rgpui 无障碍演示".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| A11yDemo::new(window, cx)),
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| A11yDemo::new(window, cx)).into()
    }
}

mod theme {
    //! 主题切换示例
    //!
    //! 此示例演示 rgpui 主题系统的使用方式：
    //! - 亮色 / 暗色模式切换
    //! - 从 `ThemeRegistry` 读取主题列表并应用任意主题
    //! - 展示当前主题的关键颜色色板
    //!
    //! 运行：
    //! ```text
    //! cargo run -p showcase --bin theme
    //! ```

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::prelude::*;
    use rgpui::{
        ActiveTheme, App, Button, ButtonVariants as _, Context, DropdownMenu, Hsla, IconName,
        IntoElement, ParentElement, PopupMenuItem, Render, Styled, Theme, ThemeMode, ThemeRegistry,
        Window, WindowOptions, div, h_flex, px, v_flex,
    };
    use rgpui_platform::application;

    struct ThemeExample {
        /// 当前选中的主题名称（跟随全局主题）。
        _active_theme: String,
    }

    impl ThemeExample {
        fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
            Self {
                _active_theme: String::new(),
            }
        }

        /// 切换全局主题模式（亮色 / 暗色）。
        fn switch_mode(&mut self, mode: ThemeMode, cx: &mut Context<Self>) {
            Theme::change(mode, None, cx);
            cx.refresh_windows();
        }
    }

    impl Render for ThemeExample {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let theme = cx.theme();
            let current_name = theme.theme_name().clone();

            v_flex()
                .id("theme-example")
                .size_full()
                .p(px(32.0))
                .gap(px(24.0))
                .child(
                    v_flex()
                        .gap(px(8.0))
                        .child(div().text_2xl().font_semibold().child("主题系统示例"))
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child(format!("当前主题：{}", current_name)),
                        ),
                )
                .child(
                    v_flex()
                        .gap(px(12.0))
                        .child(div().text_base().font_medium().child("模式切换"))
                        .child(
                            h_flex()
                                .gap(px(8.0))
                                .child(
                                    Button::new("switch-light")
                                        .label("亮色")
                                        .icon(IconName::Sun)
                                        .when(theme.is_dark(), |b| b.ghost())
                                        .when(!theme.is_dark(), |b| b.primary())
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.switch_mode(ThemeMode::Light, cx);
                                        })),
                                )
                                .child(
                                    Button::new("switch-dark")
                                        .label("暗色")
                                        .icon(IconName::Moon)
                                        .when(theme.is_dark(), |b| b.primary())
                                        .when(!theme.is_dark(), |b| b.ghost())
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.switch_mode(ThemeMode::Dark, cx);
                                        })),
                                ),
                        ),
                )
                .child(
                    v_flex()
                        .gap(px(12.0))
                        .child(div().text_base().font_medium().child("主题列表"))
                        .child(theme_selector(cx)),
                )
                .child(
                    v_flex()
                        .gap(px(12.0))
                        .child(div().text_base().font_medium().child("颜色色板"))
                        .child(color_palette(cx)),
                )
        }
    }

    fn theme_selector(cx: &Context<ThemeExample>) -> impl IntoElement {
        let current_name = cx.theme().theme_name().clone();

        // 提前收集主题元数据，克隆进 'static 菜单构建闭包。
        let items: Vec<(rgpui::SharedString, ThemeMode, bool)> = ThemeRegistry::global(cx)
            .sorted_themes()
            .into_iter()
            .map(|theme| {
                let name = theme.name.clone();
                (name.clone(), theme.mode, name == current_name)
            })
            .collect();

        Button::new("theme-list")
            .label(current_name)
            .icon(IconName::Palette)
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu.label("选择主题");
                for (name, mode, checked) in &items {
                    let name = name.clone();
                    let mode = *mode;
                    menu = menu.item(PopupMenuItem::new(name).checked(*checked).on_click(
                        move |_, window, cx| {
                            Theme::change(mode, Some(window), cx);
                            cx.refresh_windows();
                        },
                    ));
                }
                menu
            })
    }

    fn color_palette(cx: &Context<ThemeExample>) -> impl IntoElement {
        let theme = cx.theme();

        let colors: Vec<(&str, Hsla)> = vec![
            ("背景", theme.background),
            ("前景", theme.foreground),
            ("主色", theme.primary),
            ("主色悬停", theme.primary_hover),
            ("主色文字", theme.primary_foreground),
            ("次要", theme.secondary),
            ("边框", theme.border),
            ("输入框", theme.input),
            ("弱化", theme.muted),
            ("弱化文字", theme.muted_foreground),
            ("强调", theme.accent),
            ("强调文字", theme.accent_foreground),
            ("危险", theme.danger),
            ("成功", theme.success),
            ("警告", theme.warning),
            ("信息", theme.info),
        ];

        h_flex()
            .flex_wrap()
            .gap(px(12.0))
            .children(colors.into_iter().map(|(name, color)| {
                v_flex()
                    .gap(px(6.0))
                    .child(
                        div()
                            .size(px(56.0))
                            .rounded(px(8.0))
                            .bg(color)
                            .border_1()
                            .border_color(theme.border),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(name),
                    )
            }))
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            // 初始化主题系统：注册默认主题并加载亮色模式。
            rgpui::theme::init(cx);
            // 初始化菜单系统：主题下拉菜单（Popover/PopupMenu）依赖
            // 菜单全局状态（GlobalState），缺失会在打开下拉菜单时 panic。
            rgpui::menu::init(cx);

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(rgpui::WindowBounds::centered(
                        rgpui::size(px(720.), px(640.)),
                        cx,
                    )),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| ThemeExample::new(window, cx)),
            )
            .ok();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| ThemeExample::new(window, cx)).into()
    }
}

mod animation {

    //! 动画示例：展示 `with_timed_transformation`（定时变换）与 `with_animation`（自定义动画）。
    //! 用法：左侧 tab 选「animation」。窗口里两个旋转的 SVG 图标：上面一个用定时旋转，
    //! 下面一个用 bounce 缓动反复旋转。图标来自 `image_showcase/assets`。
    #![cfg_attr(target_family = "wasm", no_main)]

    use std::time::Duration;

    use anyhow::Result;
    use rgpui::{
        Animation, AnimationExt as _, App, AssetSource, Bounds, Context, SharedString,
        Transformation, Window, WindowBounds, WindowOptions, bounce, div, ease_in_out, percentage,
        prelude::*, px, size, svg,
    };
    use rgpui_platform::application;

    struct Assets {}

    impl AssetSource for Assets {
        fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
            std::fs::read(path)
                .map(Into::into)
                .map_err(Into::into)
                .map(Some)
        }

        fn list(&self, path: &str) -> Result<Vec<SharedString>> {
            Ok(std::fs::read_dir(path)?
                .filter_map(|entry| {
                    Some(SharedString::from(
                        entry.ok()?.path().to_string_lossy().into_owned(),
                    ))
                })
                .collect::<Vec<_>>())
        }
    }

    const ARROW_CIRCLE_SVG: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../image_showcase/assets/arrow_circle.svg"
    );

    struct AnimationExample {}

    impl Render for AnimationExample {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .size_full()
                .bg(rgpui::white())
                .text_color(rgpui::black())
                .justify_around()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .size_full()
                        .justify_around()
                        .child(
                            div()
                                .id("content")
                                .flex()
                                .flex_col()
                                .h(px(150.))
                                .overflow_y_scroll()
                                .w_full()
                                .flex_1()
                                .justify_center()
                                .items_center()
                                .text_xl()
                                .gap_4()
                                .child("你好，动画")
                                .child(
                                    svg()
                                        .size_20()
                                        .overflow_hidden()
                                        .path(ARROW_CIRCLE_SVG)
                                        .text_color(rgpui::black())
                                        .with_timed_transformation(
                                            // 场景级动画（D3 后的 Svg 动画管线）：SVG 只绘制一次，
                                            // 之后由场景在每次呈现前旋转它，不重绘视图。
                                            rgpui::TimedTransformation::rotation(
                                                Duration::from_secs(2),
                                                Transformation::default(),
                                            ),
                                        ),
                                )
                                .child(
                                    svg()
                                        .size_20()
                                        .overflow_hidden()
                                        .path(ARROW_CIRCLE_SVG)
                                        .text_color(rgpui::black().opacity(0.5))
                                        .with_animation(
                                            "image_circle",
                                            Animation::new(Duration::from_secs(2))
                                                .repeat()
                                                .with_easing(bounce(ease_in_out)),
                                            |svg, delta| {
                                                svg.with_transformation(Transformation::rotate(
                                                    percentage(delta),
                                                ))
                                            },
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .h(px(64.))
                                .w_full()
                                .p_2()
                                .justify_center()
                                .items_center()
                                .border_t_1()
                                .border_color(rgpui::black().opacity(0.1))
                                .bg(rgpui::black().opacity(0.05))
                                .child("其他面板"),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().with_assets(Assets {}).run(|cx: &mut App| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(300.), px(300.)),
                    cx,
                ))),
                ..Default::default()
            };
            cx.open_window(options, |_, cx| {
                cx.activate(false);
                cx.new(|_| AnimationExample {})
            })
            .unwrap();
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| AnimationExample {}).into()
    }
}

mod daemon_app {

    //! 常驻后台应用骨架：系统托盘、全局热键、电源抑制、单实例、URL scheme、通知等平台能力。
    //! 用法：左侧 tab 选「daemon_app」。窗口是设置面板；托盘图标左键显示浮层、右键出菜单、
    //! 双击激活窗口；全局热键 Cmd/Ctrl+Shift+K 触发通知；关闭窗口后应用仍在托盘常驻，
    //! 从托盘「退出」才真正结束。
    use rgpui::single_instance::{SingleInstance, send_activate_to_existing};
    use rgpui::{
        App, Bounds, Context, Keystroke, PermissionType, PowerSaveBlocker, PowerSaveBlockerKind,
        SystemPowerEvent, TrayIconEvent, TrayMenuItem, Window, WindowBackgroundAppearance,
        WindowBounds, WindowKind, WindowOptions, div, prelude::*, px, rgb, rgba, size,
    };
    use rgpui_platform::application;
    use std::cell::RefCell;
    use std::rc::Rc;

    const APP_ID: &str = "com.example.daemon-app";

    type Inhibitor = Rc<RefCell<Option<Box<dyn PowerSaveBlocker>>>>;

    struct OverlayView;

    impl Render for OverlayView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .size_full()
                .justify_center()
                .items_center()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_6()
                        .rounded(px(12.0))
                        .bg(rgba(0x000000dd))
                        .text_color(rgb(0xffffff))
                        .shadow_lg()
                        .max_w(px(400.0))
                        .child(
                            div()
                                .text_xl()
                                .font_weight(rgpui::FontWeight::BOLD)
                                .child("常驻应用浮层"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgba(0xffffffaa))
                                .child("此浮层窗口始终置顶。"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgba(0xffffffaa))
                                .child("使用 WindowKind::Overlay + 透明背景。"),
                        ),
                )
        }
    }

    struct SettingsView;

    impl Render for SettingsView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .gap_4()
                .p_6()
                .size_full()
                .bg(rgb(0xfafafa))
                .text_color(rgb(0x333333))
                .child(
                    div()
                        .text_xl()
                        .font_weight(rgpui::FontWeight::BOLD)
                        .child("设置"),
                )
                .child(div().text_sm().child("这是从托盘菜单打开的普通设置窗口。"))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x888888))
                        .child(format!("应用 ID：{}", APP_ID)),
                )
        }
    }

    pub fn run_example() {
        // 诊断时用 RUST_LOG=debug 运行，可看到电源抑制等内部日志
        rgpui::init_logging();

        let instance = match SingleInstance::acquire(APP_ID) {
            Ok(instance) => instance,
            Err(_) => {
                eprintln!("另一个实例已在运行，正在发送激活信号。");
                let _ = send_activate_to_existing(APP_ID);
                std::process::exit(0);
            }
        };

        application().run(move |cx: &mut App| {
            cx.set_keep_alive_without_windows(true);

            let inhibitor = setup_power(cx);
            setup_tray(cx, inhibitor);
            setup_global_hotkey(cx);

            // 第二个实例启动时会被唤醒：把已有窗口带回前台
            instance.on_activate(cx, |cx| {
                cx.show_all_windows();
                cx.activate(true);
            });
            log_capabilities(cx);

            // 运行时注册 URL scheme：写入用户级注册表 / 桌面入口，`daemon-app://...`
            // 之后会拉起本应用（失败只记日志，不影响常驻运行）
            cx.register_url_scheme("daemon-app").detach_and_log_err(cx);

            let _ = cx.show_notification("常驻应用", "应用已在后台启动");

            cx.activate(true);
        });
    }

    fn setup_tray(cx: &mut App, inhibitor: Inhibitor) {
        cx.set_tray_tooltip("常驻应用");

        cx.set_tray_menu(vec![
            TrayMenuItem::Action {
                label: "显示浮层".into(),
                id: "show_overlay".into(),
            },
            TrayMenuItem::Action {
                label: "设置".into(),
                id: "settings".into(),
            },
            TrayMenuItem::Separator,
            TrayMenuItem::Action {
                label: "阻止系统休眠".into(),
                id: "inhibit_sleep".into(),
            },
            TrayMenuItem::Action {
                label: "阻止息屏".into(),
                id: "inhibit_display".into(),
            },
            TrayMenuItem::Action {
                label: "取消电源阻止".into(),
                id: "release_inhibit".into(),
            },
            TrayMenuItem::Separator,
            TrayMenuItem::Action {
                label: "退出".into(),
                id: "quit".into(),
            },
        ]);

        cx.on_tray_icon_event(|event, _cx| match event {
            TrayIconEvent::LeftClick => {
                eprintln!("托盘图标左键点击");
            }
            TrayIconEvent::RightClick => {
                eprintln!("托盘图标右键点击");
            }
            TrayIconEvent::DoubleClick => {
                eprintln!("托盘图标双击");
            }
        });

        let menu_inhibitor = inhibitor;
        cx.on_tray_menu_action(move |id, cx| match id.as_ref() {
            "show_overlay" => {
                open_overlay(cx);
                cx.activate(true);
            }
            "settings" => {
                open_settings(cx);
                cx.activate(true);
            }
            // 阻止器只在这里被持有：菜单项按下才申请，取消项把句柄丢回去 ——
            // 句柄 Drop 时底层 fd 关闭，系统立刻恢复原来的省电策略。
            // 演示用的是同一个格子：换一种抑制会先丢掉前一种的句柄
            "inhibit_sleep" => {
                if menu_inhibitor.borrow().is_none() {
                    let blocker = cx.start_power_save_blocker(PowerSaveBlockerKind::PreventSleep);
                    match blocker {
                        Some(blocker) => {
                            eprintln!("已阻止系统休眠（systemd-inhibit --list 可见本条抑制）");
                            *menu_inhibitor.borrow_mut() = Some(blocker);
                        }
                        None => eprintln!("平台未能阻止系统休眠"),
                    }
                }
            }
            "inhibit_display" => {
                if menu_inhibitor.borrow().is_none() {
                    let blocker =
                        cx.start_power_save_blocker(PowerSaveBlockerKind::PreventDisplaySleep);
                    match blocker {
                        Some(blocker) => {
                            eprintln!("已阻止息屏（systemd-inhibit --list 可见本条抑制）");
                            *menu_inhibitor.borrow_mut() = Some(blocker);
                        }
                        None => eprintln!("平台未能阻止息屏"),
                    }
                }
            }
            "release_inhibit" => {
                if menu_inhibitor.borrow_mut().take().is_some() {
                    eprintln!("已取消电源阻止");
                }
            }
            "quit" => {
                cx.quit();
            }
            _ => {}
        });
    }

    fn setup_power(cx: &mut App) -> Inhibitor {
        let inhibitor: Inhibitor = Rc::new(RefCell::new(None));
        let held = inhibitor.clone();
        cx.on_system_power_event(move |event, _cx| match event {
            SystemPowerEvent::Sleep => {
                eprintln!(
                    "系统即将睡眠，抑制句柄是否仍持有: {}",
                    held.borrow().is_some()
                );
            }
            SystemPowerEvent::WakeUp => {
                eprintln!("系统已唤醒");
            }
        });
        inhibitor
    }

    fn setup_global_hotkey(cx: &mut App) {
        let keystroke = Keystroke::parse("cmd-shift-k").expect("有效按键");
        if let Err(err) = cx.register_global_hotkey(1, &keystroke) {
            eprintln!("注册全局热键失败：{}", err);
        }

        cx.on_global_hotkey(move |id, _cx| {
            if id == 1 {
                eprintln!("全局热键已触发（Cmd+Shift+K）");
            }
        });
    }

    fn log_capabilities(cx: &App) {
        let os = cx.os_info();
        eprintln!("系统：{} {}", os.name, os.version);
        eprintln!("网络：{:?}", cx.network_status());
        eprintln!(
            "空闲状态：{:?}",
            cx.system_idle_time().map(|idle| idle.as_secs())
        );
        for kind in [
            PermissionType::Accessibility,
            PermissionType::ScreenCapture,
            PermissionType::InputMonitoring,
        ] {
            eprintln!("权限 {kind:?}：{:?}", cx.check_permission(kind));
        }
        eprintln!("开机自启：{}", cx.is_auto_launch_enabled(APP_ID));
        match cx.focused_window_info() {
            Some(info) => eprintln!(
                "焦点窗口：app={:?} title={:?} pid={:?} bundle={:?}",
                info.app_name, info.window_title, info.pid, info.bundle_id
            ),
            None => eprintln!("焦点窗口：不可用（Wayland 原生会话或无活动窗口）"),
        }
    }

    fn open_overlay(cx: &mut App) {
        let bounds = Bounds::centered(None, size(px(500.), px(300.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                kind: WindowKind::Overlay,
                titlebar: None,
                focus: true,
                show: true,
                window_background: WindowBackgroundAppearance::Transparent,
                ..Default::default()
            },
            |_, cx| cx.new(|_| OverlayView),
        )
        .ok();
    }

    fn open_settings(cx: &mut App) {
        let bounds = Bounds::centered(None, size(px(400.), px(300.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                kind: WindowKind::Normal,
                titlebar: Some(rgpui::TitlebarOptions {
                    title: Some("常驻应用设置".into()),
                    ..Default::default()
                }),
                focus: true,
                show: true,
                ..Default::default()
            },
            |_, cx| cx.new(|_| SettingsView),
        )
        .ok();
    }
}

mod icons {
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

    const STAR_SVG: &[u8] = include_bytes!("../../../../crates/rgpui/assets/icons/star.svg");

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
                        .child(h_flex().flex_wrap().gap_3().children(
                            ICONS.iter().cloned().map(|(icon, name)| {
                                icon_cell(icon, name, foreground, muted, border)
                            }),
                        )),
                )
        }
    }

    pub fn run_example() {
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

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| IconsDemo::new()).into()
    }
}

mod component_gallery {
    //! 新组件演示：Upload + Carousel + Mermaid + Sidebar 分组 + B 系列小件，一页展示。
    //!
    //! B 系列：Select / Combobox / DatePicker / ColorPicker / Avatar / Alert /
    //! Breadcrumb / Card / Typography / Pagination / Steps / Timeline / Rate /
    //! Toggle(+Group) / Popconfirm。

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::{
        Alert, AlertVariant, App, Avatar, AvatarStatus, Bounds, Breadcrumb, BreadcrumbItem, Card,
        Context, Hsla, IconName, Link, Paragraph, Popconfirm, Render, ScrollHandle, TextVariant,
        Title, TitleLevel, Toggle, ToggleGroup, Window, WindowBounds, WindowOptions,
        components::{
            Carousel, CarouselState, ColorPickerState, ComboboxState, DatePickerState,
            MermaidDiagram, Pagination, Rate, Select, Sidebar, SidebarItem, SidebarSection,
            StepItem, Steps, Timeline, TimelineItem, UploadState,
        },
        div, h_flex,
        prelude::*,
        px, rgb, size, v_flex,
    };
    use rgpui_platform::application;
    use std::collections::HashSet;

    const MERMAID_SAMPLE: &str =
        "flowchart LR\nA[需求] -->|评审| B{方案}\nB --> C[实现]\nB --> D(暂缓)";

    struct ComponentsDemo {
        upload: rgpui::Entity<UploadState>,
        carousel: rgpui::Entity<CarouselState>,
        scroll: ScrollHandle,
        selected: rgpui::SharedString,
        collapsed_sections: HashSet<String>,
        sidebar_collapsed: bool,
        sidebar_hidden: bool,
        // B 系列演示状态。
        select_idx: Option<usize>,
        combo: rgpui::Entity<ComboboxState>,
        combo_status: String,
        date: rgpui::Entity<DatePickerState>,
        color: rgpui::Entity<ColorPickerState>,
        color_value: Hsla,
        alert_visible: bool,
        crumb: String,
        page: usize,
        steps_current: usize,
        rate: f32,
        toggle_on: bool,
        toggle_idx: Option<usize>,
        confirm_status: String,
    }

    impl ComponentsDemo {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let upload = cx.new(|cx| UploadState::new(window, cx));
            let carousel =
                cx.new(|_| CarouselState::new().autoplay(std::time::Duration::from_secs(3)));
            let demo = cx.entity();
            let combo = cx.new(|cx| {
                ComboboxState::new(window, cx)
                    .items(vec![
                        "苹果".into(),
                        "杏".into(),
                        "香蕉".into(),
                        "蓝莓".into(),
                    ])
                    .on_change({
                        let demo = demo.clone();
                        move |selected: Vec<usize>, _, cx| {
                            demo.update(cx, |this, _| {
                                this.combo_status = format!("已选下标：{selected:?}");
                            })
                        }
                    })
            });
            let date = cx.new(DatePickerState::new);
            let color = cx.new(|cx| {
                ColorPickerState::new(window, cx).on_change({
                    let demo = demo.clone();
                    move |color: Hsla, _, cx| {
                        demo.update(cx, |this, _| {
                            this.color_value = color;
                        })
                    }
                })
            });
            Self {
                upload,
                carousel,
                scroll: ScrollHandle::new(),
                selected: "inbox".into(),
                collapsed_sections: HashSet::new(),
                sidebar_collapsed: false,
                sidebar_hidden: false,
                select_idx: None,
                combo,
                combo_status: "输入过滤，下拉多选一".to_string(),
                date,
                color,
                color_value: Hsla::default(),
                alert_visible: true,
                crumb: "未点击".to_string(),
                page: 1,
                steps_current: 1,
                rate: 3.0,
                toggle_on: false,
                toggle_idx: None,
                confirm_status: "未确认".to_string(),
            }
        }
    }

    impl Render for ComponentsDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let selected = self.selected.clone();
            let collapsed = self.collapsed_sections.clone();
            let scroll = self.scroll.clone();
            let demo = cx.entity();

            div()
                .id("components-page")
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&scroll)
                .child(
                    v_flex()
                        .gap(px(16.0))
                        .p(px(24.0))
                        .child(div().text_xl().child("Upload（文件上传）"))
                        .child(self.upload.clone())
                        .child(div().text_xl().child("Carousel（轮播）"))
                        .child(
                            Carousel::new(self.carousel.clone())
                                .child(
                                    div()
                                        .h(px(120.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(rgb(0x3b82f6))
                                        .text_xl()
                                        .child("第一页"),
                                )
                                .child(
                                    div()
                                        .h(px(120.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(rgb(0x10b981))
                                        .text_xl()
                                        .child("第二页"),
                                )
                                .child(
                                    div()
                                        .h(px(120.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(rgb(0xf59e0b))
                                        .text_xl()
                                        .child("第三页"),
                                ),
                        )
                        .child(div().text_xl().child("Mermaid（流程图）"))
                        .child(MermaidDiagram::new(MERMAID_SAMPLE))
                        .child(div().text_xl().child("Sidebar（分组 + 角标 + 折叠/隐藏）"))
                        .child(
                            h_flex().gap(px(8.0)).child(
                                rgpui::Button::new("sidebar-hide")
                                    .label(if self.sidebar_hidden {
                                        "显示侧边栏"
                                    } else {
                                        "隐藏侧边栏"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.sidebar_hidden = !this.sidebar_hidden;
                                        cx.notify();
                                    })),
                            ),
                        )
                        .child(
                            h_flex()
                                .h(px(280.0))
                                .when(!self.sidebar_hidden, |this| {
                                    let collapsed_handle = demo.clone();
                                    let section_handle = demo.clone();
                                    this.child(
                                        Sidebar::new()
                                            .collapsible(true)
                                            .collapsed(self.sidebar_collapsed)
                                            .on_toggle_collapsed(move |collapsed, _, cx| {
                                                collapsed_handle.update(cx, |this, _| {
                                                    this.sidebar_collapsed = collapsed;
                                                });
                                            })
                                            .item(
                                                SidebarItem::new("inbox", "收件箱")
                                                    .with_icon(IconName::File)
                                                    .with_badge("12"),
                                            )
                                            .section(
                                                SidebarSection::new(
                                                    "项目",
                                                    vec![
                                                        SidebarItem::new("rgpui", "rgpui")
                                                            .with_badge("3"),
                                                        SidebarItem::new("editor", "ru_editor"),
                                                    ],
                                                )
                                                .collapsed(collapsed.contains("项目")),
                                            )
                                            .selected(selected)
                                            .on_change(cx.listener_value(
                                                |this, id: rgpui::SharedString, _, _| {
                                                    this.selected = id;
                                                },
                                            ))
                                            .on_toggle_section(move |title, collapsed, _, cx| {
                                                section_handle.update(cx, |this, _| {
                                                    if collapsed {
                                                        this.collapsed_sections
                                                            .insert(title.to_string());
                                                    } else {
                                                        this.collapsed_sections
                                                            .remove(title.as_str());
                                                    }
                                                });
                                            }),
                                    )
                                })
                                .child(
                                    div()
                                        .flex_1()
                                        .p(px(16.0))
                                        .child("右侧内容区（随左侧选择切换，此处略）。"),
                                ),
                        )
                        .child(div().text_xl().child("Select（简单值列表）"))
                        .child(
                            h_flex()
                                .gap(px(8.0))
                                .items_center()
                                .child(
                                    Select::new(vec!["苹果".into(), "香蕉".into(), "樱桃".into()])
                                        .selected(self.select_idx)
                                        .placeholder("选一个水果")
                                        .on_change({
                                            let demo = demo.clone();
                                            move |ix: usize, _: rgpui::SharedString, _, cx| {
                                                demo.update(cx, |this, _| {
                                                    this.select_idx = Some(ix);
                                                })
                                            }
                                        }),
                                )
                                .child(div().text_sm().child(format!(
                                "选中：{}",
                                self.select_idx
                                    .and_then(|ix| ["苹果", "香蕉", "樱桃"].get(ix))
                                    .unwrap_or(&"无")
                            ))),
                        )
                        .child(div().text_xl().child("Combobox（可搜索下拉）"))
                        .child(self.combo.clone())
                        .child(div().text_sm().child(self.combo_status.clone()))
                        .child(div().text_xl().child("DatePicker（日历弹窗）"))
                        .child(self.date.clone())
                        .child(div().text_sm().child(format!(
                            "已选：{}",
                            self.date
                                .read_with(cx, |state, _| state
                                    .selected()
                                    .map(|d| d.format("%Y-%m-%d").to_string()))
                                .as_deref()
                                .unwrap_or("未选择")
                        )))
                        .child(div().text_xl().child("ColorPicker（取色）"))
                        .child(self.color.clone())
                        .child(
                            h_flex()
                                .gap(px(8.0))
                                .items_center()
                                .child(
                                    div()
                                        .w(px(48.0))
                                        .h(px(24.0))
                                        .rounded_md()
                                        .bg(self.color_value),
                                )
                                .child(div().text_sm().child(format!("{:?}", self.color_value))),
                        )
                        .child(div().text_xl().child("Avatar（头像 + 状态）"))
                        .child(
                            h_flex()
                                .gap(px(12.0))
                                .items_center()
                                .child(
                                    Avatar::new("RG")
                                        .size(px(40.0))
                                        .status(AvatarStatus::Online),
                                )
                                .child(Avatar::new("AB").size(px(40.0)).status(AvatarStatus::Busy))
                                .child(Avatar::new("CD").size(px(40.0))),
                        )
                        .child(div().text_xl().child("Alert（行内提示条）"))
                        .child(Alert::new("信息提示").body("这是 info 变体。"))
                        .child(
                            Alert::new("操作成功")
                                .variant(AlertVariant::Success)
                                .body("这是 success 变体。"),
                        )
                        .child(
                            Alert::new("注意")
                                .variant(AlertVariant::Warning)
                                .body("这是 warning 变体。"),
                        )
                        .child(
                            Alert::new("出错了")
                                .variant(AlertVariant::Danger)
                                .body("这是 danger 变体。"),
                        )
                        .children(if self.alert_visible {
                            Some(
                                Alert::new("可关闭")
                                    .variant(AlertVariant::Info)
                                    .closable(true)
                                    .on_close({
                                        let demo = demo.clone();
                                        move |_, cx| {
                                            demo.update(cx, |this, _| {
                                                this.alert_visible = false;
                                            })
                                        }
                                    }),
                            )
                        } else {
                            None
                        })
                        .child(
                            h_flex().gap(px(8.0)).items_center().child(
                                rgpui::Button::new("alert-restore")
                                    .label("恢复可关闭提示条")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.alert_visible = true;
                                        cx.notify();
                                    })),
                            ),
                        )
                        .child(div().text_xl().child("Breadcrumb（路径导航）"))
                        .child(Breadcrumb::new(vec![
                            BreadcrumbItem::new("首页").on_click({
                                let demo = demo.clone();
                                move |_, _, cx| {
                                    demo.update(cx, |this, _| {
                                        this.crumb = "首页".to_string();
                                    })
                                }
                            }),
                            BreadcrumbItem::new("组件"),
                            BreadcrumbItem::new("当前页"),
                        ]))
                        .child(div().text_sm().child(format!("点击：{}", self.crumb)))
                        .child(div().text_xl().child("Card（卡片容器）"))
                        .child(
                            Card::new().title("卡片标题").child(
                                div()
                                    .text_sm()
                                    .child("卡片内容区（header/content/footer 由子元素拼）。"),
                            ),
                        )
                        .child(div().text_xl().child("Typography（排印）"))
                        .child(Title::new("二级标题").level(TitleLevel::H2))
                        .child(Paragraph::new("次要正文（灰）。").variant(TextVariant::Secondary))
                        .child(Link::new("点我也是面包屑").on_click({
                            let demo = demo.clone();
                            move |_, _, cx| {
                                demo.update(cx, |this, _| {
                                    this.crumb = "链接".to_string();
                                })
                            }
                        }))
                        .child(div().text_xl().child("Pagination（页码导航）"))
                        .child(Pagination::new(self.page, 10).on_change({
                            let demo = demo.clone();
                            move |page: usize, _, cx| {
                                demo.update(cx, |this, _| {
                                    this.page = page;
                                })
                            }
                        }))
                        .child(div().text_sm().child(format!("当前第 {} 页", self.page)))
                        .child(div().text_xl().child("Steps（步骤条）"))
                        .child(Steps::new(
                            vec![
                                StepItem::new("下单").description("已完成"),
                                StepItem::new("支付"),
                                StepItem::new("收货"),
                            ],
                            self.steps_current,
                        ))
                        .child(
                            h_flex().gap(px(8.0)).child(
                                rgpui::Button::new("steps-next").label("下一步").on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.steps_current = (this.steps_current + 1).min(2);
                                        cx.notify();
                                    }),
                                ),
                            ),
                        )
                        .child(div().text_xl().child("Timeline（时间线）"))
                        .child(Timeline::new(vec![
                            TimelineItem::new("10:24", "需求评审"),
                            TimelineItem::new("11:02", "方案确定"),
                            TimelineItem::new("14:40", "开始实现"),
                        ]))
                        .child(div().text_xl().child("Rate（星级评分）"))
                        .child(
                            h_flex()
                                .gap(px(8.0))
                                .items_center()
                                .child(Rate::new().value(self.rate).on_change({
                                    let demo = demo.clone();
                                    move |value: f32, _, cx| {
                                        demo.update(cx, |this, _| {
                                            this.rate = value;
                                        })
                                    }
                                }))
                                .child(div().text_sm().child(format!("{:.1} 星", self.rate))),
                        )
                        .child(div().text_xl().child("Toggle（切换按钮）"))
                        .child(
                            h_flex()
                                .gap(px(12.0))
                                .items_center()
                                .child(Toggle::new("飞行模式").pressed(self.toggle_on).on_change({
                                    let demo = demo.clone();
                                    move |pressed: bool, _, cx| {
                                        demo.update(cx, |this, _| {
                                            this.toggle_on = pressed;
                                        })
                                    }
                                }))
                                .child(
                                    ToggleGroup::new(vec!["日".into(), "周".into(), "月".into()])
                                        .selected(self.toggle_idx)
                                        .on_change({
                                            let demo = demo.clone();
                                            move |ix: usize, _, cx| {
                                                demo.update(cx, |this, _| {
                                                    this.toggle_idx = Some(ix);
                                                })
                                            }
                                        }),
                                )
                                .child(div().text_sm().child(format!(
                                "开关：{} / 分组：{}",
                                if self.toggle_on { "开" } else { "关" },
                                self.toggle_idx
                                    .and_then(|ix| ["日", "周", "月"].get(ix))
                                    .unwrap_or(&"无"),
                            ))),
                        )
                        .child(div().text_xl().child("Popconfirm（气泡确认）"))
                        .child(
                            h_flex()
                                .gap(px(8.0))
                                .items_center()
                                .child(
                                    Popconfirm::new("删除", "确定删除这条记录吗？")
                                        .on_confirm({
                                            let demo = demo.clone();
                                            move |_, cx| {
                                                demo.update(cx, |this, _| {
                                                    this.confirm_status = "已确认删除".to_string();
                                                })
                                            }
                                        })
                                        .on_cancel(move |_, cx| {
                                            demo.update(cx, |this, _| {
                                                this.confirm_status = "已取消".to_string();
                                            })
                                        }),
                                )
                                .child(div().text_sm().child(self.confirm_status.clone())),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            // 全局初始化（主题 + 菜单全局量/键位 + 输入/列表等；缺了 Popover 首开即 panic）。
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(860.0), px(720.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| ComponentsDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| ComponentsDemo::new(window, cx)).into()
    }
}

mod extended_components {
    //! 扩展组件示例：展示并入 rgpui 核心的扩展组件库（原 rgpui-ui）。
    //!
    //! 覆盖：
    //! - `SplitPane`（分隔面板）
    //! - `TagInput`（标签输入）、`TypeWriter`（打字机动画）
    //! - `Waveform`（音频波形）、`Sparkline`（迷你走势图）、`SVGRenderer`（SVG 路径）
    //! - 动画原语：`Spring`（值驱动弹簧）、`AnimationPreset`（便捷预设）
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin extended_components
    //! ```

    // 在 wasm 目标上禁用 main 函数入口
    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::components::{
        SVGRenderer, Sparkline, SplitPane, SplitPaneState, TagInput, TagInputState, TypeWriter,
        TypeWriterState, Waveform,
    };
    use rgpui::{
        App, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled,
        Window, WindowBackgroundAppearance, WindowOptions, div, hsla, prelude::*, px, rgb, v_flex,
    };
    use rgpui_platform::application;

    struct ShowcaseApp {
        /// 分隔面板状态。
        split_pane_state: Entity<SplitPaneState>,
        /// 标签输入状态。
        tag_input_state: Entity<TagInputState>,
        /// 打字机状态。
        type_writer_state: Entity<TypeWriterState>,
    }

    impl ShowcaseApp {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            // 初始化各组件状态实体。
            let split_pane_state = cx.new(SplitPaneState::new);
            let tag_input_state =
                cx.new(|cx| TagInputState::with_tags(window, cx, vec!["rgpui", "扩展组件"]));
            let type_writer_state =
                cx.new(|_cx| TypeWriterState::new("rgpui 扩展组件打字机动画演示"));
            // 启动打字动画（逐字显示）。
            type_writer_state.update(cx, |state, cx| state.start(cx));

            Self {
                split_pane_state,
                tag_input_state,
                type_writer_state,
            }
        }
    }

    impl Render for ShowcaseApp {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            // 分隔面板：左为交互组件，右为数据可视化。
            SplitPane::horizontal(self.split_pane_state.clone())
                .show_collapse_buttons(true)
                .first(interactive_pane(
                    self.tag_input_state.clone(),
                    self.type_writer_state.clone(),
                ))
                .second(data_pane())
        }
    }

    fn interactive_pane(
        tag_input_state: Entity<TagInputState>,
        type_writer_state: Entity<TypeWriterState>,
    ) -> impl IntoElement {
        v_flex()
            .id("interactive")
            .gap(px(8.0))
            .p(px(12.0))
            .child(
                div()
                    .child("标签输入（TagInput）")
                    .text_color(rgb(0x9ca3af)),
            )
            .child(TagInput::new(tag_input_state).suggestions(vec!["动画", "手势", "图表", "特效"]))
            .child(div().h(px(16.0)))
            .child(
                div()
                    .child("打字机（TypeWriter）")
                    .text_color(rgb(0x9ca3af)),
            )
            .child(TypeWriter::new("demo-typewriter", type_writer_state))
    }

    fn data_pane() -> impl IntoElement {
        v_flex()
            .id("data")
            .gap(px(8.0))
            .p(px(12.0))
            .child(
                div()
                    .child("音频波形（Waveform）")
                    .text_color(rgb(0x9ca3af)),
            )
            .child(waveform())
            .child(
                div()
                    .child("迷你走势图（Sparkline）")
                    .text_color(rgb(0x9ca3af)),
            )
            .child(sparklines())
            .child(
                div()
                    .child("SVG 路径（SVGRenderer）")
                    .text_color(rgb(0x9ca3af)),
            )
            .child(svg_path())
    }

    fn waveform() -> impl IntoElement {
        let mut data = Vec::new();
        for i in 0..64 {
            let v = ((i as f32 / 8.0) * std::f32::consts::PI).sin();
            data.push(v * 0.5 + 0.5);
        }
        Waveform::new()
            .data(&data)
            .playback_position(0.35)
            .color(hsla(0.62, 0.1, 0.5, 1.0))
            .active_color(hsla(0.6, 0.8, 0.55, 1.0))
            .a11y("waveform-demo", "音频波形")
    }

    fn sparklines() -> impl IntoElement {
        let line_data = vec![3.0, 7.0, 5.0, 9.0, 6.0, 12.0, 8.0, 15.0, 11.0, 18.0];
        let bar_data = vec![4.0, 6.0, 8.0, 5.0, 9.0, 7.0, 10.0];
        let area_data = vec![2.0, 5.0, 4.0, 8.0, 6.0, 9.0, 7.0, 12.0];

        v_flex()
            .id("sparklines")
            .gap(px(8.0))
            .child(
                Sparkline::line(line_data)
                    .width(px(180.0))
                    .height(px(40.0))
                    .a11y("sparkline-line", "折线走势"),
            )
            .child(
                Sparkline::bar(bar_data)
                    .width(px(180.0))
                    .height(px(40.0))
                    .a11y("sparkline-bar", "柱状走势"),
            )
            .child(
                Sparkline::area(area_data)
                    .width(px(180.0))
                    .height(px(40.0))
                    .a11y("sparkline-area", "面积走势"),
            )
    }

    fn svg_path() -> impl IntoElement {
        SVGRenderer::new()
            .path_data(
                "M100,40 C100,20 70,10 55,30 C40,10 10,20 10,40 \
             C10,70 55,100 55,100 C55,100 100,70 100,40 Z",
            )
            .view_box(0.0, 0.0, 110.0, 110.0)
            .fill(hsla(0.0, 0.75, 0.55, 1.0))
            .w(px(80.0))
            .h(px(80.0))
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            // 初始化主题与输入子系统（TagInput 内嵌的 Input 依赖其按键绑定）。
            rgpui::theme::init(cx);
            rgpui::input_ui::init(cx);

            let window_options = WindowOptions {
                window_background: WindowBackgroundAppearance::Opaque,
                titlebar: Some(rgpui::TitlebarOptions {
                    title: Some("扩展组件示例".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };

            let _ = cx.open_window(window_options, |window, cx| {
                cx.new(|cx| ShowcaseApp::new(window, cx))
            });
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| ShowcaseApp::new(window, cx)).into()
    }
}

mod scrollable {

    //! 滚动容器示例：展示 `Scrollable` / `Scrollbar` / `ScrollHandle` 的用法。
    //! 用法：左侧 tab 选「scrollable」，在窗口内滚动内容区、拖动右侧滚动条，观察 handle 位置联动。
    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::{
        App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
    };
    use rgpui_platform::application;

    struct Scrollable {}

    impl Render for Scrollable {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .id("vertical")
                .p_4()
                .overflow_scroll()
                .bg(rgpui::white())
                .child("嵌套布局双向滚动测试示例")
                .child(
                    div()
                        .h(px(5000.))
                        .border_1()
                        .border_color(rgpui::blue())
                        .bg(rgpui::blue().opacity(0.05))
                        .p_4()
                        .child(
                            div()
                                .mb_5()
                                .w_full()
                                .id("horizontal")
                                .overflow_scroll()
                                .child(
                                    div()
                                        .w(px(2000.))
                                        .h(px(150.))
                                        .bg(rgpui::green().opacity(0.1))
                                        .hover(|this| this.bg(rgpui::green().opacity(0.2)))
                                        .border_1()
                                        .border_color(rgpui::green())
                                        .p_4()
                                        .child("水平滚动"),
                                ),
                        )
                        .child("垂直滚动"),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Scrollable {}),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| Scrollable {}).into()
    }
}

mod sidebar {
    //! 侧边栏示例。
    //!
    //! 演示 `Sidebar`：导航切换、折叠为图标栏、角标、条目级自定义选中颜色，
    //! 以及亮色 / 暗色主题下侧栏 token（`sidebar_accent` 系列）与通用 `accent` 的对比。

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::{
        App, Bounds, Button, ButtonVariants as _, Colorize as _, Context, Hsla, IconName, Render,
        SharedString, Theme, ThemeMode, Window, WindowBounds, WindowOptions, div, h_flex,
        prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    struct SidebarDemo {
        /// 当前选中的条目 ID。
        selected: SharedString,
        /// 是否折叠为图标栏。
        collapsed: bool,
    }

    impl SidebarDemo {
        /// 创建演示状态。
        fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
            Self {
                selected: "home".into(),
                collapsed: false,
            }
        }

        /// 当前选中条目对应的内容区标题。
        fn content_title(&self) -> &str {
            match self.selected.as_ref() {
                "home" => "首页",
                "inbox" => "收件箱",
                "search" => "搜索",
                "alerts" => "通知（自定义选中色）",
                "settings" => "设置",
                _ => "未知",
            }
        }

        /// 切换全局主题模式（亮色 / 暗色）。
        fn switch_mode(&mut self, mode: ThemeMode, cx: &mut Context<Self>) {
            Theme::change(mode, None, cx);
            cx.refresh_windows();
        }
    }

    impl Render for SidebarDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let selected = self.selected.clone();
            let collapsed = self.collapsed;
            let title = self.content_title().to_string();
            let theme = cx.theme();
            let is_dark = theme.is_dark();

            h_flex()
            .size_full()
            .child(
                Sidebar::new()
                    .item(SidebarItem::new("home", "首页").with_icon(IconName::LayoutDashboard))
                    .item(
                        SidebarItem::new("inbox", "收件箱")
                            .with_icon(IconName::Inbox)
                            .with_badge("12"),
                    )
                    .item(SidebarItem::new("search", "搜索").with_icon(IconName::Search))
                    .item(
                        SidebarItem::new("alerts", "通知")
                            .with_icon(IconName::Bell)
                            .with_badge("3")
                            // 条目级自定义选中颜色：覆盖主题 sidebar_accent 系列，仅作用于本条目。
                            .with_selected_background(rgb(0xfee2e2))
                            .with_selected_foreground(rgb(0xb91c1c)),
                    )
                    .item(SidebarItem::new("settings", "设置").with_icon(IconName::Settings))
                    .selected(selected)
                    .collapsible(true)
                    .collapsed(collapsed)
                    .on_change(cx.listener_value(|this, id: SharedString, _, _| {
                        this.selected = id;
                    }))
                    .on_toggle_collapsed(cx.listener_value(|this, collapsed: bool, _, _| {
                        this.collapsed = collapsed;
                    })),
            )
            .child(
                v_flex()
                    .flex_1()
                    .size_full()
                    .gap(px(12.0))
                    .p(px(24.0))
                    .bg(cx.theme().tokens.background)
                    .child(div().text_xl().child(format!("当前页面：{title}")))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().tokens.muted_foreground)
                            .child("点击左侧导航切换；点底部按钮折叠为图标栏。"),
                    )
                    // 主题切换：对比亮色 / 暗色下侧栏选中态的实际效果。
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .items_center()
                            .child(div().text_sm().child("主题："))
                            .child(
                                Button::new("theme-light")
                                    .label("亮色")
                                    .icon(IconName::Sun)
                                    .when(is_dark, |b| b.ghost())
                                    .when(!is_dark, |b| b.primary())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.switch_mode(ThemeMode::Light, cx);
                                    })),
                            )
                            .child(
                                Button::new("theme-dark")
                                    .label("暗色")
                                    .icon(IconName::Moon)
                                    .when(is_dark, |b| b.primary())
                                    .when(!is_dark, |b| b.ghost())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.switch_mode(ThemeMode::Dark, cx);
                                    })),
                            ),
                    )
                    // 主题色板：侧栏专用 token vs 通用 accent，直观展示选中态用色来源。
                    .child(div().text_sm().child("侧栏选中态用色（当前主题）："))
                    .child(theme_swatches(cx))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().tokens.muted_foreground)
                            .child("“通知”条目演示了条目级自定义：with_selected_background / with_selected_foreground。"),
                    ),
            )
        }
    }

    fn theme_swatches(cx: &Context<SidebarDemo>) -> impl IntoElement {
        /// 单个色块。
        fn swatch(name: &str, color: Hsla, cx: &Context<SidebarDemo>) -> impl IntoElement {
            let theme = cx.theme();
            v_flex()
                .gap(px(6.0))
                .child(
                    div()
                        .size(px(56.0))
                        .rounded(px(8.0))
                        .bg(color)
                        .border_1()
                        .border_color(theme.tokens.border),
                )
                .child(div().text_xs().child(name.to_string()))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.tokens.muted_foreground)
                        .child(color.to_hex()),
                )
        }

        let theme = cx.theme();
        h_flex()
            .gap(px(12.0))
            .flex_wrap()
            .child(swatch("侧栏选中底", theme.tokens.sidebar_accent.color, cx))
            .child(swatch(
                "侧栏选中字",
                theme.tokens.sidebar_accent_foreground.color,
                cx,
            ))
            .child(swatch("通用强调底", theme.tokens.accent.color, cx))
            .child(swatch(
                "通用强调字",
                theme.tokens.accent_foreground.color,
                cx,
            ))
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            // 初始化主题系统（亮色 / 暗色切换依赖主题注册表）。
            rgpui::theme::init(cx);

            let bounds = Bounds::centered(None, size(px(880.0), px(560.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| SidebarDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| SidebarDemo::new(window, cx)).into()
    }
}

mod tabs {
    //! 标签页（Tabs）示例。
    //!
    //! - **静态 Tabs（值驱动，无实体）**：`Tabs::new(items).active(id).on_change(...)`，
    //!   选中项是 `Option<SharedString>`，回调用 `cx.listener_value` 按值接收。
    //! - **可拖拽排序的标签**：`TabDragState`（实体）+ `TabDragDrop`，拖动重排、可关闭。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin tabs
    //! ```

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::tabs::tab_drag::{TabDragDrop, TabDragState, TabItem};
    use rgpui::tabs::{Tabs, TabsItem};
    use rgpui::{
        App, Bounds, Context, Entity, IntoElement, ParentElement, Render, SharedString, Window,
        WindowBounds, WindowOptions, div, h_flex, prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    struct TabsDemo {
        /// 静态 Tabs 当前选中项。
        active: Option<SharedString>,
        /// 可拖拽标签状态。
        drag_state: Entity<TabDragState>,
    }

    impl TabsDemo {
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            let drag_state = cx.new(|_| TabDragState {
                enabled: true,
                tabs: vec![
                    TabItem {
                        title: "main.rs".to_string(),
                        id: "t1".to_string(),
                        closable: true,
                    },
                    TabItem {
                        title: "lib.rs".to_string(),
                        id: "t2".to_string(),
                        closable: true,
                    },
                    TabItem {
                        title: "mod.rs".to_string(),
                        id: "t3".to_string(),
                        closable: false,
                    },
                    TabItem {
                        title: "utils.rs".to_string(),
                        id: "t4".to_string(),
                        closable: true,
                    },
                ],
                ..Default::default()
            });
            Self {
                active: Some("home".into()),
                drag_state,
            }
        }
    }

    impl Render for TabsDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let tab_count = self.drag_state.read(cx).tabs.len();

            v_flex()
                .id("tabs-demo")
                .size_full()
                .gap(px(20.0))
                .p(px(24.0))
                .bg(rgb(0xffffff))
                .child(div().text_2xl().child("标签页（Tabs）"))
                // 1. 静态 Tabs。
                .child(
                    v_flex()
                        .gap(px(8.0))
                        .child(section_title("1. 静态 Tabs（值驱动）"))
                        .child(
                            Tabs::new(vec![
                                TabsItem::new("home", "首页"),
                                TabsItem::new("settings", "设置"),
                                TabsItem::new("export", "导出").disabled(true),
                            ])
                            .active(self.active.clone())
                            .on_change(cx.listener_value(
                                |this, id: SharedString, _, cx| {
                                    this.active = Some(id);
                                    cx.notify();
                                },
                            )),
                        )
                        .child(div().text_sm().child(format!(
                            "当前选中：{}",
                            self.active.as_deref().unwrap_or("无")
                        ))),
                )
                // 2. 可拖拽排序的标签。
                .child(
                    v_flex()
                        .gap(px(8.0))
                        .child(section_title("2. 可拖拽排序的标签（TabDragDrop）"))
                        .child(TabDragDrop::new(self.drag_state.clone()).on_reorder(
                            |_tabs, _window, _cx| {
                                println!("Tab 重新排序完成");
                            },
                        ))
                        .child(
                            h_flex()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .id("tabs-move-left")
                                        .px(px(12.0))
                                        .py(px(6.0))
                                        .rounded_md()
                                        .bg(rgb(0xf0f0f0))
                                        .cursor_pointer()
                                        .child("左移第一个可关闭项")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            let state = this.drag_state.read(cx);
                                            if let Some(first) =
                                                state.tabs.iter().position(|t| t.closable)
                                                && first > 0
                                            {
                                                this.drag_state.update(cx, |s, _| {
                                                    s.move_tab(first, first - 1)
                                                });
                                                cx.notify();
                                            }
                                        })),
                                )
                                .child(div().text_sm().child(format!("Tab 数量：{tab_count}")))
                                .child(div().text_sm().child("拖动 Tab 可排序，蓝色线指示落点")),
                        ),
                )
        }
    }

    fn section_title(text: &str) -> impl IntoElement {
        div().text_lg().child(text.to_string())
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(720.0), px(420.0)), cx);
            let _ = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| TabsDemo::new(window, cx)),
            );
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| TabsDemo::new(window, cx)).into()
    }
}

mod fps_hud {
    //! FPS 监控 HUD 示例。
    //!
    //! `FpsHud` 是一个 `Render` 视图，把它作为根视图的子节点插入即可：它默认持续采样
    //! 每帧间隔、经指数移动平均平滑后显示实时帧率（并请求下一帧保证 HUD 实时跳动）。
    //! CPU / 内存两行默认关闭，需要时由调用方经 `FpsHudState::update_cpu/update_memory`
    //! 喂入后再用 `FpsHud::show_cpu/show_memory` 打开。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin fps_hud
    //! ```

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::{
        App, Bounds, Context, Entity, IntoElement, ParentElement, Render, Window, WindowBounds,
        WindowOptions, div, prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    struct FpsDemo {
        /// HUD 视图实体。
        hud: Entity<FpsHud>,
    }

    impl FpsDemo {
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            // 状态实体（采样数据）。
            let state = cx.new(|_cx| FpsHudState::new());
            // HUD 视图：显示 CPU 行（默认会持续采样）。
            let hud = cx.new(|_cx| FpsHud::new(state).show_cpu(true).continuous(true));
            Self { hud }
        }
    }

    impl Render for FpsDemo {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            v_flex()
            .id("fps-demo")
            // `.relative()` 让 HUD 的 `.absolute()` 相对本视图定位。
            .relative()
            .size_full()
            .bg(rgb(0x101418))
            .text_color(rgb(0xffffff))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .p(px(24.0))
                    .child(div().text_2xl().child("FPS 监控 HUD"))
                    .child(div().text_sm().child(
                        "右上角浮层实时显示帧率、帧时间与 CPU 绘制耗时（DRAW，读自 Window::frame_stats）。",
                    ))
                    .child(div().text_sm().child(
                        "FpsHud::new(state).show_cpu(true).show_memory(true).continuous(true)",
                    )),
            )
            // 把 HUD 视图作为子节点插入。
            .child(self.hud.clone())
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(640.0), px(420.0)), cx);
            let _ = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| FpsDemo::new(window, cx)),
            );
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| FpsDemo::new(window, cx)).into()
    }
}

mod inspector {
    //! 元素检查器示例：默认面板。
    //!
    //! - `cx.enable_default_inspector()` 一行启用默认面板（含 Div 布局展示）
    //! - `window.toggle_inspector(cx)` 开关右侧面板（action `ToggleInspector` + F12）
    //! - `window.cycle_debug_overlay()` 循环切换调试帧覆盖层（action `CycleDebugOverlay` + F11）
    //!   实测帧耗时直接画进场景（右上角点阵读数），不经布局/文本系统
    //! - 崩溃后调试：`enable_crash_recorder` + `install_crash_hook`
    //!   （`.rgpui-crash/last.json` 滚动快照 + `panic-*.log`，程序死了也有据可查）
    //! - AI 调取 GUI：演示区“崩溃快照 & AI 导出”有两个复制按钮，
    //!   即文档“喂给 AI”节的程序化入口（`inspector_tree_text` /
    //!   `capture_inspector_snapshot`），点一下剪贴板里就是 AI 可读数据。
    //!
    //! 自定义面板见兄弟示例 `inspector_custom`（全自写面板的 living 范例）。
    //!
    //! 面板行为（默认面板提供）：顶栏固定（标题/拾取按钮滚不走）、拾取按钮
    //! （悬停蓝框、点击选中、滚轮穿透层级）、选中元素源码位置与布局边界、
    //! 完整树（逐节点折叠，展开集只增不重置）。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin inspector
    //! ```

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::actions;
    use rgpui::input_ui::{Input, InputState};
    use rgpui::{
        App, Bounds, Button, Checkbox, Context, Entity, IntoElement, ParentElement, Render,
        SharedString, Switch, Window, WindowBounds, WindowOptions, div, h_flex, prelude::*, px,
        rgb, size, v_flex,
    };
    use rgpui_platform::application;

    #[cfg(any(feature = "inspector", debug_assertions))]
    actions!(inspector_demo, [ToggleInspector]);

    // 调试帧覆盖层开关（与检查器无关，release 也保留）。
    actions!(inspector_demo_debug, [CycleDebugOverlay]);

    struct InspectorDemo {
        /// 演示输入框状态。
        input: Entity<InputState>,
        /// 通知开关状态。
        notify: bool,
        /// 爱好复选状态。
        hobby: bool,
        /// 演示按钮点击计数。
        clicks: u32,
        /// AI 导出按钮的操作回执（复制成功/提示先开检查器）。
        export_note: Option<String>,
    }

    impl InspectorDemo {
        /// 创建视图并初始化演示输入框。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("可拾取的输入框"));
            Self {
                input,
                notify: true,
                hobby: false,
                clicks: 0,
                export_note: None,
            }
        }

        /// 演示按钮点击计数（证明非拾取态交互正常）。
        fn on_demo_click(&mut self, cx: &mut Context<Self>) {
            self.clicks += 1;
            cx.notify();
        }

        /// 记录 AI 导出按钮的操作回执并刷新（仅调试版本编译，随面板一同剥离）。
        #[cfg(any(feature = "inspector", debug_assertions))]
        fn note_export(&mut self, note: String, cx: &mut Context<Self>) {
            self.export_note = Some(note);
            cx.notify();
        }
    }

    impl Render for InspectorDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let view = cx.entity();
            let click_view = view.clone();
            let notify_view = view.clone();
            let hobby_view = view.clone();

            // F12 开关走全局绑定 + 全局监听（见 run_example），不依赖视图焦点链：
            // 无焦点/面板聚焦时也能开关，所以根节点不挂 key_context/on_action。
            div().id("inspector-demo").size_full().bg(rgb(0xffffff)).child(
                v_flex()
                    .max_w(px(860.0))
                    .mx_auto()
                    .gap(px(16.0))
                    .p(px(24.0))
                    .child(div().text_2xl().child("元素检查器示例（默认面板）"))
                    .child(
                        div().text_sm().text_color(rgb(0x666666)).child(
                            "按 F12 打开右侧检查器面板；按 F11 循环切换右上角调试帧覆盖层（隐藏→耗时→百分位）。点「拾取元素」后悬停看蓝框、点击选中、滚轮穿透重叠层级。（正式 UI 不放调试按钮，检查器只走快捷键）",
                        ),
                    )
                    // 拾取目标：色块。
                    .child(section_title("色块目标"))
                    .child(
                        h_flex()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .id("target-red")
                                    .w(px(120.0))
                                    .h(px(80.0))
                                    .rounded_md()
                                    .bg(rgb(0xfdecea))
                                    .border_1()
                                    .border_color(rgb(0xe0a0a0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child("红盒"),
                            )
                            .child(
                                div()
                                    .id("target-green")
                                    .w(px(120.0))
                                    .h(px(80.0))
                                    .rounded_md()
                                    .bg(rgb(0xe6f4ea))
                                    .border_1()
                                    .border_color(rgb(0xa0c8a8))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child("绿盒"),
                            )
                            .child(
                                div()
                                    .id("target-blue")
                                    .w(px(120.0))
                                    .h(px(80.0))
                                    .rounded_md()
                                    .bg(rgb(0xe8f0fe))
                                    .border_1()
                                    .border_color(rgb(0xa0b8e0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child("蓝盒"),
                            ),
                    )
                    // 拾取目标：嵌套盒。
                    .child(section_title("嵌套目标"))
                    .child(
                        div()
                            .id("target-outer")
                            .p(px(16.0))
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(0xcccccc))
                            .child(
                                div()
                                    .id("target-inner")
                                    .p(px(16.0))
                                    .rounded_md()
                                    .bg(rgb(0xf5f5f5))
                                    .child("内层盒（与外层重叠处可用滚轮切换拾取层级）"),
                            ),
                    )
                    // 拾取目标：交互控件。
                    .child(section_title("控件目标"))
                    .child(
                        v_flex()
                            .gap(px(8.0))
                            .child(Input::new(&self.input))
                            .child(
                                h_flex()
                                    .gap(px(12.0))
                                    .items_center()
                                    .child(
                                        Checkbox::new("demo-hobby")
                                            .label("阅读")
                                            .checked(self.hobby)
                                            .on_change(move |checked, _, cx| {
                                                hobby_view.update(cx, |this, cx| {
                                                    this.hobby = checked;
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                    .child(
                                        Switch::new("demo-notify")
                                            .label("通知")
                                            .checked(self.notify)
                                            .on_change(move |checked, _, cx| {
                                                notify_view.update(cx, |this, cx| {
                                                    this.notify = checked;
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                    .child(
                                        Button::new("demo-click")
                                            .label("点我计数")
                                            .on_click(move |_, _, cx| {
                                                click_view.update(cx, |this, cx| {
                                                    this.on_demo_click(cx);
                                                });
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x666666))
                            .child(format!("演示按钮已点击 {} 次", self.clicks)),
                    )
                    // 报错演示：上报一条应用错误，检查器“报错”卡片即时可见。
                    .child(
                        Button::new("demo-report-error")
                            .label("模拟上报一条错误")
                            .on_click(move |_, _, cx| {
                                cx.report_error("演示手动上报的错误（点一次多一条）");
                            }),
                    )
                    // 崩溃快照 & AI 导出：库侧能力的可点击演示
                    // （滚动快照落盘在 run_example 里已开启，见下方）。
                    .child(crash_ai_section(view, self.export_note.clone())),
            )
        }
    }

    fn section_title(text: impl Into<SharedString> + IntoElement) -> impl IntoElement {
        div()
            .text_size(px(15.0))
            .font_semibold()
            .mt(px(8.0))
            .child(text)
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn crash_ai_section(view: Entity<InspectorDemo>, note: Option<String>) -> impl IntoElement {
        let tree_view = view.clone();
        let snapshot_view = view;
        v_flex()
        .gap(px(8.0))
        .child(section_title("崩溃快照 & AI 导出"))
        .child(
            div().text_sm().text_color(rgb(0x666666)).child(
                "快照已开启：.rgpui-crash/last.json 约 2 秒一写（仅检查器打开时），\
                panic 时另写 panic-*.log。崩溃后直接读这两个文件；\
                在线调试点下面按钮，剪贴板里就是 AI 可读数据（先按 F12 打开检查器）。",
            ),
        )
        .child(
            h_flex()
                .gap(px(12.0))
                .child(
                    Button::new("demo-export-tree")
                        .label("复制树文本（AI 可读）")
                        .on_click(move |_, window, cx| {
                            match window.inspector_tree_text(cx, 2000) {
                                Some(text) => {
                                    let lines = text.lines().count();
                                    cx.write_to_clipboard(rgpui::ClipboardItem::new_string(text));
                                    tree_view.update(cx, |this, cx| {
                                        this.note_export(
                                            format!(
                                                "已复制树文本（{lines} 行），可直接粘给 AI"
                                            ),
                                            cx,
                                        );
                                    });
                                }
                                None => {
                                    tree_view.update(cx, |this, cx| {
                                        this.note_export(
                                            "检查器未打开：先按 F12 打开面板再导出".to_string(),
                                            cx,
                                        );
                                    });
                                }
                            }
                        }),
                )
                .child(
                    Button::new("demo-export-snapshot")
                        .label("复制快照 JSON（AI 可读）")
                        .on_click(move |_, window, cx| {
                            match window.capture_inspector_snapshot(cx) {
                                Some(snapshot) => {
                                    let total = snapshot.tree_total;
                                    let json = serde_json::to_string_pretty(&snapshot)
                                        .unwrap_or_else(|_| "{}".to_string());
                                    cx.write_to_clipboard(rgpui::ClipboardItem::new_string(json));
                                    snapshot_view.update(cx, |this, cx| {
                                        this.note_export(
                                            format!(
                                                "已复制快照 JSON（全树 {total} 节点），可直接粘给 AI"
                                            ),
                                            cx,
                                        );
                                    });
                                }
                                None => {
                                    snapshot_view.update(cx, |this, cx| {
                                        this.note_export(
                                            "检查器未打开：先按 F12 打开面板再导出".to_string(),
                                            cx,
                                        );
                                    });
                                }
                            }
                        }),
                ),
        )
        .when_some(note, |this, note| {
            this.child(div().text_sm().text_color(rgb(0x107c10)).child(note))
        })
    }

    #[cfg(not(any(feature = "inspector", debug_assertions)))]
    fn crash_ai_section(_view: Entity<InspectorDemo>, _note: Option<String>) -> impl IntoElement {
        div()
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::theme::init(cx);
            rgpui::input_ui::init(cx);
            rgpui::menu::init(cx);

            // 一行启用默认检查器面板（含 Div 布局展示注册）。
            // 下面整个块仅调试版本编译：release 下无检查器代码、无 F12 绑定。
            // F12 用全局动作 helper（全局绑定 + 打活动窗口 + spawn 延后更新），
            // 不依赖视图焦点链。
            #[cfg(any(feature = "inspector", debug_assertions))]
            {
                cx.enable_default_inspector();

                // 崩溃后调试：滚动快照 + panic 日志（见文档 09-inspector.md“崩溃快照”节）。
                // last.json 约 2 秒一写、原子替换（仅检查器打开的窗口写入）；
                // panic-*.log 记录死因（负载 + 位置 + 强制回溯）。
                // 程序死了直接读 .rgpui-crash 目录；AI 调试同理可读这两个文件。
                cx.enable_crash_recorder(".rgpui-crash");
                rgpui::runtime_stats::install_crash_hook(".rgpui-crash");

                cx.on_global_action(ToggleInspector, Some("f12"), |window, cx| {
                    window.toggle_inspector(cx);
                });
            }

            // 调试帧覆盖层：F11 在「隐藏 → 最小 → 完整」之间循环切换。
            // 与检查器无关，release 构建同样保留（未启用时不画任何图元，零开销）。
            cx.on_global_action(CycleDebugOverlay, Some("f11"), |window, _cx| {
                window.cycle_debug_overlay();
            });

            let bounds = Bounds::centered(None, size(px(1100.0), px(700.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| InspectorDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| InspectorDemo::new(window, cx)).into()
    }
}

mod inspector_custom {
    //! 元素检查器示例：自定义面板（整板替换 recipe 的 living 范例）。
    //!
    //! 本例面板为完全自写（不用 `default_inspector_panel`）：自有顶栏/拾取按钮、
    //! 选中信息卡、单层子节点列表，并以 `register_inspector_element` 覆盖默认的
    //! Div 布局展示。本例与 `inspector` 各自独立（演示内容各自一份）。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin inspector_custom
    //! ```

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::input_ui::{Input, InputState};
    #[cfg(any(feature = "inspector", debug_assertions))]
    use rgpui::{
        AnyElement, ButtonVariants as _, Div, DivInspectorState, Inspector, InspectorElementId,
        actions,
    };
    use rgpui::{
        App, Bounds, Button, Checkbox, Context, Entity, IntoElement, ParentElement, Render,
        SharedString, Switch, Window, WindowBounds, WindowOptions, div, h_flex, prelude::*, px,
        rgb, size, v_flex,
    };
    use rgpui_platform::application;

    #[cfg(any(feature = "inspector", debug_assertions))]
    actions!(inspector_custom_inspector_demo, [ToggleInspector]);

    struct InspectorDemo {
        /// 演示输入框状态。
        input: Entity<InputState>,
        /// 通知开关状态。
        notify: bool,
        /// 爱好复选状态。
        hobby: bool,
        /// 演示按钮点击计数。
        clicks: u32,
    }

    impl InspectorDemo {
        /// 创建视图并初始化演示输入框。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("可拾取的输入框"));
            Self {
                input,
                notify: true,
                hobby: false,
                clicks: 0,
            }
        }

        /// 演示按钮点击计数（证明非拾取态交互正常）。
        fn on_demo_click(&mut self, cx: &mut Context<Self>) {
            self.clicks += 1;
            cx.notify();
        }
    }

    impl Render for InspectorDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let view = cx.entity();
            let click_view = view.clone();
            let notify_view = view.clone();
            let hobby_view = view;

            // F12 开关走全局绑定 + 全局监听（见 run_example），不依赖视图焦点链：
            // 无焦点/面板聚焦时也能开关，所以根节点不挂 key_context/on_action。
            div().id("inspector-demo").size_full().bg(rgb(0xffffff)).child(
                v_flex()
                    .max_w(px(860.0))
                    .mx_auto()
                    .gap(px(16.0))
                    .p(px(24.0))
                    .child(div().text_2xl().child("元素检查器示例（自定义面板）"))
                    .child(
                        div().text_sm().text_color(rgb(0x666666)).child(
                            "按 F12 打开右侧检查器面板；点「拾取元素」后悬停看蓝框、点击选中、滚轮穿透重叠层级。（正式 UI 不放调试按钮，检查器只走快捷键）",
                        ),
                    )
                    // 拾取目标：色块。
                    .child(section_title("色块目标"))
                    .child(
                        h_flex()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .id("target-red")
                                    .w(px(120.0))
                                    .h(px(80.0))
                                    .rounded_md()
                                    .bg(rgb(0xfdecea))
                                    .border_1()
                                    .border_color(rgb(0xe0a0a0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child("红盒"),
                            )
                            .child(
                                div()
                                    .id("target-green")
                                    .w(px(120.0))
                                    .h(px(80.0))
                                    .rounded_md()
                                    .bg(rgb(0xe6f4ea))
                                    .border_1()
                                    .border_color(rgb(0xa0c8a8))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child("绿盒"),
                            )
                            .child(
                                div()
                                    .id("target-blue")
                                    .w(px(120.0))
                                    .h(px(80.0))
                                    .rounded_md()
                                    .bg(rgb(0xe8f0fe))
                                    .border_1()
                                    .border_color(rgb(0xa0b8e0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child("蓝盒"),
                            ),
                    )
                    // 拾取目标：嵌套盒。
                    .child(section_title("嵌套目标"))
                    .child(
                        div()
                            .id("target-outer")
                            .p(px(16.0))
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(0xcccccc))
                            .child(
                                div()
                                    .id("target-inner")
                                    .p(px(16.0))
                                    .rounded_md()
                                    .bg(rgb(0xf5f5f5))
                                    .child("内层盒（与外层重叠处可用滚轮切换拾取层级）"),
                            ),
                    )
                    // 拾取目标：交互控件。
                    .child(section_title("控件目标"))
                    .child(
                        v_flex()
                            .gap(px(8.0))
                            .child(Input::new(&self.input))
                            .child(
                                h_flex()
                                    .gap(px(12.0))
                                    .items_center()
                                    .child(
                                        Checkbox::new("demo-hobby")
                                            .label("阅读")
                                            .checked(self.hobby)
                                            .on_change(move |checked, _, cx| {
                                                hobby_view.update(cx, |this, cx| {
                                                    this.hobby = checked;
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                    .child(
                                        Switch::new("demo-notify")
                                            .label("通知")
                                            .checked(self.notify)
                                            .on_change(move |checked, _, cx| {
                                                notify_view.update(cx, |this, cx| {
                                                    this.notify = checked;
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                    .child(
                                        Button::new("demo-click")
                                            .label("点我计数")
                                            .on_click(move |_, _, cx| {
                                                click_view.update(cx, |this, cx| {
                                                    this.on_demo_click(cx);
                                                });
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x666666))
                            .child(format!("演示按钮已点击 {} 次", self.clicks)),
                    )
                    // 报错演示：上报一条应用错误，检查器“报错”卡片即时可见。
                    .child(
                        Button::new("demo-report-error")
                            .label("模拟上报一条错误")
                            .on_click(move |_, _, cx| {
                                cx.report_error("演示手动上报的错误（点一次多一条）");
                            }),
                    ),
            )
        }
    }

    fn section_title(text: impl Into<SharedString> + IntoElement) -> impl IntoElement {
        div()
            .text_size(px(15.0))
            .font_semibold()
            .mt(px(8.0))
            .child(text)
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn custom_inspector_panel(
        inspector: &mut Inspector,
        window: &mut Window,
        cx: &mut Context<Inspector>,
    ) -> AnyElement {
        let entity = cx.entity();
        let picking = inspector.is_picking();
        let active_id: Option<InspectorElementId> = inspector.active_element_id().cloned();
        let status = if picking {
            "拾取中"
        } else if active_id.is_some() {
            "已选中"
        } else {
            "空闲"
        };
        // 单层子节点：有选中展选中之子，无选中展树根。
        let parented: Vec<InspectorElementId> = active_id
            .as_ref()
            .map(|id| window.inspector_tree_children(id))
            .unwrap_or_default();
        let rows: Vec<InspectorElementId> = if parented.is_empty() {
            window.inspector_tree_roots()
        } else {
            parented
        };
        let states = inspector.render_inspector_states(window, cx);

        div()
        .id("inspector-custom-panel")
        .size_full()
        .bg(rgb(0xf7f7f7))
        .child(
            v_flex()
                .size_full()
                // 顶栏固定：标题/状态/拾取按钮滚不走。
                .child(
                    v_flex()
                        .flex_shrink_0()
                        .gap(px(8.0))
                        .p(px(12.0))
                        .pb(px(4.0))
                        .child(
                            h_flex()
                                .items_center()
                                .justify_between()
                                .child(div().text_lg().font_semibold().child("自定义检查器"))
                                .child(
                                    div()
                                        .text_xs()
                                        .px(px(8.0))
                                        .py(px(2.0))
                                        .rounded_full()
                                        .bg(rgb(0xfff3e0))
                                        .child(status),
                                ),
                        )
                        .child(
                            Button::new("custom-pick")
                                .label(if picking {
                                    "拾取中，点击画布元素…"
                                } else {
                                    "拾取元素"
                                })
                                .primary()
                                .on_click(move |_, _, cx| {
                                    entity.update(cx, |inspector, _| inspector.start_picking());
                                }),
                        )
                        .child(div().text_xs().text_color(rgb(0x888888)).child(
                            "自写面板：点行选中画布对应区域；子节点只展一层，深入用行选中下钻。",
                        )),
                )
                // 内容区独立滚动。
                .child(
                    div()
                        .id("inspector-custom-scroll")
                        .flex_1()
                        .overflow_scroll()
                        .child(
                            v_flex()
                                .gap(px(8.0))
                                .p(px(12.0))
                                .pt(px(4.0))
                                .when_some(active_id.clone(), |this, id| {
                                    let loc = id.path.source_location;
                                    this.child(
                                        v_flex()
                                            .gap(px(2.0))
                                            .p(px(8.0))
                                            .rounded_md()
                                            .bg(rgb(0xffffff))
                                            .border_1()
                                            .border_color(rgb(0xe0e0e0))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_semibold()
                                                    .child("选中元素（自定义卡）"),
                                            )
                                            .child(custom_info_row(
                                                "源码",
                                                format!("{}:{}", loc.file(), loc.line()),
                                            ))
                                            .child(custom_info_row(
                                                "实例",
                                                format!("#{}", id.instance_id),
                                            )),
                                    )
                                })
                                .child(
                                    v_flex()
                                        .gap(px(1.0))
                                        .p(px(8.0))
                                        .rounded_md()
                                        .bg(rgb(0xffffff))
                                        .border_1()
                                        .border_color(rgb(0xe0e0e0))
                                        .child(div().text_sm().font_semibold().child(
                                            if active_id.is_some() {
                                                "子节点（点行下钻）"
                                            } else {
                                                "树根（先拾取一元素）"
                                            },
                                        ))
                                        .children(rows.into_iter().map(|id| {
                                            let select_id = id.clone();
                                            let label = id.short_label();
                                            let source = id.source_label();
                                            let instance = id.instance_id;
                                            let selected = active_id.as_ref() == Some(&id);
                                            h_flex()
                                                .id(format!("custom-node-{}", id.tree_key()))
                                                .items_center()
                                                .gap(px(6.0))
                                                .py(px(2.0))
                                                .px(px(6.0))
                                                .rounded_sm()
                                                .cursor_pointer()
                                                .hover(|this| this.bg(rgb(0xeeeeee)))
                                                .when(selected, |this| this.bg(rgb(0xfdf0dc)))
                                                .on_click(move |_, window, cx| {
                                                    window.select_inspector_element(&select_id, cx);
                                                })
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .text_xs()
                                                        .text_color(rgb(0x333333))
                                                        .child(format!("{label} #{instance}")),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(rgb(0x999999))
                                                        .child(source),
                                                )
                                        })),
                                )
                                .children(states),
                        ),
                ),
        )
        .into_any_element()
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn custom_info_row(label: &str, value: String) -> impl IntoElement {
        h_flex()
            .gap(px(6.0))
            .child(div().w(px(48.0)).text_xs().child(label.to_string()))
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(rgb(0x333333))
                    .child(value),
            )
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn custom_div_state(
        _id: InspectorElementId,
        state: &DivInspectorState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Div {
        let bounds = state.bounds;
        v_flex()
            .gap(px(2.0))
            .p(px(8.0))
            .rounded_md()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xe0e0e0))
            .child(div().text_sm().font_semibold().child("布局（自定义展示）"))
            .child(custom_info_row(
                "边界",
                format!(
                    "x {:.0} y {:.0} w {:.0} h {:.0}｜内容 w {:.0} h {:.0}",
                    bounds.origin.x.as_f32(),
                    bounds.origin.y.as_f32(),
                    bounds.size.width.as_f32(),
                    bounds.size.height.as_f32(),
                    state.content_size.width.as_f32(),
                    state.content_size.height.as_f32()
                ),
            ))
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::theme::init(cx);
            rgpui::input_ui::init(cx);
            rgpui::menu::init(cx);

            // 下面整个块仅调试版本编译：release 下无检查器代码、无 F12 绑定。
            // F12 用全局动作 helper（全局绑定 + 打活动窗口 + spawn 延后更新），
            // 不依赖视图焦点链。
            #[cfg(any(feature = "inspector", debug_assertions))]
            {
                cx.enable_default_inspector();
                // 整板替换 + 按类型覆盖 Div 展示（同 TypeId 后注册覆盖先注册）。
                cx.set_inspector_renderer(Box::new(custom_inspector_panel));
                cx.register_inspector_element(custom_div_state);

                cx.on_global_action(ToggleInspector, Some("f12"), |window, cx| {
                    window.toggle_inspector(cx);
                });
            }

            let bounds = Bounds::centered(None, size(px(1100.0), px(700.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| InspectorDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| InspectorDemo::new(window, cx)).into()
    }
}

mod gradient {

    //! 渐变绘制示例：展示线性渐变（`linear_gradient` / `linear_color_stop`）与多种颜色空间。
    //! 用法：左侧 tab 选「gradient」，窗口内并排展示不同色标与角度的渐变色块。
    use rgpui::{
        App, Bounds, ColorSpace, Context, Half, Render, ScrollableElement, Window, WindowOptions,
        canvas, div, linear_color_stop, linear_gradient, point, prelude::*, px, size,
    };
    use rgpui_platform::application;

    struct GradientViewer {
        color_space: ColorSpace,
    }

    impl GradientViewer {
        fn new() -> Self {
            Self {
                color_space: ColorSpace::default(),
            }
        }
    }

    impl Render for GradientViewer {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let color_space = self.color_space;

            div()
                .bg(rgpui::white())
                .size_full()
                .overflow_y_scrollbar()
                .overscroll(true)
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .justify_between()
                        .items_center()
                        .child("渐变示例")
                        .child(
                            div().flex().gap_2().items_center().child(
                                div()
                                    .id("method")
                                    .flex()
                                    .px_3()
                                    .py_1()
                                    .text_sm()
                                    .bg(rgpui::black())
                                    .text_color(rgpui::white())
                                    .child(format!("{}", color_space))
                                    .active(|this| this.opacity(0.8))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.color_space = match this.color_space {
                                            ColorSpace::Oklab => ColorSpace::Srgb,
                                            ColorSpace::Srgb => ColorSpace::Oklab,
                                        };
                                        cx.notify();
                                    })),
                            ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .gap_3()
                        .child(
                            div()
                                .size_full()
                                .rounded_xl()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(rgpui::red())
                                .text_color(rgpui::white())
                                .child("纯色"),
                        )
                        .child(
                            div()
                                .size_full()
                                .rounded_xl()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(rgpui::blue())
                                .text_color(rgpui::white())
                                .child("纯色"),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .gap_3()
                        .h_24()
                        .text_color(rgpui::white())
                        .child(
                            div().flex_1().rounded_xl().bg(linear_gradient(
                                45.,
                                linear_color_stop(rgpui::red(), 0.),
                                linear_color_stop(rgpui::blue(), 1.),
                            )
                            .color_space(color_space)),
                        )
                        .child(
                            div().flex_1().rounded_xl().bg(linear_gradient(
                                135.,
                                linear_color_stop(rgpui::red(), 0.),
                                linear_color_stop(rgpui::green(), 1.),
                            )
                            .color_space(color_space)),
                        )
                        .child(
                            div().flex_1().rounded_xl().bg(linear_gradient(
                                225.,
                                linear_color_stop(rgpui::green(), 0.),
                                linear_color_stop(rgpui::blue(), 1.),
                            )
                            .color_space(color_space)),
                        )
                        .child(
                            div().flex_1().rounded_xl().bg(linear_gradient(
                                315.,
                                linear_color_stop(rgpui::green(), 0.),
                                linear_color_stop(rgpui::yellow(), 1.),
                            )
                            .color_space(color_space)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .gap_3()
                        .h_24()
                        .text_color(rgpui::white())
                        .child(
                            div().flex_1().rounded_xl().bg(linear_gradient(
                                0.,
                                linear_color_stop(rgpui::red(), 0.),
                                linear_color_stop(rgpui::white(), 1.),
                            )
                            .color_space(color_space)),
                        )
                        .child(
                            div().flex_1().rounded_xl().bg(linear_gradient(
                                90.,
                                linear_color_stop(rgpui::blue(), 0.),
                                linear_color_stop(rgpui::white(), 1.),
                            )
                            .color_space(color_space)),
                        )
                        .child(
                            div().flex_1().rounded_xl().bg(linear_gradient(
                                180.,
                                linear_color_stop(rgpui::green(), 0.),
                                linear_color_stop(rgpui::white(), 1.),
                            )
                            .color_space(color_space)),
                        )
                        .child(
                            div().flex_1().rounded_xl().bg(linear_gradient(
                                360.,
                                linear_color_stop(rgpui::yellow(), 0.),
                                linear_color_stop(rgpui::white(), 1.),
                            )
                            .color_space(color_space)),
                        ),
                )
                .child(
                    div().flex_1().rounded_xl().bg(linear_gradient(
                        0.,
                        linear_color_stop(rgpui::green(), 0.05),
                        linear_color_stop(rgpui::yellow(), 0.95),
                    )
                    .color_space(color_space)),
                )
                .child(
                    div().flex_1().rounded_xl().bg(linear_gradient(
                        90.,
                        linear_color_stop(rgpui::blue(), 0.05),
                        linear_color_stop(rgpui::red(), 0.95),
                    )
                    .color_space(color_space)),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .gap_3()
                        .child(
                            div().flex().flex_1().gap_3().child(
                                div().flex_1().rounded_xl().bg(linear_gradient(
                                    90.,
                                    linear_color_stop(rgpui::blue(), 0.5),
                                    linear_color_stop(rgpui::red(), 0.5),
                                )
                                .color_space(color_space)),
                            ),
                        )
                        .child(
                            div().flex_1().rounded_xl().bg(linear_gradient(
                                180.,
                                linear_color_stop(rgpui::green(), 0.),
                                linear_color_stop(rgpui::blue(), 0.5),
                            )
                            .color_space(color_space)),
                        ),
                )
                .child(div().h_24().child(canvas(
                    move |_, _, _| {},
                    move |bounds, _, window, _| {
                        let size = size(bounds.size.width * 0.8, px(80.));
                        let square_bounds = Bounds {
                            origin: point(
                                bounds.size.width.half() - size.width.half(),
                                bounds.origin.y,
                            ),
                            size,
                        };
                        let height = square_bounds.size.height;
                        let horizontal_offset = height;
                        let vertical_offset = px(30.);
                        let mut builder = rgpui::PathBuilder::fill();
                        builder.move_to(square_bounds.bottom_left());
                        builder.line_to(
                            square_bounds.origin + point(horizontal_offset, vertical_offset),
                        );
                        builder.line_to(
                            square_bounds.top_right() + point(-horizontal_offset, vertical_offset),
                        );

                        builder.line_to(square_bounds.bottom_right());
                        builder.line_to(square_bounds.bottom_left());
                        let path = builder.build().unwrap();
                        window.paint_path(
                            path,
                            linear_gradient(
                                180.,
                                linear_color_stop(rgpui::red(), 0.),
                                linear_color_stop(rgpui::blue(), 1.),
                            )
                            .color_space(color_space),
                        );
                    },
                )))
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            cx.open_window(
                WindowOptions {
                    focus: true,
                    ..Default::default()
                },
                |_, cx| cx.new(|_| GradientViewer::new()),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| GradientViewer::new()).into()
    }
}

mod painting {

    //! 自定义绘制示例：展示 `canvas` 上的路径绘制、描边、填充与文本绘制。
    //! 用法：左侧 tab 选「painting」，在画布区域查看各类基本图形的绘制结果。
    use rgpui::{
        Background, Bounds, ColorSpace, Context, MouseDownEvent, Path, PathBuilder, PathStyle,
        Pixels, Point, Render, StrokeOptions, Window, WindowOptions, canvas, div,
        linear_color_stop, linear_gradient, point, prelude::*, px, quad, rgb, size,
    };
    use rgpui_platform::application;

    struct PaintingViewer {
        default_lines: Vec<(Path, Background)>,
        background_quads: Vec<(Bounds<Pixels>, Background)>,
        lines: Vec<Vec<Point<Pixels>>>,
        start: Point<Pixels>,
        dashed: bool,
        _painting: bool,
    }

    impl PaintingViewer {
        fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
            let mut lines = vec![];

            // Black squares beneath transparent paths.
            let background_quads = vec![
                (
                    Bounds {
                        origin: point(px(70.), px(70.)),
                        size: size(px(40.), px(40.)),
                    },
                    rgpui::black().into(),
                ),
                (
                    Bounds {
                        origin: point(px(170.), px(70.)),
                        size: size(px(40.), px(40.)),
                    },
                    rgpui::black().into(),
                ),
                (
                    Bounds {
                        origin: point(px(270.), px(70.)),
                        size: size(px(40.), px(40.)),
                    },
                    rgpui::black().into(),
                ),
                (
                    Bounds {
                        origin: point(px(370.), px(70.)),
                        size: size(px(40.), px(40.)),
                    },
                    rgpui::black().into(),
                ),
                (
                    Bounds {
                        origin: point(px(450.), px(50.)),
                        size: size(px(80.), px(80.)),
                    },
                    rgpui::black().into(),
                ),
            ];

            // 50% opaque red path that extends across black quad.
            let mut builder = PathBuilder::fill();
            builder.move_to(point(px(50.), px(50.)));
            builder.line_to(point(px(130.), px(50.)));
            builder.line_to(point(px(130.), px(130.)));
            builder.line_to(point(px(50.), px(130.)));
            builder.close();
            let path = builder.build().unwrap();
            let mut red = rgb(0xFF0000);
            red.a = 0.5;
            lines.push((path, red.into()));

            // 50% opaque blue path that extends across black quad.
            let mut builder = PathBuilder::fill();
            builder.move_to(point(px(150.), px(50.)));
            builder.line_to(point(px(230.), px(50.)));
            builder.line_to(point(px(230.), px(130.)));
            builder.line_to(point(px(150.), px(130.)));
            builder.close();
            let path = builder.build().unwrap();
            let mut blue = rgb(0x0000FF);
            blue.a = 0.5;
            lines.push((path, blue.into()));

            // 50% opaque green path that extends across black quad.
            let mut builder = PathBuilder::fill();
            builder.move_to(point(px(250.), px(50.)));
            builder.line_to(point(px(330.), px(50.)));
            builder.line_to(point(px(330.), px(130.)));
            builder.line_to(point(px(250.), px(130.)));
            builder.close();
            let path = builder.build().unwrap();
            let mut green = rgb(0x00FF00);
            green.a = 0.5;
            lines.push((path, green.into()));

            // 50% opaque black path that extends across black quad.
            let mut builder = PathBuilder::fill();
            builder.move_to(point(px(350.), px(50.)));
            builder.line_to(point(px(430.), px(50.)));
            builder.line_to(point(px(430.), px(130.)));
            builder.line_to(point(px(350.), px(130.)));
            builder.close();
            let path = builder.build().unwrap();
            let mut black = rgb(0x000000);
            black.a = 0.5;
            lines.push((path, black.into()));

            // Two 50% opaque red circles overlapping - center should be darker red
            let mut builder = PathBuilder::fill();
            let center = point(px(530.), px(85.));
            let radius = px(30.);
            builder.move_to(point(center.x + radius, center.y));
            builder.arc_to(
                point(radius, radius),
                px(0.),
                false,
                false,
                point(center.x - radius, center.y),
            );
            builder.arc_to(
                point(radius, radius),
                px(0.),
                false,
                false,
                point(center.x + radius, center.y),
            );
            builder.close();
            let path = builder.build().unwrap();
            let mut red1 = rgb(0xFF0000);
            red1.a = 0.5;
            lines.push((path, red1.into()));

            let mut builder = PathBuilder::fill();
            let center = point(px(570.), px(85.));
            let radius = px(30.);
            builder.move_to(point(center.x + radius, center.y));
            builder.arc_to(
                point(radius, radius),
                px(0.),
                false,
                false,
                point(center.x - radius, center.y),
            );
            builder.arc_to(
                point(radius, radius),
                px(0.),
                false,
                false,
                point(center.x + radius, center.y),
            );
            builder.close();
            let path = builder.build().unwrap();
            let mut red2 = rgb(0xFF0000);
            red2.a = 0.5;
            lines.push((path, red2.into()));

            // draw a Rust logo
            let mut builder = lyon::path::Path::svg_builder();
            lyon::extra::rust_logo::build_logo_path(&mut builder);
            // move down the Path
            let mut builder: PathBuilder = builder.into();
            builder.translate(point(px(10.), px(200.)));
            builder.scale(0.9);
            let path = builder.build().unwrap();
            lines.push((path, rgpui::black().into()));

            // draw a lightening bolt ⚡
            let mut builder = PathBuilder::fill();
            builder.add_polygon(
                &[
                    point(px(150.), px(300.)),
                    point(px(200.), px(225.)),
                    point(px(200.), px(275.)),
                    point(px(250.), px(200.)),
                ],
                false,
            );
            let path = builder.build().unwrap();
            lines.push((path, rgb(0x1d4ed8).into()));

            // draw a ⭐
            let mut builder = PathBuilder::fill();
            builder.move_to(point(px(350.), px(200.)));
            builder.line_to(point(px(370.), px(260.)));
            builder.line_to(point(px(430.), px(260.)));
            builder.line_to(point(px(380.), px(300.)));
            builder.line_to(point(px(400.), px(360.)));
            builder.line_to(point(px(350.), px(320.)));
            builder.line_to(point(px(300.), px(360.)));
            builder.line_to(point(px(320.), px(300.)));
            builder.line_to(point(px(270.), px(260.)));
            builder.line_to(point(px(330.), px(260.)));
            builder.line_to(point(px(350.), px(200.)));
            let path = builder.build().unwrap();
            lines.push((
                path,
                linear_gradient(
                    180.,
                    linear_color_stop(rgb(0xFACC15), 0.7),
                    linear_color_stop(rgb(0xD56D0C), 1.),
                )
                .color_space(ColorSpace::Oklab),
            ));

            // draw linear gradient
            let square_bounds = Bounds {
                origin: point(px(450.), px(200.)),
                size: size(px(200.), px(80.)),
            };
            let height = square_bounds.size.height;
            let horizontal_offset = height;
            let vertical_offset = px(30.);
            let mut builder = PathBuilder::fill();
            builder.move_to(square_bounds.bottom_left());
            builder.curve_to(
                square_bounds.origin + point(horizontal_offset, vertical_offset),
                square_bounds.origin + point(px(0.0), vertical_offset),
            );
            builder.line_to(square_bounds.top_right() + point(-horizontal_offset, vertical_offset));
            builder.curve_to(
                square_bounds.bottom_right(),
                square_bounds.top_right() + point(px(0.0), vertical_offset),
            );
            builder.line_to(square_bounds.bottom_left());
            let path = builder.build().unwrap();
            lines.push((
                path,
                linear_gradient(
                    180.,
                    linear_color_stop(rgpui::blue(), 0.4),
                    linear_color_stop(rgpui::red(), 1.),
                ),
            ));

            // draw a pie chart
            let center = point(px(96.), px(96.));
            let pie_center = point(px(775.), px(255.));
            let segments = [
                (
                    point(px(871.), px(255.)),
                    point(px(747.), px(163.)),
                    rgb(0x1374e9),
                ),
                (
                    point(px(747.), px(163.)),
                    point(px(679.), px(263.)),
                    rgb(0xe13527),
                ),
                (
                    point(px(679.), px(263.)),
                    point(px(754.), px(349.)),
                    rgb(0x0751ce),
                ),
                (
                    point(px(754.), px(349.)),
                    point(px(854.), px(310.)),
                    rgb(0x209742),
                ),
                (
                    point(px(854.), px(310.)),
                    point(px(871.), px(255.)),
                    rgb(0xfbc10a),
                ),
            ];

            for (start, end, color) in segments {
                let mut builder = PathBuilder::fill();
                builder.move_to(start);
                builder.arc_to(center, px(0.), false, false, end);
                builder.line_to(pie_center);
                builder.close();
                let path = builder.build().unwrap();
                lines.push((path, color.into()));
            }

            // draw a wave
            let options = StrokeOptions::default()
                .with_line_width(1.)
                .with_line_join(lyon::path::LineJoin::Bevel);
            let mut builder = PathBuilder::stroke(px(1.)).with_style(PathStyle::Stroke(options));
            builder.move_to(point(px(40.), px(420.)));
            for i in 1..50 {
                builder.line_to(point(
                    px(40.0 + i as f32 * 10.0),
                    px(420.0 + (i as f32 * 10.0).sin() * 40.0),
                ));
            }
            let path = builder.build().unwrap();
            lines.push((path, rgpui::green().into()));

            Self {
                default_lines: lines.clone(),
                background_quads,
                lines: vec![],
                start: point(px(0.), px(0.)),
                dashed: false,
                _painting: false,
            }
        }

        fn clear(&mut self, cx: &mut Context<Self>) {
            self.lines.clear();
            cx.notify();
        }
    }

    fn button(
        text: &str,
        cx: &mut Context<PaintingViewer>,
        on_click: impl Fn(&mut PaintingViewer, &mut Context<PaintingViewer>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(text.to_string())
            .child(text.to_string())
            .bg(rgpui::black())
            .text_color(rgpui::white())
            .active(|this| this.opacity(0.8))
            .flex()
            .px_3()
            .py_1()
            .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
    }

    impl Render for PaintingViewer {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let default_lines = self.default_lines.clone();
            let background_quads = self.background_quads.clone();
            let lines = self.lines.clone();
            let dashed = self.dashed;

            div()
                .bg(rgpui::white())
                .size_full()
                .p_4()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .justify_between()
                        .items_center()
                        .child("在任意位置按下并拖动绘制线条（按住 Shift 键绘制直线）")
                        .child(
                            div()
                                .flex()
                                .gap_x_2()
                                .child(button(
                                    if dashed { "Solid" } else { "Dashed" },
                                    cx,
                                    move |this, _| this.dashed = !dashed,
                                ))
                                .child(button("清除", cx, |this, cx| this.clear(cx))),
                        ),
                )
                .child(
                    div()
                        .size_full()
                        .child(
                            canvas(
                                move |_, _, _| {},
                                move |_, _, window, _| {
                                    // First draw background quads
                                    for (bounds, color) in background_quads.iter() {
                                        window.paint_quad(quad(
                                            *bounds,
                                            px(0.),
                                            *color,
                                            px(0.),
                                            rgpui::transparent_black(),
                                            Default::default(),
                                        ));
                                    }

                                    // Then draw the default paths on top
                                    for (path, color) in default_lines {
                                        window.paint_path(path, color);
                                    }

                                    for points in lines {
                                        if points.len() < 2 {
                                            continue;
                                        }

                                        let mut builder = PathBuilder::stroke(px(1.));
                                        if dashed {
                                            builder = builder.dash_array(&[px(4.), px(2.)]);
                                        }
                                        for (i, p) in points.into_iter().enumerate() {
                                            if i == 0 {
                                                builder.move_to(p);
                                            } else {
                                                builder.line_to(p);
                                            }
                                        }

                                        if let Ok(path) = builder.build() {
                                            window.paint_path(path, rgpui::black());
                                        }
                                    }
                                },
                            )
                            .size_full(),
                        )
                        .on_mouse_down(
                            rgpui::MouseButton::Left,
                            cx.listener(|this, ev: &MouseDownEvent, _, _| {
                                this._painting = true;
                                this.start = ev.position;
                                let path = vec![ev.position];
                                this.lines.push(path);
                            }),
                        )
                        .on_mouse_move(cx.listener(|this, ev: &rgpui::MouseMoveEvent, _, cx| {
                            if !this._painting {
                                return;
                            }

                            let is_shifted = ev.modifiers.shift;
                            let mut pos = ev.position;
                            // When holding shift, draw a straight line
                            if is_shifted {
                                let dx = pos.x - this.start.x;
                                let dy = pos.y - this.start.y;
                                if dx.abs() > dy.abs() {
                                    pos.y = this.start.y;
                                } else {
                                    pos.x = this.start.x;
                                }
                            }

                            if let Some(path) = this.lines.last_mut() {
                                path.push(pos);
                            }

                            cx.notify();
                        }))
                        .on_mouse_up(
                            rgpui::MouseButton::Left,
                            cx.listener(|this, _, _, _| {
                                this._painting = false;
                            }),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx| {
            cx.open_window(
                WindowOptions {
                    focus: true,
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| PaintingViewer::new(window, cx)),
            )
            .unwrap();
            cx.on_window_closed(|cx, _window_id| {
                cx.quit();
            })
            .detach();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| PaintingViewer::new(window, cx)).into()
    }
}

mod pattern {

    //! 图案/纹理绘制示例：展示重复图案的构造与平铺填充。
    //! 用法：左侧 tab 选「pattern」，查看窗口内的图案填充效果。
    use rgpui::{
        App, AppContext, Bounds, Context, Window, WindowBounds, WindowOptions, div,
        linear_color_stop, linear_gradient, pattern_slash, prelude::*, px, rgb, size,
    };
    use rgpui_platform::application;

    struct PatternExample;

    impl Render for PatternExample {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .bg(rgb(0xffffff))
                .size(px(600.0))
                .justify_center()
                .items_center()
                .shadow_lg()
                .text_xl()
                .text_color(rgb(0x000000))
                .child("图案示例")
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .border_1()
                        .border_color(rgpui::blue())
                        .child(div().w(px(54.0)).h(px(18.0)).bg(pattern_slash(
                            rgpui::red(),
                            18.0 / 4.0,
                            18.0 / 4.0,
                        )))
                        .child(div().w(px(54.0)).h(px(18.0)).bg(pattern_slash(
                            rgpui::red(),
                            18.0 / 4.0,
                            18.0 / 4.0,
                        )))
                        .child(div().w(px(54.0)).h(px(18.0)).bg(pattern_slash(
                            rgpui::red(),
                            18.0 / 4.0,
                            18.0 / 4.0,
                        )))
                        .child(div().w(px(54.0)).h(px(18.0)).bg(pattern_slash(
                            rgpui::red(),
                            18.0 / 4.0,
                            18.0 / 2.0,
                        ))),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .border_1()
                        .border_color(rgpui::blue())
                        .bg(rgpui::green().opacity(0.16))
                        .child("相同高度的元素应对齐")
                        .child(div().w(px(256.0)).h(px(56.0)).bg(pattern_slash(
                            rgpui::red(),
                            56.0 / 6.0,
                            56.0 / 6.0,
                        )))
                        .child(div().w(px(256.0)).h(px(56.0)).bg(pattern_slash(
                            rgpui::green(),
                            56.0 / 6.0,
                            56.0 / 6.0,
                        )))
                        .child(div().w(px(256.0)).h(px(56.0)).bg(pattern_slash(
                            rgpui::blue(),
                            56.0 / 6.0,
                            56.0 / 6.0,
                        )))
                        .child(div().w(px(256.0)).h(px(26.0)).bg(pattern_slash(
                            rgpui::yellow(),
                            56.0 / 6.0,
                            56.0 / 6.0,
                        ))),
                )
                .child(
                    div()
                        .border_1()
                        .border_color(rgpui::blue())
                        .w(px(240.0))
                        .h(px(40.0))
                        .bg(rgpui::red()),
                )
                .child(
                    div()
                        .border_1()
                        .border_color(rgpui::blue())
                        .w(px(240.0))
                        .h(px(40.0))
                        .bg(linear_gradient(
                            45.,
                            linear_color_stop(rgpui::red(), 0.),
                            linear_color_stop(rgpui::blue(), 1.),
                        )),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(600.0), px(600.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_window, cx| cx.new(|_cx| PatternExample),
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_cx| PatternExample).into()
    }
}

mod anchor {

    //! 锚点定位示例：展示 `anchor` 如何把浮层锚定到目标元素的边缘。
    //! 用法：左侧 tab 选「anchor」，点击按钮弹出锚定浮层，观察浮层相对按钮的位置。
    use rgpui::{
        Anchor, AnchoredPositionMode, App, Axis, Bounds, Context, Half as _, InteractiveElement,
        ParentElement, Pixels, Point, Render, SharedString, Size, Window, WindowBounds,
        WindowOptions, anchored, deferred, div, point, prelude::*, px, rgb, size,
    };
    use rgpui_platform::application;

    struct AnchorDemo {
        hovered_button: Option<usize>,
    }

    struct ButtonDemo {
        label: SharedString,
        corner: Option<Anchor>,
    }

    fn resolved_position(corner: Anchor, button_size: Size<Pixels>) -> Point<Pixels> {
        let offset = Point {
            x: px(0.),
            y: -button_size.height,
        };

        offset
            + match corner.other_side_along(Axis::Vertical) {
                Anchor::TopLeft => point(px(0.0), px(0.0)),
                Anchor::TopCenter => point(button_size.width.half(), px(0.0)),
                Anchor::TopRight => point(button_size.width, px(0.0)),
                Anchor::LeftCenter => point(button_size.width, button_size.height.half()),
                Anchor::RightCenter => point(px(0.), button_size.height.half()),
                Anchor::BottomLeft => point(px(0.0), button_size.height),
                Anchor::BottomCenter => point(button_size.width / 2.0, button_size.height),
                Anchor::BottomRight => point(button_size.width, button_size.height),
            }
    }

    impl AnchorDemo {
        fn buttons() -> Vec<ButtonDemo> {
            vec![
                ButtonDemo {
                    label: "TopLeft".into(),
                    corner: Some(Anchor::TopLeft),
                },
                ButtonDemo {
                    label: "TopCenter".into(),
                    corner: Some(Anchor::TopCenter),
                },
                ButtonDemo {
                    label: "TopRight".into(),
                    corner: Some(Anchor::TopRight),
                },
                ButtonDemo {
                    label: "LeftCenter".into(),
                    corner: Some(Anchor::LeftCenter),
                },
                ButtonDemo {
                    label: "Center".into(),
                    corner: None,
                },
                ButtonDemo {
                    label: "RightCenter".into(),
                    corner: Some(Anchor::RightCenter),
                },
                ButtonDemo {
                    label: "BottomLeft".into(),
                    corner: Some(Anchor::BottomLeft),
                },
                ButtonDemo {
                    label: "BottomCenter".into(),
                    corner: Some(Anchor::BottomCenter),
                },
                ButtonDemo {
                    label: "BottomRight".into(),
                    corner: Some(Anchor::BottomRight),
                },
            ]
        }
    }

    impl Render for AnchorDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let buttons = Self::buttons();
            let button_size = size(px(120.0), px(65.0));

            div()
                .flex()
                .flex_col()
                .size_full()
                .items_center()
                .justify_center()
                .bg(rgpui::white())
                .gap_4()
                .p_10()
                .child("带锚点的浮层")
                .child(
                    div()
                        .size_128()
                        .grid()
                        .grid_cols(3)
                        .gap_6()
                        .relative()
                        .children(buttons.iter().enumerate().map(|(index, button)| {
                            let is_hovered = self.hovered_button == Some(index);
                            let is_hoverable = button.corner.is_some();
                            div()
                                .relative()
                                .child(
                                    div()
                                        .id(("button", index))
                                        .w(button_size.width)
                                        .h(button_size.height)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(rgpui::white())
                                        .when(is_hoverable, |this| {
                                            this.border_1()
                                                .rounded_lg()
                                                .border_color(rgpui::black())
                                                .hover(|style| {
                                                    style
                                                        .bg(rgpui::black())
                                                        .text_color(rgpui::white())
                                                })
                                                .on_hover(cx.listener(
                                                    move |this, hovered, _window, cx| {
                                                        if *hovered {
                                                            this.hovered_button = Some(index);
                                                        } else if this.hovered_button == Some(index)
                                                        {
                                                            this.hovered_button = None;
                                                        }
                                                        cx.notify();
                                                    },
                                                ))
                                                .child(button.label.clone())
                                        }),
                                )
                                .when_some(
                                    self.hovered_button.filter(|_| is_hovered),
                                    |this, index| {
                                        let button = &buttons[index];
                                        let Some(corner) = button.corner else {
                                            return this;
                                        };

                                        let position = resolved_position(corner, button_size);
                                        this.child(deferred(
                                            anchored()
                                                .anchor(corner)
                                                .position(position)
                                                .position_mode(AnchoredPositionMode::Local)
                                                .snap_to_window()
                                                .child(
                                                    div()
                                                        .py_0p5()
                                                        .px_2()
                                                        .bg(rgpui::black().opacity(0.75))
                                                        .text_color(rgb(0xffffff))
                                                        .rounded_sm()
                                                        .shadow_sm()
                                                        .min_w(px(100.0))
                                                        .text_sm()
                                                        .child(button.label.clone()),
                                                ),
                                        ))
                                    },
                                )
                        })),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(750.), px(600.)),
                        cx,
                    ))),
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|_| AnchorDemo {
                        hovered_button: None,
                    })
                },
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| AnchorDemo {
            hovered_button: None,
        })
        .into()
    }
}

mod popover {

    //! 浮层示例：展示 deferred 浮层、嵌套二级浮层与关闭行为。
    //! 用法：左侧 tab 选「popover」，依次打开一级、二级浮层；点击浮层外部可关闭。
    use rgpui::{
        Anchor, App, Context, Div, Hsla, Stateful, Window, WindowOptions, anchored, deferred, div,
        prelude::*, px,
    };
    use rgpui_platform::application;

    /// An example show use deferred to create a floating layers.
    struct HelloWorld {
        open: bool,
        secondary_open: bool,
    }

    fn button(id: &'static str) -> Stateful<Div> {
        div()
            .id(id)
            .bg(rgpui::black())
            .text_color(rgpui::white())
            .px_3()
            .py_1()
    }

    fn popover() -> Div {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .shadow_lg()
            .p_3()
            .rounded_md()
            .bg(rgpui::white())
            .text_color(rgpui::black())
            .border_1()
            .text_sm()
            .border_color(rgpui::black().opacity(0.1))
    }

    fn line(color: Hsla) -> Div {
        div().w(px(480.)).h_2().bg(color.opacity(0.25))
    }

    impl HelloWorld {
        fn render_secondary_popover(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl IntoElement {
            button("secondary-btn")
                .mt_2()
                .child("子浮层")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.secondary_open = true;
                    cx.notify();
                }))
                .when(self.secondary_open, |this| {
                    this.child(
                        // Now RGPUI supports nested deferred!
                        deferred(
                            anchored()
                                .anchor(Anchor::TopLeft)
                                .snap_to_window_with_margin(px(8.))
                                .child(
                                    popover()
                                        .child("这是嵌套延迟的第二级浮层！")
                                        .bg(rgpui::white())
                                        .border_color(rgpui::blue())
                                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                            this.secondary_open = false;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .priority(2),
                    )
                })
        }
    }

    impl Render for HelloWorld {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .size_full()
                .bg(rgpui::white())
                .text_color(rgpui::black())
                .justify_center()
                .items_center()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_4()
                        .child(
                            button("popover0").child("已打开浮层").child(
                                deferred(
                                    anchored()
                                        .anchor(Anchor::TopLeft)
                                        .snap_to_window_with_margin(px(8.))
                                        .child(popover().w_96().gap_3().child(
                                            "This is a default opened Popover, \
                                        we can use deferred to render it \
                                        in a floating layer.",
                                        )),
                                )
                                .priority(0),
                            ),
                        )
                        .child(
                            button("popover1")
                                .child("打开浮层")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.open = true;
                                    cx.notify();
                                }))
                                .when(self.open, |this| {
                                    this.child(
                                        deferred(
                                            anchored()
                                                .anchor(Anchor::TopLeft)
                                                .snap_to_window_with_margin(px(8.))
                                                .child(
                                                    popover()
                                                        .w_96()
                                                        .gap_3()
                                                        .child(
                                                            "This is first level Popover, \
                                                   we can use deferred to render it \
                                                   in a floating layer.\n\
                                                   Click outside to close.",
                                                        )
                                                        .when(!self.secondary_open, |this| {
                                                            this.on_mouse_down_out(cx.listener(
                                                                |this, _, _, cx| {
                                                                    this.open = false;
                                                                    cx.notify();
                                                                },
                                                            ))
                                                        })
                                                        // Here we need render popover after the content
                                                        // to ensure it will be on top layer.
                                                        .child(
                                                            self.render_secondary_popover(
                                                                window, cx,
                                                            ),
                                                        ),
                                                ),
                                        )
                                        .priority(1),
                                    )
                                }),
                        ),
                )
                .child(
                    "Here is an example text rendered, \
                to ensure the Popover will float above this contents.",
                )
                .children([
                    line(rgpui::red()),
                    line(rgpui::yellow()),
                    line(rgpui::blue()),
                    line(rgpui::green()),
                ])
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|_| HelloWorld {
                    open: false,
                    secondary_open: false,
                })
            })
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| HelloWorld {
            open: false,
            secondary_open: false,
        })
        .into()
    }
}

/// Gallery 根视图：左侧竖行 tab + 右侧示例内容。
struct GalleryApp {
    titles: Vec<&'static str>,
    /// 当前示例的介绍与用法。
    active: usize,
    views: Vec<AnyView>,
}

impl GalleryApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let titles: Vec<&'static str> = vec![
            "hello_world",
            "a11y",
            "theme",
            "animation",
            "icons",
            "component_gallery",
            "extended_components",
            "scrollable",
            "sidebar",
            "tabs",
            "fps_hud",
            "inspector",
            "inspector_custom",
            "gradient",
            "painting",
            "pattern",
            "anchor",
            "popover",
        ];
        let builds: &[fn(&mut Window, &mut App) -> AnyView] = &[
            hello_world::build,
            a11y::build,
            theme::build,
            animation::build,
            icons::build,
            component_gallery::build,
            extended_components::build,
            scrollable::build,
            sidebar::build,
            tabs::build,
            fps_hud::build,
            inspector::build,
            inspector_custom::build,
            gradient::build,
            painting::build,
            pattern::build,
            anchor::build,
            popover::build,
        ];
        let views = builds.iter().map(|build| build(window, cx)).collect();
        Self {
            titles,
            active: 0,
            views,
        }
    }
}

impl Render for GalleryApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let theme = cx.theme();
        div()
            .id("gallery-app")
            .size_full()
            .flex()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                div()
                    .id("gallery-tabs")
                    .h_full()
                    .w(px(180.0))
                    .flex_shrink_0()
                    .flex_col()
                    .overflow_y_scroll()
                    .border_r_1()
                    .border_color(theme.border)
                    .children(self.titles.iter().enumerate().map(|(ix, title)| {
                        let this = this.clone();
                        Button::new(("gallery-tab", ix))
                            .label(*title)
                            .w_full()
                            .when(ix == self.active, |b| b.primary())
                            .when(ix != self.active, |b| b.ghost())
                            .on_click(move |_, _, cx| {
                                this.update(cx, |gallery, cx| {
                                    gallery.active = ix;
                                    cx.notify();
                                });
                            })
                    })),
            )
            .child(
                div()
                    .id("gallery-content")
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .overflow_hidden()
                    .child(self.views[self.active].clone()),
            )
    }
}

fn run() {
    // 传 -- <demo> 时保留原有独立窗口行为；不带参数时打开 gallery 导航界面。
    if let Some(name) = std::env::args().nth(1) {
        match name.as_str() {
            "hello_world" => hello_world::run_example(),
            "a11y" => a11y::run_example(),
            "theme" => theme::run_example(),
            "animation" => animation::run_example(),
            "daemon_app" => daemon_app::run_example(),
            "icons" => icons::run_example(),
            "component_gallery" => component_gallery::run_example(),
            "extended_components" => extended_components::run_example(),
            "scrollable" => scrollable::run_example(),
            "sidebar" => sidebar::run_example(),
            "tabs" => tabs::run_example(),
            "fps_hud" => fps_hud::run_example(),
            "inspector" => inspector::run_example(),
            "inspector_custom" => inspector_custom::run_example(),
            "gradient" => gradient::run_example(),
            "painting" => painting::run_example(),
            "pattern" => pattern::run_example(),
            "anchor" => anchor::run_example(),
            "popover" => popover::run_example(),
            _ => eprintln!("未知 demo：{name}"),
        }
        return;
    }
    application()
        .with_assets(GalleryAssets)
        .run(|cx: &mut App| {
            rgpui::theme::init(cx);
            rgpui::input_ui::init(cx);
            rgpui::menu::init(cx);
            let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| GalleryApp::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run();
}
