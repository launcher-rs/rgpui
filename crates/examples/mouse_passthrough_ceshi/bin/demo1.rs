use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use rgpui::{
    AppContext, Bounds, Context, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, Styled, TitlebarOptions, Window, WindowBounds, WindowOptions, div, px,
    rgb, size,
};
use rgpui_platform::application;
use std::sync::{Arc, Mutex};

// ── 自定义 Action：用于在 GPUI 内部传递穿透状态变更 ──────────────────────────
// 派生宏自动实现 Clone、PartialEq、JsonSchema 以及 gpui::Action 所需的所有 trait 方法
#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, rgpui::Action)]
struct SetPassthrough {
    /// 是否启用鼠标穿透
    enabled: bool,
}

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

fn main() {
    // 初始化穿透状态为 true（默认开启穿透）
    let passthrough_state = PassthroughState::new(true);

    // ── 注册全局快捷键 Ctrl+Shift+P ──────────────────────────────────────
    // GlobalHotKeyManager 必须绑定到变量，否则立即析构导致快捷键失效
    let _hotkey_manager = GlobalHotKeyManager::new().expect("failed to init hotkey manager");
    let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyP);
    _hotkey_manager
        .register(hotkey)
        .expect("failed to register hotkey");

    // ── 启动 GPUI 应用 ──────────────────────────────────────────────────
    application().run(move |cx| {
        // 创建居中窗口，尺寸 300x300
        let bounds = Bounds::centered(None, size(px(300.), px(300.0)), cx);

        // 打开新窗口
        let window_handle = cx
            .open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("Demo (Ctrl+Shift+P 切换穿透)".into()),
                        ..Default::default()
                    }),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    // 初始穿透状态
                    mouse_passthrough: passthrough_state.get(),
                    ..Default::default()
                },
                // 窗口的根视图
                |_, cx| cx.new(|_| ButtonStyledDemo::new()),
            )
            .expect("failed to open window");

        let state_clone = passthrough_state.clone();
        cx.spawn(async move |cx| {
            let receiver = GlobalHotKeyEvent::receiver();
            loop {
                if let Ok(event) = receiver.try_recv() {
                    if event.state == HotKeyState::Released {
                        // 触发
                        let enabled = state_clone.toggle();
                        let _ = cx.update_window(window_handle.into(), |_, window, cx| {
                            window.set_mouse_passthrough(enabled);
                            cx.dispatch_action(&SetPassthrough { enabled });
                        });
                    }
                }
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
            }
        })
        .detach();
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
        div()
            .size_full()
            .overflow_hidden()
            // 注册 Action 监听器：当收到 SetPassthrough 时更新内部状态
            // 使用 .on_action() 而非 cx.on_action()，因为后者在 render 中调用会导致 node_stack 为空而崩溃
            .on_action(cx.listener(|view, action: &SetPassthrough, _, cx| {
                view.passthrough = action.enabled;
                cx.notify(); // 触发重新渲染
            }))
            // 状态提示文字
            .child(div().child(status))
            // 点击次数显示
            .child(div().child(format!("点击次数：{}", self.click_count)))
            // 可交互按钮
            .child(
                // 增加按钮
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(56.0))
                    .h(px(56.0))
                    .rounded(px(28.0))
                    .bg(rgb(0x334155))
                    .hover(|s| s.bg(rgb(0x4ade80)).cursor_pointer())
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xe2e8f0))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.click_count += 1;
                            println!("点击 {}", this.click_count);
                            cx.notify(); // 更新点击次数显示
                        }),
                    )
                    .child("+"),
            )
    }
}
