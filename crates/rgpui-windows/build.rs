#![allow(clippy::disallowed_methods, reason = "build scripts are exempt")]
// 构建脚本同样禁用 `unwrap` / `expect` / `panic!`：失败统一走 `Result`，
// 由 cargo 以「failed to run custom build command」的形式报给开发者。
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

/// Windows 上预编译 HLSL 着色器，其他平台直接成功返回。
fn main() -> Result<(), Box<dyn std::error::Error>> {
    compile_shaders()
}

#[cfg(target_os = "windows")]
fn compile_shaders() -> Result<(), Box<dyn std::error::Error>> {
    // Compile HLSL shaders — debug 和 release 都预编译，
    // 避免 debug 模式下运行时 D3DCompileFromFile 消耗大量栈空间
    // 导致 STATUS_ACCESS_VIOLATION（1MB 默认栈溢出）。
    shader_compilation::compile_shaders()
}

#[cfg(not(target_os = "windows"))]
fn compile_shaders() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[cfg(target_os = "windows")]
mod shader_compilation {
    use std::{
        fs,
        io::Write,
        path::{Path, PathBuf},
        process::{self, Command},
    };

    /// 预编译全部着色器模块并生成 `shaders_bytes.rs`。
    pub fn compile_shaders() -> Result<(), Box<dyn std::error::Error>> {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
        let shader_path = Path::new(&manifest_dir).join("src/shaders.hlsl");
        let out_dir = std::env::var("OUT_DIR")?;

        println!("cargo:rerun-if-changed={}", shader_path.display());

        // Check if fxc.exe is available
        let fxc_path = find_fxc_compiler()?;

        // Define all modules
        let modules = [
            "quad",
            "shadow",
            "path_rasterization",
            "path_sprite",
            "underline",
            "monochrome_sprite",
            "subpixel_sprite",
            "polychrome_sprite",
        ];

        let rust_binding_path = format!("{}/shaders_bytes.rs", out_dir);
        if Path::new(&rust_binding_path).exists() {
            fs::remove_file(&rust_binding_path)?;
        }
        for module in modules {
            let shader_str = shader_path
                .to_str()
                .ok_or_else(|| format!("着色器路径不是合法 UTF-8: {}", shader_path.display()))?;
            compile_shader_for_module(module, &out_dir, &fxc_path, shader_str, &rust_binding_path)?;
        }

        {
            let color_shader_path = Path::new(&manifest_dir).join("src/color_text_raster.hlsl");
            let shader_str = color_shader_path.to_str().ok_or_else(|| {
                format!("着色器路径不是合法 UTF-8: {}", color_shader_path.display())
            })?;
            compile_shader_for_module(
                "emoji_rasterization",
                &out_dir,
                &fxc_path,
                shader_str,
                &rust_binding_path,
            )?;
        }

        Ok(())
    }

    /// Locate `binary` in the newest installed Windows SDK.
    pub fn find_latest_windows_sdk_binary(
        binary: &str,
    ) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
        let key = windows_registry::LOCAL_MACHINE
            .open("SOFTWARE\\WOW6432Node\\Microsoft\\Microsoft SDKs\\Windows\\v10.0")?;

        let install_folder: String = key.get_string("InstallationFolder")?; // "C:\Program Files (x86)\Windows Kits\10\"
        let install_folder_bin = Path::new(&install_folder).join("bin");

        let mut versions: Vec<_> = std::fs::read_dir(&install_folder_bin)?
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();

        versions.sort_by_key(|s| {
            s.split('.')
                .filter_map(|p| p.parse().ok())
                .collect::<Vec<u32>>()
        });

        let arch = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            _ => Err(format!(
                "Unsupported architecture: {}",
                std::env::consts::ARCH
            ))?,
        };

        if let Some(highest_version) = versions.last() {
            return Ok(Some(
                install_folder_bin
                    .join(highest_version)
                    .join(arch)
                    .join(binary),
            ));
        }

        Ok(None)
    }

    /// 依次尝试 `GPUI_FXC_PATH` 环境变量、`PATH`（`where.exe fxc.exe`）、Windows SDK
    /// 安装目录，都找不到则返回错误。可用 `GPUI_FXC_PATH` 手动指定 `fxc.exe` 路径。
    fn find_fxc_compiler() -> Result<String, Box<dyn std::error::Error>> {
        // Check environment variable
        if let Ok(path) = std::env::var("GPUI_FXC_PATH")
            && Path::new(&path).exists()
        {
            return Ok(path);
        }

        // Try to find in PATH
        // NOTE: This has to be `where.exe` on Windows, not `where`, it must be ended with `.exe`
        if let Ok(output) = std::process::Command::new("where.exe")
            .arg("fxc.exe")
            .output()
            && output.status.success()
        {
            let path = String::from_utf8_lossy(&output.stdout);
            return Ok(path.trim().to_string());
        }

        if let Some(path) = find_latest_windows_sdk_binary("fxc.exe")? {
            return Ok(path.to_string_lossy().into_owned());
        }

        Err("Failed to find fxc.exe".into())
    }

    /// 编译单个模块的顶点/片元着色器，并生成对应的 Rust 绑定。
    fn compile_shader_for_module(
        module: &str,
        out_dir: &str,
        fxc_path: &str,
        shader_path: &str,
        rust_binding_path: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Compile vertex shader
        let output_file = format!("{}/{}_vs.h", out_dir, module);
        let const_name = format!("{}_VERTEX_BYTES", module.to_uppercase());
        compile_shader_impl(
            fxc_path,
            &format!("{module}_vertex"),
            &output_file,
            &const_name,
            shader_path,
            "vs_4_1",
        );
        generate_rust_binding(&const_name, &output_file, rust_binding_path)?;

        // Compile fragment shader
        let output_file = format!("{}/{}_ps.h", out_dir, module);
        let const_name = format!("{}_FRAGMENT_BYTES", module.to_uppercase());
        compile_shader_impl(
            fxc_path,
            &format!("{module}_fragment"),
            &output_file,
            &const_name,
            shader_path,
            "ps_4_1",
        );
        generate_rust_binding(&const_name, &output_file, rust_binding_path)?;

        Ok(())
    }

    fn compile_shader_impl(
        fxc_path: &str,
        entry_point: &str,
        output_path: &str,
        var_name: &str,
        shader_path: &str,
        target: &str,
    ) {
        let output = Command::new(fxc_path)
            .args([
                "/T",
                target,
                "/E",
                entry_point,
                "/Fh",
                output_path,
                "/Vn",
                var_name,
                "/O3",
                shader_path,
            ])
            .output();

        match output {
            Ok(result) => {
                if result.status.success() {
                    return;
                }
                println!(
                    "cargo::error=Shader compilation failed for {}:\n{}",
                    entry_point,
                    String::from_utf8_lossy(&result.stderr)
                );
                process::exit(1);
            }
            Err(e) => {
                println!("cargo::error=Failed to run fxc for {}: {}", entry_point, e);
                process::exit(1);
            }
        }
    }

    /// 把头文件里的字节数组转成 Rust 常量追加进绑定文件。
    fn generate_rust_binding(
        const_name: &str,
        head_file: &str,
        output_path: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let header_content = fs::read_to_string(head_file)?;
        let const_definition = {
            let global_var_start = header_content
                .find("const BYTE")
                .ok_or_else(|| format!("头文件里没有 `const BYTE` 数组: {head_file}"))?;
            let global_var = &header_content[global_var_start..];
            let equal = global_var
                .find('=')
                .ok_or_else(|| format!("头文件里的数组声明缺少 `=`: {head_file}"))?;
            global_var[equal + 1..].trim()
        };
        let rust_binding = format!(
            "const {}: &[u8] = &{}\n",
            const_name,
            const_definition.replace('{', "[").replace('}', "]")
        );
        let mut options = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(output_path)?;
        options.write_all(rust_binding.as_bytes())?;

        Ok(())
    }
}
