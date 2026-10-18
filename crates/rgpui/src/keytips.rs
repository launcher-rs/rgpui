//! 按键提示（KeyTips）—— 套件风格的逐层按键导航基础件。
//!
//! 按 `Alt` / `F10` 后，功能区选项卡上会出现字母提示（如 `H` 表示「开始」）；
//! 按下提示键进入该选项卡，选项卡内各命令再显示一层提示键，按下即执行命令。
//! 提示键可由 [`CommandDescriptor`](crate::CommandDescriptor) 统一登记，
//! 让同一份命令元数据同时驱动功能区、快捷键、命令面板与按键提示。
//!
//! 本模块提供与具体控件无关的两块拼装件：
//!
//! - [`KeyTip`]：按键提示小徽标元素；
//! - [`KeyTipScope`] / [`KeyTipMatch`] / [`resolve_key`]：按键提示的层级状态与
//!   逐层按键解析（支持 `H` → `O` 这类多字符提示）。
//!
//! 具体的功能区交互见 [`crate::ribbon`]。

use crate::{
    ActiveTheme as _, App, FontWeight, Hsla, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled as _, Window, div, px,
};

/// 按键提示的当前层级。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyTipScope {
    /// 未激活。
    #[default]
    Hidden,
    /// 显示选项卡层的按键提示，等待选择选项卡。
    Tabs,
    /// 已进入某个选项卡，显示其命令的按键提示，等待选择命令。
    Controls,
}

/// 一次按键在候选提示中的解析结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyTipMatch {
    /// 命中第 `index` 个候选。
    Match(usize),
    /// 已输入的前缀仍是若干候选的前缀，需要继续输入。
    Pending,
    /// 无任何候选匹配。
    None,
}

/// 逐层解析一次按键。
///
/// `prefix` 是此前已输入但尚未定位到唯一候选的前缀（可为空），`key` 是本次按键，
/// `keytips` 是按展示顺序排列的候选提示。空串候选会被跳过。
///
/// 返回 [`KeyTipMatch::Pending`] 表示还需要再输入一个字符继续缩小范围（用于
/// `H`、`O` 这类多字符提示），此时调用方应把 `prefix + key` 存为新的前缀。
pub fn resolve_key<T: AsRef<str>>(prefix: &str, key: &str, keytips: &[T]) -> KeyTipMatch {
    let query = format!("{prefix}{key}").to_ascii_uppercase();
    if query.is_empty() {
        return KeyTipMatch::None;
    }
    let mut matched: Option<usize> = None;
    let mut has_deeper = false;

    for (index, keytip) in keytips.iter().enumerate() {
        let keytip = keytip.as_ref();
        if keytip.is_empty() {
            continue;
        }
        let keytip = keytip.to_ascii_uppercase();
        if keytip == query {
            if matched.is_none() {
                matched = Some(index);
            }
        } else if keytip.starts_with(&query) {
            has_deeper = true;
        }
    }

    match (matched, has_deeper) {
        (Some(index), _) => KeyTipMatch::Match(index),
        (None, true) => KeyTipMatch::Pending,
        (None, false) => KeyTipMatch::None,
    }
}

/// 判断某个提示是否应在当前前缀下显示（提示非空且以前缀开头）。
pub fn keytip_visible(keytip: &str, prefix: &str) -> bool {
    !keytip.is_empty()
        && keytip
            .to_ascii_uppercase()
            .starts_with(&prefix.to_ascii_uppercase())
}

/// 从标签派生默认提示键（取首个 ASCII 字母/数字并大写）。
///
/// 中文等非 ASCII 标签不派生提示，避免出现无法键入的提示。
pub fn derive_keytip(label: &str) -> Option<SharedString> {
    label
        .chars()
        .find(|c| c.is_ascii_alphanumeric())
        .map(|c| SharedString::from(c.to_ascii_uppercase().to_string()))
}

/// 按键提示小徽标。
#[derive(IntoElement)]
pub struct KeyTip {
    text: SharedString,
    color: Option<Hsla>,
}

impl KeyTip {
    /// 以提示文本创建徽标。
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            color: None,
        }
    }

    /// 覆盖背景色（默认取主题强调色，常用于上下文选项卡）。
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl RenderOnce for KeyTip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let background = self.color.unwrap_or(theme.accent);

        div()
            .px(px(4.0))
            .py(px(0.5))
            .rounded_sm()
            .bg(background)
            .text_color(theme.accent_foreground)
            .text_size(px(10.0))
            .font_weight(FontWeight::SEMIBOLD)
            .child(self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyTipMatch, derive_keytip, keytip_visible, resolve_key};

    /// 前缀为空时按键直接命中唯一候选。
    #[test]
    fn resolves_single_key() {
        let keytips = ["H", "N", "V"];
        assert_eq!(resolve_key("", "h", &keytips), KeyTipMatch::Match(0));
        assert_eq!(resolve_key("", "N", &keytips), KeyTipMatch::Match(1));
        assert_eq!(resolve_key("", "x", &keytips), KeyTipMatch::None);
    }

    /// 多字符提示先给出 Pending，再补全后命中。
    #[test]
    fn resolves_multi_char_prefix() {
        let keytips = ["HO", "HX", "N"];
        assert_eq!(resolve_key("", "h", &keytips), KeyTipMatch::Pending);
        assert_eq!(resolve_key("H", "O", &keytips), KeyTipMatch::Match(0));
        assert_eq!(resolve_key("H", "x", &keytips), KeyTipMatch::Match(1));
        assert_eq!(resolve_key("H", "z", &keytips), KeyTipMatch::None);
    }

    /// 同名首字母且均以之开头时保持 Pending，由后续按键区分。
    #[test]
    fn prefers_match_over_deeper_prefix() {
        let keytips = ["H", "HO"];
        assert_eq!(resolve_key("", "h", &keytips), KeyTipMatch::Match(0));
    }

    /// 空串候选被跳过。
    #[test]
    fn skips_empty_candidates() {
        let keytips = ["", "N"];
        assert_eq!(resolve_key("", "n", &keytips), KeyTipMatch::Match(1));
        assert_eq!(resolve_key("", "", &keytips), KeyTipMatch::None);
    }

    /// 匹配忽略大小写。
    #[test]
    fn matching_is_case_insensitive() {
        let keytips = ["pa", "pb"];
        assert_eq!(resolve_key("P", "A", &keytips), KeyTipMatch::Match(0));
    }

    /// 提示仅在以前缀开头时显示。
    #[test]
    fn visibility_follows_prefix() {
        assert!(keytip_visible("H", ""));
        assert!(keytip_visible("HO", "h"));
        assert!(!keytip_visible("N", "h"));
        assert!(!keytip_visible("", ""));
    }

    /// 默认提示取首个 ASCII 字母或数字，中文标签不派生。
    #[test]
    fn derives_from_ascii_label() {
        assert_eq!(derive_keytip("Paste").as_deref(), Some("P"));
        assert_eq!(derive_keytip("3D").as_deref(), Some("3"));
        assert_eq!(derive_keytip("粘贴"), None);
        assert_eq!(derive_keytip("").as_deref(), None);
    }
}
