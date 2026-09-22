use std::path::PathBuf;
use std::process::Command;

/// 查找 adb 可执行文件路径：先查 PATH，再查 ANDROID_HOME/platform-tools。
/// 找不到返回 None。
pub fn find_adb() -> Option<String> {
    // 1. PATH 中直接可用（which 式探测）
    if let Ok(output) = Command::new("adb").arg("version").output()
        && output.status.success()
    {
        return Some("adb".to_string());
    }
    // 2. ANDROID_HOME / ANDROID_SDK_ROOT 的 platform-tools
    let sdk = std::env::var("ANDROID_HOME")
        .or_else(|_| std::env::var("ANDROID_SDK_ROOT"))
        .ok()?;
    let exe = if cfg!(windows) { "adb.exe" } else { "adb" };
    let candidate: PathBuf = PathBuf::from(&sdk).join("platform-tools").join(exe);
    if candidate.exists() {
        return Some(candidate.to_string_lossy().to_string());
    }
    None
}

/// 执行 adb 命令并返回是否成功。adb 找不到时返回 Err。
pub fn run_adb(args: &[&str]) -> anyhow::Result<bool> {
    let adb = find_adb()
        .ok_or_else(|| anyhow::anyhow!("adb 未找到，请安装 platform-tools 或设置 ANDROID_HOME"))?;
    let status = Command::new(&adb).args(args).status()?;
    Ok(status.success())
}

/// 执行 adb 命令并捕获 stdout（trim 后）。失败或找不到返回 None。
pub fn adb_output(args: &[&str]) -> Option<String> {
    let adb = find_adb()?;
    let output = Command::new(&adb).args(args).output().ok()?;
    if output.status.success() {
        String::from_utf8(output.stdout)
            .ok()
            .map(|s| s.trim().to_string())
    } else {
        None
    }
}
