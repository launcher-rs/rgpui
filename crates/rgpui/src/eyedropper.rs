//! 屏幕取色（吸管）：抓取主屏首帧 → 全屏冻结浮层（十字线 + 放大镜）→ 点击回传颜色。
//!
//! 由 `screen-capture` feature 门控：底层走 [`App::screen_capture_sources`]
//! （Windows / Linux 为 `scap` 后端，macOS 为 core-video 实现）。
//! rgpui 的屏幕捕获只有持续推流接口，没有单帧 API，因此「截一张图」实现为：
//! 启动推流 → 取第一帧的 RGBA → 立即停流；全程**留在内存**，不落盘。
//!
//! 典型用法（配合 [`crate::components::ColorPickerState`] 的吸管按钮）：
//!
//! ```ignore
//! use rgpui::eyedropper::{EyedropperResult, open_eyedropper};
//!
//! open_eyedropper(cx, move |result, cx| match result {
//!     EyedropperResult::Picked(color) => { /* 应用颜色 */ }
//!     EyedropperResult::Cancelled => {}
//!     EyedropperResult::Failed(reason) => { /* 提示 reason */ }
//! });
//! ```
//!
//! 与区域截图同一帧来源（参考实现：rucap）：看到的是截取瞬间的静态首帧
//! （1:1 物理像素），采样坐标严格按「视图逻辑像素 × 缩放比 = 图片物理像素」换算。

use crate::{
    App, AppContext as _, Bounds, Context, FocusHandle, Hsla, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement as _, Pixels, Point,
    Render, RenderImage, Rgba, ScreenCaptureFrame, ScreenCaptureSource, SharedString, Styled as _,
    Window, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions, canvas, div, fill,
    img, point, px, rgb, rgba, size,
};
use image::RgbaImage;
use std::{
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

/// 放大镜边长像素数（奇数，保证中心唯一）。
const MAG_N: i32 = 9;
/// 放大镜单个图片像素的显示边长（逻辑像素）。
const MAG_Z: f32 = 14.0;
/// 放大镜网格总边长（逻辑像素）。
const MAG_W: f32 = MAG_N as f32 * MAG_Z;
/// 放大镜面板宽：网格 + 左右内边距。
const MAG_PANEL_W: f32 = MAG_W + 16.0;
/// 放大镜面板高：网格 + 内边距 + 底部坐标文字行（约 16）。
const MAG_PANEL_H: f32 = MAG_W + 36.0;

/// 取色结果回调（在主线程的 `App` 上下文中调用，恰好一次）。
type OnResult = Arc<dyn Fn(EyedropperResult, &mut App)>;

/// 取色结果（`on_result` 恰好回传一次）。
pub enum EyedropperResult {
    /// 选取了颜色。
    Picked(Hsla),
    /// 用户取消（`Esc` / 右键）。
    Cancelled,
    /// 启动失败，附中文原因（不支持 / 无源 / 超时 / 开窗失败等）。
    Failed(SharedString),
}

/// 打开全屏取色浮层：抓取主屏首帧并冻结，移动光标采样，左键确认、右键 / `Esc` 取消。
///
/// # 参数
/// - `cx`: 应用上下文（异步捕获流程在后台推进，浮层开窗回到主线程）。
/// - `on_result`: 结果回调，在主线程的 `App` 上下文中调用，恰好一次
///   （取消 / 失败同样回调，不会再触发 `Picked`）。
pub fn open_eyedropper(cx: &mut App, on_result: impl Fn(EyedropperResult, &mut App) + 'static) {
    // 环境不支持屏幕捕获（未启用平台后端 / 无权限）时直接失败，不进入异步流程。
    if !cx.is_screen_capture_supported() {
        (on_result)(
            EyedropperResult::Failed("当前环境不支持屏幕捕获".into()),
            cx,
        );
        return;
    }

    // 浮层窗口尺寸取主显示器边界，取不到回退默认。
    let display_bounds: Bounds<Pixels> = match cx.primary_display() {
        Some(display) => display.bounds(),
        None => Bounds::new(point(px(0.0), px(0.0)), size(px(1280.0), px(800.0))),
    };

    // 捕获源枚举是异步 oneshot：在进入异步闭包前借 App 拿到接收端与执行器句柄。
    let sources_rx = cx.screen_capture_sources();
    let foreground = cx.foreground_executor().clone();
    let background = cx.background_executor().clone();
    let on_result: OnResult = Arc::new(on_result);

    cx.spawn(async move |cx| {
        // 等待系统返回可捕获的屏幕 / 窗口列表。
        let sources: Vec<Rc<dyn ScreenCaptureSource>> = match sources_rx.await {
            Ok(Ok(sources)) if !sources.is_empty() => sources,
            _ => {
                report(
                    &on_result,
                    cx,
                    EyedropperResult::Failed("无法获取屏幕捕获源".into()),
                );
                return;
            }
        };

        // 优先选择主显示器，其次取第一个源，保证点一次按钮必有图。
        let source = pick_main_source(&sources).unwrap_or_else(|| sources[0].clone());

        // 首帧槽位与「已捕获」标志（捕获线程写、主线程读）。
        let slot: Arc<Mutex<Option<Arc<RgbaImage>>>> = Arc::new(Mutex::new(None));
        let flag = Arc::new(AtomicBool::new(false));

        let slot_for_cb = slot.clone();
        let flag_for_cb = flag.clone();
        // 推流回调跑在捕获线程，只能做 Send 操作：转 RGBA 存入槽位（仅留首帧）。
        let frame_callback: Box<dyn Fn(ScreenCaptureFrame) + Send> = Box::new(move |frame| {
            // 只要第一帧，后续帧直接丢弃。
            if flag_for_cb.load(Ordering::Relaxed) {
                return;
            }
            if let Some(rgba) = frame.to_rgba()
                && let Ok(mut guard) = slot_for_cb.lock()
            {
                *guard = Some(Arc::new(rgba));
                flag_for_cb.store(true, Ordering::Relaxed);
            }
        });

        // 推流必须在前台执行器上发起（rgpui 约束）。
        let stream_rx = source.stream(&foreground, frame_callback);

        // 等待推流真正启动，失败则直接回状态。
        let stream = match stream_rx.await {
            Ok(Ok(stream)) => stream,
            _ => {
                report(
                    &on_result,
                    cx,
                    EyedropperResult::Failed("屏幕推流启动失败".into()),
                );
                return;
            }
        };

        // 最多等待 5 秒首帧；超时一般是目标屏幕静止无帧或权限不足。
        let mut waited = 0;
        while !flag.load(Ordering::Relaxed) && waited < 100 {
            background.timer(Duration::from_millis(50)).await;
            waited += 1;
        }
        // 显式释放推流，会话随之关闭（首帧已到手，无需继续）。
        drop(stream);

        if !flag.load(Ordering::Relaxed) {
            report(
                &on_result,
                cx,
                EyedropperResult::Failed("获取首帧超时".into()),
            );
            return;
        }
        let Some(rgba) = slot.lock().ok().and_then(|mut guard| guard.take()) else {
            report(
                &on_result,
                cx,
                EyedropperResult::Failed("首帧数据不可用".into()),
            );
            return;
        };
        let (img_w, img_h) = (rgba.width(), rgba.height());

        // 解码为可直接渲染的 `RenderImage`（与 rgpui img 解码一致：RGBA 转 BGRA）。
        // 开窗前做掉解码耗时，浮层首帧立现，不会出现先透活屏再变灰的闪烁。
        let mut raw = rgba.as_raw().clone();
        let (chunks, _) = raw.as_chunks_mut::<4>();
        for pixel in chunks {
            pixel.swap(0, 2);
        }
        let bgra = image::RgbaImage::from_raw(img_w, img_h, raw)
            .unwrap_or_else(|| image::RgbaImage::new(1, 1));
        let preview = Arc::new(RenderImage::new(vec![image::Frame::new(bgra)]));

        // 回到主线程开浮层。成功后结果经浮层点击 / 取消时分发，异步流程不再回传。
        let view_w = display_bounds.size.width.as_f32();
        let view_h = display_bounds.size.height.as_f32();
        let on_result_for_view = on_result.clone();
        let opened = cx.update(|app| {
            let options = WindowOptions {
                // 无边框置顶浮层；Windows 下必须 Fullscreen。
                // 背景取 Opaque：底色是铺满全屏的冻结截图，不需要窗口级 alpha，
                // 且 X11 上 ARGB visual 在 alpha 合成不可用时会让浮层整体不可见。
                titlebar: None,
                window_bounds: Some(WindowBounds::Fullscreen(display_bounds)),
                window_background: WindowBackgroundAppearance::Opaque,
                kind: WindowKind::Overlay,
                focus: true,
                show: true,
                ..Default::default()
            };
            app.open_window(options, move |window, app| {
                let view = app.new(|cx| EyedropperView {
                    img: rgba.clone(),
                    preview,
                    img_w,
                    img_h,
                    view_w,
                    view_h,
                    cursor: None,
                    focus_handle: cx.focus_handle(),
                    on_result: on_result_for_view,
                    finished: false,
                });
                // 打开即聚焦根视图，使 Esc 立即可用。
                let handle = view.read(app).focus_handle.clone();
                window.focus(&handle, app);
                view
            })
        });
        if opened.is_ok() {
            // 成功：结果经浮层点击 / 取消时分发，异步流程到此为止。
        } else {
            report(
                &on_result,
                cx,
                EyedropperResult::Failed("取色浮层打开失败".into()),
            );
        }
    })
    .detach();
}

/// 在异步上下文回传一次结果（来源窗口已关闭等场景下静默放弃）。
fn report(on_result: &OnResult, cx: &mut crate::AsyncApp, result: EyedropperResult) {
    cx.update(|app| (on_result)(result, app));
}

/// 从候选源中挑选主显示器对应的捕获源（找不到 `is_main` 时回 `None`，由调用方回退）。
fn pick_main_source(
    sources: &[Rc<dyn ScreenCaptureSource>],
) -> Option<Rc<dyn ScreenCaptureSource>> {
    for source in sources {
        if source
            .metadata()
            .map(|meta| meta.is_main.unwrap_or(false))
            .unwrap_or(false)
        {
            return Some(source.clone());
        }
    }
    None
}

/// 取色浮层视图：冻结帧 + 十字线 + 放大镜，点击回传颜色，右键 / `Esc` 取消。
struct EyedropperView {
    /// 采样用底图像素（物理像素，1:1 与屏幕对应）。
    img: Arc<RgbaImage>,
    /// 冻结帧预览（BGRA，铺满浮层背景）。
    preview: Arc<RenderImage>,
    /// 全图宽（图片物理像素）。
    img_w: u32,
    /// 全图高（图片物理像素）。
    img_h: u32,
    /// 浮层逻辑宽（显示器逻辑像素，鼠标坐标空间）。
    view_w: f32,
    /// 浮层逻辑高。
    view_h: f32,
    /// 当前鼠标位置（视图逻辑像素），`None` 表示尚未移入。
    cursor: Option<Point<Pixels>>,
    /// 根视图焦点（`Esc` 取消用）。
    focus_handle: FocusHandle,
    /// 结果回调（点击 / 取消时各恰好一次）。
    on_result: OnResult,
    /// 结果是否已回传（防止重复触发）。
    finished: bool,
}

impl EyedropperView {
    /// 视图逻辑像素 → 图片物理像素的缩放比。
    fn scale(&self) -> (f32, f32) {
        (
            self.img_w as f32 / self.view_w.max(1.0),
            self.img_h as f32 / self.view_h.max(1.0),
        )
    }

    /// 当前光标处的采样颜色（越界 / 未移入返回 `None`）。
    fn sample(&self) -> Option<(u8, u8, u8)> {
        let cursor = self.cursor?;
        let (sx, sy) = self.scale();
        let x = (cursor.x.as_f32() * sx).round() as i32;
        let y = (cursor.y.as_f32() * sy).round() as i32;
        sample_at(&self.img, x, y)
    }

    /// 左键点击：回传当前颜色并关闭浮层。
    fn on_pick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.finished {
            return;
        }
        self.finished = true;
        if let Some((r, g, b)) = self.sample() {
            let color: Hsla = rgb(((r as u32) << 16) | ((g as u32) << 8) | b as u32).into();
            (self.on_result)(EyedropperResult::Picked(color), cx);
        } else {
            (self.on_result)(EyedropperResult::Cancelled, cx);
        }
        window.remove_window();
        // 结果回调更新的是来源窗口实体，浮层关闭后其重画请求可能丢失，强制刷新落定。
        cx.refresh_windows();
    }

    /// 取消取色：回传取消并关闭浮层。
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.finished {
            return;
        }
        self.finished = true;
        (self.on_result)(EyedropperResult::Cancelled, cx);
        window.remove_window();
        cx.refresh_windows();
    }
}

impl Render for EyedropperView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 顶栏提示 + 当前颜色（未采到色时显示占位）。
        let (hex, rgb_text, swatch) = match self.sample() {
            Some((r, g, b)) => (
                rgb_to_hex(r, g, b),
                format!("RGB({r}, {g}, {b})"),
                Rgba {
                    r: r as f32 / 255.0,
                    g: g as f32 / 255.0,
                    b: b as f32 / 255.0,
                    a: 1.0,
                },
            ),
            None => (
                "未采样".to_string(),
                "移动选择颜色".to_string(),
                Rgba {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                },
            ),
        };

        let mut root = div()
            .size_full()
            .relative()
            // 冻结帧背景：1:1 铺满浮层（图片物理像素与视图逻辑像素经缩放比对齐）。
            .child(
                div().absolute().size_full().child(
                    img(self.preview.clone())
                        .w(px(self.view_w))
                        .h(px(self.view_h)),
                ),
            )
            // 根视图可聚焦，使 Esc 立即可用。
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                if ev.keystroke.key.as_str() == "escape" {
                    this.cancel(window, cx);
                }
            }))
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
                this.cursor = Some(ev.position);
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseDownEvent, window, cx| {
                    this.on_pick(window, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _ev: &MouseDownEvent, window, cx| {
                    this.cancel(window, cx);
                }),
            )
            // 顶部信息条：色块 + HEX + RGB。
            .child(
                div()
                    .absolute()
                    .left(px(12.0))
                    .top(px(12.0))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x000000))
                    .child(div().w(px(22.0)).h(px(22.0)).rounded_sm().bg(swatch))
                    .child(div().text_sm().text_color(rgb(0xFFFFFF)).child(hex))
                    .child(div().text_xs().text_color(rgb(0xBBBBBB)).child(rgb_text)),
            );

        // 十字线 + 放大镜（有光标时才画）。
        if let Some(cursor) = self.cursor {
            let (mx, my) = (cursor.x.as_f32(), cursor.y.as_f32());
            let (sx, sy) = self.scale();
            let cx_img = (mx * sx).round() as i32;
            let cy_img = (my * sy).round() as i32;
            // 放大镜面板贴在光标右下方，越界则翻到左上方。
            let mut panel_x = mx + 24.0;
            let mut panel_y = my + 24.0;
            if panel_x + MAG_PANEL_W + 16.0 > self.view_w {
                panel_x = mx - 24.0 - MAG_PANEL_W - 16.0;
            }
            if panel_y + MAG_PANEL_H + 16.0 > self.view_h {
                panel_y = my - 24.0 - MAG_PANEL_H - 16.0;
            }
            // 9×9 格颜色提前算好，画布闭包只做绘制（闭包需 'static，用 owned 数据）。
            let mut cells = Vec::with_capacity((MAG_N * MAG_N) as usize);
            let half = MAG_N / 2;
            for dy in -half..=half {
                for dx in -half..=half {
                    cells.push(mag_cell(&self.img, cx_img, cy_img, dx, dy));
                }
            }
            root = root
                // 水平十字线。
                .child(
                    div()
                        .absolute()
                        .left(px(0.0))
                        .top(cursor.y)
                        .w(px(self.view_w))
                        .h(px(1.0))
                        .bg(rgba(0xFFFFFF80)),
                )
                // 垂直十字线。
                .child(
                    div()
                        .absolute()
                        .left(cursor.x)
                        .top(px(0.0))
                        .w(px(1.0))
                        .h(px(self.view_h))
                        .bg(rgba(0xFFFFFF80)),
                )
                // 放大镜面板。
                .child(
                    div()
                        .absolute()
                        .left(px(panel_x))
                        .top(px(panel_y))
                        .w(px(MAG_PANEL_W))
                        .rounded_md()
                        .bg(rgb(0x000000))
                        .p_2()
                        .child(
                            canvas(
                                move |_, _, _| (),
                                move |bounds, _, window, _| {
                                    let origin = bounds.origin;
                                    let n = MAG_N as usize;
                                    for (i, color) in cells.iter().enumerate() {
                                        let gx = (i % n) as f32;
                                        let gy = (i / n) as f32;
                                        let cell_bounds = Bounds::new(
                                            point(
                                                px(origin.x.as_f32() + gx * MAG_Z),
                                                px(origin.y.as_f32() + gy * MAG_Z),
                                            ),
                                            size(px(MAG_Z), px(MAG_Z)),
                                        );
                                        window.paint_quad(fill(cell_bounds, *color));
                                    }
                                    // 中心格白色描边（2px 四条）。
                                    let center = MAG_N as f32 / 2.0 * MAG_Z;
                                    let ox = origin.x.as_f32();
                                    let oy = origin.y.as_f32();
                                    let white = Rgba {
                                        r: 1.0,
                                        g: 1.0,
                                        b: 1.0,
                                        a: 1.0,
                                    };
                                    let bars = [
                                        (center, center, MAG_Z, 2.0),
                                        (center, center + MAG_Z - 2.0, MAG_Z, 2.0),
                                        (center, center, 2.0, MAG_Z),
                                        (center + MAG_Z - 2.0, center, 2.0, MAG_Z),
                                    ];
                                    for (bx, by, bw, bh) in bars {
                                        window.paint_quad(fill(
                                            Bounds::new(
                                                point(px(ox + bx), px(oy + by)),
                                                size(px(bw), px(bh)),
                                            ),
                                            white,
                                        ));
                                    }
                                },
                            )
                            // canvas 不会按绘制内容反推尺寸：不显式给宽高就塌成 0 高，
                            // 9×9 网格会溢出面板，面板只剩顶上一圈黑边（坐标文字也被顶到网格上）。
                            .w(px(MAG_W))
                            .h(px(MAG_W)),
                        )
                        .child(
                            div()
                                .mt_1()
                                .text_xs()
                                .text_center()
                                .text_color(rgb(0xBBBBBB))
                                .child(format!("{cx_img}, {cy_img}")),
                        ),
                );
        }
        root
    }
}

/// 采样图片指定物理像素（越界返回 None）。
fn sample_at(img: &RgbaImage, x: i32, y: i32) -> Option<(u8, u8, u8)> {
    if x < 0 || y < 0 || x >= img.width() as i32 || y >= img.height() as i32 {
        return None;
    }
    let p = img.get_pixel(x as u32, y as u32);
    Some((p[0], p[1], p[2]))
}

/// RGB 转 `#RRGGBB` 大写 HEX。
fn rgb_to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// 放大镜某格的颜色（中心为光标像素，向外扩散，越界按边缘钳制）。
fn mag_cell(img: &RgbaImage, cx: i32, cy: i32, dx: i32, dy: i32) -> Rgba {
    let x = (cx + dx).clamp(0, img.width() as i32 - 1);
    let y = (cy + dy).clamp(0, img.height() as i32 - 1);
    let p = img.get_pixel(x as u32, y as u32);
    Rgba {
        r: p[0] as f32 / 255.0,
        g: p[1] as f32 / 255.0,
        b: p[2] as f32 / 255.0,
        a: 1.0,
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{mag_cell, rgb_to_hex, sample_at};
    use image::RgbaImage;

    /// 采样越界与 HEX 格式断言。
    #[test]
    fn sample_and_hex() {
        let mut img = RgbaImage::new(3, 2);
        img.put_pixel(1, 0, image::Rgba([255, 0, 0, 255]));
        assert_eq!(sample_at(&img, 1, 0), Some((255, 0, 0)));
        assert_eq!(sample_at(&img, 0, 0), Some((0, 0, 0)));
        assert_eq!(sample_at(&img, 3, 0), None);
        assert_eq!(sample_at(&img, -1, 0), None);
        assert_eq!(rgb_to_hex(255, 0, 0), "#FF0000");
        assert_eq!(rgb_to_hex(1, 2, 3), "#010203");
    }

    /// 放大镜越界钳制：角落采样不 panic 且返回边缘色。
    #[test]
    fn mag_cell_clamp() {
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, image::Rgba([10, 20, 30, 255]));
        let c = mag_cell(&img, 0, 0, -4, -4);
        assert!((c.r - 10.0 / 255.0).abs() < 1e-6);
    }
}
