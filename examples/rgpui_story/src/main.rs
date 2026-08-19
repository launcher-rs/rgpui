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

// 在 wasm 目标上禁用 main 函数入口
#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::prelude::FluentBuilder;
use rgpui::{
    App, AppContext as _, Context, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, WindowOptions, div, px, size,
};
use rgpui_platform::application;

mod stories;

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
        // 对话框层：由用户视图手动挂载（Root 自身不挂载），否则对话框无法显示。
        let dialog_layer = rgpui::Root::render_dialog_layer(window, cx);

        div()
            .id("story-app")
            .size_full()
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
                    .overflow_y_scroll()
                    .when_some(content, |d, view| d.child(view)),
            )
            // 对话框层必须作为最后一个子元素挂载，否则会绘制在侧边栏/内容之下，
            // 导致遮罩无法遮蔽下层内容、点击事件穿透到下层按钮。
            .when_some(dialog_layer, |d, layer| d.child(layer))
    }
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
    application().run(|cx: &mut App| {
        // 依次初始化各子系统：主题、输入（含数字输入）、菜单（含全局状态）、
        // 列表、表格与扩展组件（快捷键绑定）。
        rgpui::theme::init(cx);
        rgpui::input_ui::init(cx);
        rgpui::menu::init(cx);
        rgpui::list::init(cx);
        rgpui::table::init(cx);
        rgpui::components::init(cx);

        let window_options = WindowOptions {
            window_background: rgpui::WindowBackgroundAppearance::Opaque,
            titlebar: Some(rgpui::TitlebarOptions {
                title: Some("rgpui 组件示例大全".into()),
                ..Default::default()
            }),
            // 设置合理的默认窗口尺寸，避免默认尺寸过大。
            window_bounds: Some(rgpui::WindowBounds::centered(
                size(px(1100.0), px(720.0)),
                cx,
            )),
            ..Default::default()
        };

        let _ = cx.open_window(window_options, |window, cx| {
            cx.new(|cx| StoryApp::new(window, cx))
        });
    });
}
