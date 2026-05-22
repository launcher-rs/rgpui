//! 桌面宠物演示
//!
//! 展示 gpui 窗口的核心交互能力：
//! - 透明无框窗口（`WindowKind::Overlay` + 透明背景）
//! - 鼠标穿透/非穿透运行时切换（`set_mouse_passthrough`）
//! - Ctrl+拖动窗口交互（`GetAsyncKeyState` 检测物理键状态）
//! - 系统托盘图标与右键菜单（最小化到托盘）
//! - 全局快捷键（Ctrl+Shift+P 暂停/恢复）
//! - 单例运行（`SingleInstance`）
//!
//! # 交互方式
//! - 默认自动移动，鼠标穿透开启，窗口不阻挡点击
//! - 按住 Ctrl 暂停自动移动并关闭穿透，可点击 UI 或拖动窗口
//! - Ctrl+Shift+P 全局热键暂停/恢复
//! - 暂停后窗口关闭穿透，测试按钮可点击
//! - 关闭窗口 → 最小化到系统托盘
//! - 托盘左键/菜单「显示窗口」恢复显示

use rgpui::*;
use rgpui_component::button::Button;
use rgpui_component::v_flex;
use rgpui_component_assets::Assets;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 跨线程共享的鼠标穿透状态
///
/// 用 `Arc<Mutex<bool>>` 包装，允许多个异步任务和视图组件共享同一状态。
#[derive(Clone)]
struct PassthroughState(Arc<Mutex<bool>>);

impl PassthroughState {
    /// 创建新的穿透状态管理器
    fn new(initial: bool) -> Self {
        Self(Arc::new(Mutex::new(initial)))
    }

    /// 切换穿透状态并返回新的值
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

/// 宠物动画状态
///
/// 维护宠物的位置、运动方向、动画帧等信息。
/// 通过 `Arc<Mutex<PetState>>` 在视图组件和异步动画循环间共享。
struct PetState {
    /// 窗口 X 位置（屏幕坐标像素值）
    x: f32,
    /// 窗口 Y 位置（屏幕坐标像素值）
    y: f32,
    /// 当前移动方向（弧度，0=向右，π/2=向下）
    angle: f32,
    /// 移动速度（像素/秒）
    speed: f32,
    /// 动画帧索引（0..4，循环）
    frame: usize,
    /// 是否处于自动移动状态
    is_moving: bool,
    /// 屏幕宽度（像素），用于边界检测
    screen_width: f32,
    /// 屏幕高度（像素），用于边界检测
    screen_height: f32,
    /// 窗口尺寸（像素，正方形）
    window_size: f32,
    /// 允许宠物跑出屏幕边缘的最大像素距离
    max_offscreen: f32,
    /// 距离下次转向的累计时间（秒）
    direction_timer: f32,
    /// 两次转向之间的间隔（秒，随机值）
    direction_interval: f32,
}

impl PetState {
    /// 创建宠物状态实例
    ///
    /// # 参数
    /// * `screen_width` - 屏幕宽度（像素）
    /// * `screen_height` - 屏幕高度（像素）
    /// * `window_size` - 窗口尺寸（像素，正方形）
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

    /// 根据时间增量更新宠物位置
    ///
    /// 如果不在移动状态（暂停），直接返回。
    /// 计算方向计时器，超时则随机改变方向。
    /// 根据角度和速度计算新位置，超出屏幕范围则折返。
    ///
    /// # 参数
    /// * `dt` - 距离上次更新的时间间隔
    fn update(&mut self, dt: Duration) {
        if !self.is_moving {
            return;
        }

        let delta = self.speed * dt.as_secs_f32();
        let dt_secs = dt.as_secs_f32();

        // 更新方向计时器，计时器归零时随机改变方向
        self.direction_timer += dt_secs;
        if self.direction_timer >= self.direction_interval {
            self.direction_timer = 0.0;
            self.direction_interval = 2.0 + (rand_f32() * 3.0);
            self.change_direction();
        }

        // 根据当前方向角度计算位移
        let new_x = self.x + self.angle.cos() * delta;
        let new_y = self.y + self.angle.sin() * delta;

        // 边界检测：允许 max_offscreen 像素跑出屏幕
        let min_x = -self.max_offscreen;
        let max_x = self.screen_width - self.window_size + self.max_offscreen;
        let min_y = -self.max_offscreen;
        let max_y = self.screen_height - self.window_size + self.max_offscreen;

        // 超出范围则转向屏幕中心，位置限制在有效范围内
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

    /// 随机改变移动方向
    ///
    /// 在当前角度基础上随机旋转 ±45 度。
    fn change_direction(&mut self) {
        let current_angle = self.angle;
        let change = (rand_f32() - 0.5) * std::f32::consts::PI * 0.5;
        self.angle = current_angle + change;
    }

    /// 转向屏幕中心
    ///
    /// 计算从当前位置指向屏幕中心的方向角。
    fn turn_toward_center(&mut self) {
        let center_x = self.screen_width / 2.0;
        let center_y = self.screen_height / 2.0;
        let dx = center_x - self.x;
        let dy = center_y - self.y;
        self.angle = dy.atan2(dx);
    }

    /// 根据当前状态返回对应的表情符号
    ///
    /// 暂停时返回 😴，移动时根据帧索引轮换显示 🐱/🐾/✨。
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

    /// 设置移动/暂停状态
    fn set_moving(&mut self, moving: bool) {
        self.is_moving = moving;
    }

    /// 从窗口当前位置同步宠物坐标
    ///
    /// 用于 Ctrl+拖动后记录拖动后的位置，恢复自动移动时从此位置继续。
    ///
    /// # 参数
    /// * `position` - 窗口当前屏幕坐标（Pixels）
    fn set_position(&mut self, position: Point<Pixels>) {
        self.x = position.x.as_f32();
        self.y = position.y.as_f32();
    }

    /// 返回宠物是否处于自动移动状态
    fn is_moving(&self) -> bool {
        self.is_moving
    }

    /// 返回当前宠物位置（Pixels 单位）
    fn position(&self) -> Point<Pixels> {
        point(px(self.x), px(self.y))
    }
}

/// 简单的线性同余随机数生成器
///
/// 使用原子计数器作为种子，避免引入外部随机数库依赖。
/// 算法：LCG（Linear Congruential Generator）
fn rand_f32() -> f32 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static STATE: AtomicU64 = AtomicU64::new(1);
    let mut s = STATE.fetch_add(1, Ordering::Relaxed);
    s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
    ((s >> 33) as f32) / (u32::MAX as f32)
}

// 通过 Win32 `GetAsyncKeyState` 检测 Ctrl 键是否按下
// 使用 `GetAsyncKeyState` 而非 `GetKeyState`，因为：
// - `GetKeyState` 依赖消息队列，穿透窗口无焦点时返回 0
// - `GetAsyncKeyState` 直接查询物理键状态，无论焦点在哪都能正确检测
#[link(name = "user32")]
unsafe extern "system" {
    fn GetAsyncKeyState(vKey: i32) -> i16;
}
const VK_CONTROL: i32 = 0x11;
fn is_ctrl_pressed() -> bool {
    unsafe { GetAsyncKeyState(VK_CONTROL) < 0 }
}

/// 应用标识符（用于单例检测）
const APP_ID: &str = "com.example.desktop_pet";

fn main() {
    // 初始化状态：默认开启穿透，宠物从屏幕中心开始自动移动
    let passthrough_state = PassthroughState::new(true);
    let pet_state = Arc::new(Mutex::new(PetState::new(1920.0, 1080.0, 220.0)));

    // 单例模式：防止启动多个实例
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

        // 无窗口时保持运行（后台热键监听）
        cx.set_keep_alive_without_windows(true);

        cx.activate(true);

        // 创建 300×300 的透明无框覆盖窗口
        let bounds = Bounds::centered(None, size(px(300.), px(300.0)), cx);

        let window_visible = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let window_visible_close = window_visible.clone();

        let window_handle = cx
            .open_window(
                WindowOptions {
                    titlebar: None,
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_background: WindowBackgroundAppearance::Transparent,
                    mouse_passthrough: passthrough_state.get(),
                    kind: WindowKind::Overlay,
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|_| DesktopPet::new(pet_state.clone()));

                    // 拦截关闭事件，隐藏到托盘而不是退出
                    window.on_window_should_close(cx, move |window, _cx| {
                        window_visible_close.store(false, std::sync::atomic::Ordering::Release);
                        window.hide_window();
                        // 返回 false 阻止窗口关闭
                        false
                    });

                    view
                },
            )
            .expect("failed to open window");

        // ── 托盘（系统通知区）设置 ──────────────────────────────────────
        cx.set_tray_tooltip("桌面宠物");

        // 设置托盘图标（复用 gpui 示例中的图标）
        let icon_bytes = include_bytes!("../../../rgpui/examples/image/app-icon.png");
        cx.set_tray_icon(Some(icon_bytes.as_slice()));

        // 左键点击托盘：若窗口已隐藏则恢复显示
        let wh_click = window_handle.clone();
        let wv_click = window_visible.clone();
        cx.on_tray_icon_event(move |event, cx| {
            if matches!(event, TrayIconEvent::LeftClick | TrayIconEvent::DoubleClick) {
                if !wv_click.load(std::sync::atomic::Ordering::Acquire) {
                    let wv = wv_click.clone();
                    cx.update_window(wh_click.into(), |_, window, _| {
                        wv.store(true, std::sync::atomic::Ordering::Release);
                        window.activate_window();
                    })
                    .ok();
                }
            }
        });

        // 设置托盘右键菜单
        cx.set_tray_menu(vec![
            TrayMenuItem::Action {
                label: "显示窗口".into(),
                id: "show_window".into(),
            },
            TrayMenuItem::Separator,
            TrayMenuItem::Action {
                label: "退出".into(),
                id: "quit".into(),
            },
        ]);

        // 注册托盘菜单动作回调
        let wh_menu = window_handle.clone();
        let wv_menu = window_visible;
        cx.on_tray_menu_action(move |id, cx| match id.as_ref() {
            "quit" => cx.quit(),
            "show_window" => {
                let wv = wv_menu.clone();
                if let Err(err) = cx.update_window(wh_menu.into(), |_, window, _| {
                    wv.store(true, std::sync::atomic::Ordering::Release);
                    window.activate_window();
                }) {
                    eprintln!("Failed to activate window: {}", err);
                }
            }
            _ => {}
        });

        setup_global_hotkey(cx, state_clone, pet_state.clone(), window_handle);

        // ── 宠物动画循环 ────────────────────────────────────────────────
        // 每 ~60fps（16ms）运行一次：
        // - 检测 Ctrl 键状态，控制鼠标穿透和窗口拖动
        // - 自动移动模式下更新宠物位置并移动窗口
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;

                let ctrl_held = is_ctrl_pressed();

                let _ = cx.update_window(window_handle.into(), |_, window, _cx| {
                    let mut pet = pet_state_clone.lock().unwrap();

                    if ctrl_held {
                        // 按住 Ctrl：关闭穿透，允许点击 UI 和拖动窗口
                        window.set_mouse_passthrough(false);
                        // 记录拖动后的窗口位置，恢复移动时从此继续
                        let current_pos = window.bounds().origin;
                        pet.set_position(current_pos);
                    } else if pet.is_moving() {
                        // 自动移动中：开启穿透（不阻挡点击），更新位置
                        window.set_mouse_passthrough(true);
                        pet.update(Duration::from_millis(16));
                        let new_pos = pet.position();
                        drop(pet);
                        window.set_position(new_pos);
                    }
                    // 暂停时（!is_moving && !ctrl_held）：不干预穿透状态，
                    // 保持由全局热键暂停时设置的 `set_mouse_passthrough(false)`
                });
            }
        })
        .detach();
    });
}

/// 注册全局快捷键 Ctrl+Shift+P 暂停/恢复宠物移动
///
/// # 参数
/// * `cx` - GPUI 应用上下文
/// * `state` - 穿透状态管理器
/// * `pet_state` - 宠物状态
/// * `window_handle` - 窗口句柄，用于更新窗口属性
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
                    // 恢复运行：关闭穿透前记录当前位置，确保自动移动从准确位置开始
                    let current_position = window.bounds().origin;
                    pet_state.lock().unwrap().set_position(current_position);
                    window.set_mouse_passthrough(true);
                } else {
                    // 暂停：关闭穿透让窗口可交互，激活窗口使焦点回到桌面宠物
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

/// 视图组件：桌面宠物
///
/// 用于展示宠物的表情符号和交互按钮。
/// `passthrough` 字段控制 UI 是否渲染穿透模式相关内容。
struct DesktopPet {
    /// 当前穿透状态，影响 UI 渲染
    passthrough: bool,
    /// 宠物状态（跨线程共享）
    pet_state: Arc<Mutex<PetState>>,
}

impl DesktopPet {
    /// 创建 DesktopPet 视图组件
    ///
    /// # 参数
    /// * `pet_state` - 宠物状态引用
    fn new(pet_state: Arc<Mutex<PetState>>) -> Self {
        Self {
            passthrough: true,
            pet_state,
        }
    }
}

impl Render for DesktopPet {
    /// 渲染宠物窗口内容
    ///
    /// 布局：垂直排列
    /// - 上方弹性区域：宠物表情符号（居中显示）
    /// - 底部：测试按钮（仅穿透关闭时可点击）
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let pet = self.pet_state.lock().unwrap();
        let pet_emoji = pet.get_emoji().to_string();
        drop(pet);

        v_flex()
            .size_full()
            .overflow_hidden()
            .child(
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .child(div().text_size(px(120.0)).child(pet_emoji)),
            )
            .child(
                Button::new("ceshi")
                    .label("测试按钮")
                    .on_click(|_, _, _| println!("Clicked!")),
            )
    }
}
