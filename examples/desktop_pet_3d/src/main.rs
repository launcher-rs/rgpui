//! rgpui-3d 3D 桌面宠物演示。
//!
//! 加载 glTF 模型，在鼠标穿透模式下自动行走，支持系统托盘控制动画切换和穿透开关。
//!
//! 运行：
//! ```text
//! cargo run --example desktop_pet_3d
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, Point, Render, SharedString, TrayMenuItem, Window,
    WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions, div, img, prelude::*, px,
    rgb, size,
};
use rgpui_3d::Scenix3D;
use rgpui_3d::scenix::{self, PerspectiveCamera, Quat, Transform, Vec3};
use rgpui_platform::application;
use std::borrow::Cow;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 3D 渲染尺寸
const RENDER_W: u32 = 320;
const RENDER_H: u32 = 320;

/// 帧率间隔
const FRAME_TIME: Duration = Duration::from_millis(33);

/// 默认动画索引（walk）
const DEFAULT_ANIM_INDEX: usize = 14;

/// 行走速度（像素/秒）
const WALK_SPEED: f32 = 60.0;

/// 方向随机变化的基准间隔（秒）
const DIRECTION_BASE_INTERVAL: f32 = 3.0;

/// 方向随机变化的波动范围（秒）
const DIRECTION_JITTER: f32 = 2.0;

/// 屏幕边界留白（像素）
const BOUNDARY_MARGIN: f32 = 40.0;

/// 方向键每帧旋转角度（弧度）
const ANGLE_STEP: f32 = 0.05;

/// 模型默认朝向 -Z（glTF 标准），屏幕 +X 对应 3D 绕 Y 轴旋转 +PI/2
const MODEL_YAW_OFFSET: f32 = PI / 2.0;

// ============================================================================
// Windows 平台：按键检测
// ============================================================================

#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    fn GetAsyncKeyState(v_key: i32) -> i16;
}

/// F9 键的虚拟键代码
#[cfg(target_os = "windows")]
const VK_F9: i32 = 0x78;

/// 方向键虚拟键代码
#[cfg(target_os = "windows")]
const VK_LEFT: i32 = 0x25;
#[cfg(target_os = "windows")]
const VK_RIGHT: i32 = 0x27;
#[cfg(target_os = "windows")]
const VK_UP: i32 = 0x26;
#[cfg(target_os = "windows")]
const VK_DOWN: i32 = 0x28;

/// 按键是否正在按下
#[cfg(target_os = "windows")]
fn is_key_pressed(vk: i32) -> bool {
    unsafe { GetAsyncKeyState(vk) < 0 }
}

#[cfg(not(target_os = "windows"))]
fn is_key_pressed(_vk: i32) -> bool {
    false
}

// ============================================================================
// 共享状态
// ============================================================================

struct SharedState {
    /// 轨道旋转角度 X（控制相机水平旋转）
    orbit_x: f32,
    /// 轨道旋转角度 Y（俯仰角）
    orbit_y: f32,
    /// 相机距离
    distance: f32,
    /// 请求切换的动画索引
    requested_anim_index: Option<usize>,
    /// 当前动画索引
    current_anim_index: Option<usize>,
    /// 动画名称列表
    anim_names: Vec<String>,
    /// 3D 渲染帧
    render_image: Option<Arc<rgpui::RenderImage>>,
    /// 帧率
    fps: f32,
    /// 是否处于交互模式（true 时关闭穿透、暂停行走）
    interactive: bool,
    // ── 行走运动参数 ──
    /// 窗口 X 位置
    pos_x: f32,
    /// 窗口 Y 位置
    pos_y: f32,
    /// 行走方向角度（弧度，0=右，PI=左）
    direction_angle: f32,
    /// 方向变化累计时间
    direction_timer: f32,
    /// 下一次方向变化的间隔
    direction_interval: f32,
    /// 屏幕宽度
    screen_width: f32,
    /// 屏幕高度
    screen_height: f32,
    /// 当前是否正在行走（非交互 + 有 walk 动画时为 true）
    walking: bool,
    /// 托盘菜单是否需要刷新
    menu_dirty: bool,
    /// 托盘菜单是否已刷新过动画列表
    menu_refreshed: bool,
    /// 模型绕 Y 轴的旋转角度（弧度，控制模型在场景中的朝向）
    model_yaw: f32,
    /// 模型当前实际旋转角度（用于平滑插值）
    current_model_yaw: f32,
    /// 交互模式下保存的用户视角（退出交互时恢复）
    saved_orbit_x: f32,
    saved_orbit_y: f32,
    saved_distance: f32,
}

impl SharedState {
    fn new() -> Self {
        Self {
            orbit_x: -0.8,
            orbit_y: -0.3,
            distance: 15.0,
            requested_anim_index: Some(DEFAULT_ANIM_INDEX),
            current_anim_index: None,
            anim_names: Vec::new(),
            render_image: None,
            fps: 0.0,
            interactive: false,
            pos_x: 100.0,
            pos_y: 100.0,
            direction_angle: 0.0,
            direction_timer: 0.0,
            direction_interval: DIRECTION_BASE_INTERVAL,
            screen_width: 1920.0,
            screen_height: 1080.0,
            walking: false,
            menu_dirty: false,
            menu_refreshed: false,
            model_yaw: 0.0,
            current_model_yaw: 0.0,
            saved_orbit_x: 0.0,
            saved_orbit_y: -0.3,
            saved_distance: 15.0,
        }
    }

    /// 更新行走方向（定时随机偏转 + 边界反弹）
    fn tick_direction(&mut self, dt: f32) {
        if !self.walking {
            return;
        }

        self.direction_timer += dt;
        if self.direction_timer >= self.direction_interval {
            self.direction_timer = 0.0;
            self.direction_interval =
                DIRECTION_BASE_INTERVAL + (pseudo_random() * 2.0 - 1.0) * DIRECTION_JITTER;
            let change = (pseudo_random() - 0.5) * PI * 0.4;
            self.direction_angle += change;
        }

        // 边界检测：接近屏幕边缘时向中心偏转
        let win_w = RENDER_W as f32;
        let win_h = RENDER_H as f32;
        let min_x = BOUNDARY_MARGIN;
        let max_x = self.screen_width - win_w - BOUNDARY_MARGIN;
        let min_y = BOUNDARY_MARGIN;
        let max_y = self.screen_height - win_h - BOUNDARY_MARGIN;

        let at_left = self.pos_x <= min_x && self.direction_angle.cos() < -0.1;
        let at_right = self.pos_x >= max_x && self.direction_angle.cos() > 0.1;
        let at_top = self.pos_y <= min_y && self.direction_angle.sin() < -0.1;
        let at_bottom = self.pos_y >= max_y && self.direction_angle.sin() > 0.1;

        if at_left || at_right || at_top || at_bottom {
            let center_x = (min_x + max_x) * 0.5;
            let center_y = (min_y + max_y) * 0.5;
            self.direction_angle = (center_y - self.pos_y).atan2(center_x - self.pos_x);
            self.direction_timer = 0.0;
            self.direction_interval = 1.5;
        }

        // 更新位置
        self.pos_x += self.direction_angle.cos() * WALK_SPEED * dt;
        self.pos_y += self.direction_angle.sin() * WALK_SPEED * dt;

        // 钳位到屏幕范围
        self.pos_x = self.pos_x.clamp(min_x, max_x);
        self.pos_y = self.pos_y.clamp(min_y, max_y);
    }
}

/// 简单的伪随机数生成器（0.0 ~ 1.0），避免引入 rand 依赖
fn pseudo_random() -> f32 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos % 10000) as f32 / 10000.0
}

/// 角度插值，自动处理 -PI ~ PI 跨越
fn lerp_angle(from: f32, to: f32, t: f32) -> f32 {
    let mut diff = to - from;
    // 归一化到 -PI ~ PI 范围
    while diff > PI {
        diff -= 2.0 * PI;
    }
    while diff < -PI {
        diff += 2.0 * PI;
    }
    from + diff * t.clamp(0.0, 1.0)
}

// ============================================================================
// 3D 渲染线程
// ============================================================================

fn spawn_3d_render_thread(state: Arc<Mutex<SharedState>>, model_path: String) {
    std::thread::spawn(move || {
        let mut ctx = smol::block_on(async {
            Scenix3D::new(RENDER_W, RENDER_H)
                .await
                .expect("创建 3D 上下文失败")
        });

        let loader = scenix::GltfLoader::new();
        let mut scene = match loader.load_file(&model_path) {
            Ok(asset) => {
                if let Err(e) = ctx.register_gltf_asset(&asset) {
                    eprintln!("[3D] GPU 注册失败: {e}");
                } else {
                    match ctx.load_gltf_skins(&model_path, &asset) {
                        Ok(_) => {
                            let names = ctx.animation_names();
                            eprintln!("[3D] 蒙皮加载成功，共 {} 个动画: {:?}", names.len(), names);
                            let mut s = state.lock().unwrap();
                            s.anim_names = names;
                        }
                        Err(e) => {
                            eprintln!("[3D] 蒙皮加载失败: {e}");
                        }
                    }
                }
                asset.scene
            }
            Err(e) => {
                eprintln!("[3D] 模型加载失败: {e}");
                return;
            }
        };

        ctx.set_clear_color(0.0, 0.0, 0.0, 0.0);

        let anim_speed = 1.0;
        let mut frame_times: Vec<f32> = Vec::with_capacity(30);
        let mut root_original_transforms: HashMap<scenix::NodeId, Transform> = HashMap::new();

        loop {
            let frame_start = Instant::now();

            // ── 一次性读取输入 ──
            let (orbit_x, orbit_y, distance, anim_idx, model_yaw) = {
                let mut s = state.lock().unwrap();
                (
                    s.orbit_x,
                    s.orbit_y,
                    s.distance,
                    s.requested_anim_index.take(),
                    s.current_model_yaw,
                )
            };

            // 切换动画
            if let Some(idx) = anim_idx {
                let anim_count = ctx.animation_names().len();
                if idx < anim_count {
                    ctx.set_active_animation(idx);
                    ctx.set_animation_time(0.0);
                    let mut s = state.lock().unwrap();
                    s.current_anim_index = Some(idx);
                    eprintln!("[3D] 切换到动画 {}: {}", idx, ctx.animation_names()[idx]);
                }
            }

            // 推进动画
            let anim_count = ctx.animation_names().len();
            if anim_count > 0 {
                ctx.advance_animation(FRAME_TIME.as_secs_f32() * anim_speed);
            }

            // 旋转场景根节点，使模型面向行走方向
            let root_ids: Vec<_> = scene.roots().to_vec();
            // 首帧保存根节点原始变换
            for &root_id in &root_ids {
                root_original_transforms.entry(root_id).or_insert_with(|| {
                    scene
                        .get(root_id)
                        .map(|n| n.transform)
                        .unwrap_or(Transform::IDENTITY)
                });
            }
            let yaw_quat = Quat::from_axis_angle(Vec3::Y, model_yaw);
            for &root_id in &root_ids {
                if let Some(&orig) = root_original_transforms.get(&root_id) {
                    let new_rotation = orig.rotation.mul_quat(yaw_quat);
                    let _ = scene.set_local_transform(
                        root_id,
                        Transform::new(orig.translation, new_rotation, orig.scale),
                    );
                }
            }

            // 构建相机（orbit_x 控制水平旋转，模拟模型朝向）
            let camera =
                PerspectiveCamera::new(45.0, RENDER_W as f32 / RENDER_H as f32, 0.1, 100.0)
                    .position(Vec3::new(
                        distance * orbit_x.sin() * orbit_y.cos(),
                        distance * orbit_y.sin() + 0.5,
                        distance * orbit_x.cos() * orbit_y.cos(),
                    ))
                    .target(Vec3::new(0.0, -0.2, 0.0));

            match ctx.render(&mut scene, &camera) {
                Ok(render_result) => {
                    let render_image = Arc::new(render_result.into_render_image());

                    let elapsed = frame_start.elapsed().as_secs_f32();
                    frame_times.push(elapsed);
                    if frame_times.len() > 30 {
                        frame_times.remove(0);
                    }
                    let avg_fps = frame_times.len() as f32 / frame_times.iter().sum::<f32>();

                    {
                        let mut s = state.lock().unwrap();
                        s.render_image = Some(render_image);
                        s.fps = avg_fps;
                    }
                }
                Err(e) => {
                    eprintln!("[3D] 渲染错误: {e}");
                }
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
// UI 视图（极简，仅显示 3D 渲染）
// ============================================================================

struct View {
    state: Arc<Mutex<SharedState>>,
}

impl View {
    fn new(state: Arc<Mutex<SharedState>>) -> Self {
        Self { state }
    }
}

impl Render for View {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let render_image = {
            let s = self.state.lock().unwrap();
            s.render_image.clone()
        };

        match &render_image {
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
                .child("加载中..."),
        }
    }
}

// ============================================================================
// 托盘菜单构建
// ============================================================================

/// 构建托盘菜单（动画子菜单 + 穿透开关 + 退出）
fn build_tray_menu(anim_names: &[String], interactive: bool) -> Vec<TrayMenuItem> {
    // 动画子菜单项
    let anim_items: Vec<TrayMenuItem> = anim_names
        .iter()
        .enumerate()
        .map(|(i, name)| TrayMenuItem::Action {
            label: name.clone().into(),
            id: format!("anim_{}", i).into(),
        })
        .collect();

    vec![
        TrayMenuItem::Action {
            label: "显示窗口".into(),
            id: "show_window".into(),
        },
        TrayMenuItem::Separator,
        TrayMenuItem::Submenu {
            label: "切换动画".into(),
            items: anim_items,
        },
        TrayMenuItem::Toggle {
            label: "交互模式（关闭穿透）".into(),
            checked: interactive,
            id: "toggle_interactive".into(),
        },
        TrayMenuItem::Separator,
        TrayMenuItem::Action {
            label: "退出".into(),
            id: "quit".into(),
        },
    ]
}

// ============================================================================
// 入口
// ============================================================================

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    application()
        .with_assets(ExampleAssets {
            base: manifest_dir.clone(),
        })
        .run(move |cx: &mut App| {
            cx.set_keep_alive_without_windows(true);

            // ── 托盘设置 ──
            cx.set_tray_tooltip("3D 桌面宠物");
            let icon_bytes = include_bytes!("../../../crates/rgpui/examples/image/app-icon.png");
            cx.set_tray_icon(Some(icon_bytes.as_slice()));
            cx.set_tray_menu(build_tray_menu(&[], false));

            // ── 共享状态 ──
            let state = Arc::new(Mutex::new(SharedState::new()));
            let state_for_tray = state.clone();
            let state_for_movement = state.clone();
            let state_for_close = state.clone();
            let state_for_view = state.clone();

            // ── 打开窗口 ──
            let bounds = Bounds::new(
                Point::new(px(100.0), px(100.0)),
                size(px(RENDER_W as f32), px(RENDER_H as f32)),
            );
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
                        // 窗口关闭时隐藏到托盘
                        let close_state = state_for_close.clone();
                        window.on_window_should_close(cx, move |window, _cx| {
                            let mut s = close_state.lock().unwrap();
                            s.interactive = false;
                            s.walking = true;
                            drop(s);
                            window.set_mouse_passthrough(true);
                            window.hide_window();
                            false
                        });

                        cx.new(move |cx| {
                            cx.spawn(async move |this, cx| {
                                loop {
                                    smol::Timer::after(Duration::from_millis(50)).await;
                                    let _ = this.update(cx, |_, cx| cx.notify());
                                }
                            })
                            .detach();

                            View::new(state_for_view)
                        })
                    },
                )
                .expect("打开窗口失败");

            // ── 托盘菜单回调 ──
            let tray_handle = window_handle;
            cx.on_tray_menu_action(move |id, cx| match id.as_ref() {
                "quit" => {
                    cx.quit();
                }
                "show_window" => {
                    let _ = tray_handle.update(cx, |_, window, _| {
                        window.activate_window();
                    });
                }
                "toggle_interactive" => {
                    let h = tray_handle;
                    let s = state_for_tray.clone();
                    let _ = h.update(cx, move |_, window, _cx| {
                        let mut st = s.lock().unwrap();
                        st.interactive = !st.interactive;
                        if st.interactive {
                            st.walking = false;
                            st.orbit_x = st.saved_orbit_x;
                            st.orbit_y = st.saved_orbit_y;
                            st.distance = st.saved_distance;
                            st.model_yaw = 0.0;
                        } else {
                            st.saved_orbit_x = st.orbit_x;
                            st.saved_orbit_y = st.orbit_y;
                            st.saved_distance = st.distance;
                        }
                        let interactive = st.interactive;
                        st.menu_dirty = true;
                        drop(st);
                        window.set_mouse_passthrough(!interactive);
                        if interactive {
                            window.activate_window();
                        }
                    });
                }
                anim_id if anim_id.starts_with("anim_") => {
                    if let Ok(idx) = anim_id[5..].parse::<usize>() {
                        let h = tray_handle;
                        let s = state_for_tray.clone();
                        let _ = h.update(cx, move |_, _window, cx| {
                            let mut st = s.lock().unwrap();
                            st.requested_anim_index = Some(idx);
                            drop(st);
                            cx.notify();
                        });
                    }
                }
                _ => {}
            });

            // ── 托盘图标事件 ──
            let tray_icon_handle = window_handle;
            cx.on_tray_icon_event(move |event, cx| {
                if matches!(event, rgpui::TrayIconEvent::DoubleClick) {
                    let _ = tray_icon_handle.update(cx, |_, window, _| {
                        window.activate_window();
                    });
                }
            });

            // ── 启动 3D 渲染线程 ──
            let model_path = format!(
                "{}/../../crates/rgpui-3d/examples/3d/1.glb",
                env!("CARGO_MANIFEST_DIR")
            );
            spawn_3d_render_thread(state.clone(), model_path);

            // ── F9 键轮询：切换交互/穿透 ──
            let toggle_state = state.clone();
            let toggle_handle = window_handle;
            let toggle_state_loop = toggle_state.clone();
            let toggle_handle_loop = toggle_handle;
            cx.spawn(async move |cx| {
                let mut was_pressed = false;
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;

                    let pressed = is_key_pressed(VK_F9);
                    if pressed && !was_pressed {
                        let h = toggle_handle_loop;
                        let s = toggle_state_loop.clone();
                        let _ = h.update(cx, move |_, window, cx| {
                            let mut st = s.lock().unwrap();
                            st.interactive = !st.interactive;
                            if st.interactive {
                                // 进入交互：保存当前视角，恢复用户之前的调整
                                st.walking = false;
                                st.orbit_x = st.saved_orbit_x;
                                st.orbit_y = st.saved_orbit_y;
                                st.distance = st.saved_distance;
                                st.model_yaw = 0.0;
                            } else {
                                // 退出交互：保存用户调整的视角
                                st.saved_orbit_x = st.orbit_x;
                                st.saved_orbit_y = st.orbit_y;
                                st.saved_distance = st.distance;
                            }
                            let interactive = st.interactive;
                            st.menu_dirty = true;
                            drop(st);
                            window.set_mouse_passthrough(!interactive);
                            if interactive {
                                window.activate_window();
                            }
                            // 刷新托盘菜单
                            let (names, inter) = {
                                let st = s.lock().unwrap();
                                (st.anim_names.clone(), st.interactive)
                            };
                            if !names.is_empty() {
                                cx.set_tray_menu(build_tray_menu(&names, inter));
                            }
                            cx.notify();
                        });
                    }
                    was_pressed = pressed;

                    // 交互模式下用方向键调整模型角度
                    {
                        let h = toggle_handle_loop;
                        let s = toggle_state_loop.clone();
                        let _ = h.update(cx, move |_, _window, cx| {
                            let mut st = s.lock().unwrap();
                            if !st.interactive {
                                return;
                            }
                            let mut changed = false;
                            if is_key_pressed(VK_LEFT) {
                                st.orbit_x += ANGLE_STEP;
                                changed = true;
                            }
                            if is_key_pressed(VK_RIGHT) {
                                st.orbit_x -= ANGLE_STEP;
                                changed = true;
                            }
                            if is_key_pressed(VK_UP) {
                                st.orbit_y = (st.orbit_y - ANGLE_STEP).clamp(-1.5, 1.5);
                                changed = true;
                            }
                            if is_key_pressed(VK_DOWN) {
                                st.orbit_y = (st.orbit_y + ANGLE_STEP).clamp(-1.5, 1.5);
                                changed = true;
                            }
                            if changed {
                                drop(st);
                                cx.notify();
                            }
                        });
                    }

                    // 检查菜单是否需要刷新（动画列表首次加载完成时）
                    {
                        let h = toggle_handle_loop;
                        let s = toggle_state_loop.clone();
                        let _ = h.update(cx, move |_, _window, cx| {
                            let mut st = s.lock().unwrap();
                            if st.menu_dirty {
                                st.menu_dirty = false;
                                let names = st.anim_names.clone();
                                let inter = st.interactive;
                                drop(st);
                                if !names.is_empty() {
                                    cx.set_tray_menu(build_tray_menu(&names, inter));
                                }
                            }
                        });
                    }
                }
            })
            .detach();

            // ── 行走运动循环 ──
            let movement_handle = window_handle;
            let movement_state = state_for_movement.clone();
            cx.spawn(async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(33))
                        .await;

                    let h = movement_handle;
                    let s = movement_state.clone();
                    let _ = h.update(cx, move |_, window, cx| {
                        let mut st = s.lock().unwrap();

                        if !st.interactive {
                            if !st.walking {
                                st.walking = true;
                                st.requested_anim_index = Some(DEFAULT_ANIM_INDEX);
                                let win_pos = window.position();
                                st.pos_x = win_pos.x.as_f32();
                                st.pos_y = win_pos.y.as_f32();
                            }

                            let dt = FRAME_TIME.as_secs_f32();
                            st.tick_direction(dt);
                            // 模型朝向行走方向（通过旋转场景根节点实现）
                            // direction_angle: 0=右, PI/2=下, PI=左, -PI/2=上
                            // 模型默认面朝 -Z，需加偏移使其面朝屏幕右侧
                            st.model_yaw = st.direction_angle + MODEL_YAW_OFFSET;
                            // 平滑插值旋转，避免瞬间跳变
                            let lerp_speed = 8.0 * dt;
                            st.current_model_yaw =
                                lerp_angle(st.current_model_yaw, st.model_yaw, lerp_speed);

                            let pos_x = st.pos_x;
                            let pos_y = st.pos_y;

                            // 检测动画列表是否首次加载完成，标记菜单需要刷新
                            if !st.anim_names.is_empty() && !st.menu_refreshed {
                                st.menu_dirty = true;
                                st.menu_refreshed = true;
                            }

                            drop(st);

                            window.set_position(Point::new(px(pos_x), px(pos_y)));
                        }

                        cx.notify();
                    });
                }
            })
            .detach();

            cx.activate(true);
        });
}
