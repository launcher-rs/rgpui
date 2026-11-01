//! Markdown 文档级选区示例。
//!
//! 演示 `MarkdownSelectionState` + `Markdown::selection`：
//! - 在正文内按住左键拖选，跨段落 / 列表 / 表格单元格连续选中；
//! - `Ctrl+C` 复制选中文本（列表项保留 `• ` / `1. ` 标记）；
//! - `Ctrl+A` 全选、`Esc` 清空选区。
//!
//! 运行：
//!
//! ```text
//! cargo run -p showcase --bin markdown_selection
//! ```

use rgpui::prelude::*;
use rgpui::{
    App, Bounds, Button, Context, Entity, FocusHandle, IntoElement, KeyDownEvent, MouseButton,
    Render, Subscription, Window, WindowBounds, WindowOptions, div, h_flex, px, rgb, size, v_flex,
};
use rgpui_markdown::{Markdown, MarkdownSelectionState};
use rgpui_platform::application;

/// 演示文档。
const DOC: &str = r##"# 文档级选区

在任意段落里按住左键拖动即可跨段落选择；**加粗**、*斜体*、`行内代码` 与
[链接](https://example.com) 都在同一段文本叶子里。

## 列表

- 第一项：列表标记会随文本一起复制
- 第二项：拖动时经过的整段会被选中
- 第三项：嵌套列表同样可选

1. 有序列表第一项
2. 有序列表第二项

> 引用块里的段落也是普通文本叶子，
> 跨行拖选会按阅读顺序拼接。

## 代码块

```rust
fn main() {
    println!("代码块按整块选中");
}
```

| 区域 | 选区行为 |
| --- | --- |
| 段落 | 按字符命中 |
| 表格单元格 | 按单元格文本命中 |
| 代码块 | 整块选中 |

最后一段用于验证拖选能一直延伸到文末。
"##;

/// 选区演示视图。
struct SelectionDemo {
    /// 选区状态实体。
    selection: Entity<MarkdownSelectionState>,
    /// 键盘焦点（`Ctrl+C` / `Ctrl+A` / `Esc`）。
    focus: FocusHandle,
    /// 观察选区实体变化，驱动重绘。
    _selection_subscription: Subscription,
    /// 最近一次操作提示。
    notice: Option<String>,
}

impl SelectionDemo {
    /// 创建视图。
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let selection = cx.new(|_| MarkdownSelectionState::new());
        let subscription = cx.observe(&selection, |_, _, cx| cx.notify());
        Self {
            selection,
            focus: cx.focus_handle(),
            _selection_subscription: subscription,
            notice: None,
        }
    }

    /// 复制当前选区到系统剪贴板。
    fn copy(&mut self, cx: &mut Context<Self>) {
        let Some(text) = self.selection.read(cx).selected_text() else {
            self.notice = Some("没有选中文本".to_string());
            cx.notify();
            return;
        };
        let chars = text.chars().count();
        let copied = self
            .selection
            .update(cx, |state, cx| state.copy_to_clipboard(cx));
        self.notice = Some(if copied {
            format!("已复制 {chars} 个字符")
        } else {
            "复制失败".to_string()
        });
        cx.notify();
    }
}

impl Render for SelectionDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selection.read(cx).selected_text();
        let status = match (&self.notice, &selected) {
            (Some(notice), _) => notice.clone(),
            (None, Some(text)) => format!("已选中 {} 个字符", text.chars().count()),
            (None, None) => "拖选正文，Ctrl+C 复制，Ctrl+A 全选".to_string(),
        };

        v_flex()
            .track_focus(&self.focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _event, window, cx| {
                    window.focus(&this.focus, cx);
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                if !event.keystroke.modifiers.secondary() {
                    return;
                }
                match event.keystroke.key.to_ascii_lowercase().as_str() {
                    "c" => this.copy(cx),
                    "a" => {
                        this.selection.update(cx, |state, cx| state.select_all(cx));
                        this.notice = Some("已全选".to_string());
                        cx.notify();
                    }
                    _ => {}
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                if event.keystroke.key == "escape" {
                    this.selection.update(cx, |state, cx| state.clear(cx));
                    this.notice = None;
                    cx.notify();
                }
            }))
            .relative()
            .size_full()
            .gap(px(12.0))
            .p(px(24.0))
            .bg(rgb(0xffffff))
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_xl().child("Markdown 文档级选区"))
                    .child(
                        Button::new("markdown-selection-clear")
                            .label("清空选区")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.selection.update(cx, |state, cx| state.clear(cx));
                                this.notice = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(div().text_sm().text_color(rgb(0x555555)).child(status))
            .child(
                v_flex()
                    .id("markdown-selection-scroll")
                    .flex_1()
                    .w_full()
                    .min_h(px(0.0))
                    .overflow_scroll()
                    .child(
                        Markdown::new(DOC)
                            .selection(self.selection.clone())
                            .w_full(),
                    ),
            )
    }
}

/// 启动示例。
fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(760.0), px(680.0)), cx);
        let _ = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| SelectionDemo::new(window, cx)),
        );
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
