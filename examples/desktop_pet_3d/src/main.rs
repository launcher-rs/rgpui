//! rgpui-3d + rgpui-character 3D 桌面宠物。
//!
//! 结合了 rgpui-character 的 2D 行为/物理系统和 rgpui-3d 的 3D 渲染引擎。
//! 猫在桌面上行走时，使用 3D 模型渲染替代传统 2D sprite 动画。
//!
//! 运行：
//! ```text
//! cargo run --example desktop_pet_3d
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::single_instance::{SingleInstance, send_activate_to_existing};
use rgpui::{
    App, Bounds, Context, Keystroke, Pixels, Point, Render, SharedString, TrayIconEvent,
    TrayMenuItem, Window, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions, div,
    img, point, prelude::*, px, rgb, rgba, size,
};
use rgpui_3d::Scenix3D;
use rgpui_3d::scenix::{self, PerspectiveCamera, SceneGraph, Vec3};
use rgpui_character::{
    Behavior, BehaviorAction, BehaviorContext, Character, CharacterRuntime, PhysicsConfig, Rect,
    Vec2,
};
use rgpui_platform::application;
use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 应用标识符
const APP_ID: &str = "com.example.rgpui-3d-desktop-pet-3d";

/// 覆盖窗口尺寸
const WINDOW_SIZE: f32 = 340.0;

/// 3D 渲染尺寸
const RENDER_W: u32 = 320;
const RENDER_H: u32 = 320;

/// 帧率
const FRAME_TIME: Duration = Duration::from_millis(16);

/// 退出交互模式后锚定窗口位置的帧数
const POSITION_ANCHOR_FRAMES: u8 = 8;

/// 快捷键编号
const HOTKEY_TOGGLE_PAUSE: u32 = 1;

// ============================================================================
// 2D 动作类型（与 desktop_pet 保持兼容）
// ============================================================================

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

// ============================================================================
// 3D 桌宠共享状态
// ============================================================================

struct Pet3DSharedState {
    /// 角色运行时（2D 行为/物理）
    runtime: CharacterRuntime,
    /// 3D 渲染上下文
    ctx_3d: Option<Scenix3D>,
    /// 3D 场景图
    scene_3d: Option<SceneGraph>,
    /// 当前渲染帧
    render_image: Option<Arc<rgpui::RenderImage>>,
    /// 相机参数
    orbit_x: f32,
    orbit_y: f32,
    distance: f32,
    /// 运行/暂停
    running: bool,
    passthrough: bool,
    controls_visible: bool,
    interactive: bool,
    activity: CatActivity,
    resume_activity: CatActivity,
    position_anchor_frames: u8,
    requested_activity: Option<CatActivity>,
    message: SharedString,
    /// 3D 模型绕 Y 轴旋转（弧度），用于方向控制
    model_rotation: f32,
    /// 模型文件路径
    model_loaded: bool,
    /// 帧率统计
    fps: f32,
}

impl Pet3DSharedState {
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

        let mut pet = Character::new("desktop-cat-3d");
        pet.position = Vec2::new(
            (screen_width - WINDOW_SIZE) / 2.0,
            (screen_height - WINDOW_SIZE) / 2.0,
        );
        pet.animation.play("walk");
        runtime.add_character(pet);

        Self {
            runtime,
            ctx_3d: None,
            scene_3d: None,
            render_image: None,
            orbit_x: 0.0,
            orbit_y: -0.15,
            distance: 2.8,
            running: true,
            passthrough: true,
            controls_visible: false,
            interactive: false,
            activity: CatActivity::Walk,
            resume_activity: CatActivity::Walk,
            position_anchor_frames: 0,
            requested_activity: None,
            message: "Ctrl+Shift+P 暂停；按住 Ctrl 拖动".into(),
            model_rotation: 0.0,
            model_loaded: false,
            fps: 0.0,
        }
    }

    fn position(&self) -> Point<Pixels> {
        let p = self.runtime.characters[0].position;
        point(px(p.x), px(p.y))
    }

    fn set_position(&mut self, position: Point<Pixels>) {
        self.runtime.characters[0].position = Vec2::new(position.x.as_f32(), position.y.as_f32());
        self.runtime.characters[0].velocity = Vec2::new(0.0, 0.0);
    }

    fn set_screen_size(&mut self, screen_width: f32, screen_height: f32) {
        self.runtime.physics.bounds = Rect::new(
            -120.0,
            -120.0,
            screen_width - WINDOW_SIZE + 240.0,
            screen_height - WINDOW_SIZE + 240.0,
        );
    }

    fn toggle_running(&mut self) -> bool {
        self.running = !self.running;
        self.passthrough = self.running;
        self.controls_visible = !self.running;
        self.interactive = !self.running;
        if self.running {
            self.requested_activity = Some(self.resume_activity);
            self.message = "自动运行中".into();
        } else {
            self.resume_activity = self.activity;
            self.activity = CatActivity::Sleep;
            self.requested_activity = Some(CatActivity::Sleep);
            self.message = "已暂停".into();
        }
        self.running
    }

    fn request_activity(&mut self, activity: CatActivity) {
        self.running = !matches!(activity, CatActivity::Sleep);
        self.resume_activity = activity;
        self.requested_activity = Some(activity);
        self.message = format!("已请求：{}", activity.label()).into();
    }

    fn enter_interactive(&mut self) {
        self.resume_activity = self.activity;
        self.running = false;
        self.passthrough = false;
        self.controls_visible = false;
        self.interactive = true;
        self.activity = CatActivity::Sleep;
        self.requested_activity = Some(CatActivity::Sleep);
        self.message = "交互模式：点击按钮或再次按 Ctrl 退出".into();
    }

    fn exit_interactive(&mut self) {
        self.running = !matches!(self.resume_activity, CatActivity::Sleep);
        self.interactive = false;
        self.passthrough = true;
        self.controls_visible = false;
        self.position_anchor_frames = POSITION_ANCHOR_FRAMES;
        self.requested_activity = Some(self.resume_activity);
        self.message = "自动运行中".into();
    }

    fn tick(&mut self, behavior: &mut CatBehavior3D) {
        if let Some(activity) = self.requested_activity.take() {
            behavior.force(activity);
        }

        let _commands = if self.running {
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

        if !self.interactive {
            self.controls_visible = false;
            self.passthrough = true;
        }

        // 更新 3D 模型旋转方向
        let vel = self.runtime.characters[0].velocity;
        if vel.x.abs() > 1.0 || vel.y.abs() > 1.0 {
            self.model_rotation = vel.y.atan2(vel.x);
        }
    }

    fn set_hovered(&mut self, hovered: bool) {
        if self.interactive {
            self.controls_visible = hovered;
            self.passthrough = false;
        } else {
            self.controls_visible = false;
            self.passthrough = true;
        }
    }

    fn anchor_position_if_needed(&mut self, position: Point<Pixels>) -> bool {
        if self.position_anchor_frames == 0 {
            return false;
        }
        self.set_position(position);
        self.position_anchor_frames -= 1;
        true
    }

    fn anchoring_position(&self) -> bool {
        self.position_anchor_frames > 0
    }
}

// ============================================================================
// 3D 猫行为（模拟 CatActivity 状态机，映射到 3D 模型）
// ============================================================================

struct CatBehavior3D {
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
    turn_timer: f32,
    turn_duration: f32,
    turn_target_angle: f32,
}

impl CatBehavior3D {
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

    fn set_screen_size(&mut self, screen_width: f32, screen_height: f32) {
        self.screen_width = screen_width;
        self.screen_height = screen_height;
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

impl Behavior for CatBehavior3D {
    fn update(&mut self, ctx: &mut BehaviorContext) -> BehaviorAction {
        self.update_activity(ctx.dt);

        if self.activity == CatActivity::Turn {
            self.turn_timer += ctx.dt;
            if self.turn_timer >= self.turn_duration {
                self.angle = self.turn_target_angle;
                self.direction_timer = 0.0;
                self.direction_interval = 2.0;
                self.force(CatActivity::Walk);
                return BehaviorAction::PlayAnimation("walk".into());
            }
            return BehaviorAction::Idle;
        }

        if self.play_requested {
            self.play_requested = false;
            return BehaviorAction::PlayAnimation(self.activity.animation_name().into());
        }

        if self.activity.moves() {
            if self.check_boundary_turn(ctx) {
                self.activity = CatActivity::Turn;
                self.turn_timer = 0.0;
                self.turn_duration = 0.6;
                self.activity_timer = 0.0;
                return BehaviorAction::PlayAnimation("turn".into());
            }
            BehaviorAction::Move(self.update_direction(ctx))
        } else {
            BehaviorAction::Idle
        }
    }
}

struct SleepBehavior;
impl Behavior for SleepBehavior {
    fn update(&mut self, _ctx: &mut BehaviorContext) -> BehaviorAction {
        BehaviorAction::PlayAnimation("sleep".into())
    }
}

// ============================================================================
// Windows Ctrl 检测（与 desktop_pet 一致，略去非 Windows 平台兼容代码）
// ============================================================================

#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    fn GetAsyncKeyState(v_key: i32) -> i16;
    fn GetCursorPos(point: *mut WinCursorPoint) -> i32;
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct WinCursorPoint {
    x: i32,
    y: i32,
}

#[cfg(target_os = "windows")]
const VK_CONTROL: i32 = 0x11;

#[cfg(target_os = "windows")]
fn is_ctrl_pressed() -> bool {
    unsafe { GetAsyncKeyState(VK_CONTROL) < 0 }
}

#[cfg(not(target_os = "windows"))]
fn is_ctrl_pressed() -> bool {
    false
}

fn cursor_screen_position(_scale_factor: f32) -> Option<Point<Pixels>> {
    #[cfg(target_os = "windows")]
    {
        let mut cursor_point = WinCursorPoint { x: 0, y: 0 };
        if unsafe { GetCursorPos(&mut cursor_point) } == 0 {
            return None;
        }
        Some(point(
            px(cursor_point.x as f32 / _scale_factor),
            px(cursor_point.y as f32 / _scale_factor),
        ))
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

fn cursor_in_bounds(position: Point<Pixels>, bounds: Bounds<Pixels>) -> bool {
    let left = bounds.origin.x.as_f32();
    let top = bounds.origin.y.as_f32();
    let right = bounds.origin.x.as_f32() + bounds.size.width.as_f32();
    let bottom = bounds.origin.y.as_f32() + bounds.size.height.as_f32();
    let x = position.x.as_f32();
    let y = position.y.as_f32();
    x >= left && x < right && y >= top && y < bottom
}

fn rand_f32() -> f32 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static STATE: AtomicU64 = AtomicU64::new(1);
    let mut value = STATE.fetch_add(1, Ordering::Relaxed);
    value = value.wrapping_mul(6364136223846793005).wrapping_add(1);
    ((value >> 33) as f32) / (u32::MAX as f32)
}

// ============================================================================
// 3D 渲染线程
// ============================================================================

/// 启动 3D 渲染线程，持续渲染猫模型并更新共享状态中的 RenderImage。
fn spawn_3d_render_thread(state: Arc<Mutex<Pet3DSharedState>>, model_path: String) {
    std::thread::spawn(move || {
        let mut ctx = smol::block_on(async {
            Scenix3D::new(RENDER_W, RENDER_H)
                .await
                .expect("创建 3D 上下文失败")
        });

        // 尝试加载 3D 模型
        let loader = scenix::GltfLoader::new();
        let (scene, _has_skins) = match loader.load_file(&model_path) {
            Ok(asset) => {
                if let Err(e) = ctx.register_gltf_asset(&asset) {
                    eprintln!("[3D] GPU 注册失败: {e}");
                    (asset.scene, false)
                } else {
                    // 尝试加载蒙皮/动画
                    match ctx.load_gltf_skins(&model_path, &asset) {
                        Ok(_) => {
                            eprintln!(
                                "[3D] 蒙皮/动画加载成功，共 {} 个动画",
                                ctx.animation_names().len()
                            );
                        }
                        Err(e) => {
                            eprintln!("[3D] 蒙皮加载失败（非蒙皮模型可忽略）: {e}");
                        }
                    }
                    (asset.scene, ctx.skinned_mesh_count() > 0)
                }
            }
            Err(e) => {
                eprintln!("[3D] 模型加载失败: {e}");
                return;
            }
        };

        ctx.set_clear_color(0.0, 0.0, 0.0, 0.0); // 透明背景

        // 将 ctx 和 scene 移入共享状态
        {
            let mut s = state.lock().unwrap();
            s.ctx_3d = Some(ctx);
            s.scene_3d = Some(scene);
            s.model_loaded = true;
        }

        let anim_speed = 1.0;
        let mut frame_times: Vec<f32> = Vec::with_capacity(30);

        loop {
            let frame_start = Instant::now();

            // 读取状态（一次性获取，避免多次锁）
            let (running, orbit_x, orbit_y, distance, model_rotation) = {
                let s = state.lock().unwrap();
                (
                    s.running,
                    s.orbit_x,
                    s.orbit_y,
                    s.distance,
                    s.model_rotation,
                )
            };

            // 取出 ctx_3d 和 scene_3d 进行渲染（避免同时借用 state 的多个字段）
            let mut ctx_scene = {
                let mut s = state.lock().unwrap();
                (s.ctx_3d.take(), s.scene_3d.take())
            };

            if let (Some(ctx), Some(scene)) = &mut ctx_scene {
                // 推进动画
                if running && ctx.animation_names().len() > 0 {
                    ctx.advance_animation(FRAME_TIME.as_secs_f32() * anim_speed);
                }

                // 构建相机：从上方斜看猫
                let camera =
                    PerspectiveCamera::new(45.0, RENDER_W as f32 / RENDER_H as f32, 0.1, 100.0)
                        .position(Vec3::new(
                            distance * orbit_x.sin() * orbit_y.cos(),
                            distance * orbit_y.sin() + 0.3,
                            distance * orbit_x.cos() * orbit_y.cos(),
                        ))
                        .target(Vec3::new(0.0, -0.2, 0.0));

                // 应用模型旋转到场景节点
                let node_ids: Vec<scenix::NodeId> = scene.iter_depth_first().collect();
                for id in node_ids {
                    if let Some(node) = scene.get_mut(id) {
                        node.transform.rotation = scenix::Quat::from_axis_angle(
                            Vec3::new(0.0, 1.0, 0.0),
                            -model_rotation,
                        );
                    }
                }

                // 执行渲染
                match ctx.render(scene, &camera) {
                    Ok(render_result) => {
                        let render_image = Arc::new(render_result.into_render_image());
                        let mut s = state.lock().unwrap();
                        s.render_image = Some(render_image);
                    }
                    Err(e) => {
                        eprintln!("[3D] 渲染错误: {e}");
                    }
                }
            }

            // 将 ctx/scene 放回 state
            {
                let mut s = state.lock().unwrap();
                if let (Some(ctx), Some(scene)) = ctx_scene {
                    s.ctx_3d = Some(ctx);
                    s.scene_3d = Some(scene);
                }
            }

            let elapsed = frame_start.elapsed().as_secs_f32();
            frame_times.push(elapsed);
            if frame_times.len() > 30 {
                frame_times.remove(0);
            }
            let avg_fps = frame_times.len() as f32 / frame_times.iter().sum::<f32>();

            {
                let mut s = state.lock().unwrap();
                s.fps = avg_fps;
            }

            let elapsed = frame_start.elapsed();
            if elapsed < FRAME_TIME {
                std::thread::sleep(FRAME_TIME - elapsed);
            }
        }
    });
}

// ============================================================================
// 资源加载器
// ============================================================================

struct ExampleAssets {
    base: PathBuf,
}

impl rgpui::AssetSource for ExampleAssets {
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
                    })
                    .map(SharedString::from)
                    .collect()
            })
            .map_err(Into::into)
    }
}

// ============================================================================
// UI 视图
// ============================================================================

struct DesktopPet3D {
    state: Arc<Mutex<Pet3DSharedState>>,
}

impl DesktopPet3D {
    fn new(state: Arc<Mutex<Pet3DSharedState>>) -> Self {
        Self { state }
    }
}

impl Render for DesktopPet3D {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.lock().unwrap();
        let fps = state.fps;

        let img_elem = match &state.render_image {
            Some(img_ref) => div()
                .size(px(RENDER_W as f32))
                .size(px(RENDER_H as f32))
                .child(img(img_ref.clone())),
            None => div()
                .size(px(RENDER_W as f32))
                .size(px(RENDER_H as f32))
                .flex()
                .items_center()
                .justify_center()
                .text_xl()
                .text_color(rgb(0xffffff))
                .child("3D 猫加载中..."),
        };

        let controls_visible = state.controls_visible;
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
            .child(img_elem)
            .child(
                div()
                    .text_xs()
                    .text_color(rgba(0xffffffaa))
                    .child(format!("{:.0} FPS", fps)),
            )
            .when(controls_visible, |this| {
                this.child(control_panel(shared_state, activity, message))
            })
    }
}

fn control_panel(
    state: Arc<Mutex<Pet3DSharedState>>,
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
                .child(format!("当前：{}", activity.label())),
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

fn action_button(state: Arc<Mutex<Pet3DSharedState>>, activity: CatActivity) -> impl IntoElement {
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

// ============================================================================
// 托盘、快捷键、主循环（简化为 3D 版本）
// ============================================================================

fn setup_tray(cx: &mut App) {
    cx.set_tray_tooltip("rgpui-3d 小猫桌宠（3D 版）");
    cx.set_tray_icon(Some(include_bytes!(
        "../../desktop_pet/assets/tray-icon.png"
    )));
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
    window_handle: rgpui::WindowHandle<DesktopPet3D>,
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
            if let Err(err) = window_handle.update(cx, |_, window, _| {
                window.activate_window();
            }) {
                eprintln!("激活窗口失败: {}", err);
            }
        }
        _ => {}
    });
}

fn setup_global_hotkey(
    cx: &mut App,
    pet_state: Arc<Mutex<Pet3DSharedState>>,
    window_handle: rgpui::WindowHandle<DesktopPet3D>,
) {
    let keystroke = Keystroke::parse("ctrl-shift-p").expect("valid keystroke");
    if let Err(err) = cx.register_global_hotkey(HOTKEY_TOGGLE_PAUSE, &keystroke) {
        eprintln!("注册快捷键失败: {}", err);
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
                eprintln!("窗口更新失败: {}", err);
            }
        }
    });
}

fn spawn_pet_loop(
    cx: &mut App,
    pet_state: Arc<Mutex<Pet3DSharedState>>,
    window_handle: rgpui::WindowHandle<DesktopPet3D>,
    screen_width: f32,
    screen_height: f32,
) {
    let mut behavior = CatBehavior3D::new(screen_width, screen_height);
    let mut was_ctrl_held = false;

    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(FRAME_TIME).await;
            let ctrl_held = is_ctrl_pressed();

            let _ = window_handle.update(cx, |_, window, cx| {
                let window_bounds = window.bounds();
                let scale_factor = window.scale_factor();
                let mut state = pet_state.lock().unwrap();

                if let Some(screen_size) = window.screen_size(cx) {
                    let sw = screen_size.width.as_f32();
                    let sh = screen_size.height.as_f32();
                    behavior.set_screen_size(sw, sh);
                    state.set_screen_size(sw, sh);
                }

                let mouse_hovered = state.interactive
                    && cursor_screen_position(scale_factor)
                        .is_some_and(|pos| cursor_in_bounds(pos, window_bounds));

                let mut exited_interactive = false;

                if ctrl_held && !was_ctrl_held {
                    if state.interactive {
                        state.set_position(window.position());
                        state.exit_interactive();
                        exited_interactive = true;
                    } else {
                        state.enter_interactive();
                        window.set_mouse_passthrough(false);
                        window.activate_window();
                    }
                }

                if ctrl_held {
                    state.set_position(window.position());
                }

                was_ctrl_held = ctrl_held;

                if exited_interactive {
                    let should_passthrough = state.passthrough;
                    drop(state);
                    window.set_mouse_passthrough(should_passthrough);
                } else if state.interactive {
                    state.set_position(window.position());
                    state.tick(&mut behavior);
                    state.set_hovered(mouse_hovered);
                    let should_passthrough = state.passthrough;
                    drop(state);
                    window.set_mouse_passthrough(should_passthrough);
                } else if state.running {
                    if state.anchoring_position() {
                        state.anchor_position_if_needed(window.position());
                        let should_passthrough = state.passthrough;
                        drop(state);
                        window.set_mouse_passthrough(should_passthrough);
                        cx.notify();
                        return;
                    }

                    state.tick(&mut behavior);
                    state.set_hovered(mouse_hovered);
                    let should_passthrough = state.passthrough;
                    let position = state.position();
                    drop(state);
                    window.set_mouse_passthrough(should_passthrough);
                    window.set_position(position);
                } else {
                    state.tick(&mut behavior);
                    state.set_hovered(mouse_hovered);
                    state.anchor_position_if_needed(window.position());
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

// ============================================================================
// 入口
// ============================================================================

fn main() {
    let _instance = match SingleInstance::acquire(APP_ID) {
        Ok(instance) => instance,
        Err(_) => {
            let _ = send_activate_to_existing(APP_ID);
            std::process::exit(0);
        }
    };

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    application()
        .with_assets(ExampleAssets {
            base: manifest_dir.clone(),
        })
        .run(move |cx: &mut App| {
            cx.set_keep_alive_without_windows(true);
            setup_tray(cx);

            let screen_bounds = cx
                .primary_display()
                .map(|display| display.bounds())
                .unwrap_or_else(|| Bounds::centered(None, size(px(1920.0), px(1080.0)), cx));
            let screen_width = screen_bounds.size.width.as_f32();
            let screen_height = screen_bounds.size.height.as_f32();

            let pet_state = Arc::new(Mutex::new(Pet3DSharedState::new(
                screen_width,
                screen_height,
            )));
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
                        let view = cx.new(|cx| {
                            cx.observe_window_bounds(
                                window,
                                |this: &mut DesktopPet3D, window, _| {
                                    this.state.lock().unwrap().set_position(window.position());
                                },
                            )
                            .detach();
                            DesktopPet3D::new(view_state)
                        });

                        window.on_window_should_close(cx, move |window, _cx| {
                            window_visible_close.store(false, std::sync::atomic::Ordering::Release);
                            window.hide_window();
                            false
                        });

                        view
                    },
                )
                .expect("打开桌宠窗口失败");

            setup_tray_callbacks(cx, window_handle, window_visible);
            setup_global_hotkey(cx, pet_state.clone(), window_handle);
            spawn_pet_loop(
                cx,
                pet_state.clone(),
                window_handle,
                screen_width,
                screen_height,
            );

            // 启动 3D 渲染线程
            // 3D 模型文件位于 crates/rgpui-3d/examples/3d/1.glb
            let model_path = format!(
                "{}/../../crates/rgpui-3d/examples/3d/1.glb",
                env!("CARGO_MANIFEST_DIR")
            );
            let render_state = pet_state.clone();
            spawn_3d_render_thread(render_state, model_path);

            cx.activate(true);
        });
}
