use anyhow::Result;
use console::style;

use crate::config::Config;

/// `cargo rgpui install` — adb install
pub fn run(release: bool) -> Result<()> {
    let apk = apk_path(release);
    if !std::path::Path::new(&apk).exists() {
        anyhow::bail!("APK 不存在: {}，请先运行 cargo rgpui build", apk);
    }

    println!("{} 安装 APK 到设备...", style("📱").cyan());

    if crate::adb::run_adb(&["install", "-r", "-d", &apk])? {
        println!("{} 安装成功", style("✅").green());
        Ok(())
    } else {
        anyhow::bail!("adb install 失败")
    }
}

/// 根据 release 标志推导 APK 输出路径。
pub fn apk_path(release: bool) -> String {
    if release {
        "android/app/build/outputs/apk/release/app-release.apk".into()
    } else {
        "android/app/build/outputs/apk/debug/app-debug.apk".into()
    }
}

/// 从当前目录 Cargo.toml 推导应用包名（读取 name 后拼配置前缀）。
pub fn package_name(cfg: &Config) -> Result<String> {
    let manifest = std::fs::read_to_string("Cargo.toml")
        .map_err(|_| anyhow::anyhow!("当前目录找不到 Cargo.toml"))?;
    let name = extract_crate_name(&manifest)
        .ok_or_else(|| anyhow::anyhow!("Cargo.toml 中找不到 [package] name"))?;
    Ok(format!(
        "{}{}",
        cfg.package_prefix(),
        name.replace('-', "_")
    ))
}

/// 从当前目录 Cargo.toml 读取 crate 名（android_logger 的 logcat tag）。
pub fn crate_name() -> Result<String> {
    let manifest = std::fs::read_to_string("Cargo.toml")
        .map_err(|_| anyhow::anyhow!("当前目录找不到 Cargo.toml"))?;
    extract_crate_name(&manifest)
        .ok_or_else(|| anyhow::anyhow!("Cargo.toml 中找不到 [package] name"))
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
