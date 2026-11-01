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

/// 模拟流式输出的整篇 Markdown。
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

/// 流式渲染演示视图。
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

/// 按字符边界把源码切成固定字节数的小块，模拟逐 token 输出。
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

/// 启动示例。
fn run_example() {
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
