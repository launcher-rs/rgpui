//! Windows 开机自启动实现
//!
//! 通过修改注册表 `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` 实现开机自启动

use rgpui::Result;

/// 注册表开机自启动键路径
const RUN_KEY_PATH: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";

/// 设置开机自启动
///
/// # 参数
/// * `app_id` - 应用程序标识符（注册表项名称）
/// * `enabled` - 是否启用
///
/// # 返回
/// 成功时返回 `Ok(())`，失败时返回错误
pub fn set_auto_launch(app_id: &str, enabled: bool) -> Result<()> {
    let key = windows_registry::CURRENT_USER.create(RUN_KEY_PATH)?;

    if enabled {
        let exe_path = std::env::current_exe()?;
        key.set_string(app_id, &exe_path.to_string_lossy())?;
    } else {
        key.remove_value(app_id)?;
    }

    Ok(())
}

/// 检查开机自启动是否已启用
///
/// # 参数
/// * `app_id` - 应用程序标识符
///
/// # 返回
/// 如果已启用返回 `true`，否则返回 `false`
pub fn is_auto_launch_enabled(app_id: &str) -> bool {
    if let Ok(key) = windows_registry::CURRENT_USER.open(RUN_KEY_PATH) {
        key.get_string(app_id).is_ok()
    } else {
        false
    }
}
