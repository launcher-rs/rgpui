use anyhow::Result;
use console::style;

struct Check {
    name: &'static str,
    ok: bool,
    detail: String,
    fix: Option<String>,
}

pub fn run() -> Result<()> {
    println!("{}", style("检查 rgpui 移动端开发环境...\n").bold().cyan());

    let checks = vec![
        // 1. JDK
        check_jdk(),
        // 2. ANDROID_HOME
        check_android_home(),
        // 3. NDK
        check_ndk(),
        // 4. Rust target
        check_rust_target(),
        // 5. cargo-ndk
        check_cargo_ndk(),
        // 6. llvm-strip
        check_llvm_strip(),
        // 7. 设备连接
        check_device(),
    ];

    let mut all_ok = true;
    for c in &checks {
        let icon = if c.ok {
            style("  ✅").green()
        } else {
            all_ok = false;
            style("  ❌").red()
        };
        println!("{} {} {}", icon, c.name, style(&c.detail).dim());
        if let Some(fix) = &c.fix {
            println!("     {} {}", style("→").yellow(), style(fix).dim());
        }
    }

    println!();
    if all_ok {
        println!("{}", style("全部通过！可以开始移动开发。").bold().green());
    } else {
        println!(
            "{}",
            style("部分检查未通过，请按上方建议修复后重试。")
                .bold()
                .red()
        );
    }

    Ok(())
}

fn run_cmd(cmd: &str, args: &[&str]) -> Option<String> {
    std::process::Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                // 部分工具（如 `java -version`）把版本信息打到 stderr，一并合并后再判断。
                let mut out = String::from_utf8_lossy(&o.stdout).into_owned();
                out.push_str(&String::from_utf8_lossy(&o.stderr));
                Some(out)
            } else {
                None
            }
        })
}

fn run_cmd_trim(cmd: &str, args: &[&str]) -> Option<String> {
    run_cmd(cmd, args).map(|s| s.trim().to_string())
}

fn check_jdk() -> Check {
    let name = "JDK";
    match run_cmd_trim("java", &["-version"]) {
        Some(out) => {
            // java -version 输出到 stderr，但有些版本输出到 stdout
            let ver = out.contains("17")
                || out.contains("21")
                || out.contains("22")
                || out.contains("23")
                || out.contains("24")
                || out.contains("25");
            if ver {
                Check {
                    name,
                    ok: true,
                    detail: extract_version(&out),
                    fix: None,
                }
            } else {
                Check {
                    name,
                    ok: false,
                    detail: extract_version(&out),
                    fix: Some("安装 JDK 17+: winget install Microsoft.OpenJDK.17".into()),
                }
            }
        }
        None => Check {
            name,
            ok: false,
            detail: "未找到".into(),
            fix: Some("安装 JDK 17+: winget install Microsoft.OpenJDK.17".into()),
        },
    }
}

fn extract_version(out: &str) -> String {
    for line in out.lines() {
        if line.contains("version") {
            return line.trim().to_string();
        }
    }
    out.lines().next().unwrap_or("unknown").to_string()
}

fn check_android_home() -> Check {
    let name = "Android SDK";
    match std::env::var("ANDROID_HOME").or_else(|_| std::env::var("ANDROID_SDK_ROOT")) {
        Ok(path) => {
            let p = std::path::Path::new(&path);
            if p.is_dir() {
                let has_platform_tools = p.join("platform-tools/adb.exe").exists()
                    || p.join("platform-tools/adb").exists();
                let has_platforms = p.join("platforms/android-34").is_dir();
                let has_build_tools =
                    p.join("build-tools/34.0.0").is_dir() || p.join("build-tools/34").is_dir();
                let mut missing = Vec::new();
                if !has_platform_tools {
                    missing.push("platform-tools");
                }
                if !has_platforms {
                    missing.push("platforms;android-34");
                }
                if !has_build_tools {
                    missing.push("build-tools;34.0.0");
                }
                if missing.is_empty() {
                    Check {
                        name,
                        ok: true,
                        detail: path,
                        fix: None,
                    }
                } else {
                    Check {
                        name,
                        ok: false,
                        detail: format!("{} (缺少: {})", path, missing.join(", ")),
                        fix: Some(format!("sdkmanager \"{}\"", missing.join("\" \""))),
                    }
                }
            } else {
                Check {
                    name,
                    ok: false,
                    detail: format!("{} (目录不存在)", path),
                    fix: Some("安装 Android SDK 或设置正确的 ANDROID_HOME".into()),
                }
            }
        }
        Err(_) => Check {
            name,
            ok: false,
            detail: "未设置".into(),
            fix: Some(
                "$env:ANDROID_HOME = \"C:\\Users\\<you>\\AppData\\Local\\Android\\Sdk\"".into(),
            ),
        },
    }
}

fn find_ndk_path() -> Option<String> {
    // 1. ANDROID_NDK_HOME
    if let Ok(p) = std::env::var("ANDROID_NDK_HOME")
        && std::path::Path::new(&p).is_dir()
    {
        return Some(p);
    }
    // 2. ANDROID_HOME/ndk/
    if let Ok(sdk) = std::env::var("ANDROID_HOME").or_else(|_| std::env::var("ANDROID_SDK_ROOT")) {
        let ndk_dir = std::path::Path::new(&sdk).join("ndk");
        if ndk_dir.is_dir() {
            // 找最新版本
            let mut versions: Vec<String> = std::fs::read_dir(&ndk_dir)
                .ok()
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().is_dir())
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect()
                })
                .unwrap_or_default();
            versions.sort();
            if let Some(latest) = versions.last() {
                let p = ndk_dir.join(latest);
                if p.is_dir() {
                    return Some(p.to_string_lossy().to_string());
                }
            }
        }
    }
    None
}

fn check_ndk() -> Check {
    let name = "Android NDK";
    match find_ndk_path() {
        Some(path) => {
            let ver_file = std::path::Path::new(&path).join("source.properties");
            let version = std::fs::read_to_string(&ver_file)
                .ok()
                .and_then(|s| {
                    s.lines()
                        .find(|l| l.starts_with("Pkg.Revision"))
                        .and_then(|l| l.split('=').nth(1))
                        .map(|v| v.trim().to_string())
                })
                .unwrap_or_else(|| "unknown".into());
            Check {
                name,
                ok: true,
                detail: format!("{} (v{})", path, version),
                fix: None,
            }
        }
        None => Check {
            name,
            ok: false,
            detail: "未找到".into(),
            fix: Some("sdkmanager \"ndk;27.0.12077973\" 或设置 ANDROID_NDK_HOME".into()),
        },
    }
}

fn check_rust_target() -> Check {
    let name = "Rust target (aarch64-linux-android)";
    match run_cmd_trim("rustup", &["target", "list", "--installed"]) {
        Some(out) if out.contains("aarch64-linux-android") => Check {
            name,
            ok: true,
            detail: "已安装".into(),
            fix: None,
        },
        _ => Check {
            name,
            ok: false,
            detail: "未安装".into(),
            fix: Some("rustup target add aarch64-linux-android".into()),
        },
    }
}

fn check_cargo_ndk() -> Check {
    let name = "cargo-ndk";
    match run_cmd_trim("cargo", &["ndk", "--version"]) {
        Some(out) => Check {
            name,
            ok: true,
            detail: out,
            fix: None,
        },
        None => Check {
            name,
            ok: false,
            detail: "未安装".into(),
            fix: Some("cargo install cargo-ndk".into()),
        },
    }
}

fn check_llvm_strip() -> Check {
    let name = "llvm-strip (NDK)";
    match find_ndk_path() {
        Some(ndk) => {
            let strip = if cfg!(windows) {
                std::path::PathBuf::from(format!(
                    "{}/toolchains/llvm/prebuilt/windows-x86_64/bin/llvm-strip.exe",
                    ndk
                ))
            } else if cfg!(target_os = "macos") {
                std::path::PathBuf::from(format!(
                    "{}/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-strip",
                    ndk
                ))
            } else {
                std::path::PathBuf::from(format!(
                    "{}/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip",
                    ndk
                ))
            };
            if strip.exists() {
                Check {
                    name,
                    ok: true,
                    detail: strip.to_string_lossy().to_string(),
                    fix: None,
                }
            } else {
                Check {
                    name,
                    ok: false,
                    detail: format!("{} (不存在)", strip.display()),
                    fix: Some("更新 NDK 或检查安装完整性".into()),
                }
            }
        }
        None => Check {
            name,
            ok: false,
            detail: "依赖 NDK，NDK 未找到".into(),
            fix: Some("先安装 NDK".into()),
        },
    }
}

fn check_device() -> Check {
    let name = "设备连接";
    let Some(out) = crate::adb::adb_output(&["devices"]) else {
        return Check {
            name,
            ok: false,
            detail: "adb 未找到".into(),
            fix: Some("安装 platform-tools 或设置 ANDROID_HOME".into()),
        };
    };
    let devices: Vec<&str> = out
        .lines()
        .skip(1)
        .filter(|l| l.contains('\t') && l.contains("device"))
        .collect();
    if devices.is_empty() {
        Check {
            name,
            ok: false,
            detail: "无设备连接".into(),
            fix: Some("开启 USB 调试并连接设备，然后 adb devices 确认".into()),
        }
    } else {
        let detail = devices
            .iter()
            .map(|l| l.split('\t').next().unwrap_or("?"))
            .collect::<Vec<_>>()
            .join(", ");
        Check {
            name,
            ok: true,
            detail,
            fix: None,
        }
    }
}
