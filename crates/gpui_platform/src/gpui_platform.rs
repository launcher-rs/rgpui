//! 便捷工具包，重新导出 GPUI 的平台特性及 `current_platform` 构造函数，
//! 使使用者无需手动编写 `#[cfg]` 条件编译代码。

pub use gpui::Platform;

use std::rc::Rc;

/// 返回当前平台的后台执行器。
pub fn background_executor() -> gpui::BackgroundExecutor {
    current_platform(true).background_executor()
}

/// 返回一个 GUI 应用程序实例。
pub fn application() -> gpui::Application {
    gpui::Application::with_platform(current_platform(false))
}

/// 返回一个无头（headless）模式的应用程序实例。
pub fn headless() -> gpui::Application {
    gpui::Application::with_platform(current_platform(true))
}

/// 与 `application` 不同，此函数返回一个单线程的 Web 应用程序。
#[cfg(target_family = "wasm")]
pub fn single_threaded_web() -> gpui::Application {
    gpui::Application::with_platform(Rc::new(gpui_web::WebPlatform::new(false)))
}

/// 初始化 Web 平台的 panic hook 和日志系统。
/// 应在 `wasm_bindgen` 入口函数中、运行应用程序之前调用。
#[cfg(target_family = "wasm")]
pub fn web_init() {
    console_error_panic_hook::set_once();
    gpui_web::init_logging();
}

/// 返回当前操作系统的默认 [`Platform`] 实现。
///
/// # 参数
/// * `headless` - 是否以无头模式初始化平台
pub fn current_platform(headless: bool) -> Rc<dyn Platform> {
    #[cfg(target_os = "macos")]
    {
        Rc::new(gpui_macos::MacPlatform::new(headless))
    }

    #[cfg(target_os = "windows")]
    {
        Rc::new(
            gpui_windows::WindowsPlatform::new(headless)
                .expect("failed to initialize Windows platform"),
        )
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        gpui_linux::current_platform(headless)
    }

    #[cfg(target_family = "wasm")]
    {
        let _ = headless;
        Rc::new(gpui_web::WebPlatform::new(true))
    }
}

/// 返回当前平台的新 [`HeadlessRenderer`] 实例（如果可用）。
#[cfg(feature = "test-support")]
pub fn current_headless_renderer() -> Option<Box<dyn gpui::PlatformHeadlessRenderer>> {
    #[cfg(target_os = "macos")]
    {
        Some(Box::new(
            gpui_macos::metal_renderer::MetalHeadlessRenderer::new(),
        ))
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use gpui::{AppContext, Empty, VisualTestAppContext};
    use std::cell::RefCell;
    use std::time::Duration;

    // 注意：默认情况下所有 VisualTestAppContext 测试都会被忽略，因为它们需要
    // macOS 主线程。标准 Rust 测试在工作线程上运行，与 macOS AppKit/Cocoa API
    // 交互时会导致 SIGABRT。
    //
    // 运行这些测试请使用：
    // cargo test -p gpui visual_test_context -- --ignored --test-threads=1

    #[test]
    #[ignore] // 需要 macOS 主线程
    fn test_foreground_tasks_run_with_run_until_parked() {
        let mut cx = VisualTestAppContext::new(current_platform(false));

        let task_ran = Rc::new(RefCell::new(false));

        // 通过 App 的 spawn 方法生成前台任务
        // 这应该使用 TestDispatcher，而不是 MacDispatcher
        {
            let task_ran = task_ran.clone();
            cx.update(|cx| {
                cx.spawn(async move |_| {
                    *task_ran.borrow_mut() = true;
                })
                .detach();
            });
        }

        // 任务尚未执行
        assert!(!*task_ran.borrow());

        // run_until_parked 应该执行前台任务
        cx.run_until_parked();

        // 现在任务应该已执行
        assert!(*task_ran.borrow());
    }

    #[test]
    #[ignore] // 需要 macOS 主线程
    fn test_advance_clock_triggers_delayed_tasks() {
        let mut cx = VisualTestAppContext::new(current_platform(false));

        let task_ran = Rc::new(RefCell::new(false));

        // 生成等待计时器的任务
        {
            let task_ran = task_ran.clone();
            let executor = cx.background_executor.clone();
            cx.update(|cx| {
                cx.spawn(async move |_| {
                    executor.timer(Duration::from_millis(500)).await;
                    *task_ran.borrow_mut() = true;
                })
                .detach();
            });
        }

        // 运行直到挂起 - 任务应该正在等待计时器
        cx.run_until_parked();
        assert!(!*task_ran.borrow());

        // 将时钟推进到超过计时器时长
        cx.advance_clock(Duration::from_millis(600));

        // 现在任务应该已完成
        assert!(*task_ran.borrow());
    }

    #[test]
    #[ignore] // 需要 macOS 主线程 - 在测试线程上窗口创建会失败
    fn test_window_spawn_uses_test_dispatcher() {
        let mut cx = VisualTestAppContext::new(current_platform(false));

        let task_ran = Rc::new(RefCell::new(false));

        let window = cx
            .open_offscreen_window_default(|_, cx| cx.new(|_| Empty))
            .expect("打开窗口失败");

        // 通过 window.spawn 生成任务 - 这是工具提示行为的关键测试用例
        // 因为工具提示使用 window.spawn 进行延迟显示
        {
            let task_ran = task_ran.clone();
            cx.update_window(window.into(), |_, window, cx| {
                window
                    .spawn(cx, async move |_| {
                        *task_ran.borrow_mut() = true;
                    })
                    .detach();
            })
            .ok();
        }

        // 任务尚未执行
        assert!(!*task_ran.borrow());

        // run_until_parked 应该执行通过 window 生成的前台任务
        cx.run_until_parked();

        // 现在任务应该已执行
        assert!(*task_ran.borrow());
    }
}
