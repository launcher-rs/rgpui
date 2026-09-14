//! 自定义标题栏示例：`TitleBar` 的完整用法。
//!
//! 运行：`cargo run -p window_showcase --bin custom_title_bar`
//!
//! 演示内容：
//! - 用 `TitleBar::window_options()` 构造窗口选项（推荐做法，
//!   已包含 `app_owns_titlebar_drag = true`，macOS 下双击/拖拽行为正常）；
//! - 用 `TitleBar::new()` 渲染自定义标题栏，左侧放标题、右侧放操作区；
//! - 标题栏内可放置可交互元素（按钮点击不会被拖拽吞掉）；
//! - 右侧窗口控制按钮（最小化/最大化/关闭）由 `TitleBar` 自动绘制，
//!   Windows/Linux 生效，macOS 显示红绿灯并预留左侧间距，Web 不显示；
//! - 内容区复用用户原示例：问候语 + 计数器 + 颜色块。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::title_bar::TitleBar;
use rgpui::{
    App, Bounds, Context, Entity, Hsla, SharedString, Window, WindowBounds, div, h_flex,
    prelude::*, px, rgb, size, v_flex,
};
use rgpui_platform::application;

/// 示例根视图，保存问候语与计数器状态。
struct TitleBarDemo {
    /// 问候语中的名字。
    text: SharedString,
    /// 标题栏与内容区共享的计数器。
    count: usize,
}

/// 标题栏左侧的小标题文本样式。
fn title_label(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_sm()
        .font_weight(rgpui::FontWeight::SEMIBOLD)
        .child(text.into())
}

/// 标题栏内的文字按钮，点击不会触发窗口拖拽。
fn title_bar_button(
    id: &'static str,
    label: SharedString,
    entity: Entity<TitleBarDemo>,
    on_click: fn(&mut TitleBarDemo),
) -> impl IntoElement {
    div()
        .id(id)
        .px_2()
        .py_0p5()
        .text_xs()
        .rounded_md()
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0xe8e8e8)))
        .active(|style| style.bg(rgb(0xd8d8d8)))
        .child(label)
        .on_click(move |_, _, cx| {
            entity.update(cx, |view, cx| {
                on_click(view);
                cx.notify();
            });
        })
}

/// 内容区按钮。
fn content_button(
    id: &'static str,
    label: SharedString,
    entity: Entity<TitleBarDemo>,
    on_click: fn(&mut TitleBarDemo),
) -> impl IntoElement {
    div()
        .id(id)
        .px_3()
        .py_1()
        .text_sm()
        .bg(rgb(0xf0f0f0))
        .border_1()
        .border_color(rgb(0xd0d0d0))
        .rounded_md()
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0xe4e4e4)))
        .active(|style| style.bg(rgb(0xd4d4d4)))
        .child(label)
        .on_click(move |_, _, cx| {
            entity.update(cx, |view, cx| {
                on_click(view);
                cx.notify();
            });
        })
}

/// 内容区颜色块。
fn color_swatch(color: Hsla, border: Hsla) -> impl IntoElement {
    div()
        .size_8()
        .bg(color)
        .border_1()
        .border_dashed()
        .rounded_md()
        .border_color(border)
}

/// 计数器加一。
fn increment(view: &mut TitleBarDemo) {
    view.count += 1;
}

/// 计数器减一，饱和到零。
fn decrement(view: &mut TitleBarDemo) {
    view.count = view.count.saturating_sub(1);
}

impl Render for TitleBarDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 当前实体句柄，供标题栏/内容区按钮回调更新计数器。
        let entity = cx.entity();
        // 当前计数值，供标题栏与内容区同时展示。
        let count = self.count;

        v_flex()
            .size_full()
            .bg(rgb(0xffffff))
            // 自定义标题栏：拖拽、双击缩放、窗口控制按钮都由 TitleBar 内部处理。
            .child(
                TitleBar::new().child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .pr_2()
                        .justify_between()
                        // 左侧：应用名 + 问候对象。
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(title_label("gpui 学习"))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0x888888))
                                        .child(format!("Hello, {}!", self.text)),
                                ),
                        )
                        // 右侧：计数展示 + 可交互按钮 + 占位文本。
                        // 右侧的最小化/最大化/关闭按钮由 TitleBar 自动追加。
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0x888888))
                                        .child(format!("count: {count}")),
                                )
                                .child(title_bar_button(
                                    "titlebar-inc",
                                    "标题栏 +1".into(),
                                    entity.clone(),
                                    increment,
                                ))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0xaaaaaa))
                                        .child("Right Item"),
                                ),
                        ),
                ),
            )
            // 内容区：问候语 + 计数器 + 颜色块（源自用户原示例，清理了重复修饰）。
            .child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .p_6()
                    .child(div().text_xl().child(format!("Hello, {}!", self.text)))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(content_button(
                                "content-dec",
                                "-1".into(),
                                entity.clone(),
                                decrement,
                            ))
                            .child(
                                div()
                                    .text_lg()
                                    .w(px(48.0))
                                    .text_center()
                                    .child(format!("{count}")),
                            )
                            .child(content_button(
                                "content-inc",
                                "+1".into(),
                                entity.clone(),
                                increment,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(color_swatch(rgpui::red(), rgpui::white()))
                            .child(color_swatch(rgpui::green(), rgpui::white()))
                            .child(color_swatch(rgpui::blue(), rgpui::white()))
                            .child(color_swatch(rgpui::yellow(), rgpui::white()))
                            .child(color_swatch(rgpui::black(), rgpui::white()))
                            .child(color_swatch(rgpui::white(), rgpui::black())),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_center()
                            .text_color(rgb(0x999999))
                            .child(
                                "拖拽标题栏空白处移动窗口，双击缩放；\
                                 右侧控制按钮 Windows/Linux 可用，macOS 用红绿灯。",
                            ),
                    ),
            )
    }
}

/// 启动示例窗口。
fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(560.0), px(420.0)), cx);
        // 推荐用 TitleBar::window_options() 做底，再补窗口尺寸与最小尺寸。
        let mut options = TitleBar::window_options();
        options.window_bounds = Some(WindowBounds::Windowed(bounds));
        options.window_min_size = Some(size(px(400.0), px(300.0)));

        cx.open_window(options, |_, cx| {
            cx.new(|_| TitleBarDemo {
                text: "World".into(),
                count: 0,
            })
        })
        .unwrap();
        cx.activate(true);
    });
}

/// 非 Web 入口。
#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

/// Web 入口。
#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
