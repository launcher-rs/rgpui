//! Windows 原生通知实现
//!
//! 用 WinRT `Windows.UI.Notifications` 的 toast 通知，与 macOS 横幅、Linux XDG
//! 门户保持一致的观感。
//!
//! shell 只把通知投递给它认识的应用，而未打包（绿色运行）的应用得自己声明身份：
//! 把 AppUserModelID 写进 `HKCU\Software\Classes\AppUserModelId`，`DisplayName` 是
//! 通知上显示的应用名，`IconUri` 决定图标。注意应用名必须写在 `DisplayName` 值里，
//! 键的默认值 shell 不认；少了这一项时 `Show` 照样返回成功、通知也会落库，但 shell
//! 找不到应用名就不渲染横幅。

use std::sync::OnceLock;

use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::core::HSTRING;

use rgpui::Result;

/// 进程内缓存的应用标识，注册一次后不再重复写注册表
static APP_ID: OnceLock<String> = OnceLock::new();

/// 显示 Windows 原生 toast 通知
///
/// # 参数
/// * `title` - 通知标题
/// * `body` - 通知内容
///
/// # 返回
/// 成功时返回 `Ok(())`，失败时返回错误
pub fn show_toast_notification(title: &str, body: &str) -> Result<()> {
    ensure_com()?;
    let aumid = register_app()?;

    let doc = XmlDocument::new()?;
    doc.LoadXml(&HSTRING::from(toast_xml(title, body)))?;

    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(aumid))?;
    let toast = ToastNotification::CreateToastNotification(&doc)?;
    notifier.Show(&toast)?;

    Ok(())
}

/// 初始化 COM；线程已按其他套间模型初始化时（`RPC_E_CHANGED_MODE`）沿用现有模型
fn ensure_com() -> Result<()> {
    // WinRT 激活只要求当前线程完成 COM 初始化，不必用 RoInitialize
    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if hr.is_ok() || hr == RPC_E_CHANGED_MODE {
        Ok(())
    } else {
        Err(anyhow::anyhow!("初始化 COM 失败：{hr:?}"))
    }
}

/// 声明应用身份并返回其 AUMID
fn register_app() -> Result<&'static str> {
    if let Some(id) = APP_ID.get() {
        return Ok(id);
    }

    let exe = std::env::current_exe()?;
    let display = exe
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "rgpui".into());
    let id = format!("{}.rgpui", sanitize_id(&display));

    let key =
        windows_registry::CURRENT_USER.create(format!(r"Software\Classes\AppUserModelId\{id}"))?;
    key.set_string("DisplayName", &display)?;
    key.set_string("IconUri", exe.to_string_lossy().as_ref())?;

    Ok(APP_ID.get_or_init(|| id))
}

/// AUMID 只保留字母数字与 `._-`，其余字符换成 `_`
fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// 组装 toast 的 XML 模板，标题与正文按 XML 规则转义
fn toast_xml(title: &str, body: &str) -> String {
    let mut texts = String::new();
    for text in [title, body].into_iter().filter(|t| !t.is_empty()) {
        texts.push_str(&format!("<text>{}</text>", xml_escape(text)));
    }
    format!(r#"<toast><visual><binding template="ToastGeneric">{texts}</binding></visual></toast>"#)
}

/// 转义 XML 文本节点中的特殊字符，避免标题/正文里的标记破坏模板
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
