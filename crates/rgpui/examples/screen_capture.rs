//! 屏幕捕获示例
//!
//! 此示例演示如何使用 screen-capture feature 捕获屏幕内容。
//! 功能包括：
//! - 检测屏幕捕获是否受支持
//! - 列出所有可用的屏幕捕获源（显示器）
//! - 点击选择屏幕捕获源
//! - 启动/停止屏幕捕获流
//! - 截屏：捕获当前帧并保存为 PNG 文件
//! - 录屏：持续捕获帧并保存为帧序列 PNG，停止后自动合并为 GIF
//! - 实时显示帧计数和元数据信息
//!
//! 注意：屏幕捕获 API 只在屏幕内容变化时产生新帧，
//! 因此屏幕静止时不会录制到帧。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, Render, SharedString, SourceMetadata, Task, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size,
};
use rgpui_platform::application;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// 屏幕捕获示例视图
struct ScreenCaptureExample {
    /// 屏幕捕获是否受支持
    capture_supported: bool,
    /// 可用的屏幕捕获源列表
    sources: Vec<Rc<dyn rgpui::ScreenCaptureSource>>,
    /// 源的元数据缓存
    source_metadata: Vec<SourceMetadata>,
    /// 当前选中的源索引
    selected_source: Option<usize>,
    /// 是否正在捕获
    is_capturing: bool,
    /// UI 显示的帧数（从原子计数器同步）
    frame_count: u64,
    /// 共享的原子帧计数器（后台线程递增）
    shared_frame_count: Arc<AtomicU64>,
    /// 状态消息
    status: SharedString,
    /// 获取源列表的异步任务句柄
    _fetch_task: Task<()>,
    /// 帧计数同步的定时轮询任务句柄
    _poll_task: Option<Task<()>>,
    /// 捕获流句柄（用于停止捕获）
    _capture_stream: Option<Box<dyn rgpui::ScreenCaptureStream>>,

    // === 截屏相关 ===
    /// 截屏请求标志（帧回调中检查）
    screenshot_requested: Arc<AtomicBool>,
    /// 共享的截屏路径（帧回调写入，render 读取）
    shared_screenshot_path: Arc<Mutex<Option<String>>>,
    /// 上次截屏的文件路径（UI 显示用）
    last_screenshot_path: SharedString,

    // === 录屏相关 ===
    /// 录屏是否激活
    is_recording: bool,
    /// 录屏请求标志（帧回调中检查）
    recording_active: Arc<AtomicBool>,
    /// 已录制帧数（从原子计数器同步）
    recorded_frame_count: u64,
    /// 共享的原子录制帧计数器
    shared_recorded_frame_count: Arc<AtomicU64>,
    /// 共享的录屏保存目录（帧回调和 UI 共享）
    shared_recording_dir: Arc<Mutex<Option<PathBuf>>>,
    /// GIF 合并结果（文件路径或错误信息）
    shared_gif_result: Arc<Mutex<Option<String>>>,
    /// GIF 合并是否正在进行
    is_merging: bool,
    /// 共享的合并中标志
    shared_is_merging: Arc<AtomicBool>,
}

impl ScreenCaptureExample {
    /// 创建新的屏幕捕获示例视图
    fn new(window: &mut Window, cx: &mut Context<Self>, capture_supported: bool) -> Self {
        let mut view = Self {
            capture_supported,
            sources: Vec::new(),
            source_metadata: Vec::new(),
            selected_source: None,
            is_capturing: false,
            frame_count: 0,
            shared_frame_count: Arc::new(AtomicU64::new(0)),
            status: "初始化中...".into(),
            _fetch_task: Task::ready(()),
            _poll_task: None,
            _capture_stream: None,
            screenshot_requested: Arc::new(AtomicBool::new(false)),
            shared_screenshot_path: Arc::new(Mutex::new(None)),
            last_screenshot_path: "无".into(),
            is_recording: false,
            recording_active: Arc::new(AtomicBool::new(false)),
            recorded_frame_count: 0,
            shared_recorded_frame_count: Arc::new(AtomicU64::new(0)),
            shared_recording_dir: Arc::new(Mutex::new(None)),
            shared_gif_result: Arc::new(Mutex::new(None)),
            is_merging: false,
            shared_is_merging: Arc::new(AtomicBool::new(false)),
        };

        if capture_supported {
            let sources_rx = cx.screen_capture_sources();
            view._fetch_task = cx.spawn_in(window, async move |this, cx| match sources_rx.await {
                Ok(Ok(sources)) => {
                    let metadata_list: Vec<SourceMetadata> =
                        sources.iter().filter_map(|s| s.metadata().ok()).collect();
                    let count = metadata_list.len();
                    this.update(cx, |view, _cx| {
                        view.source_metadata = metadata_list;
                        view.sources = sources;
                        view.status = format!("找到 {} 个屏幕捕获源", count).into();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |view, _cx| {
                        view.status = format!("获取屏幕捕获源失败: {}", e).into();
                    })
                    .ok();
                }
                Err(_) => {
                    this.update(cx, |view, _cx| {
                        view.status = "屏幕捕获源请求被取消".into();
                    })
                    .ok();
                }
            });
        } else {
            view.status = "屏幕捕获不受支持，请启用 screen-capture feature".into();
        }

        view
    }

    /// 启动帧计数同步的定时轮询
    fn start_polling(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shared_count = self.shared_frame_count.clone();
        let shared_recorded_count = self.shared_recorded_frame_count.clone();
        let shared_merging = self.shared_is_merging.clone();
        self._poll_task = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;

                let count = shared_count.load(Ordering::Relaxed);
                let recorded = shared_recorded_count.load(Ordering::Relaxed);
                let merging = shared_merging.load(Ordering::Relaxed);
                this.update(cx, |view, _cx| {
                    view.frame_count = count;
                    view.recorded_frame_count = recorded;
                    view.is_merging = merging;
                })
                .ok();
            }
        }));
    }

    /// 从共享 mutex 中检查并提取新的截屏路径（在 render 中调用）
    fn check_screenshot_result(&mut self) {
        if let Ok(mut guard) = self.shared_screenshot_path.lock() {
            if let Some(path) = guard.take() {
                self.last_screenshot_path = path.into();
                self.status = "截屏已保存".into();
            }
        }
    }

    /// 从共享 mutex 中检查 GIF 合并结果（在 render 中调用）
    fn check_gif_result(&mut self) {
        if let Ok(mut guard) = self.shared_gif_result.lock() {
            if let Some(result) = guard.take() {
                self.status = result.into();
            }
        }
    }
}

/// 将录屏帧序列合并为 GIF 文件
///
/// 读取目录中的所有 PNG 帧，按文件名排序后编码为 GIF。
/// 使用 `image` crate 的 `GifEncoder` 进行编码。
fn merge_frames_to_gif(dir: &std::path::Path) -> Result<String, String> {
    // 收集目录中的所有 PNG 文件
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("读取录屏目录失败: {}", e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("png"))
                .unwrap_or(false)
        })
        .collect();

    if entries.is_empty() {
        return Err("没有找到录屏帧（屏幕可能未发生变化）".into());
    }

    // 按文件名排序确保帧顺序正确
    entries.sort();

    let frame_count = entries.len();
    let output_name = format!(
        "recording_{}.gif",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );
    let output_path = dir.parent().unwrap_or(dir).join(&output_name);

    // 创建 GIF 编码器
    let output_file =
        std::fs::File::create(&output_path).map_err(|e| format!("创建 GIF 文件失败: {}", e))?;
    let mut encoder = image::codecs::gif::GifEncoder::new(output_file);

    // 设置无限循环播放
    encoder
        .set_repeat(image::codecs::gif::Repeat::Infinite)
        .map_err(|e| format!("设置 GIF 循环失败: {}", e))?;

    // 编码每一帧（每帧 100ms = 10fps）
    let delay = image::Delay::from_numer_denom_ms(100, 1);
    let mut frames = Vec::with_capacity(frame_count);

    for entry in &entries {
        let img =
            image::open(entry).map_err(|e| format!("读取帧失败 {}: {}", entry.display(), e))?;
        frames.push(image::Frame::from_parts(img.into_rgba8(), 0, 0, delay));
    }

    encoder
        .encode_frames(frames.into_iter())
        .map_err(|e| format!("GIF 编码失败: {}", e))?;

    // 删除单帧 PNG 文件
    for entry in &entries {
        let _ = std::fs::remove_file(entry);
    }

    Ok(format!(
        "GIF 已保存: {} ({} 帧)",
        output_path.display(),
        frame_count
    ))
}

impl Render for ScreenCaptureExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 直接从共享 mutex 检查截屏结果和 GIF 合并结果
        self.check_screenshot_result();
        self.check_gif_result();

        let can_start = self.selected_source.is_some() && !self.is_capturing;
        let can_stop = self.is_capturing;
        let can_screenshot = self.is_capturing;
        let can_start_record = self.is_capturing && !self.is_recording && !self.is_merging;
        let can_stop_record = self.is_recording;

        // 构建源列表项
        let source_items: Vec<_> = self
            .source_metadata
            .iter()
            .enumerate()
            .map(|(i, source)| {
                let is_selected = self.selected_source == Some(i);
                let label = source
                    .label
                    .as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("显示器 {}", source.id));
                let resolution = format!(
                    "{}x{}",
                    source.resolution.width.0, source.resolution.height.0
                );
                let bg_color = if is_selected {
                    rgb(0x0078D4)
                } else {
                    rgb(0xF0F0F0)
                };
                let text_color = if is_selected {
                    rgb(0xFFFFFF)
                } else {
                    rgb(0x505050)
                };

                div()
                    .id(format!("source-{}", i))
                    .flex()
                    .gap_2()
                    .items_center()
                    .p_2()
                    .rounded_md()
                    .bg(bg_color)
                    .text_color(text_color)
                    .text_sm()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, _cx| {
                        this.selected_source = Some(i);
                        this.status = format!("已选择源 {}", i + 1).into();
                    }))
                    .child(format!("{}. {} ({})", i + 1, label, resolution))
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .size(px(600.0))
            .p_4()
            .text_color(rgb(0x505050))
            .child(
                div()
                    .text_xl()
                    .font_weight(rgpui::FontWeight::BOLD)
                    .child("屏幕捕获示例"),
            )
            .child(div().text_sm().child(format!(
                "屏幕捕获支持: {}",
                if self.capture_supported { "是" } else { "否" }
            )))
            .child(
                div()
                    .text_sm()
                    .child(format!("可用源数量: {}", self.sources.len())),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(rgpui::FontWeight::BOLD)
                            .child("可用的屏幕捕获源（点击选择）:"),
                    )
                    .children(source_items),
            )
            // === 捕获控制按钮 ===
            .child({
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .id("start-button")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .text_sm()
                            .cursor_pointer()
                            .bg(if can_start {
                                rgb(0x0078D4)
                            } else {
                                rgb(0xC0C0C0)
                            })
                            .text_color(rgb(0xFFFFFF))
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(idx) = this.selected_source {
                                    if let Some(source) = this.sources.get(idx) {
                                        // 重置共享帧计数器
                                        this.shared_frame_count.store(0, Ordering::Relaxed);
                                        this.frame_count = 0;

                                        // 克隆所有需要在帧回调中使用的共享状态
                                        let frame_count_ref = this.shared_frame_count.clone();
                                        let screenshot_flag = this.screenshot_requested.clone();
                                        let screenshot_path_ref =
                                            this.shared_screenshot_path.clone();
                                        let recording_flag = this.recording_active.clone();
                                        let recorded_count_ref =
                                            this.shared_recorded_frame_count.clone();
                                        let recording_dir_ref =
                                            this.shared_recording_dir.clone();

                                        // 创建帧回调，处理截屏和录屏逻辑
                                        let frame_callback = Box::new(
                                            move |frame: rgpui::ScreenCaptureFrame| {
                                                frame_count_ref
                                                    .fetch_add(1, Ordering::Relaxed);

                                                // 截屏：捕获单帧并保存为 PNG
                                                if screenshot_flag
                                                    .swap(false, Ordering::Relaxed)
                                                {
                                                    match frame.to_rgba() {
                                                        Some(image) => {
                                                            let timestamp = std::time::SystemTime
                                                                ::now()
                                                                .duration_since(
                                                                    std::time::UNIX_EPOCH,
                                                                )
                                                                .unwrap_or_default()
                                                                .as_secs();
                                                            let path = format!(
                                                                "screenshot_{}.png",
                                                                timestamp
                                                            );
                                                            match image.save(&path) {
                                                                Ok(()) => {
                                                                    log::info!(
                                                                        "截屏已保存: {}",
                                                                        path
                                                                    );
                                                                    if let Ok(mut guard) =
                                                                        screenshot_path_ref.lock()
                                                                    {
                                                                        *guard = Some(path);
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    log::error!(
                                                                        "截屏保存失败: {}",
                                                                        e
                                                                    );
                                                                    if let Ok(mut guard) =
                                                                        screenshot_path_ref.lock()
                                                                    {
                                                                        *guard = Some(
                                                                            format!("保存失败: {}", e),
                                                                        );
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        None => {
                                                            log::error!("截屏帧转换失败: to_rgba() 返回 None");
                                                            if let Ok(mut guard) =
                                                                screenshot_path_ref.lock()
                                                            {
                                                                *guard = Some(
                                                                    "帧转换失败".into(),
                                                                );
                                                            }
                                                        }
                                                    }
                                                }

                                                // 录屏：持续保存帧序列
                                                if recording_flag.load(Ordering::Relaxed) {
                                                    if let Some(image) = frame.to_rgba() {
                                                        let idx = recorded_count_ref
                                                            .fetch_add(
                                                                1, Ordering::Relaxed,
                                                            );
                                                        // 从共享状态读取录屏目录
                                                        let dir = recording_dir_ref
                                                            .lock()
                                                            .ok()
                                                            .and_then(|guard| guard.clone());
                                                        if let Some(dir) = dir {
                                                            let frame_path = dir.join(format!(
                                                                "frame_{:06}.png",
                                                                idx
                                                            ));
                                                            if let Err(e) = image.save(&frame_path)
                                                            {
                                                                log::error!(
                                                                    "录屏帧保存失败: {}",
                                                                    e
                                                                );
                                                            }
                                                        }
                                                    } else {
                                                        log::warn!("录屏帧转换失败: to_rgba() 返回 None");
                                                    }
                                                }
                                            },
                                        );

                                        let source = source.clone();
                                        let foreground_executor =
                                            cx.foreground_executor().clone();
                                        let sources_rx =
                                            source.stream(&foreground_executor, frame_callback);

                                        this.is_capturing = true;
                                        this.status = "正在捕获...".into();

                                        // 启动帧计数轮询任务
                                        this.start_polling(window, cx);

                                        cx.spawn_in(window, async move |this, cx| {
                                            match sources_rx.await {
                                                Ok(Ok(stream)) => {
                                                    this.update(cx, |view, _cx| {
                                                        view._capture_stream = Some(stream);
                                                        view.status = "捕获中...".into();
                                                    })
                                                    .ok();
                                                }
                                                Ok(Err(e)) => {
                                                    this.update(cx, |view, _cx| {
                                                        view.is_capturing = false;
                                                        view._poll_task = None;
                                                        view.status = format!(
                                                            "启动捕获失败: {}",
                                                            e
                                                        )
                                                        .into();
                                                    })
                                                    .ok();
                                                }
                                                Err(_) => {
                                                    this.update(cx, |view, _cx| {
                                                        view.is_capturing = false;
                                                        view._poll_task = None;
                                                        view.status =
                                                            "捕获请求被取消".into();
                                                    })
                                                    .ok();
                                                }
                                            }
                                        })
                                        .detach();
                                    }
                                }
                            }))
                            .child("开始捕获"),
                    )
                    .child(
                        div()
                            .id("stop-button")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .text_sm()
                            .cursor_pointer()
                            .bg(if can_stop {
                                rgb(0xD83B01)
                            } else {
                                rgb(0xC0C0C0)
                            })
                            .text_color(rgb(0xFFFFFF))
                            .on_click(cx.listener(|this, _, _, _cx| {
                                if this.is_capturing {
                                    // 停止录屏（如果正在录屏）
                                    this.recording_active.store(false, Ordering::Relaxed);
                                    this.is_recording = false;
                                    this._capture_stream = None;
                                    this._poll_task = None;
                                    this.is_capturing = false;
                                    this.status = "已停止捕获".into();
                                }
                            }))
                            .child("停止捕获"),
                    )
            })
            // === 截屏区域 ===
            .child({
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0xD0D0D0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(rgpui::FontWeight::BOLD)
                            .child("截屏"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .id("screenshot-button")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .text_sm()
                                    .cursor_pointer()
                                    .bg(if can_screenshot {
                                        rgb(0x107C10)
                                    } else {
                                        rgb(0xC0C0C0)
                                    })
                                    .text_color(rgb(0xFFFFFF))
                                    .on_click(cx.listener(|this, _, _, _cx| {
                                        if this.is_capturing {
                                            this.screenshot_requested
                                                .store(true, Ordering::Relaxed);
                                            this.status = "正在截屏...".into();
                                        }
                                    }))
                                    .child("截屏"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x808080))
                                    .child(format!(
                                        "保存为: {}",
                                        self.last_screenshot_path
                                    )),
                            ),
                    )
            })
            // === 录屏区域 ===
            .child({
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0xD0D0D0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(rgpui::FontWeight::BOLD)
                            .child("录屏"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x808080))
                            .child("注意：屏幕静止时不会产生新帧"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .id("start-record-button")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .text_sm()
                                    .cursor_pointer()
                                    .bg(if can_start_record {
                                        rgb(0xD83B01)
                                    } else {
                                        rgb(0xC0C0C0)
                                    })
                                    .text_color(rgb(0xFFFFFF))
                                    .on_click(cx.listener(|this, _, _, _cx| {
                                        if this.is_capturing && !this.is_recording {
                                            // 创建录屏目录
                                            let timestamp = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_secs();
                                            let dir_name =
                                                format!("recording_{}", timestamp);
                                            let dir = std::path::Path::new(&dir_name);
                                            if std::fs::create_dir_all(dir).is_ok() {
                                                // 通过共享状态通知帧回调录屏目录
                                                if let Ok(mut guard) =
                                                    this.shared_recording_dir.lock()
                                                {
                                                    *guard = Some(dir.to_path_buf());
                                                }
                                                this.recording_active
                                                    .store(true, Ordering::Relaxed);
                                                this.shared_recorded_frame_count
                                                    .store(0, Ordering::Relaxed);
                                                this.recorded_frame_count = 0;
                                                this.is_recording = true;
                                                this.status = format!(
                                                    "正在录屏: {}",
                                                    dir_name
                                                )
                                                .into();
                                            }
                                        }
                                    }))
                                    .child("开始录屏"),
                            )
                            .child(
                                div()
                                    .id("stop-record-button")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .text_sm()
                                    .cursor_pointer()
                                    .bg(if can_stop_record {
                                        rgb(0xD83B01)
                                    } else {
                                        rgb(0xC0C0C0)
                                    })
                                    .text_color(rgb(0xFFFFFF))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if this.is_recording {
                                            this.recording_active
                                                .store(false, Ordering::Relaxed);
                                            this.is_recording = false;

                                            // 获取录屏目录路径并启动 GIF 合并
                                            let recording_dir = this
                                                .shared_recording_dir
                                                .lock()
                                                .ok()
                                                .and_then(|guard| guard.clone());
                                            let gif_result_ref =
                                                this.shared_gif_result.clone();
                                            let merging_flag =
                                                this.shared_is_merging.clone();

                                            if let Some(dir) = recording_dir {
                                                this.is_merging = true;
                                                this.status =
                                                    "正在合并 GIF...".into();
                                                merging_flag
                                                    .store(true, Ordering::Relaxed);

                                                cx.spawn_in(window, async move |this, cx| {
                                                    // 在后台线程执行 GIF 合并
                                                    let result = cx
                                                        .background_executor()
                                                        .spawn(async move {
                                                            merge_frames_to_gif(&dir)
                                                        })
                                                        .await;

                                                    match result {
                                                        Ok(msg) => {
                                                            if let Ok(mut guard) =
                                                                gif_result_ref.lock()
                                                            {
                                                                *guard = Some(msg);
                                                            }
                                                        }
                                                        Err(e) => {
                                                            if let Ok(mut guard) =
                                                                gif_result_ref.lock()
                                                            {
                                                                *guard = Some(e);
                                                            }
                                                        }
                                                    }
                                                    merging_flag.store(
                                                        false,
                                                        Ordering::Relaxed,
                                                    );
                                                    this.update(cx, |view, _cx| {
                                                        view.is_merging = false;
                                                    })
                                                    .ok();
                                                })
                                                .detach();
                                            } else {
                                                this.status = "录屏已停止（无帧数据）"
                                                    .into();
                                            }
                                        }
                                    }))
                                    .child(if self.is_merging {
                                        "合并中..."
                                    } else {
                                        "停止录屏"
                                    }),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x808080))
                                    .child(format!(
                                        "已录制帧数: {}",
                                        self.recorded_frame_count
                                    )),
                            )
                    )
                    .when(
                        self.shared_recording_dir
                            .lock()
                            .ok()
                            .and_then(|g| g.clone())
                            .is_some(),
                        |parent| {
                            let path_str = self
                                .shared_recording_dir
                                .lock()
                                .ok()
                                .and_then(|g| g.clone())
                                .map(|p| p.display().to_string())
                                .unwrap_or_default();
                            parent.child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x808080))
                                    .child(format!("保存路径: {}", path_str)),
                            )
                        },
                    )
            })
            // === 状态信息 ===
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x808080))
                    .child(format!("状态: {}", self.status)),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x808080))
                    .child(format!("已捕获帧数: {}", self.frame_count)),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(600.), px(720.0)), cx);

        // 检查屏幕捕获支持
        let capture_supported = cx.is_screen_capture_supported();

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| ScreenCaptureExample::new(window, cx, capture_supported)),
        )
        .unwrap();

        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
