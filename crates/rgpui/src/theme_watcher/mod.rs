//! 主题热重载 —— 运行时主题切换和监听。
//!
//! # 示例
//!
//! ```rust,ignore
//! use rgpui::theme_watcher::{ThemeWatcher, ThemeEvent};
//!
//! let mut watcher = ThemeWatcher::new();
//! watcher.on_change(|theme| {
//!     println!("主题已变更: {:?}", theme);
//! });
//! ```

use std::sync::Arc;

/// 主题类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeMode {
    /// 明亮主题。
    Light,
    /// 暗色主题。
    Dark,
    /// 跟随系统。
    System,
}

/// 主题事件。
#[derive(Debug, Clone)]
pub enum ThemeEvent {
    /// 主题已变更。
    ThemeChanged(ThemeMode),
    /// 主题颜色已更新。
    ColorsUpdated,
}

/// 主题监听器。
pub struct ThemeWatcher {
    /// 当前主题模式。
    current_theme: ThemeMode,
    /// 监听器回调。
    listeners: Vec<Arc<dyn Fn(&ThemeEvent) + Send + Sync>>,
    /// 是否启用。
    enabled: bool,
}

impl ThemeWatcher {
    /// 创建新的主题监听器。
    pub fn new() -> Self {
        Self {
            current_theme: ThemeMode::System,
            listeners: Vec::new(),
            enabled: true,
        }
    }

    /// 创建指定初始主题的监听器。
    pub fn with_theme(theme: ThemeMode) -> Self {
        Self {
            current_theme: theme,
            listeners: Vec::new(),
            enabled: true,
        }
    }

    /// 获取当前主题模式。
    pub fn current_theme(&self) -> &ThemeMode {
        &self.current_theme
    }

    /// 切换主题。
    pub fn set_theme(&mut self, theme: ThemeMode) {
        if self.current_theme != theme {
            self.current_theme = theme.clone();
            self.notify_listeners(&ThemeEvent::ThemeChanged(theme));
        }
    }

    /// 切换到下一个主题。
    pub fn toggle_theme(&mut self) {
        let next = match &self.current_theme {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::System,
            ThemeMode::System => ThemeMode::Light,
        };
        self.set_theme(next);
    }

    /// 监听主题变化。
    pub fn on_change<F>(&mut self, callback: F)
    where
        F: Fn(&ThemeEvent) + Send + Sync + 'static,
    {
        self.listeners.push(Arc::new(callback));
    }

    /// 通知监听器。
    fn notify_listeners(&self, event: &ThemeEvent) {
        if !self.enabled {
            return;
        }
        for listener in &self.listeners {
            listener(event);
        }
    }

    /// 启用/禁用监听。
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// 获取监听状态。
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for ThemeWatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// 主题颜色配置。
#[derive(Debug, Clone)]
pub struct ThemeColors {
    /// 主背景色。
    pub background: String,
    /// 次背景色。
    pub surface: String,
    /// 主文本色。
    pub text: String,
    /// 次文本色。
    pub text_secondary: String,
    /// 强调色。
    pub accent: String,
    /// 边框色。
    pub border: String,
}

impl ThemeColors {
    /// 创建亮色主题。
    pub fn light() -> Self {
        Self {
            background: "#ffffff".to_string(),
            surface: "#f5f5f5".to_string(),
            text: "#000000".to_string(),
            text_secondary: "#666666".to_string(),
            accent: "#0066ff".to_string(),
            border: "#e0e0e0".to_string(),
        }
    }

    /// 创建暗色主题。
    pub fn dark() -> Self {
        Self {
            background: "#1e1e1e".to_string(),
            surface: "#2d2d2d".to_string(),
            text: "#ffffff".to_string(),
            text_secondary: "#999999".to_string(),
            accent: "#4d9fff".to_string(),
            border: "#404040".to_string(),
        }
    }

    /// 获取指定主题的颜色。
    pub fn for_theme(mode: &ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => Self::light(),
            ThemeMode::Dark => Self::dark(),
            ThemeMode::System => Self::dark(), // 默认暗色，实际应检测系统
        }
    }
}

/// 主题管理器 —— 统一管理主题颜色和热重载。
pub struct ThemeManager {
    /// 主题监听器。
    watcher: ThemeWatcher,
    /// 当前颜色配置。
    colors: ThemeColors,
}

impl ThemeManager {
    /// 创建新的主题管理器。
    pub fn new() -> Self {
        let watcher = ThemeWatcher::new();
        let colors = ThemeColors::for_theme(watcher.current_theme());

        Self { watcher, colors }
    }

    /// 创建指定初始主题的管理器。
    pub fn with_theme(theme: ThemeMode) -> Self {
        let watcher = ThemeWatcher::with_theme(theme.clone());
        let colors = ThemeColors::for_theme(&theme);

        Self { watcher, colors }
    }

    /// 获取当前颜色配置。
    pub fn colors(&self) -> &ThemeColors {
        &self.colors
    }

    /// 获取主题监听器引用。
    pub fn watcher(&self) -> &ThemeWatcher {
        &self.watcher
    }

    /// 获取可变主题监听器引用。
    pub fn watcher_mut(&mut self) -> &mut ThemeWatcher {
        &mut self.watcher
    }

    /// 切换主题。
    pub fn set_theme(&mut self, theme: ThemeMode) {
        self.watcher.set_theme(theme.clone());
        self.colors = ThemeColors::for_theme(&theme);
    }

    /// 监听主题变化。
    pub fn on_change<F>(&mut self, callback: F)
    where
        F: Fn(&ThemeEvent) + Send + Sync + 'static,
    {
        self.watcher.on_change(callback);
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_theme_watcher_creation() {
        let watcher = ThemeWatcher::new();
        assert_eq!(*watcher.current_theme(), ThemeMode::System);
        assert!(watcher.is_enabled());
    }

    #[test]
    fn test_set_theme() {
        let mut watcher = ThemeWatcher::new();
        watcher.set_theme(ThemeMode::Dark);
        assert_eq!(*watcher.current_theme(), ThemeMode::Dark);
    }

    #[test]
    fn test_toggle_theme() {
        let mut watcher = ThemeWatcher::with_theme(ThemeMode::Light);
        watcher.toggle_theme();
        assert_eq!(*watcher.current_theme(), ThemeMode::Dark);
    }

    #[test]
    fn test_on_change_callback() {
        let mut watcher = ThemeWatcher::new();
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        watcher.on_change(move |_| {
            called_clone.store(true, Ordering::SeqCst);
        });
        watcher.set_theme(ThemeMode::Dark);
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_theme_colors() {
        let light = ThemeColors::light();
        assert_eq!(light.background, "#ffffff");
        let dark = ThemeColors::dark();
        assert_eq!(dark.background, "#1e1e1e");
    }

    #[test]
    fn test_theme_manager() {
        let mut manager = ThemeManager::new();
        assert_eq!(*manager.watcher().current_theme(), ThemeMode::System);
        manager.set_theme(ThemeMode::Light);
        assert_eq!(manager.colors().background, "#ffffff");
    }
}
