//! Linux 和 Windows 的屏幕捕获
use crate::{
    DevicePixels, ForegroundExecutor, ScreenCaptureFrame, ScreenCaptureSource, ScreenCaptureStream,
    Size, SourceMetadata, size,
};
use anyhow::{Context as _, Result, anyhow};
use futures::channel::oneshot;
use scap::Target;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{self, AtomicBool};

/// 填充接收器，包含可捕获的屏幕。
///
/// 在 Wayland 上应使用 `scap_default_target_source` 代替，因为 `scap_screen_sources`
/// 不会返回任何结果。
#[allow(dead_code)]
pub fn scap_screen_sources(
    foreground_executor: &ForegroundExecutor,
) -> oneshot::Receiver<Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
    let (sources_tx, sources_rx) = oneshot::channel();
    get_screen_targets(sources_tx);
    to_dyn_screen_capture_sources(sources_rx, foreground_executor)
}

/// 开始捕获默认目标的屏幕，并将单个源填充到接收器中。
/// 屏幕捕获的第一帧用于确定流的大小。
///
/// 在 Wayland（Linux）上，提示用户选择目标，并为其选择填充单个屏幕捕获源。
#[allow(dead_code)]
pub(crate) fn start_scap_default_target_source(
    foreground_executor: &ForegroundExecutor,
) -> oneshot::Receiver<Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
    let (sources_tx, sources_rx) = oneshot::channel();
    start_default_target_screen_capture(sources_tx);
    to_dyn_screen_capture_sources(sources_rx, foreground_executor)
}

struct ScapCaptureSource {
    target: scap::Display,
    size: Size<DevicePixels>,
}

/// 填充发送器，包含可用于捕获的屏幕。
fn get_screen_targets(sources_tx: oneshot::Sender<Result<Vec<ScapCaptureSource>>>) {
    // 由于使用了阻塞 API，使用新线程。
    std::thread::spawn(|| {
        let targets = match scap::get_all_targets() {
            Ok(targets) => targets,
            Err(err) => {
                sources_tx.send(Err(err)).ok();
                return;
            }
        };
        let sources = targets
            .into_iter()
            .filter_map(|target| match target {
                scap::Target::Display(display) => {
                    let size = Size {
                        width: DevicePixels(display.width as i32),
                        height: DevicePixels(display.height as i32),
                    };
                    Some(ScapCaptureSource {
                        target: display,
                        size,
                    })
                }
                scap::Target::Window(_) => None,
            })
            .collect::<Vec<_>>();
        sources_tx.send(Ok(sources)).ok();
    });
}

impl ScreenCaptureSource for ScapCaptureSource {
    fn metadata(&self) -> Result<SourceMetadata> {
        Ok(SourceMetadata {
            resolution: self.size,
            label: Some(self.target.title.clone().into()),
            is_main: None,
            id: self.target.id as u64,
        })
    }

    fn stream(
        &self,
        foreground_executor: &ForegroundExecutor,
        frame_callback: Box<dyn Fn(ScreenCaptureFrame) + Send>,
    ) -> oneshot::Receiver<Result<Box<dyn ScreenCaptureStream>>> {
        let (stream_tx, stream_rx) = oneshot::channel();
        let target = self.target.clone();

        // 由于使用了阻塞 API，使用专用线程。
        std::thread::spawn(move || {
            match new_scap_capturer(Some(scap::Target::Display(target.clone()))) {
                Ok(mut capturer) => {
                    capturer.start_capture();
                    run_capture(capturer, target.clone(), frame_callback, stream_tx);
                }
                Err(e) => {
                    stream_tx.send(Err(e)).ok();
                }
            }
        });

        to_dyn_screen_capture_stream(stream_rx, foreground_executor)
    }
}

struct ScapDefaultTargetCaptureSource {
    // 由 `ScreenCaptureSource::stream` 单次调用填充的发送器。
    stream_call_tx: std::sync::mpsc::SyncSender<(
        // 提供 `ScreenCaptureSource::stream` 的结果。
        oneshot::Sender<Result<ScapStream>>,
        // 帧回调。
        Box<dyn Fn(ScreenCaptureFrame) + Send>,
    )>,
    target: scap::Display,
    size: Size<DevicePixels>,
}

/// 在默认捕获目标上开始屏幕捕获，并将源填充到发送器中。
fn start_default_target_screen_capture(
    sources_tx: oneshot::Sender<Result<Vec<ScapDefaultTargetCaptureSource>>>,
) {
    // 由于使用了阻塞 API，使用专用线程。
    std::thread::spawn(|| {
        let start_result = crate::maybe!({
            let mut capturer = new_scap_capturer(None)?;
            capturer.start_capture();
            let first_frame = capturer
                .get_next_frame()
                .context("Failed to get first frame of screenshare to get the size.")?;
            let size = frame_size(&first_frame);
            let target = capturer
                .target()
                .context("Unable to determine the target display.")?;
            let target = target.clone();
            Ok((capturer, size, target))
        });

        match start_result {
            Ok((capturer, size, Target::Display(display))) => {
                let (stream_call_tx, stream_rx) = std::sync::mpsc::sync_channel(1);
                sources_tx
                    .send(Ok(vec![ScapDefaultTargetCaptureSource {
                        stream_call_tx,
                        size,
                        target: display.clone(),
                    }]))
                    .ok();
                let Ok((stream_tx, frame_callback)) = stream_rx.recv() else {
                    return;
                };
                run_capture(capturer, display, frame_callback, stream_tx);
            }
            Err(e) => {
                sources_tx.send(Err(e)).ok();
            }
            _ => {
                sources_tx
                    .send(Err(anyhow!("The screen capture source is not a display")))
                    .ok();
            }
        }
    });
}

impl ScreenCaptureSource for ScapDefaultTargetCaptureSource {
    fn metadata(&self) -> Result<SourceMetadata> {
        Ok(SourceMetadata {
            resolution: self.size,
            label: None,
            is_main: None,
            id: self.target.id as u64,
        })
    }

    fn stream(
        &self,
        foreground_executor: &ForegroundExecutor,
        frame_callback: Box<dyn Fn(ScreenCaptureFrame) + Send>,
    ) -> oneshot::Receiver<Result<Box<dyn ScreenCaptureStream>>> {
        let (tx, rx) = oneshot::channel();
        match self.stream_call_tx.try_send((tx, frame_callback)) {
            Ok(()) => {}
            Err(std::sync::mpsc::TrySendError::Full((tx, _)))
            | Err(std::sync::mpsc::TrySendError::Disconnected((tx, _))) => {
                // 注意：可以添加对在前一个流结束后再次调用的支持。
                tx.send(Err(anyhow!(
                    "Can't call ScapDefaultTargetCaptureSource::stream multiple times."
                )))
                .ok();
            }
        }
        to_dyn_screen_capture_stream(rx, foreground_executor)
    }
}

fn new_scap_capturer(target: Option<scap::Target>) -> Result<scap::capturer::Capturer> {
    scap::capturer::Capturer::build(scap::capturer::Options {
        fps: 60,
        show_cursor: true,
        show_highlight: true,
        // 注意：实际的帧输出类型可能不同。
        output_type: scap::frame::FrameType::YUVFrame,
        output_resolution: scap::capturer::Resolution::Captured,
        crop_area: None,
        target,
        excluded_targets: None,
    })
}

fn run_capture(
    mut capturer: scap::capturer::Capturer,
    display: scap::Display,
    frame_callback: Box<dyn Fn(ScreenCaptureFrame) + Send>,
    stream_tx: oneshot::Sender<Result<ScapStream>>,
) {
    let cancel_stream = Arc::new(AtomicBool::new(false));
    let size = Size {
        width: DevicePixels(display.width as i32),
        height: DevicePixels(display.height as i32),
    };
    let stream_send_result = stream_tx.send(Ok(ScapStream {
        cancel_stream: cancel_stream.clone(),
        display,
        size,
    }));
    if stream_send_result.is_err() {
        return;
    }
    while !cancel_stream.load(std::sync::atomic::Ordering::SeqCst) {
        match capturer.get_next_frame() {
            Ok(frame) => frame_callback(ScreenCaptureFrame(frame)),
            Err(err) => {
                log::error!("Halting screen capture due to error: {err}");
                break;
            }
        }
    }
    capturer.stop_capture();
}

struct ScapStream {
    cancel_stream: Arc<AtomicBool>,
    display: scap::Display,
    size: Size<DevicePixels>,
}

impl ScreenCaptureStream for ScapStream {
    fn metadata(&self) -> Result<SourceMetadata> {
        Ok(SourceMetadata {
            resolution: self.size,
            label: Some(self.display.title.clone().into()),
            is_main: None,
            id: self.display.id as u64,
        })
    }
}

impl Drop for ScapStream {
    fn drop(&mut self) {
        self.cancel_stream.store(true, atomic::Ordering::SeqCst);
    }
}

fn frame_size(frame: &scap::frame::Frame) -> Size<DevicePixels> {
    let (width, height) = match frame {
        scap::frame::Frame::YUVFrame(frame) => (frame.width, frame.height),
        scap::frame::Frame::RGB(frame) => (frame.width, frame.height),
        scap::frame::Frame::RGBx(frame) => (frame.width, frame.height),
        scap::frame::Frame::XBGR(frame) => (frame.width, frame.height),
        scap::frame::Frame::BGRx(frame) => (frame.width, frame.height),
        scap::frame::Frame::BGR0(frame) => (frame.width, frame.height),
        scap::frame::Frame::BGRA(frame) => (frame.width, frame.height),
    };
    size(DevicePixels(width), DevicePixels(height))
}

/// 此函数由 `get_screen_targets` 和 `start_default_target_screen_capture` 使用，将它们的
/// 结果转换为 `Rc<dyn ScreenCaptureSource>`。它们需要 `Send` 其捕获源，因此
/// 捕获源结构体用作 `Rc<dyn ScreenCaptureSource>` 不是 `Send` 的。
fn to_dyn_screen_capture_sources<T: ScreenCaptureSource + 'static>(
    sources_rx: oneshot::Receiver<Result<Vec<T>>>,
    foreground_executor: &ForegroundExecutor,
) -> oneshot::Receiver<Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
    let (dyn_sources_tx, dyn_sources_rx) = oneshot::channel();
    foreground_executor
        .spawn(async move {
            match sources_rx.await {
                Ok(Ok(results)) => dyn_sources_tx
                    .send(Ok(results
                        .into_iter()
                        .map(|source| Rc::new(source) as Rc<dyn ScreenCaptureSource>)
                        .collect::<Vec<_>>()))
                    .ok(),
                Ok(Err(err)) => dyn_sources_tx.send(Err(err)).ok(),
                Err(oneshot::Canceled) => None,
            }
        })
        .detach();
    dyn_sources_rx
}

/// 与上面的 `to_dyn_screen_capture_sources` 目的相同。
fn to_dyn_screen_capture_stream<T: ScreenCaptureStream + 'static>(
    sources_rx: oneshot::Receiver<Result<T>>,
    foreground_executor: &ForegroundExecutor,
) -> oneshot::Receiver<Result<Box<dyn ScreenCaptureStream>>> {
    let (dyn_sources_tx, dyn_sources_rx) = oneshot::channel();
    foreground_executor
        .spawn(async move {
            match sources_rx.await {
                Ok(Ok(stream)) => dyn_sources_tx
                    .send(Ok(Box::new(stream) as Box<dyn ScreenCaptureStream>))
                    .ok(),
                Ok(Err(err)) => dyn_sources_tx.send(Err(err)).ok(),
                Err(oneshot::Canceled) => None,
            }
        })
        .detach();
    dyn_sources_rx
}
