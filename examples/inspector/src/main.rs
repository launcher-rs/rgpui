//! 元素检查器示例：默认面板。
//!
//! - `cx.enable_default_inspector()` 一行启用默认面板（含 Div 布局展示）
//! - `window.toggle_inspector(cx)` 开关右侧面板（action `ToggleInspector` + F12）
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
//! cargo run -p inspector
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

#[cfg(any(feature = "inspector", debug_assertions))]
use rgpui::actions;
use rgpui::input_ui::{Input, InputState};
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
                    .child(crash_ai_section(view.clone(), self.export_note.clone())),
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

/// “崩溃快照 & AI 导出”演示区（仅调试版本编译；release 下为空行，原样剥离）。
///
/// 两个复制按钮即文档“喂给 AI”节的程序化入口：
/// 树文本（`Window::inspector_tree_text`，Markdown 缩进树）与结构化快照
/// （`Window::capture_inspector_snapshot`，JSON 可序列化）。
/// 导出要求检查器已打开（未打开时两个 API 都返回 `None`），按钮会提示先按 F12；
/// 崩溃落盘（`last.json` 约 2 秒一写 + `panic-*.log`）在 `run_example` 里开启，
/// 程序死了直接读 `.rgpui-crash` 目录即可，AI 同理可读。
#[cfg(any(feature = "inspector", debug_assertions))]
fn crash_ai_section(view: Entity<InspectorDemo>, note: Option<String>) -> impl IntoElement {
    let tree_view = view.clone();
    let snapshot_view = view.clone();
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

/// 发布剥离版占位：与调试版同签名，渲染空行。
#[cfg(not(any(feature = "inspector", debug_assertions)))]
fn crash_ai_section(_view: Entity<InspectorDemo>, _note: Option<String>) -> impl IntoElement {
    div()
}

/// 启动示例窗口：调试版本启用默认检查器面板 + F12 快捷键（release 自动剥离）。
fn run_example() {
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
    use rgpui::TestAppContext;

    /// F12 全局开关回归测试：无焦点时也能打开检查器。
    ///
    /// 曾用带上下文绑定 + 视图 on_action：无焦点时分发路径只有 root，
    /// 上下文匹配不上、冒泡也到不了视图 handler，F12 完全没反应。
    /// 现用全局动作 helper（全局绑定 + 打活动窗口 + spawn 延后更新）。
    #[rgpui::test]
    fn f12_toggles_inspector_without_focus(cx: &mut TestAppContext) {
        use std::{cell::Cell, rc::Rc};
        let fired = Rc::new(Cell::new(false));
        let fired_clone = fired.clone();
        cx.update(|cx| {
            cx.on_global_action(ToggleInspector, Some("f12"), move |window, cx| {
                fired_clone.set(true);
                window.toggle_inspector(cx);
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

    /// AI 导出链路回归测试：检查器打开时树文本与结构化快照可用。
    ///
    /// 演示区两个复制按钮就是这两个调用（`inspector_tree_text` /
    /// `capture_inspector_snapshot`）；崩溃记录器指向临时目录，
    /// 顺带验证滚动快照落盘不断线。
    #[rgpui::test]
    fn ai_export_chain_works_while_inspector_open(cx: &mut TestAppContext) {
        let dir = std::env::temp_dir().join(format!(
            "inspector-example-crash-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        cx.update(|cx| {
            cx.enable_crash_recorder(dir.clone());
        });
        let (_view, cx) = cx.add_window_view(InspectorDemo::new);
        cx.update(|window, cx| {
            window.toggle_inspector(cx);
            let _ = window.draw(cx);
            // 树文本：AI 可读，含演示根节点。
            let text = window
                .inspector_tree_text(cx, 2000)
                .expect("检查器打开后应有树文本");
            assert!(
                text.contains("inspector-demo"),
                "树文本应含演示根节点：{text}"
            );
            // 结构化快照：JSON 可序列化，全树非空。
            let snapshot = window
                .capture_inspector_snapshot(cx)
                .expect("检查器打开后应有快照");
            assert!(snapshot.tree_total > 0, "快照全树不应为空");
            let json = serde_json::to_string(&snapshot).unwrap();
            assert!(json.contains("tree_total"), "快照缺字段：{json}");
        });
        _ = std::fs::remove_dir_all(&dir);
    }

    /// 插槽回归测试：自定义顶栏/段落外皮走默认面板渲染不断线。
    #[rgpui::test]
    fn panel_slots_render_without_panic(cx: &mut TestAppContext) {
        use rgpui::IntoElement as _;
        use rgpui::ParentElement as _;
        use rgpui::{InspectorPanelSlots, default_inspector_header, default_inspector_section};
        use std::sync::Arc;
        cx.update(|cx| {
            cx.enable_default_inspector();
            // 包裹扩展 recipe：默认顶栏前加横幅，段落沿用默认外皮。
            cx.set_inspector_panel_slots(InspectorPanelSlots {
                header: Some(Arc::new(|inspector, window, cx| {
                    rgpui::v_flex()
                        .child(rgpui::div().child("定制横幅"))
                        .child(default_inspector_header(inspector, window, cx))
                        .into_any_element()
                })),
                section: Some(Arc::new(|title, body| {
                    default_inspector_section(title, body)
                })),
            });
            cx.on_global_action(ToggleInspector, Some("f12"), |window, cx| {
                window.toggle_inspector(cx);
            });
        });
        let (_view, cx) = cx.add_window_view(InspectorDemo::new);
        cx.update(|window, cx| {
            window.activate_window();
            let _ = window.draw(cx);
        });
        // 打开检查器即走插槽渲染两帧（顶栏 + 选中卡 + 完整树），不断线即过。
        cx.simulate_keystrokes("f12");
        cx.update(|window, cx| {
            let _ = window.draw(cx);
            assert!(window.is_inspector_picking(cx));
            let _ = window.draw(cx);
        });
    }
}
