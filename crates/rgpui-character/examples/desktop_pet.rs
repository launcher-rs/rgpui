//! rgpui-character 桌面宠物完整示例。
//!
//! 这个示例展示一条完整链路：
//! - `AssetManager` 注册 sprite 动画资源
//! - `Behavior` 决定角色当前动作
//! - `CharacterRuntime` 推进行为、物理和动画
//! - `RenderCommand` 输出当前要绘制的纹理和变换
//! - rgpui 使用 `img()` 将纹理路径绘制到透明覆盖窗口中
//!
//! 运行：
//!
//! ```text
//! cargo run -p rgpui-character --example desktop_pet
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::single_instance::{SingleInstance, send_activate_to_existing};
use rgpui::{
    App, AssetSource, Bounds, Context, Keystroke, Pixels, Point, Render, SharedString,
    TrayIconEvent, TrayMenuItem, Window, WindowBackgroundAppearance, WindowBounds, WindowKind,
    WindowOptions, div, img, point, prelude::*, px, rgb, rgba, size,
};
use rgpui_character::{
    AnimationClip, Behavior, BehaviorAction, BehaviorContext, Character, CharacterRuntime,
    PhysicsConfig, Rect, RenderCommand, TextureId, Vec2,
};
use rgpui_platform::application;
use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 应用标识符，用于单例检测。
const APP_ID: &str = "com.example.rgpui-character-desktop-pet";

/// 桌宠覆盖窗口尺寸。
const WINDOW_SIZE: f32 = 320.0;

/// sprite 在窗口中的显示尺寸。
const SPRITE_SIZE: f32 = 220.0;

/// 自动更新帧间隔，约等于 60 FPS。
const FRAME_TIME: Duration = Duration::from_millis(16);

/// 暂停或恢复桌宠的全局快捷键编号。
const HOTKEY_TOGGLE_PAUSE: u32 = 1;

/// 示例资源加载器。
struct ExampleAssets {
    /// 资源根目录。
    base: PathBuf,
}

impl AssetSource for ExampleAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        fs::read(self.base.join(path))
            .map(|data| Some(Cow::Owned(data)))
            .map_err(Into::into)
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        fs::read_dir(self.base.join(path))
            .map(|entries| {
                entries
                    .filter_map(|entry| {
                        entry
                            .ok()
                            .and_then(|entry| entry.file_name().into_string().ok())
                            .map(SharedString::from)
                    })
                    .collect()
            })
            .map_err(Into::into)
    }
}

/// 小猫动作类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CatActivity {
    Walk,
    Sleep,
    Eat,
    Spin,
    Yawn,
    Turn,
}

impl CatActivity {
    fn animation_name(self) -> &'static str {
        match self {
            Self::Walk => "walk",
            Self::Sleep => "sleep",
            Self::Eat => "eat",
            Self::Spin => "spin",
            Self::Yawn => "yawn",
            Self::Turn => "turn",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Walk => "散步",
            Self::Sleep => "睡觉",
            Self::Eat => "吃东西",
            Self::Spin => "转圈圈",
            Self::Yawn => "打哈欠",
            Self::Turn => "转头",
        }
    }

    fn moves(self) -> bool {
        matches!(self, Self::Walk)
    }
}

/// 跨线程共享的桌宠运行状态。
struct PetSharedState {
    runtime: CharacterRuntime,
    running: bool,
    passthrough: bool,
    interactive: bool,
    activity: CatActivity,
    requested_activity: Option<CatActivity>,
    last_render: Option<RenderCommand>,
    message: SharedString,
}

impl PetSharedState {
    fn new(screen_width: f32, screen_height: f32) -> Self {
        let mut runtime = CharacterRuntime::new();
        runtime.physics = PhysicsConfig {
            friction: 0.0,
            bounds: Rect::new(
                -120.0,
                -120.0,
                screen_width - WINDOW_SIZE + 240.0,
                screen_height - WINDOW_SIZE + 240.0,
            ),
            bounce: false,
        };

        register_cat_assets(&mut runtime);

        let mut pet = Character::new("desktop-cat");
        pet.position = Vec2::new(
            (screen_width - WINDOW_SIZE) / 2.0,
            (screen_height - WINDOW_SIZE) / 2.0,
        );
        pet.animation.play(CatActivity::Walk.animation_name());
        runtime.add_character(pet);

        Self {
            runtime,
            running: true,
            passthrough: true,
            interactive: false,
            activity: CatActivity::Walk,
            requested_activity: None,
            last_render: None,
            message: "Ctrl+Shift+P 暂停或恢复；按住 Ctrl 可拖动窗口".into(),
        }
    }

    fn position(&self) -> Point<Pixels> {
        let position = self.runtime.characters[0].position;
        point(px(position.x), px(position.y))
    }

    fn set_position(&mut self, position: Point<Pixels>) {
        self.runtime.characters[0].position = Vec2::new(position.x.as_f32(), position.y.as_f32());
    }

    fn toggle_running(&mut self) -> bool {
        self.running = !self.running;
        self.passthrough = self.running;
        self.interactive = !self.running;
        if self.running {
            self.requested_activity = Some(CatActivity::Walk);
            self.message = "自动运行中".into();
        } else {
            self.activity = CatActivity::Sleep;
            self.requested_activity = Some(CatActivity::Sleep);
            self.message = "已暂停，控制面板可点击".into();
        }
        self.running
    }

    fn request_activity(&mut self, activity: CatActivity) {
        self.running = !matches!(activity, CatActivity::Sleep);
        self.interactive = !matches!(activity, CatActivity::Walk);
        self.passthrough = !self.interactive;
        self.requested_activity = Some(activity);
        self.message = format!("已请求动作：{}", activity.label()).into();
    }

    fn enter_interactive(&mut self) {
        self.running = false;
        self.passthrough = false;
        self.interactive = true;
        self.activity = CatActivity::Sleep;
        self.requested_activity = Some(CatActivity::Sleep);
        self.message = "交互模式：点击按钮或再次按 Ctrl 退出".into();
    }

    /// 退出交互模式，回到自动散步状态。
    /// 强制回到 Walk，确保鼠标穿透立刻开启。
    fn exit_interactive(&mut self) {
        self.running = true;
        self.interactive = false;
        self.passthrough = true;
        self.requested_activity = Some(CatActivity::Walk);
        self.message = "自动运行中".into();
    }

    /// 更新猫的状态。每次调用都会同步 passthrough 状态：
    /// - 非交互模式下：散步时开启鼠标穿透，非散步时关闭穿透（显示面板）
    /// - 交互模式下：由 enter_interactive / request_activity 控制
    fn tick(&mut self, behavior: &mut CatBehavior) {
        if let Some(activity) = self.requested_activity.take() {
            behavior.force(activity);
        }

        let commands = if self.running {
            self.runtime.update(FRAME_TIME.as_secs_f32(), behavior)
        } else {
            self.runtime
                .update(FRAME_TIME.as_secs_f32(), &mut SleepBehavior)
        };

        if !self.running {
            self.activity = CatActivity::Sleep;
        } else {
            self.activity = behavior.activity();
        }
        self.last_render = commands.into_iter().next();

        // 非交互模式下：根据当前活动动态决定鼠标穿透
        // 散步时开启穿透（面板隐藏），非散步时关闭穿透（面板可见可交互）
        if !self.interactive {
            self.passthrough = self.activity == CatActivity::Walk;
        }
    }

    fn current_texture_path(&self) -> SharedString {
        let Some(RenderCommand::DrawSprite { texture, .. }) = &self.last_render else {
            return frame_path(CatActivity::Walk, 0).into();
        };
        texture.0.clone().into()
    }

    /// 从渲染命令中提取水平缩放（用于翻转）。
    fn current_scale_x(&self) -> f32 {
        let Some(RenderCommand::DrawSprite { scale, .. }) = &self.last_render else {
            return 1.0;
        };
        scale.x
    }
}

fn register_cat_assets(runtime: &mut CharacterRuntime) {
    for activity in [
        CatActivity::Walk,
        CatActivity::Sleep,
        CatActivity::Eat,
        CatActivity::Spin,
        CatActivity::Yawn,
        CatActivity::Turn,
    ] {
        runtime.assets.register_animation(AnimationClip::new(
            activity.animation_name(),
            (0..4)
                .map(|frame| TextureId::new(frame_path(activity, frame)))
                .collect(),
            match activity {
                CatActivity::Walk | CatActivity::Spin | CatActivity::Turn => 8.0,
                CatActivity::Eat => 6.0,
                CatActivity::Yawn => 5.0,
                CatActivity::Sleep => 2.0,
            },
            true,
        ));
    }
}

fn frame_path(activity: CatActivity, frame: usize) -> String {
    format!("assets/cat/{}_{}.png", activity.animation_name(), frame)
}

/// 小猫自动行为，支持边界检测、朝向翻转和转头动画。
struct CatBehavior {
    activity: CatActivity,
    angle: f32,
    speed: f32,
    direction_timer: f32,
    direction_interval: f32,
    activity_timer: f32,
    activity_duration: f32,
    screen_width: f32,
    screen_height: f32,
    play_requested: bool,
    /// 转头动画计时器。
    turn_timer: f32,
    /// 转头动画持续时间。
    turn_duration: f32,
    /// 转头后的新角度。
    turn_target_angle: f32,
}

impl CatBehavior {
    fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            activity: CatActivity::Walk,
            angle: 0.0,
            speed: 86.0,
            direction_timer: 0.0,
            direction_interval: 2.0 + rand_f32() * 3.0,
            activity_timer: 0.0,
            activity_duration: 6.0,
            screen_width,
            screen_height,
            play_requested: true,
            turn_timer: 0.0,
            turn_duration: 0.0,
            turn_target_angle: 0.0,
        }
    }

    fn activity(&self) -> CatActivity {
        self.activity
    }

    fn force(&mut self, activity: CatActivity) {
        self.activity = activity;
        self.activity_timer = 0.0;
        self.activity_duration = match activity {
            CatActivity::Walk => 6.0 + rand_f32() * 4.0,
            CatActivity::Eat => 3.0,
            CatActivity::Spin => 2.2,
            CatActivity::Yawn => 2.6,
            CatActivity::Sleep => 4.0,
            CatActivity::Turn => 0.6,
        };
        self.turn_timer = 0.0;
        self.play_requested = true;
    }

    fn update_activity(&mut self, dt: f32) {
        self.activity_timer += dt;
        if self.activity_timer < self.activity_duration {
            return;
        }

        let roll = rand_f32();
        let next = if roll < 0.55 {
            CatActivity::Walk
        } else if roll < 0.72 {
            CatActivity::Eat
        } else if roll < 0.88 {
            CatActivity::Yawn
        } else {
            CatActivity::Spin
        };
        self.force(next);
    }

    /// 检测是否碰到屏幕边界，需要转头。
    /// 注意：由于 physics friction = 0.0，每帧 velocity 都会被归零，
    /// 因此不能依赖 ctx.velocity 来判断移动方向，改用 angle 来推断。
    fn check_boundary_turn(&mut self, ctx: &BehaviorContext) -> bool {
        let margin = 20.0;
        let max_x = self.screen_width - WINDOW_SIZE + 120.0 - margin;
        let min_x = -120.0 + margin;
        let max_y = self.screen_height - WINDOW_SIZE + 120.0 - margin;
        let min_y = -120.0 + margin;

        let at_left = ctx.position.x <= min_x && self.angle.cos() < -0.1;
        let at_right = ctx.position.x >= max_x && self.angle.cos() > 0.1;
        let at_top = ctx.position.y <= min_y && self.angle.sin() < -0.1;
        let at_bottom = ctx.position.y >= max_y && self.angle.sin() > 0.1;

        if at_left || at_right || at_top || at_bottom {
            let center_x = self.screen_width / 2.0;
            let center_y = self.screen_height / 2.0;
            self.turn_target_angle = (center_y - ctx.position.y).atan2(center_x - ctx.position.x);
            return true;
        }
        false
    }

    fn update_direction(&mut self, ctx: &BehaviorContext) -> Vec2 {
        // 紧急回正：如果角色已经超出边界（物理 clamp 后的位置），强制转向中心
        let phys_max_x = self.screen_width - WINDOW_SIZE + 120.0;
        let phys_min_x = -120.0;
        let phys_max_y = self.screen_height - WINDOW_SIZE + 120.0;
        let phys_min_y = -120.0;

        if ctx.position.x >= phys_max_x
            || ctx.position.x <= phys_min_x
            || ctx.position.y >= phys_max_y
            || ctx.position.y <= phys_min_y
        {
            let center_x = self.screen_width / 2.0;
            let center_y = self.screen_height / 2.0;
            self.angle = (center_y - ctx.position.y).atan2(center_x - ctx.position.x);
            // 重置方向计时器，防止马上转向
            self.direction_timer = 0.0;
            self.direction_interval = 1.5;
        }

        self.direction_timer += ctx.dt;
        if self.direction_timer >= self.direction_interval {
            self.direction_timer = 0.0;
            self.direction_interval = 2.0 + rand_f32() * 3.0;
            let change = (rand_f32() - 0.5) * std::f32::consts::PI * 0.5;
            self.angle += change;
        }

        Vec2::new(self.angle.cos() * self.speed, self.angle.sin() * self.speed)
    }
}

impl Behavior for CatBehavior {
    fn update(&mut self, ctx: &mut BehaviorContext) -> BehaviorAction {
        self.update_activity(ctx.dt);

        // 如果在转头动画中，等待转头完成
        if self.activity == CatActivity::Turn {
            self.turn_timer += ctx.dt;
            if self.turn_timer >= self.turn_duration {
                self.angle = self.turn_target_angle;
                // 重置方向计时器，转头后不立即变向
                self.direction_timer = 0.0;
                self.direction_interval = 2.0;
                self.force(CatActivity::Walk);
                return BehaviorAction::PlayAnimation(CatActivity::Walk.animation_name().into());
            }
            return BehaviorAction::Idle;
        }

        if self.play_requested {
            self.play_requested = false;
            return BehaviorAction::PlayAnimation(self.activity.animation_name().into());
        }

        if self.activity.moves() {
            // 检测边界转头（只在靠近边界时触发转头动画）
            if self.check_boundary_turn(ctx) {
                self.activity = CatActivity::Turn;
                self.turn_timer = 0.0;
                self.turn_duration = 0.6;
                self.activity_timer = 0.0;
                return BehaviorAction::PlayAnimation(CatActivity::Turn.animation_name().into());
            }

            BehaviorAction::Move(self.update_direction(ctx))
        } else {
            BehaviorAction::Idle
        }
    }
}

/// 睡眠行为，用于暂停状态。
struct SleepBehavior;

impl Behavior for SleepBehavior {
    fn update(&mut self, _ctx: &mut BehaviorContext) -> BehaviorAction {
        BehaviorAction::PlayAnimation(CatActivity::Sleep.animation_name().into())
    }
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    fn GetAsyncKeyState(v_key: i32) -> i16;
}

#[cfg(target_os = "windows")]
const VK_CONTROL: i32 = 0x11;

#[cfg(target_os = "windows")]
const VK_SHIFT: i32 = 0x10;

#[cfg(target_os = "windows")]
fn is_ctrl_pressed() -> bool {
    unsafe { GetAsyncKeyState(VK_CONTROL) < 0 }
}

/// 检查 Shift 键是否按下。
/// 用于区分 Ctrl 单独按下 vs Ctrl+Shift 组合键（如 Ctrl+Shift+P 快捷键）。
#[cfg(target_os = "windows")]
fn is_shift_pressed() -> bool {
    unsafe { GetAsyncKeyState(VK_SHIFT) < 0 }
}

#[cfg(not(target_os = "windows"))]
fn is_ctrl_pressed() -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
fn is_shift_pressed() -> bool {
    false
}

fn rand_f32() -> f32 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static STATE: AtomicU64 = AtomicU64::new(1);
    let mut value = STATE.fetch_add(1, Ordering::Relaxed);
    value = value.wrapping_mul(6364136223846793005).wrapping_add(1);
    ((value >> 33) as f32) / (u32::MAX as f32)
}

fn main() {
    let _instance = match SingleInstance::acquire(APP_ID) {
        Ok(instance) => instance,
        Err(_) => {
            eprintln!("Another instance is already running. Sending activation signal.");
            let _ = send_activate_to_existing(APP_ID);
            std::process::exit(0);
        }
    };

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let pet_state = Arc::new(Mutex::new(PetSharedState::new(1920.0, 1080.0)));

    application()
        .with_assets(ExampleAssets {
            base: manifest_dir.join("examples"),
        })
        .run(move |cx: &mut App| {
            cx.set_keep_alive_without_windows(true);
            setup_tray(cx);

            let bounds = Bounds::centered(None, size(px(WINDOW_SIZE), px(WINDOW_SIZE)), cx);
            let window_visible = Arc::new(std::sync::atomic::AtomicBool::new(true));
            let window_visible_close = window_visible.clone();
            let view_state = pet_state.clone();

            let window_handle = cx
                .open_window(
                    WindowOptions {
                        titlebar: None,
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        window_background: WindowBackgroundAppearance::Transparent,
                        mouse_passthrough: true,
                        kind: WindowKind::Overlay,
                        ..Default::default()
                    },
                    |window, cx| {
                        let view = cx.new(|_| DesktopPet::new(view_state));
                        window.on_window_should_close(cx, move |window, _cx| {
                            window_visible_close.store(false, std::sync::atomic::Ordering::Release);
                            window.hide_window();
                            false
                        });
                        view
                    },
                )
                .expect("failed to open desktop pet window");

            setup_tray_callbacks(cx, window_handle, window_visible);
            setup_global_hotkey(cx, pet_state.clone(), window_handle);
            spawn_pet_loop(cx, pet_state, window_handle);

            cx.activate(true);
        });
}

fn setup_tray(cx: &mut App) {
    cx.set_tray_tooltip("rgpui-character 小猫桌宠");
    let icon_bytes = include_bytes!("assets/tray-icon.png");
    cx.set_tray_icon(Some(icon_bytes.as_slice()));
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
}

fn setup_tray_callbacks(
    cx: &mut App,
    window_handle: rgpui::WindowHandle<DesktopPet>,
    window_visible: Arc<std::sync::atomic::AtomicBool>,
) {
    let click_handle = window_handle;
    let click_visible = window_visible.clone();
    cx.on_tray_icon_event(move |event, cx| {
        if matches!(event, TrayIconEvent::LeftClick | TrayIconEvent::DoubleClick)
            && !click_visible.load(std::sync::atomic::Ordering::Acquire)
        {
            let visible = click_visible.clone();
            click_handle
                .update(cx, |_, window, _| {
                    visible.store(true, std::sync::atomic::Ordering::Release);
                    window.activate_window();
                })
                .ok();
        }
    });

    cx.on_tray_menu_action(move |id, cx| match id.as_ref() {
        "quit" => cx.quit(),
        "show_window" => {
            let visible = window_visible.clone();
            if let Err(err) = window_handle.update(cx, |_, window, _| {
                visible.store(true, std::sync::atomic::Ordering::Release);
                window.activate_window();
            }) {
                eprintln!("Failed to activate window: {}", err);
            }
        }
        _ => {}
    });
}

fn setup_global_hotkey(
    cx: &mut App,
    pet_state: Arc<Mutex<PetSharedState>>,
    window_handle: rgpui::WindowHandle<DesktopPet>,
) {
    let keystroke = Keystroke::parse("ctrl-shift-p").expect("valid keystroke");
    if let Err(err) = cx.register_global_hotkey(HOTKEY_TOGGLE_PAUSE, &keystroke) {
        eprintln!("Failed to register global hotkey: {}", err);
    }

    cx.on_global_hotkey(move |id, cx| {
        if id == HOTKEY_TOGGLE_PAUSE {
            let running = pet_state.lock().unwrap().toggle_running();
            if let Err(err) = window_handle.update(cx, |_, window, cx| {
                if running {
                    let current_position = window.bounds().origin;
                    pet_state.lock().unwrap().set_position(current_position);
                    window.set_mouse_passthrough(true);
                } else {
                    window.set_mouse_passthrough(false);
                    window.activate_window();
                }
                cx.notify();
            }) {
                eprintln!("Failed to update window: {}", err);
            }
        }
    });
}

fn spawn_pet_loop(
    cx: &mut App,
    pet_state: Arc<Mutex<PetSharedState>>,
    window_handle: rgpui::WindowHandle<DesktopPet>,
) {
    let mut behavior = CatBehavior::new(1920.0, 1080.0);
    let mut was_ctrl_held = false;
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(FRAME_TIME).await;
            let ctrl_held = is_ctrl_pressed();
            let shift_held = is_shift_pressed();

            let _ = window_handle.update(cx, |_, window, cx| {
                let mut state = pet_state.lock().unwrap();
                // Ctrl+Shift 同时按下时不切换交互模式（避免与 Ctrl+Shift+P 快捷键冲突）
                if ctrl_held && !was_ctrl_held && !shift_held {
                    if state.interactive {
                        state.exit_interactive();
                    } else {
                        state.enter_interactive();
                        window.set_mouse_passthrough(false);
                        window.activate_window();
                    }
                }

                if ctrl_held {
                    state.set_position(window.bounds().origin);
                }

                was_ctrl_held = ctrl_held;

                if state.interactive {
                    window.set_mouse_passthrough(false);
                    state.tick(&mut behavior);
                } else if state.running {
                    state.tick(&mut behavior);
                    let should_passthrough = state.passthrough;
                    let position = state.position();
                    drop(state);
                    window.set_mouse_passthrough(should_passthrough);
                    window.set_position(position);
                } else {
                    state.tick(&mut behavior);
                    let should_passthrough = state.passthrough;
                    drop(state);
                    window.set_mouse_passthrough(should_passthrough);
                }
                cx.notify();
            });
        }
    })
    .detach();
}

/// 桌宠视图组件。
struct DesktopPet {
    state: Arc<Mutex<PetSharedState>>,
}

impl DesktopPet {
    fn new(state: Arc<Mutex<PetSharedState>>) -> Self {
        Self { state }
    }
}

impl Render for DesktopPet {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.lock().unwrap();
        let texture_path = state.current_texture_path();
        let scale_x = state.current_scale_x();
        let passthrough = state.passthrough;
        let activity = state.activity;
        let message = state.message.clone();
        let shared_state = self.state.clone();
        drop(state);

        div()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .items_center()
            .justify_center()
            .child(
                img(texture_path)
                    .size(px(SPRITE_SIZE))
                    .scale_xy(scale_x, 1.0)
                    .with_fallback(|| div().child("图片加载失败").into_any_element()),
            )
            .when(!passthrough, |this| {
                this.child(control_panel(shared_state, activity, message))
            })
    }
}

fn control_panel(
    state: Arc<Mutex<PetSharedState>>,
    activity: CatActivity,
    message: SharedString,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .mt_2()
        .p_3()
        .rounded(px(8.0))
        .bg(rgba(0x00000099))
        .text_color(rgb(0xffffff))
        .child(
            div()
                .text_sm()
                .font_weight(rgpui::FontWeight::BOLD)
                .child(format!("当前动作：{}", activity.label())),
        )
        .child(div().text_xs().child(message))
        .child(
            div()
                .flex()
                .gap_1()
                .child(action_button(state.clone(), CatActivity::Walk))
                .child(action_button(state.clone(), CatActivity::Eat))
                .child(action_button(state.clone(), CatActivity::Spin))
                .child(action_button(state.clone(), CatActivity::Yawn))
                .child(action_button(state, CatActivity::Sleep)),
        )
}

fn action_button(state: Arc<Mutex<PetSharedState>>, activity: CatActivity) -> impl IntoElement {
    div()
        .id(format!("activity-{}", activity.animation_name()))
        .px_2()
        .py_1()
        .rounded_sm()
        .bg(rgba(0xffffffdd))
        .text_color(rgb(0x333333))
        .text_xs()
        .cursor_pointer()
        .child(activity.label())
        .on_click(move |_, _, _| {
            state.lock().unwrap().request_activity(activity);
        })
}
