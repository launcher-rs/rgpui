//! 移动端共享 UI：`hello_mobile` 三端（桌面 / Android / iOS）共用同一套视图。
//!
//! 平台差异只收敛在入口函数（`main.rs` / `android_main` / iOS 导出函数），
//! 视图代码经 [`rgpui_platform::target_platform`] 做运行时分支，不写 `#[cfg]`。

use rgpui::input_ui::{Input, InputState};
use rgpui::{App, BatteryStatus, Context, Entity, Window, div, prelude::*, px, rgb};
use rgpui_platform::target_platform;

/// 移动端冒烟视图：展示当前平台 + 点按计数（顺带验证触摸→点击）+
/// 电池状态（M3-7 系统 API 示例：点击时短振并刷新电量）+
/// 输入框（M3-2 输入法 testbed：拼音组词真机可 typing）。
pub struct HelloMobile {
    /// 点按/触摸计数（验证点击是否送达）。
    taps: u32,
    /// 输入法 testbed 输入框。
    input: Entity<InputState>,
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
            .child(div().text_xl().child(format!("触摸 / 点击次数: {taps}")))
            .child(div().text_lg().child(battery_line(&cx.battery_status())))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .items_center()
                    // 根视图是白字，输入框底白：显式深色否则白字看不见。
                    .child(
                        div()
                            .w(px(220.0))
                            .child(Input::new(&self.input).text_color(rgb(0x111111))),
                    )
                    // 键盘按钮：输入框获焦后点此弹软键盘（桌面端为空操作）。
                    .child(
                        div()
                            .id("keyboard-button")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(64.0))
                            .h(px(48.0))
                            .rounded_lg()
                            .bg(rgb(0x10b981))
                            .text_color(rgb(0xffffff))
                            .child("⌨️")
                            .on_click(cx.listener(|_, _, _, _| {
                                show_keyboard();
                            })),
                    ),
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
                        // M3-7 系统 API 实测：短振 30ms。
                        cx.vibrate(30);
                        log::info!(
                            "hello_mobile: tap count = {}, battery = {:?}, input = {:?}",
                            this.taps,
                            cx.battery_status(),
                            this.input.read(cx).value().to_string()
                        );
                        cx.notify();
                    })),
            )
    }
}

/// 电池状态展示行（未知时显示横线，桌面端恒为未知）。
fn battery_line(battery: &BatteryStatus) -> String {
    match battery.level_percent {
        Some(percent) => {
            let state = if battery.charging {
                "充电中"
            } else {
                "未充电"
            };
            format!("电池: {percent}%（{state}）")
        }
        None => "电池: --".to_string(),
    }
}

/// 打开主窗口（桌面与移动端共用；移动端全屏，窗口 bounds 传空）。
pub fn open_main_window(cx: &mut App) {
    cx.open_window(rgpui::WindowOptions::default(), |window, cx| {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("在此输入（拼音可组词）"));
        cx.new(|cx| HelloMobile { taps: 0, input })
    })
    .expect("打开主窗口失败");
    cx.activate(true);
}

/// 弹出软键盘（仅 Android 真机；桌面端为空操作）。
fn show_keyboard() {
    #[cfg(target_os = "android")]
    rgpui_android::show_keyboard();
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
