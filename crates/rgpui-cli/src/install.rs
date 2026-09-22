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

/// 从 `android/app/build.gradle.kts` 读取真实包名（applicationId + 调试后缀）。
/// gradle 解析失败时回退到“配置前缀 + crate 名”旧推导，保证非标准工程仍可用。
pub fn package_name(cfg: &Config, release: bool) -> Result<String> {
    let gradle_pkg = std::fs::read_to_string("android/app/build.gradle.kts")
        .ok()
        .and_then(|gradle| {
            let app_id = extract_gradle_string(&gradle, "applicationId")?;
            if release {
                Some(app_id)
            } else {
                let suffix =
                    extract_gradle_string(&gradle, "applicationIdSuffix").unwrap_or_default();
                Some(format!("{app_id}{suffix}"))
            }
        });
    if let Some(pkg) = gradle_pkg {
        return Ok(pkg);
    }
    // 回退：从当前目录 Cargo.toml 推导应用包名（读取 name 后拼配置前缀）。
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

/// 从 gradle.kts 文本提取 `key = "value"` 的 value。
/// key 后须紧跟 `=`（完整单词），避免 `applicationId` 误命中 `applicationIdSuffix`。
fn extract_gradle_string(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix(key) else {
            continue;
        };
        let Some(val) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        let val = val.trim().trim_matches('"');
        if !val.is_empty() {
            return Some(val.to_string());
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    /// gradle 包名提取：`applicationId` 不得误命中 `applicationIdSuffix`。
    #[test]
    fn gradle_package_parsing() {
        let gradle = r#"
            namespace = "com.example.hellomobile"
            defaultConfig {
                applicationId = "com.example.hellomobile"
                minSdk = 26
            }
            buildTypes {
                debug {
                    applicationIdSuffix = ".debug"
                }
            }
        "#;
        assert_eq!(
            extract_gradle_string(gradle, "applicationId").as_deref(),
            Some("com.example.hellomobile")
        );
        assert_eq!(
            extract_gradle_string(gradle, "applicationIdSuffix").as_deref(),
            Some(".debug")
        );
        assert_eq!(extract_gradle_string(gradle, "nonexistent"), None);
    }
}
