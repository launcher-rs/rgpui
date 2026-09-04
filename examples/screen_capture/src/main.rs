#![cfg_attr(target_family = "wasm", no_main)]

use image::RgbaImage;
use rgpui::{
    App, Bounds, Context, FontWeight, IntoElement, Render, ScreenCaptureFrame, ScreenCaptureSource,
    SharedString, SourceMetadata, Task, Window, WindowBounds, WindowOptions, div, img, prelude::*,
    px, rgb, size,
};
use rgpui_platform::application;
use std::{
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

/// 预览帧数据（从捕获线程传到 UI 线程）
struct PreviewFrame {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

/// 屏幕捕获示例
struct ScreenCaptureExample {
    supported: bool,
    sources: Vec<Rc<dyn ScreenCaptureSource>>,
    source_meta: Vec<SourceMetadata>,
    selected: Option<usize>,

    state: CaptureState,
    status: SharedString,
    frame_count: u64,
    fps: f64,

    /// 预览帧共享缓冲区（捕获线程写入，UI 线程读取）
    preview_buf: Arc<Mutex<Option<PreviewFrame>>>,
    preview_path: Option<PathBuf>,
    new_frame: Arc<AtomicBool>,
    _preview_task: Task<()>,

    /// 截屏结果路径
    screenshot_path: Option<PathBuf>,

    /// 录屏共享状态（帧回调使用）
    recording_flag: Arc<AtomicBool>,
    recording_dir_shared: Arc<Mutex<Option<PathBuf>>>,
    recording: bool,
    recorded_frames: u64,

    /// GIF 生成设置
    gif_delay_ms: u32,

    /// 已生成的 GIF 列表
    recorded_gifs: Vec<PathBuf>,

    /// 后台任务
    _poll_task: Option<Task<()>>,
    _capture_stream: Option<Box<dyn rgpui::ScreenCaptureStream>>,
    _merge_task: Option<Task<()>>,
}

enum CaptureState {
    Idle,
    Starting,
    Active,
    Failed(String),
}

impl ScreenCaptureExample {
    fn new(window: &mut Window, cx: &mut Context<Self>, supported: bool) -> Self {
        let preview_buf: Arc<Mutex<Option<PreviewFrame>>> = Arc::new(Mutex::new(None));
        let new_frame = Arc::new(AtomicBool::new(false));

        let preview_buf_clone = preview_buf.clone();
        let new_frame_clone = new_frame.clone();
        let preview_task = cx.spawn_in(window, async move |this, cx| {
            let temp_root = std::env::temp_dir().join("rgpui-screen-capture");
            let _ = std::fs::create_dir_all(&temp_root);
            let preview_file = temp_root.join("preview.png");

            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(66))
                    .await;

                if new_frame_clone.swap(false, Ordering::Relaxed)
                    && let Ok(mut buf) = preview_buf_clone.lock()
                    && let Some(frame) = buf.take()
                    && let Some(img) = RgbaImage::from_raw(frame.width, frame.height, frame.data)
                {
                    let _ = img.save(&preview_file);
                    this.update(cx, |view, _cx| {
                        view.preview_path = Some(preview_file.clone());
                        view.frame_count += 1;
                    })
                    .ok();
                }
            }
        });

        Self {
            supported,
            sources: Vec::new(),
            source_meta: Vec::new(),
            selected: None,
            state: CaptureState::Idle,
            status: SharedString::from("ready"),
            frame_count: 0,
            fps: 0.0,
            preview_buf,
            preview_path: None,
            new_frame,
            _preview_task: preview_task,
            screenshot_path: None,
            recording_flag: Arc::new(AtomicBool::new(false)),
            recording_dir_shared: Arc::new(Mutex::new(None)),
            recording: false,
            recorded_frames: 0,
            gif_delay_ms: 100,
            recorded_gifs: Vec::new(),
            _poll_task: None,
            _capture_stream: None,
            _merge_task: None,
        }
    }

    fn refresh_sources(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sources.clear();
        self.source_meta.clear();
        self.selected = None;
        self.state = CaptureState::Idle;
        self.fetch_sources(window, cx);
    }

    fn fetch_sources(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.status = SharedString::from("Enumerating capture sources...");
        let sources_rx = cx.screen_capture_sources();
        self._poll_task = Some(
            cx.spawn_in(window, async move |this, cx| match sources_rx.await {
                Ok(Ok(sources)) => {
                    let meta: Vec<SourceMetadata> =
                        sources.iter().filter_map(|s| s.metadata().ok()).collect();
                    this.update(cx, |view, _cx| {
                        view.sources = sources;
                        view.source_meta = meta;
                        view.state = CaptureState::Idle;
                        view.status = SharedString::from(format!(
                            "Found {} capture sources",
                            view.source_meta.len()
                        ));
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |view, _cx| {
                        view.state = CaptureState::Failed(format!("Enumerate failed: {}", e));
                        view.status = SharedString::from(format!("Enumerate failed: {}", e));
                    })
                    .ok();
                }
                Err(_) => {
                    this.update(cx, |view, _cx| {
                        view.status = SharedString::from("Enumerate cancelled");
                    })
                    .ok();
                }
            }),
        );
    }

    fn start_capture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let idx = match self.selected {
            Some(i) => i,
            None => return,
        };
        let source = match self.sources.get(idx) {
            Some(s) => s.clone(),
            None => return,
        };

        self.state = CaptureState::Starting;
        self.status = SharedString::from("Starting capture...");
        self.frame_count = 0;
        self.screenshot_path = None;
        self.preview_path = None;

        let preview_buf = self.preview_buf.clone();
        let new_frame = self.new_frame.clone();
        let fps_counter = Arc::new(Mutex::new(FpsCounter::new()));
        let recording_flag = self.recording_flag.clone();
        let recording_dir = self.recording_dir_shared.clone();

        let frame_callback: Box<dyn Fn(ScreenCaptureFrame) + Send> = {
            let preview_buf = preview_buf.clone();
            let new_frame = new_frame.clone();
            let fps_counter = fps_counter.clone();
            let recording_flag = recording_flag.clone();
            let recording_dir = recording_dir.clone();
            let frame_counter = Arc::new(AtomicU64::new(0));

            Box::new(move |frame| {
                let seq = frame_counter.fetch_add(1, Ordering::Relaxed) + 1;
                fps_counter.lock().unwrap().tick();

                if let Some(rgba) = frame.to_rgba() {
                    let (w, h) = (rgba.width(), rgba.height());
                    let data = rgba.clone().into_raw();

                    if let Ok(mut buf) = preview_buf.lock() {
                        *buf = Some(PreviewFrame {
                            data,
                            width: w,
                            height: h,
                        });
                    }
                    new_frame.store(true, Ordering::Relaxed);

                    if recording_flag.load(Ordering::Relaxed)
                        && let Ok(guard) = recording_dir.lock()
                        && let Some(ref dir) = *guard
                    {
                        let path = dir.join(format!("frame_{:06}.png", seq));
                        let _ = rgba.save(&path);
                    }
                }
            })
        };

        let foreground = cx.foreground_executor().clone();
        let stream_rx = source.stream(&foreground, frame_callback);

        let shared_count = Arc::new(AtomicU64::new(0));
        let fps_shared = fps_counter.clone();
        let poll = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(250))
                    .await;

                let count = shared_count.load(Ordering::Relaxed);
                let fps = fps_shared.lock().unwrap().fps();
                let ok = this.update(cx, |view, _cx| {
                    view.frame_count = count;
                    view.fps = fps;
                });
                if ok.is_err() {
                    break;
                }
            }
        });

        cx.spawn_in(window, async move |this, cx| match stream_rx.await {
            Ok(Ok(stream)) => {
                this.update(cx, |view, _cx| {
                    view.state = CaptureState::Active;
                    view._capture_stream = Some(stream);
                    view._poll_task = Some(poll);
                    view.status = SharedString::from("Capturing...");
                })
                .ok();
            }
            Ok(Err(e)) => {
                this.update(cx, |view, _cx| {
                    view.state = CaptureState::Failed(format!("Start failed: {}", e));
                    view.status = SharedString::from(format!("Start failed: {}", e));
                })
                .ok();
            }
            Err(_) => {
                this.update(cx, |view, _cx| {
                    view.state = CaptureState::Failed("Request cancelled".into());
                    view.status = SharedString::from("Request cancelled");
                })
                .ok();
            }
        })
        .detach();
    }

    fn stop_capture(&mut self) {
        self.recording = false;
        self.recording_flag.store(false, Ordering::Relaxed);
        self._capture_stream = None;
        self._poll_task = None;
        self.state = CaptureState::Idle;
        self.status = SharedString::from("Stopped");
    }

    fn take_screenshot(&mut self, cx: &mut Context<Self>) {
        if let Some(ref path) = self.preview_path {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let out = PathBuf::from(format!("screenshot_{}.png", ts));
            if std::fs::copy(path, &out).is_ok() {
                self.screenshot_path = Some(out);
                self.status = SharedString::from(format!(
                    "Screenshot saved: {}",
                    self.screenshot_path.as_ref().unwrap().display()
                ));
            } else {
                self.status = SharedString::from("Screenshot save failed");
            }
            cx.notify();
        }
    }

    fn start_recording(&mut self) {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let dir = PathBuf::from(format!("recording_{}", ts));
        if std::fs::create_dir_all(&dir).is_ok() {
            *self.recording_dir_shared.lock().unwrap() = Some(dir.clone());
            self.recording_flag.store(true, Ordering::Relaxed);
            self.recording = true;
            self.recorded_frames = 0;
            self.status = SharedString::from("Recording...");
        } else {
            self.status = SharedString::from("Failed to create recording directory");
        }
    }

    fn stop_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.recording = false;
        self.recording_flag.store(false, Ordering::Relaxed);
        if let Some(dir) = self.recording_dir_shared.lock().unwrap().take() {
            let actual_count = std::fs::read_dir(&dir)
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("png"))
                        .count()
                })
                .unwrap_or(0);
            self.recorded_frames = actual_count as u64;

            if actual_count == 0 {
                let _ = std::fs::remove_dir(&dir);
                self.status = SharedString::from("No frames recorded (screen may be static)");
                return;
            }

            let delay_ms = self.gif_delay_ms;
            self.status = SharedString::from(format!("Merging {} frames to GIF...", actual_count));
            let task = cx.spawn_in(window, async move |this, cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move { merge_frames_to_gif(&dir, delay_ms) })
                    .await;
                this.update(cx, |view, _cx| match result {
                    Ok(gif_path) => {
                        view.recorded_gifs.push(gif_path.clone());
                        view.status = SharedString::from(format!(
                            "GIF saved: {} ({} frames)",
                            gif_path.display(),
                            actual_count
                        ));
                    }
                    Err(e) => view.status = SharedString::from(format!("Merge failed: {}", e)),
                })
                .ok();
            });
            self._merge_task = Some(task);
        }
    }
}

impl Render for ScreenCaptureExample {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let capturing = matches!(self.state, CaptureState::Active | CaptureState::Starting);
        let idle = matches!(self.state, CaptureState::Idle);
        let failed_msg = match &self.state {
            CaptureState::Failed(msg) => Some(msg.clone()),
            _ => None,
        };

        let source_items: Vec<_> = self
            .source_meta
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let sel = self.selected == Some(i);
                let label = m.label.as_deref().unwrap_or("Unknown").to_string();
                let res = format!("{}x{}", m.resolution.width.0, m.resolution.height.0);
                let main_flag = if m.is_main.unwrap_or(false) {
                    " [MAIN]"
                } else {
                    ""
                };

                div()
                    .id(format!("src-{}", i))
                    .flex()
                    .gap_2()
                    .items_center()
                    .p_1()
                    .rounded_md()
                    .text_sm()
                    .cursor_pointer()
                    .bg(if sel { rgb(0x0078D4) } else { rgb(0xF0F0F0) })
                    .text_color(if sel { rgb(0xFFFFFF) } else { rgb(0x505050) })
                    .on_click(_cx.listener(move |this, _, _, _| this.selected = Some(i)))
                    .child(format!("#{}  {}  {}{}", i + 1, label, res, main_flag))
            })
            .collect::<Vec<_>>();

        div()
            .flex()
            .flex_col()
            .gap_2()
            .w(px(860.))
            .h(px(750.))
            .p_3()
            .text_color(rgb(0x505050))
            // ---- title bar ----
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .child("Screen Capture"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .text_sm()
                            .child(format!("Frames: {}", self.frame_count))
                            .child(format!("FPS: {:.1}", self.fps))
                            .when(self.recording, |el| {
                                el.child(
                                    div()
                                        .text_color(rgb(0xCC0000))
                                        .child(format!("[REC {}]", self.recorded_frames)),
                                )
                            }),
                    ),
            )
            // ---- status ----
            .child(
                div()
                    .text_sm()
                    .text_color(if failed_msg.is_some() {
                        rgb(0xCC0000)
                    } else {
                        rgb(0x808080)
                    })
                    .child(if self.supported {
                        SharedString::from(format!("Status: {}", self.status))
                    } else {
                        SharedString::from(format!(
                            "Status: {} (screen-capture not enabled)",
                            self.status
                        ))
                    }),
            )
            // ---- source list ----
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .child("Capture Sources"),
                            )
                            .child(
                                div()
                                    .id("btn-refresh")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .text_sm()
                                    .cursor_pointer()
                                    .bg(rgb(0xE0E0E0))
                                    .text_color(rgb(0x404040))
                                    .on_click(_cx.listener(|this, _, window, cx| {
                                        this.refresh_sources(window, cx)
                                    }))
                                    .child("Refresh"),
                            ),
                    )
                    .children(source_items),
            )
            // ---- GIF speed control ----
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .child(div().text_color(rgb(0x808080)).child("GIF Speed:"))
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(
                                div()
                                    .id("gif-speed-slow")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .text_sm()
                                    .cursor_pointer()
                                    .bg(if self.gif_delay_ms >= 200 {
                                        rgb(0x0078D4)
                                    } else {
                                        rgb(0xE0E0E0)
                                    })
                                    .text_color(if self.gif_delay_ms >= 200 {
                                        rgb(0xFFFFFF)
                                    } else {
                                        rgb(0x505050)
                                    })
                                    .on_click(_cx.listener(|this, _, _, _| this.gif_delay_ms = 200))
                                    .child("0.5x"),
                            )
                            .child(
                                div()
                                    .id("gif-speed-normal")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .text_sm()
                                    .cursor_pointer()
                                    .bg(if self.gif_delay_ms == 100 {
                                        rgb(0x0078D4)
                                    } else {
                                        rgb(0xE0E0E0)
                                    })
                                    .text_color(if self.gif_delay_ms == 100 {
                                        rgb(0xFFFFFF)
                                    } else {
                                        rgb(0x505050)
                                    })
                                    .on_click(_cx.listener(|this, _, _, _| this.gif_delay_ms = 100))
                                    .child("1x"),
                            )
                            .child(
                                div()
                                    .id("gif-speed-fast")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .text_sm()
                                    .cursor_pointer()
                                    .bg(if self.gif_delay_ms == 50 {
                                        rgb(0x0078D4)
                                    } else {
                                        rgb(0xE0E0E0)
                                    })
                                    .text_color(if self.gif_delay_ms == 50 {
                                        rgb(0xFFFFFF)
                                    } else {
                                        rgb(0x505050)
                                    })
                                    .on_click(_cx.listener(|this, _, _, _| this.gif_delay_ms = 50))
                                    .child("2x"),
                            )
                            .child(
                                div()
                                    .id("gif-speed-turbo")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .text_sm()
                                    .cursor_pointer()
                                    .bg(if self.gif_delay_ms == 25 {
                                        rgb(0x0078D4)
                                    } else {
                                        rgb(0xE0E0E0)
                                    })
                                    .text_color(if self.gif_delay_ms == 25 {
                                        rgb(0xFFFFFF)
                                    } else {
                                        rgb(0x505050)
                                    })
                                    .on_click(_cx.listener(|this, _, _, _| this.gif_delay_ms = 25))
                                    .child("4x"),
                            ),
                    ),
            )
            // ---- controls ----
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .id("btn-start")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .text_sm()
                            .cursor_pointer()
                            .bg(if idle && self.selected.is_some() {
                                rgb(0x0078D4)
                            } else {
                                rgb(0xC0C0C0)
                            })
                            .text_color(rgb(0xFFFFFF))
                            .on_click(
                                _cx.listener(|this, _, window, cx| this.start_capture(window, cx)),
                            )
                            .child("Start Capture (Space)"),
                    )
                    .child(
                        div()
                            .id("btn-stop")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .text_sm()
                            .cursor_pointer()
                            .bg(if capturing {
                                rgb(0xD83B01)
                            } else {
                                rgb(0xC0C0C0)
                            })
                            .text_color(rgb(0xFFFFFF))
                            .on_click(_cx.listener(|this, _, _, _| this.stop_capture()))
                            .child("Stop"),
                    )
                    .child(
                        div()
                            .id("btn-screenshot")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .text_sm()
                            .cursor_pointer()
                            .bg(if capturing && !self.recording {
                                rgb(0x107C10)
                            } else {
                                rgb(0xC0C0C0)
                            })
                            .text_color(rgb(0xFFFFFF))
                            .on_click(_cx.listener(|this, _, _, cx| this.take_screenshot(cx)))
                            .child("Screenshot (S)"),
                    )
                    .child(
                        div()
                            .id("btn-record")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .text_sm()
                            .cursor_pointer()
                            .bg(if capturing && !self.recording {
                                rgb(0xD83B01)
                            } else {
                                rgb(0xC0C0C0)
                            })
                            .text_color(rgb(0xFFFFFF))
                            .on_click(_cx.listener(|this, _, _, _| this.start_recording()))
                            .child("Record (R)"),
                    )
                    .child(
                        div()
                            .id("btn-stop-record")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .text_sm()
                            .cursor_pointer()
                            .bg(if self.recording {
                                rgb(0xD83B01)
                            } else {
                                rgb(0xC0C0C0)
                            })
                            .text_color(rgb(0xFFFFFF))
                            .on_click(
                                _cx.listener(|this, _, window, cx| this.stop_recording(window, cx)),
                            )
                            .child("Stop Recording"),
                    ),
            )
            // ---- preview + screenshot ----
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child({
                        let preview = self.preview_path.clone();
                        div()
                            .flex_1()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .child("Preview"),
                            )
                            .child(
                                div()
                                    .w(px(360.))
                                    .h(px(240.))
                                    .border_1()
                                    .border_color(rgb(0xD0D0D0))
                                    .rounded_md()
                                    .overflow_hidden()
                                    .when_some(preview, |el, p| {
                                        if p.exists() {
                                            el.child(img(p).w(px(360.)).h(px(240.)))
                                        } else {
                                            el.child(
                                                div()
                                                    .p_2()
                                                    .text_sm()
                                                    .text_color(rgb(0xC0C0C0))
                                                    .child("Waiting for frames..."),
                                            )
                                        }
                                    })
                                    .when(self.preview_path.is_none(), |el| {
                                        el.child(
                                            div()
                                                .p_2()
                                                .text_sm()
                                                .text_color(rgb(0xC0C0C0))
                                                .child("Waiting for frames..."),
                                        )
                                    }),
                            )
                    })
                    .when_some(self.screenshot_path.clone(), |el, path| {
                        el.child(
                            div()
                                .flex_1()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .child("Last Screenshot"),
                                )
                                .child(
                                    div()
                                        .w(px(360.))
                                        .h(px(240.))
                                        .border_1()
                                        .border_color(rgb(0xD0D0D0))
                                        .rounded_md()
                                        .overflow_hidden()
                                        .child(img(path).w(px(360.)).h(px(240.))),
                                ),
                        )
                    }),
            )
            // ---- recorded GIFs list ----
            .when(!self.recorded_gifs.is_empty(), |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .mt_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .child("Recorded GIFs"),
                        )
                        .children(self.recorded_gifs.iter().rev().enumerate().map(
                            |(i, gif_path)| {
                                let fname = gif_path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("?")
                                    .to_string();
                                div()
                                    .id(format!("gif-{}", i))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .p_1()
                                    .text_sm()
                                    .child(format!("{}. {}", i + 1, fname))
                                    .child(
                                        div()
                                            .id(format!("gif-open-{}", i))
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .text_sm()
                                            .cursor_pointer()
                                            .bg(rgb(0xE0E0E0))
                                            .text_color(rgb(0x0078D4))
                                            .child("Open"),
                                    )
                            },
                        )),
                )
            })
    }
}

/// FPS counter
struct FpsCounter {
    times: Vec<Instant>,
}

impl FpsCounter {
    fn new() -> Self {
        Self {
            times: Vec::with_capacity(60),
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        self.times.push(now);
        let cutoff = now - std::time::Duration::from_secs(1);
        self.times.retain(|t| *t > cutoff);
    }

    fn fps(&self) -> f64 {
        let len = self.times.len();
        if len < 2 {
            return 0.0;
        }
        let span = self
            .times
            .last()
            .unwrap()
            .duration_since(self.times[0])
            .as_secs_f64();
        if span < 0.001 {
            return len as f64;
        }
        len as f64 / span
    }
}

/// Merge frame PNGs in a directory into a GIF
fn merge_frames_to_gif(dir: &Path, delay_ms: u32) -> Result<PathBuf, String> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read recording dir: {}", e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("png"))
        .collect();

    entries.sort();
    if entries.is_empty() {
        return Err("No frames recorded".into());
    }

    let count = entries.len();
    let out_name = format!(
        "recording_{}.gif",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );
    let out_path = dir.parent().unwrap_or(dir).join(&out_name);

    let file = std::fs::File::create(&out_path).map_err(|e| format!("Create GIF failed: {}", e))?;
    let mut encoder = image::codecs::gif::GifEncoder::new(file);
    encoder
        .set_repeat(image::codecs::gif::Repeat::Infinite)
        .ok();

    let delay = image::Delay::from_numer_denom_ms(delay_ms, 1);
    let frames: Vec<_> = entries
        .iter()
        .filter_map(|p| {
            let img = image::open(p).ok()?;
            Some(image::Frame::from_parts(img.into_rgba8(), 0, 0, delay))
        })
        .collect();

    encoder
        .encode_frames(frames)
        .map_err(|e| format!("GIF encode failed: {}", e))?;

    for entry in &entries {
        let _ = std::fs::remove_file(entry);
    }

    println!(
        "GIF saved: {} ({} frames at {}ms delay)",
        out_path.display(),
        count,
        delay_ms
    );

    Ok(out_path)
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(860.), px(750.)), cx);
        let supported = cx.is_screen_capture_supported();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| ScreenCaptureExample::new(window, cx, supported)),
        )
        .unwrap();
        cx.activate(true);
    });
}

fn main() {
    run_example();
}
