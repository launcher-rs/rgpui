//! rgpui-3d 3D 动画测试演示。
//!
//! 加载 glTF 模型，为每个动画生成一个按钮，点击播放对应动画。
//! 支持鼠标拖拽旋转轨道控制和滚轮缩放。
//!
//! 运行：
//! ```text
//! cargo run --example desktop_pet_3d
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render,
    ScrollDelta, ScrollWheelEvent, SharedString, Window, WindowBackgroundAppearance, WindowBounds,
    WindowKind, WindowOptions, div, img, prelude::*, px, rgb, rgba, size,
};
use rgpui_3d::Scenix3D;
use rgpui_3d::scenix::{self, PerspectiveCamera, SceneGraph, Vec3};
use rgpui_platform::application;
use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 覆盖窗口尺寸
const WINDOW_WIDTH: f32 = 360.0;
const WINDOW_HEIGHT: f32 = 520.0;

/// 3D 渲染尺寸
const RENDER_W: u32 = 320;
const RENDER_H: u32 = 320;

/// 帧率
const FRAME_TIME: Duration = Duration::from_millis(16);

// ============================================================================
// 共享状态
// ============================================================================

struct SharedState {
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
    /// 请求切换的动画索引
    requested_anim_index: Option<usize>,
    /// 当前动画索引
    current_anim_index: Option<usize>,
    /// 动画名称列表
    anim_names: Vec<String>,
    /// 鼠标拖拽轨道控制
    is_dragging: bool,
    drag_start_x: f32,
    drag_start_y: f32,
    /// 帧率统计
    fps: f32,
}

impl SharedState {
    fn new() -> Self {
        Self {
            ctx_3d: None,
            scene_3d: None,
            render_image: None,
            orbit_x: 0.0,
            orbit_y: -0.3,
            distance: 5.0,
            requested_anim_index: None,
            current_anim_index: None,
            anim_names: Vec::new(),
            is_dragging: false,
            drag_start_x: 0.0,
            drag_start_y: 0.0,
            fps: 0.0,
        }
    }
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

        // 加载 3D 模型
        let loader = scenix::GltfLoader::new();
        let scene = match loader.load_file(&model_path) {
            Ok(asset) => {
                if let Err(e) = ctx.register_gltf_asset(&asset) {
                    eprintln!("[3D] GPU 注册失败: {e}");
                } else {
                    match ctx.load_gltf_skins(&model_path, &asset) {
                        Ok(_) => {
                            let names = ctx.animation_names();
                            eprintln!("[3D] 蒙皮加载成功，共 {} 个动画: {:?}", names.len(), names);
                            // 将动画名称存入共享状态
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

        {
            let mut s = state.lock().unwrap();
            s.ctx_3d = Some(ctx);
            s.scene_3d = Some(scene);
        }

        let anim_speed = 1.0;
        let mut frame_times: Vec<f32> = Vec::with_capacity(30);

        loop {
            let frame_start = Instant::now();

            // 读取状态
            let (orbit_x, orbit_y, distance) = {
                let s = state.lock().unwrap();
                (s.orbit_x, s.orbit_y, s.distance)
            };

            // 取出 ctx 和 scene 进行渲染
            let mut ctx_scene = {
                let mut s = state.lock().unwrap();
                (s.ctx_3d.take(), s.scene_3d.take())
            };

            if let (Some(ctx), Some(scene)) = &mut ctx_scene {
                // 切换动画
                let anim_idx = {
                    let mut s = state.lock().unwrap();
                    s.requested_anim_index.take()
                };
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

                // 构建相机
                let camera =
                    PerspectiveCamera::new(45.0, RENDER_W as f32 / RENDER_H as f32, 0.1, 100.0)
                        .position(Vec3::new(
                            distance * orbit_x.sin() * orbit_y.cos(),
                            distance * orbit_y.sin() + 0.5,
                            distance * orbit_x.cos() * orbit_y.cos(),
                        ))
                        .target(Vec3::new(0.0, -0.2, 0.0));

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
        let state = self.state.lock().unwrap();
        let fps = state.fps;
        let anim_names = state.anim_names.clone();
        let current_anim = state.current_anim_index;
        let orbit_state = self.state.clone();
        drop(state);

        // 3D 渲染图像区域（支持拖拽旋转）
        let img_elem = match &self.state.lock().unwrap().render_image {
            Some(img_ref) => div()
                .size(px(RENDER_W as f32))
                .size(px(RENDER_H as f32))
                .cursor_grab()
                .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, _, _| {
                    let mut s = orbit_state.lock().unwrap();
                    s.is_dragging = true;
                    s.drag_start_x = ev.position.x.as_f32();
                    s.drag_start_y = ev.position.y.as_f32();
                })
                .on_mouse_up(MouseButton::Left, {
                    let state = self.state.clone();
                    move |_: &MouseUpEvent, _, _| {
                        state.lock().unwrap().is_dragging = false;
                    }
                })
                .on_mouse_move({
                    let state = self.state.clone();
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
                    let state = self.state.clone();
                    move |ev: &ScrollWheelEvent, _, _| {
                        let mut s = state.lock().unwrap();
                        let delta = match ev.delta {
                            ScrollDelta::Pixels(d) => d.y.as_f32(),
                            ScrollDelta::Lines(d) => d.y * 20.0,
                        };
                        s.distance -= delta * 0.05;
                        s.distance = s.distance.clamp(1.0, 50.0);
                    }
                })
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
        };

        // 动画按钮列表
        let buttons = div()
            .flex()
            .flex_col()
            .gap_1()
            .mt_2()
            .w(px(RENDER_W as f32));

        let buttons = if anim_names.is_empty() {
            buttons.child(
                div()
                    .text_xs()
                    .text_color(rgba(0xffffffaa))
                    .child("暂无动画"),
            )
        } else {
            let mut btns = buttons;
            for (i, name) in anim_names.iter().enumerate() {
                let is_active = current_anim == Some(i);
                let state = self.state.clone();
                let label = format!("{} - {}", i, name);
                let btn = div()
                    .id(format!("anim-{}", i))
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .text_xs()
                    .cursor_pointer()
                    .child(label)
                    .on_click(move |_, _, _| {
                        state.lock().unwrap().requested_anim_index = Some(i);
                    });
                let btn = if is_active {
                    btn.bg(rgba(0x4488ffcc)).text_color(rgb(0xffffff))
                } else {
                    btn.bg(rgba(0xffffffdd)).text_color(rgb(0x333333))
                };
                btns = btns.child(btn);
            }
            btns
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .items_center()
            .child(img_elem)
            .child(
                div()
                    .mt_1()
                    .text_xs()
                    .text_color(rgba(0xffffffaa))
                    .child(format!("{:.0} FPS | 拖拽旋转，滚轮缩放", fps)),
            )
            .child(buttons)
    }
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

            let bounds = Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx);
            let state = Arc::new(Mutex::new(SharedState::new()));
            let view_state = state.clone();

            cx.open_window(
                WindowOptions {
                    titlebar: None,
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_background: WindowBackgroundAppearance::Transparent,
                    kind: WindowKind::Overlay,
                    ..Default::default()
                },
                |_window, cx| {
                    let view = cx.new(|_| View::new(view_state));

                    // 启动 3D 渲染线程
                    let model_path = format!(
                        "{}/../../crates/rgpui-3d/examples/3d/1.glb",
                        env!("CARGO_MANIFEST_DIR")
                    );
                    spawn_3d_render_thread(state, model_path);

                    view
                },
            )
            .expect("打开窗口失败");

            cx.activate(true);
        });
}
