use anyhow::{Context, Result};
use console::style;
use std::process::Command;

/// `cargo rgpui install` — adb install
pub fn run() -> Result<()> {
    let apk = "android/app/build/outputs/apk/debug/app-debug.apk";
    if !std::path::Path::new(apk).exists() {
        anyhow::bail!("APK 不存在: {}，请先运行 cargo rgpui build", apk);
    }

    println!("{} 安装 APK 到设备...", style("📱").cyan());

    let status = Command::new("adb")
        .args(["install", "-r", "-d", apk])
        .status()
        .context("adb 启动失败，请确认 adb 在 PATH 中")?;

    if status.success() {
        println!("{} 安装成功", style("✅").green());
    } else {
        anyhow::bail!("adb install 失败");
    }

    Ok(())
}
