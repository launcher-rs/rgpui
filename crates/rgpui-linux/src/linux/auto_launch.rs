//! Linux 自动启动实现
//!
//! 使用 XDG Autostart 规范实现开机自启动

use std::fs;
use std::path::PathBuf;

use rgpui::Result;

/// Linux 自动启动管理器
pub struct LinuxAutoLaunch;

impl LinuxAutoLaunch {
    /// 创建新的自动启动管理器
    pub fn new() -> Self {
        Self
    }

    /// 启用自动启动
    ///
    /// # 参数
    /// * `app_name` - 应用名称
    /// * `exec_path` - 可执行文件路径
    ///
    /// # 返回
    /// 成功时返回 `Ok(())`，失败时返回错误
    pub fn enable(&self, app_name: &str, exec_path: &str) -> Result<()> {
        let autostart_dir = self.get_autostart_dir()?;
        let desktop_file_path = autostart_dir.join(format!("{}.desktop", app_name));

        let desktop_content = format!(
            "[Desktop Entry]
Type=Application
Name={}
Exec={}
Hidden=false
NoDisplay=false
X-GNOME-Autostart-enabled=true
Comment=Auto-start application: {}",
            app_name, exec_path, app_name
        );

        fs::write(&desktop_file_path, desktop_content)?;
        Ok(())
    }

    /// 禁用自动启动
    ///
    /// # 参数
    /// * `app_name` - 应用名称
    pub fn disable(&self, app_name: &str) -> Result<()> {
        let autostart_dir = self.get_autostart_dir()?;
        let desktop_file_path = autostart_dir.join(format!("{}.desktop", app_name));

        if desktop_file_path.exists() {
            fs::remove_file(&desktop_file_path)?;
        }

        Ok(())
    }

    /// 检查是否已启用自动启动
    ///
    /// # 参数
    /// * `app_name` - 应用名称
    ///
    /// # 返回
    /// 如果已启用返回 `true`，否则返回 `false`
    pub fn is_enabled(&self, app_name: &str) -> bool {
        if let Ok(autostart_dir) = self.get_autostart_dir() {
            let desktop_file_path = autostart_dir.join(format!("{}.desktop", app_name));
            desktop_file_path.exists()
        } else {
            false
        }
    }

    /// 获取 XDG autostart 目录路径
    fn get_autostart_dir(&self) -> Result<PathBuf> {
        let home = std::env::var("HOME")?;
        let autostart_dir = PathBuf::from(home).join(".config/autostart");

        if !autostart_dir.exists() {
            fs::create_dir_all(&autostart_dir)?;
        }

        Ok(autostart_dir)
    }
}
