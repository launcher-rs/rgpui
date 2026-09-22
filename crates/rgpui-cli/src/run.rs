use anyhow::Result;
use console::style;

use crate::config::Config;

/// `cargo rgpui run` — adb install + start + logcat
pub fn run(release: bool, cfg: &Config) -> Result<()> {
    let package_name = crate::install::package_name(cfg, release)?;
    let activity = format!("{}/android.app.NativeActivity", package_name);
    // logcat 过滤 tag = crate 名（android_logger 默认用 crate 名做 tag）
    let log_tag = crate::install::crate_name()?;

    // 安装
    crate::install::run(release)?;

    // 强制停止旧实例
    println!("\n{} 启动应用...", style("🚀").cyan());
    let _ = crate::adb::run_adb(&["shell", "am", "force-stop", &package_name]);
    std::thread::sleep(std::time::Duration::from_millis(500));

    // 启动
    if !crate::adb::run_adb(&["shell", "am", "start", "-n", &activity])? {
        anyhow::bail!("adb shell am start 失败");
    }

    // logcat（按 crate 名过滤）
    println!("\n{} 实时日志 (Ctrl+C 退出):\n", style("📋").cyan());
    let adb = crate::adb::find_adb().ok_or_else(|| anyhow::anyhow!("adb 未找到"))?;
    let status = std::process::Command::new(adb)
        .args(["logcat", "-s", &log_tag])
        .status()?;
    if !status.success() {
        anyhow::bail!("adb logcat 退出");
    }

    Ok(())
}
