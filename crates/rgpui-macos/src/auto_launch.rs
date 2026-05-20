//! macOS 自动启动实现
//!
//! 使用 LaunchAgents plist 文件实现开机自启动

use std::fs;
use std::path::PathBuf;

use rgpui::Result;

/// macOS 自动启动管理器
pub struct MacAutoLaunch;

impl MacAutoLaunch {
    /// 创建新的自动启动管理器
    pub fn new() -> Self {
        Self
    }

    /// 启用自动启动
    ///
    /// # 参数
    /// * `app_name` - 应用名称
    /// * `app_path` - 应用路径
    ///
    /// # 返回
    /// 成功时返回 `Ok(())`，失败时返回错误
    pub fn enable(&self, app_name: &str, app_path: &str) -> Result<()> {
        let plist_dir = self.get_launchagents_dir()?;
        let plist_path = plist_dir.join(format!("{}.plist", app_name));

        let plist_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>"#,
            app_name, app_path
        );

        fs::write(&plist_path, plist_content)?;
        Ok(())
    }

    /// 禁用自动启动
    ///
    /// # 参数
    /// * `app_name` - 应用名称
    pub fn disable(&self, app_name: &str) -> Result<()> {
        let plist_dir = self.get_launchagents_dir()?;
        let plist_path = plist_dir.join(format!("{}.plist", app_name));

        if plist_path.exists() {
            fs::remove_file(&plist_path)?;
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
        if let Ok(plist_dir) = self.get_launchagents_dir() {
            let plist_path = plist_dir.join(format!("{}.plist", app_name));
            plist_path.exists()
        } else {
            false
        }
    }

    /// 获取 LaunchAgents 目录路径
    fn get_launchagents_dir(&self) -> Result<PathBuf> {
        let home = std::env::var("HOME")?;
        let plist_dir = PathBuf::from(home).join("Library/LaunchAgents");

        if !plist_dir.exists() {
            fs::create_dir_all(&plist_dir)?;
        }

        Ok(plist_dir)
    }
}
