//! rgpui WebView 示例
//!
//! 演示如何在 rgpui 窗口中嵌入原生 WebView 组件。
//! 支持加载 URL、加载 HTML、JavaScript 执行等操作。
//!
//! 运行：`cargo run -p webview_example`

use rgpui::prelude::*;
use rgpui::{
    Button, ButtonVariants as _, Context, InteractiveElement, ParentElement, Render, Window,
    WindowOptions, div, h_flex, px, rgb, size, v_flex, webview::WebView,
};
use rgpui_platform::application;

struct WebViewApp {
    webview: Option<rgpui::Entity<WebView>>,
    current_url: String,
}

impl WebViewApp {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            webview: None,
            current_url: "https://www.rust-lang.org".to_string(),
        }
    }

    fn create_webview(&mut self, url: &str, window: &mut Window, cx: &mut Context<Self>) {
        let url_clone = url.to_string();
        let webview_entity = cx.new(move |cx| {
            let wry_webview = wry::WebViewBuilder::new()
                .with_url(&url_clone)
                .build_as_child(window)
                .unwrap();
            WebView::new(wry_webview, window, cx)
        });
        self.webview = Some(webview_entity);
        self.current_url = url.to_string();
        // 必须手动通知重绘：WebView 实体只有被渲染后，`prepaint`
        // 才会把原生窗口同步到布局尺寸；否则原生窗口保持 0×0 不可见。
        cx.notify();
    }
}

impl Render for WebViewApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current_url = self.current_url.clone();

        v_flex()
            .id("webview-app")
            .size_full()
            .child(
                // 工具栏
                h_flex()
                    .id("toolbar")
                    .w_full()
                    .h(px(48.0))
                    .items_center()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .bg(rgb(0xf0f0f0))
                    .border_b(px(1.0))
                    .border_color(rgb(0xe0e0e0))
                    .child(
                        Button::new("load-rust")
                            .label("Rust 官网")
                            .ghost()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create_webview("https://www.rust-lang.org", window, cx);
                            })),
                    )
                    .child(
                        Button::new("load-github")
                            .label("GitHub")
                            .ghost()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create_webview("https://github.com", window, cx);
                            })),
                    )
                    .child(
                        Button::new("load-docs")
                            .label("Rust 文档")
                            .ghost()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create_webview(
                                    "https://doc.rust-lang.org/book/",
                                    window,
                                    cx,
                                );
                            })),
                    )
                    .child(
                        Button::new("load-html")
                            .label("加载 HTML")
                            .ghost()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                let html = r#"
                                <html>
                                <body style="font-family: sans-serif; padding: 20px; background: #1a1a2e; color: #e0e0e0;">
                                    <h1 style="color: #0078d4;">rgpui WebView</h1>
                                    <p>这是一个通过 <code>load_html</code> 加载的 HTML 页面。</p>
                                    <p>当前时间: <span id="time"></span></p>
                                    <script>
                                        document.getElementById('time').textContent = new Date().toLocaleString();
                                    </script>
                                </body>
                                </html>
                                "#;
                                if let Some(webview) = &this.webview {
                                    webview.update(cx, |wv, _| wv.load_html(html));
                                }
                            })),
                    )
                    .child(
                        Button::new("eval-js")
                            .label("执行 JS")
                            .ghost()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                if let Some(webview) = &this.webview {
                                    let _ = webview.update(cx, |wv, _| {
                                        wv.eval_script("alert('Hello from rgpui!')")
                                    });
                                }
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x666))
                            .child(current_url),
                    ),
            )
            .child(
                // WebView 内容区
                v_flex()
                    .id("webview-container")
                    .flex_1()
                    .w_full()
                    .when(self.webview.is_some(), |el| {
                        el.child(
                            div()
                                .id("webview-wrapper")
                                .flex_1()
                                .w_full()
                                .child(
                                    self.webview
                                        .as_ref()
                                        .map(|wv| {
                                            div().id("webview-slot").child(wv.clone())
                                        })
                                        .unwrap_or_else(|| {
                                            div()
                                                .id("webview-placeholder")
                                                .size_full()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_color(rgb(0x999))
                                                .child("点击上方按钮加载网页")
                                        }),
                                ),
                        )
                    })
                    .when(self.webview.is_none(), |el| {
                        el.child(
                            div()
                                .id("webview-placeholder")
                                .flex_1()
                                .w_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(rgb(0x999))
                                .child("点击上方按钮加载网页"),
                        )
                    }),
            )
    }
}

fn main() {
    application().run(|cx| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(rgpui::WindowBounds::Windowed(rgpui::Bounds::new(
                    rgpui::Point::default(),
                    size(px(900.0), px(600.0)),
                ))),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| WebViewApp::new(window, cx)),
        )
        .unwrap();
    });
}
