//! 元素检查器示例：自定义面板（整板替换 recipe 的 living 范例）。
//!
//! 本例面板为完全自写（不用 `default_inspector_panel`）：自有顶栏/拾取按钮、
//! 选中信息卡、单层子节点列表，并以 `register_inspector_element` 覆盖默认的
//! Div 布局展示。本例与 `inspector` 各自独立（演示内容各自一份）。
//!
//! 运行：
//!
//! ```text
//! cargo run -p inspector_custom
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::input_ui::{Input, InputState};
#[cfg(any(feature = "inspector", debug_assertions))]
use rgpui::{
    AnyElement, ButtonVariants as _, Div, DivInspectorState, Inspector, InspectorElementId,
    KeyBinding, actions,
};
use rgpui::{
    App, Bounds, Button, Checkbox, Context, Entity, IntoElement, ParentElement, Render,
    SharedString, Switch, Window, WindowBounds, WindowOptions, div, h_flex, prelude::*, px, rgb,
    size, v_flex,
};
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

/// 自定义检查器面板：自写布局（顶栏 + 选中卡 + 单层子节点），不复用默认面板。
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

/// 自定义面板的信息行（与默认面板样式区分：标签更宽）。
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

/// 自定义的 `DivInspectorState` 展示：覆盖默认注册，单行紧凑式布局。
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

/// 启动示例窗口：调试版本装配自定义检查器面板 + F12 快捷键（release 自动剥离）。
fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::theme::init(cx);
        rgpui::input_ui::init(cx);
        rgpui::menu::init(cx);

        // 下面整个块仅调试版本编译：release 下无检查器代码、无 F12 绑定。
        // F12 用全局绑定（无上下文）+ 全局监听打到活动窗口，不依赖视图焦点链。
        // 注意监听内必须 spawn 延后更新：按键分发中窗口已被 take 出来，
        // 同步 update_window 必失败（教训：勿用 `_ =` 吞掉 Result）。
        #[cfg(any(feature = "inspector", debug_assertions))]
        {
            cx.enable_default_inspector();
            // 整板替换 + 按类型覆盖 Div 展示（同 TypeId 后注册覆盖先注册）。
            cx.set_inspector_renderer(Box::new(custom_inspector_panel));
            cx.register_inspector_element(custom_div_state);

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
