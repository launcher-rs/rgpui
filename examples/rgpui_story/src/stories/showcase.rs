//! 从 showcase 汇总的组件示例：按大类分组，每个示例一个 story。

use super::StoryItem;

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

mod i18n {
    //! 国际化（i18n）示例。
    //!
    //! 演示 `I18nManager` 的两种用法：
    //! - **全局管理器**：`cx.set_global(i18n)` 注册后，用 `I18nText::new(key).translate_global(cx)`
    //!   在渲染时取翻译；切换语言调 `cx.update_global::<I18nManager, _>(|m, _| m.set_locale("en"))`。
    //! - **局部管理器**：直接持有一个 `I18nManager`，用 `manager.t(key, &[("name", value)])`
    //!   取值（支持 `{name}` 变量插值），另有复数规则 `PluralRule`。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin i18n
    //! ```

    #![cfg_attr(target_family = "wasm", no_main)]

    use std::collections::HashMap;

    use rgpui::i18n::{I18nManager, I18nText, PluralRule};
    use rgpui::{
        App, Bounds, Context, IntoElement, ParentElement, Render, Window, WindowBounds,
        WindowOptions, div, h_flex, prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    struct I18nDemo {
        /// 局部管理器（不放进全局，演示直接持有用法）。
        local: I18nManager,
    }

    impl I18nDemo {
        fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
            // 构造一个带中英文的局部管理器。
            let mut local = I18nManager::new("zh-CN");
            local.load_translations_map(
                "zh-CN",
                HashMap::from([
                    ("greeting".to_string(), "你好，{name}！".to_string()),
                    ("welcome".to_string(), "欢迎使用 rgpui！".to_string()),
                ]),
            );
            local.load_translations_map(
                "en",
                HashMap::from([
                    ("greeting".to_string(), "你好，{name}！".to_string()),
                    ("welcome".to_string(), "Welcome to rgpui!".to_string()),
                ]),
            );
            Self { local }
        }
    }

    impl Render for I18nDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            // 局部管理器取值：变量插值。
            let greeting = self.local.t("greeting", &[("name", "开发者")]);
            let welcome = self.local.t("welcome", &[]);
            // 复数规则：1 用单数、5 用复数。
            let one = PluralRule::new(1, "item", "items");
            let many = PluralRule::new(5, "item", "items");
            let current = self.local.current_locale().to_string();

            v_flex()
            .id("i18n-demo")
            .size_full()
            .gap(px(16.0))
            .p(px(24.0))
            .bg(rgb(0xffffff))
            .child(div().text_2xl().child("国际化（i18n）"))
            // 1. 全局管理器：切换语言 + I18nText::translate_global。
            .child(
                v_flex()
                    .gap(px(8.0))
                    .p(px(12.0))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0xe0e0e0))
                    .child(div().text_lg().child("1. 全局管理器（cx.set_global）"))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .id("i18n-locale-zh")
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .rounded_md()
                                    .bg(rgb(0x0078d4))
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .child("中文")
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.update_global::<I18nManager, _>(|m, _| {
                                            m.set_locale("zh-CN");
                                        });
                                    })),
                            )
                            .child(
                                div()
                                    .id("i18n-locale-en")
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .rounded_md()
                                    .bg(rgb(0x107c10))
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .child("English")
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.update_global::<I18nManager, _>(|m, _| {
                                            m.set_locale("en");
                                        });
                                    })),
                            ),
                    )
                    .child(
                        div().text_sm().child(
                            I18nText::new("greeting")
                                .with_arg("name", "1.5")
                                .translate_global(cx),
                        ),
                    ),
            )
            // 2. 局部管理器：直接持有 + t(key, args) + 复数。
            .child(
                v_flex()
                    .gap(px(8.0))
                    .p(px(12.0))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0xe0e0e0))
                    .child(div().text_lg().child("2. 局部管理器（I18nManager）"))
                    .child(div().text_sm().child(format!("当前语言：{current}")))
                    .child(div().text_sm().child(format!("greeting：{greeting}")))
                    .child(div().text_sm().child(format!("welcome：{welcome}")))
                    .child(div().text_sm().child(format!(
                        "复数：{} / {}",
                        one.translate(&self.local),
                        many.translate(&self.local)
                    ))),
            )
            // 用法速查。
            .child(
                div()
                    .rounded_md()
                    .bg(rgb(0x282c34))
                    .p(px(12.0))
                    .text_sm()
                    .text_color(rgb(0xabb2bf))
                    .child(
                        "let mut i18n = I18nManager::new(\"zh-CN\");\n\
                         i18n.load_translations_map(\"en\", map);\n\
                         i18n.set_locale(\"en\");\n\
                         i18n.t(\"greeting\", &[(\"name\", \"rgpui\")]);\n\n\
                         // 全局：\n\
                         cx.set_global(i18n);\n\
                         I18nText::new(\"greeting\").with_arg(\"name\", \"rgpui\").translate_global(cx);",
                    ),
            )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            // 全局管理器：中英文各一条。
            let mut i18n = I18nManager::new("zh-CN");
            i18n.load_translations_map(
                "zh-CN",
                HashMap::from([("greeting".to_string(), "你好，{name}！".to_string())]),
            );
            i18n.load_translations_map(
                "en",
                HashMap::from([("greeting".to_string(), "你好，{name}！".to_string())]),
            );
            cx.set_global(i18n);

            let bounds = Bounds::centered(None, size(px(720.0), px(560.0)), cx);
            let _ = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| I18nDemo::new(window, cx)),
            );
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| I18nDemo::new(window, cx)).into()
    }
}

mod intl_text {
    //! 国际文本排版示例（bidi + 东亚断行）
    //!
    //! 演示两个核心模块：
    //!
    //! - `rgpui::bidi`：把「段落属于 LTR 还是 RTL」解析出来（HTML `dir=auto` 口径），
    //!   `TextAlign::Start` / `End` 据此对齐 —— 同一份布局在希伯来 / 阿拉伯文本下自动
    //!   靠右 / 靠左，不必让应用自己判断。
    //! - `rgpui::line_break`：CJK 逐字换行时的行首 / 行末禁则（禁则 / kinsoku）与胶连
    //!   字符规则 —— `。` 不再跑到行首、`「` 不再留在行末。
    //! - `rgpui::i18n`：语言标签 → 默认方向（`TextDirection::for_locale`），以及语言包
    //!   缺 key 审计（`I18nManager::validate`）。
    //!
    //! 运行：`cargo run -p showcase --bin intl_text`
    //!
    //! 说明：字形重排（UBA 完整实现）属于平台 shaping 的职责 —— Web / cosmic-text 后端
    //! 内部已用 `unicode-bidi` 完成；Windows DirectWrite 后端把整行交给 `IDWriteTextLayout`，
    //! 由 DirectWrite 自行重排。核心层负责的是段落方向与断行规则。

    #![cfg_attr(target_family = "wasm", no_main)]

    use std::collections::HashMap;

    use rgpui::bidi::{TextDirection, is_rtl_char, strong_direction};
    use rgpui::i18n::I18nManager;
    use rgpui::line_break::{break_class, can_break_between};
    use rgpui::{
        App, Bounds, Context, IntoElement, ParentElement, Pixels, Render, SharedString,
        Styled as _, TextAlign, TitlebarOptions, Window, WindowBounds, WindowOptions, div,
        prelude::*, px, size,
    };
    use rgpui_platform::application;

    struct DirectionSample {
        /// 样本文本。
        text: &'static str,
        /// 样例说明。
        note: &'static str,
    }

    const DIRECTION_SAMPLES: &[DirectionSample] = &[
        DirectionSample {
            text: "你好，世界",
            note: "拉丁文本",
        },
        DirectionSample {
            text: "Привет, мир",
            note: "西里尔文本",
        },
        DirectionSample {
            text: "你好，世界",
            note: "中文（LTR）",
        },
        DirectionSample {
            text: "שלום עולם",
            note: "希伯来文",
        },
        DirectionSample {
            text: "مرحبا بالعالم",
            note: "阿拉伯文",
        },
        DirectionSample {
            text: "«שלום»",
            note: "首个强方向字符在标点之后",
        },
        DirectionSample {
            text: "12 שלום",
            note: "数字是弱方向字符，会被跳过",
        },
        DirectionSample {
            text: "٣٤٥ 12.5%",
            note: "纯数字 / 百分号：回退为 LTR",
        },
    ];

    struct PairSample {
        /// 前一个字符。
        prev: char,
        /// 后一个字符。
        next: char,
        /// 判定说明。
        note: &'static str,
    }

    const PAIR_SAMPLES: &[PairSample] = &[
        PairSample {
            prev: '你',
            next: '好',
            note: "汉字之间可断行",
        },
        PairSample {
            prev: '好',
            next: '。',
            note: "行首禁则：句号不可起行",
        },
        PairSample {
            prev: '话',
            next: '」',
            note: "行首禁则：闭引号不可起行",
        },
        PairSample {
            prev: '「',
            next: '你',
            note: "行末禁则：开引号不可收行",
        },
        PairSample {
            prev: 'あ',
            next: 'ぁ',
            note: "行首禁则：小写假名不可起行",
        },
        PairSample {
            prev: 'カ',
            next: 'ー',
            note: "行首禁则：长音符不可起行",
        },
        PairSample {
            prev: 'a',
            next: '\u{00A0}',
            note: "胶连字符：不换行空格两侧不可断",
        },
        PairSample {
            prev: 'a',
            next: ')',
            note: "半角标点仍按拉丁规则处理",
        },
    ];

    const CLASS_SAMPLES: &[char] = &['汉', 'あ', 'ぁ', 'ー', '。', '「', '한', 'a', ' ', '+'];

    const CJK_SAMPLE: &str = "他说：「今天的天气很好，我们一起去公园散步吧。」（真的很适合散步。）";

    struct IntlTextDemo;

    impl IntlTextDemo {
        /// 创建示例视图。
        fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
            Self
        }
    }

    fn section(
        title: impl Into<SharedString>,
        body: impl IntoElement,
        cx: &App,
    ) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded(px(10.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.secondary)
            .child(div().text_sm().child(title.into()))
            .child(body)
    }

    fn direction_section(cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let rows = DIRECTION_SAMPLES.iter().map(|sample| {
            let direction = TextDirection::resolve(sample.text);
            let strong = sample
                .text
                .chars()
                .find_map(|c| strong_direction(c).map(|d| (c, d)));
            let strong_label = match strong {
                Some((c, TextDirection::Ltr)) => format!("首个强方向字符 {c:?} → LTR"),
                Some((c, TextDirection::Rtl)) => format!("首个强方向字符 {c:?} → RTL"),
                None => "没有强方向字符".to_string(),
            };

            div()
                .v_flex()
                .gap_1()
                .child(
                    div()
                        .w(px(260.0))
                        .px_3()
                        .py_2()
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.background)
                        // 关键点：`Start` 让文本自动靠向段落的起始边。
                        .text_align(TextAlign::Start)
                        .child(sample.text),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "{} · 解析方向 {} · {} · 含 RTL 字符：{}",
                            sample.note,
                            if direction.is_rtl() { "RTL" } else { "LTR" },
                            strong_label,
                            if sample.text.chars().any(is_rtl_char) {
                                "是"
                            } else {
                                "否"
                            },
                        )),
                )
        });

        section(
            "段落基准方向（HTML dir=auto 口径）—— 每个样例都用 text_align(Start) 排版",
            div().v_flex().gap_3().children(rows),
            cx,
        )
    }

    fn align_section(cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let text = "שלום עולם";
        let alphas = [
            (TextAlign::Left, "Left", "显式左对齐（与文字方向无关）"),
            (
                TextAlign::Start,
                "开始",
                "起始边：RTL 段落为右 —— 段落方向说话",
            ),
            (TextAlign::End, "End", "结束边：RTL 段落为左"),
            (TextAlign::Right, "Right", "显式右对齐"),
        ];

        let rows = alphas.into_iter().map(|(align, name, note)| {
            div().v_flex().gap_1().child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(200.0))
                            .px_3()
                            .py_2()
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.background)
                            .text_align(align)
                            .child(text),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{name} —— {note}")),
                    ),
            )
        });

        section(
            "start / end 随段落方向翻转",
            div().v_flex().gap_2().children(rows),
            cx,
        )
    }

    fn kinsoku_section(cx: &App) -> impl IntoElement {
        let theme = cx.theme();

        let narrow = |width: Pixels, cx: &App| {
            div()
                .w(width)
                .px_2()
                .py_2()
                .rounded(px(6.0))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .text_sm()
                .child(CJK_SAMPLE)
        };

        let pairs = PAIR_SAMPLES.iter().map(|sample| {
            let allowed = can_break_between(sample.prev, sample.next);
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .w(px(120.0))
                        .font_family("monospace")
                        .child(format!("{} + {}", sample.prev, sample.next)),
                )
                .child(
                    div()
                        .w(px(48.0))
                        .text_color(if allowed { theme.success } else { theme.danger })
                        .child(if allowed { "可断 ✓" } else { "禁则 ✗" }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(sample.note),
                )
        });

        section(
            "东亚禁则（kinsoku）：窄容器里换行点不会落在行首 / 行末禁则字符上",
            div()
                .v_flex()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_4()
                        .items_start()
                        .child(
                            div()
                                .v_flex()
                                .gap_1()
                                .child(div().text_xs().child("宽 160px"))
                                .child(narrow(px(160.0), cx)),
                        )
                        .child(
                            div()
                                .v_flex()
                                .gap_1()
                                .child(div().text_xs().child("宽 110px"))
                                .child(narrow(px(110.0), cx)),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "样例：{CJK_SAMPLE} —— 拉窄看换行点：句号不在行首、开引号不在行末。"
                        )),
                )
                .child(div().h(px(1.0)).bg(theme.border))
                .child(div().v_flex().gap_1().children(pairs)),
            cx,
        )
    }

    fn class_section(cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let rows = CLASS_SAMPLES.iter().map(|c| {
            let class = break_class(*c);
            let name = match class {
                rgpui::line_break::BreakClass::Ideographic => "汉字（可断）",
                rgpui::line_break::BreakClass::Kana => "假名（可断）",
                rgpui::line_break::BreakClass::Hangul => "谚文（可断）",
                rgpui::line_break::BreakClass::SmallKana => "小写假名（行首禁则）",
                rgpui::line_break::BreakClass::ProlongedSoundMark => "长音符（行首禁则）",
                rgpui::line_break::BreakClass::ClosePunctuation => "闭标点（行首禁则）",
                rgpui::line_break::BreakClass::OpenPunctuation => "开标点（行末禁则）",
                rgpui::line_break::BreakClass::Word => "单词字符",
                rgpui::line_break::BreakClass::Space => "空白",
                rgpui::line_break::BreakClass::Other => "其他",
            };
            div()
                .flex()
                .flex_row()
                .gap_3()
                .child(
                    div()
                        .w(px(60.0))
                        .font_family("monospace")
                        .child(c.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("{class:?} · {name}")),
                )
        });

        section(
            "break_class：断行规则眼中的字符类别",
            div().v_flex().gap_1().children(rows),
            cx,
        )
    }

    fn i18n_section(cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let locales = [
            "zh-CN", "en-US", "ja-JP", "ar-EG", "he-IL", "fa-IR", "ur-PK",
        ];
        let rows = locales.into_iter().map(|locale| {
            let direction = TextDirection::for_locale(locale);
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(div().w(px(80.0)).font_family("monospace").child(locale))
                .child(
                    div()
                        .w(px(48.0))
                        .text_xs()
                        .text_color(if direction.is_rtl() {
                            theme.danger
                        } else {
                            theme.success
                        })
                        .child(if direction.is_rtl() { "RTL" } else { "LTR" }),
                )
                .child(div().text_xs().text_color(theme.muted_foreground).child(
                    if direction.is_rtl() {
                        "外壳应镜像：滚动条在左、图标方向翻转"
                    } else {
                        "按 LTR 排版"
                    },
                ))
        });

        // 故意让 en 少一个 key，`validate` 会把它列出来。
        let mut i18n = I18nManager::new("zh-CN").with_fallback("en");
        i18n.load_translations_map(
            "zh-CN",
            HashMap::from([
                ("hello".to_string(), "你好".to_string()),
                ("bye".to_string(), "再见".to_string()),
            ]),
        );
        i18n.load_translations_map(
            "en",
            HashMap::from([("hello".to_string(), "Hello".to_string())]),
        );
        let problems = i18n.validate();

        section(
            "语言标签 → 默认方向；语言包审计（en 故意缺一个 key）",
            div()
                .v_flex()
                .gap_3()
                .child(div().v_flex().gap_1().children(rows))
                .child(div().h(px(1.0)).bg(theme.border))
                .child(
                    div().v_flex().gap_1().children(
                        problems
                            .into_iter()
                            .map(|problem| div().text_xs().text_color(theme.danger).child(problem)),
                    ),
                ),
            cx,
        )
    }

    impl Render for IntlTextDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let theme = cx.theme().clone();

            div()
                .v_flex()
                .size_full()
                .bg(theme.background)
                .text_color(theme.foreground)
                .child(
                    div()
                        .v_flex()
                        .gap_1()
                        .px_4()
                        .py_3()
                        .border_b_1()
                        .border_color(theme.border)
                        .child(div().text_lg().child("国际文本排版（bidi + 东亚断行）"))
                        .child(div().text_xs().text_color(theme.muted_foreground).child(
                            "方向判定取首个强方向字符；Start / End 随段落方向翻转；\
                                 换行遵守 CJK 行首 / 行末禁则与胶连字符。",
                        )),
                )
                .child(
                    div()
                        .id("intl-text-scroll")
                        .flex_grow_1()
                        .min_h(px(0.0))
                        .overflow_y_scroll()
                        .p_4()
                        .v_flex()
                        .gap_4()
                        .child(direction_section(cx))
                        .child(align_section(cx))
                        .child(kinsoku_section(cx))
                        .child(class_section(cx))
                        .child(i18n_section(cx)),
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
                    titlebar: Some(TitlebarOptions {
                        title: Some("国际文本示例".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| IntlTextDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| IntlTextDemo::new(window, cx)).into()
    }
}

mod markdown {
    //! Markdown 渲染示例。
    //!
    //! 演示两条 Markdown 相关能力：
    //! - **块级渲染**：`BlockRenderer::parse_markdown` 把 Markdown 拆成 `BlockElement`
    //!   （标题/段落/代码块/引用/分隔线…），再由调用方按 `BlockType` 自行渲染。
    //! - **插件式渲染**：`MarkdownRenderer` 注册内置插件（代码高亮/数学/任务列表）后
    //!   输出渲染结果字符串，并可自定义样式。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin markdown
    //! ```

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::markdown_ext::{CodeHighlightPlugin, MathPlugin, TaskListPlugin};
    use rgpui::{
        AnyElement, App, Bounds, Context, IntoElement, ParentElement, Render, Window, WindowBounds,
        WindowOptions, div, prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    struct MarkdownDemo {
        /// 待解析的 Markdown 文本。
        input: String,
        /// 插件式渲染器。
        renderer: MarkdownRenderer,
    }

    impl MarkdownDemo {
        fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
            // 注册内置插件：代码高亮、数学、任务列表。
            let mut renderer = MarkdownRenderer::new();
            renderer.register_plugin(Box::new(CodeHighlightPlugin));
            renderer.register_plugin(Box::new(MathPlugin));
            renderer.register_plugin(Box::new(TaskListPlugin));
            renderer.add_style("highlight", "background: yellow;");

            Self {
            input: "# 标题\n\n这是 **粗体** 和 *斜体* 段落。\n\n> 一段引用\n\n```rust\nfn main() {}\n```\n\n---\n"
                .to_string(),
            renderer,
        }
        }
    }

    impl Render for MarkdownDemo {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let renderer = BlockRenderer::new();
            let blocks = renderer.parse_markdown(&self.input);
            let plugin_output = self.renderer.render(&self.input);

            v_flex()
                .id("markdown-demo")
                .size_full()
                .gap(px(16.0))
                .p(px(24.0))
                .overflow_scroll()
                .bg(rgb(0xffffff))
                .child(div().text_2xl().child("Markdown 渲染"))
                // 1. 块级渲染。
                .child(section_title("1. 块级渲染（BlockRenderer）"))
                .child(
                    v_flex()
                        .gap(px(6.0))
                        .p(px(12.0))
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(0xe0e0e0))
                        .children(blocks.into_iter().map(render_block_element)),
                )
                // 2. 插件式渲染。
                .child(section_title("2. 插件式渲染（MarkdownRenderer）"))
                .child(
                    div()
                        .rounded_md()
                        .bg(rgb(0x282c34))
                        .p(px(12.0))
                        .text_sm()
                        .text_color(rgb(0xabb2bf))
                        .child(plugin_output),
                )
        }
    }

    fn section_title(text: &str) -> impl IntoElement {
        div().text_lg().child(text.to_string())
    }

    fn render_block_element(block: BlockElement) -> AnyElement {
        match &block.block_type {
            BlockType::Heading(level) => {
                let base = div()
                    .id(format!("md-h-{}", block.content))
                    .mb(px(4.0))
                    .text_color(rgb(0x1a1a1a));
                let styled = match level {
                    1 => base.text_2xl().font_bold(),
                    2 => base.text_xl().font_semibold(),
                    3 => base.text_lg().font_medium(),
                    _ => base.text_base(),
                };
                styled.child(block.content.clone()).into_any_element()
            }
            BlockType::Paragraph => div()
                .id(format!(
                    "md-p-{}",
                    block.content.chars().take(8).collect::<String>()
                ))
                .mb(px(8.0))
                .text_sm()
                .text_color(rgb(0x333333))
                .child(block.content.clone())
                .into_any_element(),
            BlockType::CodeBlock => {
                let lang = block
                    .attributes
                    .get("language")
                    .map(|s| s.as_str())
                    .unwrap_or("");
                div()
                    .id("md-code")
                    .mb(px(8.0))
                    .rounded_md()
                    .bg(rgb(0x282c34))
                    .p(px(12.0))
                    .child(
                        v_flex()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x888888))
                                    .child(format!("语言: {lang}")),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0xabb2bf))
                                    .child(block.content.clone()),
                            ),
                    )
                    .into_any_element()
            }
            BlockType::Blockquote => div()
                .id("md-quote")
                .mb(px(8.0))
                .pl(px(12.0))
                .border_l(px(3.0))
                .border_color(rgb(0x0078d4))
                .text_sm()
                .text_color(rgb(0x666666))
                .child(block.content.clone())
                .into_any_element(),
            BlockType::HorizontalRule => div()
                .id("md-hr")
                .my(px(12.0))
                .h(px(1.0))
                .bg(rgb(0xe9ecef))
                .into_any_element(),
            _ => div()
                .text_sm()
                .child(block.content.clone())
                .into_any_element(),
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(720.0), px(640.0)), cx);
            let _ = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| MarkdownDemo::new(window, cx)),
            );
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| MarkdownDemo::new(window, cx)).into()
    }
}

mod markdown_stream {
    //! Markdown 流式增量渲染示例。
    //!
    //! 演示 `MarkdownState` + `Markdown::new_state`：
    //! - 源码按小块逐步追加，模拟大模型逐 token 输出；
    //! - 每次追加后防抖 16ms 在后台整篇重解析，解析期间继续渲染上一版块列表，
    //!   因此流式输出时主线程不会每来一个 chunk 就重建整篇文档。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin markdown_stream
    //! ```

    use std::time::Duration;

    use rgpui::prelude::*;
    use rgpui::{
        App, Bounds, Button, Context, Entity, IntoElement, Render, Task, Window, WindowBounds,
        WindowOptions, div, h_flex, px, rgb, size, v_flex,
    };
    use rgpui_markdown::{Markdown, MarkdownState};
    use rgpui_platform::application;

    const SCRIPT: &str = r##"# 流式 Markdown

这条回答由一个字符一个字符地“生成”，但**只有解析完成后才整体替换**上一版内容。

## 能力清单

- 标题、段落与**粗体** / *斜体*
- 有序与无序列表
- 行内代码 `MarkdownState::append`
- 围栏代码块

```rust
let state = cx.new(|_| MarkdownState::new(""));
state.update(cx, |state, cx| state.append("# 你好", cx));
```

> 追加调用只更新源码并入队一次防抖解析；
> 渲染取的是最近一次成功的解析结果。

| 字段 | 含义 |
| --- | --- |
| `source` | 已接收的完整源码 |
| `parsing` | 是否有后台解析未完成 |

最后一段：滚动查看上方内容，解析状态行会在完成后变为“已就绪”。
"##;

    struct StreamingDemo {
        /// 持有源码与最近一次解析结果的实体。
        state: Entity<MarkdownState>,
        /// 已注入的字节数（用于进度展示）。
        received: usize,
        /// 整篇源码长度。
        total: usize,
        /// 是否已输出完毕。
        done: bool,
        /// 当前输出任务；重新播放时丢弃即取消旧任务。
        stream_task: Option<Task<()>>,
        /// 右上角帧率/绘制耗时浮层，用于观察流式输出期间的开销。
        hud: Entity<FpsHud>,
    }

    impl StreamingDemo {
        /// 创建视图并立即开始输出。
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            let state = cx.new(|_| MarkdownState::new(""));
            let hud_state = cx.new(|_| FpsHudState::new());
            let hud = cx.new(|_| FpsHud::new(hud_state).continuous(true));
            let mut demo = Self {
                state,
                received: 0,
                total: SCRIPT.len(),
                done: false,
                stream_task: None,
                hud,
            };
            demo.start(cx);
            demo
        }

        /// 清空并重新开始逐块输出。
        fn start(&mut self, cx: &mut Context<Self>) {
            self.received = 0;
            self.done = false;
            self.state.update(cx, |state, cx| state.set_source("", cx));

            let chunks = chunk_source(SCRIPT, 32);
            self.stream_task = Some(cx.spawn(async move |this, cx| {
                for chunk in chunks {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                    let alive = this.update(cx, |demo, cx| {
                        demo.received += chunk.len();
                        demo.state.update(cx, |state, cx| state.append(chunk, cx));
                        cx.notify();
                    });
                    if alive.is_err() {
                        return;
                    }
                }
                _ = this.update(cx, |demo, cx| {
                    demo.done = true;
                    cx.notify();
                });
            }));
        }
    }

    impl Render for StreamingDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let status = if self.done {
                format!("已就绪：{} 字节", self.total)
            } else {
                format!("输出中：{}/{} 字节", self.received, self.total)
            };

            v_flex()
                .relative()
                .size_full()
                .gap(px(12.0))
                .p(px(24.0))
                .bg(rgb(0xffffff))
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(div().text_xl().child("Markdown 流式增量渲染"))
                        .child(
                            Button::new("markdown-stream-restart")
                                .label("重新播放")
                                .on_click(cx.listener(|this, _, _window, cx| {
                                    this.start(cx);
                                })),
                        ),
                )
                .child(div().text_sm().text_color(rgb(0x666666)).child(status))
                .child(
                    v_flex()
                        .id("markdown-stream-scroll")
                        .flex_1()
                        .w_full()
                        .min_h(px(0.0))
                        .overflow_scroll()
                        .child(Markdown::new_state(self.state.clone()).w_full()),
                )
                .child(self.hud.clone())
        }
    }

    fn chunk_source(source: &'static str, chunk_bytes: usize) -> Vec<&'static str> {
        let mut chunks = Vec::new();
        let mut rest = source;
        while !rest.is_empty() {
            let mut end = chunk_bytes.min(rest.len());
            while !rest.is_char_boundary(end) {
                end -= 1;
            }
            let (chunk, tail) = rest.split_at(end);
            chunks.push(chunk);
            rest = tail;
        }
        chunks
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(760.0), px(680.0)), cx);
            let _ = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| StreamingDemo::new(window, cx)),
            );
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| StreamingDemo::new(window, cx)).into()
    }
}

mod rich_text {
    //! 富文本示例
    //!
    //! 演示 `rich_text` 文档模型（段落 + 行内运行）与只读渲染元素 `RichText`：
    //! 在字符坐标上对选区应用粗体/斜体/下划线/删除线/颜色，切换段落对齐与列表，
    //! 并演示纯文本 / HTML 双向转换与富剪贴板对接（复制为 HTML，粘贴时反解析）。
    //!
    //! 运行：`cargo run -p showcase --bin rich_text`

    use std::ops::Range;

    use rgpui::rich_text::{
        ListKind, Paragraph, RichText, RichTextDoc, RichTextHistory, Run, RunProps,
    };
    use rgpui::{
        App, Bounds, Button, ClipboardItem, Context, Entity, IntoElement, ParentElement, Render,
        SharedString, Styled as _, TextAlign, TitlebarOptions, Window, WindowBounds, WindowOptions,
        div, hsla, prelude::*, px, size,
    };
    use rgpui_platform::application;

    type StyleApply = fn(&mut RunProps);

    const STYLE_TARGETS: &[(&str, StyleApply, &str)] = &[
        ("加粗", |p| p.bold = true, "加粗"),
        ("斜体", |p| p.italic = true, "斜体"),
        ("下划线", |p| p.underline = true, "下划线"),
        ("删除线", |p| p.strikethrough = true, "删除线"),
    ];

    struct RichTextApp {
        /// 指向自身的句柄，供按钮回调更新状态。
        handle: Entity<Self>,
        /// 当前文档。
        doc: RichTextDoc,
        /// 撤销 / 重做历史。
        history: RichTextHistory,
        /// 最近一次操作的结果提示。
        status: SharedString,
        /// 查找命中的字符区间（渲染为搜索高亮）。
        highlight: Vec<std::ops::Range<usize>>,
    }

    impl RichTextApp {
        /// 创建示例应用（含一段带样式的样例文档）。
        fn new(cx: &mut Context<Self>) -> Self {
            let doc = sample_doc();
            Self {
                handle: cx.entity(),
                history: RichTextHistory::new(),
                doc,
                status: "点击按钮修改文档；右侧/下方可查看 HTML 序列化结果".into(),
                highlight: Vec::new(),
            }
        }

        /// 对当前文档的纯文本范围应用一次行内样式。
        fn style_word(&mut self, label: &str, f: impl Fn(&mut RunProps), cx: &mut Context<Self>) {
            let plain = self.doc.plain_text();
            match char_range(&plain, label) {
                Some(range) => {
                    self.history.edit(&mut self.doc, |doc| {
                        doc.apply_props(range, f);
                    });
                    self.status = format!("已对「{label}」应用样式").into();
                }
                None => self.status = format!("未在文档中找到「{label}」").into(),
            }
            cx.notify();
        }

        /// 设置首段对齐。
        fn align_title(&mut self, align: TextAlign, cx: &mut Context<Self>) {
            if let Some(para) = self.doc.paragraphs.first_mut() {
                para.style.align = align;
            }
            self.status = format!("标题对齐 → {align:?}").into();
            cx.notify();
        }

        /// 切换第二段（索引 1）的项目符号。
        fn toggle_bullet(&mut self, cx: &mut Context<Self>) {
            if let Some(para) = self.doc.paragraphs.get_mut(1) {
                para.style.list = if para.style.list == ListKind::Bullet {
                    ListKind::None
                } else {
                    ListKind::Bullet
                };
                self.status = format!("第二段列表 → {:?}", para.style.list).into();
            }
            cx.notify();
        }

        /// 清除全文字内样式。
        fn clear_inline(&mut self, cx: &mut Context<Self>) {
            let total = self.doc.char_len();
            self.doc
                .apply_props(0..total, |props| *props = RunProps::new());
            self.status = "已清除全文字内样式".into();
            cx.notify();
        }

        /// 把文档以「HTML + 纯文本」写入剪贴板。
        fn copy_html(&mut self, cx: &mut Context<Self>) {
            let html = self.doc.to_html();
            let plain = self.doc.plain_text();
            cx.write_to_clipboard(ClipboardItem::new_html(html, Some(plain)));
            self.status = "已复制为 HTML + 纯文本".into();
            cx.notify();
        }

        /// 从剪贴板读回并替换文档（优先 HTML，退回纯文本）。
        fn paste(&mut self, cx: &mut Context<Self>) {
            let Some(item) = cx.read_from_clipboard() else {
                self.status = "剪贴板为空或格式不受支持".into();
                cx.notify();
                return;
            };
            if let Some(html) = item.html()
                && let Some(doc) = RichTextDoc::from_html(html)
            {
                self.status = format!("已从 HTML 粘贴（{} 段）", doc.paragraphs.len()).into();
                self.doc = doc;
                cx.notify();
                return;
            }
            if let Some(text) = item.text() {
                self.doc = RichTextDoc::from_plain_text(&text);
                self.status = "剪贴板无 HTML，已按纯文本粘贴".into();
            } else {
                self.status = "剪贴板没有可用文本".into();
            }
            cx.notify();
        }

        /// 恢复样例文档。
        fn reset(&mut self, cx: &mut Context<Self>) {
            self.doc = sample_doc();
            self.status = "已重置为样例文档".into();
            cx.notify();
        }

        /// 给正文里的某个词加一条批注。
        fn add_annotation(&mut self, cx: &mut Context<Self>) {
            let plain = self.doc.plain_text();
            match char_range(&plain, "颜色") {
                Some(range) => {
                    let id = self.doc.add_annotation(range, "批注示例：这里用红色标注");
                    if let Some(annotation) = self.doc.annotation_mut(id) {
                        annotation.author = Some("示例".to_string());
                    }
                    self.status = format!("已添加批注 #{id}").into();
                }
                None => self.status = "未在文档中找到「颜色」".into(),
            }
            cx.notify();
        }

        /// 删除第一条批注。
        fn remove_first_annotation(&mut self, cx: &mut Context<Self>) {
            match self.doc.annotations.first().map(|item| item.id) {
                Some(id) => {
                    self.doc.remove_annotation(id);
                    self.status = format!("已删除批注 #{id}").into();
                }
                None => self.status = "当前没有批注".into(),
            }
            cx.notify();
        }

        /// 在正文末尾插入脚注引用。
        fn add_footnote(&mut self, cx: &mut Context<Self>) {
            let len = self.doc.char_len();
            let id = self.doc.insert_footnote(len, "*", "脚注示例：文末脚注内容");
            self.status = format!("已插入脚注 #{id}").into();
            cx.notify();
        }

        /// 删除第一条脚注（引用标记一并移除）。
        fn remove_first_footnote(&mut self, cx: &mut Context<Self>) {
            match self.doc.footnotes.first().map(|item| item.id) {
                Some(id) => {
                    self.doc.remove_footnote(id);
                    self.status = format!("已删除脚注 #{id}").into();
                }
                None => self.status = "当前没有脚注".into(),
            }
            cx.notify();
        }

        /// 查找并替换全部命中。
        fn replace_all(&mut self, needle: &str, with: &str, cx: &mut Context<Self>) {
            let hits = self.doc.find(needle, true);
            if hits.is_empty() {
                self.status = format!("未找到「{needle}」").into();
                cx.notify();
                return;
            }
            let mut count = 0usize;
            self.history.edit(&mut self.doc, |doc| {
                count = doc.replace_all(needle, with, true);
            });
            self.status = format!("把「{needle}」替换为「{with}」：命中 {count} 处").into();
            cx.notify();
        }

        /// 查找「加粗」并高亮命中区间。
        fn find_and_highlight(&mut self, cx: &mut Context<Self>) {
            self.highlight = self.doc.find("加粗", true);
            self.status = format!("命中 {} 处「加粗」", self.highlight.len()).into();
            cx.notify();
        }

        /// 清除查找高亮。
        fn clear_highlight(&mut self, cx: &mut Context<Self>) {
            self.highlight.clear();
            self.status = "已清除查找高亮".into();
            cx.notify();
        }

        /// 撤销一步。
        fn undo(&mut self, cx: &mut Context<Self>) {
            if self.history.undo(&mut self.doc) {
                self.status = "已撤销".into();
            } else {
                self.status = "没有可撤销的操作".into();
            }
            cx.notify();
        }

        /// 重做一步。
        fn redo(&mut self, cx: &mut Context<Self>) {
            if self.history.redo(&mut self.doc) {
                self.status = "已重做".into();
            } else {
                self.status = "没有可重做的操作".into();
            }
            cx.notify();
        }
    }

    impl Render for RichTextApp {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let handle = self.handle.clone();
            let mut buttons = div().flex().flex_row().flex_wrap().gap_2();

            // 行内样式按钮：命中样例文档中的固定词。
            for (needle, apply, label) in STYLE_TARGETS {
                let handle = handle.clone();
                let needle = needle.to_string();
                let title = label.to_string();
                buttons = buttons.child(
                    Button::new(format!("style-{needle}"))
                        .label(title)
                        .on_click(move |_event, _window, cx| {
                            handle.update(cx, |this, cx| this.style_word(&needle, *apply, cx));
                        }),
                );
            }

            // 颜色按钮（单独处理，因为要传具体色值）。
            buttons = buttons.child(Button::new("style-color").label("红色").on_click({
                let handle = handle.clone();
                move |_event, _window, cx| {
                    handle.update(cx, |this, cx| {
                        this.style_word("颜色", |p| p.color = Some(hsla(0.0, 0.85, 0.5, 1.0)), cx);
                    });
                }
            }));

            buttons = buttons
                .child(Button::new("clear-inline").label("清除字内样式").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| handle.update(cx, |this, cx| this.clear_inline(cx))
                }))
                .child(Button::new("align-center").label("标题居中").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| {
                        handle.update(cx, |this, cx| this.align_title(TextAlign::Center, cx));
                    }
                }))
                .child(Button::new("align-left").label("标题左对齐").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| {
                        handle.update(cx, |this, cx| this.align_title(TextAlign::Left, cx));
                    }
                }))
                .child(Button::new("bullet").label("切换第二段列表").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| handle.update(cx, |this, cx| this.toggle_bullet(cx))
                }))
                .child(Button::new("copy-html").label("复制为 HTML").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| handle.update(cx, |this, cx| this.copy_html(cx))
                }))
                .child(Button::new("add-annotation").label("添加批注").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| handle.update(cx, |this, cx| this.add_annotation(cx))
                }))
                .child(
                    Button::new("remove-annotation")
                        .label("删除首条批注")
                        .on_click({
                            let handle = handle.clone();
                            move |_event, _window, cx| {
                                handle.update(cx, |this, cx| this.remove_first_annotation(cx))
                            }
                        }),
                )
                .child(Button::new("add-footnote").label("插入脚注").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| handle.update(cx, |this, cx| this.add_footnote(cx))
                }))
                .child(
                    Button::new("remove-footnote")
                        .label("删除首条脚注")
                        .on_click({
                            let handle = handle.clone();
                            move |_event, _window, cx| {
                                handle.update(cx, |this, cx| this.remove_first_footnote(cx))
                            }
                        }),
                )
                .child(
                    Button::new("find-add")
                        .label("查找并高亮「加粗」")
                        .on_click({
                            let handle = handle.clone();
                            move |_event, _window, cx| {
                                handle.update(cx, |this, cx| this.find_and_highlight(cx))
                            }
                        }),
                )
                .child(Button::new("find-clear").label("清除高亮").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| {
                        handle.update(cx, |this, cx| this.clear_highlight(cx))
                    }
                }))
                .child(
                    Button::new("replace-all")
                        .label("替换「项目符号」→「条目」")
                        .on_click({
                            let handle = handle.clone();
                            move |_event, _window, cx| {
                                handle
                                    .update(cx, |this, cx| this.replace_all("项目符号", "条目", cx))
                            }
                        }),
                )
                .child(Button::new("undo").label("撤销").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| handle.update(cx, |this, cx| this.undo(cx))
                }))
                .child(Button::new("redo").label("重做").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| handle.update(cx, |this, cx| this.redo(cx))
                }))
                .child(Button::new("paste").label("从剪贴板粘贴").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| handle.update(cx, |this, cx| this.paste(cx))
                }))
                .child(
                    Button::new("reset")
                        .label("重置")
                        .on_click(move |_event, _window, cx| {
                            handle.update(cx, |this, cx| this.reset(cx))
                        }),
                );

            let html = self.doc.to_html();
            let html_preview = preview(&html, 320);
            // 批注点击：回传到应用状态（示例展示批注 id）。
            let annotation_handle = self.handle.clone();
            // 链接点击：回传到应用状态（示例展示链接地址）。
            let link_handle = self.handle.clone();

            // 批注 / 脚注清单（展示模型内容与区间）。
            let mut notes = div().flex().flex_col().gap_1().text_xs();
            for annotation in &self.doc.annotations {
                notes = notes.child(format!(
                    "批注 #{} [{}..{}] {}：{}",
                    annotation.id,
                    annotation.range.start,
                    annotation.range.end,
                    annotation.author.as_deref().unwrap_or("匿名"),
                    annotation.note
                ));
            }
            for footnote in &self.doc.footnotes {
                notes = notes.child(format!(
                    "脚注 #{} [{}..{}]：{}",
                    footnote.id, footnote.range.start, footnote.range.end, footnote.text
                ));
            }
            if self.doc.annotations.is_empty() && self.doc.footnotes.is_empty() {
                notes = notes.child("（暂无批注与脚注）");
            }

            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .size_full()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(div().text_xl().child("富文本示例"))
                .child(div().text_sm().child(self.status.clone()))
                .child(buttons)
                .child(
                    div()
                        .flex_1()
                        .min_h(px(160.0))
                        .p_3()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().secondary)
                        .child(
                            RichText::new(self.doc.clone())
                                .highlight_ranges(self.highlight.clone())
                                .on_annotation_click(move |id, _window, cx| {
                                    annotation_handle.update(cx, |this, cx| {
                                        this.status = format!("点击了批注 #{id}").into();
                                        cx.notify();
                                    });
                                })
                                .on_link_click(move |href, _window, cx| {
                                    link_handle.update(cx, |this, cx| {
                                        this.status = format!("点击了链接：{href}").into();
                                        cx.notify();
                                    });
                                }),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("批注与脚注："),
                )
                .child(
                    div()
                        .p_2()
                        .rounded_md()
                        .bg(cx.theme().secondary)
                        .text_xs()
                        .child(notes),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("HTML 序列化："),
                )
                .child(
                    div()
                        .p_2()
                        .rounded_md()
                        .bg(cx.theme().secondary)
                        .text_xs()
                        .child(html_preview),
                )
        }
    }

    fn sample_doc() -> RichTextDoc {
        let mut doc = RichTextDoc::new();

        let mut title = Paragraph::default();
        title.style.align = TextAlign::Center;
        title.style.heading = Some(1);
        title.runs = vec![Run::with("rgpui 富文本原型", RunProps::new().bold())];

        let body = Paragraph {
            style: Default::default(),
            runs: vec![
                Run::new("支持 "),
                Run::with("加粗", RunProps::new().bold()),
                Run::new("、"),
                Run::with("斜体", RunProps::new().italic()),
                Run::new("、"),
                Run::with("下划线", RunProps::new().underline()),
                Run::new("、"),
                Run::with("删除线", RunProps::new().strikethrough()),
                Run::new("，以及 "),
                Run::with("颜色", RunProps::new().color(hsla(0.0, 0.85, 0.5, 1.0))),
                Run::new("，还可以加 "),
                Run::with_link(
                    "链接",
                    RunProps::default(),
                    "https://github.com/launcher-rs/rgpui",
                ),
                Run::new("。"),
            ],
        };

        let mut bullet_a = Paragraph::new("项目符号一");
        bullet_a.style.list = ListKind::Bullet;
        let mut bullet_b = Paragraph::new("项目符号二");
        bullet_b.style.list = ListKind::Bullet;

        let mut number_a = Paragraph::new("编号一");
        number_a.style.list = ListKind::Number;
        let mut number_b = Paragraph::new("编号二");
        number_b.style.list = ListKind::Number;

        doc.paragraphs = vec![title, body, bullet_a, bullet_b, number_a, number_b];

        // 预置一条批注与一个脚注（正文高亮 + 文末脚注）。
        let plain = doc.plain_text();
        if let Some(range) = char_range(&plain, "加粗") {
            let id = doc.add_annotation(range, "批注示例：加粗处可再确认语气");
            if let Some(annotation) = doc.annotation_mut(id) {
                annotation.author = Some("示例".to_string());
            }
        }
        let len = doc.char_len();
        doc.insert_footnote(len, "*", "脚注示例：文末脚注内容");
        // 修订标记：插入为主色下划线，删除为删除线变暗。
        let revised = Paragraph::new("这一段演示修订：插入与删除。");
        doc.paragraphs.push(revised);
        doc.mark_inserted(0..4);
        doc.mark_deleted(10..13);

        // 分栏节（段落复用，按栏数排布）。
        doc.add_section(
            &[
                "左栏第一段。",
                "左栏第二段。",
                "右栏第一段。",
                "右栏第二段。",
            ],
            2,
            Some("分栏小节"),
        );

        doc
    }

    fn char_range(haystack: &str, needle: &str) -> Option<Range<usize>> {
        let start_byte = haystack.find(needle)?;
        let start = haystack[..start_byte].chars().count();
        let len = needle.chars().count();
        Some(start..start + len)
    }

    fn preview(text: &str, max: usize) -> String {
        if text.chars().count() <= max {
            return text.to_string();
        }
        let mut out: String = text.chars().take(max).collect();
        out.push('…');
        out
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(760.), px(620.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Rich Text Example".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(RichTextApp::new),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(RichTextApp::new).into()
    }
}

mod rich_clipboard {
    //! 富剪贴板示例
    //!
    //! 演示 `ClipboardItem` 的富负载能力（`ClipboardFormat` / `ClipboardEntry::Format`）：
    //! 复制时同时写入纯文本与 HTML（或 RTF）两种表示，粘贴时读回剪贴板上所有格式并逐条展示。
    //! 平台层负责把 MIME 映射到原生格式（Windows 注册格式 / `CF_HTML`、macOS UTType、
    //! Linux MIME atom），应用侧只管 MIME 与字节。
    //!
    //! 运行：`cargo run -p showcase --bin rich_clipboard`

    use rgpui::{
        App, Bounds, Button, ClipboardFormat, ClipboardItem, Context, Entity, IntoElement,
        ParentElement, Render, SharedString, Styled as _, TitlebarOptions, Window, WindowBounds,
        WindowOptions, div, prelude::*, px, size,
    };
    use rgpui_platform::application;

    const SAMPLE_TEXT: &str = "rgpui 富剪贴板";
    const SAMPLE_HTML: &str = "<p><b>rgpui</b> <i>富剪贴板</i></p>";
    const SAMPLE_RTF: &str = r"{\rtf1\ansi rgpui 富剪贴板}";
    const SAMPLE_CSV: &str = "name,qty\napple,3\npear,5";

    struct RichClipboardApp {
        /// 指向自身的句柄，供按钮回调更新状态
        self_handle: Entity<Self>,
        /// 最近一次操作的结果提示
        status: SharedString,
        /// 读回剪贴板后各格式的展示文本
        formats: Vec<SharedString>,
    }

    impl RichClipboardApp {
        /// 创建示例应用
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                self_handle: cx.entity(),
                status: "点击「复制」写入富剪贴板，再点「粘贴」读回".into(),
                formats: Vec::new(),
            }
        }

        /// 把给定项目写入系统剪贴板并回显条目数
        fn copy(&mut self, item: ClipboardItem, label: &str, cx: &mut Context<Self>) {
            let count = item.entries().len();
            cx.write_to_clipboard(item);
            self.status = format!("已复制 {label}（{count} 个条目）").into();
            cx.notify();
        }

        /// 读取系统剪贴板，列出其中的每一种格式
        fn paste(&mut self, cx: &mut Context<Self>) {
            let Some(item) = cx.read_from_clipboard() else {
                self.status = "剪贴板为空或格式不受支持".into();
                self.formats.clear();
                cx.notify();
                return;
            };

            self.formats = item
                .formats()
                .enumerate()
                .map(|(index, format)| {
                    let preview = format.text().unwrap_or("<二进制负载>");
                    SharedString::from(format!(
                        "富负载 #{}：{} = {}",
                        index + 1,
                        format.mime_type,
                        preview
                    ))
                })
                .collect();

            let plain = item.text().unwrap_or_else(|| "<无纯文本>".to_string());
            self.status = format!("纯文本：{plain}").into();
            cx.notify();
        }
    }

    impl Render for RichClipboardApp {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            // 复制用的三种项目：HTML、RTF、仅 CSV。
            let html_item = ClipboardItem::new_html(SAMPLE_HTML, Some(SAMPLE_TEXT.to_string()));
            let rtf_item = ClipboardItem::new_rtf(SAMPLE_RTF, Some(SAMPLE_TEXT.to_string()));
            let csv_item = ClipboardItem::new_with_format(ClipboardFormat::new_text(
                ClipboardFormat::CSV,
                SAMPLE_CSV,
            ));

            let handle = self.self_handle.clone();
            let copy_html = handle.clone();
            let copy_rtf = handle.clone();
            let copy_csv = handle.clone();
            let paste = handle;

            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .size_full()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(div().text_xl().child("富剪贴板示例"))
                .child(div().text_sm().child(self.status.clone()))
                .child(
                    div()
                        .p_3()
                        .rounded_md()
                        .bg(cx.theme().secondary)
                        .text_sm()
                        .child("源内容：rgpui 富剪贴板（HTML 中 b/i 已标记）"),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            Button::new("copy-html")
                                .label("复制为 HTML + 纯文本")
                                .on_click({
                                    let item = html_item.clone();
                                    move |_event, _window, cx| {
                                        copy_html.update(cx, |this, cx| {
                                            this.copy(item.clone(), "HTML + 纯文本", cx)
                                        });
                                    }
                                }),
                        )
                        .child(
                            Button::new("copy-rtf")
                                .label("复制为 RTF + 纯文本")
                                .on_click({
                                    let item = rtf_item.clone();
                                    move |_event, _window, cx| {
                                        copy_rtf.update(cx, |this, cx| {
                                            this.copy(item.clone(), "RTF + 纯文本", cx)
                                        });
                                    }
                                }),
                        )
                        .child(Button::new("copy-csv").label("复制为 CSV").on_click({
                            let item = csv_item.clone();
                            move |_event, _window, cx| {
                                copy_csv.update(cx, |this, cx| this.copy(item.clone(), "CSV", cx));
                            }
                        }))
                        .child(Button::new("paste").label("粘贴并列出格式").on_click(
                            move |_event, _window, cx| {
                                paste.update(cx, |this, cx| this.paste(cx));
                            },
                        )),
                )
                .children(self.formats.iter().cloned().map(|line| {
                    div()
                        .p_2()
                        .rounded_md()
                        .bg(cx.theme().secondary)
                        .text_sm()
                        .child(line)
                }))
                .child(div().text_xs().child(format!(
                    "待复制条目数：HTML {} / RTF {} / CSV {}",
                    html_item.entries().len(),
                    rtf_item.entries().len(),
                    csv_item.entries().len()
                )))
        }
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(720.), px(460.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Rich Clipboard Example".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(RichClipboardApp::new),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(RichClipboardApp::new).into()
    }
}

mod text_layout {

    //! 文本布局示例：展示段落换行、对齐与行布局缓存。
    //! 用法：左侧 tab 选「text_layout」，查看长文本在不同宽度下的换行表现。
    use rgpui::{
        App, Bounds, Context, FontStyle, FontWeight, StyledText, Window, WindowBounds,
        WindowOptions, div, prelude::*, px, size,
    };
    use rgpui_platform::application;

    struct HelloWorld {}

    impl Render for HelloWorld {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .bg(rgpui::white())
                .flex()
                .flex_col()
                .gap_2()
                .p_4()
                .size_full()
                .child(div().child("Text left"))
                .child(div().text_center().child("Text center"))
                .child(div().text_right().child("Text right"))
                .child(div().text_decoration_1().child("Text left (underline)"))
                .child(
                    div()
                        .text_center()
                        .text_decoration_1()
                        .child("Text center (underline)"),
                )
                .child(
                    div()
                        .text_right()
                        .text_decoration_1()
                        .child("Text right (underline)"),
                )
                .child(div().line_through().child("Text left (line_through)"))
                .child(
                    div()
                        .text_center()
                        .line_through()
                        .child("Text center (line_through)"),
                )
                .child(
                    div()
                        .text_right()
                        .line_through()
                        .child("Text right (line_through)"),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .justify_between()
                        .child(
                            div()
                                .w(px(400.))
                                .border_1()
                                .border_color(rgpui::blue())
                                .p_1()
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_center()
                                .child("A long non-wrapping text align center"),
                        )
                        .child(
                            div()
                                .w_32()
                                .border_1()
                                .border_color(rgpui::blue())
                                .p_1()
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_right()
                                .child("100%"),
                        ),
                )
                .child(div().flex().gap_2().justify_between().child(
                    StyledText::new("ABCD").with_highlights([
                        (0..1, FontWeight::EXTRA_BOLD.into()),
                        (2..3, FontStyle::Italic.into()),
                    ]),
                ))
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| HelloWorld {}),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| HelloWorld {}).into()
    }
}

mod text_wrapper {

    //! 文本换行器示例：展示 `TextWrapper` 按词/字符边界断行。
    //! 用法：左侧 tab 选「text_wrapper」，调整宽度观察断行结果。
    use rgpui::{
        App, Bounds, Context, TextOverflow, Window, WindowBounds, WindowOptions, div, prelude::*,
        px, size,
    };
    use rgpui_platform::application;

    struct HelloWorld {}

    impl Render for HelloWorld {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let text = "The longest word 你好世界这段是中文，こんにちはこの段落は日本語です in any of the major \
            English language dictionaries is pneumonoultramicroscopicsilicovolcanoconiosis, a word that \
            refers to a lung disease contracted from the inhalation of very fine silica particles, \
            a url https://github.com/zed-industries/zed/pull/35724?query=foo&bar=2, \
            specifically from a volcano; medically, it is the same as silicosis.";
            div()
                .id("page")
                .size_full()
                .flex()
                .flex_col()
                .p_2()
                .gap_2()
                .bg(rgpui::white())
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_shrink_0()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .border_1()
                                .border_color(rgpui::red())
                                .text_ellipsis()
                                .child("longer text in flex 1"),
                        )
                        .child(
                            div()
                                .flex()
                                .border_1()
                                .border_color(rgpui::red())
                                .text_ellipsis()
                                .child("short flex"),
                        )
                        .child(
                            div()
                                .overflow_hidden()
                                .border_1()
                                .border_color(rgpui::red())
                                .text_ellipsis()
                                .w_full()
                                .child("A short text in normal div"),
                        ),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_xl()
                        .truncate()
                        .border_1()
                        .border_color(rgpui::blue())
                        .child("ELLIPSIS: ".to_owned() + text),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_xl()
                        .overflow_hidden()
                        .text_ellipsis()
                        .line_clamp(2)
                        .border_1()
                        .border_color(rgpui::blue())
                        .child("ELLIPSIS 2 lines: ".to_owned() + text),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_xl()
                        .overflow_hidden()
                        .text_overflow(TextOverflow::Truncate("".into()))
                        .border_1()
                        .border_color(rgpui::green())
                        .child("TRUNCATE: ".to_owned() + text),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_xl()
                        .overflow_hidden()
                        .text_overflow(TextOverflow::Truncate("".into()))
                        .line_clamp(3)
                        .border_1()
                        .border_color(rgpui::green())
                        .child("TRUNCATE 3 lines: ".to_owned() + text),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_xl()
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .border_1()
                        .border_color(rgpui::black())
                        .child("NOWRAP: ".to_owned() + text),
                )
                .child(div().text_xl().w_full().child(text))
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| HelloWorld {}),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| HelloWorld {}).into()
    }
}

mod ink {
    //! Ink 笔迹示例
    //!
    //! 演示 `ink` 墨迹层：鼠标拖拽用钢笔 / 荧光笔落笔，橡皮按半径擦除，套索框选后删除；
    //! 另含撤销与清空。笔画以中点二次贝塞尔平滑绘制，荧光笔叠加半透明；打开「压感笔」后
    //! 按轨迹长度合成压感，笔画逐段变宽（真实数位板压感走同一接口）。
    //!
    //! 多图层：`InkDocument` 管理图层（新增 / 切换 / 隐藏 / 锁定），编辑落在激活图层，
    //! 可见层逐层绘制并把图层不透明度传给 `InkLayerElement`。
    //!
    //! 运行：`cargo run -p showcase --bin ink`

    use rgpui::ink::{InkDocument, InkLayerElement, InkStyle, InkTool};
    use rgpui::{
        App, Bounds, Button, Context, IntoElement, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
        ParentElement, Pixels, Point, Render, SharedString, Styled as _, TitlebarOptions, Window,
        WindowBounds, WindowOptions, div, hsla, prelude::*, px, size,
    };
    use rgpui_platform::application;

    const PEN_WIDTH: Pixels = px(3.0);
    const HIGHLIGHTER_WIDTH: Pixels = px(16.0);
    const ERASER_RADIUS: Pixels = px(14.0);

    struct InkApp {
        /// 多图层墨迹文档。
        doc: InkDocument,
        /// 当前工具。
        tool: InkTool,
        /// 进行中的套索多边形。
        lasso: Vec<Point<Pixels>>,
        /// 橡皮是否局部擦除（false = 整笔擦除）。
        local_erase: bool,
        /// 套索选中的 `(图层, 笔画)`。
        selected: Vec<(usize, usize)>,
        /// 是否处于落笔状态。
        drawing: bool,
        /// 是否用合成压感绘制（演示逐段变宽）。
        pressure: bool,
        /// 本次笔画的起点（用来算合成压感）。
        stroke_origin: Option<Point<Pixels>>,
        /// 状态提示。
        status: SharedString,
    }

    impl InkApp {
        /// 创建示例应用。
        fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
            Self {
                doc: InkDocument::new(),
                tool: InkTool::Pen,
                local_erase: false,
                lasso: Vec::new(),
                selected: Vec::new(),
                drawing: false,
                pressure: false,
                stroke_origin: None,
                status: "拖拽绘制 · 切换工具试试橡皮与套索".into(),
            }
        }

        /// 当前工具对应的画笔样式。
        fn style(&self) -> InkStyle {
            match self.tool {
                InkTool::Highlighter => {
                    InkStyle::highlighter(hsla(0.13, 1.0, 0.5, 1.0), HIGHLIGHTER_WIDTH)
                }
                _ => InkStyle::pen(hsla(0.6, 0.75, 0.35, 1.0), PEN_WIDTH),
            }
        }

        /// 落笔。
        fn on_down(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
            self.drawing = true;
            self.selected.clear();
            self.stroke_origin = Some(position);
            match self.tool {
                InkTool::Pen | InkTool::Highlighter => {
                    let started = if self.pressure {
                        self.doc
                            .begin_stroke_with_pressure(position, 0.35, self.style())
                    } else {
                        self.doc.begin_stroke(position, self.style())
                    };
                    if !started {
                        self.status = "当前图层已隐藏或锁定，无法落笔".into();
                    }
                }
                InkTool::Eraser => {
                    if self.local_erase {
                        // 局部擦除：只剪掉圆内的采样点，笔画被切成两段。
                        let removed = self.doc.erase_local(position, ERASER_RADIUS);
                        self.status = format!("局部擦除 {removed} 个采样点").into();
                    } else {
                        let removed = self.doc.erase_at(position, ERASER_RADIUS);
                        self.status = format!("擦除 {removed} 条笔画").into();
                    }
                }
                InkTool::Lasso => self.lasso = vec![position],
            }
            cx.notify();
        }

        /// 合成本次采样的压感：沿笔画长度做正弦起伏，便于观察逐段变宽。
        fn synthetic_pressure(&self, position: Point<Pixels>) -> f32 {
            let Some(origin) = self.stroke_origin else {
                return 1.0;
            };
            let dx = (position.x - origin.x).as_f32();
            let dy = (position.y - origin.y).as_f32();
            let distance = (dx * dx + dy * dy).sqrt();
            0.25 + 0.75 * (0.5 + 0.5 * (distance / 45.0).sin())
        }

        /// 拖拽中。
        fn on_move(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
            if !self.drawing {
                return;
            }
            match self.tool {
                InkTool::Pen | InkTool::Highlighter => {
                    if self.pressure {
                        let pressure = self.synthetic_pressure(position);
                        self.doc.extend_stroke_with_pressure(position, pressure);
                    } else {
                        self.doc.extend_stroke(position);
                    }
                }
                InkTool::Eraser => {
                    self.doc.erase_at(position, ERASER_RADIUS);
                }
                InkTool::Lasso => self.lasso.push(position),
            }
            cx.notify();
        }

        /// 抬笔。
        fn on_up(&mut self, cx: &mut Context<Self>) {
            self.stroke_origin = None;
            if !self.drawing {
                return;
            }
            self.drawing = false;
            match self.tool {
                InkTool::Pen | InkTool::Highlighter => {
                    self.doc.end_stroke();
                }
                InkTool::Lasso => {
                    self.selected = self.doc.lasso(&self.lasso);
                    self.status = format!("套索选中 {} 条笔画", self.selected.len()).into();
                    self.lasso.clear();
                }
                InkTool::Eraser => {}
            }
            cx.notify();
        }

        /// 切换工具。
        fn set_tool(&mut self, tool: InkTool, cx: &mut Context<Self>) {
            self.tool = tool;
            self.status = format!("当前工具：{tool:?}").into();
            cx.notify();
        }

        /// 删除选中的笔画（跨图层，锁定 / 隐藏层跳过）。
        fn delete_selected(&mut self, cx: &mut Context<Self>) {
            let count = self.selected.len();
            self.doc.remove(&self.selected);
            self.selected.clear();
            self.status = format!("删除 {count} 条选中笔画").into();
            cx.notify();
        }

        /// 撤销激活图层最近一条笔画。
        fn undo(&mut self, cx: &mut Context<Self>) {
            self.selected.clear();
            let undone = self.doc.undo();
            self.status = if undone {
                "已撤销一条笔画".into()
            } else {
                "激活图层没有可撤销的笔画".into()
            };
            cx.notify();
        }

        /// 清空激活图层。
        fn clear(&mut self, cx: &mut Context<Self>) {
            self.doc.clear();
            self.selected.clear();
            self.status = "已清空激活图层".into();
            cx.notify();
        }

        /// 新建图层并激活。
        fn add_layer(&mut self, cx: &mut Context<Self>) {
            let name = format!("图层 {}", self.doc.layer_count() + 1);
            let index = self.doc.add_layer(name);
            self.status = format!("新建图层 #{}", index + 1).into();
            cx.notify();
        }

        /// 切换到下一图层。
        fn next_layer(&mut self, cx: &mut Context<Self>) {
            let next = (self.doc.active_index() + 1) % self.doc.layer_count();
            self.doc.set_active(next);
            self.status = format!("激活图层 #{}", next + 1).into();
            cx.notify();
        }

        /// 隐藏 / 显示激活图层。
        fn toggle_visible(&mut self, cx: &mut Context<Self>) {
            let index = self.doc.active_index();
            let visible = !self.doc.active().visible;
            self.doc.set_visible(index, visible);
            self.status = if visible {
                "显示激活图层".into()
            } else {
                "隐藏激活图层".into()
            };
            cx.notify();
        }

        /// 锁定 / 解锁激活图层。
        fn toggle_locked(&mut self, cx: &mut Context<Self>) {
            let index = self.doc.active_index();
            let locked = !self.doc.active().locked;
            self.doc.set_locked(index, locked);
            self.status = if locked {
                "锁定激活图层".into()
            } else {
                "解锁激活图层".into()
            };
            cx.notify();
        }
    }

    impl Render for InkApp {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let tool_button = |id: &'static str, label: &'static str, tool: InkTool| {
                Button::new(id).label(label).on_click(cx.listener(
                    move |this, _event, _window, cx| {
                        this.set_tool(tool, cx);
                    },
                ))
            };

            let active = self.doc.active_index();
            let layer_count = self.doc.layer_count();
            // 可见图层逐层绘制：套索只画在激活层，高亮按层过滤。
            let layer_elements: Vec<rgpui::AnyElement> = self
                .doc
                .visible_layers()
                .map(|(index, layer)| {
                    let highlight = self
                        .selected
                        .iter()
                        .filter(|(selected_layer, _)| *selected_layer == index)
                        .map(|(_, stroke)| *stroke)
                        .collect();
                    let lasso = if index == active {
                        self.lasso.clone()
                    } else {
                        Vec::new()
                    };
                    InkLayerElement::new(layer.layer.clone())
                        .opacity(layer.opacity)
                        .highlight(highlight)
                        .lasso(lasso)
                        // 每层把笔画暴露给辅助技术（画布合成子节点，播报点数 / 工具）。
                        .a11y(("ink-layer", index), layer.name.clone())
                        .into_any_element()
                })
                .collect();
            let active_layer = self.doc.active().clone();
            let active_strokes = active_layer.layer.strokes.len();
            let selected_count = self.selected.len();
            let selected_label = if self.selected.is_empty() {
                String::new()
            } else {
                format!(" · 选中 {selected_count}")
            };

            div()
                .relative()
                .size_full()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                // 墨迹画布层：铺满窗口（坐标即窗口坐标），在最底层。
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .children(layer_elements)
                        .on_mouse_down(
                            rgpui::MouseButton::Left,
                            cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                                this.on_down(ev.position, cx);
                            }),
                        )
                        .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
                            this.on_move(ev.position, cx);
                        }))
                        .on_mouse_up(
                            rgpui::MouseButton::Left,
                            cx.listener(|this, _ev: &MouseUpEvent, _window, cx| this.on_up(cx)),
                        ),
                )
                // 工具条：绘制在画布之上。
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_3()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .gap_2()
                                .child(tool_button("tool-pen", "钢笔", InkTool::Pen))
                                .child(tool_button(
                                    "tool-highlighter",
                                    "荧光笔",
                                    InkTool::Highlighter,
                                ))
                                .child(tool_button("tool-eraser", "橡皮", InkTool::Eraser))
                                .child(tool_button("tool-lasso", "套索", InkTool::Lasso))
                                .child(
                                    Button::new("pressure")
                                        .label(if self.pressure {
                                            "压感笔：开"
                                        } else {
                                            "压感笔：关"
                                        })
                                        .on_click(cx.listener(|this, _event, _window, cx| {
                                            this.pressure = !this.pressure;
                                            this.status = if this.pressure {
                                                "压感笔：笔画按合成压感逐段变宽".into()
                                            } else {
                                                "压感笔：固定宽度".into()
                                            };
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("local-erase")
                                        .label(if self.local_erase {
                                            "橡皮：局部"
                                        } else {
                                            "橡皮：整笔"
                                        })
                                        .on_click(cx.listener(|this, _event, _window, cx| {
                                            this.local_erase = !this.local_erase;
                                            cx.notify();
                                        })),
                                )
                                .child(Button::new("undo").label("撤销").on_click(cx.listener(
                                    |this, _event, _window, cx| {
                                        this.undo(cx);
                                    },
                                )))
                                .child(Button::new("delete-selected").label("删除选中").on_click(
                                    cx.listener(|this, _event, _window, cx| {
                                        this.delete_selected(cx);
                                    }),
                                ))
                                .child(Button::new("clear").label("清空").on_click(
                                    cx.listener(|this, _event, _window, cx| this.clear(cx)),
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .gap_2()
                                .child(Button::new("add-layer").label("新建图层").on_click(
                                    cx.listener(|this, _event, _window, cx| this.add_layer(cx)),
                                ))
                                .child(Button::new("next-layer").label("切换图层").on_click(
                                    cx.listener(|this, _event, _window, cx| this.next_layer(cx)),
                                ))
                                .child(Button::new("toggle-visible").label("隐藏/显示").on_click(
                                    cx.listener(|this, _event, _window, cx| {
                                        this.toggle_visible(cx);
                                    }),
                                ))
                                .child(Button::new("toggle-locked").label("锁定/解锁").on_click(
                                    cx.listener(|this, _event, _window, cx| {
                                        this.toggle_locked(cx);
                                    }),
                                )),
                        )
                        .child(div().text_sm().child(self.status.clone()))
                        .child(div().text_xs().child(format!(
                            "图层 {}/{}（{}）· 激活层笔画 {} · 可见层 {}{}",
                            active + 1,
                            layer_count,
                            if active_layer.visible {
                                "可见"
                            } else {
                                "隐藏"
                            },
                            active_strokes,
                            self.doc.visible_layers().count(),
                            selected_label,
                        ))),
                )
        }
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(760.), px(560.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Ink Example".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| InkApp::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| InkApp::new(window, cx)).into()
    }
}

mod sorter {
    //! 缩略图排序器示例
    //!
    //! 演示 `sorter` 缩略图网格：拖拽缩略图重排（拖拽时显示插入指示线）、缩放、
    //! 多选（`Ctrl` 点选 / `Shift` 范围选 / `Ctrl+A` 全选）、键盘导航（方向键移动、
    //! `Enter` 激活、`Delete` 请求删除）。重排后由应用按事件真正调整数据顺序。
    //!
    //! 运行：`cargo run -p showcase --bin sorter`

    use rgpui::sorter::{Sorter, SorterDelegate, SorterEvent, SorterState};
    use rgpui::{
        App, Bounds, Button, Context, Entity, Hsla, IntoElement, ParentElement as _, Render,
        SharedString, Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div, hsla,
        prelude::*, px, size, v_flex,
    };
    use rgpui_platform::application;

    struct Slide {
        /// 标题。
        title: SharedString,
        /// 封面底色。
        color: Hsla,
    }

    struct SlideDeck {
        /// 幻灯片列表（顺序即缩略图顺序）。
        slides: Vec<Slide>,
    }

    impl SlideDeck {
        /// 构造若干示例幻灯片。
        fn demo() -> Self {
            let slides = (0..12)
                .map(|index| Slide {
                    title: format!("第 {} 页", index + 1).into(),
                    color: hsla((index as f32 * 0.08) % 1.0, 0.55, 0.55, 1.0),
                })
                .collect();
            Self { slides }
        }
    }

    impl SorterDelegate for SlideDeck {
        fn count(&self, _cx: &App) -> usize {
            self.slides.len()
        }

        fn aspect_ratio(&self, _cx: &App) -> f32 {
            4.0 / 3.0
        }

        fn render_item(
            &mut self,
            index: usize,
            selected: bool,
            _window: &mut Window,
            _cx: &mut Context<SorterState<Self>>,
        ) -> impl IntoElement {
            let slide = &self.slides[index];
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(slide.color)
                .text_color(hsla(0.0, 0.0, 1.0, 1.0))
                .font_weight(rgpui::FontWeight::BOLD)
                .child(if selected {
                    format!("✓ {}", slide.title)
                } else {
                    slide.title.to_string()
                })
        }
    }

    struct SorterApp {
        /// 排序器状态。
        sorter: Entity<SorterState<SlideDeck>>,
        /// 状态栏文案。
        status: SharedString,
    }

    impl SorterApp {
        /// 创建示例应用并订阅排序器事件。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let sorter = cx.new(|cx| SorterState::new(SlideDeck::demo(), cx));

            cx.subscribe_in(
                &sorter,
                window,
                |this: &mut Self, state, event: &SorterEvent, _window, cx| match event {
                    SorterEvent::Reorder { from, to } => {
                        state.update(cx, |state, cx| {
                            let slides = &mut state.delegate_mut().slides;
                            let slide = slides.remove(*from);
                            slides.insert(*to, slide);
                            cx.notify();
                        });
                        this.status = format!("把第 {} 页移动到第 {} 位", from + 1, to + 1).into();
                    }
                    SorterEvent::ReorderMany { from, to } => {
                        state.update(cx, |state, cx| {
                            let slides = &mut state.delegate_mut().slides;
                            // 先按原索引抽出整组，再整组插入到落位索引
                            let len = slides.len();
                            let moved: Vec<_> = from
                                .iter()
                                .rev()
                                .filter(|index| **index < len)
                                .map(|index| slides.remove(*index))
                                .collect();
                            for (offset, slide) in moved.into_iter().rev().enumerate() {
                                slides.insert(to + offset, slide);
                            }
                            cx.notify();
                        });
                        this.status =
                            format!("整组移动 {} 页到第 {} 位", from.len(), to + 1).into();
                    }
                    SorterEvent::SelectionChanged(indices) => {
                        this.status = format!("选中 {} 项", indices.len()).into();
                    }
                    SorterEvent::Activate(index) => {
                        this.status = format!("激活第 {} 页", index + 1).into();
                    }
                    SorterEvent::Delete(indices) => {
                        let mut indices = indices.clone();
                        indices.sort_unstable();
                        state.update(cx, |state, cx| {
                            let slides = &mut state.delegate_mut().slides;
                            for index in indices.iter().rev() {
                                if *index < slides.len() {
                                    slides.remove(*index);
                                }
                            }
                            cx.notify();
                        });
                        this.status = format!("删除 {} 页", indices.len()).into();
                    }
                },
            )
            .detach();

            Self {
                sorter,
                status: "拖拽缩略图重排 · Ctrl 点选 · Shift 范围选 · Delete 删除".into(),
            }
        }

        /// 追加一张幻灯片。
        fn add_slide(&mut self, cx: &mut Context<Self>) {
            self.sorter.update(cx, |state, cx| {
                let deck = state.delegate_mut();
                let number = deck.slides.len() + 1;
                deck.slides.push(Slide {
                    title: format!("第 {number} 页").into(),
                    color: hsla((number as f32 * 0.08) % 1.0, 0.55, 0.55, 1.0),
                });
                cx.notify();
            });
            self.status = "已新建一页".into();
        }

        /// 删除当前选中项。
        fn delete_selected(&mut self, cx: &mut Context<Self>) {
            let indices: Vec<usize> = self.sorter.read(cx).selected().iter().copied().collect();
            if indices.is_empty() {
                self.status = "没有选中项".into();
                return;
            }
            self.sorter.update(cx, |state, cx| {
                let slides = &mut state.delegate_mut().slides;
                for index in indices.iter().rev() {
                    if *index < slides.len() {
                        slides.remove(*index);
                    }
                }
                cx.notify();
            });
            self.status = format!("删除 {} 页", indices.len()).into();
        }
    }

    impl Render for SorterApp {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let zoom_out = {
                let sorter = self.sorter.clone();
                Button::new("zoom-out").label("缩小").on_click(cx.listener(
                    move |_this, _event, _window, cx| {
                        sorter.update(cx, |state, cx| state.zoom_by(-0.2, cx));
                    },
                ))
            };
            let zoom_in = {
                let sorter = self.sorter.clone();
                Button::new("zoom-in").label("放大").on_click(cx.listener(
                    move |_this, _event, _window, cx| {
                        sorter.update(cx, |state, cx| state.zoom_by(0.2, cx));
                    },
                ))
            };

            v_flex()
                .size_full()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .p_3()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(zoom_out)
                        .child(zoom_in)
                        .child(
                            Button::new("add").label("新建").on_click(
                                cx.listener(|this, _event, _window, cx| this.add_slide(cx)),
                            ),
                        )
                        .child(Button::new("delete").label("删除选中").on_click(
                            cx.listener(|this, _event, _window, cx| this.delete_selected(cx)),
                        ))
                        .child(div().text_sm().child(self.status.clone())),
                )
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .child(Sorter::new(&self.sorter)),
                )
        }
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(900.), px(640.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Sorter Example".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| SorterApp::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| SorterApp::new(window, cx)).into()
    }
}

mod timeline {
    //! 动画时间线示例
    //!
    //! 演示 `anim_timeline`：多轨关键帧时间线（标尺刻度 + 值曲线 + 关键帧方块 + 播放头）。
    //! 拖拽标尺移动播放头，拖拽关键帧改时间、上下拖拽改值，双击轨道空白处新增关键帧
    //! （值取当前插值），`Delete` 删除选中关键帧，`Ctrl/Cmd + 滚轮` 缩放、
    //! `Shift + 滚轮` 纵向滚动轨道、滚轮平移时间轴，点轨道名前的箭头折叠 / 展开轨道。
    //! 三轨的值实时驱动右侧预览方块（位置 / 缩放 / 透明度），播放头播放时按 `1/60s` 步进推进。
    //!
    //! 运行：`cargo run -p showcase --bin timeline`

    use std::time::Duration;

    use rgpui::anim_timeline::{
        Interpolation, Keyframe, Timeline, TimelineEvent, TimelineState, TimelineView, Track,
    };
    use rgpui::{
        AnyElement, App, Bounds, Button, Context, Entity, IntoElement, ParentElement as _, Render,
        SharedString, Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div, hsla,
        prelude::*, px, size, v_flex,
    };
    use rgpui_platform::application;

    const FRAME_STEP: f32 = 1.0 / 60.0;

    fn demo_timeline() -> Timeline {
        Timeline::from_tracks(vec![
            Track::with_keyframes(
                "位置 X",
                vec![Keyframe::new(0.0, 0.0), Keyframe::new(2.0, 240.0)],
            ),
            Track::with_keyframes(
                "缩放",
                vec![Keyframe::new(0.0, 0.6), Keyframe::new(2.0, 1.4)],
            )
            .with_interpolation(Interpolation::Smooth),
            Track::with_keyframes(
                "透明度",
                vec![
                    Keyframe::new(0.0, 0.2),
                    Keyframe::new(1.0, 1.0),
                    Keyframe::new(2.0, 0.6),
                ],
            ),
            // 后三条只用来把轨道区撑到需要纵向滚动，预览方块不读取。
            Track::with_keyframes(
                "旋转",
                vec![Keyframe::new(0.0, 0.0), Keyframe::new(2.0, 90.0)],
            ),
            Track::with_keyframes(
                "模糊",
                vec![Keyframe::new(0.0, 0.0), Keyframe::new(2.0, 8.0)],
            ),
            Track::new("颜色"),
        ])
    }

    struct TimelineApp {
        /// 时间线状态。
        state: Entity<TimelineState>,
        /// 是否播放中（与时间线状态同步）。
        playing: bool,
        /// 状态栏文案。
        status: SharedString,
    }

    impl TimelineApp {
        /// 创建示例应用并订阅时间线事件。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let state = cx.new(|cx| TimelineState::new(demo_timeline(), cx));

            cx.subscribe_in(
                &state,
                window,
                |this: &mut Self, state, event: &TimelineEvent, _window, cx| match event {
                    TimelineEvent::PlaybackChanged(playing) => {
                        this.playing = *playing;
                        this.status = if *playing {
                            "播放中…".into()
                        } else {
                            "已暂停".into()
                        };
                    }
                    TimelineEvent::KeyframeRequested { track, time } => {
                        state.update(cx, |state, cx| {
                            if let Some(model) = state.timeline_mut().track_mut(*track) {
                                let value = model.value_at(*time);
                                model.set_keyframe(*time, value);
                            }
                            cx.notify();
                        });
                        this.status = format!("在第 {:.2} 秒新增关键帧", time).into();
                    }
                    TimelineEvent::KeyframeMoved {
                        track, time, value, ..
                    } => {
                        this.status = format!(
                            "轨道 {} 的关键帧 → {:.2} 秒 / 值 {:.2}",
                            track + 1,
                            time,
                            value
                        )
                        .into();
                    }
                    TimelineEvent::KeyframeRemoved { track, .. } => {
                        this.status = format!("删除轨道 {} 的一个关键帧", track + 1).into();
                    }
                    TimelineEvent::TimeChanged(time) => {
                        this.status = format!("播放头 {:.2} 秒", time).into();
                    }
                    TimelineEvent::SelectionChanged { .. } => {}
                    TimelineEvent::TrackMoved { from, to } => {
                        this.status = format!("轨道 {} 移到第 {} 位", from + 1, to + 1).into();
                    }
                    TimelineEvent::TrackCollapsed { index, collapsed } => {
                        this.status = if *collapsed {
                            format!("轨道 {} 已折叠", index + 1)
                        } else {
                            format!("轨道 {} 已展开", index + 1)
                        }
                        .into();
                    }
                    TimelineEvent::TrackInterpolation {
                        index,
                        interpolation,
                    } => {
                        this.status =
                            format!("轨道 {} 的插值改为「{}」", index + 1, interpolation.label())
                                .into();
                    }
                },
            )
            .detach();

            Self {
            state,
            playing: false,
            status: "拖拽标尺移动播放头 · 拖拽关键帧改时间 / 改值 · Ctrl+点击多选关键帧 · 双击轨道空白新增".into(),
        }
        }

        /// 开始播放（后台逐帧推进播放头）。
        fn play(&mut self, cx: &mut Context<Self>) {
            if self.playing {
                return;
            }
            self.playing = true;
            self.state.update(cx, |state, cx| state.play(cx));

            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(16))
                        .await;
                    let continues = this.update(cx, |this, cx| {
                        if !this.playing {
                            return false;
                        }
                        this.state
                            .update(cx, |state, cx| state.advance(FRAME_STEP, cx));
                        true
                    });
                    if !matches!(continues, Ok(true)) {
                        break;
                    }
                }
            })
            .detach();
        }

        /// 暂停播放。
        fn pause(&mut self, cx: &mut Context<Self>) {
            self.playing = false;
            self.state.update(cx, |state, cx| state.pause(cx));
        }

        /// 回到开头。
        fn rewind(&mut self, cx: &mut Context<Self>) {
            self.state.update(cx, |state, cx| state.set_time(0.0, cx));
            self.status = "回到开头".into();
        }

        /// 在播放头处给第一条轨道新增关键帧。
        fn add_keyframe(&mut self, cx: &mut Context<Self>) {
            let (time, index) = self.state.update(cx, |state, cx| {
                let time = state.time();
                let Some(model) = state.timeline_mut().track_mut(0) else {
                    return (0.0, None);
                };
                let value = model.value_at(time);
                model.set_keyframe(time, value);
                let index = model.keyframe_index_at(time, 1e-3);
                cx.notify();
                (time, index)
            });
            if let Some(index) = index {
                self.state
                    .update(cx, |state, cx| state.select_keyframe(0, index, cx));
            }
            self.status = format!("在 {time:.2} 秒新增关键帧").into();
        }

        /// 删除选中的关键帧。
        fn remove_keyframe(&mut self, cx: &mut Context<Self>) {
            let selected = self.state.read(cx).selection().1.is_some();
            self.state
                .update(cx, |state, cx| state.remove_selected_keyframe(cx));
            self.status = if selected {
                "删除选中的关键帧".into()
            } else {
                "没有选中关键帧".into()
            };
        }

        /// 时间线总时长。
        fn duration(&self, cx: &Context<Self>) -> f32 {
            self.state.read(cx).duration()
        }

        /// 上移 / 下移选中的轨道（未选中时移动第一条）。
        fn move_track(&mut self, delta: isize, cx: &mut Context<Self>) {
            let (from, count) = {
                let state = self.state.read(cx);
                (state.selection().0.unwrap_or(0), state.timeline().len())
            };
            let target = from as isize + delta;
            if target < 0 || target >= count as isize {
                self.status = "轨道已在边界".into();
                return;
            }
            self.state
                .update(cx, |state, cx| state.move_track(from, target as usize, cx));
        }

        /// 折叠 / 展开选中的轨道（未选中时处理第一条）。
        fn toggle_collapse(&mut self, cx: &mut Context<Self>) {
            let index = self.state.read(cx).selection().0.unwrap_or(0);
            self.state
                .update(cx, |state, cx| state.toggle_track_collapsed(index, cx));
        }

        /// 播放头处三条轨道的值（位置 / 缩放 / 透明度）。
        fn values(&self, cx: &Context<Self>) -> (f32, f32, f32) {
            let state = self.state.read(cx);
            let values = state.timeline().values_at(state.time());
            (
                values.first().copied().unwrap_or(0.0),
                values.get(1).copied().unwrap_or(1.0),
                values.get(2).copied().unwrap_or(1.0),
            )
        }
    }

    fn interpolation_buttons(
        cx: &mut Context<TimelineApp>,
        state: Entity<TimelineState>,
    ) -> Vec<AnyElement> {
        let selected = state.read(cx).selection().0;
        let mut out = Vec::new();
        for (index, interpolation) in [
            Interpolation::Linear,
            Interpolation::Smooth,
            Interpolation::EaseIn,
            Interpolation::EaseOut,
            Interpolation::EaseInOut,
            Interpolation::Hold,
        ]
        .into_iter()
        .enumerate()
        {
            let state = state.clone();
            let current = selected.and_then(|track| {
                state
                    .read(cx)
                    .timeline()
                    .track(track)
                    .map(|track| track.interpolation)
            });
            let active = current == Some(interpolation);
            let button = Button::new(format!("interp-{index}"))
                .label(format!("插值：{}", interpolation.label()))
                .on_click(move |_event, _window, cx| {
                    state.update(cx, |state, cx| {
                        if let Some(track) = state.selection().0 {
                            state.set_track_interpolation(track, interpolation, cx);
                        }
                    });
                });
            out.push(
                div()
                    .child(button)
                    .when(active, |this| this.text_color(cx.theme().primary))
                    .into_any_element(),
            );
        }
        out
    }
    impl Render for TimelineApp {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let (x, scale, opacity) = self.values(cx);
            let duration = self.duration(cx);
            let time = self.state.read(cx).time();
            let snapping = self.state.read(cx).is_snapping();
            let selected = self.state.read(cx).selection().1.is_some();

            let play_button = if self.playing {
                Button::new("pause")
                    .label("暂停")
                    .on_click(cx.listener(|this, _event, _window, cx| this.pause(cx)))
            } else {
                Button::new("play")
                    .label("播放")
                    .on_click(cx.listener(|this, _event, _window, cx| this.play(cx)))
            };

            v_flex()
                .size_full()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .p_3()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(play_button)
                        .child(
                            Button::new("rewind")
                                .label("回到开头")
                                .on_click(cx.listener(|this, _event, _window, cx| this.rewind(cx))),
                        )
                        .child(Button::new("add-keyframe").label("加关键帧").on_click(
                            cx.listener(|this, _event, _window, cx| this.add_keyframe(cx)),
                        ))
                        .child(Button::new("remove-keyframe").label("删关键帧").on_click(
                            cx.listener(|this, _event, _window, cx| this.remove_keyframe(cx)),
                        ))
                        .child(Button::new("zoom-in").label("放大").on_click(cx.listener(
                            |this, _event, _window, cx| {
                                let state = this.state.clone();
                                state.update(cx, |state, cx| state.zoom_by(1.25, cx));
                            },
                        )))
                        .child(Button::new("zoom-out").label("缩小").on_click(cx.listener(
                            |this, _event, _window, cx| {
                                let state = this.state.clone();
                                state.update(cx, |state, cx| state.zoom_by(0.8, cx));
                            },
                        )))
                        .child(Button::new("track-up").label("轨道上移").on_click(
                            cx.listener(|this, _event, _window, cx| this.move_track(-1, cx)),
                        ))
                        .child(Button::new("track-down").label("轨道下移").on_click(
                            cx.listener(|this, _event, _window, cx| this.move_track(1, cx)),
                        ))
                        .child(Button::new("track-fold").label("折叠轨道").on_click(
                            cx.listener(|this, _event, _window, cx| this.toggle_collapse(cx)),
                        ))
                        .children(interpolation_buttons(cx, self.state.clone()))
                        .child(
                            Button::new("toggle-snap")
                                .label(if snapping {
                                    "吸附：开"
                                } else {
                                    "吸附：关"
                                })
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    let state = this.state.clone();
                                    let next = if state.read(cx).is_snapping() {
                                        0.0
                                    } else {
                                        30.0
                                    };
                                    state.update(cx, |state, cx| state.set_fps(next, cx));
                                })),
                        )
                        .child(div().text_sm().child(self.status.clone())),
                )
                .child(
                    div()
                        .h(px(140.0))
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(TimelineView::new(&self.state)),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .p_4()
                        .gap_3()
                        .child(
                            div()
                                .text_sm()
                                .child(format!("播放头 {time:.2}s / 总时长 {duration:.2}s")),
                        )
                        .child(div().text_xs().child(format!(
                            "位置 X {x:.1}px · 缩放 {scale:.2}× · 透明度 {opacity:.2}"
                        )))
                        .child(
                            div()
                                .relative()
                                .h(px(160.0))
                                .w_full()
                                .rounded_md()
                                .border_1()
                                .border_color(cx.theme().border)
                                .overflow_hidden()
                                // 预览方块：位置 / 缩放 / 透明度由时间线驱动。
                                .child(
                                    div()
                                        .absolute()
                                        .top(px(40.0))
                                        .left(px(16.0 + x))
                                        .w(px(120.0 * scale))
                                        .h(px(80.0 * scale))
                                        .rounded_md()
                                        .bg(hsla(0.58, 0.6, 0.5, 1.0))
                                        .opacity(opacity),
                                ),
                        )
                        .child(div().text_xs().child(if selected {
                            "已选中关键帧：Delete 删除，水平拖拽改时间、上下拖拽改值"
                        } else {
                            "未选中关键帧"
                        })),
                )
        }
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1000.), px(620.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("动画时间线示例".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| TimelineApp::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| TimelineApp::new(window, cx)).into()
    }
}

mod color_picker {
    //! 取色器增强示例
    //!
    //! 演示 [`ColorPickerState`] 的主题色板 / 最近颜色快捷色块，以及可独立复用的
    //! [`ColorPalette`]：
    //! - 取色器（三滑块 + hex + 主题色板 + 最近颜色）：点击色块即应用并记录；
    //! - hex 输入框回车把当前颜色记入最近颜色；
    //! - 独立色板：把命名颜色色阶（`ColorName::all()` 的 500 色阶）铺成一格一格，
    //!   点击直接应用到取色器；
    //! - 「记录当前颜色 / 清空最近」按钮演示程序化维护最近颜色。
    //!
    //! 启用 `screen-capture` feature（showcase 默认开启）时，动作行多出「吸管取色」：
    //! 调 [`ColorPickerState::begin_eyedropper`]，从屏幕首帧取色并写回取色器。
    //!
    //! 运行：`cargo run -p showcase --bin color_picker`

    use rgpui::{
        App, Bounds, Button, Context, Entity, Hsla, IntoElement, ParentElement, Render,
        SharedString, Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions,
        components::{ColorPalette, ColorPickerState, color_name_palette},
        div, hsla,
        prelude::*,
        px, size,
    };
    use rgpui_platform::application;

    struct ColorPickerApp {
        /// 指向自身的句柄。
        handle: Entity<Self>,
        /// 取色器（含主题色板 / 最近颜色）。
        picker: Entity<ColorPickerState>,
        /// 当前颜色。
        color: Hsla,
        /// 状态提示。
        status: SharedString,
    }

    impl ColorPickerApp {
        /// 创建示例应用。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let handle = cx.entity();
            let initial = hsla(210.0 / 360.0, 0.6, 0.5, 1.0);
            let picker = cx.new(|cx| {
                ColorPickerState::new(window, cx).color(initial).on_change({
                    let handle = handle.clone();
                    move |color, _window, cx| {
                        handle.update(cx, |this, cx| {
                            this.color = color;
                            cx.notify();
                        });
                    }
                })
            });
            Self {
                handle,
                picker,
                color: initial,
                status: "点击色板色块、拖动滑块或输入 hex（回车提交）".into(),
            }
        }

        /// 把取色器当前颜色记入最近颜色。
        fn remember(&mut self, cx: &mut Context<Self>) {
            let color = self.picker.read(cx).color_value();
            let changed = self
                .picker
                .update(cx, |picker, cx| picker.push_recent(color, cx));
            self.status = if changed {
                "已记入最近颜色".into()
            } else {
                "该颜色已排在最近颜色首位".into()
            };
            cx.notify();
        }

        /// 清空最近颜色。
        fn clear(&mut self, cx: &mut Context<Self>) {
            self.picker
                .update(cx, |picker, cx| picker.clear_recents(cx));
            self.status = "已清空最近颜色".into();
            cx.notify();
        }
    }

    impl Render for ColorPickerApp {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let handle = self.handle.clone();

            // 独立色板：命名颜色 500 色阶，点击直接应用到取色器（复用 `ColorPalette`）。
            let picker_for_scale = self.picker.clone();
            let scale_palette = ColorPalette::new("scale-500")
                .colors(color_name_palette(500))
                .selected(Some(self.color))
                .swatch_size(20.0)
                .gap(6.0)
                .on_select(move |color, window, cx| {
                    picker_for_scale.update(cx, |picker, cx| picker.set_color(color, window, cx));
                });

            let hex = hex_string(self.color);

            // 动作行：吸管按钮只在 `screen-capture` feature 下编译（底层走平台屏幕捕获）。
            let action_row = {
                let row = div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("remember").label("记录当前颜色").on_click({
                        let handle = handle.clone();
                        move |_event, _window, cx| handle.update(cx, |this, cx| this.remember(cx))
                    }))
                    .child(Button::new("clear").label("清空最近").on_click({
                        move |_event, _window, cx| handle.update(cx, |this, cx| this.clear(cx))
                    }));
                #[cfg(feature = "screen-capture")]
                let row = {
                    let picker = self.picker.clone();
                    row.child(Button::new("eyedropper").label("吸管取色").on_click(
                        move |_event, _window, cx| {
                            picker.update(cx, |picker, cx| picker.begin_eyedropper(cx));
                        },
                    ))
                };
                row
            };

            div()
                .flex()
                .flex_row()
                .gap_6()
                .p_6()
                .size_full()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(div().text_xl().child("取色器增强示例"))
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(self.status.clone()),
                        )
                        .child(self.picker.clone())
                        .child(action_row),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .flex_1()
                        .child(div().text_lg().child("预览"))
                        .child(
                            div()
                                .p_4()
                                .rounded_lg()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(self.color)
                                .child(
                                    div()
                                        .text_color(contrast_foreground(self.color))
                                        .child(format!("当前颜色 {hex}")),
                                ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("命名色阶色板（500）—— 点击直接应用"),
                        )
                        .child(scale_palette),
                )
        }
    }

    fn hex_string(color: Hsla) -> String {
        let rgba = color.to_rgb();
        let channel = |value: f32| (value * 255.0).round().clamp(0.0, 255.0) as u8;
        format!(
            "#{:02x}{:02x}{:02x}",
            channel(rgba.r),
            channel(rgba.g),
            channel(rgba.b)
        )
    }

    fn contrast_foreground(color: Hsla) -> Hsla {
        if color.l > 0.55 {
            hsla(0.0, 0.0, 0.05, 1.0)
        } else {
            hsla(0.0, 0.0, 1.0, 1.0)
        }
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(880.), px(620.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("取色器示例".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| ColorPickerApp::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| ColorPickerApp::new(window, cx)).into()
    }
}

mod mdi {
    //! 多文档界面（MDI）示例
    //!
    //! 演示 `mdi`：一块 MDI 区域内并列多个子文档窗口。每个窗口带标题栏（最小化 / 最大化 /
    //! 关闭）与八向缩放边框；拖拽标题栏移动、拖拽边框缩放、双击标题栏切换最大化、
    //! 最小化的窗口缩为底部图标条。工具栏提供新建 / 层叠 / 平铺 / 页签 / 全部最小化 / 关闭全部，
    //! 激活窗口内按 `Ctrl+Tab`（或 `Ctrl+F6`）循环切换、`Ctrl+F4` 关闭、`Ctrl+M` 最小化。
    //! 窗口正文里放了一个按钮，用来验证「正文可交互 + 点击自动激活窗口」。
    //!
    //! 运行：`cargo run -p showcase --bin mdi`

    use std::collections::HashMap;

    use rgpui::mdi::{MdiArea, MdiEvent, MdiLayout, MdiState, MdiWindowFlags};
    use rgpui::{
        App, Bounds, Button, Context, Entity, IntoElement, ParentElement as _, Render,
        SharedString, Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div, hsla,
        prelude::*, px, size, v_flex,
    };
    use rgpui_platform::application;

    fn open_document(app: &mut MdiApp, title: String, cx: &mut Context<MdiApp>) {
        app.mdi.update(cx, |state, cx| state.open(title, cx));
    }

    struct MdiApp {
        /// MDI 状态。
        mdi: Entity<MdiState>,
        /// 各窗口正文里的点击次数（演示正文可交互）。
        clicks: HashMap<u64, usize>,
        /// 下一个文档序号。
        next_index: usize,
        /// 状态栏文案。
        status: SharedString,
    }

    impl MdiApp {
        /// 创建示例应用并订阅 MDI 事件。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            // 拖动松手时吸附到工作区/邻窗边缘（6px 内）
            let mdi = cx.new(|cx| MdiState::new(cx).with_snap(px(6.0)));

            // MDI 元素只负责画与转发事件，几何变化要靠宿主观察状态来触发重绘。
            cx.observe(&mdi, |_this, _mdi, cx| cx.notify()).detach();

            cx.subscribe_in(
                &mdi,
                window,
                |this: &mut Self, _mdi, event: &MdiEvent, _window, cx| {
                    this.status = match event {
                        MdiEvent::Opened(id) => format!("打开窗口 {id}").into(),
                        MdiEvent::Activated(id) => format!("激活窗口 {id}").into(),
                        MdiEvent::Closed(id) => format!("关闭窗口 {id}").into(),
                        MdiEvent::StateChanged(id) => format!("窗口 {id} 状态变化").into(),
                    };
                    cx.notify();
                },
            )
            .detach();

            let mut app = Self {
                mdi,
                clicks: HashMap::new(),
                next_index: 0,
                status: "拖拽标题栏移动（松手吸附边缘）· 拖拽边框缩放 · 双击标题栏最大化".into(),
            };
            // 初始铺开三个文档，方便一眼看到层叠效果。
            for name in ["季度报告", "预算表", "会议纪要"] {
                app.new_document(name.into(), cx);
            }
            app.mdi
                .update(cx, |state, cx| state.arrange(MdiLayout::Cascade, cx));
            app
        }

        /// 新建一个文档窗口。
        fn new_document(&mut self, name: String, cx: &mut Context<Self>) {
            self.next_index += 1;
            let title = format!("{name} ({})", self.next_index);
            open_document(self, title, cx);
        }

        /// 新建一个不可关闭 / 不可最小化的只读窗口。
        fn new_readonly(&mut self, cx: &mut Context<Self>) {
            self.next_index += 1;
            let title = format!("只读视图 ({})", self.next_index);
            self.mdi.update(cx, |state, cx| {
                state.open_with(
                    title,
                    MdiWindowFlags {
                        closable: false,
                        minimizable: false,
                        maximizable: true,
                    },
                    cx,
                );
            });
        }

        /// 按给定方式排布窗口。
        fn arrange(&mut self, layout: MdiLayout, cx: &mut Context<Self>) {
            self.mdi.update(cx, |state, cx| state.arrange(layout, cx));
            self.status = match layout {
                MdiLayout::Cascade => "层叠排布".into(),
                MdiLayout::TileHorizontal => "水平平铺".into(),
                MdiLayout::TileVertical => "垂直平铺".into(),
                MdiLayout::Tabbed => "页签式（点击标签切换）".into(),
            };
        }

        /// 最小化全部窗口。
        fn minimize_all(&mut self, cx: &mut Context<Self>) {
            self.mdi.update(cx, |state, cx| {
                for window in state
                    .windows()
                    .iter()
                    .map(|window| window.id)
                    .collect::<Vec<_>>()
                {
                    state.minimize(window, cx);
                }
            });
            self.status = "全部最小化（点击底部图标还原）".into();
        }

        /// 关闭全部窗口。
        fn close_all(&mut self, cx: &mut Context<Self>) {
            self.mdi.update(cx, |state, cx| state.close_all(cx));
            self.clicks.clear();
            self.status = "关闭全部窗口".into();
        }
    }

    impl Render for MdiApp {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let (count, active) = {
                let state = self.mdi.read(cx);
                (state.len(), state.active())
            };
            let status = self.status.clone();

            // 正文回调：每个窗口按 id 现渲染（这里放一个可点击按钮，验证正文可交互）。
            let this = cx.entity();
            let body = move |id: u64, _window: &mut Window, cx: &mut App| {
                let this = this.clone();
                let count = this.read(cx).clicks.get(&id).copied().unwrap_or(0);
                let (hue, tint) = (0.58 + 0.13 * (id % 4) as f32, 0.94);
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .size_full()
                    .p_3()
                    .bg(hsla(hue, 0.25, tint, 1.0))
                    .child(
                        div()
                            .text_sm()
                            .child(format!("文档 {id} 的正文（点击计数 {count}）")),
                    )
                    .child(Button::new(("bump", id)).label("点我 +1").on_click(
                        move |_event, _window, cx| {
                            this.update(cx, |this, cx| {
                                *this.clicks.entry(id).or_default() += 1;
                                this.status = format!("文档 {id} 正文被点击").into();
                                cx.notify();
                            });
                        },
                    ))
                    .child(
                        div()
                            .text_xs()
                            .child("正文内的控件可以正常交互，点击正文会顺带激活所在窗口。"),
                    )
                    .into_any_element()
            };

            v_flex()
                .size_full()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .p_3()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(Button::new("new").label("新建文档").on_click(cx.listener(
                            |this, _event, _window, cx| {
                                this.new_document("新文档".into(), cx);
                            },
                        )))
                        .child(
                            Button::new("new-readonly")
                                .label("只读视图")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.new_readonly(cx);
                                })),
                        )
                        .child(Button::new("cascade").label("层叠").on_click(cx.listener(
                            |this, _event, _window, cx| this.arrange(MdiLayout::Cascade, cx),
                        )))
                        .child(
                            Button::new("tile-h")
                                .label("水平平铺")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.arrange(MdiLayout::TileHorizontal, cx)
                                })),
                        )
                        .child(
                            Button::new("tile-v")
                                .label("垂直平铺")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.arrange(MdiLayout::TileVertical, cx)
                                })),
                        )
                        .child(Button::new("tabbed").label("页签").on_click(cx.listener(
                            |this, _event, _window, cx| this.arrange(MdiLayout::Tabbed, cx),
                        )))
                        .child(Button::new("minimize-all").label("全部最小化").on_click(
                            cx.listener(|this, _event, _window, cx| this.minimize_all(cx)),
                        ))
                        .child(
                            Button::new("close-all").label("关闭全部").on_click(
                                cx.listener(|this, _event, _window, cx| this.close_all(cx)),
                            ),
                        )
                        .child(div().text_sm().child(format!(
                                "窗口 {count} 个 · 激活 {} · {status}",
                                active
                                    .map(|id| format!("#{id}"))
                                    .unwrap_or_else(|| "无".to_string())
                            ))),
                )
                .child(
                    div()
                        .flex_1()
                        .p_2()
                        .child(MdiArea::new(&self.mdi).body(body)),
                )
        }
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1080.), px(720.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("MDI Example".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| MdiApp::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| MdiApp::new(window, cx)).into()
    }
}

mod task_pane {
    //! 任务窗格示例
    //!
    //! 演示 `task_pane`：右侧停靠的任务窗格（标题栏 + 正文 + 内缘拖拽把手 + 关闭按钮）。
    //! 拖动内缘把手改宽，`✕` 关闭，工具栏可切换停靠侧 / 浮动 / 显隐；标题栏是「属性 / 大纲」
    //! 页签组（点击页签或 `Ctrl+Tab` 切换，正文随之换页）；打开「自动隐藏」后鼠标移出收起成
    //! 竖排标题轨道、移入展开。浮动态变成一块可自由拖动标题栏、八向缩放的窗格（复用 MDI 的
    //! 缩放几何）。窗格正文实时读取状态，展示宽度 / 停靠侧 / 浮动状态。
    //!
    //! 运行：`cargo run -p showcase --bin task_pane`

    use rgpui::task_pane::{TaskPane, TaskPaneEvent, TaskPaneSide, TaskPaneState};
    use rgpui::{
        AnyElement, App, Bounds, Button, Context, Entity, IntoElement, ParentElement as _, Render,
        SharedString, Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div,
        prelude::*, px, size, v_flex,
    };
    use rgpui_platform::application;

    struct TaskPaneApp {
        /// 窗格状态。
        pane: Entity<TaskPaneState>,
        /// 状态栏文案。
        status: SharedString,
    }

    impl TaskPaneApp {
        /// 创建示例应用并观察窗格状态。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let pane = cx.new(|cx| {
                TaskPaneState::new(cx)
                    .with_side(TaskPaneSide::Right)
                    .with_width(px(300.0))
                    .with_tabs(["属性", "大纲"])
            });

            // 窗格元素只负责画与转发事件，几何变化要靠宿主观察状态来触发重绘。
            cx.observe(&pane, |_this, _pane, cx| cx.notify()).detach();
            cx.subscribe_in(
                &pane,
                window,
                |this: &mut Self, _pane, event: &TaskPaneEvent, _window, cx| {
                    this.status = match event {
                        TaskPaneEvent::VisibilityChanged(true) => "显示窗格".into(),
                        TaskPaneEvent::VisibilityChanged(false) => "隐藏窗格".into(),
                        TaskPaneEvent::SideChanged(TaskPaneSide::Left) => "停靠到左侧".into(),
                        TaskPaneEvent::SideChanged(TaskPaneSide::Right) => "停靠到右侧".into(),
                        TaskPaneEvent::WidthChanged(width) => format!("窗格宽 {width:.0}").into(),
                        TaskPaneEvent::FloatingChanged(true) => "切到浮动态".into(),
                        TaskPaneEvent::FloatingChanged(false) => "回到停靠态".into(),
                        TaskPaneEvent::Collapsed(true) => "窗格收起（悬停展开）".into(),
                        TaskPaneEvent::Collapsed(false) => "窗格展开".into(),
                        TaskPaneEvent::TabSelected(index) => format!(
                            "切到「{}」页签",
                            ["属性", "大纲"].get(*index).copied().unwrap_or("?")
                        )
                        .into(),
                    };
                    cx.notify();
                },
            )
            .detach();

            Self {
            pane,
            status:
                "拖动内缘把手改宽 · 把把手拖过窗口中线可换边停靠 · ✕ 关闭 · 点标题栏页签切换正文"
                    .into(),
        }
        }

        /// 切换浮动 / 停靠（浮动时贴到窗口右下角内侧）。
        fn toggle_float(&mut self, window: &mut Window, cx: &mut Context<Self>) {
            let parent = Bounds::new(rgpui::point(px(0.0), px(0.0)), window.viewport_size());
            self.pane
                .update(cx, |pane, cx| pane.toggle_float(parent, cx));
        }

        /// 相对当前宽度加宽 / 收窄。
        fn resize_by(&mut self, delta: f32, cx: &mut Context<Self>) {
            self.pane.update(cx, |pane, cx| {
                let width = pane.width() + px(delta);
                pane.set_width(width, cx);
            });
        }
    }

    impl Render for TaskPaneApp {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            // 父容器矩形 = 窗口视口：把手拖过中线即可换边停靠。
            let parent = Bounds::new(rgpui::point(px(0.0), px(0.0)), window.viewport_size());
            self.pane
                .update(cx, |pane, cx| pane.set_parent_bounds(parent, cx));
            let (visible, side, width, floating, closable, auto_hide, collapsed) =
                self.pane.read_with(cx, |pane, _| {
                    (
                        pane.is_visible(),
                        pane.side(),
                        pane.width(),
                        pane.is_floating(),
                        pane.is_closable(),
                        pane.is_auto_hide(),
                        pane.is_collapsed(),
                    )
                });
            let status = self.status.clone();

            // 正文：按当前页签渲染属性行或大纲。
            let pane_for_body = self.pane.clone();
            let body = move |_window: &mut Window, cx: &mut App| -> AnyElement {
                let (width, side, floating, active) = pane_for_body.read_with(cx, |pane, _| {
                    (
                        pane.width(),
                        pane.side(),
                        pane.is_floating(),
                        pane.active_tab(),
                    )
                });
                let theme = cx.theme();
                let muted = theme.muted_foreground;
                let foreground = theme.foreground;
                let border = theme.border;
                let row = move |label: &'static str, value: String| {
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .py_1()
                        .border_b_1()
                        .border_color(border)
                        .text_sm()
                        .child(div().text_color(muted).child(label))
                        .child(div().text_color(foreground).child(value))
                };
                let container = v_flex().gap_1().p_3();
                if active == 1 {
                    return container
                        .child(div().text_xs().text_color(muted).child("文档大纲"))
                        .children(
                            ["一、概述", "二、页面设置", "三、停靠与浮动", "四、自动隐藏"].map(
                                |label| {
                                    div()
                                        .py_1()
                                        .border_b_1()
                                        .border_color(border)
                                        .text_sm()
                                        .text_color(foreground)
                                        .child(label)
                                },
                            ),
                        )
                        .child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(muted)
                                .child("点击标题栏页签或按 Ctrl+Tab 切换。"),
                        )
                        .into_any_element();
                }
                container
                    .child(div().text_xs().text_color(muted).child("页面设置"))
                    .child(row("宽度", format!("{width:.0}")))
                    .child(row(
                        "停靠侧",
                        if floating {
                            "浮动".to_string()
                        } else if side == TaskPaneSide::Left {
                            "左侧".to_string()
                        } else {
                            "右侧".to_string()
                        },
                    ))
                    .child(row("纸张", "A4 · 纵向".into()))
                    .child(row("页边距", "上下 2.54 · 左右 3.17".into()))
                    .child(
                        div()
                            .pt_2()
                            .text_xs()
                            .text_color(muted)
                            .child("属性行随窗格状态实时刷新；拖动内缘把手即可改宽。"),
                    )
                    .into_any_element()
            };

            // 主内容区。
            let main = div().flex_1().h_full().p_4().child(
            v_flex()
                .gap_2()
                .size_full()
                .child(div().text_base().child("文档正文"))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("这里是被停靠窗格挤出的可用区域。"),
                )
                .child(
                    div()
                        .flex_1()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .p_3()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("拖动窗格内缘的细条改宽 · 点标题栏页签切换属性 / 大纲 · 工具栏可切到浮动态"),
                ),
        );

            let pane = TaskPane::new(&self.pane)
                .title("属性")
                .body(body)
                .into_any_element();

            // 容器是浮动态的定位祖先。
            let row = div()
                .relative()
                .flex()
                .flex_row()
                .flex_1()
                .overflow_hidden()
                .border_t_1()
                .border_color(cx.theme().border);
            let row = if floating || side == TaskPaneSide::Right {
                row.child(main).child(pane)
            } else {
                row.child(pane).child(main)
            };

            v_flex()
                .size_full()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .p_3()
                        .child(
                            Button::new("toggle")
                                .label(if visible {
                                    "隐藏窗格"
                                } else {
                                    "显示窗格"
                                })
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.pane.update(cx, |pane, cx| pane.toggle(cx));
                                })),
                        )
                        .child(
                            Button::new("dock-left")
                                .label("停靠左")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.pane.update(cx, |pane, cx| {
                                        pane.set_side(TaskPaneSide::Left, cx)
                                    });
                                })),
                        )
                        .child(
                            Button::new("dock-right")
                                .label("停靠右")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.pane.update(cx, |pane, cx| {
                                        pane.set_side(TaskPaneSide::Right, cx)
                                    });
                                })),
                        )
                        .child(
                            Button::new("float")
                                .label(if floating { "回到停靠" } else { "浮动" })
                                .on_click(cx.listener(|this, _event, window, cx| {
                                    this.toggle_float(window, cx);
                                })),
                        )
                        .child(Button::new("wider").label("加宽").on_click(
                            cx.listener(|this, _event, _window, cx| this.resize_by(40.0, cx)),
                        ))
                        .child(Button::new("narrower").label("收窄").on_click(
                            cx.listener(|this, _event, _window, cx| this.resize_by(-40.0, cx)),
                        ))
                        .child(
                            Button::new("closable")
                                .label(if closable {
                                    "关闭按钮：开"
                                } else {
                                    "关闭按钮：关"
                                })
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.pane.update(cx, |pane, cx| {
                                        let next = !pane.is_closable();
                                        pane.set_closable(next, cx);
                                    });
                                })),
                        )
                        .child(
                            Button::new("auto-hide")
                                .label(if auto_hide {
                                    "自动隐藏：开"
                                } else {
                                    "自动隐藏：关"
                                })
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.pane.update(cx, |pane, cx| {
                                        let next = !pane.is_auto_hide();
                                        pane.set_auto_hide(next, cx);
                                    });
                                })),
                        )
                        .child(
                            Button::new("next-tab")
                                .label("下一个页签")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.pane.update(cx, |pane, cx| pane.cycle_tab(1, cx));
                                })),
                        )
                        .child(div().text_sm().child(format!(
                            "宽 {width:.0} · {}{}{} · {status}",
                            if floating { "浮动" } else { "停靠" },
                            if visible { "" } else { " · 已隐藏" },
                            if collapsed { " · 已收起" } else { "" }
                        ))),
                )
                .child(row)
        }
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1120.), px(720.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Task Pane Example".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| TaskPaneApp::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| TaskPaneApp::new(window, cx)).into()
    }
}

mod canvas_a11y {
    //! 画布语义（无障碍）示例
    //!
    //! 演示 `canvas_a11y`：自绘画布上的每个对象都会作为**合成子节点**写入无障碍树
    //! （角色 / 名称 / 值 / 矩形 / 选中与焦点状态），于是屏幕阅读器可以枚举、定位、
    //! 点击这些对象 —— 尽管画布上一个字都没画、树里也只有一个画布元素。
    //!
    //! 三种输入路径等价：
    //! - **指针**：点击对象 = 聚焦 + 选中（命中测试与视觉叠放一致，后画者在上）；
    //! - **键盘**：← → ↑ ↓ 沿阅读顺序移动焦点，`Home` / `End` 跳首 / 末，`Enter` / `Space`
    //!   选中焦点对象，`Esc` 清除选中；
    //! - **无障碍动作**：对象声明了 `Click` / `Focus`，辅助技术触发的动作经
    //!   `on_canvas_a11y_action` 回到应用。
    //!
    //! 运行：`cargo run -p showcase --bin canvas_a11y`

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::canvas_a11y::{CanvasNode, CanvasSemantics};
    use rgpui::{
        AccessibleAction, App, BorderStyle, Bounds, Button, Context, Corners, Edges, FocusHandle,
        Hsla, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Pixels, Point,
        Render, Role, SharedString, Styled as _, TitlebarOptions, Window, WindowBounds,
        WindowOptions, canvas, div, point, prelude::*, px, quad, size, transparent_black,
    };
    use rgpui_platform::application;

    const CARD_RADIUS: Pixels = px(12.0);
    const HALO: Pixels = px(5.0);

    #[derive(Clone)]
    struct DemoObject {
        /// 稳定标识（语义节点 id）。
        id: u64,
        /// 无障碍名称。
        label: SharedString,
        /// 无障碍值。
        value: SharedString,
        /// 画布局部坐标下的矩形。
        bounds: Bounds<Pixels>,
        /// 填充色。
        tone: Hsla,
        /// 是否画成圆形（正方形 + 半边长圆角）。
        circle: bool,
    }

    impl DemoObject {
        /// 圆角（圆形对象取半边长）。
        fn corners(&self) -> Corners<Pixels> {
            if self.circle {
                Corners::all(self.bounds.size.width / 2.0)
            } else {
                Corners::all(CARD_RADIUS)
            }
        }
    }

    struct CanvasA11yDemo {
        /// 画布对象（按绘制顺序，后画者在上）。
        objects: Vec<DemoObject>,
        /// 无障碍 / 键盘焦点对象。
        focused: Option<u64>,
        /// 选中对象。
        selected: Option<u64>,
        /// 画布原点（窗口坐标，画布预绘制时静默记录）。
        canvas_origin: Point<Pixels>,
        /// 焦点句柄（键盘事件挂在这里）。
        focus_handle: FocusHandle,
        /// 状态提示。
        status: SharedString,
    }

    impl CanvasA11yDemo {
        /// 创建演示视图。
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            let accent = cx.theme().accent;
            let info = cx.theme().info;
            let success = cx.theme().success;
            let warning = cx.theme().warning;
            let secondary = cx.theme().secondary;
            let primary = cx.theme().primary;

            let objects = vec![
                DemoObject {
                    id: 1,
                    label: "标题文本框".into(),
                    value: "季度总结".into(),
                    bounds: Bounds::new(point(px(24.0), px(20.0)), size(px(240.0), px(60.0))),
                    tone: accent,
                    circle: false,
                },
                DemoObject {
                    id: 2,
                    label: "图表卡片".into(),
                    value: "12 个数据点".into(),
                    bounds: Bounds::new(point(px(288.0), px(20.0)), size(px(280.0), px(160.0))),
                    tone: info,
                    circle: false,
                },
                DemoObject {
                    id: 3,
                    label: "圆形徽标".into(),
                    value: "徽标".into(),
                    bounds: Bounds::new(point(px(24.0), px(104.0)), size(px(120.0), px(120.0))),
                    tone: success,
                    circle: true,
                },
                DemoObject {
                    id: 4,
                    label: "表格卡片".into(),
                    value: "6 行 × 4 列".into(),
                    bounds: Bounds::new(point(px(168.0), px(104.0)), size(px(100.0), px(120.0))),
                    tone: warning,
                    circle: false,
                },
                // 故意压在图表卡片上：命中测试应取绘制顺序靠后的这一个。
                DemoObject {
                    id: 5,
                    label: "重叠卡片".into(),
                    value: "覆盖在图卡右下角".into(),
                    bounds: Bounds::new(point(px(470.0), px(120.0)), size(px(200.0), px(150.0))),
                    tone: secondary,
                    circle: false,
                },
                DemoObject {
                    id: 6,
                    label: "备注卡片".into(),
                    value: "3 条批注".into(),
                    bounds: Bounds::new(point(px(60.0), px(272.0)), size(px(300.0), px(90.0))),
                    tone: primary,
                    circle: false,
                },
            ];

            Self {
                objects,
                focused: Some(1),
                selected: None,
                canvas_origin: point(px(0.0), px(0.0)),
                focus_handle: cx.focus_handle(),
                status: "点对象或用 ← → ↑ ↓ 移动焦点".into(),
            }
        }

        /// 本帧的画布语义（含焦点 / 选中状态）。
        fn semantics(&self) -> CanvasSemantics {
            let mut semantics = CanvasSemantics::new();
            for object in &self.objects {
                let mut node = CanvasNode::new(
                    object.id,
                    Role::GraphicsObject,
                    object.label.clone(),
                    object.bounds,
                )
                .with_value(object.value.clone())
                .with_actions([AccessibleAction::Click, AccessibleAction::Focus]);
                node.set_selected(self.selected == Some(object.id));
                node.set_focused(self.focused == Some(object.id));
                semantics.push(node);
            }
            semantics
        }

        /// 对象名称（按 id）。
        fn label_of(&self, id: u64) -> String {
            self.objects
                .iter()
                .find(|object| object.id == id)
                .map(|object| object.label.to_string())
                .unwrap_or_else(|| format!("#{id}"))
        }

        /// 阅读顺序的可读文本（先按行，行内自左向右）。
        fn reading_order_text(&self) -> String {
            self.semantics()
                .reading_order()
                .iter()
                .map(|id| self.label_of(*id))
                .collect::<Vec<_>>()
                .join(" → ")
        }

        /// 聚焦某个对象。
        fn focus_object(&mut self, id: u64, cx: &mut Context<Self>) {
            if self.focused == Some(id) {
                return;
            }
            self.focused = Some(id);
            self.status = format!("焦点：{}（{}）", self.label_of(id), self.value_of(id)).into();
            cx.notify();
        }

        /// 对象的值（按 id）。
        fn value_of(&self, id: u64) -> String {
            self.objects
                .iter()
                .find(|object| object.id == id)
                .map(|object| object.value.to_string())
                .unwrap_or_default()
        }

        /// 沿阅读顺序移动焦点。
        fn move_focus(&mut self, forward: bool, cx: &mut Context<Self>) {
            if let Some(next) = self
                .semantics()
                .next_in_reading_order(self.focused, forward)
            {
                self.focus_object(next, cx);
            }
        }

        /// 焦点移到阅读顺序首 / 末个对象。
        fn focus_boundary(&mut self, last: bool, cx: &mut Context<Self>) {
            let semantics = self.semantics();
            let target = if last {
                semantics.next_in_reading_order(None, false)
            } else {
                semantics.first_in_reading_order()
            };
            if let Some(id) = target {
                self.focus_object(id, cx);
            }
        }

        /// 选中焦点对象。
        fn select_focused(&mut self, cx: &mut Context<Self>) {
            let Some(id) = self.focused else {
                return;
            };
            self.selected = Some(id);
            self.status = format!("选中：{}（{}）", self.label_of(id), self.value_of(id)).into();
            cx.notify();
        }

        /// 清除选中。
        fn clear_selection(&mut self, cx: &mut Context<Self>) {
            self.selected = None;
            self.status = "已清除选中".into();
            cx.notify();
        }

        /// 指针按下：按窗口坐标命中对象（画布局部坐标 = 窗口坐标 - 画布原点）。
        fn on_pointer_down(&mut self, window_point: Point<Pixels>, cx: &mut Context<Self>) {
            let local = window_point - self.canvas_origin;
            match self.semantics().hit_test(local) {
                Some(id) => {
                    self.focused = Some(id);
                    self.selected = Some(id);
                    self.status =
                        format!("点击：{}（{}）", self.label_of(id), self.value_of(id)).into();
                }
                None => {
                    self.selected = None;
                    self.status = "点击空白：清除选中".into();
                }
            }
            cx.notify();
        }

        /// 键盘：方向键沿阅读顺序移动焦点。
        fn on_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
            match event.keystroke.key.as_str() {
                "right" | "down" => self.move_focus(true, cx),
                "left" | "up" => self.move_focus(false, cx),
                "home" => self.focus_boundary(false, cx),
                "end" => self.focus_boundary(true, cx),
                "enter" | "space" => self.select_focused(cx),
                "escape" => self.clear_selection(cx),
                _ => {}
            }
        }

        /// 无障碍动作：与指针 / 键盘等价。
        fn on_canvas_action(&mut self, id: u64, action: AccessibleAction, cx: &mut Context<Self>) {
            match action {
                AccessibleAction::Click => {
                    self.focused = Some(id);
                    self.selected = Some(id);
                    self.status =
                        format!("无障碍点击：{}（{}）", self.label_of(id), self.value_of(id))
                            .into();
                }
                AccessibleAction::Focus => {
                    self.focused = Some(id);
                    self.status = format!("无障碍聚焦：{}", self.label_of(id)).into();
                }
                _ => return,
            }
            cx.notify();
        }
    }

    fn translate(bounds: Bounds<Pixels>, origin: Point<Pixels>) -> Bounds<Pixels> {
        Bounds::new(
            point(bounds.origin.x + origin.x, bounds.origin.y + origin.y),
            bounds.size,
        )
    }

    fn inflate(bounds: Bounds<Pixels>, by: Pixels) -> Bounds<Pixels> {
        Bounds::new(
            point(bounds.origin.x - by, bounds.origin.y - by),
            size(bounds.size.width + by * 2.0, bounds.size.height + by * 2.0),
        )
    }

    fn paint_objects(
        objects: &[DemoObject],
        origin: Point<Pixels>,
        focused: Option<u64>,
        selected: Option<u64>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let ring = cx.theme().ring;
        let selection = cx.theme().selection;
        let border = cx.theme().border;

        for object in objects {
            let is_focused = focused == Some(object.id);
            let is_selected = selected == Some(object.id);
            if !is_focused && !is_selected {
                continue;
            }
            let halo = inflate(translate(object.bounds, origin), HALO);
            window.paint_quad(quad(
                halo,
                object.corners(),
                if is_focused { ring } else { selection },
                Edges::all(px(0.0)),
                transparent_black(),
                BorderStyle::Solid,
            ));
        }

        for object in objects {
            window.paint_quad(quad(
                translate(object.bounds, origin),
                object.corners(),
                object.tone,
                Edges::all(px(1.0)),
                border,
                BorderStyle::Solid,
            ));
        }
    }

    impl Render for CanvasA11yDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let theme = cx.theme().clone();
            let semantics = self.semantics();
            let objects = self.objects.clone();
            let (focused, selected) = (self.focused, self.selected);

            // 画布：铺满容器；预绘制时记下原点（静默，不触发重绘）。
            let origin_recorder = cx.entity().downgrade();
            let surface = canvas(
                move |bounds, _window, cx| {
                    origin_recorder
                        .update(cx, |this, _| this.canvas_origin = bounds.origin)
                        .ok();
                },
                move |bounds, (), window, cx| {
                    paint_objects(&objects, bounds.origin, focused, selected, window, cx)
                },
            )
            .size_full()
            .id("canvas-a11y-surface")
            .a11y(Role::Canvas, semantics)
            .a11y_label("画布对象")
            .on_canvas_a11y_action({
                let weak = cx.entity().downgrade();
                let focus_handle = self.focus_handle.clone();
                move |id, action, _data, window, cx| {
                    // 辅助技术操作对象时，也把键盘焦点交给画布容器。
                    window.focus(&focus_handle, cx);
                    weak.update(cx, |this, cx| this.on_canvas_action(id, action, cx))
                        .ok();
                }
            });

            div()
                .flex()
                .flex_col()
                .size_full()
                .bg(theme.background)
                .text_color(theme.foreground)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(theme.border)
                        .child(div().text_sm().child("画布语义（无障碍）"))
                        .child(
                            Button::new("canvas-a11y-prev")
                                .label("上一个")
                                .on_click(cx.listener(|this, _, _, cx| this.move_focus(false, cx))),
                        )
                        .child(
                            Button::new("canvas-a11y-next")
                                .label("下一个")
                                .on_click(cx.listener(|this, _, _, cx| this.move_focus(true, cx))),
                        )
                        .child(
                            Button::new("canvas-a11y-select")
                                .label("选中焦点")
                                .on_click(cx.listener(|this, _, _, cx| this.select_focused(cx))),
                        )
                        .child(
                            Button::new("canvas-a11y-clear")
                                .label("清除选中")
                                .on_click(cx.listener(|this, _, _, cx| this.clear_selection(cx))),
                        )
                        .child(
                            div()
                                .ml_auto()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!(
                                    "对象 {} · 焦点 {} · 选中 {}",
                                    self.objects.len(),
                                    self.focused
                                        .map(|id| self.label_of(id))
                                        .unwrap_or_else(|| "无".into()),
                                    self.selected
                                        .map(|id| self.label_of(id))
                                        .unwrap_or_else(|| "无".into()),
                                )),
                        ),
                )
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("阅读顺序：{}", self.reading_order_text())),
                )
                .child(
                    div()
                        .id("canvas-a11y-host")
                        .track_focus(&self.focus_handle)
                        .key_context("CanvasA11y")
                        .flex_grow_1()
                        .min_h(px(0.0))
                        .relative()
                        .overflow_hidden()
                        .m_3()
                        .rounded(px(8.0))
                        .border_1()
                        .border_color(theme.border)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                                this.on_pointer_down(event.position, cx);
                            }),
                        )
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                            this.on_key_down(event, cx);
                        }))
                        .child(surface),
                )
                .child(
                    div()
                        .px_3()
                        .pb_3()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(self.status.clone()),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(900.0), px(640.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("画布无障碍示例".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| CanvasA11yDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| CanvasA11yDemo::new(window, cx)).into()
    }
}

mod page_view {
    //! 文档页视图演示：`PageView` 分页画布 + `Viewport` 缩放/平移。
    //!
    //! 三页 A4 文档排布（单页 / 对开 / 封面对开可切），`PageView` 绘制纸页与页眉页脚，
    //! 正文按 `PageLayout::content_rect` 导出的矩形叠加（字号随缩放同步）。滚轮 `Ctrl/⌘`
    //! 以光标为中心缩放、否则平移；左键拖拽平移；工具栏可缩小/放大/100%/适应窗口/切换排布。

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::page_view::{PageArrangement, PageLabels, PageLayout, PageSetup, PageView};
    use rgpui::viewport::Viewport;
    use rgpui::{
        App, Bounds, Button, Context, FocusHandle, IntoElement, MouseButton, MouseDownEvent,
        MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, ScrollWheelEvent,
        SharedString, Size, Styled, Window, WindowBounds, WindowOptions, div, h_flex, prelude::*,
        px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    const PAGE_COUNT: usize = 3;

    const ARRANGEMENTS: [PageArrangement; 3] = [
        PageArrangement::Single,
        PageArrangement::TwoUp,
        PageArrangement::TwoUpCover,
    ];

    fn arrangement_label(arrangement: PageArrangement) -> &'static str {
        match arrangement {
            PageArrangement::Single => "单页",
            PageArrangement::TwoUp => "对开",
            PageArrangement::TwoUpCover => "封面对开",
        }
    }

    struct DocLabels {
        /// 页眉标题。
        title: SharedString,
    }

    impl PageLabels for DocLabels {
        fn header(&self, _page: usize, _cx: &App) -> SharedString {
            self.title.clone()
        }

        fn footer(&self, page: usize, _cx: &App) -> SharedString {
            format!("第 {} / {PAGE_COUNT} 页", page + 1).into()
        }
    }

    fn page_text(page: usize) -> &'static str {
        match page {
            0 => {
                "文档页视图（Document Page View）\n\n\
              本演示用 PageView 渲染分页纸页：纸张尺寸、页边距、页眉页脚高度与页间距\
              共同决定文档坐标，屏幕映射交给 Viewport。\n\n\
              正文由应用按 PageLayout::content_rect(page) 的矩形叠加，\
              框架只出几何与纸页外观，分页/断行算法留在应用层。"
            }
            1 => {
                "坐标系统\n\n\
              screen = document × zoom + offset。\n\n\
              滚轮 Ctrl/⌘ 以光标为锚点缩放；普通滚轮平移；左键拖拽亦可平移。\n\n\
              页与页之间保留 gap 间距，命中判定 page_at 会把间距内的点归给上一页；\
              对开排布（TwoUp / 封面对开 TwoUpCover）下每行两页，但分页/断行仍由应用决定。"
            }
            _ => {
                "内容区\n\n\
              content_rect 扣除了左右边距与页眉页脚占位，\
              是正文可安全排布的竖直带状区域。\n\n\
              页眉贴内容区上方、页脚贴内容区下方并居中，随缩放一起移动。\n\n\
              —— 演示结束 ——"
            }
        }
    }

    struct PageViewDemo {
        /// 分页布局。
        layout: PageLayout,
        /// 当前页面排布方式。
        arrangement: PageArrangement,
        /// 视口（缩放 + 平移）。
        viewport: Viewport,
        /// 焦点句柄。
        focus_handle: FocusHandle,
        /// 画布可用尺寸。
        view_size: Size<Pixels>,
        /// 缩放/平移锚点（最近鼠标位置）。
        anchor: Point<Pixels>,
        /// 是否正在拖拽平移。
        dragging: bool,
        /// 上次指针位置。
        last_pointer: Point<Pixels>,
    }

    impl PageViewDemo {
        /// 创建演示视图。
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            let setup = PageSetup::default();
            let arrangement = PageArrangement::Single;
            let layout = PageLayout::new(setup, PAGE_COUNT).with_arrangement(arrangement);
            Self {
                layout,
                arrangement,
                viewport: Viewport::new()
                    .with_zoom(0.55)
                    .with_offset(px_point(120.0, 16.0)),
                focus_handle: cx.focus_handle(),
                view_size: size(px(900.0), px(640.0)),
                anchor: px_point(450.0, 320.0),
                dragging: false,
                last_pointer: px_point(0.0, 0.0),
            }
        }
    }

    fn px_point(x: f32, y: f32) -> Point<Pixels> {
        rgpui::point(px(x), px(y))
    }

    impl Render for PageViewDemo {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            if window.bounds().size != self.view_size && !self.dragging {
                self.view_size = window.bounds().size;
            }

            let viewport = self.viewport;
            let layout = self.layout.clone();
            let zoom = viewport.zoom();
            let font_size = px((14.0 * zoom).max(4.0));

            // 正文叠加：按文档矩形换算到屏幕，字号随缩放同步。
            let mut overlays: Vec<rgpui::AnyElement> = Vec::new();
            for page in 0..layout.page_count() {
                let content = layout.content_rect(page);
                let origin = viewport.document_to_screen(content.origin);
                let size = viewport.document_size_to_screen(content.size);
                overlays.push(
                    div()
                        .absolute()
                        .left(origin.x)
                        .top(origin.y)
                        .w(size.width)
                        .h(size.height)
                        .p(px(4.0))
                        .text_size(font_size)
                        .line_height(rgpui::relative(1.4))
                        .text_color(rgb(0x1e293b))
                        .child(page_text(page))
                        .into_any_element(),
                );
            }

            v_flex()
                .id("page-view-demo")
                .track_focus(&self.focus_handle)
                .size_full()
                .bg(rgb(0xf1f5f9))
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .px(px(12.0))
                        .py(px(8.0))
                        .items_center()
                        .border_b_1()
                        .border_color(rgb(0xcbd5e1))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(rgpui::FontWeight::SEMIBOLD)
                                .child("文档页视图（PageView）"),
                        )
                        .child(
                            Button::new("pv-zoom-out")
                                .label("缩小")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let anchor = this.anchor;
                                    this.viewport.zoom_out_at(anchor);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("pv-zoom-in")
                                .label("放大")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let anchor = this.anchor;
                                    this.viewport.zoom_in_at(anchor);
                                    cx.notify();
                                })),
                        )
                        .child(Button::new("pv-100").label("100%").on_click(cx.listener(
                            |this, _, _, cx| {
                                let anchor = this.anchor;
                                this.viewport.set_zoom_at(1.0, anchor);
                                cx.notify();
                            },
                        )))
                        .child(
                            Button::new("pv-fit")
                                .label("适应窗口")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let view = this.view_size;
                                    let doc = this.layout.document_size();
                                    this.viewport.fit(doc, view, px(24.0));
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("pv-arrangement")
                                .label(format!("排布：{}", arrangement_label(self.arrangement)))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    // 依固定顺序循环，并同步重建布局（排布影响文档尺寸与命中）。
                                    let index = match this.arrangement {
                                        PageArrangement::Single => 0,
                                        PageArrangement::TwoUp => 1,
                                        PageArrangement::TwoUpCover => 2,
                                    };
                                    let next = ARRANGEMENTS[(index + 1) % ARRANGEMENTS.len()];
                                    this.arrangement = next;
                                    this.layout = this.layout.clone().with_arrangement(next);
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .ml_auto()
                                .text_xs()
                                .text_color(rgb(0x64748b))
                                .child(format!(
                                    "缩放 {:.0}% · {} 页 · {} 行",
                                    zoom * 100.0,
                                    self.layout.page_count(),
                                    self.layout.row_count()
                                )),
                        ),
                )
                .child(
                    div()
                        .relative()
                        .flex_grow_1()
                        .min_h(px(0.0))
                        .overflow_hidden()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                this.dragging = true;
                                this.last_pointer = event.position;
                                cx.notify();
                            }),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, _, cx| {
                                this.dragging = false;
                                cx.notify();
                            }),
                        )
                        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                            this.anchor = event.position;
                            if this.dragging {
                                let delta = event.position - this.last_pointer;
                                this.last_pointer = event.position;
                                this.viewport.pan_by(delta);
                            }
                            cx.notify();
                        }))
                        .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                            this.viewport.apply_wheel(event, px(50.0));
                            cx.notify();
                        }))
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .child(PageView::new(
                                    layout,
                                    viewport,
                                    DocLabels {
                                        title: "rgpui 文档套件基座".into(),
                                    },
                                )),
                        )
                        .children(overlays),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(980.0), px(720.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| PageViewDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| PageViewDemo::new(window, cx)).into()
    }
}

mod drag_drop {

    //! 拖拽示例：展示元素拖动、拖放落点与拖动预览。
    //! 用法：左侧 tab 选「drag_drop」，按住色块拖动到别处，观察落点反馈。
    use rgpui::{
        App, Bounds, Context, Half, Hsla, Pixels, Point, Window, WindowBounds, WindowOptions, div,
        prelude::*, px, rgb, size,
    };
    use rgpui_platform::application;

    #[derive(Clone, Copy)]
    struct DragInfo {
        ix: usize,
        color: Hsla,
        position: Point<Pixels>,
    }

    impl DragInfo {
        fn new(ix: usize, color: Hsla) -> Self {
            Self {
                ix,
                color,
                position: Point::default(),
            }
        }

        fn position(mut self, pos: Point<Pixels>) -> Self {
            self.position = pos;
            self
        }
    }

    impl Render for DragInfo {
        fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
            let size = rgpui::size(px(120.), px(50.));

            div()
                .pl(self.position.x - size.width.half())
                .pt(self.position.y - size.height.half())
                .child(
                    div()
                        .flex()
                        .justify_center()
                        .items_center()
                        .w(size.width)
                        .h(size.height)
                        .bg(self.color.opacity(0.5))
                        .text_color(rgpui::white())
                        .text_xs()
                        .shadow_md()
                        .child(format!("Item {}", self.ix)),
                )
        }
    }

    struct DragDrop {
        drop_on: Option<DragInfo>,
    }

    impl DragDrop {
        fn new() -> Self {
            Self { drop_on: None }
        }
    }

    impl Render for DragDrop {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let items = [rgpui::blue(), rgpui::red(), rgpui::green()];

            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_5()
                .bg(rgpui::white())
                .justify_center()
                .items_center()
                .text_color(rgb(0x333333))
                .child(div().text_xl().text_center().child("Drop & Drop"))
                .child(
                    div()
                        .w_full()
                        .mb_10()
                        .justify_center()
                        .flex()
                        .flex_row()
                        .gap_4()
                        .items_center()
                        .children(items.into_iter().enumerate().map(|(ix, color)| {
                            let drag_info = DragInfo::new(ix, color);

                            div()
                                .id(("item", ix))
                                .size_32()
                                .flex()
                                .justify_center()
                                .items_center()
                                .border_2()
                                .border_color(color)
                                .text_color(color)
                                .cursor_move()
                                .hover(|this| this.bg(color.opacity(0.2)))
                                .child(format!("Item ({})", ix))
                                .on_drag(drag_info, |info: &DragInfo, position, _, cx| {
                                    cx.new(|_| info.position(position))
                                })
                        })),
                )
                .child(
                    div()
                        .id("drop-target")
                        .w_128()
                        .h_32()
                        .flex()
                        .justify_center()
                        .items_center()
                        .border_3()
                        .border_color(
                            self.drop_on
                                .map(|info| info.color)
                                .unwrap_or(rgpui::black()),
                        )
                        .when_some(self.drop_on, |this, info| this.bg(info.color.opacity(0.5)))
                        .on_drop(cx.listener(|this, info: &DragInfo, _, _| {
                            this.drop_on = Some(*info);
                        }))
                        .child("拖到此处"),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(800.), px(600.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| DragDrop::new()),
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| DragDrop::new()).into()
    }
}

mod focus_visible {

    //! 焦点可见性示例：展示键盘聚焦时的高亮边框（focus ring）。
    //! 用法：左侧 tab 选「focus_visible」，用 Tab 在按钮间移动，观察焦点样式。
    use rgpui::{
        App, Bounds, Context, Div, ElementId, FocusHandle, KeyBinding, SharedString, Stateful,
        Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, size,
    };
    use rgpui_platform::application;

    actions!(focus_visible_example, [Tab, TabPrev, Quit]);

    struct Example {
        focus_handle: FocusHandle,
        items: Vec<(FocusHandle, &'static str)>,
        message: SharedString,
    }

    impl Example {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let items = vec![
                (
                    cx.focus_handle().tab_index(1).tab_stop(true),
                    "Button with .focus() - always shows border when focused",
                ),
                (
                    cx.focus_handle().tab_index(2).tab_stop(true),
                    "Button with .focus_visible() - only shows border with keyboard",
                ),
                (
                    cx.focus_handle().tab_index(3).tab_stop(true),
                    "Button with both .focus() and .focus_visible()",
                ),
            ];

            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);

            Self {
                focus_handle,
                items,
                message: SharedString::from(
                    "Try clicking vs tabbing! Click shows no border, Tab shows border.",
                ),
            }
        }

        fn on_tab(&mut self, _: &Tab, window: &mut Window, cx: &mut Context<Self>) {
            window.focus_next(cx);
            self.message = SharedString::from("Pressed Tab - focus-visible border should appear!");
        }

        fn on_tab_prev(&mut self, _: &TabPrev, window: &mut Window, cx: &mut Context<Self>) {
            window.focus_prev(cx);
            self.message =
                SharedString::from("Pressed Shift-Tab - focus-visible border should appear!");
        }

        fn on_quit(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
            cx.quit();
        }
    }

    impl Render for Example {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            fn button_base(id: impl Into<ElementId>, label: &'static str) -> Stateful<Div> {
                div()
                    .id(id)
                    .h_16()
                    .w_full()
                    .flex()
                    .justify_center()
                    .items_center()
                    .bg(rgpui::rgb(0x2563eb))
                    .text_color(rgpui::white())
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|style| style.bg(rgpui::rgb(0x1d4ed8)))
                    .child(label)
            }

            div()
            .id("app")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_tab))
            .on_action(cx.listener(Self::on_tab_prev))
            .on_action(cx.listener(Self::on_quit))
            .size_full()
            .flex()
            .flex_col()
            .p_8()
            .gap_6()
            .bg(rgpui::rgb(0xf3f4f6))
            .child(
                div()
                    .text_2xl()
                    .font_weight(rgpui::FontWeight::BOLD)
                    .text_color(rgpui::rgb(0x111827))
                    .child("CSS focus-visible Demo"),
            )
            .child(
                div()
                    .p_4()
                    .rounded_md()
                    .bg(rgpui::rgb(0xdbeafe))
                    .text_color(rgpui::rgb(0x1e3a8a))
                    .child(self.message.clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(rgpui::FontWeight::BOLD)
                                    .text_color(rgpui::rgb(0x374151))
                                    .child("1. Regular .focus() - always visible:"),
                            )
                            .child(
                                button_base("button1", self.items[0].1)
                                    .track_focus(&self.items[0].0)
                                    .focus(|style| {
                                        style.border_4().border_color(rgpui::rgb(0xfbbf24))
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.message =
                                            "Clicked button 1 - focus border is visible!".into();
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(rgpui::FontWeight::BOLD)
                                    .text_color(rgpui::rgb(0x374151))
                                    .child("2. New .focus_visible() - only keyboard:"),
                            )
                            .child(
                                button_base("button2", self.items[1].1)
                                    .track_focus(&self.items[1].0)
                                    .focus_visible(|style| {
                                        style.border_4().border_color(rgpui::rgb(0x10b981))
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.message =
                                            "Clicked button 2 - no border! Try Tab instead.".into();
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(rgpui::FontWeight::BOLD)
                                    .text_color(rgpui::rgb(0x374151))
                                    .child(
                                        "3. Both .focus() (yellow) and .focus_visible() (green):",
                                    ),
                            )
                            .child(
                                button_base("button3", self.items[2].1)
                                    .track_focus(&self.items[2].0)
                                    .focus(|style| {
                                        style.border_4().border_color(rgpui::rgb(0xfbbf24))
                                    })
                                    .focus_visible(|style| {
                                        style.border_4().border_color(rgpui::rgb(0x10b981))
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.message =
                                            "Clicked button 3 - yellow border. Tab shows green!"
                                                .into();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            cx.bind_keys([
                KeyBinding::new("tab", Tab, None),
                KeyBinding::new("shift-tab", TabPrev, None),
                KeyBinding::new("cmd-q", Quit, None),
            ]);

            let bounds = Bounds::centered(None, size(px(800.), px(600.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Example::new(window, cx)),
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| Example::new(window, cx)).into()
    }
}

mod mouse_pressure {

    //! 鼠标压感示例：展示压力敏感输入的状态读取与可视化。
    //! 用法：左侧 tab 选「mouse_pressure」，在触控板/压感笔上按压观察数值变化。
    use rgpui::{
        App, Bounds, Context, MousePressureEvent, PressureStage, Window, WindowBounds,
        WindowOptions, div, prelude::*, px, rgb, size,
    };
    use rgpui_platform::application;

    struct MousePressureExample {
        pressure_stage: PressureStage,
        pressure_amount: f32,
    }

    impl Render for MousePressureExample {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                .child(format!("Pressure stage: {:?}", self.pressure_stage))
                .child(format!("Pressure amount: {:.2}", self.pressure_amount))
                .on_mouse_pressure(cx.listener(Self::on_mouse_pressure))
        }
    }

    impl MousePressureExample {
        fn on_mouse_pressure(
            &mut self,
            pressure_event: &MousePressureEvent,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.pressure_amount = pressure_event.pressure;
            self.pressure_stage = pressure_event.stage;

            cx.notify();
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
                |_, cx| {
                    cx.new(|_| MousePressureExample {
                        pressure_stage: PressureStage::Zero,
                        pressure_amount: 0.0,
                    })
                },
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| MousePressureExample {
            pressure_stage: PressureStage::Zero,
            pressure_amount: 0.0,
        })
        .into()
    }
}

mod native_menu {
    //! OS 原生弹出菜单示例
    //!
    //! 演示 [`NativeMenu`](rgpui::NativeMenu) + [`Window::show_native_menu`]：菜单交给系统
    //! 自己的控件画出来（Windows `TrackPopupMenuEx`、macOS `NSMenu`），选中结果按菜单项的
    //! `id` 经通道异步回传。平台没有这项能力时（Linux 的窗口菜单只能由窗管自己画）返回
    //! `None`，示例如实提示，并给出框架自绘菜单（[`ContextMenuExt`]）作为替代路径。
    //!
    //! 运行：`cargo run -p showcase --bin native_menu`

    use rgpui::{
        App, Bounds, Button, Context, ContextMenuExt as _, Entity, IntoElement, NativeMenu,
        ParentElement, PopupMenuItem, Render, SharedString, Styled as _, TitlebarOptions, Window,
        WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
    };
    use rgpui_platform::application;

    struct NativeMenuApp {
        /// 指向自身的句柄，供菜单项回调更新状态
        self_handle: Entity<Self>,
        /// 最近一次菜单交互的结果文本
        status: SharedString,
        /// 「自动换行」开关的当前状态，原生菜单里以勾选态呈现
        word_wrap: bool,
    }

    impl NativeMenuApp {
        /// 创建示例应用
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                self_handle: cx.entity(),
                status: "点击按钮或右键下方区域试试".into(),
                word_wrap: true,
            }
        }

        /// 按当前状态构建要弹出的原生菜单。
        ///
        /// 勾选态与禁用态都在描述阶段给出，平台层只负责把它们翻译成原生菜单的属性，
        /// 所以这份构建在两个平台上表现一致。
        fn build_menu(&self) -> NativeMenu {
            NativeMenu::new()
                .action("复制", "copy")
                .action("粘贴", "paste")
                .disabled("删除", "delete", true)
                .separator()
                .checked("自动换行", "word-wrap", self.word_wrap)
                .submenu("打开方式", |menu| {
                    menu.action("新窗口", "open-new")
                        .action("默认程序", "open-default")
                })
                .separator()
                .action("关于 rgpui", "about")
        }

        /// 处理一次菜单选择：切换开关项，其余只更新状态文本。
        fn apply_selection(&mut self, id: &str) {
            match id {
                "word-wrap" => {
                    self.word_wrap = !self.word_wrap;
                    self.status =
                        format!("自动换行已{}", if self.word_wrap { "开启" } else { "关闭" })
                            .into();
                }
                other => self.status = format!("菜单选中：{other}").into(),
            }
        }

        /// 在鼠标位置弹出 OS 原生菜单，并把选中结果回写到状态文本。
        fn show_native(&mut self, window: &mut Window, cx: &mut Context<Self>) {
            let menu = self.build_menu();
            let Some(receiver) = window.show_native_menu(window.mouse_position(), menu) else {
                self.status =
                    "本平台没有原生弹出菜单，请改用框架自绘的 PopupMenu（右键下方区域）".into();
                cx.notify();
                return;
            };

            // 菜单在平台自己的事件循环里模态运行，所以结果只能等：用户关掉菜单时通道被丢弃。
            cx.spawn(async move |this, cx| {
                let text = match receiver.await {
                    Ok(id) => Selected::Picked(id),
                    Err(_) => Selected::Dismissed,
                };
                cx.update(|cx| {
                    let _ = this.update(cx, |this, cx| {
                        match text {
                            Selected::Picked(id) => this.apply_selection(&id),
                            Selected::Dismissed => {
                                this.status = "菜单已关闭（未选择任何项）".into()
                            }
                        }
                        cx.notify();
                    });
                });
            })
            .detach();
        }
    }

    enum Selected {
        Picked(SharedString),
        Dismissed,
    }

    impl Render for NativeMenuApp {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let native_handle = self.self_handle.clone();
            let menu_handle = self.self_handle.clone();
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .bg(rgb(0x2b2b2b))
                .size_full()
                .text_color(rgb(0xffffff))
                .child(div().text_xl().child("OS 原生弹出菜单示例"))
                .child(div().text_sm().child(self.status.clone()))
                .child(
                    Button::new("native")
                        .label("在鼠标位置弹出原生菜单")
                        .on_click(move |_, window, cx| {
                            native_handle.update(cx, |this, cx| this.show_native(window, cx));
                        }),
                )
                .child(
                    // 框架自绘菜单：跨平台都可用，是原生菜单缺位时的替代
                    div()
                        .id("fallback-area")
                        .flex()
                        .items_center()
                        .justify_center()
                        .w_full()
                        .h(px(120.))
                        .rounded(px(8.))
                        .bg(rgb(0x3c3f41))
                        .child("右键此处：框架自绘的 ContextMenu")
                        .context_menu(move |menu, _, _| {
                            let copy_handle = menu_handle.clone();
                            let about_handle = menu_handle.clone();
                            menu.item(PopupMenuItem::new("复制").on_click(move |_, _, cx| {
                                copy_handle.update(cx, |this, cx| {
                                    this.status = "框架菜单选中：复制".into();
                                    cx.notify();
                                });
                            }))
                            .item(PopupMenuItem::new("粘贴").disabled(true))
                            .separator()
                            .item(
                                PopupMenuItem::new("关于 rgpui").on_click(move |_, _, cx| {
                                    about_handle.update(cx, |this, cx| {
                                        this.status = "框架菜单选中：关于 rgpui".into();
                                        cx.notify();
                                    });
                                }),
                            )
                        }),
                )
        }
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(560.), px(360.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Native Menu Example".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(NativeMenuApp::new),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(NativeMenuApp::new).into()
    }
}

mod native_notification {
    //! 原生系统通知示例
    //!
    //! 演示 `App::show_notification` / `App::push_notification`：
    //! 调用经 `Platform` trait 分发到各平台原生实现
    //! （Windows Toast、macOS User Notifications、Linux XDG Desktop Portal）。
    //! 一个按钮直接发送；另一个把发送结果回显到窗口状态文本。
    //!
    //! 运行：`cargo run -p showcase --bin interaction_gallery -- native_notification`

    use rgpui::{
        App, Bounds, Button, Context, Entity, IntoElement, ParentElement, Render, SharedString,
        Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div, prelude::*, px,
        rgb, size,
    };
    use rgpui_platform::application;

    /// 通知示例应用：记录最近一次发送结果并渲染状态文本与按钮
    struct NotificationApp {
        /// 指向自身的句柄，供按钮点击回调更新状态使用
        self_handle: Entity<Self>,
        /// 最近一次通知发送结果的展示文本
        status: SharedString,
    }

    impl NotificationApp {
        /// 创建示例应用，初始状态提示用户操作
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                self_handle: cx.entity(),
                status: "点击下方按钮发送原生通知".into(),
            }
        }

        /// 发送一条操作系统原生通知，并按发送结果更新窗口内状态文本。
        ///
        /// `show_notification` 返回 `Result`：平台不支持或通知门户拒绝时
        /// 会得到 `Err`，这里如实展示，便于在 headless 等环境下定位问题。
        fn send_notification(&mut self, title: &str, body: &str, cx: &mut Context<Self>) {
            self.status = match cx.show_notification(title, body) {
                Ok(()) => format!("已发送通知：{title}").into(),
                Err(err) => format!("发送失败：{err}").into(),
            };
            cx.notify();
        }
    }

    impl Render for NotificationApp {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let handle = self.self_handle.clone();
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .bg(rgb(0x2b2b2b))
                .size_full()
                .justify_center()
                .items_center()
                .text_color(rgb(0xffffff))
                .child(div().text_xl().child("原生系统通知示例"))
                .child(div().text_sm().child(self.status.clone()))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(
                            // 直接调用 App 便捷方法，不关心发送结果
                            Button::new("send-simple").label("发送通知").on_click(
                                |_event, _window, cx| {
                                    let _ = cx.push_notification(
                                        "rgpui",
                                        "这是一条来自 rgpui 的原生通知",
                                    );
                                },
                            ),
                        )
                        .child(
                            // 经实体句柄更新状态文本，把成败回显到窗口内
                            Button::new("send-with-status")
                                .label("发送通知并回显结果")
                                .on_click(move |_event, _window, cx| {
                                    handle.update(cx, |this, cx| {
                                        this.send_notification(
                                            "rgpui 带参数的通知",
                                            "标题与内容均由调用方传入",
                                            cx,
                                        );
                                    });
                                }),
                        ),
                )
        }
    }

    pub fn run_example() {
        // 诊断时用 RUST_LOG=info 运行，可看到通知门户调用等内部日志
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(480.), px(260.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Native Notification Example".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(NotificationApp::new),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(NotificationApp::new).into()
    }
}

mod set_menus {

    //! 原生菜单数据示例：展示如何为窗口设置应用菜单与菜单项回调。
    //! 用法：左侧 tab 选「set_menus」，点击窗口顶部菜单项观察控制台输出。
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

    pub fn run_example() {
        application().run(|cx: &mut App| {
            cx.set_global(AppState::new());

            // Bring the menu bar to the foreground (so you can see the menu bar)
            cx.activate(true);
            // Register the `quit` function so it can be referenced
            // by the `MenuItem::action` in the menu bar
            cx.on_action(quit);
            cx.on_action(toggle_check);
            // 原生菜单项动作的落点：macOS 全局菜单被选中、Windows 托盘/跳转列表被点击、
            // Linux 用旧 set_tray API 随菜单项带过去的 Action 被点击
            cx.on_app_menu_action(|action, cx| cx.dispatch_action(action));
            // Add menu items
            set_app_menus(cx);
            cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| SetMenus {}))
                .unwrap();
        });
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
            MenuItem::action("禁用项", rgpui::NoAction).disabled(true),
            MenuItem::submenu(Menu::new("禁用子菜单").disabled(true)),
            MenuItem::separator(),
            MenuItem::action("List Mode", ToggleCheck)
                .checked(app_state.view_mode == ViewMode::List),
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
    actions!(set_menus_set_menus, [Quit, ToggleCheck]);

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

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        // 该示例的菜单项回调依赖全局 AppState，嵌入模式下先注册。
        cx.set_global(AppState::new());
        cx.new(|_| SetMenus {}).into()
    }
}

mod tab_stop {

    //! Tab 焦点遍历示例：展示 `tab_stop` 顺序与可聚焦元素。
    //! 用法：左侧 tab 选「tab_stop」，连续按 Tab 观察焦点按序移动。
    use rgpui::{
        App, Bounds, Context, Div, ElementId, FocusHandle, KeyBinding, SharedString, Stateful,
        Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, size,
    };
    use rgpui_platform::application;

    actions!(tab_stop_example, [Tab, TabPrev]);

    struct Example {
        focus_handle: FocusHandle,
        items: Vec<FocusHandle>,
        message: SharedString,
    }

    impl Example {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let items = vec![
                cx.focus_handle().tab_index(1).tab_stop(true),
                cx.focus_handle().tab_index(2).tab_stop(true),
                cx.focus_handle().tab_index(3).tab_stop(true),
                cx.focus_handle(),
                cx.focus_handle().tab_index(2).tab_stop(true),
            ];

            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);

            Self {
                focus_handle,
                items,
                message: SharedString::from("Press `Tab`, `Shift-Tab` to switch focus."),
            }
        }

        fn on_tab(&mut self, _: &Tab, window: &mut Window, cx: &mut Context<Self>) {
            window.focus_next(cx);
            self.message = SharedString::from("You have pressed `Tab`.");
        }

        fn on_tab_prev(&mut self, _: &TabPrev, window: &mut Window, cx: &mut Context<Self>) {
            window.focus_prev(cx);
            self.message = SharedString::from("You have pressed `Shift-Tab`.");
        }
    }

    impl Render for Example {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            fn tab_stop_style<T: Styled>(this: T) -> T {
                this.border_3().border_color(rgpui::blue())
            }

            fn button(id: impl Into<ElementId>) -> Stateful<Div> {
                div()
                    .id(id)
                    .h_10()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .items_center()
                    .border_1()
                    .border_color(rgpui::black())
                    .bg(rgpui::black())
                    .text_color(rgpui::white())
                    .focus(tab_stop_style)
                    .shadow_sm()
            }

            div()
                .id("app")
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(Self::on_tab))
                .on_action(cx.listener(Self::on_tab_prev))
                .size_full()
                .flex()
                .flex_col()
                .p_4()
                .gap_3()
                .bg(rgpui::white())
                .text_color(rgpui::black())
                .child(self.message.clone())
                .children(
                    self.items
                        .clone()
                        .into_iter()
                        .enumerate()
                        .map(|(ix, item_handle)| {
                            div()
                                .id(("item", ix))
                                .track_focus(&item_handle)
                                .h_10()
                                .w_full()
                                .flex()
                                .justify_center()
                                .items_center()
                                .border_1()
                                .border_color(rgpui::black())
                                .when(
                                    item_handle.tab_stop && item_handle.is_focused(window),
                                    tab_stop_style,
                                )
                                .map(|this| match item_handle.tab_stop {
                                    true => this
                                        .hover(|this| this.bg(rgpui::black().opacity(0.1)))
                                        .child(format!("tab_index: {}", item_handle.tab_index)),
                                    false => this.opacity(0.4).child("tab_stop: false"),
                                })
                        }),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_3()
                        .items_center()
                        .child(
                            button("el1")
                                .tab_index(4)
                                .child("Button 1")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.message = "You have clicked Button 1.".into();
                                    cx.notify();
                                })),
                        )
                        .child(
                            button("el2")
                                .tab_index(5)
                                .child("Button 2")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.message = "You have clicked Button 2.".into();
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    div()
                        .id("group-1")
                        .tab_index(6)
                        .tab_group()
                        .tab_stop(false)
                        .child(
                            button("group-1-button-1")
                                .tab_index(1)
                                .child("Tab index [6, 1]"),
                        )
                        .child(
                            button("group-1-button-2")
                                .tab_index(2)
                                .child("Tab index [6, 2]"),
                        )
                        .child(
                            button("group-1-button-3")
                                .tab_index(3)
                                .child("Tab index [6, 3]"),
                        ),
                )
                .child(
                    div()
                        .id("group-2")
                        .tab_index(7)
                        .tab_group()
                        .tab_stop(false)
                        .child(
                            button("group-2-button-1")
                                .tab_index(1)
                                .child("Tab index [7, 1]"),
                        )
                        .child(
                            button("group-2-button-2")
                                .tab_index(2)
                                .child("Tab index [7, 2]"),
                        )
                        .child(
                            button("group-2-button-3")
                                .tab_index(3)
                                .child("Tab index [7, 3]"),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            cx.bind_keys([
                KeyBinding::new("tab", Tab, None),
                KeyBinding::new("shift-tab", TabPrev, None),
            ]);

            let bounds = Bounds::centered(None, size(px(800.), px(600.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Example::new(window, cx)),
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| Example::new(window, cx)).into()
    }
}

mod chat {
    //! 聊天三件演示：Bubble 气泡 + MessageScroller 吸底/未读 + Marker 分隔线。

    use rgpui::{
        App, Avatar, Bounds, Context, Render, Window, WindowBounds, WindowOptions,
        chat::{
            Bubble, BubbleSide, Marker, Message, MessageGroup, MessageScroller,
            MessageScrollerState,
        },
        div, h_flex,
        prelude::*,
        px, size, v_flex,
    };
    use rgpui_platform::application;

    struct ChatDemo {
        scroller: rgpui::Entity<MessageScrollerState>,
        groups: Vec<MessageGroup>,
        counter: usize,
    }

    impl ChatDemo {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let scroller = cx.new(|_| MessageScrollerState::new());
            let _ = window;
            Self {
                scroller,
                groups: vec![MessageGroup {
                    sender: "助手".into(),
                    messages: vec![Message::text("你好，有什么可以帮你？")],
                }],
                counter: 0,
            }
        }

        fn push(&mut self, sender: &str, mine: bool, cx: &mut Context<Self>) {
            self.counter += 1;
            let text = if mine {
                format!("我的第 {} 条消息", self.counter)
            } else {
                format!("助手的第 {} 条回复", self.counter)
            };
            match self.groups.last_mut() {
                Some(group) if group.sender.as_str() == sender => {
                    group.messages.push(Message::text(text));
                }
                _ => self.groups.push(MessageGroup {
                    sender: sender.into(),
                    messages: vec![Message::text(text)],
                }),
            }
            cx.notify();
        }
    }

    impl Render for ChatDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let groups = self.groups.clone();
            v_flex()
                .size_full()
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .p(px(8.0))
                        .child(
                            rgpui::Button::new("chat-push-theirs")
                                .label("追加助手消息")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.push("助手", false, cx);
                                })),
                        )
                        .child(
                            rgpui::Button::new("chat-push-mine")
                                .label("追加我的消息")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.push("我", true, cx);
                                })),
                        )
                        .child(rgpui::Button::new("chat-clear").label("清空").on_click(
                            cx.listener(|this, _, _, cx| {
                                this.groups.clear();
                                this.counter = 0;
                                cx.notify();
                            }),
                        )),
                )
                .child(
                    div().flex_1().p(px(12.0)).child(
                        MessageScroller::new(self.scroller.clone()).child(
                            v_flex()
                                .gap(px(4.0))
                                .child(Marker::new("今天 09:00"))
                                .children(groups.into_iter().map(|group| {
                                    let mine = group.sender.as_str() == "我";
                                    let sender = group.sender.clone();
                                    let initial: String =
                                        sender.chars().next().unwrap_or('?').to_string();
                                    v_flex()
                                        .gap(px(2.0))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .w_full()
                                                .when(mine, |this| this.justify_end())
                                                .when(!mine, |this| this.justify_start())
                                                .child(div().text_xs().px(px(4.0)).child(sender)),
                                        )
                                        .children(group.messages.into_iter().map(move |msg| {
                                            let text = match msg.content {
                                                MessageType::Text(text) => text,
                                                MessageType::CodeBlock { code, .. } => code,
                                            };
                                            Bubble::new()
                                                .side(if mine {
                                                    BubbleSide::Right
                                                } else {
                                                    BubbleSide::Left
                                                })
                                                .avatar(Avatar::new(initial.clone()).size(px(28.0)))
                                                .child(div().text_sm().child(text))
                                        }))
                                })),
                        ),
                    ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(560.0), px(680.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| ChatDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| ChatDemo::new(window, cx)).into()
    }
}

mod dock {
    //! Dock 布局演示：四区域标签页、拖拽跨区、关闭/显隐、布局持久化。

    use rgpui::{
        App, Bounds, Context, Render, Window, WindowBounds, WindowOptions,
        components::{DockArea, DockAreaState, DockPanel, DockPosition},
        div, h_flex,
        prelude::*,
        px, size, v_flex,
    };
    use rgpui_platform::application;

    struct DockDemo {
        dock: rgpui::Entity<DockAreaState>,
        saved_len: usize,
    }

    impl DockDemo {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let dock = cx.new(|_| DockAreaState::new());
            dock.update(cx, |dock, cx| {
                dock.add_panel(
                    DockPanel::new("files", "文件树", DockPosition::Left, |_, _| {
                        div()
                            .p(px(12.0))
                            .child("src/\n  main.rs\n  lib.rs")
                            .into_any_element()
                    }),
                    cx,
                );
                dock.add_panel(
                    DockPanel::new("outline", "大纲", DockPosition::Left, |_, _| {
                        div()
                            .p(px(12.0))
                            .child("fn main\nfn render")
                            .into_any_element()
                    }),
                    cx,
                );
                dock.add_panel(
                    DockPanel::new("editor", "编辑器", DockPosition::Center, |_, _| {
                        div()
                            .p(px(12.0))
                            .child("fn main() {\n    println!(\"hello\");\n}")
                            .into_any_element()
                    }),
                    cx,
                );
                dock.add_panel(
                    DockPanel::new("props", "属性", DockPosition::Right, |_, _| {
                        div()
                            .p(px(12.0))
                            .child("width: 240\nheight: 自适应")
                            .into_any_element()
                    }),
                    cx,
                );
                dock.add_panel(
                    DockPanel::new("terminal", "终端", DockPosition::Bottom, |_, _| {
                        div().p(px(12.0)).child("$ cargo run").into_any_element()
                    }),
                    cx,
                );
            });
            let _ = window;
            Self { dock, saved_len: 0 }
        }
    }

    impl Render for DockDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let saved = self.saved_len;
            v_flex()
                .size_full()
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .p(px(8.0))
                        .child(rgpui::Button::new("dock-save").label("保存布局").on_click(
                            cx.listener(|this, _, _, cx| {
                                let json = this.dock.read(cx).layout_json();
                                this.saved_len = json.len();
                                cx.notify();
                            }),
                        ))
                        .child(
                            rgpui::Button::new("dock-hide-right")
                                .label("显隐右侧")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let visible =
                                        this.dock.read(cx).is_visible(DockPosition::Right);
                                    this.dock.update(cx, |dock, cx| {
                                        dock.set_visible(DockPosition::Right, !visible, cx);
                                    });
                                })),
                        )
                        .child(div().text_sm().child(format!("已保存布局 {saved} 字节"))),
                )
                .child(div().flex_1().child(DockArea::new(self.dock.clone())))
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(960.0), px(640.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| DockDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| DockDemo::new(window, cx)).into()
    }
}

mod grid_layout {

    //! 网格布局示例（圣杯布局）：展示弹性布局中多栏组合。
    //! 用法：左侧 tab 选「grid_layout」，查看多栏自适应排布。
    use rgpui::{
        App, Bounds, Context, Hsla, Window, WindowBounds, WindowOptions, container_query, div,
        prelude::*, px, rgb, size,
    };
    use rgpui_platform::application;

    // https://en.wikipedia.org/wiki/Holy_grail_(web_design)
    //
    // Resize the window: the layout is chosen by `container_query` based on the
    // measured size of the container, collapsing to a single stacked column when
    // it becomes too narrow for the three-column grid.
    struct HolyGrailExample {}

    impl Render for HolyGrailExample {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            container_query(|container_size, _window, _cx| {
                let block = |color: Hsla| {
                    div()
                        .size_full()
                        .bg(color)
                        .border_1()
                        .border_dashed()
                        .rounded_md()
                        .border_color(rgpui::white())
                        .items_center()
                };

                let header =
                    block(rgpui::white()).child(format!("Header  — {}", container_size.width));
                let table_of_contents = block(rgpui::red()).child("Table of contents");
                let content = block(rgpui::green()).child("Content");
                let ad = block(rgpui::blue())
                    .child("AD :(")
                    .text_color(rgpui::white());
                let footer = block(rgpui::black())
                    .text_color(rgpui::white())
                    .child("Footer");

                let container = div().gap_1().bg(rgb(0x505050)).shadow_lg().size_full();

                if container_size.width < px(400.) {
                    container
                        .flex()
                        .flex_col()
                        .child(header.h_12().flex_none())
                        .child(table_of_contents.h_20().flex_none())
                        .child(content.flex_1())
                        .child(ad.h_20().flex_none())
                        .child(footer.h_12().flex_none())
                } else {
                    container
                        .grid()
                        .grid_cols(5)
                        .grid_rows(5)
                        .child(header.row_span(1).col_span_full())
                        .child(table_of_contents.col_span(1).h_56())
                        .child(content.col_span(3).row_span(3))
                        .child(ad.col_span(1).row_span(3))
                        .child(footer.row_span(1).col_span_full())
                }
            })
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
                |_, cx| cx.new(|_| HolyGrailExample {}),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| HolyGrailExample {}).into()
    }
}

mod status_bar {
    //! 状态栏（StatusBar）示例。
    //!
    //! `StatusBar` 是纯值驱动的组件：左/右两组 `StatusBarItem`，每项可带图标、提示、
    //! 点击回调、置灰（muted）/激活（active）/禁用（disabled）等状态。每次重绘按当前
    //! 状态重新组装即可，无需同步。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin layout_gallery -- status_bar
    //! ```

    use rgpui::components::status_bar::{StatusBar, StatusBarItem};
    use rgpui::{
        App, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window, WindowBounds,
        WindowOptions, div, h_flex, prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    /// 状态栏演示状态（纯值）。
    struct StatusBarDemo {
        line: usize,
        column: usize,
        language: &'static str,
        lsp_connected: bool,
        errors: usize,
        warnings: usize,
        branch: &'static str,
    }

    impl StatusBarDemo {
        fn new() -> Self {
            Self {
                line: 42,
                column: 15,
                language: "Rust",
                lsp_connected: true,
                errors: 2,
                warnings: 5,
                branch: "main",
            }
        }

        /// 按当前值组装状态栏。
        fn status_bar(&self) -> StatusBar {
            let lsp = if self.lsp_connected {
                "rust-analyzer ✓".to_string()
            } else {
                "No LSP".to_string()
            };
            let mut left = vec![StatusBarItem::new(lsp).id("lsp").active(self.lsp_connected)];
            if self.errors > 0 {
                left.push(StatusBarItem::new(format!("{} errors", self.errors)).id("errors"));
            }
            if self.warnings > 0 {
                left.push(StatusBarItem::new(format!("{} warnings", self.warnings)).id("warnings"));
            }
            let right = vec![
                StatusBarItem::new(self.branch).id("branch").muted(true),
                StatusBarItem::new(format!("Ln {}, Col {}", self.line, self.column))
                    .id("cursor")
                    .muted(true),
                StatusBarItem::new(self.language).id("language").muted(true),
                StatusBarItem::new("UTF-8").id("encoding").muted(true),
            ];
            StatusBar::new().left(left).right(right)
        }
    }

    impl Render for StatusBarDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let status_bar = self.status_bar();

            v_flex()
                .id("status-bar-demo")
                .size_full()
                .bg(rgb(0xffffff))
                .child(
                    v_flex()
                        .flex_1()
                        .gap(px(16.0))
                        .p(px(24.0))
                        .child(div().text_2xl().child("状态栏（StatusBar）"))
                        .child(
                            h_flex()
                                .gap(px(8.0))
                                .flex_wrap()
                                .child(button("sb-line-up", "行号 +1", cx, |this, cx| {
                                    this.line += 1;
                                    cx.notify();
                                }))
                                .child(button("sb-col-up", "列号 +5", cx, |this, cx| {
                                    this.column += 5;
                                    cx.notify();
                                }))
                                .child(button("sb-lang", "切换语言", cx, |this, cx| {
                                    this.language = if this.language == "Rust" {
                                        "TypeScript"
                                    } else {
                                        "Rust"
                                    };
                                    cx.notify();
                                }))
                                .child(button("sb-lsp", "切换 LSP", cx, |this, cx| {
                                    this.lsp_connected = !this.lsp_connected;
                                    cx.notify();
                                }))
                                .child(button("sb-error", "错误 +1", cx, |this, cx| {
                                    this.errors += 1;
                                    cx.notify();
                                }))
                                .child(button("sb-reset", "重置", cx, |this, cx| {
                                    *this = StatusBarDemo::new();
                                    cx.notify();
                                })),
                        ),
                )
                // 底部状态栏。
                .child(status_bar)
        }
    }

    /// 一个简单的按钮辅助函数。
    fn button(
        id: &'static str,
        label: &'static str,
        cx: &mut Context<StatusBarDemo>,
        on_click: impl Fn(&mut StatusBarDemo, &mut Context<StatusBarDemo>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .px(px(12.0))
            .py(px(6.0))
            .rounded_md()
            .bg(rgb(0xf0f0f0))
            .cursor_pointer()
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
    }

    /// 启动示例。
    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(720.0), px(420.0)), cx);
            let _ = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_window, cx| cx.new(|_cx| StatusBarDemo::new()),
            );
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_cx| StatusBarDemo::new()).into()
    }
}

mod tree {
    //! 树（Tree）示例。
    //!
    //! 演示 `TreeState` 实体 + `tree` 视图：层级展开、点击选中、回车确认。
    //! 通过订阅 `TreeEvent::{Selected, Confirmed}` 拿到选中/确认的节点 id。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin layout_gallery -- tree
    //! ```

    use rgpui::tree::{TreeEvent, TreeItem, TreeState, tree};
    use rgpui::{
        App, Bounds, Context, Entity, IntoElement, ParentElement, Render, Window, WindowBounds,
        WindowOptions, div, prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    /// 示例根视图。
    struct TreeDemo {
        /// 树状态实体。
        tree: Entity<TreeState>,
        /// 事件回显文本。
        message: String,
    }

    impl TreeDemo {
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            let tree = cx.new(|cx| {
                TreeState::new(cx).items(vec![
                    TreeItem::new("src", "src")
                        .child(TreeItem::new("src/lib.rs", "lib.rs"))
                        .child(TreeItem::new("src/main.rs", "main.rs"))
                        .child(
                            TreeItem::new("src/components", "components")
                                .child(TreeItem::new("src/components/button.rs", "button.rs")),
                        ),
                    TreeItem::new("Cargo.toml", "Cargo.toml"),
                    TreeItem::new("README.md", "README.md"),
                ])
            });
            // 订阅选中 / 确认事件。
            cx.subscribe(&tree, |this, _state, event: &TreeEvent, cx| match event {
                TreeEvent::Selected(id) => {
                    this.message = format!("点击选中：{id}");
                    cx.notify();
                }
                TreeEvent::Confirmed(id) => {
                    this.message = format!("回车确认：{id}");
                    cx.notify();
                }
                _ => {}
            })
            .detach();

            Self {
                tree,
                message: "点击树行选中，回车确认当前行".to_string(),
            }
        }
    }

    impl Render for TreeDemo {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            v_flex()
                .id("tree-demo")
                .size_full()
                .gap(px(16.0))
                .p(px(24.0))
                .bg(rgb(0xffffff))
                .child(div().text_2xl().child("树（Tree）"))
                .child(
                    v_flex()
                        .gap(px(8.0))
                        .p(px(12.0))
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(0xe0e0e0))
                        .child(tree(&self.tree, |ix, entry, _selected, _window, _cx| {
                            rgpui::list::ListItem::new(("tree-demo", ix))
                                .child(entry.item().label.clone())
                        }))
                        .child(div().text_sm().child(self.message.clone())),
                )
        }
    }

    /// 启动示例：注册树的键绑定。
    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::tree::init(cx);
            let bounds = Bounds::centered(None, size(px(560.0), px(520.0)), cx);
            let _ = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| TreeDemo::new(window, cx)),
            );
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| TreeDemo::new(window, cx)).into()
    }
}

mod image_loading {

    //! 图片加载示例：展示加载中骨架动画与失败占位。
    //! 用法：左侧 tab 选「image_loading」，点击图片可重新加载观察不同状态。
    use std::{path::Path, sync::Arc, time::Duration};

    use rgpui::{
        Animation, AnimationExt, App, Asset, AssetLogger, AssetSource, Bounds, Context, Hsla,
        ImageAssetLoader, ImageCacheError, ImgResourceLoader, LOADING_DELAY, Length, RenderImage,
        Resource, SharedString, Window, WindowBounds, WindowOptions, black, div, img, prelude::*,
        pulsating_between, px, red, size,
    };
    use rgpui_platform::application;

    struct Assets {}

    impl AssetSource for Assets {
        fn load(&self, path: &str) -> anyhow::Result<Option<std::borrow::Cow<'static, [u8]>>> {
            std::fs::read(path)
                .map(Into::into)
                .map_err(Into::into)
                .map(Some)
        }

        fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
            Ok(std::fs::read_dir(path)?
                .filter_map(|entry| {
                    Some(SharedString::from(
                        entry.ok()?.path().to_string_lossy().into_owned(),
                    ))
                })
                .collect::<Vec<_>>())
        }
    }

    const IMAGE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../image_showcase/assets/app-icon.png"
    );

    #[derive(Copy, Clone, Hash)]
    struct LoadImageParameters {
        timeout: Duration,
        fail: bool,
    }

    struct LoadImageWithParameters {}

    impl Asset for LoadImageWithParameters {
        type Source = LoadImageParameters;

        type Output = Result<Arc<RenderImage>, ImageCacheError>;

        fn load(
            parameters: Self::Source,
            cx: &mut App,
        ) -> impl std::future::Future<Output = Self::Output> + Send + 'static {
            let timer = cx.background_executor().timer(parameters.timeout);
            let data = AssetLogger::<ImageAssetLoader>::load(
                Resource::Path(Path::new(IMAGE).to_path_buf().into()),
                cx,
            );
            async move {
                timer.await;
                if parameters.fail {
                    log::error!("Intentionally failed to load image");
                    Err(anyhow::anyhow!("Failed to load image").into())
                } else {
                    data.await
                }
            }
        }
    }

    struct ImageLoadingExample {}

    impl ImageLoadingExample {
        fn loading_element() -> impl IntoElement {
            div().size_full().flex_none().p_0p5().rounded_xs().child(
                div().size_full().with_animation(
                    "loading-bg",
                    Animation::new(Duration::from_secs(3))
                        .repeat()
                        .with_easing(pulsating_between(0.04, 0.24)),
                    move |this, delta| this.bg(black().opacity(delta)),
                ),
            )
        }

        fn fallback_element() -> impl IntoElement {
            let fallback_color: Hsla = black().opacity(0.5);

            div().size_full().flex_none().p_0p5().child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_xs()
                    .text_sm()
                    .text_color(fallback_color)
                    .border_1()
                    .border_color(fallback_color)
                    .child("?"),
            )
        }
    }

    impl Render for ImageLoadingExample {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().flex().flex_col().size_full().justify_around().child(
                div().flex().flex_row().w_full().justify_around().child(
                    div()
                        .flex()
                        .bg(rgpui::white())
                        .size(Length::Definite(px(300.0).into()))
                        .justify_center()
                        .items_center()
                        .child({
                            let image_source = LoadImageParameters {
                                timeout: LOADING_DELAY.saturating_sub(Duration::from_millis(25)),
                                fail: false,
                            };

                            // Load within the 'loading delay', should not show loading fallback
                            img(move |window: &mut Window, cx: &mut App| {
                                window.use_asset::<LoadImageWithParameters>(&image_source, cx)
                            })
                            .id("image-1")
                            .border_1()
                            .size_12()
                            .with_fallback(|| Self::fallback_element().into_any_element())
                            .border_color(red())
                            .with_loading(|| Self::loading_element().into_any_element())
                            .on_click(move |_, _, cx| {
                                cx.remove_asset::<LoadImageWithParameters>(&image_source);
                            })
                        })
                        .child({
                            // Load after a long delay
                            let image_source = LoadImageParameters {
                                timeout: Duration::from_secs(5),
                                fail: false,
                            };

                            img(move |window: &mut Window, cx: &mut App| {
                                window.use_asset::<LoadImageWithParameters>(&image_source, cx)
                            })
                            .id("image-2")
                            .with_fallback(|| Self::fallback_element().into_any_element())
                            .with_loading(|| Self::loading_element().into_any_element())
                            .size_12()
                            .border_1()
                            .border_color(red())
                            .on_click(move |_, _, cx| {
                                cx.remove_asset::<LoadImageWithParameters>(&image_source);
                            })
                        })
                        .child({
                            // Fail to load image after a long delay
                            let image_source = LoadImageParameters {
                                timeout: Duration::from_secs(5),
                                fail: true,
                            };

                            // Fail to load after a long delay
                            img(move |window: &mut Window, cx: &mut App| {
                                window.use_asset::<LoadImageWithParameters>(&image_source, cx)
                            })
                            .id("image-3")
                            .with_fallback(|| Self::fallback_element().into_any_element())
                            .with_loading(|| Self::loading_element().into_any_element())
                            .size_12()
                            .border_1()
                            .border_color(red())
                            .on_click(move |_, _, cx| {
                                cx.remove_asset::<LoadImageWithParameters>(&image_source);
                            })
                        })
                        .child({
                            // Ensure that the normal image loader doesn't spam logs
                            let image_source = Path::new(
                                "this/file/really/shouldn't/exist/or/won't/be/an/image/I/hope",
                            )
                            .to_path_buf();
                            img(image_source.clone())
                                .id("image-4")
                                .border_1()
                                .size_12()
                                .with_fallback(|| Self::fallback_element().into_any_element())
                                .border_color(red())
                                .with_loading(|| Self::loading_element().into_any_element())
                                .on_click(move |_, _, cx| {
                                    cx.remove_asset::<ImgResourceLoader>(
                                        &image_source.clone().into(),
                                    );
                                })
                        }),
                ),
            )
        }
    }

    pub fn run_example() {
        env_logger::init();
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
                cx.new(|_| ImageLoadingExample {})
            })
            .unwrap();
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| ImageLoadingExample {}).into()
    }
}

mod image_gallery {

    //! 图片画廊示例：对比手动缓存与 LRU 自动缓存的图片列表。
    //! 用法：左侧 tab 选「image_gallery」，滚动列表观察缓存行为。
    use futures::FutureExt;
    #[cfg(not(target_family = "wasm"))]
    use rgpui::http_client::reqwest_client::ReqwestClient;
    use rgpui::{
        App, AppContext, Asset as _, AssetLogger, Bounds, ClickEvent, Context, ElementId, Entity,
        ImageAssetLoader, ImageCache, ImageCacheProvider, KeyBinding, Menu, MenuItem,
        RetainAllImageCache, SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions,
        actions, div, hash, image_cache, img, prelude::*, px, rgb, size,
    };
    use std::{collections::HashMap, sync::Arc};

    const IMAGES_IN_GALLERY: usize = 30;

    struct ImageGallery {
        image_key: String,
        items_count: usize,
        total_count: usize,
        image_cache: Entity<RetainAllImageCache>,
    }

    impl ImageGallery {
        fn on_next_image(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
            self.image_cache
                .update(cx, |image_cache, cx| image_cache.clear(window, cx));

            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis();

            self.image_key = format!("{}", t);
            self.total_count += self.items_count;
            cx.notify();
        }
    }

    impl Render for ImageGallery {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let image_url: SharedString =
                format!("https://picsum.photos/400/200?t={}", self.image_key).into();

            div()
                .flex()
                .flex_col()
                .text_color(rgpui::white())
                .child("Manually managed image cache:")
                .child(
                    div()
                        .image_cache(self.image_cache.clone())
                        .id("main")
                        .text_color(rgpui::black())
                        .bg(rgb(0xE9E9E9))
                        .overflow_y_scroll()
                        .p_4()
                        .size_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .flex_row()
                                .justify_between()
                                .child(format!(
                                    "展示图片并测试内存占用（已渲染 {} 张图片）。",
                                    self.total_count
                                ))
                                .child(
                                    div()
                                        .id("btn")
                                        .py_1()
                                        .px_4()
                                        .bg(rgpui::black())
                                        .hover(|this| this.opacity(0.8))
                                        .text_color(rgpui::white())
                                        .text_center()
                                        .w_40()
                                        .child("Next Photos")
                                        .on_click(cx.listener(Self::on_next_image)),
                                ),
                        )
                        .child(
                            div()
                                .id("image-gallery")
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .gap_x_4()
                                .gap_y_2()
                                .justify_around()
                                .children(
                                    (0..self.items_count)
                                        .map(|ix| img(format!("{}-{}", image_url, ix)).size_20()),
                                ),
                        ),
                )
                .child("Automatically managed image cache:")
                .child(
                    image_cache(simple_lru_cache("lru-cache", IMAGES_IN_GALLERY)).child(
                        div()
                            .id("main")
                            .bg(rgb(0xE9E9E9))
                            .text_color(rgpui::black())
                            .overflow_y_scroll()
                            .p_4()
                            .size_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("image-gallery")
                                    .flex()
                                    .flex_row()
                                    .flex_wrap()
                                    .gap_x_4()
                                    .gap_y_2()
                                    .justify_around()
                                    .children(
                                        (0..self.items_count).map(|ix| {
                                            img(format!("{}-{}", image_url, ix)).size_20()
                                        }),
                                    ),
                            ),
                    ),
                )
        }
    }

    fn simple_lru_cache(id: impl Into<ElementId>, max_items: usize) -> SimpleLruCacheProvider {
        SimpleLruCacheProvider {
            id: id.into(),
            max_items,
        }
    }

    struct SimpleLruCacheProvider {
        id: ElementId,
        max_items: usize,
    }

    impl ImageCacheProvider for SimpleLruCacheProvider {
        fn provide(&mut self, window: &mut Window, cx: &mut App) -> rgpui::AnyImageCache {
            window
                .with_global_id(self.id.clone(), |global_id, window| {
                    window.with_element_state::<Entity<SimpleLruCache>, _>(
                        global_id,
                        |lru_cache, _window| {
                            let mut lru_cache = lru_cache.unwrap_or_else(|| {
                                cx.new(|cx| SimpleLruCache::new(self.max_items, cx))
                            });
                            if lru_cache.read(cx).max_items != self.max_items {
                                lru_cache = cx.new(|cx| SimpleLruCache::new(self.max_items, cx));
                            }
                            (lru_cache.clone(), lru_cache)
                        },
                    )
                })
                .into()
        }
    }

    struct SimpleLruCache {
        max_items: usize,
        usages: Vec<u64>,
        cache: HashMap<u64, rgpui::ImageCacheItem>,
    }

    impl SimpleLruCache {
        fn new(max_items: usize, cx: &mut Context<Self>) -> Self {
            cx.on_release(|simple_cache, cx| {
                for (_, mut item) in std::mem::take(&mut simple_cache.cache) {
                    if let Some(Ok(image)) = item.get() {
                        cx.drop_image(image, None);
                    }
                }
            })
            .detach();

            Self {
                max_items,
                usages: Vec::with_capacity(max_items),
                cache: HashMap::with_capacity(max_items),
            }
        }
    }

    impl ImageCache for SimpleLruCache {
        fn load(
            &mut self,
            resource: &rgpui::Resource,
            window: &mut Window,
            cx: &mut App,
        ) -> Option<Result<Arc<rgpui::RenderImage>, rgpui::ImageCacheError>> {
            assert_eq!(self.usages.len(), self.cache.len());
            assert!(self.cache.len() <= self.max_items);

            let hash = hash(resource);

            if let Some(item) = self.cache.get_mut(&hash) {
                let current_ix = self
                    .usages
                    .iter()
                    .position(|item| *item == hash)
                    .expect("缓存与用量必须保持一致");
                self.usages.remove(current_ix);
                self.usages.insert(0, hash);

                return item.get();
            }

            let fut = AssetLogger::<ImageAssetLoader>::load(resource.clone(), cx);
            let task = cx.background_executor().spawn(fut).shared();
            if self.usages.len() == self.max_items {
                let oldest = self.usages.pop().unwrap();
                let mut image = self.cache.remove(&oldest).expect("缓存与用量必须同步");
                if let Some(Ok(image)) = image.get() {
                    cx.drop_image(image, Some(window));
                }
            }
            self.cache
                .insert(hash, rgpui::ImageCacheItem::Loading(task.clone()));
            self.usages.insert(0, hash);

            let entity = window.current_view();
            window
                .spawn(cx, {
                    async move |cx| {
                        _ = task.await;
                        cx.on_next_frame(move |_, cx| {
                            cx.notify(entity);
                        });
                    }
                })
                .detach();

            None
        }
    }

    actions!(image_gallery_image, [Quit]);

    pub fn run_example() {
        env_logger::init();
        #[cfg(not(target_family = "wasm"))]
        let app = rgpui_platform::application();
        #[cfg(target_family = "wasm")]
        let app = rgpui_platform::single_threaded_web();

        app.run(move |cx: &mut App| {
            #[cfg(not(target_family = "wasm"))]
            {
                let http_client = ReqwestClient::user_agent("gpui example").unwrap();
                cx.set_http_client(Arc::new(http_client));
            }
            #[cfg(target_family = "wasm")]
            {
                // Safety: the web examples run single-threaded; the client is
                // created and used exclusively on the main thread.
                let http_client = unsafe {
                    rgpui_web::FetchHttpClient::with_user_agent("gpui example")
                        .expect("创建 FetchHttpClient 失败")
                };
                cx.set_http_client(Arc::new(http_client));
            }

            cx.activate(true);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
            cx.set_menus([Menu::new("Image Gallery").items([MenuItem::action("Quit", Quit)])]);

            let window_options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(SharedString::from("Image Gallery")),
                    appears_transparent: false,
                    ..Default::default()
                }),

                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1100.), px(860.)),
                    cx,
                ))),

                ..Default::default()
            };

            cx.open_window(window_options, |_, cx| {
                cx.new(|ctx| ImageGallery {
                    image_key: "".into(),
                    items_count: IMAGES_IN_GALLERY,
                    total_count: 0,
                    image_cache: RetainAllImageCache::new(ctx),
                })
            })
            .unwrap();
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|ctx| ImageGallery {
            image_key: "".into(),
            items_count: IMAGES_IN_GALLERY,
            total_count: 0,
            image_cache: RetainAllImageCache::new(ctx),
        })
        .into()
    }
}

mod svg {

    //! SVG 矢量图示例：展示 SVG 加载与 `text_color` 着色。
    //! 用法：左侧 tab 选「svg」，查看矢量图缩放与着色效果。
    use std::fs;
    use std::path::PathBuf;

    use anyhow::Result;
    use rgpui::{
        App, AssetSource, Bounds, Context, SharedString, Window, WindowBounds, WindowOptions, div,
        prelude::*, px, rgb, size, svg,
    };
    use rgpui_platform::application;

    struct Assets {
        base: PathBuf,
    }

    impl AssetSource for Assets {
        fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
            fs::read(self.base.join(path))
                .map(|data| Some(std::borrow::Cow::Owned(data)))
                .map_err(|err| err.into())
        }

        fn list(&self, path: &str) -> Result<Vec<SharedString>> {
            fs::read_dir(self.base.join(path))
                .map(|entries| {
                    entries
                        .filter_map(|entry| {
                            entry
                                .ok()
                                .and_then(|entry| entry.file_name().into_string().ok())
                                .map(SharedString::from)
                        })
                        .collect()
                })
                .map_err(|err| err.into())
        }
    }

    struct SvgExample;

    impl Render for SvgExample {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_row()
                .size_full()
                .justify_center()
                .items_center()
                .gap_8()
                .bg(rgb(0xffffff))
                .child(
                    svg()
                        .path("assets/dragon.svg")
                        .size_8()
                        .text_color(rgb(0xff0000)),
                )
                .child(
                    svg()
                        .path("assets/dragon.svg")
                        .size_8()
                        .text_color(rgb(0x00ff00)),
                )
                .child(
                    svg()
                        .path("assets/dragon.svg")
                        .size_8()
                        .text_color(rgb(0x0000ff)),
                )
        }
    }

    pub fn run_example() {
        application()
            .with_assets(Assets {
                base: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            })
            .run(|cx: &mut App| {
                let bounds = Bounds::centered(None, size(px(300.0), px(300.0)), cx);
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        ..Default::default()
                    },
                    |_, cx| cx.new(|_| SvgExample),
                )
                .unwrap();
                cx.activate(true);
            });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| SvgExample).into()
    }
}

mod window {

    //! 多窗口示例：展示从主窗口打开子窗口与窗口间通信。
    //! 用法：左侧 tab 选「window」，点击按钮打开第二个窗口。
    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::{
        App, Bounds, Context, KeyBinding, PromptButton, PromptLevel, Window, WindowBounds,
        WindowKind, WindowOptions, actions, div, prelude::*, px, rgb, size,
    };
    use rgpui_platform::application;

    struct SubWindow {
        custom_titlebar: bool,
        is_dialog: bool,
    }

    fn button(text: &str, on_click: impl Fn(&mut Window, &mut App) + 'static) -> impl IntoElement {
        div()
            .id(text.to_string())
            .flex_none()
            .px_2()
            .bg(rgb(0xf7f7f7))
            .active(|this| this.opacity(0.85))
            .border_1()
            .border_color(rgb(0xe0e0e0))
            .rounded_sm()
            .cursor_pointer()
            .child(text.to_string())
            .on_click(move |_, window, cx| on_click(window, cx))
    }

    impl Render for SubWindow {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let window_bounds =
                WindowBounds::Windowed(Bounds::centered(None, size(px(250.0), px(200.0)), cx));

            div()
                .flex()
                .flex_col()
                .bg(rgb(0xffffff))
                .size_full()
                .gap_2()
                .when(self.custom_titlebar, |cx| {
                    cx.child(
                        div()
                            .flex()
                            .h(px(32.))
                            .px_4()
                            .bg(rgpui::blue())
                            .text_color(rgpui::white())
                            .w_full()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size_full()
                                    .child("自定义标题栏"),
                            ),
                    )
                })
                .child(
                    div()
                        .p_8()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child("SubWindow")
                        .when(self.is_dialog, |div| {
                            div.child(button("Open Nested Dialog", move |_, cx| {
                                cx.open_window(
                                    WindowOptions {
                                        window_bounds: Some(window_bounds),
                                        kind: WindowKind::Dialog,
                                        ..Default::default()
                                    },
                                    |_, cx| {
                                        cx.new(|_| SubWindow {
                                            custom_titlebar: false,
                                            is_dialog: true,
                                        })
                                    },
                                )
                                .unwrap();
                            }))
                        })
                        .child(button("关闭", |window, _| {
                            window.remove_window();
                        })),
                )
        }
    }

    struct WindowDemo {
        /// 原生标题栏当前是否可见，驱动按钮文案与下一次切换的目标值
        titlebar_visible: bool,
    }

    impl Render for WindowDemo {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let window_bounds =
                WindowBounds::Windowed(Bounds::centered(None, size(px(300.0), px(300.0)), cx));

            div()
                .p_4()
                .flex()
                .flex_wrap()
                .bg(rgb(0xffffff))
                .size_full()
                .justify_center()
                .content_center()
                .gap_2()
                .child(button("Normal", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            window_bounds: Some(window_bounds),
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: false,
                                is_dialog: false,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child(button("Popup", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            window_bounds: Some(window_bounds),
                            kind: WindowKind::PopUp,
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: false,
                                is_dialog: false,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child(button("Floating", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            window_bounds: Some(window_bounds),
                            kind: WindowKind::Floating,
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: false,
                                is_dialog: false,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child(button("Dialog", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            window_bounds: Some(window_bounds),
                            kind: WindowKind::Dialog,
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: false,
                                is_dialog: true,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child(button("自定义标题栏", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            titlebar: None,
                            window_bounds: Some(window_bounds),
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: true,
                                is_dialog: false,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child(button("Invisible", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            show: false,
                            window_bounds: Some(window_bounds),
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: false,
                                is_dialog: false,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child(button("Unmovable", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            is_movable: false,
                            titlebar: None,
                            window_bounds: Some(window_bounds),
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: false,
                                is_dialog: false,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child(button("Unresizable", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            is_resizable: false,
                            window_bounds: Some(window_bounds),
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: false,
                                is_dialog: false,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child(button("Unminimizable", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            is_minimizable: false,
                            window_bounds: Some(window_bounds),
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: false,
                                is_dialog: false,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child({
                    let entity = cx.entity();
                    // set_titlebar_visible 只在服务端装饰下有效：Wayland 无对应协议、
                    // 客户端装饰下服务端本就不画标题栏（由 TitleBar 元素绘制）。
                    let titlebar_toggle_supported =
                        matches!(window.window_decorations(), rgpui::Decorations::Server);
                    let label: &'static str =
                        match (self.titlebar_visible, titlebar_toggle_supported) {
                            (true, true) => "Hide Titlebar",
                            (false, true) => "Show Titlebar",
                            (true, false) => "Hide Titlebar (N/A)",
                            (false, false) => "Show Titlebar (N/A)",
                        };
                    button(label, move |window, app| {
                        if !titlebar_toggle_supported {
                            return;
                        }
                        let visible = entity.update(app, |demo, _| {
                            demo.titlebar_visible = !demo.titlebar_visible;
                            demo.titlebar_visible
                        });
                        window.set_titlebar_visible(visible);
                    })
                })
                .child(button("Hide Application", |window, cx| {
                    cx.hide();

                    // Restore the application after 3 seconds
                    window
                        .spawn(cx, async move |cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(3))
                                .await;
                            cx.update(|_, cx| {
                                cx.activate(false);
                            })
                        })
                        .detach();
                }))
                .child(button("Resize", |window, _| {
                    let content_size = window.bounds().size;
                    window.resize(size(content_size.height, content_size.width));
                }))
                .child(button("Prompt", |window, cx| {
                    let answer = window.prompt(
                        PromptLevel::Info,
                        "Are you sure?",
                        None,
                        &["OK", "取消"],
                        cx,
                    );

                    cx.spawn(async move |_| {
                        if answer.await.unwrap() == 0 {
                            println!("You have clicked Ok");
                        } else {
                            println!("You have clicked Cancel");
                        }
                    })
                    .detach();
                }))
                .child(button("Prompt (non-English)", |window, cx| {
                    let answer = window.prompt(
                        PromptLevel::Info,
                        "Are you sure?",
                        None,
                        &[PromptButton::ok("确定"), PromptButton::cancel("取消")],
                        cx,
                    );

                    cx.spawn(async move |_| {
                        if answer.await.unwrap() == 0 {
                            println!("You have clicked Ok");
                        } else {
                            println!("You have clicked Cancel");
                        }
                    })
                    .detach();
                }))
        }
    }

    actions!(window_window, [Quit]);

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        cx.observe_window_bounds(window, move |_, window, _| {
                            println!("Window bounds changed: {:?}", window.bounds());
                        })
                        .detach();

                        WindowDemo {
                            titlebar_visible: true,
                        }
                    })
                },
            )
            .unwrap();

            cx.activate(true);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| SubWindow {
            custom_titlebar: false,
            is_dialog: true,
        })
        .into()
    }
}

mod opacity {

    //! 窗口不透明度示例：展示 `WindowBackgroundAppearance` 与透明度设置。
    //! 用法：左侧 tab 选「opacity」，观察窗口背景的半透明效果。
    use std::{fs, path::PathBuf};

    use anyhow::Result;
    use rgpui::{
        App, AssetSource, Bounds, BoxShadow, ClickEvent, Context, SharedString, Task, Window,
        WindowBounds, WindowOptions, div, hsla, img, point, prelude::*, px, rgb, size, svg,
    };
    use rgpui_platform::application;

    struct Assets {
        base: PathBuf,
    }

    impl AssetSource for Assets {
        fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
            fs::read(self.base.join(path))
                .map(|data| Some(std::borrow::Cow::Owned(data)))
                .map_err(|e| e.into())
        }

        fn list(&self, path: &str) -> Result<Vec<SharedString>> {
            fs::read_dir(self.base.join(path))
                .map(|entries| {
                    entries
                        .filter_map(|entry| {
                            entry
                                .ok()
                                .and_then(|entry| entry.file_name().into_string().ok())
                                .map(SharedString::from)
                        })
                        .collect()
                })
                .map_err(|e| e.into())
        }
    }

    struct HelloWorld {
        _task: Option<Task<()>>,
        opacity: f32,
        animating: bool,
    }

    impl HelloWorld {
        fn new(_window: &mut Window, _: &mut Context<Self>) -> Self {
            Self {
                _task: None,
                opacity: 0.5,
                animating: false,
            }
        }

        fn start_animation(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
            self.opacity = 0.0;
            self.animating = true;
            cx.notify();
        }
    }

    impl Render for HelloWorld {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            if self.animating {
                self.opacity += 0.005;
                if self.opacity >= 1.0 {
                    self.animating = false;
                    self.opacity = 1.0;
                } else {
                    window.request_animation_frame();
                }
            }

            div()
                .flex()
                .flex_row()
                .size_full()
                .bg(rgb(0xe0e0e0))
                .text_xl()
                .child(
                    div()
                        .flex()
                        .size_full()
                        .justify_center()
                        .items_center()
                        .border_1()
                        .text_color(rgpui::blue())
                        .child(div().child("This is background text.")),
                )
                .child(
                    div()
                        .id("panel")
                        .on_click(cx.listener(Self::start_animation))
                        .absolute()
                        .top_8()
                        .left_8()
                        .right_8()
                        .bottom_8()
                        .opacity(self.opacity)
                        .flex()
                        .justify_center()
                        .items_center()
                        .bg(rgpui::white())
                        .border_3()
                        .border_color(rgpui::red())
                        .text_color(rgpui::yellow())
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .justify_center()
                                .items_center()
                                .size(px(300.))
                                .bg(rgpui::blue())
                                .border_3()
                                .border_color(rgpui::black())
                                .shadow(vec![BoxShadow {
                                    color: hsla(0.0, 0.0, 0.0, 0.5),
                                    blur_radius: px(1.0),
                                    spread_radius: px(5.0),
                                    offset: point(px(10.0), px(10.0)),
                                    inset: false,
                                }])
                                .child(img("image/app-icon.png").size_8())
                                .child("Opacity Panel (Click to test)")
                                .child(
                                    div()
                                        .id("deep-level-text")
                                        .flex()
                                        .justify_center()
                                        .items_center()
                                        .p_4()
                                        .bg(rgpui::black())
                                        .text_color(rgpui::white())
                                        .text_decoration_2()
                                        .text_decoration_wavy()
                                        .text_decoration_color(rgpui::red())
                                        .child(format!("opacity: {:.1}", self.opacity)),
                                )
                                .child(
                                    svg()
                                        .path("assets/arrow_circle.svg")
                                        .text_color(rgpui::black())
                                        .text_2xl()
                                        .size_8(),
                                )
                                .child(div().flex().children(
                                    ["🎊", "✈️", "🎉", "🎈", "🎁", "🎂"].map(|emoji| {
                                        div()
                                            .child(emoji.to_string())
                                            .hover(|style| style.opacity(0.5))
                                    }),
                                ))
                                .child(img("image/black-cat-typing.gif").size_12()),
                        ),
                )
        }
    }

    pub fn run_example() {
        application()
            .with_assets(Assets {
                base: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples"),
            })
            .run(|cx: &mut App| {
                let bounds = Bounds::centered(None, size(px(500.0), px(500.0)), cx);
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        ..Default::default()
                    },
                    |window, cx| cx.new(|cx| HelloWorld::new(window, cx)),
                )
                .unwrap();
                cx.activate(true);
            });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| HelloWorld::new(window, cx)).into()
    }
}

mod shadow {

    //! 窗口阴影示例：展示窗口阴影配置与效果。
    //! 用法：左侧 tab 选「shadow」，观察窗口边缘阴影。
    use rgpui::{
        App, Bounds, BoxShadow, Context, Div, SharedString, Window, WindowBounds, WindowOptions,
        div, hsla, point, prelude::*, px, relative, rgb, size,
    };
    use rgpui_platform::application;

    struct Shadow {}

    impl Shadow {
        fn base() -> Div {
            div()
                .size_16()
                .bg(rgb(0xffffff))
                .rounded_full()
                .border_1()
                .border_color(hsla(0.0, 0.0, 0.0, 0.1))
        }

        fn square() -> Div {
            div()
                .size_16()
                .bg(rgb(0xffffff))
                .border_1()
                .border_color(hsla(0.0, 0.0, 0.0, 0.1))
        }

        fn rounded_small() -> Div {
            div()
                .size_16()
                .bg(rgb(0xffffff))
                .rounded(px(4.))
                .border_1()
                .border_color(hsla(0.0, 0.0, 0.0, 0.1))
        }

        fn rounded_medium() -> Div {
            div()
                .size_16()
                .bg(rgb(0xffffff))
                .rounded(px(8.))
                .border_1()
                .border_color(hsla(0.0, 0.0, 0.0, 0.1))
        }

        fn rounded_large() -> Div {
            div()
                .size_16()
                .bg(rgb(0xffffff))
                .rounded(px(12.))
                .border_1()
                .border_color(hsla(0.0, 0.0, 0.0, 0.1))
        }
    }

    fn example(label: impl Into<SharedString>, example: impl IntoElement) -> impl IntoElement {
        let label = label.into();

        div()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .w(relative(1. / 6.))
            .border_r_1()
            .border_color(hsla(0.0, 0.0, 0.0, 1.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_1()
                    .py_12()
                    .child(example),
            )
            .child(
                div()
                    .w_full()
                    .border_t_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .p_1()
                    .flex()
                    .items_center()
                    .child(label),
            )
    }

    impl Render for Shadow {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .id("shadow-example")
                .overflow_y_scroll()
                .bg(rgb(0xffffff))
                .size_full()
                .text_xs()
                .child(div().flex().flex_col().w_full().children(vec![
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .flex_row()
                    .children(vec![
                        example(
                            "Square",
                            Shadow::square()
                                .shadow(vec![BoxShadow {
                                    color: hsla(0.0, 0.5, 0.5, 0.3),
                                    offset: point(px(0.), px(8.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(0.),
                                    inset: false,
                                }]),
                        ),
                        example(
                            "Rounded 4",
                            Shadow::rounded_small()
                                .shadow(vec![BoxShadow {
                                    color: hsla(0.0, 0.5, 0.5, 0.3),
                                    offset: point(px(0.), px(8.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(0.),
                                    inset: false,
                                }]),
                        ),
                        example(
                            "Rounded 8",
                            Shadow::rounded_medium()
                                .shadow(vec![BoxShadow {
                                    color: hsla(0.0, 0.5, 0.5, 0.3),
                                    offset: point(px(0.), px(8.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(0.),
                                    inset: false,
                                }]),
                        ),
                        example(
                            "Rounded 16",
                            Shadow::rounded_large()
                                .shadow(vec![BoxShadow {
                                    color: hsla(0.0, 0.5, 0.5, 0.3),
                                    offset: point(px(0.), px(8.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(0.),
                                    inset: false,
                                }]),
                        ),
                        example(
                            "Circle",
                            Shadow::base()
                                .shadow(vec![BoxShadow {
                                    color: hsla(0.0, 0.5, 0.5, 0.3),
                                    offset: point(px(0.), px(8.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(0.),
                                    inset: false,
                                }]),
                        ),
                    ]),
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .w_full()
                    .children(vec![
                        example("None", Shadow::base()),
                        // 2Xsmall shadow
                        example("2X Small", Shadow::base().shadow_2xs()),
                        // Xsmall shadow
                        example("超小", Shadow::base().shadow_xs()),
                        // Small shadow
                        example("Small", Shadow::base().shadow_sm()),
                        // Medium shadow
                        example("Medium", Shadow::base().shadow_md()),
                        // Large shadow
                        example("Large", Shadow::base().shadow_lg()),
                        example("超大", Shadow::base().shadow_xl()),
                        example("2X Large", Shadow::base().shadow_2xl()),
                    ]),
                // Horizontal list of increasing blur radii
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .children(vec![
                        example(
                            "Blur 0",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(0.),
                                    spread_radius: px(0.),
                                    inset: false,
                                }]),
                        ),
                        example(
                            "Blur 0",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(0.),
                                spread_radius: px(0.),
                                inset: false,
                            }]),
                        ),
                        example(
                            "Blur 2",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(2.),
                                spread_radius: px(0.),
                                inset: false,
                            }]),
                        ),
                        example(
                            "Blur 4",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(4.),
                                spread_radius: px(0.),
                                inset: false,
                            }]),
                        ),
                        example(
                            "Blur 8",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                                inset: false,
                            }]),
                        ),
                        example(
                            "Blur 16",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(16.),
                                spread_radius: px(0.),
                                inset: false,
                            }]),
                        ),
                        example(
                            "Blur 4",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(4.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Blur 8",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Blur 16",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(16.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                    ]),
                // Horizontal list of increasing spread radii
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .children(vec![
                        example(
                            "Spread 0",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Spread 2",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(2.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Spread 4",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(4.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Spread 8",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(8.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Spread 16",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(16.),
                            inset: false,
                            }]),
                        ),
                    ]),
                // Square spread examples
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .children(vec![
                        example(
                            "Square Spread 0",
                            Shadow::square().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Square Spread 8",
                            Shadow::square().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(8.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Square Spread 16",
                            Shadow::square().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(16.),
                            inset: false,
                            }]),
                        ),
                    ]),
                // Rounded large spread examples
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .children(vec![
                        example(
                            "Rounded Large Spread 0",
                            Shadow::rounded_large().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Rounded Large Spread 8",
                            Shadow::rounded_large().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(8.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Rounded Large Spread 16",
                            Shadow::rounded_large().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(16.),
                            inset: false,
                            }]),
                        ),
                    ]),
                // Directional shadows
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .children(vec![
                        example(
                            "Left",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(-8.), px(0.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Right",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(8.), px(0.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Top",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(0.), px(-8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Bottom",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                    ]),
                // Square directional shadows
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .children(vec![
                        example(
                            "Square Left",
                            Shadow::square().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(-8.), px(0.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Square Right",
                            Shadow::square().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(8.), px(0.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Square Top",
                            Shadow::square().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(0.), px(-8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Square Bottom",
                            Shadow::square().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                    ]),
                // Rounded large directional shadows
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .children(vec![
                        example(
                            "Rounded Large Left",
                            Shadow::rounded_large().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(-8.), px(0.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Rounded Large Right",
                            Shadow::rounded_large().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(8.), px(0.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Rounded Large Top",
                            Shadow::rounded_large().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(0.), px(-8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Rounded Large Bottom",
                            Shadow::rounded_large().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                    ]),
                // Multiple shadows for different shapes
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .children(vec![
                        example(
                            "多圆",
                            Shadow::base().shadow(vec![
                                BoxShadow {
                                    color: hsla(0.0 / 360., 1.0, 0.5, 0.3), // Red
                                    offset: point(px(0.), px(-12.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(60.0 / 360., 1.0, 0.5, 0.3), // Yellow
                                    offset: point(px(12.), px(0.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(120.0 / 360., 1.0, 0.5, 0.3), // Green
                                    offset: point(px(0.), px(12.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(240.0 / 360., 1.0, 0.5, 0.3), // Blue
                                    offset: point(px(-12.), px(0.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                            ]),
                        ),
                        example(
                            "Square Multiple",
                            Shadow::square().shadow(vec![
                                BoxShadow {
                                    color: hsla(0.0 / 360., 1.0, 0.5, 0.3), // Red
                                    offset: point(px(0.), px(-12.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(60.0 / 360., 1.0, 0.5, 0.3), // Yellow
                                    offset: point(px(12.), px(0.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(120.0 / 360., 1.0, 0.5, 0.3), // Green
                                    offset: point(px(0.), px(12.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(240.0 / 360., 1.0, 0.5, 0.3), // Blue
                                    offset: point(px(-12.), px(0.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                            ]),
                        ),
                        example(
                            "Rounded Large Multiple",
                            Shadow::rounded_large().shadow(vec![
                                BoxShadow {
                                    color: hsla(0.0 / 360., 1.0, 0.5, 0.3), // Red
                                    offset: point(px(0.), px(-12.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(60.0 / 360., 1.0, 0.5, 0.3), // Yellow
                                    offset: point(px(12.), px(0.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(120.0 / 360., 1.0, 0.5, 0.3), // Green
                                    offset: point(px(0.), px(12.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(240.0 / 360., 1.0, 0.5, 0.3), // Blue
                                    offset: point(px(-12.), px(0.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                            ]),
                        ),
                    ]),
            ]))
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1000.0), px(800.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Shadow {}),
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| Shadow {}).into()
    }
}

mod transparent {

    //! 透明窗口示例：展示透明背景窗口的构造。
    //! 用法：左侧 tab 选「transparent」，透过窗口看到下层内容。
    //！ 背景透明

    use rgpui::{
        App, Bounds, Context, SharedString, Window, WindowBackgroundAppearance, WindowBounds,
        WindowDecorations, WindowOptions, div, prelude::*, px, rgb, size,
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
                .size(px(500.0))
                .justify_center()
                .items_center()
                .text_xl()
                .text_color(rgb(0x505050))
                .child(
                    div()
                        .child(format!("Hello, {}!", self.text))
                        .bg(rgb(0xff00ff)),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    // 背景透明
                    window_background: WindowBackgroundAppearance::Transparent,
                    // 去掉标题栏和边框
                    window_decorations: Some(WindowDecorations::Client),

                    titlebar: None,
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|_| HelloWorld {
                        text: "World".into(),
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
            text: "World".into(),
        })
        .into()
    }
}

mod window_shadow {

    //! 窗口阴影进阶示例：对比不同阴影参数的效果。
    //! 用法：左侧 tab 选「window_shadow」，观察阴影差异。
    use rgpui::{
        App, Bounds, Context, CursorStyle, Decorations, HitboxBehavior, Hsla, MouseButton, Pixels,
        Point, ResizeEdge, Size, Window, WindowBackgroundAppearance, WindowBounds,
        WindowDecorations, WindowOptions, black, canvas, div, green, point, prelude::*, px, rgb,
        size, transparent_black, white,
    };
    use rgpui_platform::application;

    struct WindowShadow {}

    // Things to do:
    // 1. We need a way of calculating which edge or corner the mouse is on,
    //    and then dispatch on that
    // 2. We need to improve the shadow rendering significantly
    // 3. We need to implement the techniques in here in Zed

    impl Render for WindowShadow {
        fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let decorations = window.window_decorations();
            let rounding = px(10.0);
            let shadow_size = px(10.0);
            let border_size = px(1.0);
            let grey = rgb(0x808080);
            window.set_client_inset(shadow_size);

            div()
                .id("window-backdrop")
                .bg(transparent_black())
                .map(|div| match decorations {
                    Decorations::Server => div,
                    Decorations::Client { tiling, .. } => div
                        .bg(rgpui::transparent_black())
                        .child(
                            canvas(
                                |_bounds, window, _cx| {
                                    window.insert_hitbox(
                                        Bounds::new(
                                            point(px(0.0), px(0.0)),
                                            window.window_bounds().get_bounds().size,
                                        ),
                                        HitboxBehavior::Normal,
                                    )
                                },
                                move |_bounds, hitbox, window, _cx| {
                                    let mouse = window.mouse_position();
                                    let size = window.window_bounds().get_bounds().size;
                                    let Some(edge) = resize_edge(mouse, shadow_size, size) else {
                                        return;
                                    };
                                    window.set_cursor_style(
                                        match edge {
                                            ResizeEdge::Top | ResizeEdge::Bottom => {
                                                CursorStyle::ResizeUpDown
                                            }
                                            ResizeEdge::Left | ResizeEdge::Right => {
                                                CursorStyle::ResizeLeftRight
                                            }
                                            ResizeEdge::TopLeft | ResizeEdge::BottomRight => {
                                                CursorStyle::ResizeUpLeftDownRight
                                            }
                                            ResizeEdge::TopRight | ResizeEdge::BottomLeft => {
                                                CursorStyle::ResizeUpRightDownLeft
                                            }
                                        },
                                        &hitbox,
                                    );
                                },
                            )
                            .size_full()
                            .absolute(),
                        )
                        .when(!(tiling.top || tiling.right), |div| {
                            div.rounded_tr(rounding)
                        })
                        .when(!(tiling.top || tiling.left), |div| div.rounded_tl(rounding))
                        .when(!tiling.top, |div| div.pt(shadow_size))
                        .when(!tiling.bottom, |div| div.pb(shadow_size))
                        .when(!tiling.left, |div| div.pl(shadow_size))
                        .when(!tiling.right, |div| div.pr(shadow_size))
                        .on_mouse_move(|_e, window, _cx| window.refresh())
                        .on_mouse_down(MouseButton::Left, move |e, window, _cx| {
                            let size = window.window_bounds().get_bounds().size;
                            let pos = e.position;

                            match resize_edge(pos, shadow_size, size) {
                                Some(edge) => window.start_window_resize(edge),
                                None => window.start_window_move(),
                            };
                        }),
                })
                .size_full()
                .child(
                    div()
                        .cursor(CursorStyle::Arrow)
                        .map(|div| match decorations {
                            Decorations::Server => div,
                            Decorations::Client { tiling } => div
                                .border_color(grey)
                                .when(!(tiling.top || tiling.right), |div| {
                                    div.rounded_tr(rounding)
                                })
                                .when(!(tiling.top || tiling.left), |div| div.rounded_tl(rounding))
                                .when(!tiling.top, |div| div.border_t(border_size))
                                .when(!tiling.bottom, |div| div.border_b(border_size))
                                .when(!tiling.left, |div| div.border_l(border_size))
                                .when(!tiling.right, |div| div.border_r(border_size))
                                .when(!tiling.is_tiled(), |div| {
                                    div.shadow(vec![rgpui::BoxShadow {
                                        color: Hsla {
                                            h: 0.,
                                            s: 0.,
                                            l: 0.,
                                            a: 0.4,
                                        },
                                        blur_radius: shadow_size / 2.,
                                        spread_radius: px(0.),
                                        inset: false,
                                        offset: point(px(0.0), px(0.0)),
                                    }])
                                }),
                        })
                        .on_mouse_move(|_e, _, cx| {
                            cx.stop_propagation();
                        })
                        .bg(rgpui::rgb(0xCCCCFF))
                        .size_full()
                        .flex()
                        .flex_col()
                        .justify_around()
                        .child(
                            div().w_full().flex().flex_row().justify_around().child(
                                div()
                                    .flex()
                                    .bg(white())
                                    .size(px(300.0))
                                    .justify_center()
                                    .items_center()
                                    .shadow_lg()
                                    .border_1()
                                    .border_color(rgb(0x0000ff))
                                    .text_xl()
                                    .text_color(rgb(0xffffff))
                                    .child(
                                        div()
                                            .id("hello")
                                            .w(px(200.0))
                                            .h(px(100.0))
                                            .bg(green())
                                            .shadow(vec![rgpui::BoxShadow {
                                                color: Hsla {
                                                    h: 0.,
                                                    s: 0.,
                                                    l: 0.,
                                                    a: 1.0,
                                                },
                                                blur_radius: px(20.0),
                                                spread_radius: px(0.0),
                                                inset: false,
                                                offset: point(px(0.0), px(0.0)),
                                            }])
                                            .map(|div| match decorations {
                                                Decorations::Server => div,
                                                Decorations::Client { .. } => div
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        |_e, window, _| {
                                                            window.start_window_move();
                                                        },
                                                    )
                                                    .on_click(|e, window, _| {
                                                        if e.is_right_click() {
                                                            window.show_window_menu(e.position());
                                                        }
                                                    })
                                                    .text_color(black())
                                                    .child("this is the custom titlebar"),
                                            }),
                                    ),
                            ),
                        ),
                )
        }
    }

    fn resize_edge(
        pos: Point<Pixels>,
        shadow_size: Pixels,
        size: Size<Pixels>,
    ) -> Option<ResizeEdge> {
        let edge = if pos.y < shadow_size && pos.x < shadow_size {
            ResizeEdge::TopLeft
        } else if pos.y < shadow_size && pos.x > size.width - shadow_size {
            ResizeEdge::TopRight
        } else if pos.y < shadow_size {
            ResizeEdge::Top
        } else if pos.y > size.height - shadow_size && pos.x < shadow_size {
            ResizeEdge::BottomLeft
        } else if pos.y > size.height - shadow_size && pos.x > size.width - shadow_size {
            ResizeEdge::BottomRight
        } else if pos.y > size.height - shadow_size {
            ResizeEdge::Bottom
        } else if pos.x < shadow_size {
            ResizeEdge::Left
        } else if pos.x > size.width - shadow_size {
            ResizeEdge::Right
        } else {
            return None;
        };
        Some(edge)
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(600.0), px(600.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_background: WindowBackgroundAppearance::Opaque,
                    window_decorations: Some(WindowDecorations::Client),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        cx.observe_window_appearance(window, |_, window, _| {
                            window.refresh();
                        })
                        .detach();
                        WindowShadow {}
                    })
                },
            )
            .unwrap();
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| {
            cx.observe_window_appearance(window, |_, window, _| {
                window.refresh();
            })
            .detach();
            WindowShadow {}
        })
        .into()
    }
}

mod list_example {

    //! 虚拟列表示例：展示 `List` + `ScrollHandle` 的海量数据渲染。
    //! 用法：左侧 tab 选「list_example」，滚动列表观察虚拟化渲染。
    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::{
        App, Bounds, Context, ListAlignment, ListState, Render, Window, WindowBounds,
        WindowOptions, div, list, prelude::*, px, rgb, size,
    };
    use rgpui_platform::application;

    const ITEM_COUNT: usize = 40;
    const SCROLLBAR_WIDTH: f32 = 12.;

    struct BottomListDemo {
        list_state: ListState,
    }

    impl BottomListDemo {
        fn new() -> Self {
            Self {
                list_state: ListState::new(ITEM_COUNT, ListAlignment::Bottom, px(500.))
                    .measure_all(),
            }
        }
    }

    impl Render for BottomListDemo {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let max_offset = self.list_state.max_offset_for_scrollbar().y;
            let current_offset = -self.list_state.scroll_px_offset_for_scrollbar().y;

            let viewport_height = self.list_state.viewport_bounds().size.height;

            let raw_fraction = if max_offset > px(0.) {
                current_offset / max_offset
            } else {
                0.
            };

            let total_height = viewport_height + max_offset;
            let thumb_height = if total_height > px(0.) {
                px(viewport_height.as_f32() * viewport_height.as_f32() / total_height.as_f32())
                    .max(px(30.))
            } else {
                px(30.)
            };

            let track_space = viewport_height - thumb_height;
            let thumb_top = track_space * raw_fraction;

            let bug_detected = raw_fraction > 1.0;

            div()
                .size_full()
                .bg(rgb(0xFFFFFF))
                .flex()
                .flex_col()
                .p_4()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(format!(
                            "offset: {:.0} / max: {:.0} | fraction: {:.3}",
                            current_offset.as_f32(),
                            max_offset.as_f32(),
                            raw_fraction,
                        ))
                        .child(
                            div()
                                .text_color(if bug_detected {
                                    rgb(0xCC0000)
                                } else {
                                    rgb(0x008800)
                                })
                                .child(if bug_detected {
                                    format!(
                                        "BUG: fraction is {:.3} (> 1.0) — thumb is off-track!",
                                        raw_fraction
                                    )
                                } else {
                                    "OK: fraction <= 1.0 — thumb is within track.".to_string()
                                }),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_row()
                        .overflow_hidden()
                        .border_1()
                        .border_color(rgb(0xCCCCCC))
                        .rounded_sm()
                        .child(
                            list(self.list_state.clone(), |index, _window, _cx| {
                                let height = px(30. + (index % 5) as f32 * 10.);
                                div()
                                    .h(height)
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .px_3()
                                    .border_b_1()
                                    .border_color(rgb(0xEEEEEE))
                                    .bg(if index % 2 == 0 {
                                        rgb(0xFAFAFA)
                                    } else {
                                        rgb(0xFFFFFF)
                                    })
                                    .text_sm()
                                    .child(format!("Item {index}"))
                                    .into_any()
                            })
                            .flex_1(),
                        )
                        // Scrollbar track
                        .child(
                            div()
                                .w(px(SCROLLBAR_WIDTH))
                                .h_full()
                                .flex_shrink_0()
                                .bg(rgb(0xE0E0E0))
                                .relative()
                                .child(
                                    // Thumb — position is unclamped to expose the bug
                                    div()
                                        .absolute()
                                        .top(thumb_top)
                                        .w_full()
                                        .h(thumb_height)
                                        .bg(if bug_detected {
                                            rgb(0xCC0000)
                                        } else {
                                            rgb(0x888888)
                                        })
                                        .rounded_sm(),
                                ),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(400.), px(500.)), cx);
            cx.open_window(
                WindowOptions {
                    focus: true,
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| BottomListDemo::new()),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| BottomListDemo::new()).into()
    }
}

mod uniform_list {

    //! 等高虚拟列表示例：展示 `uniform_list` 的懒加载。
    //! 用法：左侧 tab 选「uniform_list」，滚动查看万行级列表。
    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::{
        App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
        uniform_list,
    };
    use rgpui_platform::application;

    struct UniformListExample {}

    impl Render for UniformListExample {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().bg(rgb(0xffffff)).child(
                uniform_list(
                    "entries",
                    50,
                    cx.processor(|_this, range, _window, _cx| {
                        let mut items = Vec::new();
                        for ix in range {
                            let item = ix + 1;

                            items.push(
                                div()
                                    .id(ix)
                                    .px_2()
                                    .cursor_pointer()
                                    .on_click(move |_event, _window, _cx| {
                                        println!("clicked Item {item:?}");
                                    })
                                    .child(format!("Item {item}")),
                            );
                        }
                        items
                    }),
                )
                .h_full(),
            )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(300.0), px(300.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| UniformListExample {}),
            )
            .unwrap();
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| UniformListExample {}).into()
    }
}

mod data_table {

    //! 数据表格示例：展示 `DataTable` 的列定义、排序与选中。
    //! 用法：左侧 tab 选「data_table」，点击表头排序、点击行选中。
    #![cfg_attr(target_family = "wasm", no_main)]

    use std::{ops::Range, rc::Rc, time::Duration};

    use rgpui::{
        App, Bounds, Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Render,
        SharedString, UniformListScrollHandle, Window, WindowBounds, WindowOptions, canvas, div,
        point, prelude::*, px, rgb, size, uniform_list,
    };
    use rgpui_platform::application;

    const TOTAL_ITEMS: usize = 10000;
    const SCROLLBAR_THUMB_WIDTH: Pixels = px(8.);
    const SCROLLBAR_THUMB_HEIGHT: Pixels = px(100.);

    pub struct Quote {
        name: SharedString,
        symbol: SharedString,
        last_done: f64,
        prev_close: f64,
        open: f64,
        high: f64,
        low: f64,
        timestamp: Duration,
        volume: i64,
        turnover: f64,
        ttm: f64,
        market_cap: f64,
        float_cap: f64,
        shares: f64,
        pb: f64,
        pe: f64,
        eps: f64,
        dividend: f64,
        dividend_yield: f64,
        dividend_per_share: f64,
        dividend_date: SharedString,
        dividend_payment: f64,
    }

    impl Quote {
        pub fn random() -> Self {
            use rand::RngExt;
            let mut rng = rand::rng();
            // simulate a base price in a realistic range
            let prev_close = rng.random_range(100.0..200.0);
            let change = rng.random_range(-5.0..5.0);
            let last_done = prev_close + change;
            let open = prev_close + rng.random_range(-3.0..3.0);
            let high = (prev_close + rng.random_range::<f64, _>(0.0..10.0)).max(open);
            let low = (prev_close - rng.random_range::<f64, _>(0.0..10.0)).min(open);
            let timestamp = Duration::from_secs(rng.random_range(0..86400));
            let volume = rng.random_range(1_000_000..100_000_000);
            let turnover = last_done * volume as f64;
            let symbol = {
                let mut ticker = String::new();
                if rng.random_bool(0.5) {
                    ticker.push_str(&format!(
                        "{:03}.{}",
                        rng.random_range(100..1000),
                        rng.random_range(0..10)
                    ));
                } else {
                    ticker.push_str(&format!(
                        "{}{}",
                        rng.random_range('A'..='Z'),
                        rng.random_range('A'..='Z')
                    ));
                }
                ticker.push_str(&format!(".{}", rng.random_range('A'..='Z')));
                ticker
            };
            let name = format!(
                "{} {} - #{}",
                symbol,
                rng.random_range(1..100),
                rng.random_range(10000..100000)
            );
            let ttm = rng.random_range(0.0..10.0);
            let market_cap = rng.random_range(1_000_000.0..10_000_000.0);
            let float_cap = market_cap + rng.random_range(1_000.0..10_000.0);
            let shares = rng.random_range(100.0..1000.0);
            let pb = market_cap / shares;
            let pe = market_cap / shares;
            let eps = market_cap / shares;
            let dividend = rng.random_range(0.0..10.0);
            let dividend_yield = rng.random_range(0.0..10.0);
            let dividend_per_share = rng.random_range(0.0..10.0);
            let dividend_date = SharedString::new(format!(
                "{}-{}-{}",
                rng.random_range(2000..2023),
                rng.random_range(1..12),
                rng.random_range(1..28)
            ));
            let dividend_payment = rng.random_range(0.0..10.0);

            Self {
                name: name.into(),
                symbol: symbol.into(),
                last_done,
                prev_close,
                open,
                high,
                low,
                timestamp,
                volume,
                turnover,
                pb,
                pe,
                eps,
                ttm,
                market_cap,
                float_cap,
                shares,
                dividend,
                dividend_yield,
                dividend_per_share,
                dividend_date,
                dividend_payment,
            }
        }

        fn change(&self) -> f64 {
            (self.last_done - self.prev_close) / self.prev_close * 100.0
        }

        fn change_color(&self) -> rgpui::Hsla {
            if self.change() > 0.0 {
                rgpui::green()
            } else {
                rgpui::red()
            }
        }

        fn turnover_ratio(&self) -> f64 {
            self.volume as f64 / self.turnover * 100.0
        }
    }

    #[derive(IntoElement)]
    struct TableRow {
        ix: usize,
        quote: Rc<Quote>,
    }
    impl TableRow {
        fn new(ix: usize, quote: Rc<Quote>) -> Self {
            Self { ix, quote }
        }

        fn render_cell(&self, key: &str, width: Pixels, color: rgpui::Hsla) -> impl IntoElement {
            div()
                .whitespace_nowrap()
                .truncate()
                .w(width)
                .px_1()
                .child(match key {
                    "id" => div().child(format!("{}", self.ix)),
                    "symbol" => div().child(self.quote.symbol.clone()),
                    "name" => div().child(self.quote.name.clone()),
                    "last_done" => div()
                        .text_color(color)
                        .child(format!("{:.3}", self.quote.last_done)),
                    "prev_close" => div()
                        .text_color(color)
                        .child(format!("{:.3}", self.quote.prev_close)),
                    "change" => div()
                        .text_color(color)
                        .child(format!("{:.2}%", self.quote.change())),
                    "timestamp" => div()
                        .text_color(color)
                        .child(format!("{:?}", self.quote.timestamp.as_secs())),
                    "open" => div()
                        .text_color(color)
                        .child(format!("{:.2}", self.quote.open)),
                    "low" => div()
                        .text_color(color)
                        .child(format!("{:.2}", self.quote.low)),
                    "high" => div()
                        .text_color(color)
                        .child(format!("{:.2}", self.quote.high)),
                    "ttm" => div()
                        .text_color(color)
                        .child(format!("{:.2}", self.quote.ttm)),
                    "eps" => div()
                        .text_color(color)
                        .child(format!("{:.2}", self.quote.eps)),
                    "market_cap" => {
                        div().child(format!("{:.2} M", self.quote.market_cap / 1_000_000.0))
                    }
                    "float_cap" => {
                        div().child(format!("{:.2} M", self.quote.float_cap / 1_000_000.0))
                    }
                    "turnover" => {
                        div().child(format!("{:.2} M", self.quote.turnover / 1_000_000.0))
                    }
                    "volume" => {
                        div().child(format!("{:.2} M", self.quote.volume as f64 / 1_000_000.0))
                    }
                    "turnover_ratio" => div().child(format!("{:.2}%", self.quote.turnover_ratio())),
                    "pe" => div().child(format!("{:.2}", self.quote.pe)),
                    "pb" => div().child(format!("{:.2}", self.quote.pb)),
                    "shares" => div().child(format!("{:.2}", self.quote.shares)),
                    "dividend" => div().child(format!("{:.2}", self.quote.dividend)),
                    "yield" => div().child(format!("{:.2}%", self.quote.dividend_yield)),
                    "dividend_per_share" => {
                        div().child(format!("{:.2}", self.quote.dividend_per_share))
                    }
                    "dividend_date" => div().child(format!("{}", self.quote.dividend_date)),
                    "dividend_payment" => {
                        div().child(format!("{:.2}", self.quote.dividend_payment))
                    }
                    _ => div().child("--"),
                })
        }
    }

    const FIELDS: [(&str, f32); 24] = [
        ("id", 64.),
        ("symbol", 64.),
        ("name", 180.),
        ("last_done", 80.),
        ("prev_close", 80.),
        ("open", 80.),
        ("low", 80.),
        ("high", 80.),
        ("ttm", 50.),
        ("market_cap", 96.),
        ("float_cap", 96.),
        ("turnover", 120.),
        ("volume", 100.),
        ("turnover_ratio", 96.),
        ("pe", 64.),
        ("pb", 64.),
        ("eps", 64.),
        ("shares", 96.),
        ("dividend", 64.),
        ("yield", 64.),
        ("dividend_per_share", 64.),
        ("dividend_date", 96.),
        ("dividend_payment", 64.),
        ("timestamp", 120.),
    ];

    impl RenderOnce for TableRow {
        fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
            let color = self.quote.change_color();
            div()
                .flex()
                .flex_row()
                .border_b_1()
                .border_color(rgb(0xE0E0E0))
                .bg(if self.ix.is_multiple_of(2) {
                    rgb(0xFFFFFF)
                } else {
                    rgb(0xFAFAFA)
                })
                .py_0p5()
                .px_2()
                .children(FIELDS.map(|(key, width)| self.render_cell(key, px(width), color)))
        }
    }

    struct DataTable {
        /// Use `Rc` to share the same quote data across multiple items, avoid cloning.
        quotes: Vec<Rc<Quote>>,
        visible_range: Range<usize>,
        scroll_handle: UniformListScrollHandle,
        /// The position in thumb bounds when dragging start mouse down.
        drag_position: Option<Point<Pixels>>,
    }

    impl DataTable {
        fn new() -> Self {
            Self {
                quotes: Vec::new(),
                visible_range: 0..0,
                scroll_handle: UniformListScrollHandle::new(),
                drag_position: None,
            }
        }

        fn generate(&mut self) {
            self.quotes = (0..TOTAL_ITEMS).map(|_| Rc::new(Quote::random())).collect();
        }

        fn table_bounds(&self) -> Bounds<Pixels> {
            self.scroll_handle.0.borrow().base_handle.bounds()
        }

        fn scroll_top(&self) -> Pixels {
            self.scroll_handle.0.borrow().base_handle.offset().y
        }

        fn scroll_height(&self) -> Pixels {
            self.scroll_handle
                .0
                .borrow()
                .last_item_size
                .unwrap_or_default()
                .contents
                .height
        }

        fn render_scrollbar(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let scroll_height = self.scroll_height();
            let table_bounds = self.table_bounds();
            let table_height = table_bounds.size.height;
            if table_height == px(0.) {
                return div().id("scrollbar");
            }

            let percentage = -self.scroll_top() / scroll_height;
            let offset_top = (table_height * percentage).clamp(
                px(4.),
                (table_height - SCROLLBAR_THUMB_HEIGHT - px(4.)).max(px(4.)),
            );
            let entity = cx.entity();
            let scroll_handle = self.scroll_handle.0.borrow().base_handle.clone();

            div()
                .id("scrollbar")
                .absolute()
                .top(offset_top)
                .right_1()
                .h(SCROLLBAR_THUMB_HEIGHT)
                .w(SCROLLBAR_THUMB_WIDTH)
                .bg(rgb(0xC0C0C0))
                .hover(|this| this.bg(rgb(0xA0A0A0)))
                .rounded_lg()
                .child(
                    canvas(
                        |_, _, _| (),
                        move |thumb_bounds, _, window, _| {
                            window.on_mouse_event({
                                let entity = entity.clone();
                                move |ev: &MouseDownEvent, _, _, cx| {
                                    if !thumb_bounds.contains(&ev.position) {
                                        return;
                                    }

                                    entity.update(cx, |this, _| {
                                        this.drag_position = Some(
                                            ev.position - thumb_bounds.origin - table_bounds.origin,
                                        );
                                    })
                                }
                            });
                            window.on_mouse_event({
                                let entity = entity.clone();
                                move |_: &MouseUpEvent, _, _, cx| {
                                    entity.update(cx, |this, _| {
                                        this.drag_position = None;
                                    })
                                }
                            });

                            window.on_mouse_event(move |ev: &MouseMoveEvent, _, _, cx| {
                                if !ev.dragging() {
                                    return;
                                }

                                let Some(drag_pos) = entity.read(cx).drag_position else {
                                    return;
                                };

                                let inside_offset = drag_pos.y;
                                let percentage = ((ev.position.y - table_bounds.origin.y
                                    + inside_offset)
                                    / (table_bounds.size.height))
                                    .clamp(0., 1.);

                                let offset_y = ((scroll_height - table_bounds.size.height)
                                    * percentage)
                                    .clamp(px(0.), scroll_height - SCROLLBAR_THUMB_HEIGHT);
                                scroll_handle.set_offset(point(px(0.), -offset_y));
                                cx.notify(entity.entity_id());
                            })
                        },
                    )
                    .size_full(),
                )
        }
    }

    impl Render for DataTable {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .bg(rgpui::white())
                .text_sm()
                .size_full()
                .p_4()
                .gap_2()
                .flex()
                .flex_col()
                .child(format!(
                    "Total {} items, visible range: {:?}",
                    self.quotes.len(),
                    self.visible_range
                ))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .overflow_hidden()
                        .border_1()
                        .border_color(rgb(0xE0E0E0))
                        .rounded_sm()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .w_full()
                                .overflow_hidden()
                                .border_b_1()
                                .border_color(rgb(0xE0E0E0))
                                .text_color(rgb(0x555555))
                                .bg(rgb(0xF0F0F0))
                                .py_1()
                                .px_2()
                                .text_xs()
                                .children(FIELDS.map(|(key, width)| {
                                    div()
                                        .whitespace_nowrap()
                                        .flex_shrink_0()
                                        .truncate()
                                        .px_1()
                                        .w(px(width))
                                        .child(key.replace("_", " ").to_uppercase())
                                })),
                        )
                        .child(
                            div()
                                .relative()
                                .size_full()
                                .child(
                                    uniform_list(
                                        "items",
                                        self.quotes.len(),
                                        cx.processor(move |this, range: Range<usize>, _, _| {
                                            this.visible_range = range.clone();
                                            let mut items =
                                                Vec::with_capacity(range.end - range.start);
                                            for i in range {
                                                if let Some(quote) = this.quotes.get(i) {
                                                    items.push(TableRow::new(i, quote.clone()));
                                                }
                                            }
                                            items
                                        }),
                                    )
                                    .size_full()
                                    .track_scroll(&self.scroll_handle),
                                )
                                .child(self.render_scrollbar(window, cx)),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            cx.open_window(
                WindowOptions {
                    focus: true,
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(1280.0), px(1000.0)),
                        cx,
                    ))),
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|_| {
                        let mut table = DataTable::new();
                        table.generate();
                        table
                    })
                },
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| {
            let mut table = DataTable::new();
            table.generate();
            table
        })
        .into()
    }
}

mod tree_list {

    //! 深层级树示例：用于验证深层嵌套结构的渲染性能。
    //! 用法：左侧 tab 选「tree」，展开/折叠观察层级渲染。
    #![cfg_attr(target_family = "wasm", no_main)]
    //! Renders a div with deep children hierarchy. This example is useful to exemplify that Zed can
    //! handle deep hierarchies (even though it cannot just yet!).
    use std::sync::LazyLock;

    use rgpui::{
        App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
    };
    use rgpui_platform::application;

    struct Tree {}

    static DEPTH: LazyLock<u64> = LazyLock::new(|| {
        std::env::var("GPUI_TREE_DEPTH")
            .ok()
            .and_then(|depth| depth.parse().ok())
            .unwrap_or(50)
    });

    impl Render for Tree {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let mut depth = *DEPTH;
            static COLORS: [rgpui::Hsla; 4] =
                [rgpui::red(), rgpui::blue(), rgpui::green(), rgpui::yellow()];
            let mut colors = COLORS.iter().cycle().copied();
            let mut next_div = || div().p_0p5().bg(colors.next().unwrap());
            let mut innermost_node = next_div();
            while depth > 0 {
                innermost_node = next_div().child(innermost_node);
                depth -= 1;
            }
            innermost_node
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(300.0), px(300.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Tree {}),
            )
            .unwrap();
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| Tree {}).into()
    }
}

mod backstage {
    //! 应用菜单（Backstage）演示：点击左上「文件」展开覆盖整窗的面板，
    //! 左侧是页面导航（新建 / 打开 / 保存 / 打印 / 选项 / 关于），右侧随页面
    //! 切换为自定义内容或一条命令。Esc 或「返回」关闭。
    //!
    //! 对应 `rgpui` 的 `ribbon` feature（`rgpui::backstage`）。

    use rgpui::backstage::{Backstage, BackstageState, BackstageTab};
    use rgpui::{
        App, Bounds, ClickEvent, CommandDescriptor, Context, FocusHandle, IconName,
        InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
        StatefulInteractiveElement as _, Styled as _, Window, WindowBounds, WindowOptions, actions,
        div, prelude::*, px, register_command, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    actions!(backstage_demo, [New, Open, Save, Print, Options]);

    /// 「新建」命令。
    fn cmd_new() -> CommandDescriptor {
        CommandDescriptor::new("backstage::New", "新建").with_category("文件")
    }

    /// 「打开」命令。
    fn cmd_open() -> CommandDescriptor {
        CommandDescriptor::new("backstage::Open", "打开")
            .with_category("文件")
            .with_keystroke("ctrl-o")
    }

    /// 「保存」命令。
    fn cmd_save() -> CommandDescriptor {
        CommandDescriptor::new("backstage::Save", "保存")
            .with_category("文件")
            .with_keystroke("ctrl-s")
    }

    /// 「打印」命令。
    fn cmd_print() -> CommandDescriptor {
        CommandDescriptor::new("backstage::Print", "打印").with_category("文件")
    }

    /// 「选项」命令。
    fn cmd_options() -> CommandDescriptor {
        CommandDescriptor::new("backstage::Options", "选项").with_category("文件")
    }

    register_command!(cmd_new);
    register_command!(cmd_open);
    register_command!(cmd_save);
    register_command!(cmd_print);
    register_command!(cmd_options);

    /// 演示视图：顶栏「文件」按钮 + 内容区 + 应用菜单覆盖层。
    struct BackstageDemo {
        state: rgpui::Entity<BackstageState>,
        focus_handle: FocusHandle,
        last: String,
        template: Option<SharedString>,
    }

    impl BackstageDemo {
        /// 创建视图。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let state = cx.new(BackstageState::new);
            cx.observe(&state, |_, _, cx| cx.notify()).detach();
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            Self {
                state,
                focus_handle,
                last: "（尚未执行命令）".to_string(),
                template: None,
            }
        }

        /// 记录最近执行的命令。
        fn record(&mut self, name: &str, cx: &mut Context<Self>) {
            self.last = name.to_string();
            cx.notify();
        }

        /// 选择模板（来自「新建」页的自定义内容）。
        fn pick_template(&mut self, template: &str, cx: &mut Context<Self>) {
            let template: SharedString = template.into();
            self.template = Some(template.clone());
            self.record(&format!("新建自模板「{template}」"), cx);
        }

        /// 打开/关闭应用菜单，并把焦点交给面板（这样 Esc 才生效）。
        fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
            let open = !self.state.read(cx).is_open();
            self.state.update(cx, |state, cx| {
                if open {
                    state.open(cx);
                } else {
                    state.close(cx);
                }
            });
            let focus_handle = self.state.read(cx).focus_handle();
            window.focus(&focus_handle, cx);
        }
    }

    impl Render for BackstageDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            // 「新建」页的自定义内容需要回写本视图，用弱句柄避免循环引用。
            let weak = cx.entity().downgrade();
            let template = self.template.clone();
            let last = self.last.clone();

            let tabs = vec![
                BackstageTab::new("new", "新建")
                    .icon(IconName::Plus)
                    .content({
                        move |_window, _cx| {
                            let weak = weak.clone();
                            v_flex()
                                .gap(px(8.0))
                                .child(div().text_lg().child("从模板新建"))
                                .children(["空白文档", "报表模板", "会议纪要"].map(|name| {
                                    let weak = weak.clone();
                                    div()
                                        .id(name)
                                        .px(px(12.0))
                                        .py(px(8.0))
                                        .rounded_md()
                                        .cursor_pointer()
                                        .text_sm()
                                        .bg(rgb(0xf1f5f9))
                                        .hover(|style| style.bg(rgb(0xe2e8f0)))
                                        .child(name)
                                        .on_click(move |_event: &ClickEvent, _window, cx| {
                                            _ = weak.update(cx, |this, cx| {
                                                this.pick_template(name, cx);
                                            });
                                        })
                                }))
                                .into_any_element()
                        }
                    }),
                BackstageTab::new("open", "打开")
                    .icon(IconName::FolderOpen)
                    .command("backstage::Open"),
                BackstageTab::new("save", "保存")
                    .icon(IconName::Check)
                    .command("backstage::Save"),
                BackstageTab::new("print", "打印")
                    .icon(IconName::File)
                    .command("backstage::Print"),
                BackstageTab::new("options", "选项")
                    .icon(IconName::Settings)
                    .content(|_window, _cx| {
                        v_flex()
                            .gap(px(8.0))
                            .child(div().text_lg().child("选项"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x64748b))
                                    .child("设置页可承载任意表单，这里只演示内容页占位。"),
                            )
                            .into_any_element()
                    }),
                BackstageTab::new("about", "关于")
                    .icon(IconName::Search)
                    .content(|_window, _cx| {
                        v_flex()
                            .gap(px(8.0))
                            .child(div().text_lg().child("关于"))
                            .child(div().text_sm().child("应用菜单（Backstage）演示 · rgpui"))
                            .into_any_element()
                    }),
            ];

            let template_line = match &template {
                Some(name) => format!("当前模板：{name}"),
                None => "当前模板：未选择".to_string(),
            };

            v_flex()
                .id("backstage-demo")
                .relative()
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(|this, _: &New, _, cx| this.record("新建", cx)))
                .on_action(cx.listener(|this, _: &Open, _, cx| this.record("打开", cx)))
                .on_action(cx.listener(|this, _: &Save, _, cx| this.record("保存", cx)))
                .on_action(cx.listener(|this, _: &Print, _, cx| this.record("打印", cx)))
                .on_action(cx.listener(|this, _: &Options, _, cx| this.record("选项", cx)))
                .size_full()
                .bg(rgb(0xffffff))
                // 顶栏：「文件」按钮即应用菜单入口。
                .child(
                    div()
                        .id("file-button")
                        .w_full()
                        .flex_shrink_0()
                        .px(px(16.0))
                        .py(px(8.0))
                        .bg(rgb(0x1e293b))
                        .text_color(rgb(0xf8fafc))
                        .text_sm()
                        .cursor_pointer()
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, window, cx| this.toggle(window, cx)),
                        )
                        .child("文件"),
                )
                .child(
                    v_flex()
                        .gap(px(8.0))
                        .p(px(16.0))
                        .child(
                            div()
                                .text_lg()
                                .font_weight(rgpui::FontWeight::BOLD)
                                .child("应用菜单（Backstage）demo"),
                        )
                        .child(div().text_sm().child(format!("最近操作：{last}")))
                        .child(div().text_sm().child(template_line))
                        .child(div().text_xs().text_color(rgb(0x64748b)).child(
                            "点击左上「文件」展开面板：左侧切换页面，右侧是内容页或一条命令；\
                         关闭可通过「返回」或 Esc。",
                        )),
                )
                .child(Backstage::new(&self.state, tabs))
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(720.0), px(440.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| BackstageDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| BackstageDemo::new(window, cx)).into()
    }
}

mod command_registry {
    //! command registry 演示：`register_command!` 登记命令元数据 → `CommandRegistry` 读取 →
    //! 点击按 id 派发（`App::dispatch_command`）→ 视图 `on_action` 处理。
    //!
    //! 展示了套件类应用「一切皆命令」的骨架：UI、快捷键、命令面板、CLI、MCP 共用同一份
    //! 命令表，按 id 分发，与具体前端解耦。

    use rgpui::{
        App, Bounds, CommandDescriptor, CommandRegistry, Context, FocusHandle, Render, Window,
        WindowBounds, WindowOptions, actions, div, h_flex, prelude::*, px, register_command, size,
        v_flex,
    };
    use rgpui_platform::application;

    actions!(command_registry_demo, [Increment, Decrement, Reset]);

    /// 「增加」命令的描述符。
    fn cmd_increment() -> CommandDescriptor {
        CommandDescriptor::new("demo::Increment", "增加")
            .with_category("计数")
            .with_description("把计数加一")
            .with_keystroke("ctrl-i")
    }

    /// 「减少」命令的描述符。
    fn cmd_decrement() -> CommandDescriptor {
        CommandDescriptor::new("demo::Decrement", "减少")
            .with_category("计数")
            .with_description("把计数减一")
            .with_keystroke("ctrl-d")
    }

    /// 「重置」命令的描述符。
    fn cmd_reset() -> CommandDescriptor {
        CommandDescriptor::new("demo::Reset", "重置")
            .with_category("计数")
            .with_description("把计数清零")
    }

    register_command!(cmd_increment);
    register_command!(cmd_decrement);
    register_command!(cmd_reset);

    /// 演示视图：显示计数，并列出注册表中的全部命令。
    struct CommandRegistryDemo {
        count: i32,
        focus_handle: FocusHandle,
    }

    impl CommandRegistryDemo {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            Self {
                count: 0,
                focus_handle,
            }
        }

        fn on_increment(&mut self, _: &Increment, _window: &mut Window, cx: &mut Context<Self>) {
            self.count += 1;
            cx.notify();
        }

        fn on_decrement(&mut self, _: &Decrement, _window: &mut Window, cx: &mut Context<Self>) {
            self.count -= 1;
            cx.notify();
        }

        fn on_reset(&mut self, _: &Reset, _window: &mut Window, cx: &mut Context<Self>) {
            self.count = 0;
            cx.notify();
        }
    }

    impl Render for CommandRegistryDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let registry = CommandRegistry::global();

            let rows = registry.iter().map(|descriptor| {
                let id = descriptor.id().clone();
                let shortcut = descriptor
                    .default_keystrokes()
                    .first()
                    .map(|key| key.to_string())
                    .unwrap_or_default();
                let label = format!("{}（{}）{}", descriptor.label(), descriptor.id(), shortcut);
                div()
                    .id(descriptor.id().clone())
                    .px(px(10.0))
                    .py(px(6.0))
                    .rounded_md()
                    .bg(rgpui::rgb(0xeef2ff))
                    .cursor_pointer()
                    .text_sm()
                    .hover(|style| style.bg(rgpui::rgb(0xdfe6ff)))
                    .child(label)
                    .on_click(move |_, _window, cx| {
                        if let Err(error) = cx.dispatch_command(&id, None) {
                            log::warn!("派发命令 `{id}` 失败：{error}");
                        }
                    })
            });

            let summary = format!(
                "计数：{}  ｜  已登记 {} 条命令，分类：{}",
                self.count,
                registry.len(),
                registry
                    .categories()
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(" / ")
            );

            v_flex()
                .id("command-registry-demo")
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(Self::on_increment))
                .on_action(cx.listener(Self::on_decrement))
                .on_action(cx.listener(Self::on_reset))
                .size_full()
                .gap(px(10.0))
                .p(px(16.0))
                .bg(rgpui::rgb(0xf8fafc))
                .child(
                    div()
                        .text_lg()
                        .font_weight(rgpui::FontWeight::BOLD)
                        .child("命令注册表 demo"),
                )
                .child(div().text_sm().child(summary))
                .child(
                    div().text_xs().text_color(rgpui::rgb(0x64748b)).child(
                        "点击任意命令按 id 派发；也可用 Ctrl+I / Ctrl+D（需先加载快捷键）。",
                    ),
                )
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .items_center()
                        .child(
                            div()
                                .id("cmd-increment")
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded_md()
                                .cursor_pointer()
                                .text_xs()
                                .child("增加（demo::Increment）")
                                .on_click(|_, _window, cx| {
                                    let _ = cx.dispatch_command("demo::Increment", None);
                                }),
                        )
                        .child(
                            div()
                                .id("cmd-decrement")
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded_md()
                                .cursor_pointer()
                                .text_xs()
                                .child("减少（demo::Decrement）")
                                .on_click(|_, _window, cx| {
                                    let _ = cx.dispatch_command("demo::Decrement", None);
                                }),
                        ),
                )
                .child(
                    v_flex()
                        .gap(px(4.0))
                        .flex_1()
                        .overflow_hidden()
                        .children(rows),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(560.0), px(480.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| CommandRegistryDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| CommandRegistryDemo::new(window, cx)).into()
    }
}

mod mini_toolbar {
    //! 迷你工具栏（MiniToolbar）演示：在「文档」上拖动选择后就近浮出紧凑命令条，
    //! 按钮与 QAT 同源（`QatItem`，按命令 id 派发）。
    //!
    //! 对应 `rgpui` 的 `ribbon` feature（`rgpui::mini_toolbar` + `rgpui::qat`）。

    use rgpui::mini_toolbar::MiniToolbar;
    use rgpui::qat::QatItem;
    use rgpui::{
        App, Bounds, CommandDescriptor, Context, FocusHandle, IconName, MouseButton, Pixels,
        Render, SharedString, Window, WindowBounds, WindowOptions, actions, div, point, prelude::*,
        px, register_command, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    /// 页面顶部说明条高度（此区域不参与选择）。
    const HEADER_H: Pixels = px(56.0);

    actions!(
        mini_toolbar_minitoolbar,
        [Bold, Italic, Underline, Highlight, Copy]
    );

    /// 「加粗」命令描述符。
    fn cmd_bold() -> CommandDescriptor {
        CommandDescriptor::new("minitoolbar::Bold", "加粗")
            .with_category("格式")
            .with_description("切换选区加粗")
    }

    /// 「斜体」命令描述符。
    fn cmd_italic() -> CommandDescriptor {
        CommandDescriptor::new("minitoolbar::Italic", "斜体").with_category("格式")
    }

    /// 「下划线」命令描述符。
    fn cmd_underline() -> CommandDescriptor {
        CommandDescriptor::new("minitoolbar::Underline", "下划线").with_category("格式")
    }

    /// 「高亮」命令描述符。
    fn cmd_highlight() -> CommandDescriptor {
        CommandDescriptor::new("minitoolbar::Highlight", "高亮").with_category("格式")
    }

    /// 「复制」命令描述符。
    fn cmd_copy() -> CommandDescriptor {
        CommandDescriptor::new("minitoolbar::Copy", "复制")
            .with_category("剪贴板")
            .with_keystroke("ctrl-c")
    }

    register_command!(cmd_bold);
    register_command!(cmd_italic);
    register_command!(cmd_underline);
    register_command!(cmd_highlight);
    register_command!(cmd_copy);

    /// 选区（起点 → 终点，窗口坐标）。
    type Selection = (rgpui::Point<Pixels>, rgpui::Point<Pixels>);

    /// 演示视图：假文档 + 拖动选择 + 就近浮出的迷你工具栏。
    struct MiniToolbarDemo {
        focus_handle: FocusHandle,
        dragging: bool,
        selection: Option<Selection>,
        bold: bool,
        italic: bool,
        underline: bool,
        highlight: bool,
        status: SharedString,
    }

    impl MiniToolbarDemo {
        /// 创建视图并聚焦自身。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            Self {
                focus_handle,
                dragging: false,
                selection: None,
                bold: false,
                italic: false,
                underline: false,
                highlight: false,
                status: SharedString::from("在下方文档区按住左键拖动即可选中。"),
            }
        }

        /// 设置状态栏文本。
        fn set_status(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
            self.status = text.into();
            cx.notify();
        }

        fn on_bold(&mut self, _: &Bold, _window: &mut Window, cx: &mut Context<Self>) {
            self.bold = !self.bold;
            let state = if self.bold { "开" } else { "关" };
            self.set_status(format!("加粗：{state}"), cx);
        }

        fn on_italic(&mut self, _: &Italic, _window: &mut Window, cx: &mut Context<Self>) {
            self.italic = !self.italic;
            let state = if self.italic { "开" } else { "关" };
            self.set_status(format!("斜体：{state}"), cx);
        }

        fn on_underline(&mut self, _: &Underline, _window: &mut Window, cx: &mut Context<Self>) {
            self.underline = !self.underline;
            let state = if self.underline { "开" } else { "关" };
            self.set_status(format!("下划线：{state}"), cx);
        }

        fn on_highlight(&mut self, _: &Highlight, _window: &mut Window, cx: &mut Context<Self>) {
            self.highlight = !self.highlight;
            let state = if self.highlight { "开" } else { "关" };
            self.set_status(format!("高亮：{state}"), cx);
        }

        fn on_copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
            self.set_status("已复制选区（演示）", cx);
        }

        /// 当前格式状态的文字摘要。
        fn format_summary(&self) -> String {
            let mark = |on: bool| if on { "开" } else { "关" };
            format!(
                "加粗 {} ｜ 斜体 {} ｜ 下划线 {} ｜ 高亮 {}",
                mark(self.bold),
                mark(self.italic),
                mark(self.underline),
                mark(self.highlight)
            )
        }

        /// 迷你工具栏的落点：选区终点右下偏移，避免遮住选区。
        fn toolbar_at(selection: Selection) -> rgpui::Point<Pixels> {
            point(selection.1.x + px(4.0), selection.1.y + px(18.0))
        }
    }

    impl Render for MiniToolbarDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let selection = self.selection;
            let summary = self.format_summary();
            let status = self.status.clone();

            // 选区高亮矩形（根元素原点即窗口原点，坐标可直接用）。
            let selection_rect = selection.map(|(a, b)| {
                let origin = point(a.x.min(b.x), a.y.min(b.y));
                let extent = size((a.x - b.x).abs(), (a.y - b.y).abs());
                div()
                    .absolute()
                    .left(origin.x)
                    .top(origin.y)
                    .w(extent.width)
                    .h(extent.height)
                    .bg(rgb(0x60a5fa).opacity(0.25))
            });

            div()
                .id("mini-toolbar-demo")
                .relative()
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(Self::on_bold))
                .on_action(cx.listener(Self::on_italic))
                .on_action(cx.listener(Self::on_underline))
                .on_action(cx.listener(Self::on_highlight))
                .on_action(cx.listener(Self::on_copy))
                .on_key_down(
                    cx.listener(|this, event: &rgpui::KeyDownEvent, _window, cx| {
                        if event.keystroke.key == "escape" && this.selection.take().is_some() {
                            cx.notify();
                        }
                    }),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &rgpui::MouseDownEvent, _window, cx| {
                        if event.position.y < HEADER_H {
                            return;
                        }
                        this.dragging = true;
                        this.selection = Some((event.position, event.position));
                        cx.notify();
                    }),
                )
                .on_mouse_move(
                    cx.listener(|this, event: &rgpui::MouseMoveEvent, _window, cx| {
                        if let Some((start, _)) = this.selection
                            && this.dragging
                        {
                            this.selection = Some((start, event.position));
                            cx.notify();
                        }
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _event: &rgpui::MouseUpEvent, _window, cx| {
                        this.dragging = false;
                        cx.notify();
                    }),
                )
                .size_full()
                .bg(rgb(0xf8fafc))
                // 说明条（覆盖在顶部，不参与选择）。
                .child(
                    v_flex()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w_full()
                        .h(HEADER_H)
                        .justify_center()
                        .gap(px(2.0))
                        .px(px(16.0))
                        .bg(rgb(0xf1f5f9))
                        .border_b_1()
                        .border_color(rgb(0xe2e8f0))
                        .child(
                            div()
                                .text_lg()
                                .font_weight(rgpui::FontWeight::BOLD)
                                .child("迷你工具栏（MiniToolbar）demo"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x64748b))
                                .child("在文档区按住左键拖动 → 就近浮出命令条；Esc 取消选择。"),
                        ),
                )
                // 假文档内容。
                .child(
                    v_flex()
                        .absolute()
                        .top(HEADER_H)
                        .left_0()
                        .w_full()
                        .p(px(24.0))
                        .gap(px(10.0))
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x334155))
                                .child("rgpui 的迷你工具栏与快速访问工具栏共用 QatItem："),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x334155))
                                .child("按钮按命令 id 绑定，点击经 App::dispatch_command 派发。"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x334155))
                                .child("选择这段文字试试：松开鼠标后面板会停在选区旁边。"),
                        ),
                )
                // 选区高亮。
                .children(selection_rect)
                // 迷你工具栏（就近浮出）。
                .children(selection.map(|selection| {
                    MiniToolbar::new()
                        .at(Self::toolbar_at(selection))
                        .item(QatItem::button("minitoolbar::Bold").label("B"))
                        .item(QatItem::button("minitoolbar::Italic").label("I"))
                        .item(QatItem::button("minitoolbar::Underline").label("U"))
                        .item(QatItem::separator())
                        .item(QatItem::button("minitoolbar::Highlight").icon(IconName::Palette))
                        .item(QatItem::button("minitoolbar::Copy").icon(IconName::Copy))
                }))
                // 底部状态条。
                .child(
                    v_flex()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .w_full()
                        .gap(px(2.0))
                        .px(px(16.0))
                        .py(px(6.0))
                        .bg(rgb(0xf1f5f9))
                        .border_t_1()
                        .border_color(rgb(0xe2e8f0))
                        .child(div().text_xs().child(status))
                        .child(div().text_xs().text_color(rgb(0x64748b)).child(summary)),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(640.0), px(420.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| MiniToolbarDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| MiniToolbarDemo::new(window, cx)).into()
    }
}

mod qat {
    //! 快速访问工具栏（QAT）演示：一排按**命令 id** 绑定的紧凑图标按钮，
    //! 通常置于 Ribbon / 标题栏之上；点击经 `App::dispatch_command` 派发，
    //! 与 Ribbon、菜单、命令面板共用同一份命令表。
    //!
    //! 本例对应 `rgpui` 的 `ribbon` feature（`QatItem` / `QuickAccessToolbar`）。

    use rgpui::qat::{QatItem, QuickAccessToolbar};
    use rgpui::{
        App, Bounds, CommandDescriptor, Context, FocusHandle, IconName, Render, SharedString,
        Window, WindowBounds, WindowOptions, actions, div, h_flex, prelude::*, px,
        register_command, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    actions!(qat_qat, [Save, Undo, Redo, Settings, Search]);

    /// 「保存」命令描述符。
    fn cmd_save() -> CommandDescriptor {
        CommandDescriptor::new("qat::Save", "保存")
            .with_category("快速访问")
            .with_keystroke("ctrl-s")
    }

    /// 「撤销」命令描述符。
    fn cmd_undo() -> CommandDescriptor {
        CommandDescriptor::new("qat::Undo", "撤销")
            .with_category("快速访问")
            .with_keystroke("ctrl-z")
    }

    /// 「重做」命令描述符。
    fn cmd_redo() -> CommandDescriptor {
        CommandDescriptor::new("qat::Redo", "重做")
            .with_category("快速访问")
            .with_keystroke("ctrl-y")
    }

    /// 「设置」命令描述符。
    fn cmd_settings() -> CommandDescriptor {
        CommandDescriptor::new("qat::Settings", "设置")
            .with_category("快速访问")
            .with_description("打开应用设置")
    }

    /// 「查找」命令描述符。
    fn cmd_search() -> CommandDescriptor {
        CommandDescriptor::new("qat::Search", "查找").with_category("快速访问")
    }

    register_command!(cmd_save);
    register_command!(cmd_undo);
    register_command!(cmd_redo);
    register_command!(cmd_settings);
    register_command!(cmd_search);

    /// 演示视图：顶部一条 QAT，下方是等价的普通按钮与最近一次命令回显。
    struct QatDemo {
        last: Option<SharedString>,
        focus_handle: FocusHandle,
    }

    impl QatDemo {
        /// 创建视图并聚焦自身。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            Self {
                last: None,
                focus_handle,
            }
        }

        /// 记录最近一次被派发的命令。
        fn record(&mut self, label: &str, cx: &mut Context<Self>) {
            self.last = Some(label.into());
            cx.notify();
        }

        fn on_save(&mut self, _: &Save, _window: &mut Window, cx: &mut Context<Self>) {
            self.record("保存", cx);
        }

        fn on_undo(&mut self, _: &Undo, _window: &mut Window, cx: &mut Context<Self>) {
            self.record("撤销", cx);
        }

        fn on_redo(&mut self, _: &Redo, _window: &mut Window, cx: &mut Context<Self>) {
            self.record("重做", cx);
        }

        fn on_settings(&mut self, _: &Settings, _window: &mut Window, cx: &mut Context<Self>) {
            self.record("设置", cx);
        }

        fn on_search(&mut self, _: &Search, _window: &mut Window, cx: &mut Context<Self>) {
            self.record("查找", cx);
        }
    }

    impl Render for QatDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let last = self
                .last
                .clone()
                .unwrap_or_else(|| SharedString::from("（尚未派发）"));

            v_flex()
                .id("qat-demo")
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(Self::on_save))
                .on_action(cx.listener(Self::on_undo))
                .on_action(cx.listener(Self::on_redo))
                .on_action(cx.listener(Self::on_settings))
                .on_action(cx.listener(Self::on_search))
                .size_full()
                .bg(rgb(0xf8fafc))
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .px(px(8.0))
                        .py(px(4.0))
                        .bg(rgb(0xf1f5f9))
                        .border_b_1()
                        .border_color(rgb(0xe2e8f0))
                        .child(
                            QuickAccessToolbar::new()
                                .item(QatItem::button("qat::Save").icon(IconName::Check))
                                .item(QatItem::button("qat::Undo").icon(IconName::Undo))
                                .item(QatItem::button("qat::Redo").icon(IconName::Redo))
                                .item(QatItem::separator())
                                .item(QatItem::button("qat::Search").label("查找"))
                                .item(QatItem::button("qat::Settings").icon(IconName::Settings)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x64748b))
                                .child(format!("最近派发：{last}")),
                        ),
                )
                .child(
                    v_flex()
                        .gap(px(10.0))
                        .p(px(16.0))
                        .child(
                            div()
                                .text_lg()
                                .font_weight(rgpui::FontWeight::BOLD)
                                .child("快速访问工具栏（QAT）demo"),
                        )
                        .child(div().text_sm().text_color(rgb(0x475569)).child(
                            "顶部一条 QAT 按命令 id 派发；下方按钮派发同一条命令，效果一致。",
                        ))
                        .child(div().text_sm().child(format!("最近派发：{last}")))
                        .child(
                            h_flex()
                                .gap(px(8.0))
                                .items_center()
                                .child(
                                    div()
                                        .id("plain-save")
                                        .px(px(10.0))
                                        .py(px(4.0))
                                        .rounded_md()
                                        .bg(rgb(0xeef2ff))
                                        .cursor_pointer()
                                        .text_xs()
                                        .child("普通按钮：保存（qat::Save）")
                                        .on_click(|_, _window, cx| {
                                            let _ = cx.dispatch_command("qat::Save", None);
                                        }),
                                )
                                .child(
                                    div()
                                        .id("plain-settings")
                                        .px(px(10.0))
                                        .py(px(4.0))
                                        .rounded_md()
                                        .bg(rgb(0xeef2ff))
                                        .cursor_pointer()
                                        .text_xs()
                                        .child("普通按钮：设置（qat::Settings）")
                                        .on_click(|_, _window, cx| {
                                            let _ = cx.dispatch_command("qat::Settings", None);
                                        }),
                                ),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(620.0), px(340.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| QatDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| QatDemo::new(window, cx)).into()
    }
}

mod ribbon {
    //! Ribbon 功能区演示：数据驱动的选项卡/分组/大按钮/小按钮/切换/下拉，
    //! 命令按 id 绑定到 `CommandRegistry`，点击经 `App::dispatch_command` 派发到 `on_action`。

    use rgpui::ribbon::{
        Ribbon, RibbonComboBoxItem, RibbonContext, RibbonDropdownItem, RibbonGalleryItem,
        RibbonGroup, RibbonItem, RibbonState, RibbonTab,
    };
    use rgpui::{
        App, Bounds, CommandDescriptor, Context, FocusHandle, IconName, Render, Window,
        WindowBounds, WindowOptions, actions, div, prelude::*, px, register_command, rgb, size,
        v_flex,
    };
    use rgpui_platform::application;

    actions!(
        demo,
        [
            Paste,
            PasteSpecial,
            Cut,
            Copy,
            Bold,
            InsertTable,
            InsertPicture,
            InsertChart,
            StyleNormal,
            StyleTitle,
            StyleEmphasis,
            Settings
        ]
    );

    /// 「粘贴」命令。
    fn cmd_paste() -> CommandDescriptor {
        CommandDescriptor::new("demo::Paste", "粘贴")
            .with_category("剪贴板")
            .with_keystroke("ctrl-v")
            .with_keytip("V")
    }
    /// 「选择性粘贴」命令（粘贴分裂按钮的下拉项）。
    fn cmd_paste_special() -> CommandDescriptor {
        CommandDescriptor::new("demo::PasteSpecial", "选择性粘贴").with_category("剪贴板")
    }
    /// 「剪切」命令。
    fn cmd_cut() -> CommandDescriptor {
        CommandDescriptor::new("demo::Cut", "剪切")
            .with_category("剪贴板")
            .with_keytip("X")
    }
    /// 「复制」命令。
    fn cmd_copy() -> CommandDescriptor {
        CommandDescriptor::new("demo::Copy", "复制")
            .with_category("剪贴板")
            .with_keytip("C")
    }
    /// 「加粗」命令。
    fn cmd_bold() -> CommandDescriptor {
        CommandDescriptor::new("demo::Bold", "加粗")
            .with_category("字体")
            .with_keytip("B")
    }
    /// 「常规样式」命令（样式库项）。
    fn cmd_style_normal() -> CommandDescriptor {
        CommandDescriptor::new("demo::StyleNormal", "常规").with_category("样式")
    }
    /// 「标题样式」命令（样式库项）。
    fn cmd_style_title() -> CommandDescriptor {
        CommandDescriptor::new("demo::StyleTitle", "标题").with_category("样式")
    }
    /// 「强调样式」命令（样式库项）。
    fn cmd_style_emphasis() -> CommandDescriptor {
        CommandDescriptor::new("demo::StyleEmphasis", "强调").with_category("样式")
    }
    /// 「插入表格」命令。
    fn cmd_insert_table() -> CommandDescriptor {
        CommandDescriptor::new("demo::InsertTable", "表格")
            .with_category("插入")
            .with_keytip("T")
    }
    /// 「插入图片」命令。
    fn cmd_insert_picture() -> CommandDescriptor {
        CommandDescriptor::new("demo::InsertPicture", "图片")
            .with_category("插入")
            .with_keytip("P")
    }
    /// 「插入图表」命令（插入组合框项）。
    fn cmd_insert_chart() -> CommandDescriptor {
        CommandDescriptor::new("demo::InsertChart", "图表").with_category("插入")
    }
    /// 「设置」命令。
    fn cmd_settings() -> CommandDescriptor {
        CommandDescriptor::new("demo::Settings", "设置")
            .with_category("文件")
            .with_keytip("S")
    }

    register_command!(cmd_paste);
    register_command!(cmd_paste_special);
    register_command!(cmd_cut);
    register_command!(cmd_copy);
    register_command!(cmd_bold);
    register_command!(cmd_style_normal);
    register_command!(cmd_style_title);
    register_command!(cmd_style_emphasis);
    register_command!(cmd_insert_table);
    register_command!(cmd_insert_picture);
    register_command!(cmd_insert_chart);
    register_command!(cmd_settings);

    /// 演示视图。
    struct RibbonDemo {
        state: rgpui::Entity<RibbonState>,
        focus_handle: FocusHandle,
        last: String,
        bold: bool,
    }

    impl RibbonDemo {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let state = cx.new(RibbonState::new);
            // 切换选项卡时重绘本视图。
            cx.observe(&state, |_, _, cx| cx.notify()).detach();
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            Self {
                state,
                focus_handle,
                last: "（尚未执行命令）".to_string(),
                bold: false,
            }
        }

        fn record(&mut self, name: &str, cx: &mut Context<Self>) {
            self.last = name.to_string();
            cx.notify();
        }
    }

    impl Render for RibbonDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let tabs = vec![
                RibbonTab::new("home", "开始")
                    .keytip("H")
                    .group(
                        RibbonGroup::new("剪贴板")
                            .item(
                                RibbonItem::split("demo::Paste")
                                    .label("粘贴")
                                    .keytip("V")
                                    .icon(IconName::Copy)
                                    .large()
                                    .item(
                                        RibbonDropdownItem::new("demo::PasteSpecial")
                                            .label("选择性粘贴"),
                                    ),
                            )
                            .item(
                                RibbonItem::button("demo::Cut")
                                    .label("剪切")
                                    .icon(IconName::Delete),
                            )
                            .item(
                                RibbonItem::button("demo::Copy")
                                    .label("复制")
                                    .icon(IconName::Copy),
                            ),
                    )
                    .group(
                        RibbonGroup::new("插入")
                            .item(
                                RibbonItem::button("demo::InsertTable")
                                    .label("表格")
                                    .icon(IconName::LayoutDashboard)
                                    .large(),
                            )
                            .item(
                                RibbonItem::dropdown("更多")
                                    .keytip("M")
                                    .icon(IconName::Plus)
                                    .item(RibbonDropdownItem::new("demo::InsertPicture"))
                                    .item(RibbonDropdownItem::new("demo::Settings")),
                            )
                            .item(
                                RibbonItem::combobox("insert-kind")
                                    .keytip("K")
                                    .width(96.0)
                                    .selected(0)
                                    .placeholder("插入")
                                    .item(
                                        RibbonComboBoxItem::new("demo::InsertTable").label("表格"),
                                    )
                                    .item(
                                        RibbonComboBoxItem::new("demo::InsertPicture")
                                            .label("图片"),
                                    )
                                    .item(
                                        RibbonComboBoxItem::new("demo::InsertChart").label("图表"),
                                    ),
                            ),
                    )
                    .group(
                        RibbonGroup::new("样式")
                            .item(
                                RibbonItem::gallery("样式库")
                                    .keytip("G")
                                    .columns(2)
                                    .large()
                                    .icon(IconName::Palette)
                                    .item(
                                        RibbonGalleryItem::new("demo::StyleNormal")
                                            .label("常规")
                                            .icon(IconName::File)
                                            .selected(true),
                                    )
                                    .item(
                                        RibbonGalleryItem::new("demo::StyleTitle")
                                            .label("标题")
                                            .icon(IconName::Star),
                                    )
                                    .item(
                                        RibbonGalleryItem::new("demo::StyleEmphasis")
                                            .label("强调")
                                            .icon(IconName::Palette),
                                    ),
                            )
                            .item(
                                RibbonItem::toggle("demo::Bold")
                                    .label("加粗")
                                    .icon(IconName::Palette)
                                    .checked(self.bold),
                            ),
                    ),
                RibbonTab::new("insert", "插入")
                    .keytip("N")
                    .group(
                        RibbonGroup::new("插图").item(
                            RibbonItem::button("demo::InsertPicture")
                                .label("图片")
                                .icon(IconName::GalleryVerticalEnd)
                                .large(),
                        ),
                    )
                    .context(RibbonContext::new("图片工具", rgb(0xc2410c))),
            ];

            v_flex()
                .id("ribbon-demo")
                .key_context("Ribbon")
                .track_focus(&self.focus_handle)
                .on_action(
                    cx.listener(|this, _: &rgpui::ribbon::ToggleKeyTips, window, cx| {
                        this.state
                            .update(cx, |state, cx| state.toggle_keytips(window, cx));
                    }),
                )
                .on_action(cx.listener(|this, _: &Paste, _, cx| this.record("粘贴", cx)))
                .on_action(
                    cx.listener(|this, _: &PasteSpecial, _, cx| this.record("选择性粘贴", cx)),
                )
                .on_action(cx.listener(|this, _: &Cut, _, cx| this.record("剪切", cx)))
                .on_action(cx.listener(|this, _: &Copy, _, cx| this.record("复制", cx)))
                .on_action(cx.listener(|this, _: &Bold, _, cx| {
                    let bold = !this.bold;
                    this.bold = bold;
                    this.record("加粗", cx);
                }))
                .on_action(cx.listener(|this, _: &StyleNormal, _, cx| this.record("常规样式", cx)))
                .on_action(cx.listener(|this, _: &StyleTitle, _, cx| this.record("标题样式", cx)))
                .on_action(
                    cx.listener(|this, _: &StyleEmphasis, _, cx| this.record("强调样式", cx)),
                )
                .on_action(cx.listener(|this, _: &InsertTable, _, cx| this.record("插入表格", cx)))
                .on_action(
                    cx.listener(|this, _: &InsertPicture, _, cx| this.record("插入图片", cx)),
                )
                .on_action(cx.listener(|this, _: &InsertChart, _, cx| this.record("插入图表", cx)))
                .on_action(cx.listener(|this, _: &Settings, _, cx| this.record("设置", cx)))
                .size_full()
                .bg(rgpui::rgb(0xffffff))
                .child(Ribbon::new(&self.state, tabs))
                .child(
                    v_flex()
                        .gap(px(8.0))
                        .p(px(16.0))
                        .child(
                            div()
                                .text_lg()
                                .font_weight(rgpui::FontWeight::BOLD)
                                .child("Ribbon 功能区 demo"),
                        )
                        .child(div().text_sm().child(format!("最近执行：{}", self.last)))
                        .child(div().text_xs().text_color(rgpui::rgb(0x64748b)).child(
                            "点击功能区按钮或切换选项卡；命令来自 CommandRegistry，按 id 派发。\
                         按 Alt/F10 显示按键提示，再按选项卡/命令提示键即可键盘操作。",
                        )),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(820.0), px(420.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| RibbonDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| RibbonDemo::new(window, cx)).into()
    }
}

mod canvas_objects {
    //! 画布对象工具演示：选区 / 缩放手柄 / 旋转手柄 / 对齐 / 分布。
    //!
    //! 三个矩形对象放在可缩放平移的画布上：点击选中、拖动移动、拖手柄缩放、拖顶部旋转
    //! 手柄旋转；工具栏可左/水平居中/右对齐与水平分布。几何全部来自
    //! `rgpui::object_toolkit`，屏幕映射来自 `rgpui::viewport::Viewport`。

    use rgpui::object_toolkit::{
        AlignMode, ResizeHandle, SelectionHandles, align, distribute, hit_handle, resize_bounds,
        union_bounds,
    };
    use rgpui::viewport::Viewport;
    use rgpui::{
        App, Axis, Bounds, Button, Context, FocusHandle, IntoElement, MouseButton, MouseDownEvent,
        MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, ScrollWheelEvent,
        Styled, Window, WindowBounds, WindowOptions, div, h_flex, point, prelude::*, px, rgb, size,
        v_flex,
    };
    use rgpui_platform::application;

    /// 画布对象。
    struct CanvasObject {
        /// 边界（文档坐标）。
        rect: Bounds<Pixels>,
        /// 旋转角度（度，顺时针）。
        rotation: f32,
        /// 填充色。
        color: rgpui::Hsla,
        /// 标签。
        label: &'static str,
    }

    /// 拖拽模式。
    enum Drag {
        /// 未拖拽。
        None,
        /// 移动：记录起点文档坐标。
        Move(Point<Pixels>),
        /// 缩放：记录手柄。
        Resize(ResizeHandle),
        /// 旋转：记录中心、起始指针角度与起始角度。
        Rotate {
            /// 旋转中心（文档坐标）。
            center: Point<Pixels>,
            /// 起始指针相对中心的角度（弧度）。
            start_angle: f32,
            /// 起始旋转角度（度）。
            start_rotation: f32,
        },
    }

    /// 演示视图。
    struct CanvasObjectsDemo {
        /// 对象集合。
        objects: Vec<CanvasObject>,
        /// 当前选中对象下标。
        selected: Option<usize>,
        /// 视口。
        viewport: Viewport,
        /// 拖拽状态。
        drag: Drag,
        /// 焦点句柄。
        focus_handle: FocusHandle,
    }

    impl CanvasObjectsDemo {
        /// 创建演示视图。
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            let objects = vec![
                CanvasObject {
                    rect: Bounds::new(point(px(80.0), px(80.0)), size(px(220.0), px(140.0))),
                    rotation: 0.0,
                    color: rgb(0x60a5fa).into(),
                    label: "卡片 A",
                },
                CanvasObject {
                    rect: Bounds::new(point(px(360.0), px(120.0)), size(px(180.0), px(200.0))),
                    rotation: 0.0,
                    color: rgb(0x34d399).into(),
                    label: "卡片 B",
                },
                CanvasObject {
                    rect: Bounds::new(point(px(140.0), px(320.0)), size(px(260.0), px(120.0))),
                    rotation: 0.0,
                    color: rgb(0xfbbf24).into(),
                    label: "卡片 C",
                },
            ];
            Self {
                objects,
                selected: None,
                viewport: Viewport::new().with_offset(point(px(48.0), px(24.0))),
                drag: Drag::None,
                focus_handle: cx.focus_handle(),
            }
        }

        /// 选区（选中对象的边界，文档坐标）。
        fn selection(&self) -> Option<Bounds<Pixels>> {
            self.selected.map(|i| self.objects[i].rect)
        }

        /// 鼠标按下：命中手柄 → 缩放/旋转；命中对象 → 选中并移动；否则取消选择。
        fn on_mouse_down(&mut self, screen: Point<Pixels>, cx: &mut Context<Self>) {
            let doc = self.viewport.screen_to_document(screen);
            if let Some(selection) = self.selection()
                && let Some(handle) = hit_handle(selection, doc, px(7.0))
            {
                let index = self.selected.unwrap();
                if handle.is_rotate() {
                    let center = point(
                        selection.origin.x + selection.size.width / 2.0,
                        selection.origin.y + selection.size.height / 2.0,
                    );
                    let start_angle = (doc.y - center.y)
                        .as_f32()
                        .atan2((doc.x - center.x).as_f32());
                    self.drag = Drag::Rotate {
                        center,
                        start_angle,
                        start_rotation: self.objects[index].rotation,
                    };
                } else {
                    self.drag = Drag::Resize(handle);
                }
                cx.notify();
                return;
            }
            // 从上到下命中对象。
            let hit = (0..self.objects.len())
                .rev()
                .find(|i| self.objects[*i].rect.contains(&doc));
            self.selected = hit;
            self.drag = match hit {
                Some(_) => Drag::Move(doc),
                None => Drag::None,
            };
            cx.notify();
        }

        /// 鼠标移动：按拖拽模式更新。
        fn on_mouse_move(&mut self, screen: Point<Pixels>, cx: &mut Context<Self>) {
            let doc = self.viewport.screen_to_document(screen);
            let Some(index) = self.selected else {
                return;
            };
            match self.drag {
                Drag::Move(last) => {
                    let delta_x = doc.x - last.x;
                    let delta_y = doc.y - last.y;
                    let rect = &mut self.objects[index].rect;
                    rect.origin = point(rect.origin.x + delta_x, rect.origin.y + delta_y);
                    self.drag = Drag::Move(doc);
                    cx.notify();
                }
                Drag::Resize(handle) => {
                    let rect = self.objects[index].rect;
                    self.objects[index].rect = resize_bounds(rect, handle, doc, px(24.0));
                    cx.notify();
                }
                Drag::Rotate {
                    center,
                    start_angle,
                    start_rotation,
                } => {
                    let now = (doc.y - center.y)
                        .as_f32()
                        .atan2((doc.x - center.x).as_f32());
                    let delta = now - start_angle;
                    self.objects[index].rotation = start_rotation + delta.to_degrees();
                    cx.notify();
                }
                Drag::None => {}
            }
        }

        /// 鼠标释放：结束拖拽。
        fn on_mouse_up(&mut self, cx: &mut Context<Self>) {
            self.drag = Drag::None;
            cx.notify();
        }

        /// 对全部对象应用对齐。
        fn apply_align(&mut self, mode: AlignMode, cx: &mut Context<Self>) {
            let rects: Vec<Bounds<Pixels>> = self.objects.iter().map(|o| o.rect).collect();
            let aligned = align(&rects, mode);
            for (object, rect) in self.objects.iter_mut().zip(aligned) {
                object.rect = rect;
            }
            cx.notify();
        }

        /// 水平分布全部对象。
        fn apply_distribute(&mut self, cx: &mut Context<Self>) {
            let rects: Vec<Bounds<Pixels>> = self.objects.iter().map(|o| o.rect).collect();
            let distributed = distribute(&rects, Axis::Horizontal);
            for (object, rect) in self.objects.iter_mut().zip(distributed) {
                object.rect = rect;
            }
            cx.notify();
        }
    }

    impl Render for CanvasObjectsDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let viewport = self.viewport;
            let union = union_bounds(&self.objects.iter().map(|o| o.rect).collect::<Vec<_>>());

            // 对象层（后绘制的在上）。
            let objects: Vec<rgpui::AnyElement> = self
                .objects
                .iter()
                .enumerate()
                .map(|(index, object)| {
                    let origin = viewport.document_to_screen(object.rect.origin);
                    let size = viewport.document_size_to_screen(object.rect.size);
                    let selected = self.selected == Some(index);
                    div()
                        .absolute()
                        .left(origin.x)
                        .top(origin.y)
                        .w(size.width)
                        .h(size.height)
                        .rotate(object.rotation)
                        .rounded(px(8.0))
                        .border_2()
                        .border_color(if selected {
                            rgb(0x2563eb)
                        } else {
                            rgb(0x00000000)
                        })
                        .bg(object.color)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_sm()
                        .text_color(rgb(0x0f172a))
                        .child(object.label)
                        .into_any_element()
                })
                .collect();

            let overlay = self.selection().map(|selection| {
                SelectionHandles::new(selection, viewport)
                    .rotate(true)
                    .into_any_element()
            });

            v_flex()
                .id("canvas-objects-demo")
                .track_focus(&self.focus_handle)
                .size_full()
                .bg(rgb(0xf8fafc))
                .child(
                    h_flex()
                        .gap(px(6.0))
                        .px(px(12.0))
                        .py(px(8.0))
                        .items_center()
                        .border_b_1()
                        .border_color(rgb(0xcbd5e1))
                        .child(
                            div()
                                .mr(px(6.0))
                                .text_sm()
                                .font_weight(rgpui::FontWeight::SEMIBOLD)
                                .child("画布对象工具"),
                        )
                        .child(Button::new("obj-left").label("左对齐").on_click(
                            cx.listener(|this, _, _, cx| this.apply_align(AlignMode::Left, cx)),
                        ))
                        .child(
                            Button::new("obj-hcenter")
                                .label("水平居中")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.apply_align(AlignMode::CenterHorizontal, cx)
                                })),
                        )
                        .child(Button::new("obj-right").label("右对齐").on_click(
                            cx.listener(|this, _, _, cx| this.apply_align(AlignMode::Right, cx)),
                        ))
                        .child(
                            Button::new("obj-distribute")
                                .label("水平分布")
                                .on_click(cx.listener(|this, _, _, cx| this.apply_distribute(cx))),
                        )
                        .child(
                            div()
                                .ml_auto()
                                .text_xs()
                                .text_color(rgb(0x64748b))
                                .child(format!(
                                    "缩放 {:.0}% · 选中 {}",
                                    viewport.zoom() * 100.0,
                                    self.selected.map(|i| self.objects[i].label).unwrap_or("无")
                                )),
                        ),
                )
                .child(
                    div()
                        .relative()
                        .flex_grow_1()
                        .min_h(px(0.0))
                        .overflow_hidden()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                this.on_mouse_down(event.position, cx);
                            }),
                        )
                        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                            this.on_mouse_move(event.position, cx);
                        }))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, _, cx| this.on_mouse_up(cx)),
                        )
                        .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                            this.viewport.apply_wheel(event, px(50.0));
                            cx.notify();
                        }))
                        .children(objects)
                        .children(overlay)
                        .child(
                            div()
                                .absolute()
                                .left(px(12.0))
                                .bottom(px(8.0))
                                .text_xs()
                                .text_color(rgb(0x94a3b8))
                                .child(match union {
                                    Some(u) => format!(
                                        "对象外接框：{:.0} × {:.0}",
                                        u.size.width.as_f32(),
                                        u.size.height.as_f32()
                                    ),
                                    None => String::new(),
                                }),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(920.0), px(680.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| CanvasObjectsDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| CanvasObjectsDemo::new(window, cx)).into()
    }
}

mod ruler {
    //! 标尺与对齐参考线演示：顶部标尺刻度 + 拖拽方块互相吸附并显示参考线。
    //!
    //! 刻度由 `rgpui::guides::ruler_ticks` 计算，吸附由 `rgpui::guides::align` 计算。
    //! `Ctrl/⌘` + 滚轮缩放标尺步长（仅影响标尺演示）。

    use rgpui::guides::{Guide, GuideAxis, align, ruler_ticks};
    use rgpui::{
        App, Bounds, Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
        Point, Render, ScrollWheelEvent, Window, WindowBounds, WindowOptions, bounds, canvas, div,
        fill, outline, point, prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    /// 标尺条高度。
    const RULER_HEIGHT: f32 = 28.0;
    /// 方块边长。
    const BOX: f32 = 80.0;
    /// 吸附阈值。
    const SNAP: f32 = 6.0;

    /// 演示视图。
    struct RulerDemo {
        zoom: f32,
        boxes: [Bounds<Pixels>; 2],
        dragging: Option<usize>,
        drag_offset: Point<Pixels>,
        guides: Vec<Guide>,
    }

    impl RulerDemo {
        fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
            Self {
                zoom: 1.0,
                boxes: [
                    bounds(point(px(160.0), px(160.0)), size(px(BOX), px(BOX))),
                    bounds(point(px(360.0), px(280.0)), size(px(BOX), px(BOX))),
                ],
                dragging: None,
                drag_offset: point(px(0.0), px(0.0)),
                guides: Vec::new(),
            }
        }
    }

    impl Render for RulerDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let zoom = self.zoom;
            let boxes = self.boxes;
            let guides = self.guides.clone();

            v_flex().size_full().child(
                div()
                    .size_full()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            for (index, rect) in this.boxes.iter().enumerate() {
                                if rect.contains(&event.position) {
                                    this.dragging = Some(index);
                                    this.drag_offset = event.position - rect.origin;
                                    break;
                                }
                            }
                            cx.notify();
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, cx| {
                            this.dragging = None;
                            this.guides.clear();
                            cx.notify();
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        let Some(index) = this.dragging else {
                            return;
                        };
                        let origin = event.position - this.drag_offset;
                        let mut rect = bounds(origin, size(px(BOX), px(BOX)));
                        let other = this.boxes[1 - index];
                        let result = align(rect, &[other], &[], &[], px(SNAP));
                        rect.origin += result.offset;
                        this.boxes[index] = rect;
                        this.guides = result.guides;
                        cx.notify();
                    }))
                    .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                        if event.modifiers.secondary() {
                            let dy = match event.delta {
                                rgpui::ScrollDelta::Lines(delta) => delta.y,
                                rgpui::ScrollDelta::Pixels(delta) => delta.y.as_f32() / 40.0,
                            };
                            this.zoom = (this.zoom * (1.0 + dy * 0.1)).clamp(0.25, 4.0);
                            cx.notify();
                        }
                    }))
                    .child(canvas(
                        |area, _window, _cx| area,
                        move |area, _state, window, _cx| {
                            window.paint_quad(fill(area, rgb(0xf8fafc)));

                            // 顶部标尺条。
                            let ruler =
                                bounds(area.origin, size(area.size.width, px(RULER_HEIGHT)));
                            window.paint_quad(fill(ruler, rgb(0xe2e8f0)));
                            window.paint_quad(outline(
                                ruler,
                                rgb(0xcbd5e1),
                                rgpui::BorderStyle::default(),
                            ));

                            // 刻度：可见文档区间 0..(宽度/zoom)。
                            let visible = area.size.width.as_f32() / zoom;
                            let ticks = ruler_ticks(0.0..visible, zoom, 60.0);
                            for (index, tick) in ticks.ticks.iter().enumerate() {
                                let x = area.origin.x + px(tick * zoom);
                                let major = index % 5 == 0;
                                let height = if major { 12.0 } else { 6.0 };
                                window.paint_quad(fill(
                                    bounds(
                                        point(x, area.origin.y + px(RULER_HEIGHT - height)),
                                        size(px(1.0), px(height)),
                                    ),
                                    rgb(0x64748b),
                                ));
                            }

                            // 方块。
                            for rect in boxes.iter() {
                                window.paint_quad(fill(*rect, rgb(0xdbeafe)));
                                window.paint_quad(outline(
                                    *rect,
                                    rgb(0x2563eb),
                                    rgpui::BorderStyle::default(),
                                ));
                            }

                            // 参考线。
                            for guide in guides.iter() {
                                let (line, color) = match guide.axis {
                                    GuideAxis::Vertical => (
                                        bounds(
                                            point(guide.position, area.origin.y),
                                            size(px(1.0), area.size.height),
                                        ),
                                        rgb(0xdc2626),
                                    ),
                                    GuideAxis::Horizontal => (
                                        bounds(
                                            point(area.origin.x, guide.position),
                                            size(area.size.width, px(1.0)),
                                        ),
                                        rgb(0xdc2626),
                                    ),
                                };
                                window.paint_quad(fill(line, color));
                            }
                        },
                    )),
            )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(820.0), px(560.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| RulerDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| RulerDemo::new(window, cx)).into()
    }
}

mod viewport {
    //! 视口演示：可缩放/平移的 A4 文档页视图。
    //!
    //! 页面由 `canvas` 自绘，坐标全部经 `rgpui::viewport::Viewport` 在
    //! 文档坐标与屏幕坐标间换算；滚轮 `Ctrl/⌘` 以光标为中心缩放、否则平移，
    //! 左键拖拽平移。

    use rgpui::viewport::Viewport;
    use rgpui::{
        App, Bounds, Button, Context, FocusHandle, MouseButton, MouseDownEvent, MouseMoveEvent,
        MouseUpEvent, Pixels, Point, Render, ScrollWheelEvent, Size, Window, WindowBounds,
        WindowOptions, bounds, canvas, div, fill, h_flex, outline, point, prelude::*, px, rgb,
        size, v_flex,
    };
    use rgpui_platform::application;

    /// A4 页面尺寸（96 DPI，像素）。
    fn page_size() -> Size<Pixels> {
        size(px(794.0), px(1123.0))
    }

    /// 页面边距。
    const MARGIN: f32 = 72.0;

    /// 演示视图。
    struct ViewportDemo {
        viewport: Viewport,
        focus_handle: FocusHandle,
        dragging: bool,
        last_pointer: Point<Pixels>,
        anchor: Point<Pixels>,
        view_size: Size<Pixels>,
    }

    impl ViewportDemo {
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            Self {
                viewport: Viewport::new()
                    .with_zoom(0.6)
                    .with_offset(point(px(80.0), px(24.0))),
                focus_handle: cx.focus_handle(),
                dragging: false,
                last_pointer: point(px(0.0), px(0.0)),
                anchor: point(px(400.0), px(300.0)),
                view_size: size(px(800.0), px(600.0)),
            }
        }
    }

    impl Render for ViewportDemo {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            // 记录视口可用尺寸，供「适应窗口」使用。
            self.view_size = window.bounds().size;

            // `Viewport` 为 `Copy`，复制一份供绘制闭包按值捕获。
            let viewport = self.viewport;
            let page = page_size();

            v_flex()
                .id("viewport-demo")
                .track_focus(&self.focus_handle)
                .size_full()
                .bg(rgb(0xf1f5f9))
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .px(px(12.0))
                        .py(px(8.0))
                        .items_center()
                        .border_b_1()
                        .border_color(rgb(0xcbd5e1))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(rgpui::FontWeight::SEMIBOLD)
                                .child("视口（Viewport）"),
                        )
                        .child(
                            Button::new("vp-zoom-out")
                                .label("缩小")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let anchor = this.anchor;
                                    this.viewport.zoom_out_at(anchor);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("vp-zoom-in")
                                .label("放大")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let anchor = this.anchor;
                                    this.viewport.zoom_in_at(anchor);
                                    cx.notify();
                                })),
                        )
                        .child(Button::new("vp-100").label("100%").on_click(cx.listener(
                            |this, _, _, cx| {
                                let anchor = this.anchor;
                                this.viewport.set_zoom_at(1.0, anchor);
                                cx.notify();
                            },
                        )))
                        .child(
                            Button::new("vp-fit")
                                .label("适应窗口")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let view = this.view_size;
                                    this.viewport.fit(page_size(), view, px(32.0));
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .ml_auto()
                                .text_xs()
                                .text_color(rgb(0x64748b))
                                .child(format!(
                                    "缩放 {:.0}% · 偏移 ({:.0}, {:.0})",
                                    viewport.zoom() * 100.0,
                                    viewport.offset().x.as_f32(),
                                    viewport.offset().y.as_f32()
                                )),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .relative()
                        .overflow_hidden()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                this.dragging = true;
                                this.last_pointer = event.position;
                                cx.notify();
                            }),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, _, cx| {
                                this.dragging = false;
                                cx.notify();
                            }),
                        )
                        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                            this.anchor = event.position;
                            if this.dragging {
                                let delta = event.position - this.last_pointer;
                                this.last_pointer = event.position;
                                this.viewport.pan_by(delta);
                            }
                            cx.notify();
                        }))
                        .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                            this.viewport.apply_wheel(event, px(50.0));
                            cx.notify();
                        }))
                        .child(canvas(
                            |bounds, _window, _cx| bounds,
                            move |area, _state, window, _cx| {
                                let to_screen = |x: f32, y: f32| -> Point<Pixels> {
                                    viewport.document_to_screen(point(px(x), px(y)))
                                };
                                let doc_rect = |x: f32, y: f32, w: f32, h: f32| -> Bounds<Pixels> {
                                    bounds(
                                        to_screen(x, y),
                                        viewport.document_size_to_screen(size(px(w), px(h))),
                                    )
                                };

                                window.paint_quad(fill(area, rgb(0xe2e8f0)));

                                let paper =
                                    doc_rect(0.0, 0.0, page.width.as_f32(), page.height.as_f32());
                                window.paint_quad(fill(paper, rgb(0xffffff)));
                                window.paint_quad(outline(
                                    paper,
                                    rgb(0x94a3b8),
                                    rgpui::BorderStyle::default(),
                                ));

                                let content_width = page.width.as_f32() - MARGIN * 2.0;
                                window.paint_quad(fill(
                                    doc_rect(MARGIN, MARGIN, content_width, 40.0),
                                    rgb(0x1e293b),
                                ));

                                // 用等宽色条模拟正文段落。
                                let mut y = MARGIN + 80.0;
                                let mut line = 0usize;
                                while y < page.height.as_f32() - MARGIN {
                                    let width = if line % 5 == 4 {
                                        content_width * 0.6
                                    } else {
                                        content_width
                                    };
                                    window.paint_quad(fill(
                                        doc_rect(MARGIN, y, width, 10.0),
                                        rgb(0xcbd5e1),
                                    ));
                                    y += 24.0;
                                    line += 1;
                                }
                            },
                        )),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(900.0), px(680.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| ViewportDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| ViewportDemo::new(window, cx)).into()
    }
}

mod formula_bar {
    //! 公式栏演示：名称框 + fx + 公式输入，绑定一个 4×5 的迷你「单元格」模型。
    //! 名称框输入 `B3` 跳转、公式框回车写入当前单元格、fx 插入 `=SUM(`、Esc 取消。

    use rgpui::formula_bar::{FormulaBar, FormulaBarEvent, FormulaBarState};
    use rgpui::{
        App, Bounds, Context, Entity, FocusHandle, IntoElement, ParentElement, Render, Styled,
        Window, WindowBounds, WindowOptions, div, h_flex, prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    /// 列数。
    const COLS: usize = 4;
    /// 行数。
    const ROWS: usize = 5;

    /// 演示视图。
    struct FormulaBarDemo {
        /// 公式栏状态。
        bar: Entity<FormulaBarState>,
        /// 迷你单元格内容（`[row][col]`）。
        cells: Vec<Vec<String>>,
        /// 当前活动单元格（row, col）。
        active: (usize, usize),
        /// 焦点句柄。
        focus_handle: FocusHandle,
        /// 状态栏文本。
        status: String,
    }

    /// 将列号（0 起）转为列名（A、B、…）。
    fn column_name(col: usize) -> String {
        let mut col = col;
        let mut name = String::new();
        loop {
            name.insert(0, (b'A' + (col % 26) as u8) as char);
            if col < 26 {
                break;
            }
            col = col / 26 - 1;
        }
        name
    }

    /// 单元格引用，如 `B3`。
    fn cell_ref(row: usize, col: usize) -> String {
        format!("{}{}", column_name(col), row + 1)
    }

    /// 解析 `B3` 形式的引用为 (row, col)。
    fn parse_ref(text: &str) -> Option<(usize, usize)> {
        let text = text.trim().to_ascii_uppercase();
        let col_end = text.find(|c: char| c.is_ascii_digit())?;
        let (letters, digits) = text.split_at(col_end);
        if letters.is_empty() || letters.len() > 2 {
            return None;
        }
        let mut col = 0usize;
        for ch in letters.chars() {
            if !ch.is_ascii_uppercase() {
                return None;
            }
            col = col * 26 + (ch as usize - 'A' as usize + 1);
        }
        let col = col.checked_sub(1)?;
        let row: usize = digits.parse().ok()?;
        if row == 0 || col >= COLS {
            return None;
        }
        Some((row - 1, col))
    }

    impl FormulaBarDemo {
        /// 创建演示视图。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let bar = cx.new(|cx| FormulaBarState::new(window, cx));
            // 展开态等变化时重绘。
            cx.observe(&bar, |_, _, cx| cx.notify()).detach();
            // 接收公式栏事件。
            cx.subscribe_in(
                &bar,
                window,
                |this, _, event: &FormulaBarEvent, window, cx| match event {
                    FormulaBarEvent::NameSubmit(name) => {
                        if let Some((row, col)) = parse_ref(name) {
                            this.select(row, col, window, cx);
                            this.status = format!("跳转到 {}", cell_ref(row, col));
                        } else {
                            this.status = format!("无法识别的引用：{name}");
                        }
                        cx.notify();
                    }
                    FormulaBarEvent::FormulaSubmit(text) => {
                        let (row, col) = this.active;
                        this.cells[row][col] = text.to_string();
                        this.status = format!("{} ← {text}", cell_ref(row, col));
                        cx.notify();
                    }
                    FormulaBarEvent::Cancel => {
                        let (row, col) = this.active;
                        let value = this.cells[row][col].clone();
                        let bar = this.bar.clone();
                        bar.update(cx, |bar, cx| bar.set_formula(value, window, cx));
                        this.status = "已取消编辑".to_string();
                        cx.notify();
                    }
                    FormulaBarEvent::InsertFunction => {
                        let current = this.bar.read(cx).formula(cx);
                        let next = if current.trim().is_empty() {
                            "=SUM(".to_string()
                        } else if current.starts_with('=') {
                            current.to_string()
                        } else {
                            format!("={current}")
                        };
                        let bar = this.bar.clone();
                        bar.update(cx, |bar, cx| bar.set_formula(next, window, cx));
                        this.status = "插入函数".to_string();
                        cx.notify();
                    }
                },
            )
            .detach();

            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            Self {
                bar,
                cells: vec![vec![String::new(); COLS]; ROWS],
                active: (0, 0),
                focus_handle,
                status: "选择单元格或使用公式栏编辑".to_string(),
            }
        }

        /// 选中单元格并同步公式栏显示。
        fn select(&mut self, row: usize, col: usize, window: &mut Window, cx: &mut Context<Self>) {
            self.active = (row, col);
            let value = self.cells[row][col].clone();
            let bar = self.bar.clone();
            bar.update(cx, |bar, cx| {
                bar.set_name(cell_ref(row, col), window, cx);
                bar.set_formula(value, window, cx);
            });
        }
    }

    impl Render for FormulaBarDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let header = h_flex()
                .gap(px(1.0))
                .child(div().w(px(36.0)))
                .children((0..COLS).map(|col| {
                    div()
                        .w(px(90.0))
                        .px(px(6.0))
                        .py(px(2.0))
                        .text_sm()
                        .font_weight(rgpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(0x475569))
                        .child(column_name(col))
                }));

            let rows = (0..ROWS).map(|row| {
                let cells = (0..COLS).map(|col| {
                    let is_active = self.active == (row, col);
                    div()
                        .id(("cell", row * COLS + col))
                        .w(px(90.0))
                        .h(px(26.0))
                        .px(px(6.0))
                        .flex()
                        .items_center()
                        .border_1()
                        .border_color(rgb(0xe2e8f0))
                        .text_sm()
                        .when(is_active, |this| {
                            this.border_color(rgb(0x2563eb)).bg(rgb(0xeff6ff))
                        })
                        .cursor_pointer()
                        .child(self.cells[row][col].clone())
                        .on_click(cx.listener(move |this, _event, window, cx| {
                            this.select(row, col, window, cx);
                            this.status = format!("选中 {}", cell_ref(row, col));
                            cx.notify();
                        }))
                });

                h_flex()
                    .gap(px(1.0))
                    .child(
                        div()
                            .w(px(36.0))
                            .h(px(26.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_sm()
                            .text_color(rgb(0x475569))
                            .child((row + 1).to_string()),
                    )
                    .children(cells)
            });

            v_flex()
                .id("formula-bar-demo")
                .key_context("FormulaBarDemo")
                .track_focus(&self.focus_handle)
                .size_full()
                .bg(rgb(0xffffff))
                .child(FormulaBar::new(&self.bar))
                .child(
                    v_flex()
                        .gap(px(1.0))
                        .p(px(12.0))
                        .child(header)
                        .children(rows),
                )
                .child(
                    div()
                        .px(px(12.0))
                        .py(px(6.0))
                        .text_sm()
                        .text_color(rgb(0x64748b))
                        .child(format!(
                            "活动单元格：{}    状态：{}",
                            cell_ref(self.active.0, self.active.1),
                            self.status
                        )),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(640.0), px(420.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| FormulaBarDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| FormulaBarDemo::new(window, cx)).into()
    }
}

mod sheet_grid {
    //! 电子表格网格演示：`SheetGrid` + `FormulaBar` 绑定一个 8×6 的迷你表格。
    //!
    //! 鼠标拖拽选区、Shift 扩展、行列头点击整行/整列、方向键/PageUp/PageDown 移动、
    //! Ctrl+A 全选、双击或 Enter/F2 请求编辑、Delete 清空、右下角填充柄向下/向右填充。
    //! 选区变化会同步到公式栏的名称框与公式框，公式栏回车写回活动单元格。

    use rgpui::formula_bar::{FormulaBar, FormulaBarEvent, FormulaBarState};
    use rgpui::sheet_grid::{
        CellRef, SheetDelegate, SheetGrid, SheetGridEvent, SheetGridState, column_label,
    };
    use rgpui::{
        App, Bounds, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
        ParentElement, Pixels, Render, SharedString, Styled, Window, WindowBounds, WindowOptions,
        div, h_flex, prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    /// 列数。
    const COLS: usize = 6;
    /// 行数。
    const ROWS: usize = 8;

    /// 单元格引用文本，如 `B3`。
    fn cell_ref(cell: CellRef) -> String {
        format!("{}{}", column_label(cell.col), cell.row + 1)
    }

    /// 解析 `B3` 形式的引用为单元格坐标。
    fn parse_ref(text: &str) -> Option<CellRef> {
        let text = text.trim().to_ascii_uppercase();
        let col_end = text.find(|c: char| c.is_ascii_digit())?;
        let (letters, digits) = text.split_at(col_end);
        if letters.is_empty() || letters.len() > 2 {
            return None;
        }
        let mut col = 0usize;
        for ch in letters.chars() {
            if !ch.is_ascii_uppercase() {
                return None;
            }
            col = col * 26 + (ch as usize - 'A' as usize + 1);
        }
        let col = col.checked_sub(1)?;
        let row: usize = digits.parse().ok()?;
        if row == 0 {
            return None;
        }
        Some(CellRef::new(row - 1, col))
    }

    /// 演示表格数据委托。
    struct DemoSheet {
        /// 单元格文本（`[row][col]`）。
        cells: Vec<Vec<String>>,
        /// 用户拖拽调整过的列宽（列号 → 像素）。
        col_widths: std::collections::BTreeMap<usize, Pixels>,
        /// 用户拖拽调整过的行高（行号 → 像素）。
        row_heights: std::collections::BTreeMap<usize, Pixels>,
    }

    impl DemoSheet {
        /// 创建预填数据的表格。
        fn new() -> Self {
            let mut cells = vec![vec![String::new(); COLS]; ROWS];
            cells[0][0] = "季度".to_string();
            cells[0][1] = "收入".to_string();
            cells[0][2] = "成本".to_string();
            cells[0][3] = "利润".to_string();
            for (row, line) in cells.iter_mut().enumerate().skip(1) {
                line[0] = format!("Q{row}");
                line[1] = (1200 * row).to_string();
                line[2] = (700 * row).to_string();
                line[3] = format!("=B{}-C{}", row + 1, row + 1);
            }
            Self {
                cells,
                col_widths: Default::default(),
                row_heights: Default::default(),
            }
        }

        /// 读取单元格文本。
        fn value(&self, cell: CellRef) -> String {
            self.cells
                .get(cell.row)
                .and_then(|row| row.get(cell.col))
                .cloned()
                .unwrap_or_default()
        }

        /// 写入单元格文本。
        fn set(&mut self, cell: CellRef, value: String) {
            if let Some(row) = self.cells.get_mut(cell.row)
                && let Some(slot) = row.get_mut(cell.col)
            {
                *slot = value;
            }
        }
    }

    impl SheetDelegate for DemoSheet {
        fn rows_count(&self, _cx: &App) -> usize {
            self.cells.len()
        }

        fn cols_count(&self, _cx: &App) -> usize {
            self.cells.first().map_or(0, Vec::len)
        }

        fn text(&self, cell: CellRef, _cx: &App) -> SharedString {
            self.value(cell).into()
        }

        fn column_width(&self, col: usize, _cx: &App) -> Pixels {
            self.col_widths.get(&col).copied().unwrap_or(px(96.0))
        }

        fn row_height(&self, row: usize, _cx: &App) -> Pixels {
            self.row_heights.get(&row).copied().unwrap_or(px(24.0))
        }
    }

    /// 演示视图。
    struct SheetGridDemo {
        /// 网格状态。
        grid: Entity<SheetGridState<DemoSheet>>,
        /// 公式栏状态。
        bar: Entity<FormulaBarState>,
        /// 焦点句柄。
        focus_handle: FocusHandle,
        /// 状态栏文本。
        status: String,
    }

    impl SheetGridDemo {
        /// 创建演示视图。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let bar = cx.new(|cx| FormulaBarState::new(window, cx));
            cx.observe(&bar, |_, _, cx| cx.notify()).detach();

            let grid = cx.new(|cx| SheetGridState::new(DemoSheet::new(), cx));
            cx.observe(&grid, |_, _, cx| cx.notify()).detach();

            // 网格事件 → 同步公式栏与数据。
            cx.subscribe_in(
                &grid,
                window,
                |this, _, event: &SheetGridEvent, window, cx| match event {
                    SheetGridEvent::SelectionChanged(range) => {
                        let cell = range.cursor;
                        let value = this.grid.read(cx).delegate().value(cell);
                        let bar = this.bar.clone();
                        bar.update(cx, |bar, cx| {
                            bar.set_name(cell_ref(cell), window, cx);
                            bar.set_formula(value, window, cx);
                        });
                        this.status = format!(
                            "选区 {}:{}（{} 格）",
                            cell_ref(range.min()),
                            cell_ref(range.max()),
                            range.len()
                        );
                        cx.notify();
                    }
                    SheetGridEvent::EditRequested(cell) => {
                        // 演示里没有单元格内输入框实体，传 None：网格进入编辑态但
                        // 单元格仍按普通格渲染（编辑经公式栏完成）。
                        this.grid
                            .update(cx, |grid, cx| grid.set_editing(Some(*cell), None, cx));
                        this.status = format!("编辑 {}（在公式栏输入后回车）", cell_ref(*cell));
                        cx.notify();
                    }
                    SheetGridEvent::Clear(range) => {
                        let value = String::new();
                        let cells: Vec<CellRef> = range
                            .rows()
                            .flat_map(|row| range.cols().map(move |col| CellRef::new(row, col)))
                            .collect();
                        this.grid.update(cx, |grid, cx| {
                            for cell in cells {
                                grid.delegate_mut().set(cell, value.clone());
                            }
                            cx.notify();
                        });
                        this.status = "已清空选区".to_string();
                        cx.notify();
                    }
                    SheetGridEvent::Fill { source, target } => {
                        let value = this.grid.read(cx).delegate().value(source.min());
                        let cells: Vec<CellRef> = target
                            .rows()
                            .flat_map(|row| target.cols().map(move |col| CellRef::new(row, col)))
                            .collect();
                        this.grid.update(cx, |grid, cx| {
                            for cell in cells {
                                grid.delegate_mut().set(cell, value.clone());
                            }
                            cx.notify();
                        });
                        this.status = format!(
                            "填充 {} → {}",
                            cell_ref(source.min()),
                            cell_ref(target.max())
                        );
                        cx.notify();
                    }
                    SheetGridEvent::ResetRowHeight { row } => {
                        // 双击行头分隔线：移除该行的自定义行高，回落默认值。
                        this.grid.update(cx, |grid, cx| {
                            grid.delegate_mut().row_heights.remove(row);
                            cx.notify();
                        });
                        this.status = format!("第 {} 行行高已重置", row + 1);
                        cx.notify();
                    }
                    SheetGridEvent::HeaderClicked { col, row } => {
                        this.status = match (row, col) {
                            (Some(row), _) => format!("选中第 {} 行", row + 1),
                            (_, Some(col)) => format!("选中 {} 列", column_label(*col)),
                            _ => "选中表头".to_string(),
                        };
                        cx.notify();
                    }
                    SheetGridEvent::ColumnResized { col, width } => {
                        this.grid.update(cx, |grid, cx| {
                            grid.delegate_mut().col_widths.insert(*col, *width);
                            cx.notify();
                        });
                        this.status =
                            format!("{} 列宽 {:.0}px", column_label(*col), width.as_f32());
                        cx.notify();
                    }
                    SheetGridEvent::RowResized { row, height } => {
                        this.grid.update(cx, |grid, cx| {
                            grid.delegate_mut().row_heights.insert(*row, *height);
                            cx.notify();
                        });
                        this.status = format!("第 {} 行行高 {:.0}px", row + 1, height.as_f32());
                        cx.notify();
                    }
                    SheetGridEvent::AutoFitColumn { col } => {
                        // 演示用估算：按最长文本的字符数近似（实际应用可用文本系统精确测量）。
                        let sheet = this.grid.read(cx).delegate();
                        let max_chars = (0..sheet.rows_count(cx))
                            .map(|row| sheet.value(CellRef::new(row, *col)).chars().count())
                            .max()
                            .unwrap_or(4) as f32;
                        let width = px((max_chars * 9.0 + 16.0).clamp(40.0, 400.0));
                        this.grid.update(cx, |grid, cx| {
                            grid.delegate_mut().col_widths.insert(*col, width);
                            cx.notify();
                        });
                        this.status =
                            format!("{} 列自动宽度 {:.0}px", column_label(*col), width.as_f32());
                        cx.notify();
                    }
                },
            )
            .detach();

            // 公式栏事件 → 写回网格。
            cx.subscribe_in(
                &bar,
                window,
                |this, _, event: &FormulaBarEvent, window, cx| match event {
                    FormulaBarEvent::NameSubmit(name) => {
                        if let Some(cell) = parse_ref(name) {
                            this.grid
                                .update(cx, |grid, cx| grid.set_active_cell(cell, cx));
                            this.status = format!("跳转到 {}", cell_ref(cell));
                        } else {
                            this.status = format!("无法识别的引用：{name}");
                        }
                        cx.notify();
                    }
                    FormulaBarEvent::FormulaSubmit(text) => {
                        let active = this.grid.read(cx).active_cell();
                        let text = text.to_string();
                        this.grid.update(cx, |grid, cx| {
                            grid.delegate_mut().set(active, text.clone());
                            grid.set_editing(None, None, cx);
                            cx.notify();
                        });
                        this.status = format!("{} ← {}", cell_ref(active), text);
                        cx.notify();
                    }
                    FormulaBarEvent::Cancel => {
                        let active = this.grid.read(cx).active_cell();
                        let value = this.grid.read(cx).delegate().value(active);
                        let bar = this.bar.clone();
                        bar.update(cx, |bar, cx| bar.set_formula(value, window, cx));
                        this.grid
                            .update(cx, |grid, cx| grid.set_editing(None, None, cx));
                        this.status = "已取消编辑".to_string();
                        cx.notify();
                    }
                    FormulaBarEvent::InsertFunction => {
                        let current = this.bar.read(cx).formula(cx);
                        let next = if current.trim().is_empty() || current.trim() == "=" {
                            "=SUM(".to_string()
                        } else if current.starts_with('=') {
                            current.to_string()
                        } else {
                            format!("={current}")
                        };
                        let bar = this.bar.clone();
                        bar.update(cx, |bar, cx| bar.set_formula(next, window, cx));
                        this.status = "插入函数".to_string();
                        cx.notify();
                    }
                },
            )
            .detach();

            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            Self {
                grid,
                bar,
                focus_handle,
                status: "拖拽选区 · 双击编辑 · Ctrl+A 全选 · 拖填充柄复制".to_string(),
            }
        }
    }

    impl Render for SheetGridDemo {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            v_flex()
                .id("sheet-grid-demo")
                .key_context("SheetGridDemo")
                .track_focus(&self.focus_handle)
                .size_full()
                .bg(rgb(0xffffff))
                .child(FormulaBar::new(&self.bar))
                .child(
                    div()
                        .flex_grow_1()
                        .min_h(px(0.0))
                        .border_t_1()
                        .border_color(rgb(0xe2e8f0))
                        .child(SheetGrid::new(&self.grid)),
                )
                .child(
                    h_flex()
                        .px(px(12.0))
                        .py(px(6.0))
                        .border_t_1()
                        .border_color(rgb(0xe2e8f0))
                        .text_sm()
                        .text_color(rgb(0x64748b))
                        .child(self.status.clone()),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(760.0), px(520.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| SheetGridDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| SheetGridDemo::new(window, cx)).into()
    }
}

mod app_suite {
    //! 应用套件基座整合演示：一份命令表驱动 Ribbon / 按键提示 / 快速访问工具栏 /
    //! 应用菜单（Backstage）/ 分页视口（PageView + Viewport + 参考线）/ 迷你工具栏，
    //! 并用「编号标注 + 组件清单」把画面上每一块对应到哪个套件基座模块直接标出来。
    //!
    //! 与单功能示例（`ribbon` / `qat` / `backstage` / `page_view`）不同，本例把套件基座的
    //! 各块拼成一个可用的编辑器骨架：每个入口只写命令 id，点击经 `App::dispatch_command`
    //! 派发到同一批 `on_action`，改动同一份文档模型；状态栏回显
    //! `CommandRegistry::label_for` 取出的标签，用来证明「UI 文案」与「命令清单」同源。
    //!
    //! 组件对照：屏幕上的 ①…⑨ 编号就是 `Region` 枚举（本文件内唯一真源，同时驱动浮标、
    //! 侧边编号条与右侧清单），悬停任一编号即给对应区域描蓝边；Shift + 左键拖框选唤出
    //! `MiniToolbar`（它的入口与 QAT 一样都是 `QatItem` + 命令 id）。
    //!
    //! 运行：`cargo run -p showcase --bin app_suite`（依赖 `ribbon` + `viewport` + `document`
    //! feature，showcase 默认开启）。按 Alt/F10 显示按键提示，再按提示键即可纯键盘操作功能区；
    //! `Ctrl+Shift+E` 开关区域浮标、`Ctrl+Shift+I` 开关右侧组件清单、Esc 取消框选；
    //! 缩放与命令注册表同源：`Ctrl+↑`/`Ctrl+↓` 放大缩小、`Ctrl+0` 实际大小、`Ctrl+Shift+F` 适应窗口。

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::backstage::{Backstage, BackstageState, BackstageTab};
    use rgpui::mini_toolbar::MiniToolbar;
    use rgpui::page_view::{
        PageArrangement, PageLabels, PageLayout, PageMargins, PageSetup, PageView,
    };
    use rgpui::qat::{QatItem, QuickAccessToolbar};
    use rgpui::ribbon::{
        Ribbon, RibbonContext, RibbonDropdownItem, RibbonGalleryItem, RibbonGroup, RibbonItem,
        RibbonState, RibbonTab,
    };
    use rgpui::viewport::Viewport;
    use rgpui::{
        App, Bounds, ClickEvent, CommandDescriptor, CommandRegistry, Context, Entity, FocusHandle,
        IconName, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
        Point, Render, ScrollWheelEvent, SharedString, Size, Window, WindowBounds, WindowOptions,
        actions, div, h_flex, point, prelude::*, px, register_command, relative, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    actions!(
        app_suite,
        [
            New,
            Open,
            Save,
            Print,
            Paste,
            PasteSpecial,
            Cut,
            Copy,
            Bold,
            StyleNormal,
            StyleTitle,
            StyleEmphasis,
            InsertTable,
            InsertPicture,
            NewPage,
            ZoomIn,
            ZoomOut,
            ZoomActual,
            ZoomFit,
            LayoutSingle,
            LayoutTwoUp,
            LayoutCover,
            ToggleMargins,
            ToggleGuides,
            CollapseRibbon,
            Recalc,
            ToggleCallouts,
            ToggleInspector
        ]
    );

    const NS: &str = "app_suite::";

    fn command(
        name: &str,
        label: &str,
        category: &str,
        keytip: Option<&str>,
        keystroke: Option<&str>,
    ) -> CommandDescriptor {
        let mut descriptor =
            CommandDescriptor::new(format!("{NS}{name}"), label).with_category(category);
        if let Some(keytip) = keytip {
            descriptor = descriptor.with_keytip(keytip);
        }
        if let Some(keystroke) = keystroke {
            descriptor = descriptor.with_keystroke(keystroke);
        }
        descriptor
    }

    fn cmd_new() -> CommandDescriptor {
        command("New", "新建", "文件", Some("N"), Some("ctrl-n"))
    }
    fn cmd_open() -> CommandDescriptor {
        command("打开", "打开", "文件", Some("O"), Some("ctrl-o"))
    }
    fn cmd_save() -> CommandDescriptor {
        command("保存", "保存", "文件", Some("S"), Some("ctrl-s"))
    }
    fn cmd_print() -> CommandDescriptor {
        command("Print", "打印", "文件", Some("U"), Some("ctrl-p"))
    }
    fn cmd_paste() -> CommandDescriptor {
        command("Paste", "粘贴", "剪贴板", Some("V"), Some("ctrl-v"))
    }
    fn cmd_paste_special() -> CommandDescriptor {
        command("PasteSpecial", "选择性粘贴", "剪贴板", None, None)
    }
    fn cmd_cut() -> CommandDescriptor {
        command("Cut", "剪切", "剪贴板", Some("X"), Some("ctrl-x"))
    }
    fn cmd_copy() -> CommandDescriptor {
        command("复制", "复制", "剪贴板", Some("C"), Some("ctrl-c"))
    }
    fn cmd_bold() -> CommandDescriptor {
        command("Bold", "加粗", "字体", Some("B"), Some("ctrl-b"))
    }
    fn cmd_style_normal() -> CommandDescriptor {
        command("StyleNormal", "常规", "样式", None, None)
    }
    fn cmd_style_title() -> CommandDescriptor {
        command("StyleTitle", "标题", "样式", None, None)
    }
    fn cmd_style_emphasis() -> CommandDescriptor {
        command("StyleEmphasis", "强调", "样式", None, None)
    }
    fn cmd_insert_table() -> CommandDescriptor {
        command("InsertTable", "表格", "插入", Some("T"), None)
    }
    fn cmd_insert_picture() -> CommandDescriptor {
        command("InsertPicture", "图片", "插入", Some("I"), None)
    }
    fn cmd_new_page() -> CommandDescriptor {
        command("NewPage", "分页", "插入", Some("G"), None)
    }
    fn cmd_zoom_in() -> CommandDescriptor {
        command("ZoomIn", "放大", "视图", Some("Z"), Some("ctrl-up"))
    }
    fn cmd_zoom_out() -> CommandDescriptor {
        command("ZoomOut", "缩小", "视图", Some("Q"), Some("ctrl-down"))
    }
    fn cmd_zoom_actual() -> CommandDescriptor {
        command("ZoomActual", "100%", "视图", Some("A"), Some("ctrl-0"))
    }
    fn cmd_zoom_fit() -> CommandDescriptor {
        command(
            "ZoomFit",
            "适应窗口",
            "视图",
            Some("F"),
            Some("ctrl-shift-f"),
        )
    }
    fn cmd_layout_single() -> CommandDescriptor {
        command("LayoutSingle", "单页", "视图", None, None)
    }
    fn cmd_layout_two_up() -> CommandDescriptor {
        command("LayoutTwoUp", "对开", "视图", None, None)
    }
    fn cmd_layout_cover() -> CommandDescriptor {
        command("LayoutCover", "封面对开", "视图", None, None)
    }
    fn cmd_toggle_margins() -> CommandDescriptor {
        command("ToggleMargins", "窄边距", "视图", Some("M"), None)
    }
    fn cmd_toggle_guides() -> CommandDescriptor {
        command("ToggleGuides", "内容参考线", "视图", Some("D"), None)
    }
    fn cmd_collapse_ribbon() -> CommandDescriptor {
        command("CollapseRibbon", "折叠功能区", "视图", Some("R"), None)
    }
    fn cmd_recalc() -> CommandDescriptor {
        command("Recalc", "重新计算", "数据", Some("L"), Some("f9"))
    }
    fn cmd_toggle_callouts() -> CommandDescriptor {
        command(
            "ToggleCallouts",
            "组件标注",
            "视图",
            Some("E"),
            Some("ctrl-shift-e"),
        )
    }
    fn cmd_toggle_inspector() -> CommandDescriptor {
        command(
            "ToggleInspector",
            "组件清单",
            "视图",
            Some("W"),
            Some("ctrl-shift-i"),
        )
    }

    register_command!(cmd_new);
    register_command!(cmd_open);
    register_command!(cmd_save);
    register_command!(cmd_print);
    register_command!(cmd_paste);
    register_command!(cmd_paste_special);
    register_command!(cmd_cut);
    register_command!(cmd_copy);
    register_command!(cmd_bold);
    register_command!(cmd_style_normal);
    register_command!(cmd_style_title);
    register_command!(cmd_style_emphasis);
    register_command!(cmd_insert_table);
    register_command!(cmd_insert_picture);
    register_command!(cmd_new_page);
    register_command!(cmd_zoom_in);
    register_command!(cmd_zoom_out);
    register_command!(cmd_zoom_actual);
    register_command!(cmd_zoom_fit);
    register_command!(cmd_layout_single);
    register_command!(cmd_layout_two_up);
    register_command!(cmd_layout_cover);
    register_command!(cmd_toggle_margins);
    register_command!(cmd_toggle_guides);
    register_command!(cmd_collapse_ribbon);
    register_command!(cmd_recalc);
    register_command!(cmd_toggle_callouts);
    register_command!(cmd_toggle_inspector);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum DocStyle {
        /// 常规正文。
        Normal,
        /// 标题（更大更深）。
        Title,
        /// 强调（琥珀色）。
        Emphasis,
    }

    enum Block {
        /// 一段正文。
        Paragraph(SharedString),
        /// 表格：行名与数值，`total` 为 `None` 表示尚未重算。
        Table {
            rows: Vec<(SharedString, f64)>,
            total: Option<f64>,
        },
        /// 图片占位（不做真实解码，只演示块级内容）。
        Picture(SharedString),
    }

    struct Page {
        /// 本页内容块。
        blocks: Vec<Block>,
    }

    fn sample_table() -> Block {
        const VALUES: [(&str, f64); 4] = [
            ("一月", 120.0),
            ("二月", 180.0),
            ("三月", 90.0),
            ("四月", 240.0),
        ];
        Block::Table {
            rows: VALUES
                .iter()
                .map(|(name, value)| (SharedString::from(*name), *value))
                .collect(),
            total: None,
        }
    }

    fn starter_pages() -> Vec<Page> {
        vec![
            Page {
                blocks: vec![
                    Block::Paragraph("应用套件基座 · 季度报表".into()),
                    Block::Paragraph(
                        "顶部三处入口（快速访问工具栏、功能区、文件菜单）都只写命令 id，\
                     落到同一批动作处理上；改一处命令清单，三处 UI 同步生效。"
                            .into(),
                    ),
                    sample_table(),
                ],
            },
            Page {
                blocks: vec![Block::Paragraph(
                    "第二页：插入选项卡的「表格 / 图片 / 分页」改动的就是这份模型；\
                 视图选项卡的缩放与排布只改视口和分页设置，不动内容。"
                        .into(),
                )],
            },
        ]
    }

    struct SuiteLabels {
        /// 文档标题。
        title: SharedString,
        /// 总页数。
        pages: usize,
    }

    impl PageLabels for SuiteLabels {
        fn header(&self, _page: usize, _cx: &App) -> SharedString {
            self.title.clone()
        }

        fn footer(&self, page: usize, _cx: &App) -> SharedString {
            format!("第 {} / {} 页", page + 1, self.pages).into()
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Region {
        /// 快速访问工具栏。
        Qat,
        /// 应用菜单（Backstage）。
        Backstage,
        /// 功能区。
        Ribbon,
        /// 按键提示。
        KeyTips,
        /// 分页画布。
        PageView,
        /// 视口（缩放 / 平移）。
        Viewport,
        /// 页面内容参考线（`PageView` 的虚线框开关）。
        Guides,
        /// 迷你工具栏。
        MiniToolbar,
        /// 命令注册表。
        Commands,
    }

    impl Region {
        /// 中文模块名（浮标与清单标题）。
        fn title(self) -> &'static str {
            match self {
                Region::Qat => "快速访问工具栏",
                Region::Backstage => "应用菜单",
                Region::Ribbon => "功能区",
                Region::KeyTips => "按键提示",
                Region::PageView => "分页画布",
                Region::Viewport => "视口",
                Region::Guides => "内容参考线",
                Region::MiniToolbar => "迷你工具栏",
                Region::Commands => "命令注册表",
            }
        }

        /// 模块名（类型 / 函数名）。
        fn module(self) -> &'static str {
            match self {
                Region::Qat => "QuickAccessToolbar",
                Region::Backstage => "Backstage",
                Region::Ribbon => "Ribbon",
                Region::KeyTips => "KeyTips",
                Region::PageView => "PageView",
                Region::Viewport => "Viewport",
                Region::Guides => "PageView::content_guide",
                Region::MiniToolbar => "MiniToolbar",
                Region::Commands => "CommandRegistry",
            }
        }

        /// 源文件与所属 feature。
        fn source(self) -> &'static str {
            match self {
                Region::Qat => "qat.rs · ribbon",
                Region::Backstage => "backstage.rs · ribbon",
                Region::Ribbon => "ribbon.rs · ribbon",
                Region::KeyTips => "keytips.rs · ribbon",
                Region::PageView => "page_view.rs · document",
                Region::Viewport => "viewport.rs · viewport",
                Region::Guides => "page_view.rs · document",
                Region::MiniToolbar => "mini_toolbar.rs · ribbon",
                Region::Commands => "commands.rs · 核心",
            }
        }

        /// 在本示例里的触发方式。
        fn trigger(self) -> &'static str {
            match self {
                Region::Qat => "顶栏图标条，每项只写命令 id",
                Region::Backstage => "点顶栏「文件」开合整窗菜单",
                Region::Ribbon => "四个选项卡 + 「数据」上下文选项卡",
                Region::KeyTips => "按 Alt 或 F10 显示按键提示，再按字母",
                Region::PageView => "画布上的纸页、页眉页脚与页间距",
                Region::Viewport => "滚轮缩放、左键拖拽平移、视图 › 适应窗口",
                Region::Guides => "视图 › 内容参考线（虚线框即内容区）",
                Region::MiniToolbar => "Shift + 左键拖框选，命令条就近浮出",
                Region::Commands => "状态栏回显 label_for，条数即注册表规模",
            }
        }

        /// 带圈编号（按画面顺序：顶栏 → 功能区 → 画布 → 状态栏，与 [`REGIONS`] 一致）。
        fn badge(self) -> &'static str {
            match self {
                Region::Backstage => "①",
                Region::Qat => "②",
                Region::Ribbon => "③",
                Region::KeyTips => "④",
                Region::PageView => "⑤",
                Region::Viewport => "⑥",
                Region::Guides => "⑦",
                Region::MiniToolbar => "⑧",
                Region::Commands => "⑨",
            }
        }
    }

    const REGIONS: [Region; 9] = [
        Region::Backstage,
        Region::Qat,
        Region::Ribbon,
        Region::KeyTips,
        Region::PageView,
        Region::Viewport,
        Region::Guides,
        Region::MiniToolbar,
        Region::Commands,
    ];

    struct AppSuite {
        /// 文档页。
        pages: Vec<Page>,
        /// 当前段落样式。
        style: DocStyle,
        /// 是否加粗。
        bold: bool,
        /// 是否有未保存改动。
        dirty: bool,
        /// 分页设置（纸张 / 边距 / 页眉页脚 / 页间距）。
        setup: PageSetup,
        /// 页面排布方式。
        arrangement: PageArrangement,
        /// 是否显示内容区参考线。
        guides: bool,
        /// 是否显示组件标注（编号浮标 + 区域描边）。
        callouts: bool,
        /// 是否显示右侧组件清单面板。
        inspector: bool,
        /// 当前被高亮的组件（清单悬停时给出）。
        highlighted: Option<Region>,
        /// 框选区间（窗口坐标，Shift + 左键拖拽）。
        selection: Option<(Point<Pixels>, Point<Pixels>)>,
        /// 是否正在框选。
        marquee: bool,
        /// 视口（缩放 + 平移）。
        viewport: Viewport,
        /// 功能区状态。
        ribbon: Entity<RibbonState>,
        /// 应用菜单状态。
        backstage: Entity<BackstageState>,
        /// 焦点句柄。
        focus_handle: FocusHandle,
        /// 最近执行的命令回显。
        last: SharedString,
        /// 画布可用尺寸（供适应窗口使用）。
        view_size: Size<Pixels>,
        /// 缩放锚点（最近鼠标位置）。
        anchor: Point<Pixels>,
        /// 是否正在拖拽平移。
        dragging: bool,
        /// 上次指针位置。
        last_pointer: Point<Pixels>,
    }

    impl AppSuite {
        /// 创建演示视图：建子状态、聚焦自身。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let ribbon = cx.new(RibbonState::new);
            let backstage = cx.new(BackstageState::new);
            // 子状态变化（切选项卡、开合应用菜单）都要重绘本视图。
            cx.observe(&ribbon, |_, _, cx| cx.notify()).detach();
            cx.observe(&backstage, |_, _, cx| cx.notify()).detach();
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            Self {
                pages: starter_pages(),
                style: DocStyle::Normal,
                bold: false,
                dirty: false,
                setup: PageSetup {
                    margins: PageMargins::NORMAL,
                    ..PageSetup::default()
                },
                arrangement: PageArrangement::Single,
                guides: true,
                callouts: true,
                inspector: true,
                highlighted: None,
                selection: None,
                marquee: false,
                viewport: Viewport::new()
                    .with_zoom(0.5)
                    .with_offset(point(px(64.0), px(16.0))),
                ribbon,
                backstage,
                focus_handle,
                last: "（尚未执行命令）".into(),
                view_size: size(px(960.0), px(600.0)),
                anchor: point(px(480.0), px(300.0)),
                dragging: false,
                last_pointer: point(px(0.0), px(0.0)),
            }
        }

        /// 记录一次命令执行：标签取自注册表，与 UI 文案同源。
        fn record(&mut self, name: &str, cx: &mut Context<Self>) {
            let id = format!("{NS}{name}");
            let label = CommandRegistry::global().label_for(&id);
            self.last = format!("{label}（{id}）").into();
            cx.notify();
        }

        /// 标记文档已改动。
        fn touch(&mut self) {
            self.dirty = true;
        }

        /// 往当前页追加一个块（演示口径：始终编辑最后一页）。
        fn push_block(&mut self, block: Block) {
            let page = self.pages.len().saturating_sub(1);
            self.pages[page].blocks.push(block);
            self.touch();
        }

        /// 打开 / 关闭应用菜单，并把焦点交给面板（Esc 才生效）。
        fn toggle_backstage(&mut self, window: &mut Window, cx: &mut Context<Self>) {
            self.backstage.update(cx, |state, cx| state.toggle(cx));
            let focus = self.backstage.read(cx).focus_handle();
            window.focus(&focus, cx);
        }

        /// 待重算的表格数量（状态栏提示用）。
        fn stale_tables(&self) -> usize {
            self.pages
                .iter()
                .flat_map(|page| page.blocks.iter())
                .filter(|block| matches!(block, Block::Table { total: None, .. }))
                .count()
        }

        /// 依当前设置与页数构造分页布局。
        fn layout(&self) -> PageLayout {
            PageLayout::new(self.setup, self.pages.len()).with_arrangement(self.arrangement)
        }

        /// 排布方式的中文标签。
        fn arrangement_label(&self) -> &'static str {
            match self.arrangement {
                PageArrangement::Single => "单页",
                PageArrangement::TwoUp => "对开",
                PageArrangement::TwoUpCover => "封面对开",
            }
        }

        /// 组件标注浮标：编号 + 中文模块名，高亮时补出源文件与 feature。
        ///
        /// `slot` 用来区分同一组件在不同位置的浮标（顶栏 / 功能区 / 标注条 / 清单），
        /// 避免元素 id 冲突；悬停浮标即把对应组件设为高亮。
        fn tag(&self, region: Region, slot: &str, entity: Entity<Self>) -> rgpui::AnyElement {
            let active = self.highlighted == Some(region);
            h_flex()
                .id(format!("tag-{slot}-{}", region.module()))
                .flex_shrink_0()
                .items_center()
                .gap(px(4.0))
                .px(px(6.0))
                .py(px(2.0))
                .rounded_full()
                .cursor_pointer()
                .text_xs()
                .bg(if active { rgb(0x1d4ed8) } else { rgb(0x475569) })
                .text_color(rgb(0xf8fafc))
                .on_hover(move |hovered, _window, app| {
                    entity.update(app, |this, cx| {
                        this.highlighted = if *hovered { Some(region) } else { None };
                        cx.notify();
                    });
                })
                .child(region.badge())
                .child(region.title())
                .when(active, |this| this.child(format!("· {}", region.source())))
                .into_any_element()
        }

        /// 编号方块：只占一个方块宽，挂在各区域的标注轨道上（悬停即高亮该区域）。
        fn chip(&self, region: Region, entity: Entity<Self>) -> rgpui::AnyElement {
            let active = self.highlighted == Some(region);
            div()
                .id(format!("chip-{}", region.module()))
                .flex_shrink_0()
                .w(px(22.0))
                .h(px(20.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .cursor_pointer()
                .text_xs()
                .bg(if active { rgb(0x2563eb) } else { rgb(0x334155) })
                .text_color(rgb(0xf8fafc))
                .on_hover(move |hovered, _window, app| {
                    entity.update(app, |this, cx| {
                        this.highlighted = if *hovered { Some(region) } else { None };
                        cx.notify();
                    });
                })
                .child(region.badge())
                .into_any_element()
        }

        /// 竖向标注轨道：把若干区域的编号方块贴在对应区域旁边。
        fn rail(&self, regions: &[Region], entity: Entity<Self>) -> rgpui::AnyElement {
            v_flex()
                .w(px(30.0))
                .flex_shrink_0()
                .items_center()
                .gap(px(3.0))
                .py(px(3.0))
                .bg(rgb(0x0f172a))
                .children(
                    regions
                        .iter()
                        .map(|region| self.chip(*region, entity.clone())),
                )
                .into_any_element()
        }

        /// 标注说明行：悬停时给出该组件的源文件、feature 与触发方式，
        /// 平时给出整套标注操作口径（浮标 / 清单 / 迷你工具栏）。
        fn hint_line(&self) -> rgpui::AnyElement {
            let text = match self.highlighted {
                Some(region) => format!(
                    "{} {} · {} —— {}",
                    region.badge(),
                    region.title(),
                    region.source(),
                    region.trigger()
                ),
                None => "编号方块 = 组件标注：悬停方块或右侧清单行即高亮对应区域；\
                 Ctrl+Shift+E 开关浮标，Ctrl+Shift+I 开关清单；Shift + 左键拖框选唤出迷你工具栏。"
                    .to_string(),
            };
            h_flex()
                .w_full()
                .flex_shrink_0()
                .px(px(10.0))
                .py(px(3.0))
                .bg(rgb(0x0f172a))
                .text_xs()
                .text_color(rgb(0xcbd5e1))
                .child(text)
                .into_any_element()
        }

        /// 右侧组件清单：编号 / 模块 / 源文件与 feature / 触发方式，悬停行高亮对应区域。
        fn inspector_panel(&self, entity: Entity<Self>) -> rgpui::AnyElement {
            v_flex()
                .id("suite-inspector")
                .w(px(288.0))
                .flex_shrink_0()
                .border_l_1()
                .border_color(rgb(0xcbd5e1))
                .bg(rgb(0xf8fafc))
                .px(px(10.0))
                .py(px(8.0))
                .gap(px(5.0))
                .overflow_y_scroll()
                .child(
                    div()
                        .text_sm()
                        .font_weight(rgpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(0x0f172a))
                        .child("本演示用到的套件基座组件"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x64748b))
                        .child("悬停任意一行 → 界面上对应区域点亮描边；Ctrl+Shift+I 开关本面板。"),
                )
                .children(REGIONS.iter().map(|region| {
                    let region = *region;
                    let active = self.highlighted == Some(region);
                    let entity = entity.clone();
                    v_flex()
                        .id(format!("insp-{}", region.module()))
                        .w_full()
                        .flex_shrink_0()
                        .gap(px(1.0))
                        .px(px(6.0))
                        .py(px(4.0))
                        .rounded_md()
                        .cursor_pointer()
                        .border_1()
                        .border_color(if active { rgb(0x2563eb) } else { rgb(0xe2e8f0) })
                        .bg(if active { rgb(0xdbeafe) } else { rgb(0xffffff) })
                        .on_hover(move |hovered, _window, app| {
                            entity.update(app, |this, cx| {
                                this.highlighted = if *hovered { Some(region) } else { None };
                                cx.notify();
                            });
                        })
                        .child(
                            h_flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .w(px(18.0))
                                        .h(px(18.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_full()
                                        .text_xs()
                                        .bg(if active { rgb(0x2563eb) } else { rgb(0x64748b) })
                                        .text_color(rgb(0xffffff))
                                        .child(region.badge()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(rgpui::FontWeight::SEMIBOLD)
                                        .text_color(rgb(0x0f172a))
                                        .child(region.title()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0x475569))
                                        .child(region.module()),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x64748b))
                                .child(region.source()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x334155))
                                .child(region.trigger()),
                        )
                }))
                .child(div().text_xs().text_color(rgb(0x64748b)).child(
                    "未在本文档区演示：公式栏 / 电子表格网格 → showcase --bin spreadsheet；\
                         控制通道 / MCP → showcase --bin automation。",
                ))
                .into_any_element()
        }

        /// 迷你工具栏的落点：框选终点右下偏移，避免压住选区。
        fn toolbar_at(selection: (Point<Pixels>, Point<Pixels>)) -> Point<Pixels> {
            point(selection.1.x + px(6.0), selection.1.y + px(20.0))
        }

        /// 功能区选项卡：所有条目只写命令 id，标签由命令描述符提供。
        /// 按键提示在整块功能区里必须互不相同（前缀解析按字符匹配候选项），
        /// 选项卡用 H/J/N/K，条目用其余字母。
        fn ribbon_tabs(&self, collapsed: bool) -> Vec<RibbonTab> {
            vec![
                RibbonTab::new("home", "开始")
                    .keytip("H")
                    .group(
                        RibbonGroup::new("剪贴板")
                            .item(
                                RibbonItem::split("app_suite::Paste")
                                    .label("粘贴")
                                    .keytip("V")
                                    .icon(IconName::Copy)
                                    .large()
                                    .item(
                                        RibbonDropdownItem::new("app_suite::PasteSpecial")
                                            .label("选择性粘贴"),
                                    ),
                            )
                            .item(
                                RibbonItem::button("app_suite::Cut")
                                    .label("剪切")
                                    .icon(IconName::Delete),
                            )
                            .item(
                                RibbonItem::button("app_suite::Copy")
                                    .label("复制")
                                    .icon(IconName::Copy),
                            ),
                    )
                    .group(
                        RibbonGroup::new("字体").item(
                            RibbonItem::toggle("app_suite::Bold")
                                .label("加粗")
                                .keytip("B")
                                .icon(IconName::ALargeSmall)
                                .checked(self.bold),
                        ),
                    )
                    .group(
                        RibbonGroup::new("样式").item(
                            RibbonItem::gallery("样式库")
                                .keytip("Y")
                                .columns(1)
                                .large()
                                .icon(IconName::Palette)
                                .item(
                                    RibbonGalleryItem::new("app_suite::StyleNormal")
                                        .label("常规")
                                        .icon(IconName::File)
                                        .selected(self.style == DocStyle::Normal),
                                )
                                .item(
                                    RibbonGalleryItem::new("app_suite::StyleTitle")
                                        .label("标题")
                                        .icon(IconName::Star)
                                        .selected(self.style == DocStyle::Title),
                                )
                                .item(
                                    RibbonGalleryItem::new("app_suite::StyleEmphasis")
                                        .label("强调")
                                        .icon(IconName::Palette)
                                        .selected(self.style == DocStyle::Emphasis),
                                ),
                        ),
                    ),
                RibbonTab::new("insert", "插入").keytip("J").group(
                    RibbonGroup::new("插图")
                        .item(
                            RibbonItem::button("app_suite::InsertTable")
                                .label("表格")
                                .keytip("T")
                                .icon(IconName::LayoutDashboard)
                                .large(),
                        )
                        .item(
                            RibbonItem::button("app_suite::InsertPicture")
                                .label("图片")
                                .keytip("I")
                                .icon(IconName::GalleryVerticalEnd),
                        )
                        .item(
                            RibbonItem::button("app_suite::NewPage")
                                .label("分页")
                                .keytip("G")
                                .icon(IconName::BookOpen),
                        ),
                ),
                RibbonTab::new("view", "视图")
                    .keytip("N")
                    .group(
                        RibbonGroup::new("显示")
                            .item(
                                RibbonItem::button("app_suite::ZoomIn")
                                    .label("放大")
                                    .keytip("Z")
                                    .icon(IconName::Plus),
                            )
                            .item(
                                RibbonItem::button("app_suite::ZoomOut")
                                    .label("缩小")
                                    .keytip("Q")
                                    .icon(IconName::Minus),
                            )
                            .item(
                                RibbonItem::button("app_suite::ZoomActual")
                                    .label("100%")
                                    .keytip("A")
                                    .icon(IconName::Search),
                            )
                            .item(
                                RibbonItem::button("app_suite::ZoomFit")
                                    .label("适应窗口")
                                    .keytip("F")
                                    .icon(IconName::Frame),
                            ),
                    )
                    .group(
                        RibbonGroup::new("页面")
                            .item(
                                RibbonItem::dropdown("排布")
                                    .keytip("P")
                                    .icon(IconName::PanelLeft)
                                    .item(RibbonDropdownItem::new("app_suite::LayoutSingle"))
                                    .item(RibbonDropdownItem::new("app_suite::LayoutTwoUp"))
                                    .item(RibbonDropdownItem::new("app_suite::LayoutCover")),
                            )
                            .item(
                                RibbonItem::toggle("app_suite::ToggleMargins")
                                    .label("窄边距")
                                    .keytip("M")
                                    .icon(IconName::ResizeCorner)
                                    .checked(self.setup.margins == PageMargins::NARROW),
                            )
                            .item(
                                RibbonItem::toggle("app_suite::ToggleGuides")
                                    .label("参考线")
                                    .keytip("D")
                                    .icon(IconName::Eye)
                                    .checked(self.guides),
                            )
                            .item(
                                RibbonItem::toggle("app_suite::CollapseRibbon")
                                    .label("折叠功能区")
                                    .keytip("R")
                                    .icon(IconName::WindowMinimize)
                                    .checked(collapsed),
                            ),
                    )
                    .group(
                        RibbonGroup::new("演示标注")
                            .item(
                                RibbonItem::toggle("app_suite::ToggleCallouts")
                                    .label("区域浮标")
                                    .keytip("E")
                                    .icon(IconName::Eye)
                                    .checked(self.callouts),
                            )
                            .item(
                                RibbonItem::toggle("app_suite::ToggleInspector")
                                    .label("组件清单")
                                    .keytip("W")
                                    .icon(IconName::PanelRight)
                                    .checked(self.inspector),
                            ),
                    ),
                RibbonTab::new("data", "数据")
                    .keytip("K")
                    .group(
                        RibbonGroup::new("计算").item(
                            RibbonItem::button("app_suite::Recalc")
                                .label("重新计算")
                                .keytip("L")
                                .icon(IconName::Redo)
                                .large(),
                        ),
                    )
                    .context(RibbonContext::new("表格工具", rgb(0x0e7490))),
            ]
        }

        /// 应用菜单页签：打开 / 保存 / 打印直接绑命令 id，新建页是自定义内容。
        fn backstage_tabs(&self) -> Vec<BackstageTab> {
            vec![
                BackstageTab::new("new", "新建")
                    .icon(IconName::Plus)
                    .content(|_window, _cx| {
                        v_flex()
                            .gap(px(8.0))
                            .child(div().text_lg().child("新建文档"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x64748b))
                                    .child("下面这条走的是与功能区、QAT 完全相同的派发路径。"),
                            )
                            .child(
                                div()
                                    .id("suite-new")
                                    .w_full()
                                    .px(px(12.0))
                                    .py(px(8.0))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_sm()
                                    .bg(rgb(0xf1f5f9))
                                    .hover(|style| style.bg(rgb(0xe2e8f0)))
                                    .child("空白文档（app_suite::New）")
                                    .on_click(|_event: &ClickEvent, _window, cx| {
                                        if let Err(error) =
                                            cx.dispatch_command("app_suite::New", None)
                                        {
                                            log::warn!("派发命令失败：{error}");
                                        }
                                    }),
                            )
                            .into_any_element()
                    }),
                BackstageTab::new("open", "打开")
                    .icon(IconName::FolderOpen)
                    .command("app_suite::Open"),
                BackstageTab::new("save", "保存")
                    .icon(IconName::Check)
                    .command("app_suite::Save"),
                BackstageTab::new("print", "打印")
                    .icon(IconName::File)
                    .command("app_suite::Print"),
                BackstageTab::new("options", "选项")
                    .icon(IconName::Settings)
                    .content(|_window, _cx| {
                        v_flex()
                            .gap(px(8.0))
                            .child(div().text_lg().child("选项"))
                            .child(div().text_sm().text_color(rgb(0x64748b)).child(
                                "本演示的开关（窄边距 / 参考线 / 折叠功能区）都做成了命令，\
                                 在视图选项卡里；选项页可承载任意表单。",
                            ))
                            .into_any_element()
                    }),
            ]
        }

        /// 正文叠加层：按 `content_rect` 换算到屏幕，字号随缩放同步。
        fn overlays(&self, zoom: f32) -> Vec<rgpui::AnyElement> {
            let layout = self.layout();
            let viewport = self.viewport;
            let font_size = px((15.0 * zoom).max(4.0));
            let title_size = px((22.0 * zoom).max(6.0));
            let weight = if self.bold {
                rgpui::FontWeight::BOLD
            } else {
                rgpui::FontWeight::NORMAL
            };
            let paragraph = match self.style {
                DocStyle::Normal => (font_size, rgb(0x1e293b)),
                DocStyle::Title => (title_size, rgb(0x0f172a)),
                DocStyle::Emphasis => (font_size, rgb(0xb45309)),
            };
            let gap = px((10.0 * zoom).max(2.0));
            let cell = px((12.0 * zoom).max(2.0));

            let mut overlays = Vec::new();
            for (index, page) in self.pages.iter().enumerate().take(layout.page_count()) {
                let content = layout.content_rect(index);
                let origin = viewport.document_to_screen(content.origin);
                let scaled = viewport.document_size_to_screen(content.size);
                let blocks = page.blocks.iter().map(|block| match block {
                    Block::Paragraph(text) => div()
                        .w_full()
                        .text_size(paragraph.0)
                        .font_weight(weight)
                        .line_height(relative(1.5))
                        .text_color(paragraph.1)
                        .child(text.clone())
                        .into_any_element(),
                    Block::Table { rows, total } => v_flex()
                        .w_full()
                        .border_1()
                        .border_color(rgb(0x94a3b8))
                        .children(rows.iter().map(|(name, value)| {
                            h_flex()
                                .w_full()
                                .justify_between()
                                .px(cell)
                                .text_size(font_size)
                                .text_color(rgb(0x334155))
                                .child(name.clone())
                                .child(format!("{value:.0}"))
                        }))
                        .child(
                            h_flex()
                                .w_full()
                                .justify_between()
                                .px(cell)
                                .font_weight(rgpui::FontWeight::SEMIBOLD)
                                .text_size(font_size)
                                .text_color(rgb(0x0f172a))
                                .bg(rgb(0xf1f5f9))
                                .child("合计")
                                .child(match total {
                                    Some(sum) => format!("{sum:.0}"),
                                    None => "待重算".to_string(),
                                }),
                        )
                        .into_any_element(),
                    Block::Picture(name) => div()
                        .w_full()
                        .py(px(8.0))
                        .text_center()
                        .text_size(font_size)
                        .text_color(rgb(0x475569))
                        .border_1()
                        .border_color(rgb(0xcbd5e1))
                        .bg(rgb(0xe2e8f0))
                        .child(name.clone())
                        .into_any_element(),
                });
                overlays.push(
                    v_flex()
                        .absolute()
                        .left(origin.x)
                        .top(origin.y)
                        .w(scaled.width)
                        .h(scaled.height)
                        .gap(gap)
                        .children(blocks)
                        .into_any_element(),
                );
            }
            overlays
        }

        fn on_new(&mut self, _: &New, _window: &mut Window, cx: &mut Context<Self>) {
            self.pages = vec![Page {
                blocks: vec![Block::Paragraph("空白文档".into())],
            }];
            self.style = DocStyle::Normal;
            self.bold = false;
            self.dirty = false;
            self.record("New", cx);
        }

        fn on_open(&mut self, _: &Open, _window: &mut Window, cx: &mut Context<Self>) {
            self.push_block(Block::Paragraph("（已载入示例文档）季度复盘要点".into()));
            self.record("打开", cx);
        }

        fn on_save(&mut self, _: &Save, _window: &mut Window, cx: &mut Context<Self>) {
            self.dirty = false;
            self.record("保存", cx);
        }

        fn on_print(&mut self, _: &Print, _window: &mut Window, cx: &mut Context<Self>) {
            let pages = self.pages.len();
            self.last = format!("已提交 {pages} 页到打印队列（{NS}Print）").into();
            cx.notify();
        }

        fn on_paste(&mut self, _: &Paste, _window: &mut Window, cx: &mut Context<Self>) {
            self.push_block(Block::Paragraph("粘贴自剪贴板的正文段落。".into()));
            self.record("Paste", cx);
        }

        fn on_paste_special(
            &mut self,
            _: &PasteSpecial,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.push_block(Block::Paragraph(
                "选择性粘贴：保留源格式的一段文字。".into(),
            ));
            self.record("PasteSpecial", cx);
        }

        fn on_cut(&mut self, _: &Cut, _window: &mut Window, cx: &mut Context<Self>) {
            self.push_block(Block::Paragraph("（剪切走了当前段落）".into()));
            self.record("Cut", cx);
        }

        fn on_copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
            self.record("复制", cx);
        }

        fn on_bold(&mut self, _: &Bold, _window: &mut Window, cx: &mut Context<Self>) {
            self.bold = !self.bold;
            self.touch();
            self.record("Bold", cx);
        }

        fn on_style_normal(
            &mut self,
            _: &StyleNormal,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.style = DocStyle::Normal;
            self.touch();
            self.record("StyleNormal", cx);
        }

        fn on_style_title(&mut self, _: &StyleTitle, _window: &mut Window, cx: &mut Context<Self>) {
            self.style = DocStyle::Title;
            self.touch();
            self.record("StyleTitle", cx);
        }

        fn on_style_emphasis(
            &mut self,
            _: &StyleEmphasis,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.style = DocStyle::Emphasis;
            self.touch();
            self.record("StyleEmphasis", cx);
        }

        fn on_insert_table(
            &mut self,
            _: &InsertTable,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.push_block(sample_table());
            self.record("InsertTable", cx);
        }

        fn on_insert_picture(
            &mut self,
            _: &InsertPicture,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.push_block(Block::Picture("示意图 · 季度趋势".into()));
            self.record("InsertPicture", cx);
        }

        fn on_new_page(&mut self, _: &NewPage, _window: &mut Window, cx: &mut Context<Self>) {
            self.pages.push(Page {
                blocks: vec![Block::Paragraph("新的一页".into())],
            });
            self.touch();
            self.record("NewPage", cx);
        }

        fn on_zoom_in(&mut self, _: &ZoomIn, _window: &mut Window, cx: &mut Context<Self>) {
            let anchor = self.anchor;
            self.viewport.zoom_in_at(anchor);
            self.record("ZoomIn", cx);
        }

        fn on_zoom_out(&mut self, _: &ZoomOut, _window: &mut Window, cx: &mut Context<Self>) {
            let anchor = self.anchor;
            self.viewport.zoom_out_at(anchor);
            self.record("ZoomOut", cx);
        }

        fn on_zoom_actual(&mut self, _: &ZoomActual, _window: &mut Window, cx: &mut Context<Self>) {
            let anchor = self.anchor;
            self.viewport.set_zoom_at(1.0, anchor);
            self.record("ZoomActual", cx);
        }

        fn on_zoom_fit(&mut self, _: &ZoomFit, window: &mut Window, cx: &mut Context<Self>) {
            let view = window.bounds().size;
            let document = self.layout().document_size();
            self.viewport.fit(document, view, px(24.0));
            self.record("ZoomFit", cx);
        }

        fn on_layout_single(
            &mut self,
            _: &LayoutSingle,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.arrangement = PageArrangement::Single;
            self.record("LayoutSingle", cx);
        }

        fn on_layout_two_up(
            &mut self,
            _: &LayoutTwoUp,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.arrangement = PageArrangement::TwoUp;
            self.record("LayoutTwoUp", cx);
        }

        fn on_layout_cover(
            &mut self,
            _: &LayoutCover,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.arrangement = PageArrangement::TwoUpCover;
            self.record("LayoutCover", cx);
        }

        fn on_toggle_margins(
            &mut self,
            _: &ToggleMargins,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.setup.margins = if self.setup.margins == PageMargins::NORMAL {
                PageMargins::NARROW
            } else {
                PageMargins::NORMAL
            };
            self.record("ToggleMargins", cx);
        }

        fn on_toggle_guides(
            &mut self,
            _: &ToggleGuides,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.guides = !self.guides;
            self.record("ToggleGuides", cx);
        }

        fn on_collapse_ribbon(
            &mut self,
            _: &CollapseRibbon,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.ribbon
                .update(cx, |state, cx| state.toggle_collapsed(cx));
            self.record("CollapseRibbon", cx);
        }

        fn on_toggle_callouts(
            &mut self,
            _: &ToggleCallouts,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.callouts = !self.callouts;
            if !self.callouts {
                self.highlighted = None;
            }
            self.record("ToggleCallouts", cx);
        }

        fn on_toggle_inspector(
            &mut self,
            _: &ToggleInspector,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.inspector = !self.inspector;
            if !self.inspector {
                self.highlighted = None;
            }
            self.record("ToggleInspector", cx);
        }

        fn on_recalc(&mut self, _: &Recalc, _window: &mut Window, cx: &mut Context<Self>) {
            let mut filled = 0usize;
            for page in &mut self.pages {
                for block in &mut page.blocks {
                    if let Block::Table { rows, total } = block {
                        let sum = rows.iter().map(|(_, value)| *value).sum::<f64>();
                        if total.replace(sum).is_none() {
                            filled += 1;
                        }
                    }
                }
            }
            self.last = format!("已重算 {filled} 张表格（{NS}Recalc）").into();
            cx.notify();
        }
    }

    impl Render for AppSuite {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            if !self.dragging {
                self.view_size = window.bounds().size;
            }

            let viewport = self.viewport;
            let zoom = viewport.zoom();
            let page_count = self.pages.len();
            let overlays = self.overlays(zoom);
            let last = self.last.clone();
            let dirty = if self.dirty { "未保存" } else { "已保存" };
            let margins = if self.setup.margins == PageMargins::NARROW {
                "窄边距"
            } else {
                "常规边距"
            };
            let stale = self.stale_tables();
            let arrangement = self.arrangement_label();
            let collapsed = self.ribbon.read(cx).is_collapsed();
            let tabs = self.ribbon_tabs(collapsed);
            let backstage_tabs = self.backstage_tabs();
            // 高亮判定：清单 / 方块悬停中落在哪块区域，就给对应区域描蓝边。
            let header_hot = matches!(
                self.highlighted,
                Some(Region::Backstage) | Some(Region::Qat)
            );
            let ribbon_hot = matches!(
                self.highlighted,
                Some(Region::Ribbon) | Some(Region::KeyTips)
            );
            let canvas_hot = matches!(
                self.highlighted,
                Some(Region::PageView)
                    | Some(Region::Viewport)
                    | Some(Region::Guides)
                    | Some(Region::MiniToolbar)
            );
            let status_hot = self.highlighted == Some(Region::Commands);
            let selection = self.selection;
            // 悬停高亮走实体句柄：浮标与清单行都通过它改写 `highlighted`。
            let entity = cx.entity();

            v_flex()
                .id("app-suite")
                .key_context("Ribbon")
                .track_focus(&self.focus_handle)
                .relative()
                .size_full()
                .bg(rgb(0xffffff))
                .on_action(cx.listener(Self::on_new))
                .on_action(cx.listener(Self::on_open))
                .on_action(cx.listener(Self::on_save))
                .on_action(cx.listener(Self::on_print))
                .on_action(cx.listener(Self::on_paste))
                .on_action(cx.listener(Self::on_paste_special))
                .on_action(cx.listener(Self::on_cut))
                .on_action(cx.listener(Self::on_copy))
                .on_action(cx.listener(Self::on_bold))
                .on_action(cx.listener(Self::on_style_normal))
                .on_action(cx.listener(Self::on_style_title))
                .on_action(cx.listener(Self::on_style_emphasis))
                .on_action(cx.listener(Self::on_insert_table))
                .on_action(cx.listener(Self::on_insert_picture))
                .on_action(cx.listener(Self::on_new_page))
                .on_action(cx.listener(Self::on_zoom_in))
                .on_action(cx.listener(Self::on_zoom_out))
                .on_action(cx.listener(Self::on_zoom_actual))
                .on_action(cx.listener(Self::on_zoom_fit))
                .on_action(cx.listener(Self::on_layout_single))
                .on_action(cx.listener(Self::on_layout_two_up))
                .on_action(cx.listener(Self::on_layout_cover))
                .on_action(cx.listener(Self::on_toggle_margins))
                .on_action(cx.listener(Self::on_toggle_guides))
                .on_action(cx.listener(Self::on_collapse_ribbon))
                .on_action(cx.listener(Self::on_recalc))
                .on_action(cx.listener(Self::on_toggle_callouts))
                .on_action(cx.listener(Self::on_toggle_inspector))
                .on_key_down(
                    cx.listener(|this, event: &rgpui::KeyDownEvent, _window, cx| {
                        // Esc 取消框选（迷你工具栏随之收起）。
                        if event.keystroke.key == "escape" && this.selection.take().is_some() {
                            cx.notify();
                        }
                    }),
                )
                // 顶栏：① 文件入口（Backstage）+ ② 快速访问工具栏 + 保存状态。
                .child(
                    h_flex()
                        .w_full()
                        .flex_shrink_0()
                        .items_center()
                        .gap(px(6.0))
                        .px(px(8.0))
                        .py(px(4.0))
                        .bg(rgb(0x1e293b))
                        .border_b_2()
                        .border_color(if header_hot {
                            rgb(0x38bdf8)
                        } else {
                            rgb(0x1e293b)
                        })
                        .when(self.callouts, |this| {
                            this.child(self.tag(Region::Backstage, "head", entity.clone()))
                        })
                        .child(
                            div()
                                .id("file-entry")
                                .px(px(12.0))
                                .py(px(4.0))
                                .rounded_md()
                                .cursor_pointer()
                                .text_xs()
                                .text_color(rgb(0xf8fafc))
                                .bg(rgb(0x334155))
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    this.toggle_backstage(window, cx);
                                }))
                                .child("文件"),
                        )
                        .when(self.callouts, |this| {
                            this.child(self.tag(Region::Qat, "head", entity.clone()))
                        })
                        .child(
                            QuickAccessToolbar::new()
                                .item(QatItem::button("app_suite::New").icon(IconName::Plus))
                                .item(QatItem::button("app_suite::Open").icon(IconName::FolderOpen))
                                .item(QatItem::button("app_suite::Save").icon(IconName::Check))
                                .item(QatItem::separator())
                                .item(QatItem::button("app_suite::Cut").icon(IconName::Delete))
                                .item(QatItem::button("app_suite::Copy").icon(IconName::Copy))
                                .item(QatItem::button("app_suite::Paste").label("粘贴")),
                        )
                        .child(
                            div()
                                .ml_auto()
                                .text_xs()
                                .text_color(rgb(0xcbd5e1))
                                .child(format!("{dirty} · {page_count} 页")),
                        ),
                )
                .child(
                    // 功能区（③）与按键提示（④）：左侧编号轨道 + 高亮描边。
                    h_flex()
                        .w_full()
                        .flex_shrink_0()
                        .items_stretch()
                        .when(self.callouts, |this| {
                            this.child(
                                self.rail(&[Region::Ribbon, Region::KeyTips], entity.clone()),
                            )
                        })
                        .child(
                            div()
                                .flex_grow_1()
                                .min_w(px(0.0))
                                .border_2()
                                .border_color(if ribbon_hot {
                                    rgb(0x2563eb)
                                } else {
                                    rgb(0xe2e8f0)
                                })
                                .child(Ribbon::new(&self.ribbon, tabs)),
                        ),
                )
                // 内容区：标注轨道（⑤⑥⑦⑧）+ 分页画布 + 视口 + 右侧组件清单。
                // 滚轮 Ctrl/⌘ 缩放、左键拖拽平移（视口）；Shift + 左键拖框选（迷你工具栏）。
                .child(
                    h_flex()
                        .w_full()
                        .flex_grow_1()
                        .min_h(px(0.0))
                        .items_stretch()
                        .when(self.callouts, |this| {
                            this.child(self.rail(
                                &[
                                    Region::PageView,
                                    Region::Viewport,
                                    Region::Guides,
                                    Region::MiniToolbar,
                                ],
                                entity.clone(),
                            ))
                        })
                        .child(
                            div()
                                .relative()
                                .flex_grow_1()
                                .min_w(px(0.0))
                                .min_h(px(0.0))
                                .overflow_hidden()
                                .border_2()
                                .border_color(if canvas_hot {
                                    rgb(0x2563eb)
                                } else {
                                    rgb(0xe2e8f0)
                                })
                                .bg(rgb(0xf1f5f9))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                        if event.modifiers.shift {
                                            this.marquee = true;
                                            this.selection = Some((event.position, event.position));
                                        } else {
                                            this.dragging = true;
                                            this.last_pointer = event.position;
                                        }
                                        cx.notify();
                                    }),
                                )
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(|this, _: &MouseUpEvent, _, cx| {
                                        this.dragging = false;
                                        this.marquee = false;
                                        cx.notify();
                                    }),
                                )
                                .on_mouse_move(cx.listener(
                                    |this, event: &MouseMoveEvent, _, cx| {
                                        this.anchor = event.position;
                                        if this.marquee {
                                            if let Some((start, _)) = this.selection {
                                                this.selection = Some((start, event.position));
                                            }
                                        } else if this.dragging {
                                            let delta = event.position - this.last_pointer;
                                            this.last_pointer = event.position;
                                            this.viewport.pan_by(delta);
                                        }
                                        cx.notify();
                                    },
                                ))
                                .on_scroll_wheel(cx.listener(
                                    |this, event: &ScrollWheelEvent, _, cx| {
                                        this.viewport.apply_wheel(event, px(50.0));
                                        cx.notify();
                                    },
                                ))
                                .child(
                                    div().absolute().top_0().left_0().size_full().child(
                                        PageView::new(
                                            self.layout(),
                                            viewport,
                                            SuiteLabels {
                                                title: "rgpui 应用套件基座".into(),
                                                pages: page_count,
                                            },
                                        )
                                        .content_guide(self.guides)
                                        .area_guide(self.guides),
                                    ),
                                )
                                .children(overlays),
                        )
                        .when(self.inspector, |this| {
                            this.child(self.inspector_panel(entity.clone()))
                        }),
                )
                .when(self.callouts, |this| this.child(self.hint_line()))
                // 状态栏：⑨ 命令注册表 + 最近命令、命令清单规模、待重算表格、视口读数。
                .child(
                    h_flex()
                        .w_full()
                        .flex_shrink_0()
                        .items_center()
                        .gap(px(12.0))
                        .px(px(12.0))
                        .py(px(6.0))
                        .border_t_1()
                        .border_color(rgb(0xcbd5e1))
                        .bg(if status_hot {
                            rgb(0xdbeafe)
                        } else {
                            rgb(0xf8fafc)
                        })
                        .text_xs()
                        .text_color(rgb(0x475569))
                        .child(format!("最近命令：{last}"))
                        .child(format!(
                            "命令清单 {} 条 · 待重算 {stale} 张 · 排布 {arrangement} · {margins}",
                            CommandRegistry::global().len()
                        ))
                        .when(self.callouts, |this| {
                            this.child(div().ml_auto().child(self.tag(
                                Region::Commands,
                                "status",
                                entity.clone(),
                            )))
                        })
                        .child(div().ml_auto().child(format!("缩放 {:.0}%", zoom * 100.0))),
                )
                // 框选矩形与迷你工具栏：鼠标事件给的是窗口坐标，所以挂在根容器上。
                .children(selection.map(|(start, end)| {
                    div()
                        .absolute()
                        .left(start.x.min(end.x))
                        .top(start.y.min(end.y))
                        .w((start.x - end.x).abs())
                        .h((start.y - end.y).abs())
                        .border_1()
                        .border_color(rgb(0x2563eb))
                        .bg(rgb(0x60a5fa).opacity(0.25))
                }))
                .children(selection.map(|selection| {
                    MiniToolbar::new()
                        .at(Self::toolbar_at(selection))
                        .item(QatItem::button("app_suite::Bold").label("B"))
                        .item(QatItem::button("app_suite::StyleTitle").label("标题"))
                        .item(QatItem::separator())
                        .item(QatItem::button("app_suite::Copy").icon(IconName::Copy))
                }))
                .child(Backstage::new(&self.backstage, backstage_tabs).sidebar_width(px(150.0)))
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            // 快捷键与注册表同源：描述符里的 `with_keystroke` 在这里逐条落到键映射上。
            cx.bind_keys([
                KeyBinding::new("ctrl-n", New, None),
                KeyBinding::new("ctrl-o", Open, None),
                KeyBinding::new("ctrl-s", Save, None),
                KeyBinding::new("ctrl-p", Print, None),
                KeyBinding::new("ctrl-v", Paste, None),
                KeyBinding::new("ctrl-x", Cut, None),
                KeyBinding::new("ctrl-c", Copy, None),
                KeyBinding::new("ctrl-b", Bold, None),
                KeyBinding::new("ctrl-up", ZoomIn, None),
                KeyBinding::new("ctrl-down", ZoomOut, None),
                KeyBinding::new("ctrl-0", ZoomActual, None),
                KeyBinding::new("ctrl-shift-f", ZoomFit, None),
                KeyBinding::new("f9", Recalc, None),
                KeyBinding::new("ctrl-shift-e", ToggleCallouts, None),
                KeyBinding::new("ctrl-shift-i", ToggleInspector, None),
            ]);
            let bounds = Bounds::centered(None, size(px(1040.0), px(780.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| AppSuite::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| AppSuite::new(window, cx)).into()
    }
}

mod charts {
    //! 图表组件示例（`charts` feature 门控）。
    //!
    //! 展示柱状图、折线图、饼图/环形图的使用方法。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin charts --features charts
    //! ```

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::components::charts::{
        BarChart, BarChartData, LineChart, LineChartPoint, LineChartSeries, PieChart,
        PieChartSegment,
    };
    use rgpui::{
        App, AppContext as _, Bounds, Context, IntoElement, ParentElement, Render,
        ScrollableElement, Styled, Window, WindowBounds, WindowOptions, div, px, size,
    };
    use rgpui_platform::application;

    struct ChartsApp;

    impl Render for ChartsApp {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let bars = vec![
                BarChartData::new("一月", 30.0),
                BarChartData::new("二月", 55.0),
                BarChartData::new("三月", 42.0),
                BarChartData::new("四月", 78.0),
                BarChartData::new("五月", 63.0),
            ];

            let line_series = LineChartSeries::new(
                "访问量",
                vec![
                    LineChartPoint::new(0.0, 10.0),
                    LineChartPoint::new(1.0, 18.0),
                    LineChartPoint::new(2.0, 12.0),
                    LineChartPoint::new(3.0, 30.0),
                    LineChartPoint::new(4.0, 24.0),
                    LineChartPoint::new(5.0, 40.0),
                ],
            )
            .show_points(true);

            let pie_segments = vec![
                PieChartSegment::new("Chrome", 45.0),
                PieChartSegment::new("Safari", 25.0),
                PieChartSegment::new("Firefox", 18.0),
                PieChartSegment::new("其他", 12.0),
            ];

            div()
                .size_full()
                .overflow_y_scrollbar()
                .p(px(24.0))
                .flex()
                .flex_col()
                .gap_6()
                .child(div().text_2xl().child("图表组件示例"))
                .child(
                    div().text_lg().child("柱状图（BarChart）").child(
                        BarChart::new(bars)
                            .chart_height(px(260.0))
                            .show_values(true),
                    ),
                )
                .child(
                    div()
                        .text_lg()
                        .child("折线图（LineChart）")
                        .child(LineChart::new(vec![line_series]).show_grid(true)),
                )
                .child(
                    div().text_lg().child("饼图（PieChart）").child(
                        PieChart::pie(pie_segments)
                            .size_px(220)
                            .show_percentages(true),
                    ),
                )
                .child(
                    div().text_lg().child("环形图（PieChart donut）").child(
                        PieChart::donut(vec![
                            PieChartSegment::new("产品 A", 40.0),
                            PieChartSegment::new("产品 B", 35.0),
                            PieChartSegment::new("产品 C", 25.0),
                        ])
                        .size_px(200)
                        .center_label("总计")
                        .show_percentages(true),
                    ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(900.), px(800.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| ChartsApp),
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| ChartsApp).into()
    }
}

mod effects {
    //! 特效组件示例（`effects` feature 门控）。
    //!
    //! 展示极光背景、彩带、粒子发射器、涟漪、微光、跑马灯、脉冲指示器。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin effects --features effects
    //! ```

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::components::{
        Aurora, Confetti, ConfettiState, Marquee, ParticleEmitter, ParticleEmitterState,
        PulseIndicator, Ripple, Shimmer,
    };
    use rgpui::{
        App, AppContext as _, Bounds, Context, Entity, InteractiveElement, IntoElement,
        ParentElement, Render, StatefulInteractiveElement, Styled, Window, WindowBounds,
        WindowOptions, div, point, px, size,
    };
    use rgpui_platform::application;

    struct EffectsApp {
        /// 彩带状态实体。
        confetti_state: Entity<ConfettiState>,
        /// 粒子发射器状态实体。
        particle_state: Entity<ParticleEmitterState>,
    }

    impl EffectsApp {
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            let confetti_state = cx.new(ConfettiState::new);
            let particle_state = cx.new(ParticleEmitterState::new);
            Self {
                confetti_state,
                particle_state,
            }
        }
    }

    impl Render for EffectsApp {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let confetti_state = self.confetti_state.clone();
            let particle_state = self.particle_state.clone();

            Aurora::new()
                .colors(vec![
                    rgpui::hsla(0.55, 0.9, 0.6, 0.6),
                    rgpui::hsla(0.8, 0.9, 0.6, 0.5),
                    rgpui::hsla(0.1, 0.9, 0.7, 0.5),
                ])
                .child(
                    div()
                        .size_full()
                        .p(px(24.0))
                        .flex()
                        .flex_col()
                        .gap_6()
                        .child(div().text_2xl().child("特效组件示例"))
                        .child(
                            div().child("彩带（Confetti，点击触发）").child(
                                div()
                                    .id("confetti-trigger")
                                    .w(px(200.0))
                                    .h(px(60.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(8.0))
                                    .bg(rgpui::hsla(0.0, 0.0, 0.0, 0.25))
                                    .text_color(rgpui::white())
                                    .cursor_pointer()
                                    .on_click(cx.listener(
                                        move |this: &mut EffectsApp,
                                              _: &rgpui::ClickEvent,
                                              _window,
                                              cx| {
                                            this.confetti_state
                                                .update(cx, |state, cx| state.burst(cx));
                                        },
                                    ))
                                    .child("点击放彩带"),
                            ),
                        )
                        .child(
                            Confetti::new("confetti", confetti_state)
                                .particle_count(80, cx)
                                .gravity(0.6, cx),
                        )
                        .child(
                            div().child("粒子发射器（ParticleEmitter）").child(
                                ParticleEmitter::new("particles", particle_state)
                                    .spawn_rate(20.0, cx)
                                    .lifetime(std::time::Duration::from_secs(2), cx),
                            ),
                        )
                        .child(
                            div().child("微光（Shimmer）").child(
                                div()
                                    .w(px(260.0))
                                    .h(px(48.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(8.0))
                                    .bg(rgpui::hsla(0.0, 0.0, 0.0, 0.2))
                                    .child(Shimmer::new().child("加载中…")),
                            ),
                        )
                        .child(
                            div().child("跑马灯（Marquee）").child(
                                div()
                                    .w(px(400.0))
                                    .child(Marquee::new("marquee", || {
                                        div()
                                            .child("这是一条跑马灯文本，用于展示滚动横幅效果")
                                            .into_any_element()
                                    }))
                                    .child(div().h(px(8.0)))
                                    .child(
                                        Marquee::new("marquee-reverse", || {
                                            div()
                                                .child("反向滚动：从左到右展示内容")
                                                .into_any_element()
                                        })
                                        .direction(rgpui::components::MarqueeDirection::Right),
                                    ),
                            ),
                        )
                        .child(
                            div().child("脉冲指示器（PulseIndicator）").child(
                                div()
                                    .flex()
                                    .gap_4()
                                    .child(PulseIndicator::new("pulse-1"))
                                    .child(
                                        PulseIndicator::new("pulse-2")
                                            .color(rgpui::red())
                                            .size(px(24.0)),
                                    ),
                            ),
                        )
                        .child(div().child("涟漪（Ripple）").child(Ripple::new(
                            "ripple",
                            point(px(20.0), px(20.0)),
                            rgpui::white(),
                        ))),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(900.), px(1200.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| EffectsApp::new(window, cx)),
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| EffectsApp::new(window, cx)).into()
    }
}

mod qr_code {
    //! 二维码组件示例（`qr-code` feature 门控）。
    //!
    //! 展示二维码的生成与自定义样式。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin qr_code --features qr-code
    //! ```

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::components::QRCodeComponent;
    use rgpui::{
        App, AppContext as _, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window,
        WindowBounds, WindowOptions, div, px, size,
    };
    use rgpui_platform::application;

    struct QrCodeApp;

    impl Render for QrCodeApp {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .p(px(24.0))
                .flex()
                .flex_col()
                .gap_6()
                .child(div().text_2xl().child("二维码组件示例"))
                .child(
                    div()
                        .text_lg()
                        .child("默认二维码")
                        .child(QRCodeComponent::new("https://github.com/").size(px(180.0))),
                )
                .child(
                    div().text_lg().child("自定义颜色").child(
                        QRCodeComponent::new("rgpui 二维码")
                            .size(px(180.0))
                            .fg_color(rgpui::hsla(0.6, 0.8, 0.45, 1.0))
                            .bg_color(rgpui::white()),
                    ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(600.), px(720.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| QrCodeApp),
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| QrCodeApp).into()
    }
}

mod editor {
    //! Editor 演示：单个 `EditorState` + `Editor` 全家桶。
    //!
    //! 大纲/搜索面板/右键追加档/LSP/片段/inlay/粘性顶栏/语言切换全接这一个编辑器
    //! （搜索面板 `attach_editor` 接内部输入；右键经 `Editor::context_menu_extra`
    //! 追加转大写，默认菜单开箱即用）。tree-sitter 高亮/折叠 + 行操作 + 多光标
    //!（键位来自全局 `init_all` 默认注册）。

    #![cfg_attr(target_family = "wasm", no_main)]

    use std::rc::Rc;

    use rgpui::{
        App, Bounds, ClickEvent, Context, MouseButton, Pixels, Point, PopupMenuItem, Render,
        SharedString, Switch, Window, WindowBounds, WindowOptions, anchored, blue,
        components::SearchPanelState,
        deferred, div, green, h_flex,
        input_ui::{
            CodeActionProvider, CodeLens, CodeLensOverlay, CodeLensProvider, Editor, EditorState,
            InputEvent, StickyPosition,
        },
        lsp::{
            CompletionProvider, DefinitionProvider, DiagnosticEntry, DiagnosticsProvider,
            FormattingProvider, HoverContent, HoverProvider, HoverResponse, RenameProvider,
            SemanticTokensProvider, SignatureHelpProvider,
        },
        prelude::*,
        px, rgb, size, v_flex, white, yellow,
    };
    use rgpui_platform::application;

    const SAMPLE: &str = "fn main() {\n    let name = \"rgpui\";\n    println!(\"hello, {name}\");\n}\n\nstruct Point {\n    x: f32,\n    y: f32,\n}\n\nimpl Point {\n    fn len(&self) -> f32 {\n        (self.x * self.x + self.y * self.y).sqrt()\n    }\n}\n";

    struct DemoCompletionProvider;

    const DEMO_WORDS: &[(&str, lsp_types::CompletionItemKind, &str)] = &[
        (
            "println",
            lsp_types::CompletionItemKind::FUNCTION,
            "宏 · 打印换行",
        ),
        (
            "print",
            lsp_types::CompletionItemKind::FUNCTION,
            "宏 · 打印不换行",
        ),
        ("private", lsp_types::CompletionItemKind::KEYWORD, "关键字"),
        ("pub", lsp_types::CompletionItemKind::KEYWORD, "关键字"),
        ("Point", lsp_types::CompletionItemKind::STRUCT, "演示结构体"),
        ("len", lsp_types::CompletionItemKind::METHOD, "方法"),
    ];

    fn word_prefix_before(text: &rgpui::input_ui::Rope, offset: usize) -> String {
        let offset = offset.min(text.len());
        let mut start = 0;
        let mut off = 0;
        for ch in text.slice(..offset).chars() {
            off += ch.len_utf8();
            if !(ch.is_alphanumeric() || ch == '_') {
                start = off;
            }
        }
        text.slice(start..offset).to_string()
    }

    impl CompletionProvider for DemoCompletionProvider {
        fn completions(
            &self,
            text: &rgpui::input_ui::Rope,
            offset: usize,
            _trigger: lsp_types::CompletionContext,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<lsp_types::CompletionResponse>> {
            // 大小写不敏感的前缀匹配；真 LSP 服务端按同样语义过滤后返回。
            let prefix = word_prefix_before(text, offset).to_lowercase();
            let items = DEMO_WORDS
                .iter()
                .filter(|(label, _, _)| {
                    prefix.is_empty() || label.to_lowercase().starts_with(&prefix)
                })
                .map(|(label, kind, detail)| lsp_types::CompletionItem {
                    label: label.to_string(),
                    kind: Some(*kind),
                    detail: Some(detail.to_string()),
                    ..Default::default()
                })
                .collect();
            rgpui::Task::ready(Ok(lsp_types::CompletionResponse::Array(items)))
        }
    }

    struct DemoDiagnosticsProvider;

    struct DemoSemanticTokensProvider;

    impl SemanticTokensProvider for DemoSemanticTokensProvider {
        fn semantic_tokens_full_with_legend(
            &self,
            _uri: &lsp_types::Uri,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Option<rgpui::lsp::SemanticTokensPayload>>> {
            // 第 0 行第 0 列长度 3（"fn "）为 function，第 3 列长度 4（"main"）为 function。
            rgpui::Task::ready(Ok(Some(rgpui::lsp::SemanticTokensPayload {
                data: vec![0, 0, 3, 0, 0, 0, 3, 4, 0, 0],
                token_types: vec![lsp_types::SemanticTokenType::new("function")],
                token_modifiers: Vec::new(),
            })))
        }
    }
    struct DemoSignatureHelpProvider;

    impl SignatureHelpProvider for DemoSignatureHelpProvider {
        fn signature_help(
            &self,
            _text: &rgpui::input_ui::Rope,
            _offset: usize,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Option<lsp_types::SignatureHelp>>> {
            rgpui::Task::ready(Ok(Some(lsp_types::SignatureHelp {
                signatures: vec![lsp_types::SignatureInformation {
                    label: "fn add(a: i32, b: i32) -> i32".to_string(),
                    documentation: None,
                    parameters: None,
                    active_parameter: None,
                }],
                active_signature: Some(0),
                active_parameter: None,
            })))
        }
    }
    struct DemoDefinitionProvider;

    impl DefinitionProvider for DemoDefinitionProvider {
        fn definition(
            &self,
            text: &rgpui::input_ui::Rope,
            offset: usize,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Vec<rgpui::lsp::DefinitionLocation>>> {
            let content = text.to_string();
            let byte = offset.min(content.len());
            let is_word = |c: char| c.is_alphanumeric() || c == '_';
            let mut start = byte;
            while start > 0 {
                let Some(prev) = content[..start].chars().next_back() else {
                    break;
                };
                if !is_word(prev) {
                    break;
                }
                start -= prev.len_utf8();
            }
            let mut end = byte;
            while end < content.len() {
                let Some(next) = content[end..].chars().next() else {
                    break;
                };
                if !is_word(next) {
                    break;
                }
                end += next.len_utf8();
            }
            if start >= end {
                return rgpui::Task::ready(Ok(Vec::new()));
            }
            let line = content[..start].matches('\n').count() as u32;
            let column = start - content[..start].rfind('\n').map_or(0, |i| i + 1);
            let uri: lsp_types::Uri = "file:///demo.rs".parse().expect("URI");
            rgpui::Task::ready(Ok(vec![rgpui::lsp::DefinitionLocation {
                uri,
                range: lsp_types::Range {
                    start: lsp_types::Position::new(line, column as u32),
                    end: lsp_types::Position::new(line, (column + (end - start)) as u32),
                },
                display_name: None,
            }]))
        }

        fn references(
            &self,
            _text: &rgpui::input_ui::Rope,
            _offset: usize,
            _include_declarations: bool,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Vec<rgpui::lsp::DefinitionLocation>>> {
            rgpui::Task::ready(Ok(Vec::new()))
        }

        fn document_highlights(
            &self,
            text: &rgpui::input_ui::Rope,
            offset: usize,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Vec<rgpui::lsp::DocumentHighlight>>> {
            // 同符号高亮：光标所在的词整体标出。
            let content = text.to_string();
            let byte = offset.min(content.len());
            let is_word = |c: char| c.is_alphanumeric() || c == '_';
            let mut start = byte;
            while start > 0 {
                let Some(prev) = content[..start].chars().next_back() else {
                    break;
                };
                if !is_word(prev) {
                    break;
                }
                start -= prev.len_utf8();
            }
            let mut end = byte;
            while end < content.len() {
                let Some(next) = content[end..].chars().next() else {
                    break;
                };
                if !is_word(next) {
                    break;
                }
                end += next.len_utf8();
            }
            if start >= end {
                return rgpui::Task::ready(Ok(Vec::new()));
            }
            let line = content[..start].matches('\n').count() as u32;
            let column = start - content[..start].rfind('\n').map_or(0, |i| i + 1);
            rgpui::Task::ready(Ok(vec![rgpui::lsp::DocumentHighlight {
                range: lsp_types::Range {
                    start: lsp_types::Position::new(line, column as u32),
                    end: lsp_types::Position::new(line, (column + (end - start)) as u32),
                },
                kind: Some(rgpui::lsp::DocumentHighlightKind::Text),
            }]))
        }
    }
    struct DemoRenameProvider;

    impl RenameProvider for DemoRenameProvider {
        fn rename(
            &self,
            text: &rgpui::input_ui::Rope,
            offset: usize,
            new_name: &str,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Option<lsp_types::WorkspaceEdit>>> {
            // 找出光标所在的词（空白分隔），生成一条替换编辑。
            let content = text.to_string();
            let byte = offset.min(content.len());
            let is_word = |c: char| c.is_alphanumeric() || c == '_';
            let mut start = byte;
            while start > 0 {
                let Some(prev) = content[..start].chars().next_back() else {
                    break;
                };
                if !is_word(prev) {
                    break;
                }
                start -= prev.len_utf8();
            }
            let mut end = byte;
            while end < content.len() {
                let Some(next) = content[end..].chars().next() else {
                    break;
                };
                if !is_word(next) {
                    break;
                }
                end += next.len_utf8();
            }
            if start >= end {
                return rgpui::Task::ready(Ok(None));
            }
            // 行号 / 字节列（LSP 行列按字节列换算）。
            let line = content[..start].matches('\n').count() as u32;
            let column = start - content[..start].rfind('\n').map_or(0, |i| i + 1);
            let end_line = line + content[start..end].matches('\n').count() as u32;
            let end_column = column + (end - start);
            let uri: lsp_types::Uri = "file:///demo.rs".parse().expect("URI");
            rgpui::Task::ready(Ok(Some(lsp_types::WorkspaceEdit {
                changes: Some(std::collections::HashMap::from([(
                    uri,
                    vec![lsp_types::TextEdit {
                        range: lsp_types::Range {
                            start: lsp_types::Position::new(line, column as u32),
                            end: lsp_types::Position::new(end_line, end_column as u32),
                        },
                        new_text: new_name.to_string(),
                    }],
                )])),
                document_changes: None,
                change_annotations: None,
            })))
        }
    }
    struct DemoFormattingProvider;

    impl FormattingProvider for DemoFormattingProvider {
        fn formatting(
            &self,
            text: &rgpui::input_ui::Rope,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Vec<lsp_types::TextEdit>>> {
            let original = text.to_string();
            let lines: Vec<&str> = original.split('\n').collect();
            let formatted: String = lines
                .iter()
                .map(|line| line.trim_end())
                .collect::<Vec<_>>()
                .join("\n");
            if formatted == original {
                return rgpui::Task::ready(Ok(Vec::new()));
            }
            // 整篇替换：结束位置取最后一行及其字节长度（LSP 行列按字节列换算）。
            let last = lines.len().saturating_sub(1);
            let end = lsp_types::Position::new(last as u32, lines[last].len() as u32);
            rgpui::Task::ready(Ok(vec![lsp_types::TextEdit {
                range: lsp_types::Range {
                    start: lsp_types::Position::new(0, 0),
                    end,
                },
                new_text: formatted,
            }]))
        }
    }

    impl DiagnosticsProvider for DemoDiagnosticsProvider {
        fn diagnostics(
            &self,
            _uri: &lsp_types::Uri,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Vec<DiagnosticEntry>>> {
            use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
            rgpui::Task::ready(Ok(vec![DiagnosticEntry::from_diagnostic(Diagnostic {
                range: Range {
                    start: Position::new(0, 0),
                    end: Position::new(0, 2),
                },
                severity: Some(DiagnosticSeverity::WARNING),
                message: "演示诊断：示例警告（假 provider）".to_string(),
                ..Default::default()
            })]))
        }
    }

    struct DemoHoverProvider;

    impl HoverProvider for DemoHoverProvider {
        fn hover(
            &self,
            _text: &rgpui::input_ui::Rope,
            offset: usize,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Option<HoverResponse>>> {
            rgpui::Task::ready(Ok(Some(HoverResponse {
                range: offset..offset,
                contents: vec![
                    HoverContent::CodeBlock {
                        language: "rust".to_string(),
                        code: "fn main()".to_string(),
                    },
                    HoverContent::Text("演示悬停：假 provider".to_string()),
                ],
            })))
        }
    }

    struct DemoCodelensProvider;

    impl CodeLensProvider for DemoCodelensProvider {
        fn codelenses(
            &self,
            _text: &rgpui::input_ui::Rope,
            _visible: std::ops::Range<usize>,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Vec<CodeLens>>> {
            rgpui::Task::ready(Ok(vec![CodeLens {
                line: 5,
                title: "Point · 1 引用".into(),
            }]))
        }
    }

    struct DemoCodeActionProvider;

    impl CodeActionProvider for DemoCodeActionProvider {
        fn code_actions(
            &self,
            text: &rgpui::input_ui::Rope,
            range: std::ops::Range<usize>,
            diagnostics: Vec<lsp_types::Diagnostic>,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Vec<lsp_types::CodeActionOrCommand>>> {
            use lsp_types::{CodeAction, CodeActionKind, Range, TextEdit, Uri, WorkspaceEdit};
            let len = text.len();
            let start = range.start.min(len);
            let end = range.end.min(len);
            // 选区所在行行首字节偏移（无 ropey 直接依赖，逐字符扫换行）。
            let mut line_start = 0;
            let mut off = 0;
            for ch in text.slice(..start).chars() {
                off += ch.len_utf8();
                if ch == '\n' {
                    line_start = off;
                }
            }
            let position =
                |offset: usize| rgpui::lsp::PositionMapping::offset_to_position(text, offset);
            let uri: Uri = "file:///demo.rs".parse().unwrap();
            let action = |title: &str,
                          kind: CodeActionKind,
                          edits: Vec<TextEdit>,
                          preferred: Option<bool>| CodeAction {
                title: title.to_string(),
                kind: Some(kind),
                edit: Some(WorkspaceEdit {
                    changes: Some(std::collections::HashMap::from([(uri.clone(), edits)])),
                    ..Default::default()
                }),
                is_preferred: preferred,
                ..Default::default()
            };
            let mut actions = vec![action(
                "行首加注释",
                CodeActionKind::REFACTOR,
                vec![TextEdit {
                    range: Range::new(position(line_start), position(line_start)),
                    new_text: "// ".to_string(),
                }],
                None,
            )];
            if start != end {
                actions.push(action(
                    "选区替换为 demo",
                    CodeActionKind::REFACTOR,
                    vec![TextEdit {
                        range: Range::new(position(start), position(end)),
                        new_text: "demo".to_string(),
                    }],
                    None,
                ));
            }
            // 有诊断时给一条首选修复：在诊断末尾标注（演示 `is_preferred` 默认选中）。
            if let Some(first) = diagnostics.first() {
                actions.insert(
                    0,
                    action(
                        "标注首条诊断",
                        CodeActionKind::QUICKFIX,
                        vec![TextEdit {
                            range: Range::new(first.range.end, first.range.end),
                            new_text: " /* 已标注 */".to_string(),
                        }],
                        Some(true),
                    ),
                );
            }
            rgpui::Task::ready(Ok(actions
                .into_iter()
                .map(lsp_types::CodeActionOrCommand::CodeAction)
                .collect()))
        }
    }

    struct DemoInlayProvider;

    impl rgpui::input_ui::InlayProvider for DemoInlayProvider {
        fn inlay_hints(
            &self,
            text: &rgpui::input_ui::Rope,
            _visible: std::ops::Range<usize>,
            _window: &mut Window,
            _cx: &mut App,
        ) -> rgpui::Task<anyhow::Result<Vec<rgpui::input_ui::InlayHint>>> {
            // 首行末尾（无 ropey 直接依赖，用 chars 数字节）。
            let end: usize = text
                .chars()
                .take_while(|c| *c != '\n')
                .map(|c| c.len_utf8())
                .sum();
            rgpui::Task::ready(Ok(vec![rgpui::input_ui::InlayHint {
                offset: end,
                text: ": demo".to_string(),
            }]))
        }
    }

    struct DemoLangStub;

    impl rgpui::highlight::Highlighter for DemoLangStub {
        fn language(&self) -> rgpui::SharedString {
            "demo".into()
        }

        fn update(
            &mut self,
            _edit: Option<rgpui::highlight::TextEdit>,
            _text: &rgpui::input_ui::Rope,
            _folding: bool,
            _window: &mut Window,
            _cx: &mut App,
        ) {
        }

        fn styles(
            &self,
            range: &std::ops::Range<usize>,
            _resolver: &dyn rgpui::highlight::HighlightStyleResolver,
        ) -> Vec<(std::ops::Range<usize>, rgpui::HighlightStyle)> {
            vec![(range.clone(), rgpui::HighlightStyle::default())]
        }

        fn fold_ranges(&self, _text: &rgpui::input_ui::Rope) -> Vec<rgpui::highlight::FoldRange> {
            Vec::new()
        }

        fn document_symbols(
            &self,
            _text: &rgpui::input_ui::Rope,
        ) -> Vec<rgpui::highlight::DocumentSymbol> {
            vec![rgpui::highlight::DocumentSymbol {
                kind: rgpui::highlight::SymbolKind::Function,
                name: "Demo".into(),
                range: 0..2,
                start_row: 0,
            }]
        }
    }

    fn hover_preview(state: &EditorState) -> String {
        let hover = state.hover_state();
        if !hover.visible {
            return "悬停：未请求".to_string();
        }
        let first = hover
            .response
            .as_ref()
            .and_then(|r| r.contents.first())
            .map(|c| match c {
                HoverContent::Text(s) | HoverContent::Markdown(s) => s.clone(),
                HoverContent::CodeBlock { code, .. } => code.clone(),
            })
            .unwrap_or_default();
        format!("悬停：{first}")
    }

    fn lsp_button(
        demo: &rgpui::Entity<EditorDemo>,
        id: &'static str,
        label: &'static str,
        action: impl Fn(&mut EditorState, &mut Window, &mut Context<EditorState>) + 'static,
    ) -> impl IntoElement {
        let demo = demo.clone();
        div()
            .id(id)
            .px(px(10.0))
            .py(px(4.0))
            .rounded_md()
            .cursor_pointer()
            .bg(rgb(0x000000).opacity(0.05))
            .hover(|this| this.bg(rgb(0x000000).opacity(0.1)))
            .text_xs()
            .child(label)
            .on_click(move |_, window, cx| {
                demo.update(cx, |this, cx| {
                    this.editor.update(cx, |state, cx| {
                        action(state, window, cx);
                    });
                });
            })
    }

    struct EditorDemo {
        editor: rgpui::Entity<EditorState>,
        search: rgpui::Entity<SearchPanelState>,
        breakpoints: std::collections::HashSet<usize>,
        bookmarks: std::collections::HashSet<usize>,
        gutter_status: String,
        /// 搜索弹窗：`None` 关闭，`Some(show_replace)` 打开（`Ctrl+F` 关替换/`Ctrl+R` 开）。
        search_open: Option<bool>,
        /// gutter 菜单：打开的行 + 窗口坐标（左键/右键都开菜单，不再常驻图标）。
        gutter_menu: Option<(usize, Point<Pixels>)>,
        /// 折叠开关状态（诊断行首遮挡用：关掉后折叠箭头消失）。
        folding: bool,
        /// 布局调试标尺（行首遮挡定位用：四色竖线标出 gutter/行号/折叠/文本边界）。
        debug_chrome: bool,
    }

    fn sorted_vec(set: &std::collections::HashSet<usize>) -> Vec<usize> {
        let mut v: Vec<usize> = set.iter().copied().collect();
        v.sort_unstable();
        v
    }

    fn open_gutter_menu(
        demo: &rgpui::Entity<EditorDemo>,
        row: usize,
        pos: Point<Pixels>,
        cx: &mut App,
    ) {
        demo.update(cx, |this, _| {
            this.gutter_menu = Some((row, pos));
        });
    }

    fn gutter_menu_row(
        demo: &rgpui::Entity<EditorDemo>,
        id: &'static str,
        label: String,
        apply: impl Fn(&mut EditorDemo) + 'static,
    ) -> impl IntoElement {
        let demo = demo.clone();
        div()
            .id(id)
            .px(px(10.0))
            .py(px(6.0))
            .rounded_md()
            .cursor_pointer()
            .hover(|this| this.bg(rgb(0x000000).opacity(0.08)))
            .text_sm()
            .child(label)
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
            })
            .on_mouse_down(MouseButton::Right, move |_, _, cx| {
                cx.stop_propagation();
            })
            .on_click(move |_, _, cx| {
                demo.update(cx, |this, _| {
                    apply(this);
                    this.gutter_menu = None;
                });
            })
    }
    fn offset_of(text: &str, line: usize, col: usize) -> usize {
        let mut offset = 0;
        for (ix, part) in text.split('\n').enumerate() {
            if ix == line {
                return offset + col.min(part.len());
            }
            offset += part.len() + 1;
        }
        offset
    }

    impl EditorDemo {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            // 编辑器状态一次配好（多行 + 行号 + 折叠 + 键入体验 + 大纲订阅）。
            let editor = cx.new(|cx| EditorState::new(window, cx, SAMPLE));
            // 演示语言注册（注册表机制；真语言按三步加 grammar）。
            rgpui::highlight::register_highlighter(
                "demo",
                Box::new(
                    |_| Some(Box::new(DemoLangStub) as Box<dyn rgpui::highlight::Highlighter>),
                ),
            );
            editor.update(cx, |state, cx| {
                // 高亮器经编辑器状态透传接入（大纲同步刷新）。
                state.set_highlighter(Some(rgpui::highlight::rust_highlighter()), window, cx);
                // 面包屑放状态栏（顶栏永不出现，无高度跳变，编辑区不位移）。
                state.set_sticky_position(StickyPosition::Status, cx);
                // LSP 假 provider 注入（传输层由真应用实现后替换）。
                state.set_completion_provider(Some(Rc::new(DemoCompletionProvider)), cx);
                state.set_diagnostics_provider(Some(Rc::new(DemoDiagnosticsProvider)), cx);
                state.set_hover_provider(Some(Rc::new(DemoHoverProvider)), cx);
                state.set_formatting_provider(Some(Rc::new(DemoFormattingProvider)), cx);
                state.set_rename_provider(Some(Rc::new(DemoRenameProvider)), cx);
                state.set_definition_provider(Some(Rc::new(DemoDefinitionProvider)), cx);
                state.set_signature_help_provider(Some(Rc::new(DemoSignatureHelpProvider)), cx);
                state.set_semantic_tokens_provider(
                    Some(Rc::new(DemoSemanticTokensProvider)),
                    "rust",
                    cx,
                );
                // 透镜假 provider（文本变更自动刷新，点击跳光标到透镜行）。
                state.set_codelens_provider(Some(Rc::new(DemoCodelensProvider)), cx);
                state.set_document_uri(Some("file:///demo.rs".parse().unwrap()), cx);
                // 快速修复：`Alt+Enter` 弹菜单（菜单浮层由 `Editor` 自带渲染）。
                state.set_code_action_provider(Some(Rc::new(DemoCodeActionProvider)), cx);
                // inlay 默认开启（演示绘制；关开关即零开销）。
                state.set_inlay_provider(Some(Rc::new(DemoInlayProvider)), cx);
                state.set_inlay_hints_enabled(true, cx);
            });
            let input = editor.read_with(cx, |state, _| state.input().clone());

            // gutter 单 provider（菜单模式）：已设行才画图标，未设行返回隐形可点格
            // （左键/右键都开菜单选动作，不再逐行常驻图标）。
            // 同行多标记显示规则（格宽 20px 只够一个）：断点 ● > 书签 ⚑ > 运行 ▶；
            // 菜单行实时反映各类型有无（设/取消），全量状态点开即见。
            {
                let demo = cx.entity();
                let text_handle = input.clone();
                editor.update(cx, |state, cx| {
                    state.add_gutter_provider(
                        "menu",
                        move |row, _window, cx| {
                            let demo_for_cell = demo.clone();
                            let (bp_set, mark_set) = demo_for_cell.read_with(cx, |this, _| {
                                (
                                    this.breakpoints.contains(&row),
                                    this.bookmarks.contains(&row),
                                )
                            });
                            // 运行标记只在 `fn main` 行且无断点书签时展示。
                            let runnable = !bp_set
                                && !mark_set
                                && text_handle.read_with(cx, |s, _| {
                                    s.text()
                                        .to_string()
                                        .lines()
                                        .nth(row)
                                        .map(|line| line.trim_start().starts_with("fn main"))
                                        .unwrap_or(false)
                                });
                            let marker: Option<(SharedString, u32)> = if bp_set {
                                Some(("●".into(), 0xef4444))
                            } else if mark_set {
                                Some(("⚑".into(), 0xeab308))
                            } else if runnable {
                                Some(("▶".into(), 0x22c55e))
                            } else {
                                None
                            };
                            let open_menu_row = row;
                            let demo_for_click = demo_for_cell.clone();
                            let mut cell = div()
                                .w_full()
                                .h_full()
                                .overflow_hidden()
                                .flex()
                                .items_center()
                                .justify_center()
                                .id(("gutter-cell", row))
                                .cursor_pointer()
                                .on_click(move |event, _, cx| {
                                    if let ClickEvent::Mouse(e) = event {
                                        open_gutter_menu(
                                            &demo_for_click,
                                            open_menu_row,
                                            e.down.position,
                                            cx,
                                        );
                                    }
                                })
                                // 右键走捕获阶段先手 + 停传播：压住编辑器自身的右键菜单，
                                // 否则两层菜单叠加，点 gutter 菜单会误触下层（如粘贴）。
                                // 所有鼠标监听同属窗口级循环（捕获先行），先到先停可靠。
                                .capture_any_mouse_down({
                                    move |event, _, cx| {
                                        if event.button == MouseButton::Right {
                                            open_gutter_menu(
                                                &demo_for_cell,
                                                open_menu_row,
                                                event.position,
                                                cx,
                                            );
                                            cx.stop_propagation();
                                        }
                                    }
                                });
                            if let Some((glyph, color)) = marker {
                                cell = cell.text_xs().text_color(rgb(color)).child(glyph);
                            }
                            Some(cell.into_any_element())
                        },
                        cx,
                    );
                    state.set_gutter_column_enabled(true, cx);
                });
            }

            // 大纲缓存由编辑器状态维护，这里只在文本变更时重渲染。
            cx.subscribe(&input, |_, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
            .detach();

            // 搜索面板：一行接通编辑器（文本同步 + 匹配标黄 + 默认跳转）。
            let search = cx.new(|cx| SearchPanelState::new(window, cx));
            search.update(cx, |panel, cx| panel.attach_editor(&input, cx));
            // 替换接线（替换权在外部：当前匹配 / 全部匹配从后往前保偏移）。
            {
                let text = input.clone();
                let panel_handle = search.clone();
                search.update(cx, |panel, _| {
                    panel.set_on_replace(move |_, replacement, window, cx| {
                        let full = text.read_with(cx, |state, _| state.text().to_string());
                        let range = panel_handle.read_with(cx, |panel, cx| {
                            panel.state().read(cx).current_match().map(|m| {
                                let base = offset_of(&full, m.line, 0);
                                base + m.start_col..base + m.end_col
                            })
                        });
                        if let Some(range) = range {
                            text.update(cx, |state, cx| {
                                state.set_selected_range(range, cx);
                                state.replace(replacement, window, cx);
                            });
                        }
                    });
                });
            }
            {
                let text = input.clone();
                let panel_handle = search.clone();
                search.update(cx, |panel, _| {
                    panel.set_on_replace_all(move |_, replacement, window, cx| {
                        let full = text.read_with(cx, |state, _| state.text().to_string());
                        let mut ranges: Vec<_> = panel_handle.read_with(cx, |panel, cx| {
                            panel
                                .state()
                                .read(cx)
                                .matches()
                                .iter()
                                .map(|m| {
                                    let base = offset_of(&full, m.line, 0);
                                    base + m.start_col..base + m.end_col
                                })
                                .collect()
                        });
                        ranges.sort_by_key(|range| std::cmp::Reverse(range.start));
                        text.update(cx, |state, cx| {
                            for range in ranges {
                                state.set_selected_range(range, cx);
                                state.replace(replacement.clone(), window, cx);
                            }
                        });
                    });
                });
            }

            Self {
                editor,
                search,
                breakpoints: std::collections::HashSet::new(),
                bookmarks: std::collections::HashSet::new(),
                gutter_status: "gutter：点格子开菜单（断点/书签/运行），同行多标记只显优先级最高者"
                    .to_string(),
                search_open: None,
                gutter_menu: None,
                folding: true,
                debug_chrome: false,
            }
        }

        fn goto(&mut self, symbol: &rgpui::highlight::DocumentSymbol, cx: &mut Context<Self>) {
            let symbol = symbol.clone();
            self.editor.update(cx, |state, cx| {
                state.goto_symbol(&symbol, cx);
            });
        }
    }

    impl Render for EditorDemo {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let symbols = self
                .editor
                .read_with(cx, |state, _| state.outline().to_vec());
            let (diag_count, hover_text) = self.editor.read_with(cx, |state, _| {
                (state.diagnostics().len(), hover_preview(state))
            });
            let lang_text = self.editor.read_with(cx, |state, _| {
                state
                    .language()
                    .map(|s| s.to_string())
                    .unwrap_or("纯文本".to_string())
            });
            let auto_complete = self
                .editor
                .read_with(cx, |state, _| state.auto_completion_enabled());
            let minimap_on = self
                .editor
                .read_with(cx, |state, _| state.minimap_enabled());
            let vim_on = self.editor.read_with(cx, |state, cx| state.vim_enabled(cx));
            let demo = cx.entity();
            // 自动补全开关（开后键入单词字符即弹补全，空格/换行自动收起）。
            // 补全弹窗（光标处锚定，`deferred` 浮层，点击行即确认插入）。
            let popup = self
                .editor
                .read_with(cx, |state, _| state.completion_popup().clone());
            let demo_for_popup = demo.clone();
            let popup_el =
                rgpui::lsp::CompletionPopup::new(popup).on_change(move |ix, window, cx| {
                    demo_for_popup.update(cx, |this, cx| {
                        this.editor.update(cx, |state, cx| {
                            state.accept_completion(Some(ix), window, cx);
                        });
                    });
                });
            // 透镜浮层（行首上方，点击跳光标到透镜行）。
            let demo_for_lens = demo.clone();
            let lens_el = CodeLensOverlay::new(&self.editor).on_lens(move |ix, _, cx| {
                demo_for_lens.update(cx, |this, cx| {
                    let offset = this.editor.read_with(cx, |state, _| {
                        state.codelenses().get(ix).map(|lens| lens.offset)
                    });
                    if let Some(offset) = offset {
                        this.editor.update(cx, |state, cx| {
                            state.set_selected_range(offset..offset, cx);
                        });
                    }
                });
            });
            h_flex()
            .size_full()
            .items_stretch()
            .gap(px(12.0))
            .p(px(12.0)).child(
            v_flex()
                .w(px(220.0))
                .gap(px(4.0))
                .child(div().text_sm().child("大纲（符号）"))
                .children(symbols.into_iter().map(|symbol| {
                    let label = format!("{} · {}行", symbol.name, symbol.start_row + 1);
                    let demo = demo.clone();
                    div()
                        .id(("outline-symbol", symbol.start_row))
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|this| this.bg(rgb(0x000000).opacity(0.05)))
                        .child(div().text_sm().child(label))
                        .on_click(move |_, _, cx| {
                            demo.update(cx, |this, cx| {
                                this.goto(&symbol, cx);
                            });
                        })
                })),
        )
        .child(
            v_flex()
                .flex_1()
                .gap(px(8.0))
                .child(
                    div().text_xs().child(
                        "行操作：Shift+Alt+↓复制行 Ctrl+Shift+K删行 Alt+↑↓移行 Ctrl+/注释 Ctrl+J合行 ｜ \
                         多光标：Ctrl+Alt+↑↓加光标 Alt+点击",
                    ),
                )
                .child(
                    div().text_xs().child(
                        "键入：自动补括号/电缩进/括号匹配/当前行高亮 ｜ \
                         补全：↑↓改选 Enter确认 Esc收起（确认时替换光标处单词）｜ \
                         修复：Alt+Enter 弹菜单，↑↓改选 Enter应用 Esc收起",
                    ),
                )
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap(px(8.0))
                        .items_center()
                        .child(lsp_button(&demo, "lsp-complete", "请求补全", |state, window, cx| {
                            state.request_completions(window, cx);
                        }))
                        .child(lsp_button(&demo, "lsp-diagnostics", "请求诊断", |state, window, cx| {
                            state.request_diagnostics(window, cx);
                        }))
                        .child(lsp_button(&demo, "lsp-hover", "悬停光标处", |state, window, cx| {
                            let offset = state.cursor(cx);
                            state.request_hover(offset, window, cx);
                        }))
                        .child(lsp_button(&demo, "lsp-inlay", "请求 inlay", |state, window, cx| {
                            state.request_inlay_hints(window, cx);
                        }))
                        .child({
                            let demo = demo.clone();
                            Switch::new("auto-complete")
                                .checked(auto_complete)
                                .label("输入时自动补全")
                                .on_change(move |checked, _, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.editor.update(cx, |state, cx| {
                                            state.set_auto_completion_enabled(checked, cx);
                                        });
                                    });
                                })
                        })
                        .child(
                            div()
                                .text_xs()
                                .child("自动：单词前缀满 2 个字符或键入 ./: 才请求，按前缀过滤，无匹配自动隐藏"),
                        )
                        .child({
                            let demo = demo.clone();
                            Switch::new("minimap")
                                .checked(minimap_on)
                                .label("缩略图")
                                .on_change(move |checked, _, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.editor.update(cx, |state, cx| {
                                            state.set_minimap_enabled(checked, cx);
                                        });
                                    });
                                })
                        })
                        .child({
                            let demo = demo.clone();
                            Switch::new("vim-mode")
                                .checked(vim_on)
                                .label("Vim 模式")
                                .on_change(move |checked, _, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.editor.update(cx, |state, cx| {
                                            state.set_vim_enabled(checked, cx);
                                        });
                                    });
                                })
                        })
                        .child(
                            div()
                                .text_xs()
                                .child("Vim：hjkl/wb/0/$/gg/G 移动，i/a/o/v 切换，x/dd/yy/p/u 编辑，可视 y/d"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .child(format!("诊断 {diag_count} ｜ {hover_text}")),
                        ),
                )
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap(px(8.0))
                        .items_center()
                        .child(lsp_button(&demo, "snip-expand", "展开片段", |state, window, cx| {
                            state.expand_snippet(
                                "fn ${1:name}(${2:args}) {\n    $0\n}",
                                window,
                                cx,
                            );
                        }))
                        .child(lsp_button(&demo, "snip-next", "下一占位 Tab", |state, _, cx| {
                            state.next_placeholder(cx);
                        }))
                        .child(lsp_button(&demo, "snip-prev", "上一占位 S-Tab", |state, _, cx| {
                            state.prev_placeholder(cx);
                        }))
                        .child(div().text_xs().child(
                            "片段：$1/$2 跳转，$0 收尾；会话内键入只跟踪跳转（镜像/强制不做）",
                        )),
                )
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap(px(8.0))
                        .items_center()
                        .child(lsp_button(&demo, "lang-rust", "语言: rust", |state, window, cx| {
                            state.set_language("rust", window, cx);
                        }))
                        .child(lsp_button(&demo, "lang-demo", "语言: demo", |state, window, cx| {
                            state.set_language("demo", window, cx);
                        }))
                        .child(lsp_button(
                            &demo,
                            "lang-python",
                            "语言: python",
                            |state, window, cx| {
                                state.set_language("python", window, cx);
                            },
                        ))
                        .child(lsp_button(
                            &demo,
                            "lang-javascript",
                            "语言: javascript",
                            |state, window, cx| {
                                state.set_language("javascript", window, cx);
                            },
                        ))
                        .child(lsp_button(&demo, "lang-go", "语言: go", |state, window, cx| {
                            state.set_language("go", window, cx);
                        }))
                        .child(lsp_button(&demo, "lang-json", "语言: json", |state, window, cx| {
                            state.set_language("json", window, cx);
                        }))
                        .child(lsp_button(
                            &demo,
                            "format-doc",
                            "格式化文档（LSP）",
                            |state, window, cx| {
                                state.request_formatting(window, cx);
                            },
                        ))
                        .child(lsp_button(
                            &demo,
                            "rename-symbol",
                            "重命名光标处 → renamed",
                            |state, window, cx| {
                                state.request_rename("renamed", window, cx);
                            },
                        ))
                        .child(lsp_button(
                            &demo,
                            "goto-definition",
                            "跳转定义（LSP）",
                            |state, window, cx| {
                                state.goto_definition(window, cx);
                            },
                        ))
                        .child(lsp_button(
                            &demo,
                            "signature-help",
                            "签名提示（LSP）",
                            |state, window, cx| {
                                state.request_signature_help(window, cx);
                            },
                        ))
                        .child(lsp_button(
                            &demo,
                            "document-highlights",
                            "同符号高亮（LSP）",
                            |state, window, cx| {
                                state.request_document_highlights(window, cx);
                            },
                        ))
                        .child(lsp_button(
                            &demo,
                            "semantic-tokens",
                            "语义高亮（LSP）",
                            |state, window, cx| {
                                state.request_semantic_tokens(window, cx);
                            },
                        ))
                        .child(lsp_button(
                            &demo,
                            "lang-unknown",
                            "语言: 未知降级",
                            |state, window, cx| {
                                state.set_language("brainfuck-x", window, cx);
                            },
                        ))
                        .child(div().text_xs().child(format!("当前语言：{lang_text}")))
                        .child(div().text_xs().child("标黄："))
                        .child({
                            let demo = demo.clone();
                            div()
                                .id("hl-yellow")
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded_md()
                                .cursor_pointer()
                                .text_xs()
                                .child("黄")
                                .on_click(move |_, _, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.search.update(cx, |panel, cx| {
                                            panel.set_highlight_colors(yellow(), None, cx)
                                        });
                                    });
                                })
                        })
                        .child({
                            let demo = demo.clone();
                            div()
                                .id("hl-green")
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded_md()
                                .cursor_pointer()
                                .text_xs()
                                .child("绿")
                                .on_click(move |_, _, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.search.update(cx, |panel, cx| {
                                            panel.set_highlight_colors(green(), None, cx)
                                        });
                                    });
                                })
                        })
                        .child({
                            let demo = demo.clone();
                            div()
                                .id("hl-blue")
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded_md()
                                .cursor_pointer()
                                .text_xs()
                                .child("蓝")
                                .on_click(move |_, _, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.search.update(cx, |panel, cx| {
                                            panel.set_highlight_colors(blue(), Some(white()), cx)
                                        });
                                    });
                                })
                        }),
                )
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap(px(8.0))
                        .items_center()
                        .child(lsp_button(&demo, "gutter-master", "gutter 开关", |state, _, cx| {
                            let enabled = !state.gutter_column_enabled(cx);
                            state.set_gutter_column_enabled(enabled, cx);
                        }))
                        .child({
                            let demo = demo.clone();
                            let folding = self.folding;
                            Switch::new("folding")
                                .checked(folding)
                                .label("折叠")
                                .on_change(move |checked, window, cx| {
                                    demo.update(cx, |this, cx| {
                                        this.folding = checked;
                                        this.editor.update(cx, |state, cx| {
                                            state.set_folding(checked, window, cx);
                                        });
                                    });
                                })
                        })
                        .child({
                            let demo = demo.clone();
                            let debug_chrome = self.debug_chrome;
                            Switch::new("debug-chrome")
                                .checked(debug_chrome)
                                .label("布局调试")
                                .on_change(move |checked, _, cx| {
                                    demo.update(cx, |this, _| {
                                        this.debug_chrome = checked;
                                    });
                                })
                        })
                        .child(
                            div()
                                .text_xs()
                                .child("标尺：红=gutter缘 绿=行号缘 黄=折叠缘 青=文本首"),
                        )
                        .child(div().text_xs().child(self.gutter_status.clone())),
                )
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .child(
                            Editor::new(&self.editor)
                                .flex_1()
                                .context_menu_extra({
                                    let upper = self
                                        .editor
                                        .read_with(cx, |state, _| state.input().clone());
                                    move |menu, _, _, _| {
                                        let upper = upper.clone();
                                        menu.item(
                                            PopupMenuItem::new("转为大写 UPPERCASE").on_click(
                                                move |_, window, cx| {
                                                    upper.update(cx, |state, cx| {
                                                        let selected =
                                                            state.selected_value().to_string();
                                                        if !selected.is_empty() {
                                                            state.replace(
                                                                selected.to_uppercase(),
                                                                window,
                                                                cx,
                                                            );
                                                        }
                                                    });
                                                },
                                            ),
                                        )
                                    }
                                }),
                        )
                        .child(popup_el)
                        .child(lens_el)
                        .when_some(self.search_open, |this, show_replace| {
                            // 搜索弹窗（编辑器左上浮动；右侧经常被裁，左上最稳）。
                            // `Ctrl+F` 纯查找/`Ctrl+R` 带替换，`Esc` 关闭。
                            let demo = demo.clone();
                            this.child(
                                div()
                                    .absolute()
                                    .top(px(8.0))
                                    .left(px(8.0))
                                    .child(
                                        v_flex()
                                            .gap(px(4.0))
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .child(div().text_sm().child(if show_replace {
                                                        "查找替换"
                                                    } else {
                                                        "查找"
                                                    }))
                                                    .child(
                                                        div()
                                                            .id("search-popup-close")
                                                            .px(px(8.0))
                                                            .cursor_pointer()
                                                            .text_sm()
                                                            .child("×")
                                                            .on_click(move |_, _, cx| {
                                                                demo.update(cx, |this, cx| {
                                                                    this.search_open = None;
                                                                    // 关弹窗清标黄（装饰不清会一直留在编辑器上）。
                                                                    this.search.update(cx, |panel, cx| {
                                                                        panel.clear_highlights(cx);
                                                                    });
                                                                });
                                                            }),
                                                    ),
                                            )
                                            .child(self.search.clone())
                                            .child(
                                                div().text_xs().child(
                                                    "Esc 关闭 · Enter 下一个 · Shift+Enter 上一个",
                                                ),
                                            ),
                                    ),
                            )
                        }),
                )
                .child(div().text_xs().child(
                    "右键=默认菜单 + 追加转大写（已集成进编辑器，无独立演示行）。",
                )),
        )
        .when_some(
            self.debug_chrome
                .then(|| {
                    self.editor
                        .read_with(cx, |state, cx| state.chrome_geometry(cx))
                })
                .flatten(),
            |this, geo| {
                // 布局调试标尺（窗口坐标竖线，不拦截点击）。
                let h = window.bounds().size.height;
                let rule = |x: Pixels, color: u32| {
                    deferred(
                        div()
                            .absolute()
                            .top_0()
                            .left(x)
                            .w(px(1.))
                            .h(h)
                            .bg(rgb(color)),
                    )
                    .into_any_element()
                };
                this.children([
                    rule(geo.gutter_end, 0xff0000),
                    rule(geo.numbers_end, 0x00ff00),
                    rule(geo.fold_end, 0xffff00),
                    rule(geo.text_start, 0x00ffff),
                ])
            },
        )
        .when_some(self.gutter_menu, |this, (row, pos)| {
            // gutter 菜单（点击行处锚定；透罩点空处关闭）。
            let demo_for_catcher = demo.clone();
            let demo_for_right = demo.clone();
            let bp_set = self.breakpoints.contains(&row);
            let mark_set = self.bookmarks.contains(&row);
            let popover = cx.theme().tokens.popover;
            let border = cx.theme().tokens.border;
            let radius = cx.theme().radius;
            let win_size = window.bounds().size;
            this.children([
                deferred(
                    div()
                        .id("gutter-menu-catcher")
                        .absolute()
                        .top_0()
                        .left_0()
                        .w(win_size.width)
                        .h(win_size.height)
                        // 透罩吞按下（ shield 下层编辑器；点击合成照常触发关闭）。
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            cx.stop_propagation();
                        })
                        .on_mouse_down(MouseButton::Right, move |_, _, cx| {
                            demo_for_right.update(cx, |this, _| {
                                this.gutter_menu = None;
                            });
                            cx.stop_propagation();
                        })
                        .on_click(move |_, _, cx| {
                            demo_for_catcher.update(cx, |this, _| {
                                this.gutter_menu = None;
                            });
                        }),
                )
                .into_any_element(),
                deferred(
                    anchored()
                        .position(pos)
                        .anchor(rgpui::Anchor::TopLeft)
                        .snap_to_window_with_margin(px(8.))
                        .child(
                            v_flex()
                                .w(px(180.))
                                .p(px(4.))
                                .gap(px(2.))
                                .bg(popover)
                                .border_1()
                                .border_color(border)
                                .rounded(radius)
                                .shadow_lg()
                                .child(gutter_menu_row(
                                    &demo,
                                    "gutter-menu-bp",
                                    if bp_set {
                                        "● 取消断点".to_string()
                                    } else {
                                        "○ 设断点".to_string()
                                    },
                                    move |this| {
                                        if !this.breakpoints.remove(&row) {
                                            this.breakpoints.insert(row);
                                        }
                                        this.gutter_status =
                                            format!("断点行：{:?}", sorted_vec(&this.breakpoints));
                                    },
                                ))
                                .child(gutter_menu_row(
                                    &demo,
                                    "gutter-menu-mark",
                                    if mark_set {
                                        "⚑ 取消书签".to_string()
                                    } else {
                                        "⚑ 设书签".to_string()
                                    },
                                    move |this| {
                                        if !this.bookmarks.remove(&row) {
                                            this.bookmarks.insert(row);
                                        }
                                        this.gutter_status =
                                            format!("书签行：{:?}", sorted_vec(&this.bookmarks));
                                    },
                                ))
                                .child(gutter_menu_row(
                                    &demo,
                                    "gutter-menu-run",
                                    "▶ 运行此行".to_string(),
                                    move |this| {
                                        this.gutter_status =
                                            format!("运行第 {} 行（演示，未真跑）", row + 1);
                                    },
                                )),
                        ),
                )
                .with_priority(1)
                .into_any_element(),
            ])
        })
        .capture_key_down({
            let demo = demo.clone();
            move |event: &rgpui::KeyDownEvent, window: &mut Window, cx: &mut App| {
                let mods = &event.keystroke.modifiers;
                let key = event.keystroke.key.as_str();
                // Ctrl+F/R 开搜索弹窗（替换行显隐不同；聚到搜索框）。
                if mods.control && !mods.alt && !mods.shift && (key == "f" || key == "r") {
                    let show_replace = key == "r";
                    demo.update(cx, |this, cx| {
                        this.search_open = Some(show_replace);
                        this.search.update(cx, |panel, cx| {
                            panel.set_show_replace(show_replace, cx);
                            // 重开把关闭期清掉的标黄标回来。
                            panel.refresh_highlights(cx);
                        });
                    });
                    let search = demo.read_with(cx, |this, _| this.search.clone());
                    search.update(cx, |panel, cx| {
                        panel.search_input().update(cx, |input, cx| {
                            input.focus(window, cx);
                        });
                    });
                } else if key == "escape" {
                    // Esc 先关 gutter 菜单，再关搜索弹窗（并回焦编辑器）。
                    let closed = demo.read_with(cx, |this, _| {
                        this.gutter_menu.is_some() || this.search_open.is_some()
                    });
                    demo.update(cx, |this, cx| {
                        this.gutter_menu = None;
                        // Esc 关搜索弹窗同样清标黄（与 × 按钮同语义）。
                        if this.search_open.is_some() {
                            this.search.update(cx, |panel, cx| {
                                panel.clear_highlights(cx);
                            });
                        }
                        this.search_open = None;
                    });
                    if closed {
                        // 回焦编辑器输入框。
                        let input = demo.read_with(cx, |this, cx| {
                            this.editor.read_with(cx, |state, _| state.input().clone())
                        });
                        input.update(cx, |state, cx| {
                            state.focus(window, cx);
                        });
                    }
                }
            }
        })
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| EditorDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| EditorDemo::new(window, cx)).into()
    }
}

/// 返回按大类分组的 showcase 示例注册表。
pub fn groups() -> Vec<(&'static str, Vec<StoryItem>)> {
    vec![
        (
            "核心组件",
            vec![
                StoryItem {
                    title: "hello_world",
                    build: hello_world::build,
                },
                StoryItem {
                    title: "a11y",
                    build: a11y::build,
                },
                StoryItem {
                    title: "theme",
                    build: theme::build,
                },
                StoryItem {
                    title: "animation",
                    build: animation::build,
                },
                StoryItem {
                    title: "icons",
                    build: icons::build,
                },
                StoryItem {
                    title: "component_gallery",
                    build: component_gallery::build,
                },
                StoryItem {
                    title: "extended_components",
                    build: extended_components::build,
                },
                StoryItem {
                    title: "scrollable",
                    build: scrollable::build,
                },
                StoryItem {
                    title: "sidebar",
                    build: sidebar::build,
                },
                StoryItem {
                    title: "tabs",
                    build: tabs::build,
                },
                StoryItem {
                    title: "fps_hud",
                    build: fps_hud::build,
                },
                StoryItem {
                    title: "inspector",
                    build: inspector::build,
                },
                StoryItem {
                    title: "inspector_custom",
                    build: inspector_custom::build,
                },
                StoryItem {
                    title: "gradient",
                    build: gradient::build,
                },
                StoryItem {
                    title: "painting",
                    build: painting::build,
                },
                StoryItem {
                    title: "pattern",
                    build: pattern::build,
                },
                StoryItem {
                    title: "anchor",
                    build: anchor::build,
                },
                StoryItem {
                    title: "popover",
                    build: popover::build,
                },
            ],
        ),
        ("输入与表单", vec![]),
        (
            "文本与国际化",
            vec![
                StoryItem {
                    title: "i18n",
                    build: i18n::build,
                },
                StoryItem {
                    title: "intl_text",
                    build: intl_text::build,
                },
                StoryItem {
                    title: "markdown",
                    build: markdown::build,
                },
                StoryItem {
                    title: "markdown_stream",
                    build: markdown_stream::build,
                },
                StoryItem {
                    title: "rich_text",
                    build: rich_text::build,
                },
                StoryItem {
                    title: "rich_clipboard",
                    build: rich_clipboard::build,
                },
                StoryItem {
                    title: "text_layout",
                    build: text_layout::build,
                },
                StoryItem {
                    title: "text_wrapper",
                    build: text_wrapper::build,
                },
            ],
        ),
        (
            "创意与交互",
            vec![
                StoryItem {
                    title: "ink",
                    build: ink::build,
                },
                StoryItem {
                    title: "sorter",
                    build: sorter::build,
                },
                StoryItem {
                    title: "timeline",
                    build: timeline::build,
                },
                StoryItem {
                    title: "color_picker",
                    build: color_picker::build,
                },
                StoryItem {
                    title: "mdi",
                    build: mdi::build,
                },
                StoryItem {
                    title: "task_pane",
                    build: task_pane::build,
                },
                StoryItem {
                    title: "canvas_a11y",
                    build: canvas_a11y::build,
                },
                StoryItem {
                    title: "page_view",
                    build: page_view::build,
                },
                StoryItem {
                    title: "drag_drop",
                    build: drag_drop::build,
                },
                StoryItem {
                    title: "focus_visible",
                    build: focus_visible::build,
                },
                StoryItem {
                    title: "mouse_pressure",
                    build: mouse_pressure::build,
                },
                StoryItem {
                    title: "native_menu",
                    build: native_menu::build,
                },
                StoryItem {
                    title: "native_notification",
                    build: native_notification::build,
                },
                StoryItem {
                    title: "set_menus",
                    build: set_menus::build,
                },
                StoryItem {
                    title: "tab_stop",
                    build: tab_stop::build,
                },
                StoryItem {
                    title: "chat",
                    build: chat::build,
                },
                StoryItem {
                    title: "dock",
                    build: dock::build,
                },
                StoryItem {
                    title: "grid_layout",
                    build: grid_layout::build,
                },
                StoryItem {
                    title: "status_bar",
                    build: status_bar::build,
                },
                StoryItem {
                    title: "tree",
                    build: tree::build,
                },
            ],
        ),
        (
            "图片与窗口",
            vec![
                StoryItem {
                    title: "image_loading",
                    build: image_loading::build,
                },
                StoryItem {
                    title: "image_gallery",
                    build: image_gallery::build,
                },
                StoryItem {
                    title: "svg",
                    build: svg::build,
                },
                StoryItem {
                    title: "window",
                    build: window::build,
                },
                StoryItem {
                    title: "opacity",
                    build: opacity::build,
                },
                StoryItem {
                    title: "shadow",
                    build: shadow::build,
                },
                StoryItem {
                    title: "transparent",
                    build: transparent::build,
                },
                StoryItem {
                    title: "window_shadow",
                    build: window_shadow::build,
                },
                StoryItem {
                    title: "list_example",
                    build: list_example::build,
                },
                StoryItem {
                    title: "uniform_list",
                    build: uniform_list::build,
                },
                StoryItem {
                    title: "data_table",
                    build: data_table::build,
                },
                StoryItem {
                    title: "tree_list",
                    build: tree_list::build,
                },
            ],
        ),
        (
            "应用套件",
            vec![
                StoryItem {
                    title: "backstage",
                    build: backstage::build,
                },
                StoryItem {
                    title: "command_registry",
                    build: command_registry::build,
                },
                StoryItem {
                    title: "mini_toolbar",
                    build: mini_toolbar::build,
                },
                StoryItem {
                    title: "qat",
                    build: qat::build,
                },
                StoryItem {
                    title: "ribbon",
                    build: ribbon::build,
                },
                StoryItem {
                    title: "canvas_objects",
                    build: canvas_objects::build,
                },
                StoryItem {
                    title: "ruler",
                    build: ruler::build,
                },
                StoryItem {
                    title: "viewport",
                    build: viewport::build,
                },
                StoryItem {
                    title: "formula_bar",
                    build: formula_bar::build,
                },
                StoryItem {
                    title: "sheet_grid",
                    build: sheet_grid::build,
                },
                StoryItem {
                    title: "app_suite",
                    build: app_suite::build,
                },
            ],
        ),
        (
            "图表与特效",
            vec![
                StoryItem {
                    title: "charts",
                    build: charts::build,
                },
                StoryItem {
                    title: "effects",
                    build: effects::build,
                },
                StoryItem {
                    title: "qr_code",
                    build: qr_code::build,
                },
            ],
        ),
        (
            "编辑器与工具",
            vec![StoryItem {
                title: "editor",
                build: editor::build,
            }],
        ),
    ]
}

/// 独立窗口运行入口：保留原 gallery 的初始化逻辑，避免辅助代码变成死代码。
/// 通过环境变量 `RGPUI_STORY_DEMO=<名称>` 触发。
pub fn run_standalone(name: &str) -> bool {
    match name {
        "hello_world" => {
            hello_world::run_example();
            true
        }
        "a11y" => {
            a11y::run_example();
            true
        }
        "theme" => {
            theme::run_example();
            true
        }
        "animation" => {
            animation::run_example();
            true
        }
        "icons" => {
            icons::run_example();
            true
        }
        "component_gallery" => {
            component_gallery::run_example();
            true
        }
        "extended_components" => {
            extended_components::run_example();
            true
        }
        "scrollable" => {
            scrollable::run_example();
            true
        }
        "sidebar" => {
            sidebar::run_example();
            true
        }
        "tabs" => {
            tabs::run_example();
            true
        }
        "fps_hud" => {
            fps_hud::run_example();
            true
        }
        "inspector" => {
            inspector::run_example();
            true
        }
        "inspector_custom" => {
            inspector_custom::run_example();
            true
        }
        "gradient" => {
            gradient::run_example();
            true
        }
        "painting" => {
            painting::run_example();
            true
        }
        "pattern" => {
            pattern::run_example();
            true
        }
        "anchor" => {
            anchor::run_example();
            true
        }
        "popover" => {
            popover::run_example();
            true
        }
        "i18n" => {
            i18n::run_example();
            true
        }
        "intl_text" => {
            intl_text::run_example();
            true
        }
        "markdown" => {
            markdown::run_example();
            true
        }
        "markdown_stream" => {
            markdown_stream::run_example();
            true
        }
        "rich_text" => {
            rich_text::run_example();
            true
        }
        "rich_clipboard" => {
            rich_clipboard::run_example();
            true
        }
        "text_layout" => {
            text_layout::run_example();
            true
        }
        "text_wrapper" => {
            text_wrapper::run_example();
            true
        }
        "ink" => {
            ink::run_example();
            true
        }
        "sorter" => {
            sorter::run_example();
            true
        }
        "timeline" => {
            timeline::run_example();
            true
        }
        "color_picker" => {
            color_picker::run_example();
            true
        }
        "mdi" => {
            mdi::run_example();
            true
        }
        "task_pane" => {
            task_pane::run_example();
            true
        }
        "canvas_a11y" => {
            canvas_a11y::run_example();
            true
        }
        "page_view" => {
            page_view::run_example();
            true
        }
        "drag_drop" => {
            drag_drop::run_example();
            true
        }
        "focus_visible" => {
            focus_visible::run_example();
            true
        }
        "mouse_pressure" => {
            mouse_pressure::run_example();
            true
        }
        "native_menu" => {
            native_menu::run_example();
            true
        }
        "native_notification" => {
            native_notification::run_example();
            true
        }
        "set_menus" => {
            set_menus::run_example();
            true
        }
        "tab_stop" => {
            tab_stop::run_example();
            true
        }
        "chat" => {
            chat::run_example();
            true
        }
        "dock" => {
            dock::run_example();
            true
        }
        "grid_layout" => {
            grid_layout::run_example();
            true
        }
        "status_bar" => {
            status_bar::run_example();
            true
        }
        "tree" => {
            tree::run_example();
            true
        }
        "image_loading" => {
            image_loading::run_example();
            true
        }
        "image_gallery" => {
            image_gallery::run_example();
            true
        }
        "svg" => {
            svg::run_example();
            true
        }
        "window" => {
            window::run_example();
            true
        }
        "opacity" => {
            opacity::run_example();
            true
        }
        "shadow" => {
            shadow::run_example();
            true
        }
        "transparent" => {
            transparent::run_example();
            true
        }
        "window_shadow" => {
            window_shadow::run_example();
            true
        }
        "list_example" => {
            list_example::run_example();
            true
        }
        "uniform_list" => {
            uniform_list::run_example();
            true
        }
        "data_table" => {
            data_table::run_example();
            true
        }
        "tree_list" => {
            tree_list::run_example();
            true
        }
        "backstage" => {
            backstage::run_example();
            true
        }
        "command_registry" => {
            command_registry::run_example();
            true
        }
        "mini_toolbar" => {
            mini_toolbar::run_example();
            true
        }
        "qat" => {
            qat::run_example();
            true
        }
        "ribbon" => {
            ribbon::run_example();
            true
        }
        "canvas_objects" => {
            canvas_objects::run_example();
            true
        }
        "ruler" => {
            ruler::run_example();
            true
        }
        "viewport" => {
            viewport::run_example();
            true
        }
        "formula_bar" => {
            formula_bar::run_example();
            true
        }
        "sheet_grid" => {
            sheet_grid::run_example();
            true
        }
        "app_suite" => {
            app_suite::run_example();
            true
        }
        "charts" => {
            charts::run_example();
            true
        }
        "effects" => {
            effects::run_example();
            true
        }
        "qr_code" => {
            qr_code::run_example();
            true
        }
        "editor" => {
            editor::run_example();
            true
        }
        _ => false,
    }
}
