//! 元素检查器示例：默认面板。
//!
//! - `cx.enable_default_inspector()` 一行启用默认面板（含 Div 布局展示）
//! - `window.toggle_inspector(cx)` 开关右侧面板（action `ToggleInspector` + F12）
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
//! cargo run -p inspector
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::input_ui::{Input, InputState};
use rgpui::{
    App, Bounds, Button, Checkbox, Context, Entity, IntoElement, ParentElement, Render,
    SharedString, Switch, Window, WindowBounds, WindowOptions, div, h_flex, prelude::*, px, rgb,
    size, v_flex,
};
#[cfg(any(feature = "inspector", debug_assertions))]
use rgpui::{KeyBinding, actions};
use rgpui_platform::application;

#[cfg(any(feature = "inspector", debug_assertions))]
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
                    ),
            )
    }
}

/// 章节标题辅助函数。
fn section_title(text: impl Into<SharedString> + IntoElement) -> impl IntoElement {
    div()
        .text_size(px(15.0))
        .font_semibold()
        .mt(px(8.0))
        .child(text)
}

/// 启动示例窗口：调试版本启用默认检查器面板 + F12 快捷键（release 自动剥离）。
fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::theme::init(cx);
        rgpui::input_ui::init(cx);
        rgpui::menu::init(cx);

        // 一行启用默认检查器面板（含 Div 布局展示注册）。
        // 下面整个块仅调试版本编译：release 下无检查器代码、无 F12 绑定。
        // F12 用全局绑定（无上下文）+ 全局监听打到活动窗口，不依赖视图焦点链。
        // 注意监听内必须 spawn 延后更新：按键分发中窗口已被 take 出来，
        // 同步 update_window 必失败（教训：勿用 `_ =` 吞掉 Result）。
        #[cfg(any(feature = "inspector", debug_assertions))]
        {
            cx.enable_default_inspector();

            cx.bind_keys([KeyBinding::new("f12", ToggleInspector, None)]);
            cx.on_action(|_: &ToggleInspector, cx: &mut App| {
                if let Some(window) = cx.active_window() {
                    cx.spawn(async move |cx| {
                        _ = window.update(cx, |_, window, cx| {
                            window.toggle_inspector(cx);
                        });
                    })
                    .detach();
                }
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

#[cfg(test)]
mod tests {
    use super::{InspectorDemo, ToggleInspector};
    use rgpui::{App, KeyBinding, TestAppContext};

    /// F12 全局开关回归测试：无焦点时也能打开检查器。
    ///
    /// 曾用带上下文绑定 + 视图 on_action：无焦点时分发路径只有 root，
    /// 上下文匹配不上、冒泡也到不了视图 handler，F12 完全没反应。
    /// 现用全局绑定（无上下文）+ 全局监听打到活动窗口。
    #[rgpui::test]
    fn f12_toggles_inspector_without_focus(cx: &mut TestAppContext) {
        use std::{cell::Cell, rc::Rc};
        let fired = Rc::new(Cell::new(false));
        let fired_clone = fired.clone();
        cx.update(|cx| {
            cx.bind_keys([KeyBinding::new("f12", ToggleInspector, None)]);
            cx.on_action(move |_: &ToggleInspector, cx: &mut App| {
                fired_clone.set(true);
                if let Some(window) = cx.active_window() {
                    cx.spawn(async move |cx| {
                        _ = window.update(cx, |_, window, cx| {
                            window.toggle_inspector(cx);
                        });
                    })
                    .detach();
                }
            });
        });
        let (_view, cx) = cx.add_window_view(InspectorDemo::new);
        cx.update(|window, cx| {
            // 测试平台的 App::activate 是空实现，这里直接激活窗口以设置 active_window。
            window.activate_window();
            // 初始无焦点、无检查器。
            assert!(!window.is_inspector_picking(cx));
            let _ = window.draw(cx);
        });
        // 无焦点直按 F12：检查器应打开（进入拾取态）。
        cx.update(|_, cx| assert!(cx.active_window().is_some()));
        cx.simulate_keystrokes("f12");
        assert!(fired.get(), "全局监听未触发：按键绑定未匹配");
        cx.update(|window, cx| {
            let _ = window.draw(cx);
            assert!(window.is_inspector_picking(cx));
        });
        // 再按 F12：检查器应关闭。
        cx.simulate_keystrokes("f12");
        cx.update(|window, cx| {
            let _ = window.draw(cx);
            assert!(!window.is_inspector_picking(cx));
        });
    }
}
