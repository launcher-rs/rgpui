#![allow(clippy::disallowed_methods, reason = "build scripts are exempt")]
// 构建脚本同样禁用 `unwrap` / `expect` / `panic!`：失败统一走 `Result`，
// 由 cargo 以「failed to run custom build command」的形式报给开发者。
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

/// macOS 上用 xcrun + bindgen 生成 CoreMedia/CoreVideo 绑定，并预编译 Metal 着色器。
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env, path::PathBuf, process::Command};

    let sdk_output = Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-path"])
        .output()
        .map_err(|err| format!("运行 `xcrun --show-sdk-path` 失败: {}", err))?;
    let sdk_path = String::from_utf8(sdk_output.stdout)?;
    let sdk_path = sdk_path.trim_end();

    println!("cargo:rerun-if-changed=src/media/bindings.h");
    let bindings = bindgen::Builder::default()
        .header("src/media/bindings.h")
        .clang_arg(format!("-isysroot{}", sdk_path))
        .clang_arg("-xobjective-c")
        .allowlist_type("CMItemIndex")
        .allowlist_type("CMSampleTimingInfo")
        .allowlist_type("CMVideoCodecType")
        .allowlist_type("VTEncodeInfoFlags")
        .allowlist_function("CMTimeMake")
        .allowlist_var("kCVPixelFormatType_.*")
        .allowlist_var("kCVReturn.*")
        .allowlist_var("VTEncodeInfoFlags_.*")
        .allowlist_var("kCMVideoCodecType_.*")
        .allowlist_var("kCMTime.*")
        .allowlist_var("kCMSampleAttachmentKey_.*")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .layout_tests(false)
        .generate()
        .map_err(|err| format!("生成 bindings 失败: {}", err))?;

    let out_path = PathBuf::from(env::var("OUT_DIR")?);
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .map_err(|err| format!("写入 bindings.rs 失败: {}", err))?;

    // 编译 Metal 着色器
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let shader_src = format!("{}/src/shaders.metal", manifest_dir);
    println!("cargo:rerun-if-changed={}", shader_src);
    let air_path = out_path.join("shaders.air");
    let metallib_path = out_path.join("shaders.metallib");
    let air_str = air_path
        .to_str()
        .ok_or_else(|| format!("AIR 输出路径不是合法 UTF-8: {}", air_path.display()))?;
    let metallib_str = metallib_path.to_str().ok_or_else(|| {
        format!(
            "metallib 输出路径不是合法 UTF-8: {}",
            metallib_path.display()
        )
    })?;

    let metallib_result = Command::new("xcrun")
        .args(["--sdk", "macosx", "metal", "-c", &shader_src, "-o", air_str])
        .output()
        .map_err(|err| format!("调用 metal 编译器失败: {}", err))?;

    if !metallib_result.status.success() {
        return Err(format!(
            "Failed to compile Metal shader to AIR:\n{}",
            String::from_utf8_lossy(&metallib_result.stderr)
        )
        .into());
    }

    let metallib_result = Command::new("xcrun")
        .args(["--sdk", "macosx", "metallib", air_str, "-o", metallib_str])
        .output()
        .map_err(|err| format!("调用 metallib 失败: {}", err))?;

    if !metallib_result.status.success() {
        return Err(format!(
            "Failed to compile AIR to metallib:\n{}",
            String::from_utf8_lossy(&metallib_result.stderr)
        )
        .into());
    }

    Ok(())
}

/// 非 macOS 目标不需要生成绑定，也不编译 Metal 着色器。
#[cfg(not(target_os = "macos"))]
fn main() {}
