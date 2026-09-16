//! 元素检查器示例。
//!
//! 演示 `rgpui` 自带检查器的开箱用法：
//! - `cx.enable_default_inspector()` 一行启用默认面板（含 Div 布局展示）
//! - `window.toggle_inspector(cx)` 开关右侧面板（action `ToggleInspector` + F12）
//! - `cx.set_inspector_renderer` 整板替换：本例包一层自定义横幅，内部复用
//!   `rgpui::default_inspector_panel`（整板替换 recipe 的 living 范例）。
//!
//! 面板行为（默认面板提供）：拾取按钮（悬停蓝框、点击选中、滚轮穿透层级）、
//! 选中元素源码位置与布局边界、祖先链树、完整树（逐节点折叠）。
//!
//! 运行：
//!
//! ```text
//! cargo run -p inspector
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::input_ui::{Input, InputState};
use rgpui::{
    AnyElement, App, Bounds, Button, ButtonVariants, Checkbox, Context, Entity, Inspector,
    IntoElement, KeyBinding, ParentElement, Render, Switch, Window, WindowBounds, WindowOptions,
    actions, div, h_flex, prelude::*, px, rgb, size, v_flex,
};
use rgpui_platform::application;

actions!(inspector_demo, [ToggleInspector]);

/// 检查器演示根视图，持有演示区的输入状态与点击计数。
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

    /// 开关检查器面板（按钮与 F12 共用）。
    fn toggle_inspector(
        &mut self,
        _: &ToggleInspector,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.toggle_inspector(cx);
    }

    /// 演示按钮点击计数（证明非拾取态交互正常）。
    fn on_demo_click(&mut self, cx: &mut Context<Self>) {
        self.clicks += 1;
        cx.notify();
    }
}

impl Render for InspectorDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let click_view = view.clone();
        let notify_view = view.clone();
        let hobby_view = view.clone();

        div()
            .id("inspector-demo")
            .key_context("inspector-demo")
            .on_action(cx.listener(Self::toggle_inspector))
            .size_full()
            .bg(rgb(0xffffff))
            .child(
                v_flex()
                    .max_w(px(860.0))
                    .mx_auto()
                    .gap(px(16.0))
                    .p(px(24.0))
                    .child(div().text_2xl().child("元素检查器示例"))
                    .child(
                        div().text_sm().text_color(rgb(0x666666)).child(
                            "点「打开检查器」（或按 F12），右侧滑出面板；点「拾取元素」后悬停看蓝框、点击选中、滚轮穿透重叠层级。",
                        ),
                    )
                    // 工具栏：检查器开关。
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .items_center()
                            .child(
                                Button::new("open-inspector")
                                    .label("打开检查器")
                                    .primary()
                                    .on_click(move |_, window, cx| {
                                        window.toggle_inspector(cx);
                                    }),
                            )
                            .child(div().text_sm().child("或按 F12 开关")),
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
                                            .on_click(move |checked, _, cx| {
                                                hobby_view.update(cx, |this, cx| {
                                                    this.hobby = *checked;
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                    .child(
                                        Switch::new("demo-notify")
                                            .label("通知")
                                            .checked(self.notify)
                                            .on_click(move |checked, _, cx| {
                                                notify_view.update(cx, |this, cx| {
                                                    this.notify = *checked;
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
                    ),
            )
    }
}

/// 章节标题辅助函数。
fn section_title(text: impl Into<rgpui::SharedString> + IntoElement) -> impl IntoElement {
    div()
        .text_size(px(15.0))
        .font_semibold()
        .mt(px(8.0))
        .child(text)
}

/// 自定义检查器面板（整板替换 recipe 的 living 范例）。
///
/// `set_inspector_renderer` 整板替换，内部复用库默认面板
/// [`rgpui::default_inspector_panel`] 并在顶部包一层自定义横幅；
/// 按状态类型扩展则用 `register_inspector_element`（默认面板已注册 Div 布局展示）。
fn custom_inspector_panel(
    inspector: &mut Inspector,
    window: &mut Window,
    cx: &mut Context<Inspector>,
) -> AnyElement {
    div()
        .id("inspector-custom-panel")
        .size_full()
        .bg(rgb(0xf7f7f7))
        .child(
            v_flex()
                .size_full()
                .child(
                    v_flex()
                        .flex_none()
                        .mx(px(12.0))
                        .mt(px(12.0))
                        .gap(px(2.0))
                        .p(px(8.0))
                        .rounded_md()
                        .bg(rgb(0xffffff))
                        .border_1()
                        .border_color(rgb(0xe0e0e0))
                        .child(div().text_sm().font_semibold().child("自定义展示"))
                        .child(
                            div().text_xs().text_color(rgb(0x888888)).child(
                                "本面板 = 默认面板 + 顶部横幅：整板替换 recipe，内部复用 default_inspector_panel。",
                            ),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .child(rgpui::default_inspector_panel(inspector, window, cx)),
                ),
        )
        .into_any_element()
}

/// 面板主体已移入库默认面板（见 `rgpui::inspector_panel`），本示例仅保留开关演示与自定义横幅。
/// 启动示例窗口，一行启用默认检查器面板后再包自定义横幅，另设 F12 快捷键。
fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::theme::init(cx);
        rgpui::input_ui::init(cx);
        rgpui::menu::init(cx);

        // 一行启用默认检查器面板（含 Div 布局展示注册），
        // 再整板替换为“默认面板 + 自定义横幅”（整板替换 recipe）。
        cx.enable_default_inspector();
        cx.set_inspector_renderer(Box::new(custom_inspector_panel));

        cx.bind_keys([KeyBinding::new(
            "f12",
            ToggleInspector,
            Some("inspector-demo"),
        )]);

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

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
/// WASM 入口：初始化 Web 环境后启动示例。
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
