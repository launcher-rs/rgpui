//! Tokio 异步运行时集成示例
//!
//! 此示例演示如何将 Tokio 异步任务与 RGPUI 应用结合使用。
//! 功能包括：
//! - 使用 `rgpui::tokio::init` 初始化 Tokio 运行时
//! - 通过 `rgpui::Tokio::spawn` 在 Tokio 线程池上执行异步任务
//! - 通过 `rgpui::Tokio::spawn_result` 处理带错误结果的任务
//! - 通过 `rgpui::Tokio::handle` 直接访问 Tokio 运行时句柄
//! - 异步任务完成后自动更新界面
//! - （`reqwest` feature）通过 `Tokio::spawn` + `reqwest` 执行真实 HTTP 请求
//!
//! 运行：
//! - 基础：cargo run -p rgpui --example tokio --features tokio
//! - 含 HTTP 请求：cargo run -p rgpui --example tokio --features tokio,reqwest

#![cfg_attr(target_family = "wasm", no_main)]

use std::time::{Duration, Instant};

use rgpui::{
    App, Bounds, Context, Render, SharedString, Task, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};
use rgpui_platform::application;

/// 应用状态
struct TokioExample {
    /// 异步任务执行状态信息
    status: SharedString,
    /// 是否正在执行异步任务
    running: bool,
}

impl TokioExample {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            status: "点击按钮启动 Tokio 任务".into(),
            running: false,
        }
    }

    /// 启动一个普通的 Tokio 异步任务（返回结果或 JoinError）
    fn run_spawn(&mut self, cx: &mut Context<Self>) {
        self.set_running(cx, "Tokio::spawn 任务执行中...");

        // 在 Tokio 线程池上执行异步计算，模拟耗时操作
        let raw_task = rgpui::tokio::Tokio::spawn(cx, async {
            // 使用 tokio::time::sleep 模拟异步 IO
            tokio::time::sleep(Duration::from_secs(1)).await;

            // 模拟 CPU 密集计算
            let mut sum: u64 = 0;
            for i in 0..1_000_000 {
                sum = sum.wrapping_add(i);
            }

            SharedString::from(format!("Tokio::spawn 完成：计算结果 = {sum}"))
        });

        // 将 JoinError 转换为 anyhow::Error，统一交给 await_task 处理
        let task = cx.spawn(async move |_, _| {
            let result = raw_task.await.map_err(anyhow::Error::from);
            result
        });
        self.await_task(cx, task);
    }

    /// 启动一个带错误处理能力的 Tokio 异步任务
    fn run_spawn_result(&mut self, cx: &mut Context<Self>) {
        self.set_running(cx, "Tokio::spawn_result 任务执行中...");

        // spawn_result 直接返回 Task<anyhow::Result<R>>，无需额外转换
        let task = rgpui::tokio::Tokio::spawn_result(cx, async {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let elapsed = Instant::now().elapsed();
            Ok(SharedString::from(format!(
                "Tokio::spawn_result 完成：耗时 {} ms",
                elapsed.as_millis()
            )))
        });

        self.await_task(cx, task);
    }

    /// 启动一个真实 HTTP 请求（reqwest 底层即 Tokio，无需额外转换）
    #[cfg(feature = "reqwest")]
    fn run_http_request(&mut self, cx: &mut Context<Self>) {
        self.set_running(cx, "HTTP 请求执行中...");

        let task = rgpui::tokio::Tokio::spawn_result(cx, async {
            // 请求外部服务获取公网 IP，演示真实网络异步 IO
            let response = reqwest::get("https://httpbin.org/ip").await?;
            let body = response.text().await?;
            Ok(SharedString::from(format!("HTTP 响应：{body}")))
        });

        self.await_task(cx, task);
    }

    /// 设置任务状态并标记运行中
    fn set_running(&mut self, cx: &mut Context<Self>, status: impl Into<SharedString>) {
        self.running = true;
        self.status = status.into();
        cx.notify();
    }

    /// 等待异步任务完成并更新界面
    fn await_task(&self, cx: &mut Context<Self>, task: Task<anyhow::Result<SharedString>>) {
        cx.spawn(async move |this, cx| {
            // 等待任务结果（后台线程执行，不会阻塞主线程）
            let result = task.await;
            this.update(cx, |this, cx| {
                this.running = false;
                match result {
                    Ok(message) => this.status = message,
                    Err(err) => this.status = format!("任务失败：{err}").into(),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

/// HTTP 请求按钮（仅 reqwest feature 下可用）
#[cfg(feature = "reqwest")]
fn http_request_button(running: bool, cx: &mut Context<TokioExample>) -> impl IntoElement {
    div()
        .id("http_request")
        .px_4()
        .py_2()
        .rounded(px(8.0))
        .bg(rgb(0xe67e22))
        .text_color(rgb(0xffffff))
        .cursor_pointer()
        .when(running, |this| this.opacity(0.5))
        .on_click(cx.listener(|this, _, _, cx| {
            if !this.running {
                this.run_http_request(cx);
            }
        }))
        .child("HTTP 请求")
}

/// HTTP 请求按钮占位（未开启 reqwest feature 时不渲染）
#[cfg(not(feature = "reqwest"))]
fn http_request_button(_running: bool, _cx: &mut Context<TokioExample>) -> impl IntoElement {
    div()
}

impl Render for TokioExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let running = self.running;

        div()
            .flex()
            .flex_col()
            .gap_4()
            .size(px(520.0))
            .justify_center()
            .items_center()
            .bg(rgb(0xfafafa))
            .text_color(rgb(0x333333))
            .child(
                div()
                    .text_xl()
                    .font_weight(rgpui::FontWeight::BOLD)
                    .child("Tokio 异步集成示例"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x888888))
                    .child("Tokio 任务在独立线程池执行，完成后通过 RGPUI 调度回主线程更新界面"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_3()
                    .child(
                        div()
                            .id("spawn")
                            .px_4()
                            .py_2()
                            .rounded(px(8.0))
                            .bg(rgb(0x4a7dff))
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .when(running, |this| this.opacity(0.5))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if !this.running {
                                    this.run_spawn(cx);
                                }
                            }))
                            .child("spawn"),
                    )
                    .child(
                        div()
                            .id("spawn_result")
                            .px_4()
                            .py_2()
                            .rounded(px(8.0))
                            .bg(rgb(0x2ecc71))
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .when(running, |this| this.opacity(0.5))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if !this.running {
                                    this.run_spawn_result(cx);
                                }
                            }))
                            .child("spawn_result"),
                    )
                    // 仅在开启 reqwest feature 时提供 HTTP 请求按钮
                    .when(cfg!(feature = "reqwest"), |this| {
                        this.child(http_request_button(running, cx))
                    }),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(if running {
                        rgb(0xe67e22)
                    } else {
                        rgb(0x2ecc71)
                    })
                    .child(self.status.clone()),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        // 初始化 Tokio 运行时（2 个工作线程）
        rgpui::tokio::init(cx);

        let bounds = Bounds::centered(None, size(px(520.), px(320.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_window, cx| cx.new(TokioExample::new),
        )
        .ok();

        cx.activate(true);
    });
}

fn main() {
    run_example();
}
