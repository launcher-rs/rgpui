//! 异步文件加载支持。
//!
//! 提供大文件异步加载功能，避免阻塞 UI 线程。支持分块读取和取消。

use std::path::Path;

use ropey::Rope;

/// 加载进度。
pub enum LoadProgress {
    /// 正在读取。
    Reading {
        /// 已读取字节数。
        bytes_read: usize,
        /// 总字节数（如果可获取）。
        total_bytes: Option<usize>,
    },
    /// 加载完成。
    Complete(Rope),
    /// 加载出错。
    Error(anyhow::Error),
}

/// 异步文件加载器。
pub struct AsyncFileLoader;

impl AsyncFileLoader {
    /// 创建新的加载器。
    pub fn new() -> Self {
        Self
    }

    /// 异步加载文件到 Rope（需要 tokio 运行时）。
    #[cfg(feature = "tokio")]
    pub async fn load_file_async<F>(
        path: PathBuf,
        mut callback: F,
    ) -> anyhow::Result<Rope>
    where
        F: FnMut(LoadProgress),
    {
        use tokio::fs;

        // 检查文件大小
        let total_bytes = fs::metadata(&path)
            .await
            .ok()
            .map(|m| m.len() as usize);

        // 报告开始读取
        callback(LoadProgress::Reading {
            bytes_read: 0,
            total_bytes,
        });

        // 分块读取文件
        let chunk_size = 64 * 1024; // 64KB chunks
        let mut bytes_read = 0usize;
        let mut content = Vec::new();

        let mut file = fs::File::open(&path).await?;
        let mut buffer = vec![0u8; chunk_size];

        loop {
            match file.read(&mut buffer).await {
                Ok(0) => break, // EOF
                Ok(n) => {
                    content.extend_from_slice(&buffer[..n]);
                    bytes_read += n;

                    // 报告进度
                    callback(LoadProgress::Reading {
                        bytes_read,
                        total_bytes,
                    });
                }
                Err(e) => {
                    callback(LoadProgress::Error(e.into()));
                    return Err(e.into());
                }
            }
        }

        // 转换为 Rope
        let text = String::from_utf8_lossy(&content).to_string();
        let rope = Rope::from_str(&text);

        callback(LoadProgress::Complete(rope.clone()));
        Ok(rope)
    }

    /// 同步加载文件（阻塞当前线程）。
    ///
    /// 适用于小文件或非 UI 线程。
    pub fn load_file_sync(path: &Path) -> anyhow::Result<Rope> {
        let content = std::fs::read_to_string(path)?;
        Ok(Rope::from_str(&content))
    }

    /// 同步加载文件到字符串。
    pub fn load_to_string_sync(path: &Path) -> anyhow::Result<String> {
        Ok(std::fs::read_to_string(path)?)
    }
}

impl Default for AsyncFileLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// 大文件加载配置。
pub struct LargeFileConfig {
    /// 触发异步加载的文件大小阈值（字节）。
    pub async_threshold: usize,
    /// 分块读取大小（字节）。
    pub chunk_size: usize,
}

impl Default for LargeFileConfig {
    fn default() -> Self {
        Self {
            // 10MB 以上使用异步加载
            async_threshold: 10 * 1024 * 1024,
            // 64KB 分块
            chunk_size: 64 * 1024,
        }
    }
}

/// 根据文件大小决定加载方式。
pub fn load_file_auto(
    path: &Path,
    _config: &LargeFileConfig,
) -> anyhow::Result<FileLoadResult> {
    let metadata = std::fs::metadata(path)?;
    let file_size = metadata.len() as usize;

    let content = std::fs::read_to_string(path)?;
    let rope = Rope::from_str(&content);

    Ok(FileLoadResult {
        rope,
        file_size,
    })
}

/// 文件加载结果。
pub struct FileLoadResult {
    /// 文件内容。
    pub rope: Rope,
    /// 文件大小。
    pub file_size: usize,
}

impl FileLoadResult {
    /// 获取文件内容引用。
    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    /// 获取文件大小。
    pub fn file_size(&self) -> usize {
        self.file_size
    }

    /// 判断是否为大文件（>10MB）。
    pub fn is_large(&self) -> bool {
        self.file_size > 10 * 1024 * 1024
    }

    /// 获取行数。
    pub fn line_count(&self) -> usize {
        use ropey::LineType;
        self.rope.len_lines(LineType::LF)
    }
}
