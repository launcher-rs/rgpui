use anyhow::{Context, Result};
use console::style;
use std::path::Path;
use std::process::Command;

/// `cargo rgpui build` — cargo ndk → llvm-strip → gradlew assembleDebug
pub fn run(release: bool) -> Result<()> {
    // 找到项目名（当前目录的 Cargo.toml 中的 name）
    let manifest = std::fs::read_to_string("Cargo.toml")
        .context("当前目录找不到 Cargo.toml，请在 rgpui 项目根目录运行")?;
    let name = extract_crate_name(&manifest).context("Cargo.toml 中找不到 [package] name")?;
    let lib_name = name.replace('-', "_");

    let build_type = if release { "release" } else { "debug" };
    let gradle_task = if release {
        "assembleRelease"
    } else {
        "assembleDebug"
    };

    println!(
        "{} 构建 {} ({})...",
        style("🔨").cyan(),
        style(&name).bold(),
        build_type
    );

    // Step 1: cargo ndk build
    println!("\n{} Step 1/3: cargo ndk build", style("▸").cyan());
    let mut ndk_args = vec![
        "-t",
        "arm64-v8a",
        "-P",
        "31",
        "-o",
        "android/app/src/main/jniLibs",
    ];
    ndk_args.push("build");
    if release {
        ndk_args.push("--release");
    }
    ndk_args.push("-p");
    ndk_args.push(&name);

    let status = Command::new("cargo")
        .args(&ndk_args)
        .status()
        .context("cargo ndk 启动失败，请确认 cargo-ndk 已安装 (cargo install cargo-ndk)")?;
    if !status.success() {
        anyhow::bail!("cargo ndk build 失败");
    }

    // Step 2: strip debug symbols
    println!("\n{} Step 2/3: strip debug symbols", style("▸").cyan());
    let so_path = format!("android/app/src/main/jniLibs/arm64-v8a/lib{}.so", lib_name);

    if Path::new(&so_path).exists() {
        if let Some(strip_bin) = find_llvm_strip() {
            let status = Command::new(&strip_bin)
                .args(["--strip-debug", &so_path])
                .status();
            match status {
                Ok(s) if s.success() => {
                    let size = std::fs::metadata(&so_path).map(|m| m.len()).unwrap_or(0);
                    println!(
                        "  {} 已 strip ({:.1} MB)",
                        style("✓").green(),
                        size as f64 / 1_048_576.0
                    );
                }
                _ => {
                    println!(
                        "  {} strip 失败，继续构建（APK 体积会偏大）",
                        style("⚠").yellow()
                    );
                }
            }
        } else {
            println!("  {} llvm-strip 未找到，跳过 strip", style("⚠").yellow());
        }
    } else {
        println!("  {} .so 不存在: {}", style("⚠").yellow(), so_path);
    }

    // Step 3: gradle build
    println!("\n{} Step 3/3: gradlew {}", style("▸").cyan(), gradle_task);
    if !Path::new("android").exists() {
        anyhow::bail!("当前目录下找不到 android/ 目录，请确认在 rgpui 项目根目录运行");
    }

    let gradlew = if cfg!(windows) {
        "android/gradlew.bat"
    } else {
        "./android/gradlew"
    };

    let status = Command::new(gradlew)
        .args(["-p", "android", gradle_task])
        .status()
        .context("gradlew 启动失败")?;
    if !status.success() {
        anyhow::bail!("gradlew {} 失败", gradle_task);
    }

    let apk_path = if release {
        "android/app/build/outputs/apk/release/app-release.apk"
    } else {
        "android/app/build/outputs/apk/debug/app-debug.apk"
    };
    let apk_size = std::fs::metadata(apk_path)
        .map(|m| format!("{:.1} MB", m.len() as f64 / 1_048_576.0))
        .unwrap_or_else(|_| "unknown".into());

    println!(
        "\n{} 构建成功！APK: {} ({})",
        style("✅").green(),
        apk_path,
        apk_size
    );

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

fn find_llvm_strip() -> Option<String> {
    let ndk_dir = find_ndk_dir()?;
    let triple = if cfg!(windows) {
        "windows-x86_64"
    } else if cfg!(target_os = "macos") {
        "darwin-x86_64"
    } else {
        "linux-x86_64"
    };
    let ext = if cfg!(windows) { ".exe" } else { "" };
    let path = format!(
        "{}/toolchains/llvm/prebuilt/{}/bin/llvm-strip{}",
        ndk_dir, triple, ext
    );
    if std::path::Path::new(&path).exists() {
        Some(path)
    } else {
        None
    }
}

fn find_ndk_dir() -> Option<String> {
    if let Ok(p) = std::env::var("ANDROID_NDK_HOME")
        && std::path::Path::new(&p).is_dir()
    {
        return Some(p);
    }
    if let Ok(sdk) = std::env::var("ANDROID_HOME").or_else(|_| std::env::var("ANDROID_SDK_ROOT")) {
        let ndk_dir = std::path::PathBuf::from(sdk).join("ndk");
        if ndk_dir.is_dir() {
            let mut versions: Vec<String> = std::fs::read_dir(&ndk_dir)
                .ok()?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect();
            versions.sort();
            return versions
                .last()
                .map(|v| ndk_dir.join(v).to_string_lossy().to_string());
        }
    }
    None
}
