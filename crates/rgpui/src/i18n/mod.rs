//! 国际化 (I18n) 支持 —— 多语言文本管理和本地化。
//!
//! # 示例
//!
//! ```rust,ignore
//! use rgpui::i18n::{I18nManager, Locale};
//!
//! let mut i18n = I18nManager::new("en");
//! i18n.load_translations("zh-CN", include_str!("zh-CN.json"));
//! let text = i18n.t("hello", &[]);
//! ```

use std::collections::HashMap;

/// 语言代码。
pub type Locale = String;

/// 翻译文本。
#[derive(Debug, Clone)]
pub struct Translation {
    /// 原始 key。
    pub key: String,
    /// 翻译后的文本。
    pub value: String,
}

/// I18n 管理器。
pub struct I18nManager {
    /// 当前语言。
    current_locale: Locale,
    /// 所有翻译数据。
    translations: HashMap<Locale, HashMap<String, String>>,
    /// 回退语言。
    fallback_locale: Option<Locale>,
}

impl I18nManager {
    /// 创建新的 I18n 管理器。
    pub fn new(current_locale: &str) -> Self {
        Self {
            current_locale: current_locale.to_string(),
            translations: HashMap::new(),
            fallback_locale: None,
        }
    }

    /// 设置回退语言。
    pub fn with_fallback(mut self, fallback: &str) -> Self {
        self.fallback_locale = Some(fallback.to_string());
        self
    }

    /// 加载翻译数据。
    pub fn load_translations(&mut self, locale: &str, json: &str) -> anyhow::Result<()> {
        let data: HashMap<String, String> = serde_json::from_str(json)?;
        self.translations.insert(locale.to_string(), data);
        Ok(())
    }

    /// 加载翻译数据（HashMap）。
    pub fn load_translations_map(&mut self, locale: &str, map: HashMap<String, String>) {
        self.translations.insert(locale.to_string(), map);
    }

    /// 获取翻译文本。
    pub fn t(&self, key: &str, args: &[(&str, &str)]) -> String {
        let value = self.get_translated_text(key);
        self.format_text(&value, args)
    }

    /// 获取翻译后的文本（不格式化参数）。
    fn get_translated_text(&self, key: &str) -> String {
        // 先尝试当前语言
        if let Some(lang_translations) = self.translations.get(&self.current_locale) {
            if let Some(value) = lang_translations.get(key) {
                return value.clone();
            }
        }

        // 尝试回退语言
        if let Some(ref fallback) = self.fallback_locale {
            if let Some(lang_translations) = self.translations.get(fallback) {
                if let Some(value) = lang_translations.get(key) {
                    return value.clone();
                }
            }
        }

        // 返回 key 本身作为默认值
        key.to_string()
    }

    /// 格式化文本，替换占位符 {name}。
    fn format_text(&self, text: &str, args: &[(&str, &str)]) -> String {
        let mut result = text.to_string();
        for (key, value) in args {
            result = result.replace(&format!("{{{}}}", key), value);
        }
        result
    }

    /// 获取当前语言。
    pub fn current_locale(&self) -> &str {
        &self.current_locale
    }

    /// 切换语言。
    pub fn set_locale(&mut self, locale: &str) {
        self.current_locale = locale.to_string();
    }

    /// 获取所有已加载的语言。
    pub fn available_locales(&self) -> Vec<&str> {
        self.translations.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for I18nManager {
    fn default() -> Self {
        Self::new("en")
    }
}

/// 国际化文本组件 —— 自动根据当前语言显示文本。
pub struct I18nText {
    /// 翻译 key。
    key: String,
    /// 格式化参数。
    args: Vec<(String, String)>,
}

impl I18nText {
    /// 创建新的国际化文本。
    pub fn new(key: &str) -> Self {
        Self {
            key: key.to_string(),
            args: Vec::new(),
        }
    }

    /// 添加格式化参数。
    pub fn with_arg(mut self, key: &str, value: &str) -> Self {
        self.args.push((key.to_string(), value.to_string()));
        self
    }

    /// 获取翻译后的文本。
    pub fn translate(&self, i18n: &I18nManager) -> String {
        let args: Vec<(&str, &str)> = self
            .args
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        i18n.t(&self.key, &args)
    }
}

/// 复数形式支持。
pub struct PluralRule {
    /// 数量。
    pub count: i64,
    /// 单数形式 key。
    pub singular: String,
    /// 复数形式 key。
    pub plural: String,
}

impl PluralRule {
    /// 创建复数规则。
    pub fn new(count: i64, singular: &str, plural: &str) -> Self {
        Self {
            count,
            singular: singular.to_string(),
            plural: plural.to_string(),
        }
    }

    /// 获取对应形式的 key。
    pub fn key(&self) -> &str {
        if self.count == 1 {
            &self.singular
        } else {
            &self.plural
        }
    }

    /// 获取格式化后的文本。
    pub fn translate(&self, i18n: &I18nManager) -> String {
        let key = self.key();
        i18n.t(key, &[("count", &self.count.to_string())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_i18n_manager_creation() {
        let manager = I18nManager::new("en");
        assert_eq!(manager.current_locale(), "en");
    }

    #[test]
    fn test_load_translations() {
        let mut manager = I18nManager::new("en");
        let mut translations = HashMap::new();
        translations.insert("hello".to_string(), "Hello".to_string());
        translations.insert("world".to_string(), "World".to_string());
        manager.load_translations_map("en", translations);
        assert_eq!(manager.t("hello", &[]), "Hello");
        assert_eq!(manager.t("world", &[]), "World");
    }

    #[test]
    fn test_fallback_locale() {
        let mut manager = I18nManager::new("zh-CN").with_fallback("en");
        let mut en_translations = HashMap::new();
        en_translations.insert("hello".to_string(), "Hello".to_string());
        manager.load_translations_map("en", en_translations);
        assert_eq!(manager.t("hello", &[]), "Hello");
    }

    #[test]
    fn test_format_text() {
        let mut manager = I18nManager::new("en");
        let mut translations = HashMap::new();
        translations.insert("greeting".to_string(), "Hello, {name}!".to_string());
        manager.load_translations_map("en", translations);
        assert_eq!(manager.t("greeting", &[("name", "World")]), "Hello, World!");
    }

    #[test]
    fn test_set_locale() {
        let mut manager = I18nManager::new("en");
        manager.set_locale("zh-CN");
        assert_eq!(manager.current_locale(), "zh-CN");
    }

    #[test]
    fn test_plural_rule() {
        let rule = PluralRule::new(1, "item", "items");
        assert_eq!(rule.key(), "item");
        let rule = PluralRule::new(5, "item", "items");
        assert_eq!(rule.key(), "items");
    }
}
