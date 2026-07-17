use anyhow::anyhow;
use rgpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

/// 原生平台实现 - 使用 RustEmbed 嵌入资源
#[derive(rust_embed::RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
pub struct Assets;

impl Assets {
    /// 创建新的 Assets 实例
    ///
    /// # 参数
    ///
    /// * `_endpoint` - CDN 端点地址（原生构建中忽略此参数）
    pub fn new(_endpoint: impl Into<SharedString>) -> Self {
        Self
    }
}

impl AssetSource for Assets {
    /// 加载指定路径的资源
    ///
    /// # 参数
    ///
    /// * `path` - 资源路径
    ///
    /// # 返回值
    ///
    /// 成功时返回资源数据的 `Cow`，如果路径为空或资源不存在则返回错误。
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        Self::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow!("无法找到路径为 \"{}\" 的资源", path))
    }

    /// 列出指定路径下的所有资源
    ///
    /// # 参数
    ///
    /// * `path` - 要列出的路径前缀
    ///
    /// # 返回值
    ///
    /// 返回所有以指定路径开头的资源路径列表。
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect())
    }
}
