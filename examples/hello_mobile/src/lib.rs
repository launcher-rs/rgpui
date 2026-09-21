//! 移动端共享 UI：`hello_mobile` 三端（桌面 / Android / iOS）共用同一套视图。
//!
//! 平台差异只收敛在入口函数（`main.rs` / `android_main` / iOS 导出函数），
//! 视图代码经 [`rgpui_platform::target_platform`] 做运行时分支，不写 `#[cfg]`。

use rgpui::{App, Context, Window, div, prelude::*, px, rgb};
use rgpui_platform::target_platform;

/// 移动端问候视图（展示当前平台 + 点击计数验证触摸输入）。
pub struct HelloMobile {
    /// 点击/触摸计数，用于验证输入是否工作。
    taps: u32,
}

impl Render for HelloMobile {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let platform = target_platform();
        let taps = self.taps;
        let subtitle = if platform.is_mobile() {
            "触摸优先：所有交互都可点"
        } else {
            "桌面预览：同一套代码，真机行为以 Android 为准"
        };
        div()
            .flex()
            .flex_col()
            .gap_4()
            .items_center()
            .justify_center()
            .size_full()
            .bg(rgb(0x1a1a2e))
            .text_color(rgb(0xffffff))
            .child(div().text_3xl().child(format!("Hello, {platform}!")))
            .child(div().text_lg().child(subtitle))
            .child(
                div()
                    .text_xl()
                    .child(format!("触摸 / 点击次数: {taps}")),
            )
            .child(
                div()
                    .id("tap-target")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(200.0))
                    .h(px(80.0))
                    .rounded_lg()
                    .bg(rgb(0x3b82f6))
                    .text_color(rgb(0xffffff))
                    .text_lg()
                    .child("点我")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.taps += 1;
                        log::info!("hello_mobile: tap count = {}", this.taps);
                        cx.notify();
                    })),
            )
    }
}

/// 打开主窗口（桌面与移动端共用；移动端全屏，窗口 bounds 传空）。
pub fn open_main_window(cx: &mut App) {
    cx.open_window(rgpui::WindowOptions::default(), |_, cx| {
        cx.new(|_| HelloMobile { taps: 0 })
    })
    .expect("打开主窗口失败");
    cx.activate(true);
}

// ── Android 入口 ─────────────────────────────────────────────────────────────

/// `android-activity` 回调的原生入口：`.so` 被加载后在此线程常驻事件循环。
///
/// 全流程：日志 → panic hook → `init_platform`（存 `AndroidApp` + 建全局平台）→
/// `Application::with_platform`（共享同一平台实例）→ `run`（阻塞跑事件循环，
/// 首窗 surface 就绪后调启动回调挂视图）。详见 `docs/1.4.0/android-guide.md`。
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    use rgpui::Application;
    use rgpui_android::bridge::{init_platform, install_panic_hook, shared_platform};

    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("hello_mobile"),
    );
    install_panic_hook();
    log::info!("hello_mobile: android_main entered");

    let _platform = init_platform(&app);
    let shared = match shared_platform() {
        Some(shared) => shared,
        None => {
            log::error!("hello_mobile: 全局平台未就绪");
            return;
        }
    };
    log::info!("hello_mobile: entering Application::run");
    Application::with_platform(shared.into_rc()).run(|cx| {
        open_main_window(cx);
    });
    log::info!("hello_mobile: Application::run returned");
}

// ── iOS 入口（M4 实现，M1 仅占位保证 check 通过） ─────────────────────────────

/// iOS 宿主经此符号注册应用回调（M4 接 `UIKit` 生命周期）。
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn rgpui_ios_register_app() {
    log::info!("hello_mobile: iOS 入口占位（M4 实现）");
}
