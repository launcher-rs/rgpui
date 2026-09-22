use anyhow::{Context, Result};
use console::style;
use std::process::Command;

/// `cargo rgpui run` — adb install + start + logcat
pub fn run() -> Result<()> {
    // 从 Cargo.toml 读取包名
    let manifest = std::fs::read_to_string("Cargo.toml").context("当前目录找不到 Cargo.toml")?;
    let name = extract_crate_name(&manifest).context("Cargo.toml 中找不到 [package] name")?;
    let package_name = format!("com.example.{}", name.replace('-', "_"));
    let activity = format!("{}/android.app.NativeActivity", package_name);

    // 安装
    let apk = "android/app/build/outputs/apk/debug/app-debug.apk";
    if !std::path::Path::new(apk).exists() {
        anyhow::bail!("APK 不存在: {}，请先运行 cargo rgpui build", apk);
    }

    println!("{} 安装 APK 到设备...", style("📱").cyan());
    let status = Command::new("adb")
        .args(["install", "-r", "-d", apk])
        .status()
        .context("adb 启动失败，请确认 adb 在 PATH 中")?;
    if !status.success() {
        anyhow::bail!("adb install 失败");
    }
    println!("{} 安装成功", style("✅").green());

    // 强制停止旧实例
    println!("\n{} 启动应用...", style("🚀").cyan());
    let _ = Command::new("adb")
        .args(["shell", "am", "force-stop", &package_name])
        .status();
    std::thread::sleep(std::time::Duration::from_millis(500));

    // 启动
    let status = Command::new("adb")
        .args(["shell", "am", "start", "-n", &activity])
        .status()
        .context("adb 启动失败")?;
    if !status.success() {
        anyhow::bail!("adb shell am start 失败");
    }

    // logcat
    println!("\n{} 实时日志 (Ctrl+C 退出):\n", style("📋").cyan());
    let status = Command::new("adb")
        .args(["logcat", "-s", &name])
        .status()
        .context("adb logcat 失败")?;

    if !status.success() {
        anyhow::bail!("adb logcat 退出");
    }

    Ok(())
}

fn extract_crate_name(cargo_toml: &str) -> Option<String> {
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name") && trimmed.contains('=') {
            let val = trimmed.split('=').nth(1)?.trim().trim_matches('"');
            return Some(val.to_string());
        }
    }
    None
}
