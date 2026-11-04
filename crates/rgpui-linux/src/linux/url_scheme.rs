//! Linux URL scheme 注册
//!
//! 按 XDG 规范落一份桌面入口到 `$XDG_DATA_HOME/applications`（默认
//! `~/.local/share/applications`），再用 `xdg-mime default` 把它设为
//! `x-scheme-handler/<scheme>` 的默认处理器。全部写用户目录，不需要 root。

use std::fs;
use std::path::PathBuf;

use anyhow::{Context as _, Result, anyhow};

/// 把本应用注册为 `scheme` 的 URL 处理器。
///
/// # 参数
/// * `scheme` - URL scheme（例如 `myapp`，不含 `://`）
///
/// # 返回
/// 桌面入口写入且 `xdg-mime` 执行成功时返回 `Ok(())`。
pub(crate) fn register_url_scheme(scheme: &str) -> Result<()> {
    validate_scheme(scheme)?;

    let exe = std::env::current_exe().context("无法获取当前可执行文件路径")?;
    let applications_dir = applications_dir()?;
    fs::create_dir_all(&applications_dir)
        .with_context(|| format!("创建应用目录失败: {}", applications_dir.display()))?;

    let desktop_file_name = format!("{scheme}.desktop");
    let desktop_file = applications_dir.join(&desktop_file_name);
    fs::write(&desktop_file, desktop_entry(scheme, &exe.to_string_lossy()))
        .with_context(|| format!("写入桌面入口失败: {}", desktop_file.display()))?;

    let handler = format!("x-scheme-handler/{scheme}");
    let status = std::process::Command::new("xdg-mime")
        .args(["default", &desktop_file_name, &handler])
        .status()
        .context("无法执行 xdg-mime（请确认已安装 xdg-utils）")?;
    if !status.success() {
        return Err(anyhow!("xdg-mime default 退出码非零: {status}"));
    }

    // 刷新桌面数据库失败不影响注册本身（很多轻量桌面没有这个工具）
    if let Err(error) = std::process::Command::new("update-desktop-database")
        .arg(&applications_dir)
        .status()
    {
        log::debug!("update-desktop-database 不可用: {error}");
    }

    Ok(())
}

/// 返回 `$XDG_DATA_HOME/applications`，未设置时按规范回退到 `~/.local/share`。
fn applications_dir() -> Result<PathBuf> {
    let data_home = match std::env::var_os("XDG_DATA_HOME") {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => {
            let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME 未设置"))?;
            PathBuf::from(home).join(".local/share")
        }
    };
    Ok(data_home.join("applications"))
}

/// RFC 3986 的 scheme 语法：字母开头，后续允许字母、数字、`+`、`-`、`.`。
fn validate_scheme(scheme: &str) -> Result<()> {
    let mut chars = scheme.chars();
    let valid_start = chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic());
    let valid_rest = chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'));
    if valid_start && valid_rest {
        Ok(())
    } else {
        Err(anyhow!("非法 URL scheme: {scheme:?}"))
    }
}

/// 生成桌面入口内容。
///
/// `%u` 让桌面环境把完整 URL 作为单个参数传给应用；`NoDisplay=true` 避免
/// 这个仅用于协议注册的入口出现在应用菜单里。
fn desktop_entry(scheme: &str, exe: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={scheme}\n\
         Exec={} %u\n\
         NoDisplay=true\n\
         Terminal=false\n\
         MimeType=x-scheme-handler/{scheme};\n",
        quote_exec_arg(exe)
    )
}

/// 按 Desktop Entry 规范给 Exec 参数加引号：仅引号、反引号、`$` 与反斜杠需要转义。
fn quote_exec_arg(arg: &str) -> String {
    let mut escaped = String::with_capacity(arg.len() + 2);
    escaped.push('"');
    for ch in arg.chars() {
        if matches!(ch, '"' | '`' | '$' | '\\') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped.push('"');
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    /// scheme 校验：字母开头、只允许 RFC 3986 的合法字符。
    #[test]
    fn validates_schemes() {
        for scheme in ["myapp", "my-app", "app.v2", "x1"] {
            assert!(validate_scheme(scheme).is_ok(), "{scheme} 应该合法");
        }
        for scheme in ["", "1app", "my app", "my/app", "my_app"] {
            assert!(validate_scheme(scheme).is_err(), "{scheme} 应该被拒绝");
        }
    }

    /// 桌面入口必须声明 MimeType 与 %u，路径中的特殊字符要转义。
    #[test]
    fn builds_desktop_entry() {
        let entry = desktop_entry("myapp", "/opt/my app/bin");
        assert!(entry.contains("MimeType=x-scheme-handler/myapp;"));
        assert!(entry.contains("Exec=\"/opt/my app/bin\" %u"));
        assert_eq!(quote_exec_arg(r#"a"b$c\d`e"#), r#""a\"b\$c\\d\`e""#);
    }
}
