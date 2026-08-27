//! rgpui 组件示例大全（Storybook）。
//!
//! 通过左侧侧边栏分类导航，右侧内容区展示对应组件的用法示例。
//! 覆盖核心基础组件、输入/表单、菜单、对话框、列表、表格、标签页、
//! 扩展组件（原 rgpui-ui 并入）与特效/图表（feature 门控）。
//!
//! 运行：
//!
//! ```text
//! cargo run -p rgpui_story
//! ```

#[cfg(target_family = "wasm")]
use std::borrow::Cow;
#[cfg(target_family = "wasm")]
use std::cell::RefCell;

use rgpui::prelude::FluentBuilder;
use rgpui::title_bar::TitleBar;
use rgpui::{
    ActiveTheme, App, AppContext as _, Button, ButtonVariants, Context, DropdownMenu, IconName,
    InteractiveElement, IntoElement, ParentElement, PopupMenuItem, Render, Sizable,
    StatefulInteractiveElement, Styled, Theme, ThemeMode, ThemeRegistry, Window, WindowOptions,
    div, h_flex, px, size,
};
#[cfg(not(target_family = "wasm"))]
use rgpui_platform::application;

mod stories;

// 持有 wasm 下 ApplicationHandle，防止 App 在启动回调返回后被释放。
//
// Web 平台的 `run` 通过 `spawn_local` 启动后立即返回，若句柄被丢弃，
// 事件监听器闭包会被回收而 DOM 监听仍挂载，导致
// "closure invoked recursively or after being dropped" 崩溃。
#[cfg(target_family = "wasm")]
thread_local! {
    static APPLICATION: RefCell<Option<rgpui::ApplicationHandle>> = const { RefCell::new(None) };
}

use stories::{StoryItem, registry};

/// 故事书根视图：左侧导航 + 右侧内容区。
struct StoryApp {
    /// 故事分类列表。
    groups: Vec<(&'static str, Vec<StoryItem>)>,
    /// 当前激活的故事索引（分类, 故事）。
    active: (usize, usize),
    /// 所有故事视图（与 `groups` 扁平化索引对应），一次性创建以保持状态。
    views: Vec<rgpui::AnyView>,
}

impl StoryApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let groups = registry();
        // 一次性构建所有故事视图，保证状态实体跨帧存活。
        let mut views = Vec::new();
        for (_, items) in &groups {
            for item in items {
                views.push((item.build)(window, cx));
            }
        }
        Self {
            groups,
            active: (0, 0),
            views,
        }
    }

    /// 计算扁平化视图索引并返回当前激活视图。
    fn active_view(&self) -> Option<rgpui::AnyView> {
        let (gix, six) = self.active;
        let mut flat = 0;
        for (ix, (_, items)) in self.groups.iter().enumerate() {
            if ix < gix {
                flat += items.len();
            } else if ix == gix {
                return self.views.get(flat + six).cloned();
            }
        }
        None
    }
}

impl Render for StoryApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 内容区：渲染当前激活的故事视图。
        let content = self.active_view();
        // 复制当前激活索引（Copy），供下方闭包内构造唯一 id 使用，避免借用 self。
        let active = self.active;
        // 对话框层：由用户视图手动挂载（Root 自身不挂载），否则对话框无法显示。
        let dialog_layer = rgpui::Root::render_dialog_layer(window, cx);

        div()
            .id("story-app")
            .size_full()
            .flex()
            .flex_col()
            .child(title_bar(window, cx))
            .child(
                div()
                    .id("story-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .flex()
                    .child(
                        div()
                            .id("story-sidebar")
                            .h_full()
                            .w(px(240.0))
                            .flex_col()
                            .child(
                                div()
                                    .p(px(16.0))
                                    .child(div().child("rgpui 组件示例大全").text_size(px(16.0))),
                            )
                            .child(sidebar_nav(self, window, cx)),
                    )
                    .child(
                        div()
                            .id("story-content")
                            .flex_1()
                            .h_full()
                            .min_w_0()
                            .overflow_y_scroll()
                            .when_some(content, |d, view| {
                                // 给当前激活的故事视图包一层「按分类+条目」唯一且跨帧稳定的 id，
                                // 使其 DOM 节点的 key 与其他视图区分。否则不同视图的根元素会作为
                                // story-content 下的匿名子节点共享同一 key，切换组件时互相碰撞、
                                // 样式错乱（ViewElement 是透传包裹，本身不产生 DOM 节点）。
                                let (gix, six) = active;
                                d.child(div().id(format!("story-view-{gix}-{six}")).child(view))
                            }),
                    ),
            )
            // 对话框层必须作为最后一个子元素挂载，否则会绘制在侧边栏/内容之下，
            // 导致遮罩无法遮蔽下层内容、点击事件穿透到下层按钮。
            .when_some(dialog_layer, |d, layer| d.child(layer))
    }
}

/// 渲染自定义标题栏，右侧提供主题切换下拉菜单。
fn title_bar(_window: &mut Window, cx: &mut Context<StoryApp>) -> impl IntoElement {
    let theme = cx.theme();
    let current_name = theme.theme_name().clone();

    // 提前收集主题元数据，克隆进 'static 菜单构建闭包。
    let items: Vec<(rgpui::SharedString, ThemeMode, bool)> = ThemeRegistry::global(cx)
        .sorted_themes()
        .into_iter()
        .map(|theme| {
            let name = theme.name.clone();
            (name.clone(), theme.mode, name == current_name)
        })
        .collect();

    TitleBar::new().child(
        h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .px_2()
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(theme.muted_foreground)
                    .child("rgpui 组件示例大全"),
            )
            .child(
                Button::new("title-bar-theme")
                    .small()
                    .ghost()
                    .icon(IconName::Palette)
                    .label(current_name)
                    .dropdown_menu(move |menu, _, _| {
                        let mut menu = menu.label("选择主题");
                        for (name, mode, checked) in &items {
                            let name = name.clone();
                            let mode = *mode;
                            menu = menu.item(
                                PopupMenuItem::new(name.clone()).checked(*checked).on_click(
                                    move |_, window, cx| {
                                        Theme::change(mode, Some(window), cx);
                                        // wasm 下主题切换不会重置内嵌字体（配置无 font_family），
                                        // 但保持与桌面一致的健壮性，切换后显式恢复内嵌字体族。
                                        #[cfg(target_family = "wasm")]
                                        {
                                            let theme = cx.global_mut::<Theme>();
                                            theme.font_family = "Inter Variable".into();
                                            theme.mono_font_family = "JetBrains Mono".into();
                                        }
                                        cx.refresh_windows();
                                    },
                                ),
                            );
                        }
                        menu
                    }),
            ),
    )
}

/// 渲染侧边栏分类导航：每组标题 + 该组下的故事条目按钮。
fn sidebar_nav(
    this: &mut StoryApp,
    _window: &mut Window,
    cx: &mut Context<StoryApp>,
) -> impl IntoElement {
    let active = this.active;
    let mut nav = div()
        .id("story-nav")
        .w_full()
        .h_full()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .p(px(8.0));

    for (gix, (group_title, items)) in this.groups.iter().enumerate() {
        nav = nav
            .child(
                div()
                    .p(px(8.0))
                    .text_color(rgpui::hsla(0.0, 0.0, 0.5, 0.7))
                    .child(*group_title),
            )
            .children(items.iter().enumerate().map(|(six, item)| {
                let selected = active == (gix, six);
                let item_title = item.title;
                let on_click = cx.listener(
                    move |this: &mut StoryApp, _: &rgpui::ClickEvent, window, cx| {
                        this.active = (gix, six);
                        window.refresh();
                        cx.notify();
                    },
                );

                div()
                    .id(rgpui::ElementId::Name(
                        format!("story-{gix}-{six}-{item_title}").into(),
                    ))
                    .w_full()
                    .p(px(8.0))
                    .rounded(px(6.0))
                    .when(selected, |d| d.bg(rgpui::hsla(0.6, 0.6, 0.55, 0.25)))
                    .hover(|d| d.bg(rgpui::hsla(0.6, 0.6, 0.55, 0.12)))
                    .on_click(on_click)
                    .child(item_title)
            }))
            .child(div().h(px(8.0)));
    }

    nav
}

fn main() {
    // wasm 下初始化 panic hook 与日志系统。
    #[cfg(target_family = "wasm")]
    rgpui_platform::web_init();

    // wasm 下启用 DOM 文本覆盖层：让浏览器原生提供文本选择/复制/IME 能力
    // （在 canvas 之上叠加绘制，v1 接受双重绘制）。可在打开窗口前按需关闭：
    // rgpui::set_dom_layer_enabled(false)。
    #[cfg(target_family = "wasm")]
    rgpui::set_dom_layer_enabled(true);

    // wasm 使用单线程 Web 平台（无需 SharedArrayBuffer/atomics），
    // 桌面端使用默认平台。
    let app = {
        #[cfg(target_family = "wasm")]
        {
            rgpui_platform::single_threaded_web()
        }
        #[cfg(not(target_family = "wasm"))]
        {
            application()
        }
    };

    let launch = move |cx: &mut App| {
        // wasm 下无系统字体，需内嵌加载应用用到的字体族（CJK/Emoji 通过回退匹配）。
        #[cfg(target_family = "wasm")]
        {
            let ui_font = Cow::Borrowed(include_bytes!("../fonts/Inter-Regular.ttf").as_slice());
            let mono_font =
                Cow::Borrowed(include_bytes!("../fonts/JetBrainsMono-Regular.ttf").as_slice());
            let emoji_font =
                Cow::Borrowed(include_bytes!("../fonts/NotoEmoji-Regular.ttf").as_slice());
            let cjk_font =
                Cow::Borrowed(include_bytes!("../fonts/NotoSansSC-Regular-subset.ttf").as_slice());
            cx.text_system()
                .add_fonts(vec![ui_font, mono_font, emoji_font, cjk_font])
                .expect("字体加载失败");
            // 把内嵌字体同步注册给 DOM 覆盖层（@font-face），避免双重绘制时
            // 浏览器回退到默认字体造成“重影”。
            rgpui::set_dom_font_face(
                "Inter Variable",
                include_bytes!("../fonts/Inter-Regular.ttf"),
            );
            rgpui::set_dom_font_face(
                "JetBrains Mono",
                include_bytes!("../fonts/JetBrainsMono-Regular.ttf"),
            );
        }

        // 依次初始化各子系统：主题、输入（含数字输入）、菜单（含全局状态）、
        // 列表、表格与扩展组件（快捷键绑定）。
        rgpui::theme::init(cx);
        rgpui::input_ui::init(cx);
        rgpui::menu::init(cx);
        rgpui::list::init(cx);
        rgpui::table::init(cx);
        rgpui::components::init(cx);

        // wasm 下指定内嵌字体族，避免回退到不存在的系统字体。
        #[cfg(target_family = "wasm")]
        {
            let theme = cx.global_mut::<Theme>();
            theme.font_family = "Inter Variable".into();
            theme.mono_font_family = "JetBrains Mono".into();
        }

        let window_options = WindowOptions {
            window_background: rgpui::WindowBackgroundAppearance::Opaque,
            // 设置合理的默认窗口尺寸，避免默认尺寸过大。
            window_bounds: Some(rgpui::WindowBounds::centered(
                size(px(1100.0), px(720.0)),
                cx,
            )),
            // 使用自定义 TitleBar：隐藏系统标题栏，由 StoryApp 自行渲染标题栏。
            ..TitleBar::window_options()
        };

        let _ = cx.open_window(window_options, |window, cx| {
            cx.new(|cx| StoryApp::new(window, cx))
        });
    };

    // wasm 下通过 run_embedded 持有 ApplicationHandle，避免 App 在启动后即被释放；
    // 桌面端 run 会阻塞直至退出。
    #[cfg(target_family = "wasm")]
    APPLICATION.with(|application| {
        *application.borrow_mut() = Some(app.run_embedded(launch));
    });
    #[cfg(not(target_family = "wasm"))]
    app.run(launch);
}
