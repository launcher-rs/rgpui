use rgpui::PlatformKeyboardLayout;

/// Web 平台键盘布局实现
pub struct WebKeyboardLayout;

impl WebKeyboardLayout {
    /// 创建新的 WebKeyboardLayout 实例
    pub fn new() -> Self {
        WebKeyboardLayout
    }
}

impl PlatformKeyboardLayout for WebKeyboardLayout {
    fn id(&self) -> &str {
        "us"
    }

    fn name(&self) -> &str {
        "US"
    }
}
