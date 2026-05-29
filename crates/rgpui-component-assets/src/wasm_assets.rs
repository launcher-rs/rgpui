use anyhow::anyhow;
use rgpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use wasm_bindgen_futures::spawn_local;

/// WASM 实现 - 按需下载资源
pub struct Assets {
    /// CDN 端点地址
    endpoint: SharedString,
    /// 已下载资源的内存缓存
    cache: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    /// 正在下载中的资源标记
    pending: Arc<RwLock<HashMap<String, bool>>>,
}

impl Assets {
    /// 创建新的 Assets 实例
    ///
    /// # 参数
    ///
    /// * `endpoint` - CDN 端点地址
    pub fn new(endpoint: impl Into<SharedString>) -> Self {
        Self {
            endpoint: endpoint.into(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            pending: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for Assets {
    fn default() -> Self {
        Self::new("")
    }
}

impl AssetSource for Assets {
    /// 加载指定路径的资源
    ///
    /// 对于图标资源（icons/*.svg），如果缓存中存在则直接返回，
    /// 否则发起异步下载请求。下载期间返回错误以触发 GPUI 重试机制。
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        if path.starts_with("icons/") && path.ends_with(".svg") {
            // 检查是否已缓存
            if let Ok(cache) = self.cache.read() {
                if let Some(data) = cache.get(path) {
                    return Ok(Some(Cow::Owned(data.clone())));
                }
            }

            // 检查是否已有下载任务在进行中
            let is_pending = self
                .pending
                .read()
                .map(|p| p.contains_key(path))
                .unwrap_or(false);

            if !is_pending {
                // 标记为待下载并启动下载任务
                if let Ok(mut pending) = self.pending.write() {
                    pending.insert(path.to_string(), true);
                }

                let url = format!("{}/assets/{}", self.endpoint, path);
                let path_clone = path.to_string();
                let cache = self.cache.clone();
                let pending = self.pending.clone();

                spawn_local(async move {
                    match reqwest::get(&url).await {
                        Ok(response) => {
                            if response.status().is_success() {
                                match response.bytes().await {
                                    Ok(bytes) => {
                                        if let Ok(mut cache) = cache.write() {
                                            cache.insert(path_clone.clone(), bytes.to_vec());
                                        }
                                    }
                                    Err(e) => {
                                        log::warn!("读取图标 {} 失败: {}", path_clone, e);
                                    }
                                }
                            } else {
                                log::warn!(
                                    "下载图标 {} 失败: HTTP {}",
                                    path_clone,
                                    response.status()
                                );
                            }
                        }
                        Err(e) => {
                            log::warn!("获取图标 {} 失败: {}", path_clone, e);
                        }
                    }

                    // 从待下载列表中移除
                    if let Ok(mut pending) = pending.write() {
                        pending.remove(&path_clone);
                    }
                });
            }

            // 返回错误以便 GPUI 重试（每个图标只记录一次）
            Err(anyhow!("Wasm 资源加载中，即将可用..."))
        } else {
            Ok(None)
        }
    }

    /// 列出指定路径下的所有资源
    ///
    /// WASM 模式下不支持列出资源，返回空列表。
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let _ = path;
        Ok(Vec::new())
    }
}
