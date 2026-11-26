//! 平台能力与契约示例：展示本框架「看似跨平台、实际有平台差异」的 API。
//!
//! 这些读数直接来自 `App` / `Window`，在 Linux（X11 / Wayland）上可实时验证。
//! 每项附平台差异说明，完整审计见 `docs/linux-api-parity-audit.md`：
//! - 窗口：window_controls / window_decorations / is_resizable / is_minimizable
//! - 系统：os_info / compositor_name / network_status / focused_window_info / window_stack
//! - 显示：primary_display / button_layout / gpu_specs
//! - 外观：reduce_motion / should_auto_hide_scrollbars（GNOME overlay-scrolling）
//! - 能力：权限 / 屏幕捕获 / URL scheme / 电源事件 / 托盘事件（只读说明）

use rgpui::prelude::*;
use rgpui::{
    Button, Context, FontWeight, IntoElement, PermissionType, Styled, Window, WindowDecorations,
    div, h_flex, px, v_flex,
};

use super::StoryItem;

/// 平台故事注册表。
pub fn stories() -> Vec<StoryItem> {
    vec![
        StoryItem {
            title: "平台能力",
            build: |window, cx| cx.new(|cx| PlatformStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "窗口契约",
            build: |window, cx| cx.new(|cx| WindowContractStory::new(window, cx)).into(),
        },
    ]
}

/// 平台能力读数视图。
struct PlatformStory;

impl PlatformStory {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for PlatformStory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let os = cx.os_info();
        let gpu = window.gpu_specs();
        let display = cx.primary_display();
        let focused = cx.focused_window_info();
        let layout = cx.button_layout();

        v_flex()
            .id("platform-story")
            .gap(px(16.0))
            .p(px(16.0))
            .child(section_title("系统信息"))
            .child(kv("OS", format!("{} {}", os.name, os.version)))
            .child(kv("合成器", cx.compositor_name().to_string()))
            .child(kv("网络", format!("{:?}", cx.network_status())))
            .child(kv(
                "窗口栈",
                cx.window_stack()
                    .map(|v| format!("{} 个窗口", v.len()))
                    .unwrap_or_else(|| "None（Wayland 无全局 Z-order）".into()),
            ))
            .child(section_title("显卡（gpu_specs）"))
            .child(match gpu {
                Some(specs) => v_flex()
                    .gap(px(4.0))
                    .child(kv("设备", specs.device_name))
                    .child(kv("驱动", specs.driver_name))
                    .child(kv("软件模拟", specs.is_software_emulated.to_string()))
                    .child(note("未实现时返回 None，不等于没有 GPU"))
                    .into_any_element(),
                None => kv("设备", "当前平台不可用").into_any_element(),
            })
            .child(section_title("显示与外观"))
            .child(kv(
                "主显示器",
                display
                    .map(|d| format!("{:?} {:?}", d.bounds(), d.uuid().ok()))
                    .unwrap_or_else(|| "无".into()),
            ))
            .child(kv(
                "按钮布局",
                layout
                    .map(|l| format!("left={:?} right={:?}", l.left, l.right))
                    .unwrap_or_else(|| "None（平台不提供）".into()),
            ))
            .child(kv(
                "聚焦窗口",
                focused
                    .map(|f| format!("{} / {} (pid {:?})", f.app_name, f.window_title, f.pid))
                    .unwrap_or_else(|| "无（Wayland 恒 None，不等于没有窗口）".into()),
            ))
            .child(kv("减少动效", cx.reduce_motion().to_string()))
            .child(kv(
                "滚动条自动隐藏",
                format!(
                    "{}（GNOME overlay-scrolling 实时同步）",
                    cx.should_auto_hide_scrollbars()
                ),
            ))
            .child(section_title("热状态（本分支未做）"))
            .child(kv("thermal_state", format!("{:?}", cx.thermal_state())))
            .child(note("Linux 恒 Nominal；需 sysfs 阈值口径，见 linux-parity-followups.md"))
            .child(section_title("权限"))
            .child(kv(
                "辅助功能",
                format!("{:?}", cx.check_permission(PermissionType::Accessibility)),
            ))
            .child(kv(
                "屏幕捕获",
                format!("{:?}", cx.check_permission(PermissionType::ScreenCapture)),
            ))
            .child(kv(
                "输入监控",
                format!("{:?}", cx.check_permission(PermissionType::InputMonitoring)),
            ))
            .child(note("macOS 输入监控已是独立 TCC 类别；Linux 按门户/会话判定"))
            .child(section_title("能力契约（只读说明）"))
            .child(kv(
                "支持屏幕捕获",
                cx.is_screen_capture_supported().to_string(),
            ))
            .child(note(
                "URL scheme：Linux 启动时重放桌面入口 %u 传入的 URL；Windows 仍只保存回调不派发。",
            ))
            .child(note(
                "电源事件 / 托盘事件：通过 App::on_system_power_event / on_tray_menu_action 订阅；Linux 托盘左键语义与 Windows/macOS 不同。",
            ))
            .child(
                Button::new("refresh-platform")
                    .label("重新读取")
                    .on_click(cx.listener(|_, _, _, cx| cx.refresh_windows())),
            )
    }
}

/// 窗口契约视图：展示窗口级能力查询与「请求 ≠ 实际」。
struct WindowContractStory;

impl WindowContractStory {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for WindowContractStory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let controls = window.window_controls();
        let decorations = window.window_decorations();

        v_flex()
            .id("window-contract-story")
            .gap(px(16.0))
            .p(px(16.0))
            .child(section_title("窗口能力查询"))
            .child(kv("可调整大小", window.is_resizable().to_string()))
            .child(kv("可最小化", window.is_minimizable().to_string()))
            .child(kv("已最大化", window.is_maximized().to_string()))
            .child(kv(
                "装饰模式",
                format!("{decorations:?}（协商后的实际值）"),
            ))
            .child(section_title("控制按钮（window_controls）"))
            .child(kv("最小化", controls.minimize.to_string()))
            .child(kv("最大化", controls.maximize.to_string()))
            .child(kv("全屏", controls.fullscreen.to_string()))
            .child(kv("窗口菜单", controls.window_menu.to_string()))
            .child(note(
                "maximize=is_resizable、minimize=is_minimizable：能力标志收敛按钮显示，避免画出点不动的按钮。",
            ))
            .child(section_title("请求 ≠ 实际"))
            .child(
                h_flex()
                    .gap(px(8.0))
                    .child(
                        Button::new("request-csd")
                            .label("请求 Client 装饰")
                            .on_click(cx.listener(|_, _, window, _| {
                                window.request_decorations(WindowDecorations::Client);
                            })),
                    )
                    .child(
                        Button::new("request-ssd")
                            .label("请求 Server 装饰")
                            .on_click(cx.listener(|_, _, window, _| {
                                window.request_decorations(WindowDecorations::Server);
                            })),
                    ),
            )
            .child(note(
                "Wayland 无 xdg-decoration 时 Server→Client；X11 无合成器时 Client→Server。回退记 warn，用 window_decorations() 查真实值。回退到 Server 时框架自动收敛自绘标题栏，避免与系统栏重复。",
            ))
            .child(section_title("跨平台契约说明"))
            .child(note(
                "window_control_area(Drag/Min/Max/Close) 仅 Windows/Web 生效；Linux/macOS 用显式鼠标事件 + start_window_move/zoom_window/minimize_window，或直接用 TitleBar。",
            ))
            .child(note(
                "show_window_menu 依赖 GTK WM 或 MousePress serial；set_position 在 Wayland 尽力而为、macOS 空操作；map_window 在 Wayland 隐式完成；request_attention 仅 X11 实现。",
            ))
            .child(note(
                "request_frame 已在 X11 / 无头后端补齐：纯定时动画不再依赖下次事件推进，无头下等一帧的 Future 可 resolve。",
            ))
    }
}

/// 键值行：参数直接是 IntoElement，避免 SharedString 无法作为子元素。
fn kv(key: impl IntoElement, value: impl IntoElement) -> impl IntoElement {
    h_flex()
        .gap(px(12.0))
        .child(
            div()
                .w(px(150.0))
                .text_sm()
                .font_weight(FontWeight(600.0))
                .child(key),
        )
        .child(div().text_sm().child(value))
}

/// 章节标题。
fn section_title(text: impl IntoElement) -> impl IntoElement {
    div()
        .text_size(px(14.0))
        .font_weight(FontWeight(700.0))
        .text_color(rgpui::hsla(0.0, 0.0, 0.35, 1.0))
        .child(text)
}

/// 说明性脚注。
fn note(text: impl IntoElement) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(rgpui::hsla(0.0, 0.0, 0.45, 1.0))
        .child(text)
}
