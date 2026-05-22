//! 桌面宠物演示 - 透明窗口 + 鼠标穿透 + 窗口移动宠物

use rgpui::*;
use rgpui_component::{v_flex, TitleBar};
use rgpui_component_assets::Assets;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ── 穿透状态管理器（跨线程共享） ─────────────────────────────────────────
#[derive(Clone)]
struct PassthroughState(Arc<Mutex<bool>>);

impl PassthroughState {
    fn new(initial: bool) -> Self {
        Self(Arc::new(Mutex::new(initial)))
    }

    fn toggle(&self) -> bool {
        let mut v = self.0.lock().unwrap();
        *v = !*v;
        *v
    }

    fn get(&self) -> bool {
        *self.0.lock().unwrap()
    }
}

// ── 宠物状态 ─────────────────────────────────────────────────────────────
/// 宠物动画状态（跨线程共享）
struct PetState {
    /// 窗口 X 位置（屏幕坐标）
    x: f32,
    /// 窗口 Y 位置（屏幕坐标）
    y: f32,
    /// 当前移动方向角度（弧度）
    angle: f32,
    /// 移动速度（像素/秒）
    speed: f32,
    /// 动画帧计数器
    frame: usize,
    /// 是否正在移动
    is_moving: bool,
    /// 屏幕宽度
    screen_width: f32,
    /// 屏幕高度
    screen_height: f32,
    /// 窗口大小
    window_size: f32,
    /// 允许跑出屏幕外的最大距离
    max_offscreen: f32,
    /// 方向改变计时器
    direction_timer: f32,
    /// 下次方向改变的间隔（秒）
    direction_interval: f32,
}

impl PetState {
    fn new(screen_width: f32, screen_height: f32, window_size: f32) -> Self {
        Self {
            x: (screen_width - window_size) / 2.0,
            y: (screen_height - window_size) / 2.0,
            angle: 0.0,
            speed: 80.0,
            frame: 0,
            is_moving: true,
            screen_width,
            screen_height,
            window_size,
            max_offscreen: 100.0,
            direction_timer: 0.0,
            direction_interval: 2.0 + (rand_f32() * 3.0),
        }
    }

    /// 更新宠物位置
    fn update(&mut self, dt: Duration) {
        if !self.is_moving {
            return;
        }

        let delta = self.speed * dt.as_secs_f32();
        let dt_secs = dt.as_secs_f32();

        // 更新方向计时器
        self.direction_timer += dt_secs;
        if self.direction_timer >= self.direction_interval {
            self.direction_timer = 0.0;
            self.direction_interval = 2.0 + (rand_f32() * 3.0);
            self.change_direction();
        }

        // 计算新位置
        let new_x = self.x + self.angle.cos() * delta;
        let new_y = self.y + self.angle.sin() * delta;

        // 检查是否超出允许范围
        let min_x = -self.max_offscreen;
        let max_x = self.screen_width - self.window_size + self.max_offscreen;
        let min_y = -self.max_offscreen;
        let max_y = self.screen_height - self.window_size + self.max_offscreen;

        // 如果超出范围，转向屏幕中心并将位置限制在有效范围内
        if new_x < min_x || new_x > max_x || new_y < min_y || new_y > max_y {
            self.turn_toward_center();
            self.x = new_x.clamp(min_x, max_x);
            self.y = new_y.clamp(min_y, max_y);
        } else {
            self.x = new_x;
            self.y = new_y;
        }

        self.frame = (self.frame + 1) % 4;
    }

    /// 随机改变方向
    fn change_direction(&mut self) {
        let current_angle = self.angle;
        let change = (rand_f32() - 0.5) * std::f32::consts::PI * 0.5;
        self.angle = current_angle + change;
    }

    /// 转向屏幕中心
    fn turn_toward_center(&mut self) {
        let center_x = self.screen_width / 2.0;
        let center_y = self.screen_height / 2.0;
        let dx = center_x - self.x;
        let dy = center_y - self.y;
        self.angle = dy.atan2(dx);
    }

    /// 获取当前宠物表情
    fn get_emoji(&self) -> &'static str {
        if !self.is_moving {
            return "😴";
        }
        match self.frame {
            0 => "🐱",
            1 => "🐾",
            2 => "🐱",
            3 => "✨",
            _ => "🐱",
        }
    }

    fn set_moving(&mut self, moving: bool) {
        self.is_moving = moving;
    }

    /// 同步窗口当前位置，恢复自动移动时从用户拖动后的位置继续
    fn set_position(&mut self, position: Point<Pixels>) {
        self.x = position.x.as_f32();
        self.y = position.y.as_f32();
    }

    /// 返回宠物当前是否处于自动移动状态
    fn is_moving(&self) -> bool {
        self.is_moving
    }

    fn position(&self) -> Point<Pixels> {
        point(px(self.x), px(self.y))
    }
}

/// 简单的随机数生成（避免引入额外依赖）
fn rand_f32() -> f32 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static STATE: AtomicU64 = AtomicU64::new(1);
    let mut s = STATE.fetch_add(1, Ordering::Relaxed);
    s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
    ((s >> 33) as f32) / (u32::MAX as f32)
}

const APP_ID: &str = "com.example.desktop_pet";

fn main() {
    // 初始化穿透状态为 true（默认开启穿透）
    let passthrough_state = PassthroughState::new(true);
    let pet_state = Arc::new(Mutex::new(PetState::new(1920.0, 1080.0, 220.0)));
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
    let pet_state_clone = pet_state.clone();

    // ── 启动 GPUI 应用 ──────────────────────────────────────────────────
    let app = rgpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        rgpui_component::init(cx);

        cx.set_keep_alive_without_windows(true);

        cx.activate(true);

        let bounds = Bounds::centered(None, size(px(300.), px(300.0)), cx);

        let window_handle = cx
            .open_window(
                WindowOptions {
                    titlebar: None,
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    // 背景透明
                    window_background: WindowBackgroundAppearance::Transparent,
                    // 初始穿透状态（true 时开启穿透，隐藏 titlebar 和任务栏图标）
                    mouse_passthrough: passthrough_state.get(),
                    // is_resizable: false,
                    // is_minimizable: false,
                    kind: WindowKind::Overlay,
                    ..Default::default()
                },
                |_, cx| cx.new(|_| DesktopPet::new(pet_state.clone())),
            )
            .expect("failed to open window");

        setup_global_hotkey(cx, state_clone, pet_state.clone(), window_handle);

        // 启动宠物动画循环
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let new_pos = {
                    let mut pet = pet_state_clone.lock().unwrap();
                    if !pet.is_moving() {
                        continue;
                    }
                    pet.update(Duration::from_millis(16));
                    pet.position()
                };

                // 移动窗口到新位置
                let _ = cx.update_window(window_handle.into(), |_, window, _cx| {
                    window.set_position(new_pos);
                });
            }
        })
        .detach();
    });
}

fn setup_global_hotkey(
    cx: &mut App,
    state: PassthroughState,
    pet_state: Arc<Mutex<PetState>>,
    window_handle: WindowHandle<DesktopPet>,
) {
    let keystroke = Keystroke::parse("ctrl-shift-p").expect("valid keystroke");
    if let Err(err) = cx.register_global_hotkey(1, &keystroke) {
        eprintln!("Failed to register global hotkey: {}", err);
    }

    cx.on_global_hotkey(move |id, cx| {
        if id == 1 {
            eprintln!("Global hotkey triggered (Ctrl+Shift+P)");
            let enabled = state.toggle();

            {
                let mut pet = pet_state.lock().unwrap();
                pet.set_moving(enabled);
            }

            if let Err(err) = cx.update_window(window_handle.into(), |view, window, cx| {
                if enabled {
                    // 恢复运行前先记录用户通过标题栏拖动后的新位置
                    let current_position = window.bounds().origin;
                    pet_state.lock().unwrap().set_position(current_position);
                    window.set_mouse_passthrough(true);
                } else {
                    // 暂停时关闭穿透并显示系统标题栏，交给 Windows 原生 HTCAPTION 拖动
                    window.set_mouse_passthrough(false);
                    window.activate_window();
                }
                if let Ok(view) = view.downcast::<DesktopPet>() {
                    view.update(cx, |v, cx| {
                        v.passthrough = enabled;
                        cx.notify();
                    });
                }
            }) {
                eprintln!("Failed to update window: {}", err);
            }
        }
    });
}

// ── 视图组件：桌面宠物 ───────────────────────────────────────────────────
struct DesktopPet {
    passthrough: bool,
    pet_state: Arc<Mutex<PetState>>,
}

impl DesktopPet {
    fn new(pet_state: Arc<Mutex<PetState>>) -> Self {
        Self {
            passthrough: true,
            pet_state,
        }
    }
}

impl Render for DesktopPet {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let pet = self.pet_state.lock().unwrap();
        let pet_emoji = pet.get_emoji().to_string();
        drop(pet);

        v_flex().size_full().overflow_hidden()
            .child(if self.passthrough {
                div()
            } else {
                div().child(TitleBar::new())
            })
            .child(
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(div().text_size(px(120.0)).child(pet_emoji)),
        )
    }
}
