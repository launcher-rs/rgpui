//! 文件监视 API —— 监控文件或目录的变化。
//!
//! # 示例
//!
//! ```rust,ignore
//! use rgpui::file_watcher::{FileWatcher, FileEvent};
//!
//! let mut watcher = FileWatcher::new();
//! watcher.watch("src/main.rs", |event| {
//!     match event {
//!         FileEvent::Modified(path) => println!("文件修改: {:?}", path),
//!         FileEvent::Created(path) => println!("文件创建: {:?}", path),
//!         FileEvent::Deleted(path) => println!("文件删除: {:?}", path),
//!     }
//! });
//! ```

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(feature = "tokio")]
use std::sync::Mutex;

/// 文件事件类型。
#[derive(Debug, Clone)]
pub enum FileEvent {
    /// 文件被修改。
    Modified(PathBuf),
    /// 文件被创建。
    Created(PathBuf),
    /// 文件被删除。
    Deleted(PathBuf),
    /// 文件被重命名。
    Renamed {
        /// 旧路径。
        from: PathBuf,
        /// 新路径。
        to: PathBuf,
    },
    /// 访问权限变化。
    AccessChanged(PathBuf),
}

/// 文件监视器配置。
pub struct FileWatcherConfig {
    /// 递归监视子目录。
    pub recursive: bool,
    /// 轮询间隔（毫秒）。
    pub poll_interval_ms: u64,
    /// 忽略的路径模式。
    pub ignore_patterns: Vec<String>,
}

impl Default for FileWatcherConfig {
    fn default() -> Self {
        Self {
            recursive: false,
            poll_interval_ms: 500,
            ignore_patterns: vec![
                ".git".into(),
                "node_modules".into(),
                "target".into(),
                ".DS_Store".into(),
            ],
        }
    }
}

/// 文件监视器。
pub struct FileWatcher {
    /// 监视的路径 → 回调。
    watchers: HashMap<PathBuf, Arc<dyn Fn(FileEvent) + Send + Sync>>,
    /// 配置。
    config: FileWatcherConfig,
    /// 最后已知的文件状态（用于检测变化）。
    last_state: HashMap<PathBuf, std::time::SystemTime>,
}

impl FileWatcher {
    /// 创建新的文件监视器。
    pub fn new() -> Self {
        Self {
            watchers: HashMap::new(),
            config: FileWatcherConfig::default(),
            last_state: HashMap::new(),
        }
    }

    /// 创建带有自定义配置的文件监视器。
    pub fn with_config(config: FileWatcherConfig) -> Self {
        Self {
            watchers: HashMap::new(),
            config,
            last_state: HashMap::new(),
        }
    }

    /// 监视文件或目录。
    pub fn watch<F>(&mut self, path: impl AsRef<Path>, callback: F)
    where
        F: Fn(FileEvent) + Send + Sync + 'static,
    {
        let path = path.as_ref().to_path_buf();
        self.watchers.insert(path, Arc::new(callback));
    }

    /// 停止监视。
    pub fn unwatch(&mut self, path: &Path) {
        self.watchers.remove(path);
    }

    /// 检查文件变化（轮询模式）。
    pub fn check_changes(&mut self) -> Vec<FileEvent> {
        let mut events = Vec::new();

        for (path, callback) in &self.watchers {
            if let Ok(metadata) = std::fs::metadata(path) {
                if let Ok(modified) = metadata.modified() {
                    let last = self.last_state.get(path).cloned();
                    self.last_state.insert(path.clone(), modified);

                    if let Some(last_time) = last {
                        if modified > last_time {
                            let event = FileEvent::Modified(path.clone());
                            callback(event.clone());
                            events.push(event);
                        }
                    } else {
                        // 首次监视，记录状态
                    }
                }
            } else {
                // 文件不存在，可能被删除
                if self.last_state.contains_key(path) {
                    let event = FileEvent::Deleted(path.clone());
                    callback(event.clone());
                    events.push(event);
                    self.last_state.remove(path);
                }
            }
        }

        events
    }

    /// 获取监视的路径列表。
    pub fn watched_paths(&self) -> Vec<&Path> {
        self.watchers.keys().map(|p| p.as_path()).collect()
    }

    /// 获取配置引用。
    pub fn config(&self) -> &FileWatcherConfig {
        &self.config
    }
}

impl Default for FileWatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// 异步文件监视器（需要 tokio）。
#[cfg(feature = "tokio")]
pub struct AsyncFileWatcher {
    /// 内部同步监视器。
    inner: Arc<Mutex<FileWatcher>>,
    /// 运行中的监视任务。
    _task: Option<tokio::task::JoinHandle<()>>,
}

#[cfg(feature = "tokio")]
impl AsyncFileWatcher {
    /// 创建新的异步文件监视器。
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(FileWatcher::new())),
            _task: None,
        }
    }

    /// 异步监视文件变化。
    pub async fn watch_async<F>(&self, path: impl AsRef<Path>, callback: F)
    where
        F: Fn(FileEvent) + Send + Sync + 'static,
    {
        let path = path.as_ref().to_path_buf();
        self.inner.lock().unwrap().watch(path, callback);
    }

    /// 启动轮询检查。
    pub fn start_polling(&mut self, interval_ms: u64) {
        let inner = self.inner.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_millis(interval_ms),
            );
            loop {
                interval.tick().await;
                // check_changes() 内部会调用注册的回调派发事件
                let _events = inner.lock().unwrap().check_changes();
            }
        });
        self._task = Some(handle);
    }
}

/// 文件监视工具函数。
pub mod utils {
    use super::*;

    /// 检查文件是否被修改。
    pub fn is_file_modified(path: &Path, last_check: std::time::SystemTime) -> bool {
        std::fs::metadata(path)
            .and_then(|m| m.modified())
            .map(|modified| modified > last_check)
            .unwrap_or(false)
    }

    /// 获取文件最后修改时间。
    pub fn last_modified(path: &Path) -> Option<std::time::SystemTime> {
        std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
    }

    /// 检查路径是否应该被忽略。
    pub fn should_ignore(path: &Path, patterns: &[String]) -> bool {
        let path_str = path.to_string_lossy();
        patterns.iter().any(|pattern| path_str.contains(pattern.as_str()))
    }
}
