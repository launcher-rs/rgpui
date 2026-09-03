//! rgpui + scenekit 通用 3D 模型查看器
//!
//! 基于 scenekit_skin.rs，支持命令行动加模型、运行时文件对话框切换模型、
//! 骨骼可视化、动画控制、关节操作等完整功能。
//!
//! 用法:
//!   cargo run --example viewer3d
//!   cargo run --example viewer3d -- --model 模型.glb
//!   cargo run --example viewer3d -- --auto-rotate
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rgpui::{
    App, Bounds, Context, FocusHandle, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Render, RenderImage, ScrollDelta, ScrollWheelEvent, TitlebarOptions, Window,
    WindowBounds, WindowOptions, div, img, prelude::*, px, rgb, size,
};
use rgpui_3d::Scenix3D;
use rgpui_3d::scenekit::{self, PerspectiveCamera, SceneGraph, Vec3};
use rgpui_platform::application;

const RENDER_W: u32 = 800;
const RENDER_H: u32 = 600;

const MODEL_EXTENSIONS: &[&str] = &["glb", "gltf"];

fn is_supported_model(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| MODEL_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

#[derive(Clone)]
struct JointScreenInfo {
    index: usize,
    screen_x: f32,
    screen_y: f32,
}

struct SharedState {
    orbit_x: f32,
    orbit_y: f32,
    distance: f32,
    is_dragging: bool,
    drag_start_x: f32,
    drag_start_y: f32,
    render_image: Option<Arc<RenderImage>>,
    fps: f32,
    model_info: String,
    anim_names: Vec<String>,
    current_anim: usize,
    anim_time: f32,
    anim_duration: f32,
    anim_speed: f32,
    anim_paused: bool,
    joint_count: usize,
    skin_count: usize,
    joints_info: Vec<JointScreenInfo>,
    selected_joint: Option<usize>,
    show_skeleton: bool,
    show_skin: bool,
    msaa_enabled: bool,
    pending_override: Option<(usize, [f32; 4])>,
    pending_clear: bool,
    pending_anim_switch: Option<isize>,
    pending_anim_pause: Option<bool>,
    move_forward: bool,
    move_backward: bool,
    move_left: bool,
    move_right: bool,
    move_up: bool,
    move_down: bool,
    auto_rotate: bool,
    pending_model_path: Option<String>,
}

impl SharedState {
    fn new() -> Self {
        Self {
            orbit_x: 0.0,
            orbit_y: 0.2,
            distance: 15.0,
            is_dragging: false,
            drag_start_x: 0.0,
            drag_start_y: 0.0,
            render_image: None,
            fps: 0.0,
            model_info: String::new(),
            anim_names: Vec::new(),
            current_anim: 0,
            anim_time: 0.0,
            anim_duration: 0.0,
            anim_speed: 1.0,
            anim_paused: false,
            joint_count: 0,
            skin_count: 0,
            joints_info: Vec::new(),
            selected_joint: None,
            show_skeleton: true,
            show_skin: true,
            msaa_enabled: false,
            pending_override: None,
            pending_clear: false,
            pending_anim_switch: None,
            pending_anim_pause: None,
            move_forward: false,
            move_backward: false,
            move_left: false,
            move_right: false,
            move_up: false,
            move_down: false,
            auto_rotate: true,
            pending_model_path: None,
        }
    }
}

fn draw_circle(
    data: &mut [u8],
    width: u32,
    height: u32,
    cx: f32,
    cy: f32,
    radius: f32,
    r: u8,
    g: u8,
    b: u8,
) {
    let cx_i = cx as i32;
    let cy_i = cy as i32;
    let r_i = radius.max(2.0) as i32;
    let r2 = radius * radius;
    for dy in -r_i..=r_i {
        for dx in -r_i..=r_i {
            let px = cx_i + dx;
            let py = cy_i + dy;
            if px >= 0 && px < width as i32 && py >= 0 && py < height as i32 {
                let d = (dx * dx + dy * dy) as f32;
                if d <= r2 {
                    let idx = ((py as u32) * width + (px as u32)) as usize * 4;
                    if idx + 3 < data.len() {
                        data[idx] = b;
                        data[idx + 1] = g;
                        data[idx + 2] = r;
                        data[idx + 3] = 255;
                    }
                }
            }
        }
    }
}

fn draw_line(
    data: &mut [u8],
    width: u32,
    height: u32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    r: u8,
    g: u8,
    b: u8,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let steps = (dx.abs().max(dy.abs()).max(1.0)) as i32;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let px = (x1 + dx * t) as i32;
        let py = (y1 + dy * t) as i32;
        if px >= 0 && px < width as i32 && py >= 0 && py < height as i32 {
            let idx = ((py as u32) * width + (px as u32)) as usize * 4;
            if idx + 3 < data.len() {
                data[idx] = b;
                data[idx + 1] = g;
                data[idx + 2] = r;
                data[idx + 3] = 255;
            }
        }
    }
}

fn project_to_screen(
    pos: Vec3,
    vp: scenekit::Mat4,
    width: u32,
    height: u32,
) -> Option<(f32, f32, f32)> {
    let clip = scenekit::Vec4::new(pos.x, pos.y, pos.z, 1.0);
    let clip = scenekit::Vec4::new(
        vp.cols[0].x * clip.x
            + vp.cols[1].x * clip.y
            + vp.cols[2].x * clip.z
            + vp.cols[3].x * clip.w,
        vp.cols[0].y * clip.x
            + vp.cols[1].y * clip.y
            + vp.cols[2].y * clip.z
            + vp.cols[3].y * clip.w,
        vp.cols[0].z * clip.x
            + vp.cols[1].z * clip.y
            + vp.cols[2].z * clip.z
            + vp.cols[3].z * clip.w,
        vp.cols[0].w * clip.x
            + vp.cols[1].w * clip.y
            + vp.cols[2].w * clip.z
            + vp.cols[3].w * clip.w,
    );
    if clip.w <= 0.0 {
        return None;
    }
    let ndc_x = clip.x / clip.w;
    let ndc_y = clip.y / clip.w;
    let screen_x = (ndc_x * 0.5 + 0.5) * width as f32;
    let screen_y = (1.0 - (ndc_y * 0.5 + 0.5)) * height as f32;
    let depth = clip.z / clip.w;
    Some((screen_x, screen_y, depth))
}

/// 尝试加载 3D 模型文件
fn try_load_model(ctx: &mut Scenix3D, path: &str) -> (Option<SceneGraph>, String, Vec<String>) {
    // 清除上一模型的 GPU 资源，避免残留数据干扰
    ctx.clear_skin_data();
    ctx.gpu_scene_mut().clear_meshes();
    ctx.gpu_scene_mut().clear_materials();
    ctx.gpu_scene_mut().clear_textures();

    let loader = scenekit::GltfLoader::new();
    match loader.load_file(path) {
        Ok(asset) => match ctx.register_gltf_asset(&asset) {
            Ok(_) => {
                let skin_result = ctx.load_gltf_skins(path, &asset);
                let mut names = ctx.animation_names();

                if names.is_empty() && ctx.joint_count() > 0 {
                    ctx.generate_walk_animation(0.8, 0.5);
                    names = ctx.animation_names();
                }

                let info = format!(
                    "{} | {} 网格, {} 材质{}",
                    std::path::Path::new(path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy(),
                    asset.meshes.len(),
                    asset.materials.len(),
                    if !names.is_empty() {
                        format!(" | {} 动画", names.len())
                    } else {
                        String::new()
                    }
                );
                let info = if skin_result.is_err() && names.is_empty() {
                    format!("{info} | 蒙皮加载失败")
                } else {
                    info
                };
                (Some(asset.scene), info, names)
            }
            Err(e) => (None, format!("GPU 注册失败: {e}"), Vec::new()),
        },
        Err(e) => (None, format!("加载失败: {e}"), Vec::new()),
    }
}

struct Viewer3D {
    state: Arc<Mutex<SharedState>>,
    focus_handle: FocusHandle,
}

impl Render for Viewer3D {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.state.lock().unwrap();
        let fps = s.fps;
        let info = s.model_info.clone();
        let ox = s.orbit_x;
        let oy = s.orbit_y;
        let dist = s.distance;
        let anim_names = s.anim_names.clone();
        let current_anim = s.current_anim;
        let anim_time = s.anim_time;
        let anim_duration = s.anim_duration;
        let anim_speed = s.anim_speed;
        let anim_paused = s.anim_paused;
        let joint_count = s.joint_count;
        let skin_count = s.skin_count;
        let joints_info = s.joints_info.clone();
        let selected_joint = s.selected_joint;
        let show_skeleton = s.show_skeleton;
        let show_skin = s.show_skin;
        let auto_rotate = s.auto_rotate;
        let msaa_enabled = s.msaa_enabled;

        let img_elem = match &s.render_image {
            Some(img_ref) => div()
                .w(px(RENDER_W as f32))
                .h(px(RENDER_H as f32))
                .bg(rgb(0x0f0f23))
                .child(img(img_ref.clone())),
            None => div()
                .w(px(RENDER_W as f32))
                .h(px(RENDER_H as f32))
                .bg(rgb(0x1a1a2e))
                .flex()
                .items_center()
                .justify_center()
                .text_xl()
                .text_color(rgb(0x666666))
                .child("加载中..."),
        };
        drop(s);

        let state = self.state.clone();

        _window.focus(&self.focus_handle, _cx);

        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.0))
            .bg(rgb(0x0f0f23))
            .on_key_down({
                let state = state.clone();
                move |ev: &KeyDownEvent, _, _| {
                    let mut s = state.lock().unwrap();
                    match ev.keystroke.key.as_str() {
                        "w" | "up" => s.move_forward = true,
                        "s" | "down" => s.move_backward = true,
                        "a" | "left" => s.move_left = true,
                        "d" | "right" => s.move_right = true,
                        "e" => s.move_up = true,
                        "q" => s.move_down = true,
                        _ => {}
                    }
                }
            })
            .on_key_up({
                let state = state.clone();
                move |ev: &rgpui::KeyUpEvent, _, _| {
                    let mut s = state.lock().unwrap();
                    match ev.keystroke.key.as_str() {
                        "w" | "up" => s.move_forward = false,
                        "s" | "down" => s.move_backward = false,
                        "a" | "left" => s.move_left = false,
                        "d" | "right" => s.move_right = false,
                        "e" => s.move_up = false,
                        "q" => s.move_down = false,
                        _ => {}
                    }
                }
            })
            .child(
                div()
                    .py(px(8.0))
                    .px(px(12.0))
                    .flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_2xl()
                            .text_color(rgb(0xffffff))
                            .child("3D 模型查看器"),
                    )
                    .child(
                        div()
                            .px(px(10.0))
                            .py(px(4.0))
                            .bg(rgb(0x1565c0))
                            .text_color(rgb(0xffffff))
                            .text_sm()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, {
                                let state = state.clone();
                                move |_: &MouseDownEvent, _, cx| {
                                    let state = state.clone();
                                    cx.background_executor()
                                        .spawn(async move {
                                            if let Some(path) = rfd::FileDialog::new()
                                                .add_filter("3D 模型", &["glb", "gltf"])
                                                .pick_file()
                                            {
                                                let path_str = path.to_string_lossy().to_string();
                                                state.lock().unwrap().pending_model_path =
                                                    Some(path_str);
                                            }
                                        })
                                        .detach();
                                }
                            })
                            .child("加载模型"),
                    ),
            )
            .child(
                div()
                    .cursor_grab()
                    .on_mouse_down(MouseButton::Left, {
                        let state = state.clone();
                        move |ev: &MouseDownEvent, _, _| {
                            let s = state.lock().unwrap();
                            let mx = ev.position.x.as_f32();
                            let my = ev.position.y.as_f32();
                            let mut nearest: Option<(usize, f32)> = None;
                            let hit_radius = 15.0;
                            for ji in &s.joints_info {
                                let dx = mx - ji.screen_x;
                                let dy = my - ji.screen_y;
                                let dist2 = dx * dx + dy * dy;
                                let threshold = hit_radius + 10.0;
                                if dist2 < threshold * threshold {
                                    let dist = dist2.sqrt();
                                    if nearest.map_or(true, |(_, nd)| dist < nd) {
                                        nearest = Some((ji.index, dist));
                                    }
                                }
                            }
                            drop(s);
                            let mut s2 = state.lock().unwrap();
                            s2.selected_joint = nearest.map(|(idx, _)| idx);
                            if !s2.is_dragging {
                                s2.is_dragging = true;
                                s2.drag_start_x = ev.position.x.as_f32();
                                s2.drag_start_y = ev.position.y.as_f32();
                            }
                        }
                    })
                    .on_mouse_up(MouseButton::Left, {
                        let state = state.clone();
                        move |_: &MouseUpEvent, _, cx| {
                            let mut s = state.lock().unwrap();
                            s.is_dragging = false;
                            // 检查是否有文件被拖拽放下
                            if let Some(value) = cx.take_active_drag_value() {
                                if let Some(paths) = value.downcast_ref::<rgpui::ExternalPaths>() {
                                    for path in paths.0.iter() {
                                        if is_supported_model(path) {
                                            s.pending_model_path =
                                                Some(path.to_string_lossy().to_string());
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    })
                    .on_mouse_move({
                        let state = state.clone();
                        move |ev: &MouseMoveEvent, _, _| {
                            let mut s = state.lock().unwrap();
                            if s.is_dragging {
                                let dx = ev.position.x.as_f32() - s.drag_start_x;
                                let dy = ev.position.y.as_f32() - s.drag_start_y;
                                s.drag_start_x = ev.position.x.as_f32();
                                s.drag_start_y = ev.position.y.as_f32();
                                s.orbit_x += dx * 0.008;
                                s.orbit_y += dy * 0.008;
                                s.orbit_y = s.orbit_y.clamp(-1.5, 1.5);
                            }
                        }
                    })
                    .on_scroll_wheel({
                        let state = state.clone();
                        move |ev: &ScrollWheelEvent, _, _| {
                            let mut s = state.lock().unwrap();
                            let delta = match ev.delta {
                                ScrollDelta::Pixels(d) => d.y.as_f32(),
                                ScrollDelta::Lines(d) => d.y * 20.0,
                            };
                            if let Some(sel) = s.selected_joint {
                                let angle = delta * 0.02;
                                let q = scenekit::Quat::from_axis_angle(Vec3::Y, angle);
                                s.pending_override = Some((sel, [q.x, q.y, q.z, q.w]));
                            } else {
                                s.distance -= delta * 0.05;
                                s.distance = s.distance.clamp(1.0, 50.0);
                            }
                        }
                    })
                    .child(img_elem),
            )
            .child(
                div()
                    .py(px(6.0))
                    .px(px(8.0))
                    .flex()
                    .gap(px(12.0))
                    .items_center()
                    .child(
                        div()
                            .text_color(rgb(0x64b5f6))
                            .text_sm()
                            .child(format!("FPS: {:.0}", fps)),
                    )
                    .child(div().text_color(rgb(0xaaaaaa)).text_sm().child(info))
                    .child(
                        div()
                            .text_color(rgb(0x888888))
                            .text_sm()
                            .child(format!("关节: {} | 皮肤: {}", joint_count, skin_count)),
                    ),
            )
            .child(
                div()
                    .py(px(4.0))
                    .px(px(8.0))
                    .flex()
                    .gap(px(8.0))
                    .items_center()
                    .child(div().text_color(rgb(0xff9800)).text_sm().child(format!(
                        "动画: {}",
                        anim_names
                            .get(current_anim)
                            .map(|n| n.as_str())
                            .unwrap_or("无")
                    )))
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .bg(rgb(0x333333))
                            .text_color(rgb(0xcccccc))
                            .text_sm()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, {
                                let state = state.clone();
                                move |_: &MouseDownEvent, _, _| {
                                    state.lock().unwrap().pending_anim_switch = Some(-1);
                                }
                            })
                            .child("◀"),
                    )
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .bg(rgb(0x333333))
                            .text_color(rgb(0xcccccc))
                            .text_sm()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, {
                                let state = state.clone();
                                move |_: &MouseDownEvent, _, _| {
                                    state.lock().unwrap().pending_anim_switch = Some(1);
                                }
                            })
                            .child("▶"),
                    )
                    .child(
                        div()
                            .text_color(rgb(0x888888))
                            .text_sm()
                            .child(format!("{:.2}s / {:.2}s", anim_time, anim_duration)),
                    )
                    .child(
                        div()
                            .text_color(rgb(0x888888))
                            .text_sm()
                            .child(format!("速度: {:.1}x", anim_speed)),
                    )
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .bg(if anim_paused {
                                rgb(0x4a0000)
                            } else {
                                rgb(0x1a4a1a)
                            })
                            .text_color(rgb(0xcccccc))
                            .text_sm()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, {
                                let state = state.clone();
                                move |_: &MouseDownEvent, _, _| {
                                    let mut s = state.lock().unwrap();
                                    s.pending_anim_pause = Some(!s.anim_paused);
                                }
                            })
                            .child(if anim_paused {
                                "▶ 播放"
                            } else {
                                "⏸ 暂停"
                            }),
                    )
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .bg(if show_skeleton {
                                rgb(0x1a4a1a)
                            } else {
                                rgb(0x4a0000)
                            })
                            .text_color(rgb(0xcccccc))
                            .text_sm()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, {
                                let state = state.clone();
                                move |_: &MouseDownEvent, _, _| {
                                    let mut s = state.lock().unwrap();
                                    s.show_skeleton = !s.show_skeleton;
                                }
                            })
                            .child(if show_skeleton {
                                "隐藏骨架"
                            } else {
                                "显示骨架"
                            }),
                    )
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .bg(if show_skin {
                                rgb(0x1a4a1a)
                            } else {
                                rgb(0x4a0000)
                            })
                            .text_color(rgb(0xcccccc))
                            .text_sm()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, {
                                let state = state.clone();
                                move |_: &MouseDownEvent, _, _| {
                                    let mut s = state.lock().unwrap();
                                    s.show_skin = !s.show_skin;
                                }
                            })
                            .child(if show_skin {
                                "隐藏皮肤"
                            } else {
                                "显示皮肤"
                            }),
                    )
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .bg(if auto_rotate {
                                rgb(0x1a4a1a)
                            } else {
                                rgb(0x4a0000)
                            })
                            .text_color(rgb(0xcccccc))
                            .text_sm()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, {
                                let state = state.clone();
                                move |_: &MouseDownEvent, _, _| {
                                    let mut s = state.lock().unwrap();
                                    s.auto_rotate = !s.auto_rotate;
                                }
                            })
                            .child(if auto_rotate {
                                "自动旋转"
                            } else {
                                "停止旋转"
                            }),
                    )
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .bg(if msaa_enabled {
                                rgb(0x1a4a1a)
                            } else {
                                rgb(0x4a0000)
                            })
                            .text_color(rgb(0xcccccc))
                            .text_sm()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, {
                                let state = state.clone();
                                move |_: &MouseDownEvent, _, _| {
                                    let mut s = state.lock().unwrap();
                                    s.msaa_enabled = !s.msaa_enabled;
                                }
                            })
                            .child(if msaa_enabled {
                                "MSAA 4x"
                            } else {
                                "MSAA 关闭"
                            }),
                    ),
            )
            .child(
                div()
                    .py(px(4.0))
                    .px(px(8.0))
                    .flex()
                    .gap(px(16.0))
                    .items_center()
                    .child(div().text_color(rgb(0x999999)).text_xs().child(
                        "拖拽旋转 | 滚轮缩放 |
                         WASD/方向键移动 | Q/E 上下 | 切换动画",
                    ))
                    .child(div().text_color(rgb(0x999999)).text_xs().child(format!(
                        "视角 ({:.0}°, {:.0}°) | 距离 {:.1}",
                        ox.to_degrees(),
                        oy.to_degrees(),
                        dist
                    ))),
            )
            .child(
                div()
                    .py(px(2.0))
                    .px(px(8.0))
                    .flex()
                    .gap(px(8.0))
                    .items_center()
                    .child(div().text_color(rgb(0x999999)).text_xs().child(
                        if let Some(sel) = selected_joint {
                            format!("选中关节 #{} (共 {} 个) | 滚轮旋转关节", sel, joint_count)
                        } else {
                            "点击骨骼节点可选中".to_string()
                        },
                    ))
                    .child(
                        div()
                            .text_color(rgb(0x999999))
                            .text_xs()
                            .child(format!("屏幕关节数: {}", joints_info.len())),
                    ),
            )
            .child(
                div()
                    .py(px(4.0))
                    .px(px(8.0))
                    .flex()
                    .gap(px(8.0))
                    .items_center()
                    .child(
                        div()
                            .px(px(8.0))
                            .py(px(3.0))
                            .bg(rgb(0x8d6e63))
                            .text_color(rgb(0xffffff))
                            .text_sm()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, {
                                let state = state.clone();
                                move |_: &MouseDownEvent, _, _| {
                                    state.lock().unwrap().pending_clear = true;
                                }
                            })
                            .child("清除关节"),
                    ),
            )
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut model_path: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--model" | "-m" => {
                i += 1;
                model_path = args.get(i).cloned();
            }
            _ => {}
        }
        i += 1;
    }

    let shared = Arc::new(Mutex::new(SharedState::new()));
    let render_shared = shared.clone();

    std::thread::spawn(move || {
        let mut ctx: Scenix3D = smol::block_on(async {
            Scenix3D::new(RENDER_W, RENDER_H)
                .await
                .expect("创建 3D 上下文失败")
        });

        ctx.set_clear_color(0.1, 0.1, 0.18, 1.0);

        let mut loaded_model: Option<SceneGraph> = None;
        let mut info: String = "未加载模型".into();
        let mut anim_names: Vec<String> = Vec::new();

        if let Some(ref path) = model_path {
            let (model, model_info, names) = try_load_model(&mut ctx, path);
            loaded_model = model;
            info = model_info;
            anim_names.clone_from(&names);
        }

        {
            let mut s = render_shared.lock().unwrap();
            s.model_info = info;
            s.anim_names = anim_names.clone();
            s.joint_count = ctx.joint_count();
            s.skin_count = ctx.skinned_mesh_count();
        }

        let mut frame_times: Vec<f32> = Vec::with_capacity(30);
        let mut last_time = Instant::now();

        loop {
            let frame_start = Instant::now();
            let dt = frame_start.duration_since(last_time).as_secs_f32();
            last_time = frame_start;

            let new_path = {
                let mut s = render_shared.lock().unwrap();
                s.pending_model_path.take()
            };
            if let Some(path) = new_path {
                let (model, model_info, names) = try_load_model(&mut ctx, &path);
                loaded_model = model;
                anim_names.clone_from(&names);
                let mut s = render_shared.lock().unwrap();
                s.model_info = model_info;
                s.anim_names = names;
                s.orbit_x = 0.0;
                s.orbit_y = 0.2;
                s.distance = 15.0;
                s.joint_count = ctx.joint_count();
                s.skin_count = ctx.skinned_mesh_count();
                s.selected_joint = None;
                s.joints_info.clear();
            }

            let (pending, anim_switch, do_pause, msaa_enabled, show_skeleton, show_skin) = {
                let mut s = render_shared.lock().unwrap();
                let result = s.pending_override.take();
                if s.pending_clear {
                    ctx.clear_joint_overrides();
                    s.pending_clear = false;
                }
                let anim_switch = s.pending_anim_switch.take();
                let do_pause = s.pending_anim_pause.take();
                let msaa = s.msaa_enabled;
                let show = s.show_skeleton;
                let skin = s.show_skin;
                (result, anim_switch, do_pause, msaa, show, skin)
            };
            ctx.set_msaa_enabled(msaa_enabled);
            if let Some((idx, q)) = pending {
                ctx.set_joint_rotation_override(idx, scenekit::Quat::new(q[0], q[1], q[2], q[3]));
            }
            if let Some(pause) = do_pause {
                ctx.set_animation_paused(pause);
            }
            if let Some(dir) = anim_switch {
                let count = ctx.animation_names().len();
                if count > 0 {
                    let new_idx = (ctx.active_animation_index() as isize + dir)
                        .rem_euclid(count as isize) as usize;
                    ctx.set_active_animation(new_idx);
                }
            }

            let movement = {
                let s = render_shared.lock().unwrap();
                (
                    s.move_forward,
                    s.move_backward,
                    s.move_left,
                    s.move_right,
                    s.move_up,
                    s.move_down,
                    s.auto_rotate,
                )
            };
            let move_speed = 2.0 * dt;
            {
                let mut s = render_shared.lock().unwrap();
                if movement.0 {
                    s.orbit_x += move_speed * 0.5;
                }
                if movement.1 {
                    s.orbit_x -= move_speed * 0.5;
                }
                if movement.2 {
                    s.orbit_y += move_speed * 0.3;
                }
                if movement.3 {
                    s.orbit_y -= move_speed * 0.3;
                }
                if movement.4 {
                    s.distance -= move_speed * 0.5;
                }
                if movement.5 {
                    s.distance += move_speed * 0.5;
                }
                s.orbit_y = s.orbit_y.clamp(-1.5, 1.5);
                s.distance = s.distance.clamp(1.0, 50.0);
                if movement.6 {
                    s.orbit_x += 0.5 * dt;
                }
            }

            let (ox, oy, dist) = {
                let s = render_shared.lock().unwrap();
                (s.orbit_x, s.orbit_y, s.distance)
            };

            let cam_pos = Vec3::new(
                dist * ox.sin() * oy.cos(),
                dist * oy.sin(),
                dist * ox.cos() * oy.cos(),
            );
            let camera =
                PerspectiveCamera::new(45.0, RENDER_W as f32 / RENDER_H as f32, 0.1, 100.0)
                    .position(cam_pos)
                    .target(Vec3::ZERO);

            if let Some(ref mut scene) = loaded_model {
                let mesh_ids: Vec<scenekit::NodeId> = scene
                    .iter_depth_first()
                    .filter(|&id| {
                        scene
                            .get(id)
                            .map_or(false, |n| matches!(n.kind, scenekit::NodeKind::Mesh { .. }))
                    })
                    .collect();
                for id in mesh_ids {
                    if let Some(node) = scene.get_mut(id) {
                        node.visible = show_skin;
                    }
                }

                ctx.advance_animation(dt);
                let result = ctx.render(scene, &camera).expect("渲染失败");

                let vp = camera.view_projection();
                let num_joints = ctx.joint_count();
                let mut joints_info: Vec<JointScreenInfo> = Vec::new();
                let mut pixel_data = result.data;

                if show_skeleton {
                    for i in 0..num_joints {
                        if let Some(world_pos) = ctx.joint_world_position(i) {
                            if let Some((sx, sy, depth)) =
                                project_to_screen(world_pos, vp, RENDER_W, RENDER_H)
                            {
                                let base_radius = 6.0;
                                let radius = base_radius / (1.0 + depth.abs() * 0.2);
                                joints_info.push(JointScreenInfo {
                                    index: i,
                                    screen_x: sx,
                                    screen_y: sy,
                                });

                                let is_selected = {
                                    let s = render_shared.lock().unwrap();
                                    s.selected_joint == Some(i)
                                };
                                let col = if is_selected {
                                    (255, 50, 50)
                                } else {
                                    (50, 180, 255)
                                };
                                draw_circle(
                                    &mut pixel_data,
                                    RENDER_W,
                                    RENDER_H,
                                    sx,
                                    sy,
                                    radius,
                                    col.0,
                                    col.1,
                                    col.2,
                                );

                                if let Some(parent_idx) = ctx.joint_parent(i) {
                                    if parent_idx < num_joints {
                                        if let Some(parent_pos) =
                                            ctx.joint_world_position(parent_idx)
                                        {
                                            if let Some((px, py, _)) = project_to_screen(
                                                parent_pos, vp, RENDER_W, RENDER_H,
                                            ) {
                                                draw_line(
                                                    &mut pixel_data,
                                                    RENDER_W,
                                                    RENDER_H,
                                                    sx,
                                                    sy,
                                                    px,
                                                    py,
                                                    180,
                                                    180,
                                                    180,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                let render_image = {
                    let img_buffer = image::RgbaImage::from_raw(RENDER_W, RENDER_H, pixel_data)
                        .expect("无效的像素数据尺寸");
                    let frame = image::Frame::new(img_buffer);
                    Arc::new(RenderImage::new(smallvec::smallvec![frame]))
                };

                {
                    let mut s = render_shared.lock().unwrap();
                    s.render_image = Some(render_image);
                    s.anim_time = ctx.animation_time();
                    s.anim_duration = ctx.animation_duration();
                    s.anim_speed = ctx.animation_speed();
                    s.anim_paused = ctx.is_animation_paused();
                    s.current_anim = ctx.active_animation_index();
                    s.joints_info = joints_info;
                }
            }

            let elapsed = frame_start.elapsed().as_secs_f32();
            frame_times.push(elapsed);
            if frame_times.len() > 30 {
                frame_times.remove(0);
            }
            let avg_fps = frame_times.len() as f32 / frame_times.iter().sum::<f32>();
            {
                let mut s = render_shared.lock().unwrap();
                s.fps = avg_fps;
            }

            let frame_time = frame_start.elapsed();
            let target = Duration::from_millis(33);
            if frame_time < target {
                std::thread::sleep(target - frame_time);
            }
        }
    });

    application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(900.0), px(830.0)), cx);
        let state = shared;

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("3D 模型查看器".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            move |_, cx| {
                let s = state;
                cx.new(move |cx| {
                    let focus_handle = cx.focus_handle();
                    cx.spawn(async move |this, cx| {
                        loop {
                            smol::Timer::after(Duration::from_millis(33)).await;
                            let _ = this.update(cx, |_, cx| cx.notify());
                        }
                    })
                    .detach();
                    Viewer3D {
                        state: s,
                        focus_handle,
                    }
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
