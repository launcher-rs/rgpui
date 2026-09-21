//! 移动端公共类型（1.4.0）：运行时平台判断与移动端共享 helper。
//!
//! 本模块是“分 crate”架构的共享层：`TargetPlatform` 等与具体 OS 无关的类型
//! 放在这里，桌面端零开销；`rgpui-android` / `rgpui-ios` 只实现各自平台。
//! 扩展新平台时照 `docs/1.4.0/1.4.0-dev-plan.md` §7.3 加变体 + 分支即可。

/// 应用当前运行的目标平台（编译期解析，运行时无开销）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetPlatform {
    /// Android（NDK / JNI）。
    Android,
    /// iOS（UIKit / Metal）。
    IOS,
    /// macOS（AppKit / Metal）。
    MacOS,
    /// Linux（含 FreeBSD，X11 / Wayland）。
    Linux,
    /// Windows（Win32）。
    Windows,
    /// Web（wasm / WebGPU）。
    Web,
}

impl TargetPlatform {
    /// 是否移动平台（Android 或 iOS）。
    pub fn is_mobile(self) -> bool {
        matches!(self, Self::Android | Self::IOS)
    }

    /// 是否桌面平台（macOS / Linux / Windows）。
    pub fn is_desktop(self) -> bool {
        matches!(self, Self::MacOS | Self::Linux | Self::Windows)
    }

    /// 是否 Android。
    pub fn is_android(self) -> bool {
        self == Self::Android
    }

    /// 是否 iOS。
    pub fn is_ios(self) -> bool {
        self == Self::IOS
    }

    /// 是否 macOS。
    pub fn is_macos(self) -> bool {
        self == Self::MacOS
    }

    /// 是否 Linux。
    pub fn is_linux(self) -> bool {
        self == Self::Linux
    }

    /// 是否 Windows。
    pub fn is_windows(self) -> bool {
        self == Self::Windows
    }

    /// 是否 Web（WASM）。
    pub fn is_web(self) -> bool {
        self == Self::Web
    }

    /// 是否苹果平台（iOS 或 macOS）。
    pub fn is_apple(self) -> bool {
        matches!(self, Self::IOS | Self::MacOS)
    }
}

impl std::fmt::Display for TargetPlatform {
    /// 返回平台展示名（如 `Android` / `Windows`）。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Android => write!(f, "Android"),
            Self::IOS => write!(f, "iOS"),
            Self::MacOS => write!(f, "macOS"),
            Self::Linux => write!(f, "Linux"),
            Self::Windows => write!(f, "Windows"),
            Self::Web => write!(f, "Web"),
        }
    }
}

/// 返回当前编译目标对应的 [`TargetPlatform`]。
///
/// 编译期经 `cfg` 解析，无运行时开销；应用层可用它代替满屏 `#[cfg]`。
pub fn target_platform() -> TargetPlatform {
    #[cfg(target_os = "android")]
    {
        TargetPlatform::Android
    }
    #[cfg(target_os = "ios")]
    {
        TargetPlatform::IOS
    }
    #[cfg(target_os = "macos")]
    {
        return TargetPlatform::MacOS;
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        return TargetPlatform::Linux;
    }
    #[cfg(target_os = "windows")]
    {
        return TargetPlatform::Windows;
    }
    #[cfg(target_family = "wasm")]
    {
        return TargetPlatform::Web;
    }
}

/// 当前编译目标的默认平台（`const` 上下文用，语义同 [`target_platform`]）。
pub const DEFAULT_PLATFORM: TargetPlatform = {
    #[cfg(target_os = "android")]
    {
        TargetPlatform::Android
    }
    #[cfg(target_os = "ios")]
    {
        TargetPlatform::IOS
    }
    #[cfg(target_os = "macos")]
    {
        TargetPlatform::MacOS
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        TargetPlatform::Linux
    }
    #[cfg(target_os = "windows")]
    {
        TargetPlatform::Windows
    }
    #[cfg(target_family = "wasm")]
    {
        TargetPlatform::Web
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    /// 当前主机 target 下 `target_platform()` 与常量一致。
    #[test]
    fn target_platform_matches_default_const() {
        assert_eq!(target_platform(), DEFAULT_PLATFORM);
    }

    /// `Display` 与 `is_*` 判断自洽。
    #[test]
    fn platform_predicates_agree_with_display() {
        let platform = target_platform();
        assert!(!format!("{platform}").is_empty());
        assert_eq!(
            platform.is_mobile(),
            matches!(platform, TargetPlatform::Android | TargetPlatform::IOS)
        );
        assert_eq!(
            platform.is_desktop(),
            matches!(
                platform,
                TargetPlatform::MacOS | TargetPlatform::Linux | TargetPlatform::Windows
            )
        );
        assert_eq!(
            platform.is_apple(),
            matches!(platform, TargetPlatform::IOS | TargetPlatform::MacOS)
        );
    }
}
