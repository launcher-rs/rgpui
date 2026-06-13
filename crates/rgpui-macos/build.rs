#![allow(clippy::disallowed_methods, reason = "build scripts are exempt")]
#[cfg(target_os = "macos")]
fn main() {
    use std::{env, path::PathBuf, process::Command};

    let sdk_path = String::from_utf8(
        Command::new("xcrun")
            .args(["--sdk", "macosx", "--show-sdk-path"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
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
        .expect("unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("couldn't write dispatch bindings");

    // 编译 Metal 着色器
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let shader_src = format!("{}/src/shaders.metal", manifest_dir);
    println!("cargo:rerun-if-changed={}", shader_src);
    let air_path = out_path.join("shaders.air");
    let metallib_path = out_path.join("shaders.metallib");

    let metallib_result = Command::new("xcrun")
        .args([
            "--sdk",
            "macosx",
            "metal",
            "-c",
            &shader_src,
            "-o",
            air_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to compile Metal shader to AIR");

    if !metallib_result.status.success() {
        panic!(
            "Failed to compile Metal shader to AIR:\n{}",
            String::from_utf8_lossy(&metallib_result.stderr)
        );
    }

    let metallib_result = Command::new("xcrun")
        .args([
            "--sdk",
            "macosx",
            "metallib",
            air_path.to_str().unwrap(),
            "-o",
            metallib_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to compile AIR to metallib");

    if !metallib_result.status.success() {
        panic!(
            "Failed to compile AIR to metallib:\n{}",
            String::from_utf8_lossy(&metallib_result.stderr)
        );
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {}
