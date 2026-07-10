# Screen Capture Example

基于 rgpui 的屏幕捕获示例程序，支持实时预览、截屏和录屏。

## 用法

```bash
cargo run -p screen_capture
```

需要启用 `screen-capture` feature（如需从根工作区构建，需要先打开该 feature）：

```bash
cargo run -p screen_capture --features rgpui/screen-capture
```

## 功能

| 功能 | 快捷键 | 说明 |
|------|--------|------|
| 实时预览 | — | 捕获源画面实时显示 (~15fps) |
| 截屏 | S | 将当前帧保存为 `screenshot_<ts>.png` |
| 开始录制 | R | 逐帧保存 PNG 到 `recording_<ts>/` 目录 |
| 停止录制 | — | 将帧合成为 GIF，清理中间文件 |
| GIF 速度 | — | 0.5x / 1x / 2x / 4x 四档帧延迟控制 |
| 刷新源 | — | 重新枚举可用捕获源 |
| FPS 显示 | — | 实时帧率计数器 |

## 操作流程

1. 启动后自动枚举屏幕/窗口捕获源
2. 点击来源列表选中要捕获的源
3. 点击 "Start Capture"（或 Space）开始预览
4. 预览中可随时按 S 截屏、按 R 录屏
5. 停止录制后自动合并为 GIF，显示在下方列表中

## 架构

- 捕获线程通过 `ScreenCaptureSource::stream()` 启动
- 帧回调写入共享 `Arc<Mutex<PreviewFrame>>`，UI 轮询线程每 66ms 写入临时 PNG
- 录屏帧直接写入 `frame_XXXXXX.png` 到录制目录
- GIF 合并使用 `image` crate 的 `GifEncoder`
