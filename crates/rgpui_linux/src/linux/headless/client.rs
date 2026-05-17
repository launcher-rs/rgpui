//! 无头（headless）模式客户端实现
//!
//! 在无图形界面环境下提供平台抽象，不支持窗口创建

use std::cell::RefCell;
use std::rc::Rc;

use calloop::{EventLoop, LoopHandle};
use util::ResultExt;

use crate::linux::{LinuxClient, LinuxCommon, LinuxKeyboardLayout};
use rgpui::{
    AnyWindowHandle, CursorStyle, DisplayId, PlatformDisplay, PlatformKeyboardLayout,
    PlatformWindow, WindowParams,
};

/// 无头客户端状态
pub struct HeadlessClientState {
    pub(crate) _loop_handle: LoopHandle<'static, HeadlessClient>,
    pub(crate) event_loop: Option<calloop::EventLoop<'static, HeadlessClient>>,
    pub(crate) common: LinuxCommon,
}

/// 无头客户端实现
#[derive(Clone)]
pub(crate) struct HeadlessClient(Rc<RefCell<HeadlessClientState>>);

impl HeadlessClient {
    /// 创建新的无头客户端实例
    pub(crate) fn new() -> Self {
        let event_loop = EventLoop::try_new().unwrap();

        let (common, main_receiver) = LinuxCommon::new(event_loop.get_signal());

        let handle = event_loop.handle();

        handle
            .insert_source(main_receiver, |event, _, _: &mut HeadlessClient| {
                if let calloop::channel::Event::Msg(runnable) = event {
                    runnable.run();
                }
            })
            .ok();

        HeadlessClient(Rc::new(RefCell::new(HeadlessClientState {
            event_loop: Some(event_loop),
            _loop_handle: handle,
            common,
        })))
    }
}

impl LinuxClient for HeadlessClient {
    fn with_common<R>(&self, f: impl FnOnce(&mut LinuxCommon) -> R) -> R {
        f(&mut self.0.borrow_mut().common)
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(LinuxKeyboardLayout::new("unknown".into()))
    }

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        vec![]
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        None
    }

    fn display(&self, _id: DisplayId) -> Option<Rc<dyn PlatformDisplay>> {
        None
    }

    #[cfg(feature = "screen-capture")]
    fn screen_capture_sources(
        &self,
    ) -> futures::channel::oneshot::Receiver<anyhow::Result<Vec<Rc<dyn rgpui::ScreenCaptureSource>>>>
    {
        let (tx, rx) = futures::channel::oneshot::channel();
        tx.send(Err(anyhow::anyhow!(
            "Headless mode does not support screen capture."
        )))
        .ok();
        rx
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        None
    }

    fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        None
    }

    fn open_window(
        &self,
        _handle: AnyWindowHandle,
        _params: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>> {
        anyhow::bail!("neither DISPLAY nor WAYLAND_DISPLAY is set. You can run in headless mode");
    }

    fn compositor_name(&self) -> &'static str {
        "headless"
    }

    fn set_cursor_style(&self, _style: CursorStyle) {}

    fn open_uri(&self, _uri: &str) {}

    fn reveal_path(&self, _path: std::path::PathBuf) {}

    fn write_to_primary(&self, _item: rgpui::ClipboardItem) {}

    fn write_to_clipboard(&self, _item: rgpui::ClipboardItem) {}

    fn read_from_primary(&self) -> Option<rgpui::ClipboardItem> {
        None
    }

    fn read_from_clipboard(&self) -> Option<rgpui::ClipboardItem> {
        None
    }

    fn run(&self) {
        let mut event_loop = self
            .0
            .borrow_mut()
            .event_loop
            .take()
            .expect("App is already running");

        event_loop.run(None, &mut self.clone(), |_| {}).log_err();
    }
}
