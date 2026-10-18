use rgpui::single_instance::{SingleInstance, send_activate_to_existing};
use rgpui::{
    App, Bounds, Context, Keystroke, PermissionType, PowerSaveBlocker, PowerSaveBlockerKind,
    SystemPowerEvent, TrayIconEvent, TrayMenuItem, Window, WindowBackgroundAppearance,
    WindowBounds, WindowKind, WindowOptions, div, prelude::*, px, rgb, rgba, size,
};
use rgpui_platform::application;
use std::cell::RefCell;
use std::rc::Rc;

const APP_ID: &str = "com.example.daemon-app";

/// 电源抑制句柄：留着它抑制就在，`Drop` 即恢复系统的省电策略
type Inhibitor = Rc<RefCell<Option<Box<dyn PowerSaveBlocker>>>>;

struct OverlayView;

impl Render for OverlayView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .justify_center()
            .items_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_6()
                    .rounded(px(12.0))
                    .bg(rgba(0x000000dd))
                    .text_color(rgb(0xffffff))
                    .shadow_lg()
                    .max_w(px(400.0))
                    .child(
                        div()
                            .text_xl()
                            .font_weight(rgpui::FontWeight::BOLD)
                            .child("Daemon App Overlay"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgba(0xffffffaa))
                            .child("This overlay window is always on top."),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgba(0xffffffaa))
                            .child("Uses WindowKind::Overlay + transparent background."),
                    ),
            )
    }
}

struct SettingsView;

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_6()
            .size_full()
            .bg(rgb(0xfafafa))
            .text_color(rgb(0x333333))
            .child(
                div()
                    .text_xl()
                    .font_weight(rgpui::FontWeight::BOLD)
                    .child("Settings"),
            )
            .child(
                div()
                    .text_sm()
                    .child("This is a normal settings window opened from the tray menu."),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x888888))
                    .child(format!("Application ID: {}", APP_ID)),
            )
    }
}

fn main() {
    // 诊断时用 RUST_LOG=debug 运行，可看到电源抑制等内部日志
    rgpui::init_logging();

    let _instance = match SingleInstance::acquire(APP_ID) {
        Ok(instance) => instance,
        Err(_) => {
            eprintln!("Another instance is already running. Sending activation signal.");
            let _ = send_activate_to_existing(APP_ID);
            std::process::exit(0);
        }
    };

    application().run(|cx: &mut App| {
        cx.set_keep_alive_without_windows(true);

        let inhibitor = setup_power(cx);
        setup_tray(cx, inhibitor);
        setup_global_hotkey(cx);
        log_capabilities(cx);

        let _ = cx.show_notification("Daemon App", "Application started in background");

        cx.activate(true);
    });
}

fn setup_tray(cx: &mut App, inhibitor: Inhibitor) {
    cx.set_tray_tooltip("Daemon App");

    cx.set_tray_menu(vec![
        TrayMenuItem::Action {
            label: "Show Overlay".into(),
            id: "show_overlay".into(),
        },
        TrayMenuItem::Action {
            label: "Settings".into(),
            id: "settings".into(),
        },
        TrayMenuItem::Separator,
        TrayMenuItem::Action {
            label: "阻止系统休眠".into(),
            id: "inhibit_sleep".into(),
        },
        TrayMenuItem::Action {
            label: "阻止息屏".into(),
            id: "inhibit_display".into(),
        },
        TrayMenuItem::Action {
            label: "取消电源阻止".into(),
            id: "release_inhibit".into(),
        },
        TrayMenuItem::Separator,
        TrayMenuItem::Action {
            label: "Quit".into(),
            id: "quit".into(),
        },
    ]);

    cx.on_tray_icon_event(|event, _cx| match event {
        TrayIconEvent::LeftClick => {
            eprintln!("Tray icon left-clicked");
        }
        TrayIconEvent::RightClick => {
            eprintln!("Tray icon right-clicked");
        }
        TrayIconEvent::DoubleClick => {
            eprintln!("Tray icon double-clicked");
        }
    });

    let menu_inhibitor = inhibitor;
    cx.on_tray_menu_action(move |id, cx| match id.as_ref() {
        "show_overlay" => {
            open_overlay(cx);
            cx.activate(true);
        }
        "settings" => {
            open_settings(cx);
            cx.activate(true);
        }
        // 阻止器只在这里被持有：菜单项按下才申请，取消项把句柄丢回去 ——
        // 句柄 Drop 时底层 fd 关闭，系统立刻恢复原来的省电策略。
        // 演示用的是同一个格子：换一种抑制会先丢掉前一种的句柄
        "inhibit_sleep" => {
            if menu_inhibitor.borrow().is_none() {
                let blocker = cx.start_power_save_blocker(PowerSaveBlockerKind::PreventSleep);
                match blocker {
                    Some(blocker) => {
                        eprintln!("已阻止系统休眠（systemd-inhibit --list 可见本条抑制）");
                        *menu_inhibitor.borrow_mut() = Some(blocker);
                    }
                    None => eprintln!("平台未能阻止系统休眠"),
                }
            }
        }
        "inhibit_display" => {
            if menu_inhibitor.borrow().is_none() {
                let blocker =
                    cx.start_power_save_blocker(PowerSaveBlockerKind::PreventDisplaySleep);
                match blocker {
                    Some(blocker) => {
                        eprintln!("已阻止息屏（systemd-inhibit --list 可见本条抑制）");
                        *menu_inhibitor.borrow_mut() = Some(blocker);
                    }
                    None => eprintln!("平台未能阻止息屏"),
                }
            }
        }
        "release_inhibit" => {
            if menu_inhibitor.borrow_mut().take().is_some() {
                eprintln!("已取消电源阻止");
            }
        }
        "quit" => {
            cx.quit();
        }
        _ => {}
    });
}

/// 注册系统电源事件回调，并交出保存抑制句柄的位置
///
/// 事件源是 login1 的 `PrepareForSleep` 信号：参数为真表示「即将睡眠」，此刻还在
/// 事件循环里，是应用收尾的窗口；为假表示「已唤醒」。
fn setup_power(cx: &mut App) -> Inhibitor {
    let inhibitor: Inhibitor = Rc::new(RefCell::new(None));
    let held = inhibitor.clone();
    cx.on_system_power_event(move |event, _cx| match event {
        SystemPowerEvent::Sleep => {
            eprintln!(
                "系统即将睡眠，抑制句柄是否仍持有: {}",
                held.borrow().is_some()
            );
        }
        SystemPowerEvent::WakeUp => {
            eprintln!("系统已唤醒");
        }
    });
    inhibitor
}

fn setup_global_hotkey(cx: &mut App) {
    let keystroke = Keystroke::parse("cmd-shift-k").expect("valid keystroke");
    if let Err(err) = cx.register_global_hotkey(1, &keystroke) {
        eprintln!("Failed to register global hotkey: {}", err);
    }

    cx.on_global_hotkey(move |id, _cx| {
        if id == 1 {
            eprintln!("Global hotkey triggered (Cmd+Shift+K)");
        }
    });
}

/// 打印当前平台的能力查询结果，用于确认这些 API 在应用层真的可达
fn log_capabilities(cx: &App) {
    let os = cx.os_info();
    eprintln!("OS: {} {}", os.name, os.version);
    eprintln!("Network: {:?}", cx.network_status());
    eprintln!(
        "Idle: {:?}",
        cx.system_idle_time().map(|idle| idle.as_secs())
    );
    for kind in [
        PermissionType::Accessibility,
        PermissionType::ScreenCapture,
        PermissionType::InputMonitoring,
    ] {
        eprintln!("Permission {kind:?}: {:?}", cx.check_permission(kind));
    }
    eprintln!("Auto launch enabled: {}", cx.is_auto_launch_enabled(APP_ID));
}

fn open_overlay(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(500.), px(300.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::Overlay,
            titlebar: None,
            focus: true,
            show: true,
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        |_, cx| cx.new(|_| OverlayView),
    )
    .ok();
}

fn open_settings(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(400.), px(300.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::Normal,
            titlebar: Some(rgpui::TitlebarOptions {
                title: Some("Daemon App Settings".into()),
                ..Default::default()
            }),
            focus: true,
            show: true,
            ..Default::default()
        },
        |_, cx| cx.new(|_| SettingsView),
    )
    .ok();
}
