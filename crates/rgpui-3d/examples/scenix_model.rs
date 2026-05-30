//! rgpui + scenix 3D 模型查看器（带自动旋转）
//!
//! 从文件加载 glTF/GLB 3D 模型，支持鼠标拖拽轨道相机、滚轮缩放和自动旋转。
//!
//! 用法:
//!   cargo run --example scenix_model                          # 加载默认 cat.glb
//!   cargo run --example scenix_model -- --model 模型.glb      # 加载指定模型
//!   cargo run --example scenix_model -- --auto-rotate         # 自动旋转

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rgpui::{
    App, Bounds, Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render,
    RenderImage, ScrollDelta, ScrollWheelEvent, TitlebarOptions, Window,
    WindowBackgroundAppearance, WindowBounds, WindowOptions, div, img, prelude::*, px, rgb, size,
};
use rgpui_3d::Scenix3D;
use rgpui_3d::scenix::{self, PerspectiveCamera, SceneGraph, Vec3};
use rgpui_platform::application;

/// 渲染尺寸
const RENDER_W: u32 = 800;
const RENDER_H: u32 = 600;

/// UI 与渲染线程共享的状态
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
    auto_rotate: bool,
}

impl SharedState {
    fn new() -> Self {
        Self {
            orbit_x: 0.0,
            orbit_y: 0.4,
            distance: 4.0,
            is_dragging: false,
            drag_start_x: 0.0,
            drag_start_y: 0.0,
            render_image: None,
            fps: 0.0,
            model_info: String::new(),
            auto_rotate: false,
        }
    }
}

struct ModelView {
    state: Arc<Mutex<SharedState>>,
}

impl Render for ModelView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.state.lock().unwrap();
        let fps = s.fps;
        let info = s.model_info.clone();
        let ox = s.orbit_x;
        let oy = s.orbit_y;
        let dist = s.distance;
        let rotating = s.auto_rotate;

        let img_elem = match &s.render_image {
            Some(img_ref) => div()
                .size(px(RENDER_W as f32))
                .size(px(RENDER_H as f32))
                .child(img(img_ref.clone())),
            None => div()
                .size(px(RENDER_W as f32))
                .size(px(RENDER_H as f32))
                .bg(rgb(0x0f0f23))
                .flex()
                .items_center()
                .justify_center()
                .text_xl()
                .text_color(rgb(0xffffff))
                .child("加载中..."),
        };
        drop(s);

        let state = self.state.clone();

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.0))
            .bg(rgb(0x0f0f23))
            .size(px(900.0))
            .size(px(720.0))
            .child(
                div().py(px(8.0)).child(
                    div()
                        .text_2xl()
                        .text_color(rgb(0xffffff))
                        .child("rgpui + scenix 模型查看器"),
                ),
            )
            .child(
                div()
                    .cursor_grab()
                    .on_mouse_down(MouseButton::Left, {
                        let state = state.clone();
                        move |ev: &MouseDownEvent, _, _| {
                            let mut s = state.lock().unwrap();
                            s.is_dragging = true;
                            s.drag_start_x = ev.position.x.as_f32();
                            s.drag_start_y = ev.position.y.as_f32();
                        }
                    })
                    .on_mouse_up(MouseButton::Left, {
                        let state = state.clone();
                        move |_: &MouseUpEvent, _, _| {
                            state.lock().unwrap().is_dragging = false;
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
                        let state = state;
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
                    .child(img_elem),
            )
            .child(
                div()
                    .py(px(4.0))
                    .flex()
                    .gap(px(16.0))
                    .child(
                        div()
                            .text_color(rgb(0xaaaaaa))
                            .child(format!("{:.0} FPS", fps)),
                    )
                    .child(div().text_color(rgb(0x888888)).child(info))
                    .child(div().text_color(rgb(0x666666)).child(format!(
                        "角度 ({:.0}°, {:.0}°) | 距离 {:.1}",
                        ox.to_degrees(),
                        oy.to_degrees(),
                        dist
                    )))
                    .child(
                        div()
                            .text_color(if rotating {
                                rgb(0x4fc3f7)
                            } else {
                                rgb(0x666666)
                            })
                            .child(if rotating {
                                "自动旋转 ON"
                            } else {
                                "自动旋转 OFF"
                            }),
                    ),
            )
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut model_path: Option<String> = None;
    let mut auto_rotate = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--model" | "-m" => {
                i += 1;
                model_path = args.get(i).cloned();
            }
            "--auto-rotate" | "-r" => {
                auto_rotate = true;
            }
            _ => {}
        }
        i += 1;
    }

    if model_path.is_none() {
        let default_path = format!("{}/examples/3d/1.glb", env!("CARGO_MANIFEST_DIR"));
        if std::path::Path::new(&default_path).exists() {
            model_path = Some(default_path);
        }
    }

    let shared = Arc::new(Mutex::new(SharedState {
        auto_rotate,
        ..SharedState::new()
    }));

    // 后台渲染线程
    let render_shared = shared.clone();

    std::thread::spawn(move || {
        let mut ctx: Scenix3D = smol::block_on(async {
            Scenix3D::new(RENDER_W, RENDER_H)
                .await
                .expect("创建 3D 上下文失败")
        });

        let mut loaded_model: Option<SceneGraph> = None;
        let info: String;

        if let Some(ref path) = model_path {
            let loader = scenix::GltfLoader::new();
            match loader.load_file(path) {
                Ok(asset) => match ctx.register_gltf_asset(&asset) {
                    Ok(_) => {
                        let name = std::path::Path::new(path)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy();
                        info = format!(
                            "{} ({} 网格, {} 材质)",
                            name,
                            asset.meshes.len(),
                            asset.materials.len()
                        );
                        loaded_model = Some(asset.scene);
                    }
                    Err(e) => {
                        info = format!("GPU 注册失败: {e}");
                    }
                },
                Err(e) => {
                    info = format!("加载失败: {e}");
                }
            }
        } else {
            info = "未指定模型文件（使用 --model <路径>）".into();
        };

        {
            let mut s = render_shared.lock().unwrap();
            s.model_info = info;
        }

        let mut frame_times: Vec<f32> = Vec::with_capacity(30);

        loop {
            let frame_start = Instant::now();

            let (mut ox, oy, dist) = {
                let s = render_shared.lock().unwrap();
                (s.orbit_x, s.orbit_y, s.distance)
            };

            let auto_rotate = {
                let s = render_shared.lock().unwrap();
                s.auto_rotate
            };

            if auto_rotate {
                ox += 0.015;
                {
                    let mut s = render_shared.lock().unwrap();
                    s.orbit_x = ox;
                }
            }

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
                let result = ctx.render(scene, &camera).expect("渲染失败");
                let render_image = Arc::new(result.into_render_image());
                {
                    let mut s = render_shared.lock().unwrap();
                    s.render_image = Some(render_image);
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
            let target = Duration::from_millis(50);
            if frame_time < target {
                std::thread::sleep(target - frame_time);
            }
        }
    });

    application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(900.0), px(720.0)), cx);
        let state = shared;

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_background: WindowBackgroundAppearance::Transparent,
                titlebar: Some(TitlebarOptions {
                    title: Some("rgpui + scenix 模型查看器".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            move |_, cx| {
                let s = state;
                cx.new(move |cx| {
                    cx.spawn(async move |this, cx| {
                        loop {
                            smol::Timer::after(Duration::from_millis(50)).await;
                            let _ = this.update(cx, |_, cx| cx.notify());
                        }
                    })
                    .detach();
                    ModelView { state: s }
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
