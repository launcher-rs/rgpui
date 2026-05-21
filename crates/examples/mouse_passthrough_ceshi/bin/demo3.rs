//! 显示titlebar有问题，可以考虑是的自定义titlebar

use rgpui::*;
use rgpui_component::button::{Button, ButtonVariants};
use rgpui_component::{TitleBar, v_flex};
use rgpui_component_assets::Assets;
use std::sync::{Arc, Mutex};

// ── 穿透状态管理器（跨线程共享） ─────────────────────────────────────────
// 使用 Arc<Mutex<bool>> 保证在后台线程和主线程之间安全共享状态
#[derive(Clone)]
struct PassthroughState(Arc<Mutex<bool>>);

impl PassthroughState {
    /// 创建新的状态管理器，初始值由 initial 指定
    fn new(initial: bool) -> Self {
        Self(Arc::new(Mutex::new(initial)))
    }

    /// 切换穿透状态并返回新值
    fn toggle(&self) -> bool {
        let mut v = self.0.lock().unwrap();
        *v = !*v;
        *v
    }

    /// 获取当前穿透状态
    fn get(&self) -> bool {
        *self.0.lock().unwrap()
    }
}

const APP_ID: &str = "com.example.demo2";

fn main() {
    // 初始化穿透状态为 true（默认开启穿透）
    let passthrough_state = PassthroughState::new(true);

    // 单例模式
    let _instance = match SingleInstance::acquire(APP_ID) {
        Ok(instance) => instance,
        Err(_) => {
            eprintln!("Another instance is already running. Sending activation signal.");
            let _ = send_activate_to_existing(APP_ID);
            std::process::exit(0);
        }
    };

    let state_clone = passthrough_state.clone();

    // ── 启动 GPUI 应用 ──────────────────────────────────────────────────
    let app = rgpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        rgpui_component::init(cx);

        cx.set_keep_alive_without_windows(true);

        cx.activate(true);

        // 创建居中窗口，尺寸 300x300
        let bounds = Bounds::centered(None, size(px(300.), px(300.0)), cx);

        // 打开新窗口
        let window_handle = cx
            .open_window(
                WindowOptions {
                    titlebar: None,
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    // 背景透明
                    window_background: WindowBackgroundAppearance::Transparent,
                    // 初始穿透状态（true 时开启穿透，隐藏 titlebar 和任务栏图标）
                    mouse_passthrough: passthrough_state.get(),
                    ..Default::default()
                },
                // 窗口的根视图
                |_, cx| cx.new(|_| ButtonStyledDemo::new()),
            )
            .expect("failed to open window");

        // 延迟设置 titlebar 可见性，等待窗口完全初始化
        let initial_visible = !passthrough_state.get();
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            let _ = cx.update_window(window_handle.into(), |_, window, _| {
                window.set_titlebar_visible(initial_visible);
            });
        })
        .detach();

        // 设置全局快捷键
        setup_global_hotkey(cx, state_clone, window_handle);
    });
}

fn setup_global_hotkey(
    cx: &mut App,
    state: PassthroughState,
    window_handle: WindowHandle<ButtonStyledDemo>,
) {
    let keystroke = Keystroke::parse("ctrl-shift-p").expect("valid keystroke");
    if let Err(err) = cx.register_global_hotkey(1, &keystroke) {
        eprintln!("Failed to register global hotkey: {}", err);
    }

    cx.on_global_hotkey(move |id, cx| {
        if id == 1 {
            eprintln!("Global hotkey triggered (Ctrl+Shift+P)");
            let enabled = state.toggle();
            let _ = cx.update_window(window_handle.into(), |view, window, cx| {
                window.set_mouse_passthrough(enabled);
                if let Ok(view) = view.downcast::<ButtonStyledDemo>() {
                    view.update(cx, |v, cx| {
                        v.passthrough = enabled;
                        cx.notify();
                    });
                }
            });
        }
    });
}

// ── 视图组件：展示穿透状态和交互按钮 ───────────────────────────────────────
struct ButtonStyledDemo {
    /// 按钮点击次数
    click_count: usize,
    /// 当前是否处于穿透模式
    passthrough: bool,
}

impl ButtonStyledDemo {
    fn new() -> Self {
        Self {
            click_count: 0,
            passthrough: true, // 默认开启穿透
        }
    }
}

impl Render for ButtonStyledDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 根据穿透状态显示不同的提示文字
        let status = if self.passthrough {
            "🔵 穿透开启 — Ctrl+Shift+P 关闭"
        } else {
            "🟢 穿透关闭 — Ctrl+Shift+P 开启"
        };

        // 构建 UI 布局
        v_flex()
            .size_full()
            .overflow_hidden()
            .child(if self.passthrough {
                println!("不显示...");

                div()
            } else {
                println!("显示.....");
                div().child(TitleBar::new())
            })
            // 状态提示文字
            .child(div().child(status))
            // 点击次数显示
            .child(div().child(format!("点击次数：{}", self.click_count)))
            // 可交互按钮
            .child(
                // 增加按钮
                Button::new("ok")
                    .primary()
                    .label("增加")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.click_count += 1;
                        println!("点击 {}", this.click_count);
                        cx.notify(); // 更新点击次数显示
                    })),
            )
    }
}
