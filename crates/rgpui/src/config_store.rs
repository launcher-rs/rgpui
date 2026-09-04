//! 配置持久化 API —— 应用配置的加载、保存和监听。

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 配置存储。
pub struct ConfigStore {
    /// 配置目录路径。
    config_dir: PathBuf,
    /// 配置文件路径。
    config_file: PathBuf,
    /// 配置数据（JSON 值）。
    data: serde_json::Value,
    /// 监听器。
    listeners: Vec<Arc<dyn Fn(&serde_json::Value) + Send + Sync>>,
}

impl ConfigStore {
    /// 创建新的配置存储。
    pub fn new(app_name: &str) -> Self {
        let config_dir = Self::get_config_dir(app_name);
        let config_file = config_dir.join("config.json");

        Self {
            config_dir,
            config_file,
            data: serde_json::Value::Object(serde_json::Map::new()),
            listeners: Vec::new(),
        }
    }

    /// 创建带有自定义路径的配置存储。
    pub fn with_path(config_file: PathBuf) -> Self {
        let config_dir = config_file.parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();

        Self {
            config_dir,
            config_file,
            data: serde_json::Value::Object(serde_json::Map::new()),
            listeners: Vec::new(),
        }
    }

    /// 获取配置目录路径。
    fn get_config_dir(app_name: &str) -> PathBuf {
        if let Some(config_dir) = dirs::config_dir() {
            config_dir.join(app_name)
        } else {
            PathBuf::from(format!("./{}", app_name))
        }
    }

    /// 加载配置。
    pub fn load<T: serde::de::DeserializeOwned + serde::Serialize + Default>(&mut self) -> anyhow::Result<T> {
        if self.config_file.exists() {
            let content = std::fs::read_to_string(&self.config_file)?;
            self.data = serde_json::from_str(&content)?;
            Ok(serde_json::from_value(self.data.clone())?)
        } else {
            let default = T::default();
            self.data = serde_json::to_value(&default)?;
            Ok(serde_json::from_value(self.data.clone())?)
        }
    }

    /// 保存配置。
    pub fn save<T: serde::Serialize>(&mut self, config: &T) -> anyhow::Result<()> {
        self.data = serde_json::to_value(config)?;

        // 确保目录存在
        if !self.config_dir.exists() {
            std::fs::create_dir_all(&self.config_dir)?;
        }

        let content = serde_json::to_string_pretty(&self.data)?;
        std::fs::write(&self.config_file, content)?;

        // 通知监听器
        for listener in &self.listeners {
            listener(&self.data);
        }

        Ok(())
    }

    /// 获取原始 JSON 值。
    pub fn data(&self) -> &serde_json::Value {
        &self.data
    }

    /// 获取配置文件路径。
    pub fn config_file(&self) -> &Path {
        &self.config_file
    }

    /// 监听配置变化。
    pub fn on_change<F>(&mut self, callback: F)
    where
        F: Fn(&serde_json::Value) + Send + Sync + 'static,
    {
        self.listeners.push(Arc::new(callback));
    }
}

impl Default for ConfigStore {
    fn default() -> Self {
        Self::new("rgpui_app")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default, PartialEq)]
    struct TestConfig {
        name: String,
        value: i32,
    }

    #[test]
    fn test_config_store_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("config.json");
        let mut store = ConfigStore::with_path(config_file);
        let config = TestConfig { name: "test".to_string(), value: 42 };
        store.save(&config).unwrap();
        let loaded: TestConfig = store.load().unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn test_config_store_load_default() {
        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("config.json");
        let mut store = ConfigStore::with_path(config_file);
        let loaded: TestConfig = store.load().unwrap();
        assert_eq!(loaded, TestConfig::default());
    }
}
