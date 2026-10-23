#![allow(clippy::disallowed_methods, reason = "build scripts are exempt")]
// 构建脚本同样禁用 `unwrap` / `expect` / `panic!`：失败统一走 `Result`，
// 由 cargo 以「failed to run custom build command」的形式报给开发者。
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

/// Windows 上内嵌应用 manifest，其他平台只登记 gles 的 cfg 检查。
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rustc-check-cfg=cfg(gles)");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if target_os == "windows" {
        #[cfg(feature = "windows-manifest")]
        return embed_resource();
    }

    Ok(())
}

#[cfg(feature = "windows-manifest")]
fn embed_resource() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = std::path::Path::new("resources/windows/gpui.manifest.xml");
    let rc_file = std::path::Path::new("resources/windows/gpui.rc");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rerun-if-changed={}", rc_file.display());
    embed_resource::compile(rc_file, embed_resource::NONE)
        .manifest_required()
        .map_err(|err| format!("内嵌 Windows manifest 失败: {:?}", err))?;

    Ok(())
}
