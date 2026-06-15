//! 屏幕捕获示例
//!
//! 此示例演示如何使用 screen-capture feature 捕获屏幕内容。
//! 功能包括：
//! - 检测屏幕捕获是否受支持
//! - 列出所有可用的屏幕捕获源（显示器）
//! - 点击选择屏幕捕获源
//! - 启动/停止屏幕捕获流
//! - 实时显示帧计数和元数据信息

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, Render, SharedString, SourceMetadata, Task, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size,
};
use rgpui_platform::application;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

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
        self._poll_task = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                // 每 100ms 轮询一次原子计数器
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;

                let count = shared_count.load(Ordering::Relaxed);
                this.update(cx, |view, _cx| {
                    view.frame_count = count;
                })
                .ok();
            }
        }));
    }
}

impl Render for ScreenCaptureExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_start = self.selected_source.is_some() && !self.is_capturing;
        let can_stop = self.is_capturing;

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

                                        // 创建帧回调，递增原子计数器
                                        let frame_count_ref = this.shared_frame_count.clone();
                                        let frame_callback = Box::new(move |_frame| {
                                            frame_count_ref.fetch_add(1, Ordering::Relaxed);
                                        });

                                        let source = source.clone();
                                        let foreground_executor = cx.foreground_executor().clone();
                                        let sources_rx =
                                            source.stream(&foreground_executor, frame_callback);

                                        this.is_capturing = true;
                                        this.status = "正在捕获...".into();

                                        // 启动帧计数轮询任务
                                        this.start_polling(window, cx);

                                        cx.spawn_in(window, async move |this, cx| match sources_rx
                                            .await
                                        {
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
                                                    view.status =
                                                        format!("启动捕获失败: {}", e).into();
                                                })
                                                .ok();
                                            }
                                            Err(_) => {
                                                this.update(cx, |view, _cx| {
                                                    view.is_capturing = false;
                                                    view._poll_task = None;
                                                    view.status = "捕获请求被取消".into();
                                                })
                                                .ok();
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
                                    this._capture_stream = None;
                                    this._poll_task = None;
                                    this.is_capturing = false;
                                    this.status = "已停止捕获".into();
                                }
                            }))
                            .child("停止捕获"),
                    )
            })
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
        let bounds = Bounds::centered(None, size(px(600.), px(500.0)), cx);

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
