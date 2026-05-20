use rgpui::*;
use rgpui_component::{
    ActiveTheme as _, Root, h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use rgpui_webview::WebView;

/// 示例应用结构体
pub struct Example {
    /// 焦点处理器
    focus_handle: FocusHandle,
    /// 网页视图实体
    webview: Entity<WebView>,
    /// 地址栏输入框状态
    address_input: Entity<InputState>,
}

impl Example {
    /// 创建新的示例应用实例
    ///
    /// # 参数
    ///
    /// * `window` - GPUI 窗口
    /// * `cx` - 应用上下文
    pub fn new(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let webview = cx.new(|cx| {
            let builder = wry::WebViewBuilder::new();
            #[cfg(any(debug_assertions, feature = "inspector"))]
            let builder = builder.with_devtools(true);

            #[cfg(not(any(
                target_os = "windows",
                target_os = "macos",
                target_os = "ios",
                target_os = "android"
            )))]
            let webview = {
                use gtk::prelude::*;
                use wry::WebViewBuilderExtUnix;
                // 参考自 https://github.com/tauri-apps/wry/blob/dev/examples/gtk_multiwebview.rs
                // 尚未完全正常工作
                // TODO: 如何正确初始化？
                let fixed = gtk::Fixed::builder().build();
                fixed.show_all();
                builder.build_gtk(&fixed).unwrap()
            };
            #[cfg(any(
                target_os = "windows",
                target_os = "macos",
                target_os = "ios",
                target_os = "android"
            ))]
            let webview = {
                use raw_window_handle::HasWindowHandle;

                let window_handle = window.window_handle().expect("无法获取窗口句柄");
                builder.build_as_child(&window_handle).unwrap()
            };

            WebView::new(webview, window, cx)
        });

        let address_input = cx.new(|cx| {
            InputState::new(window, cx).default_value("https://github.com/launcher-rs/rgpui")
        });

        let url = address_input.read(cx).value().clone();
        webview.update(cx, |view, _| {
            view.load_url(&url);
        });

        cx.new(|cx| {
            let this = Self {
                focus_handle: cx.focus_handle(),
                webview,
                address_input: address_input.clone(),
            };

            // 监听输入框的回车事件以加载新 URL
            cx.subscribe(
                &address_input,
                |this: &mut Self, input, event: &InputEvent, cx| match event {
                    InputEvent::PressEnter { .. } => {
                        let url = input.read(cx).value().clone();
                        this.webview.update(cx, |view, _| {
                            view.load_url(&url);
                        });
                    }
                    _ => {}
                },
            )
            .detach();

            this
        })
    }

    /// 隐藏网页视图
    pub fn hide(&self, _: &mut Window, cx: &mut App) {
        self.webview.update(cx, |webview, _| webview.hide())
    }

    /// 后退到历史记录上一页
    #[allow(unused)]
    fn go_back(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.webview.update(cx, |webview, _| {
            webview.back().unwrap();
        });
    }
}

impl Focusable for Example {
    fn focus_handle(&self, _cx: &rgpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Example {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let webview = self.webview.clone();

        v_flex()
            .p_2()
            .gap_3()
            .size_full()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(Input::new(&self.address_input)),
            )
            .child(
                div()
                    .flex_1()
                    .border_1()
                    .h(rgpui::px(400.))
                    .border_color(cx.theme().border)
                    .child(webview.clone()),
            )
    }
}

fn main() {
    // Windows 平台渲染 WebView 所必需的环境变量设置
    #[cfg(target_os = "windows")]
    unsafe {
        std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "true");
    }

    rgpui_platform::application().run(move |cx| {
        // 在使用任何 GPUI Component 功能前必须调用此初始化函数
        rgpui_component::init(cx);

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = Example::new(window, cx);
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("打开窗口失败");
        })
        .detach();
    });
}
