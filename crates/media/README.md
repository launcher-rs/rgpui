# media

macOS 媒体处理 API 的 Rust 绑定。

## 描述

`media` crate 提供了 macOS 平台上 CoreMedia 和 CoreVideo 框架的 Rust 绑定，用于处理视频帧、样本缓冲区、Metal 纹理转换等媒体相关操作。仅在 macOS 平台上可用。

## 主要功能

### `core_media` 模块

- **`CMSampleBuffer`**: 媒体样本缓冲区封装
  - `attachments()` — 获取样本附件数组
  - `image_buffer()` — 获取图像缓冲区
  - `sample_timing_info()` — 获取样本时序信息
  - `format_description()` — 获取格式描述
  - `data()` — 获取数据缓冲区

- **`CMFormatDescription`**: 视频格式描述
  - `h264_parameter_set_count()` — 获取 H.264 参数集数量
  - `h264_parameter_set_at_index()` — 获取指定索引的 H.264 参数集

- **`CMBlockBuffer`**: 块缓冲区
  - `bytes()` — 获取原始字节数据

### `core_video` 模块

- **`CVMetalTextureCache`**: Metal 纹理缓存
  - `new()` — 创建纹理缓存
  - `create_texture_from_image()` — 从图像创建 Metal 纹理

- **`CVMetalTexture`**: Metal 纹理封装
  - `as_texture_ref()` — 获取 Metal 纹理引用

### 支持的像素格式

- `kCVPixelFormatType_32BGRA`
- `kCVPixelFormatType_420YpCbCr8BiPlanarFullRange`
- `kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange`
- `kCVPixelFormatType_420YpCbCr8Planar`

## 使用示例

```rust
#[cfg(target_os = "macos")]
use media::core_media::{CMSampleBuffer, CMFormatDescription};

#[cfg(target_os = "macos")]
use media::core_video::{CVMetalTextureCache, CVMetalTexture};

// 从样本缓冲区获取图像
fn process_sample_buffer(buffer: &CMSampleBuffer) {
    if let Some(image_buffer) = buffer.image_buffer() {
        // 处理图像缓冲区
    }
    
    let timing = buffer.sample_timing_info(0).unwrap();
    let format = buffer.format_description();
    let data = buffer.data().bytes();
}

// 创建 Metal 纹理缓存并转换纹理
#[cfg(target_os = "macos")]
unsafe fn create_metal_texture(
    device: *mut metal::MTLDevice,
    image_buffer: CVImageBufferRef,
) -> anyhow::Result<CVMetalTexture> {
    let cache = CVMetalTextureCache::new(device)?;
    cache.create_texture_from_image(
        image_buffer,
        std::ptr::null(),
        metal::MTLPixelFormat::BGRA8Unorm,
        1920,
        1080,
        0,
    )
}
```

## 依赖

- `anyhow` — 错误处理
- `core-foundation` — Core Foundation 类型封装
- `core-video` — Core Video 绑定
- `metal` — Metal API
- `foreign-types` — 外部类型封装
- `bindgen` — 自动生成 C 绑定（构建依赖）

## 平台支持

仅限 macOS (`target_os = "macos"`)
