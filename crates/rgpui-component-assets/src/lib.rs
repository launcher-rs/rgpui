/// 为 GPUI Component 嵌入应用资源。
///
/// 本模块为 [IconName](https://docs.rs/gpui-component/latest/gpui_component/enum.IconName.html) 提供图标 SVG 文件。
///
/// ## 用法
///
/// ```rust,no_run
/// use rgpui::*;
/// use rgpui_component_assets::Assets;
///
/// let app = gpui_platform::application().with_assets(Assets);
/// ```
///
/// ## 平台差异
///
/// - **原生（桌面端）**: 图标使用 RustEmbed 嵌入到二进制文件中
/// - **WASM（Web 端）**: 图标通过 web_sys::Request 从 CDN 下载
///   - 这显著减小了 WASM 包体积
///   - 图标在首次使用时按需下载
///   - 下载的图标会在内存中缓存
#[cfg(not(target_family = "wasm"))]
mod native_assets;

#[cfg(target_family = "wasm")]
mod wasm_assets;

#[cfg(not(target_family = "wasm"))]
pub use native_assets::Assets;

#[cfg(target_family = "wasm")]
pub use wasm_assets::Assets;
