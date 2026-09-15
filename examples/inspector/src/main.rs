//! 元素检查器示例。
//!
//! 演示 `rgpui` 自带检查器的完整用法：
//! - `window.toggle_inspector(cx)` 开关右侧检查器面板（-action `ToggleInspector` + F12）；
//! - `cx.set_inspector_renderer` 提供面板 UI（本例显示拾取状态、选中元素源码位置与布局边界）；
//! - `cx.register_inspector_element` 注册 `DivInspectorState` 的展示；
//! - 面板内「拾取元素」进入拾取模式：悬停高亮（蓝框）、点击选中、滚轮穿透重叠层级。
//!
//! 注意：拾取中时画布点击被检查器接管（点任意元素即完成拾取并恢复交互）。
//!
//! 运行：
//!
//! ```text
//! cargo run -p inspector
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::input_ui::{Input, InputState};
use rgpui::{
    AnyElement, App, Bounds, Button, ButtonVariants, Checkbox, ClipboardItem, Context, Div,
    DivInspectorState, Entity, Global, Inspector, InspectorElementId, IntoElement, KeyBinding,
    ParentElement, Render, Switch, Window, WindowBounds, WindowOptions, actions, div, h_flex,
    prelude::*, px, rgb, size, v_flex,
};
use rgpui_platform::application;

actions!(inspector_demo, [ToggleInspector]);

/// 祖先链超过该层级数时折叠中间层。
const SPINE_COLLAPSE_THRESHOLD: usize = 8;
/// 折叠时保留的头部层级数。
const SPINE_HEAD_KEPT: usize = 2;
/// 折叠时保留的尾部层级数。
const SPINE_TAIL_KEPT: usize = 3;

/// 元素树折叠/复制的面板本地 UI 状态（存 App 全局）。
#[derive(Default)]
struct TreeUiState {
    /// 当前选中元素的标识（路径 + 实例），变化时重置折叠/复制态。
    selected_key: String,
    /// 长链是否展开。
    expanded: bool,
    /// 已复制路径的层级索引。
    copied: Option<usize>,
}

impl Global for TreeUiState {}

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

/// 检查器面板 UI（经 `set_inspector_renderer` 注册）。
///
/// 展示拾取状态、选中元素的源码位置、实例号、元素树（祖先链），
/// 以及已注册的状态渲染器输出（如 `DivInspectorState` 的边界信息）。
fn inspector_panel(
    inspector: &mut Inspector,
    window: &mut Window,
    cx: &mut Context<Inspector>,
) -> AnyElement {
    let entity = cx.entity();
    let picking = inspector.is_picking();
    // 先物化选中信息为 owned 数据，结束借用后再渲染各状态。
    // 祖先链：GlobalElementId 本身就是从根到选中元素的 id 路径。
    let selected: Option<(Vec<String>, String, usize)> = inspector.active_element_id().map(|id| {
        let loc = id.path.source_location;
        let spine = id.path.global_id.iter().map(|e| e.to_string()).collect();
        (
            spine,
            format!("{}:{}", loc.file(), loc.line()),
            id.instance_id,
        )
    });
    let status = if picking {
        "拾取中"
    } else if selected.is_some() {
        "已选中"
    } else {
        "空闲"
    };
    // 同步树 UI 状态：选中变化时重置折叠/复制态。
    let (expanded, copied) = cx.update_default_global::<TreeUiState, _>(|state, _| {
        let key = selected
            .as_ref()
            .map(|(spine, _, instance)| format!("{}#{instance}", spine.join(".")))
            .unwrap_or_default();
        if state.selected_key != key {
            state.selected_key = key;
            state.expanded = false;
            state.copied = None;
        }
        (state.expanded, state.copied)
    });
    let states = inspector.render_inspector_states(window, cx);

    div()
        .id("inspector-panel")
        .size_full()
        .overflow_scroll()
        .bg(rgb(0xf7f7f7))
        .child(
            v_flex()
                .gap(px(8.0))
                .p(px(12.0))
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(div().text_lg().font_semibold().child("元素检查器"))
                        .child(
                            div()
                                .text_xs()
                                .px(px(8.0))
                                .py(px(2.0))
                                .rounded_full()
                                .bg(rgb(0xe8f0fe))
                                .child(status),
                        ),
                )
                .child(
                    Button::new("inspector-pick")
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
                .child(
                    div().text_xs().text_color(rgb(0x888888)).child(
                        "悬停高亮蓝框，点击选中元素；重叠处滚轮切换层级。拾取中时画布点击被接管，点任意元素即完成拾取。树节点点击复制该层路径，超长链自动折叠。",
                    ),
                )
                .when_some(selected, |this, (spine, loc, instance)| {
                    this.child(
                        v_flex()
                            .gap(px(2.0))
                            .p(px(8.0))
                            .rounded_md()
                            .bg(rgb(0xffffff))
                            .border_1()
                            .border_color(rgb(0xe0e0e0))
                            .child(div().text_sm().font_semibold().child("选中元素"))
                            .child(info_row("源码", loc))
                            .child(info_row("实例", format!("#{instance}")))
                            .child(
                                div()
                                    .text_sm()
                                    .font_semibold()
                                    .mt(px(4.0))
                                    .child("元素树（祖先链）"),
                            )
                            .child(element_spine(spine, expanded, copied)),
                    )
                })
                .children(states),
        )
        .into_any_element()
}

/// 祖先链渲染为缩进树（HTML 元素树式，根在上、选中在下）。
///
/// 框架为即时模式，每帧后不保留完整元素树；但选中元素的
/// `GlobalElementId` 本身就是从根到该元素的祖先路径，可据此渲染树脊。
/// 超长链自动折叠中间层；点击节点复制该层完整路径，点击折叠行展开/收起。
fn element_spine(spine: Vec<String>, expanded: bool, copied: Option<usize>) -> impl IntoElement {
    let depth = spine.len();
    let collapsible = depth > SPINE_COLLAPSE_THRESHOLD;
    // 折叠时仅保留首尾若干层。
    let visible: Vec<(usize, &String)> = if collapsible && !expanded {
        spine
            .iter()
            .enumerate()
            .take(SPINE_HEAD_KEPT)
            .chain(spine.iter().enumerate().skip(depth - SPINE_TAIL_KEPT))
            .collect()
    } else {
        spine.iter().enumerate().collect()
    };
    v_flex()
        .gap(px(1.0))
        .children(visible.into_iter().map(|(ix, name)| {
            let last = ix + 1 == depth;
            let path_to_here = spine[..=ix].join(".");
            spine_row(ix, name, last, copied == Some(ix), path_to_here)
        }))
        // 折叠行：展示被隐藏的层级数，点击展开。
        .when(collapsible && !expanded, |this| {
            this.child(
                div()
                    .id("spine-toggle")
                    .text_xs()
                    .text_color(rgb(0x666666))
                    .ml(px(SPINE_HEAD_KEPT as f32 * 12.0))
                    .py(px(2.0))
                    .cursor_pointer()
                    .child(format!(
                        "⋯ {} 级已折叠，点击展开",
                        depth - SPINE_HEAD_KEPT - SPINE_TAIL_KEPT
                    ))
                    .on_click(|_, _, cx| {
                        cx.update_default_global::<TreeUiState, _>(|state, _| {
                            state.expanded = true;
                        });
                    }),
            )
        })
        // 展开态收起行。
        .when(collapsible && expanded, |this| {
            this.child(
                div()
                    .id("spine-toggle")
                    .text_xs()
                    .text_color(rgb(0x666666))
                    .py(px(2.0))
                    .cursor_pointer()
                    .child("收起，点击折叠")
                    .on_click(|_, _, cx| {
                        cx.update_default_global::<TreeUiState, _>(|state, _| {
                            state.expanded = false;
                        });
                    }),
            )
        })
        .when(depth == 0, |this| {
            this.child(div().text_xs().child("(根元素，无祖先路径)"))
        })
}

/// 树节点行：缩进 + 连接符 + 名称，点击复制该层完整路径并打钩反馈。
fn spine_row(
    ix: usize,
    name: &str,
    last: bool,
    is_copied: bool,
    path_to_here: String,
) -> impl IntoElement {
    h_flex()
        .id(format!("spine-node-{ix}"))
        .items_center()
        .gap(px(4.0))
        .ml(px(ix as f32 * 12.0))
        .py(px(1.0))
        .px(px(4.0))
        .rounded_sm()
        .cursor_pointer()
        .hover(|this| this.bg(rgb(0xeeeeee)))
        .when(last, |this| this.bg(rgb(0xe8f0fe)))
        .on_click(move |_, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(path_to_here.clone()));
            cx.update_default_global::<TreeUiState, _>(|state, _| {
                state.copied = Some(ix);
            });
        })
        .child(div().text_xs().text_color(rgb(0x999999)).child(if last {
            "└─"
        } else {
            "├─"
        }))
        .child(
            div()
                .flex_1()
                .text_xs()
                .text_color(rgb(0x333333))
                .child(if name.is_empty() {
                    "(空)".to_string()
                } else {
                    name.to_string()
                }),
        )
        .when(is_copied, |this| {
            this.child(div().text_xs().text_color(rgb(0x107c10)).child("✓"))
        })
}

/// 面板信息行辅助函数。
fn info_row(label: &str, value: String) -> impl IntoElement {
    h_flex()
        .gap(px(6.0))
        .child(div().w(px(36.0)).text_xs().child(label.to_string()))
        .child(
            div()
                .flex_1()
                .text_xs()
                .text_color(rgb(0x333333))
                .child(value),
        )
}

/// `DivInspectorState` 的检查器展示（经 `register_inspector_element` 注册）。
///
/// 显示被选中 Div 的布局边界与内容尺寸。
fn render_div_state(
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
        .child(div().text_sm().font_semibold().child("布局"))
        .child(info_row(
            "边界",
            format!(
                "x {:.0} · y {:.0} · w {:.0} · h {:.0}",
                bounds.origin.x.as_f32(),
                bounds.origin.y.as_f32(),
                bounds.size.width.as_f32(),
                bounds.size.height.as_f32()
            ),
        ))
        .child(info_row(
            "内容",
            format!(
                "w {:.0} · h {:.0}",
                state.content_size.width.as_f32(),
                state.content_size.height.as_f32()
            ),
        ))
}

/// 启动示例窗口，注册检查器面板、Div 状态展示与 F12 快捷键。
fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::theme::init(cx);
        rgpui::input_ui::init(cx);
        rgpui::menu::init(cx);

        // 注册检查器面板 UI 与 Div 布局状态展示。
        cx.set_inspector_renderer(Box::new(inspector_panel));
        cx.register_inspector_element(render_div_state);

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
