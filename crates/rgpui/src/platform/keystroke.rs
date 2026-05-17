use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt::{Display, Write},
};

use crate::PlatformKeyboardMapper;

/// 这是一个辅助 trait，用于简化某些函数的实现
pub trait AsKeystroke {
    /// 返回按键的 GPUI 表示。
    fn as_keystroke(&self) -> &Keystroke;
}

/// 由平台生成的按键及关联元数据
#[derive(Clone, Debug, Eq, PartialEq, Default, Deserialize, Hash)]
pub struct Keystroke {
    /// 生成按键时的修饰键状态
    pub modifiers: Modifiers,

    /// key 是被按下的键上打印的字符
    /// 例如，对于 option-s，key 为 "s"
    /// 在没有 ascii 键的布局上（例如泰语）
    /// 这将是 ASCII 等效字符（q 而不是 ๆ），
    /// 而输入的字符将出现在 key_char 中。
    pub key: String,

    /// key_char 是按下此绑定时可能输入的字符。
    /// 例如，对于 s 是 "s"，对于 option-s 是 "ß"，对于 cmd-s 是 None
    pub key_char: Option<String>,
}

/// 表示可用于键绑定并向用户显示的按键。
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct KeybindingKeystroke {
    /// 按键的 GPUI 表示。
    inner: Keystroke,
    /// 要显示的修饰键。
    #[cfg(target_os = "windows")]
    display_modifiers: Modifiers,
    /// 要显示的键。
    #[cfg(target_os = "windows")]
    display_key: String,
}

/// `Keystroke::parse` 的错误类型。使用此类型而不是 `anyhow::Error`，以便 Zed 可以使用
/// markdown 来显示它。
#[derive(Debug)]
pub struct InvalidKeystrokeError {
    /// 无效的按键。
    pub keystroke: String,
}

impl Error for InvalidKeystrokeError {}

impl Display for InvalidKeystrokeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Invalid keystroke \"{}\". {}",
            self.keystroke, KEYSTROKE_PARSE_EXPECTED_MESSAGE
        )
    }
}

/// 按键解析器期望的格式说明，以 "Expected ..." 开头
pub const KEYSTROKE_PARSE_EXPECTED_MESSAGE: &str = "Expected a sequence of modifiers \
    (`ctrl`, `alt`, `shift`, `fn`, `cmd`, `super`, or `win`) \
    followed by a key, separated by `-`.";

impl Keystroke {
    /// 在匹配键时，我们无法知道用户是想要输入 key_char 还是 key 本身。在某些非美式键盘上，
    /// 我们绑定中使用的键位于 option 后面（例如 $ 在捷克键盘上输入为 alt-ç），
    /// 在某些键盘上，IME 处理器会将一系列键转换为特定字符（例如 " 在巴西键盘上输入为 " 空格"）。
    ///
    /// 此方法假设 `self` 是输入的，而 `target` 在键映射中，并检查
    /// self 的两种可能性与 target 的匹配。
    pub fn should_match(&self, target: &KeybindingKeystroke) -> bool {
        #[cfg(not(target_os = "windows"))]
        if let Some(key_char) = self
            .key_char
            .as_ref()
            .filter(|key_char| key_char != &&self.key)
        {
            let ime_modifiers = Modifiers {
                control: self.modifiers.control,
                platform: self.modifiers.platform,
                ..Default::default()
            };

            if &target.inner.key == key_char && target.inner.modifiers == ime_modifiers {
                return true;
            }
        }

        #[cfg(target_os = "windows")]
        if let Some(key_char) = self
            .key_char
            .as_ref()
            .filter(|key_char| key_char != &&self.key)
        {
            // 在 Windows 上，如果设置了 key_char，则输入的按键产生了 key_char
            if &target.inner.key == key_char && target.inner.modifiers == Modifiers::none() {
                return true;
            }
        }

        target.inner.modifiers == self.modifiers && target.inner.key == self.key
    }

    /// key 语法：
    /// [secondary-][ctrl-][alt-][shift-][cmd-][fn-]key[->key_char]
    /// key_char 语法仅用于生成测试事件，
    /// secondary 在 macOS 上表示 "cmd"，在其他平台上表示 "ctrl"
    /// 匹配带有 key_char 的键时，将不带 key_char 进行匹配。
    pub fn parse(source: &str) -> std::result::Result<Self, InvalidKeystrokeError> {
        let mut modifiers = Modifiers::none();
        let mut key = None;
        let mut key_char = None;

        let mut components = source.split('-').peekable();
        while let Some(component) = components.next() {
            if component.eq_ignore_ascii_case("ctrl") {
                modifiers.control = true;
                continue;
            }
            if component.eq_ignore_ascii_case("alt") {
                modifiers.alt = true;
                continue;
            }
            if component.eq_ignore_ascii_case("shift") {
                modifiers.shift = true;
                continue;
            }
            if component.eq_ignore_ascii_case("fn") {
                modifiers.function = true;
                continue;
            }
            if component.eq_ignore_ascii_case("secondary") {
                if cfg!(target_os = "macos") {
                    modifiers.platform = true;
                } else {
                    modifiers.control = true;
                };
                continue;
            }

            let is_platform = component.eq_ignore_ascii_case("cmd")
                || component.eq_ignore_ascii_case("super")
                || component.eq_ignore_ascii_case("win");

            if is_platform {
                modifiers.platform = true;
                continue;
            }

            let mut key_str = component.to_string();

            if let Some(next) = components.peek() {
                if next.is_empty() && source.ends_with('-') {
                    key = Some(String::from("-"));
                    break;
                } else if next.len() > 1 && next.starts_with('>') {
                    key = Some(key_str);
                    key_char = Some(String::from(&next[1..]));
                    components.next();
                } else {
                    return Err(InvalidKeystrokeError {
                        keystroke: source.to_owned(),
                    });
                }
                continue;
            }

            if component.len() == 1 && component.as_bytes()[0].is_ascii_uppercase() {
                // 转换为 shift + 小写字符
                modifiers.shift = true;
                key_str.make_ascii_lowercase();
            } else {
                // 将 ascii 字符转换为小写，以便 "tab" 和 "enter" 等命名键
                // 可以不分大小写地被接受，并按我们期望的方式存储，以便正确匹配
                key_str.make_ascii_lowercase()
            }
            key = Some(key_str);
        }

        // 允许用户将修饰键作为键本身
        // 这将 `key` 设置为修饰键，并禁用该修饰键
        key = key.or_else(|| {
            use std::mem;
            // std::mem::take 清除 bool 值（如果为 true）
            if mem::take(&mut modifiers.shift) {
                Some("shift".to_string())
            } else if mem::take(&mut modifiers.control) {
                Some("control".to_string())
            } else if mem::take(&mut modifiers.alt) {
                Some("alt".to_string())
            } else if mem::take(&mut modifiers.platform) {
                Some("platform".to_string())
            } else if mem::take(&mut modifiers.function) {
                Some("function".to_string())
            } else {
                None
            }
        });

        let key = key.ok_or_else(|| InvalidKeystrokeError {
            keystroke: source.to_owned(),
        })?;

        Ok(Keystroke {
            modifiers,
            key,
            key_char,
        })
    }

    /// 生成此键的可被 Parse 理解的表示。
    pub fn unparse(&self) -> String {
        unparse(&self.modifiers, &self.key)
    }

    /// 返回此按键是否使 IME 系统处于未完成状态。
    pub fn is_ime_in_progress(&self) -> bool {
        self.key_char.is_none()
            && (is_printable_key(&self.key) || self.key.is_empty())
            && !(self.modifiers.platform
                || self.modifiers.control
                || self.modifiers.function
                || self.modifiers.alt)
    }

    /// 返回填充了 key_char 的新按键。
    /// 这用于 dispatch_keystroke，我们希望用户能够
    /// 模拟输入 "space" 等。
    pub fn with_simulated_ime(mut self) -> Self {
        if self.key_char.is_none()
            && !self.modifiers.platform
            && !self.modifiers.control
            && !self.modifiers.function
            && !self.modifiers.alt
        {
            self.key_char = match self.key.as_str() {
                "space" => Some(" ".into()),
                "tab" => Some("\t".into()),
                "enter" => Some("\n".into()),
                key if !is_printable_key(key) || key.is_empty() => None,
                key => {
                    if self.modifiers.shift {
                        Some(key.to_uppercase())
                    } else {
                        Some(key.into())
                    }
                }
            }
        }
        self
    }
}

impl KeybindingKeystroke {
    #[cfg(target_os = "windows")]
    #[expect(missing_docs)]
    pub fn new(inner: Keystroke, display_modifiers: Modifiers, display_key: String) -> Self {
        KeybindingKeystroke {
            inner,
            display_modifiers,
            display_key,
        }
    }

    /// 使用给定的键盘映射器从给定的按键创建新的键绑定按键。
    pub fn new_with_mapper(
        inner: Keystroke,
        use_key_equivalents: bool,
        keyboard_mapper: &dyn PlatformKeyboardMapper,
    ) -> Self {
        keyboard_mapper.map_key_equivalent(inner, use_key_equivalents)
    }

    /// 从给定的按键创建新的键绑定按键，不进行任何平台特定的映射。
    pub fn from_keystroke(keystroke: Keystroke) -> Self {
        #[cfg(target_os = "windows")]
        {
            let key = keystroke.key.clone();
            let modifiers = keystroke.modifiers;
            KeybindingKeystroke {
                inner: keystroke,
                display_modifiers: modifiers,
                display_key: key,
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            KeybindingKeystroke { inner: keystroke }
        }
    }

    /// 返回按键的 GPUI 表示。
    pub fn inner(&self) -> &Keystroke {
        &self.inner
    }

    /// 返回修饰键。
    ///
    /// 平台特定行为：
    /// - 在 macOS 和 Linux 上，此修饰键与 `inner.modifiers` 相同，即按键的 GPUI 表示。
    /// - 在 Windows 上，此修饰键是显示修饰键，例如 `ctrl-@` 按键的 `inner.modifiers` 为
    /// `Modifiers::control()`，而 `display_modifiers` 为 `Modifiers::control_shift()`。
    pub fn modifiers(&self) -> &Modifiers {
        #[cfg(target_os = "windows")]
        {
            &self.display_modifiers
        }
        #[cfg(not(target_os = "windows"))]
        {
            &self.inner.modifiers
        }
    }

    /// 返回键。
    ///
    /// 平台特定行为：
    /// - 在 macOS 和 Linux 上，此键与 `inner.key` 相同，即按键的 GPUI 表示。
    /// - 在 Windows 上，此键是显示键，例如 `ctrl-@` 按键的 `inner.key` 为 `@`，而 `display_key` 为 `2`。
    pub fn key(&self) -> &str {
        #[cfg(target_os = "windows")]
        {
            &self.display_key
        }
        #[cfg(not(target_os = "windows"))]
        {
            &self.inner.key
        }
    }

    /// 设置修饰键。在 Windows 上，这会同时修改 `inner.modifiers` 和 `display_modifiers`。
    pub fn set_modifiers(&mut self, modifiers: Modifiers) {
        self.inner.modifiers = modifiers;
        #[cfg(target_os = "windows")]
        {
            self.display_modifiers = modifiers;
        }
    }

    /// 设置键。在 Windows 上，这会同时修改 `inner.key` 和 `display_key`。
    pub fn set_key(&mut self, key: String) {
        #[cfg(target_os = "windows")]
        {
            self.display_key = key.clone();
        }
        self.inner.key = key;
    }

    /// 生成此键的可被 Parse 理解的表示。
    pub fn unparse(&self) -> String {
        #[cfg(target_os = "windows")]
        {
            unparse(&self.display_modifiers, &self.display_key)
        }
        #[cfg(not(target_os = "windows"))]
        {
            unparse(&self.inner.modifiers, &self.inner.key)
        }
    }

    /// 移除 key_char
    pub fn remove_key_char(&mut self) {
        self.inner.key_char = None;
    }
}

/// 判断是否为可打印键
fn is_printable_key(key: &str) -> bool {
    !matches!(
        key,
        "f1" | "f2"
            | "f3"
            | "f4"
            | "f5"
            | "f6"
            | "f7"
            | "f8"
            | "f9"
            | "f10"
            | "f11"
            | "f12"
            | "f13"
            | "f14"
            | "f15"
            | "f16"
            | "f17"
            | "f18"
            | "f19"
            | "f20"
            | "f21"
            | "f22"
            | "f23"
            | "f24"
            | "f25"
            | "f26"
            | "f27"
            | "f28"
            | "f29"
            | "f30"
            | "f31"
            | "f32"
            | "f33"
            | "f34"
            | "f35"
            | "backspace"
            | "delete"
            | "left"
            | "right"
            | "up"
            | "down"
            | "pageup"
            | "pagedown"
            | "insert"
            | "home"
            | "end"
            | "back"
            | "forward"
            | "escape"
    )
}

impl std::fmt::Display for Keystroke {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display_modifiers(&self.modifiers, f)?;
        display_key(&self.key, f)
    }
}

impl std::fmt::Display for KeybindingKeystroke {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display_modifiers(self.modifiers(), f)?;
        display_key(self.key(), f)
    }
}

/// 某个时间点的修饰键状态
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize, Hash, JsonSchema)]
pub struct Modifiers {
    /// control 键
    #[serde(default)]
    pub control: bool,

    /// alt 键
    /// 有时也称为 "meta" 键
    #[serde(default)]
    pub alt: bool,

    /// shift 键
    #[serde(default)]
    pub shift: bool,

    /// command 键（在 macOS 上）
    /// windows 键（在 Windows 上）
    /// super 键（在 Linux 上）
    #[serde(default)]
    pub platform: bool,

    /// function 键
    #[serde(default)]
    pub function: bool,
}

impl Modifiers {
    /// 返回是否有任何修饰键被按下。
    pub fn modified(&self) -> bool {
        self.control || self.alt || self.shift || self.platform || self.function
    }

    /// 语义上的"次要"修饰键是否被按下。
    ///
    /// 在 macOS 上，这是 command 键。
    /// 在 Linux 和 Windows 上，这是 control 键。
    pub fn secondary(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.platform
        }

        #[cfg(not(target_os = "macos"))]
        {
            self.control
        }
    }

    /// 返回有多少个修饰键被按下。
    pub fn number_of_modifiers(&self) -> u8 {
        self.control as u8
            + self.alt as u8
            + self.shift as u8
            + self.platform as u8
            + self.function as u8
    }

    /// 返回没有修饰键的 [`Modifiers`]。
    pub fn none() -> Modifiers {
        Default::default()
    }

    /// 返回只有 command 键的 [`Modifiers`]。
    pub fn command() -> Modifiers {
        Modifiers {
            platform: true,
            ..Default::default()
        }
    }

    /// 返回只有次要键被按下的 [`Modifiers`]。
    pub fn secondary_key() -> Modifiers {
        #[cfg(target_os = "macos")]
        {
            Modifiers {
                platform: true,
                ..Default::default()
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            Modifiers {
                control: true,
                ..Default::default()
            }
        }
    }

    /// 返回只有 windows 键的 [`Modifiers`]。
    pub fn windows() -> Modifiers {
        Modifiers {
            platform: true,
            ..Default::default()
        }
    }

    /// 返回只有 super 键的 [`Modifiers`]。
    pub fn super_key() -> Modifiers {
        Modifiers {
            platform: true,
            ..Default::default()
        }
    }

    /// 返回只有 control 键的 [`Modifiers`]。
    pub fn control() -> Modifiers {
        Modifiers {
            control: true,
            ..Default::default()
        }
    }

    /// 返回只有 alt 键的 [`Modifiers`]。
    pub fn alt() -> Modifiers {
        Modifiers {
            alt: true,
            ..Default::default()
        }
    }

    /// 返回只有 shift 键的 [`Modifiers`]。
    pub fn shift() -> Modifiers {
        Modifiers {
            shift: true,
            ..Default::default()
        }
    }

    /// 返回只有 function 键的 [`Modifiers`]。
    pub fn function() -> Modifiers {
        Modifiers {
            function: true,
            ..Default::default()
        }
    }

    /// 返回 command + shift 的 [`Modifiers`]。
    pub fn command_shift() -> Modifiers {
        Modifiers {
            shift: true,
            platform: true,
            ..Default::default()
        }
    }

    /// 返回 control + shift 的 [`Modifiers`]。
    pub fn control_shift() -> Modifiers {
        Modifiers {
            shift: true,
            control: true,
            ..Default::default()
        }
    }

    /// 检查此 [`Modifiers`] 是否是另一个 [`Modifiers`] 的子集。
    pub fn is_subset_of(&self, other: &Modifiers) -> bool {
        (*other & *self) == *self
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;

    fn bitor(mut self, other: Self) -> Self::Output {
        self |= other;
        self
    }
}

impl std::ops::BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, other: Self) {
        self.control |= other.control;
        self.alt |= other.alt;
        self.shift |= other.shift;
        self.platform |= other.platform;
        self.function |= other.function;
    }
}

impl std::ops::BitXor for Modifiers {
    type Output = Self;
    fn bitxor(mut self, rhs: Self) -> Self::Output {
        self ^= rhs;
        self
    }
}

impl std::ops::BitXorAssign for Modifiers {
    fn bitxor_assign(&mut self, other: Self) {
        self.control ^= other.control;
        self.alt ^= other.alt;
        self.shift ^= other.shift;
        self.platform ^= other.platform;
        self.function ^= other.function;
    }
}

impl std::ops::BitAnd for Modifiers {
    type Output = Self;
    fn bitand(mut self, rhs: Self) -> Self::Output {
        self &= rhs;
        self
    }
}

impl std::ops::BitAndAssign for Modifiers {
    fn bitand_assign(&mut self, other: Self) {
        self.control &= other.control;
        self.alt &= other.alt;
        self.shift &= other.shift;
        self.platform &= other.platform;
        self.function &= other.function;
    }
}

/// 某个时间点的 capslock 键状态
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize, Hash, JsonSchema)]
pub struct Capslock {
    /// capslock 键已开启
    #[serde(default)]
    pub on: bool,
}

impl AsKeystroke for Keystroke {
    fn as_keystroke(&self) -> &Keystroke {
        self
    }
}

impl AsKeystroke for KeybindingKeystroke {
    fn as_keystroke(&self) -> &Keystroke {
        &self.inner
    }
}

fn display_modifiers(modifiers: &Modifiers, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if modifiers.control {
        #[cfg(target_os = "macos")]
        f.write_char('^')?;

        #[cfg(not(target_os = "macos"))]
        write!(f, "ctrl-")?;
    }
    if modifiers.alt {
        #[cfg(target_os = "macos")]
        f.write_char('⌥')?;

        #[cfg(not(target_os = "macos"))]
        write!(f, "alt-")?;
    }
    if modifiers.platform {
        #[cfg(target_os = "macos")]
        f.write_char('⌘')?;

        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        f.write_char('❖')?;

        #[cfg(target_os = "windows")]
        f.write_char('⊞')?;
    }
    if modifiers.shift {
        #[cfg(target_os = "macos")]
        f.write_char('⇧')?;

        #[cfg(not(target_os = "macos"))]
        write!(f, "shift-")?;
    }
    Ok(())
}

fn display_key(key: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let key = match key {
        #[cfg(target_os = "macos")]
        "backspace" => '⌫',
        #[cfg(target_os = "macos")]
        "up" => '↑',
        #[cfg(target_os = "macos")]
        "down" => '↓',
        #[cfg(target_os = "macos")]
        "left" => '←',
        #[cfg(target_os = "macos")]
        "right" => '→',
        #[cfg(target_os = "macos")]
        "tab" => '⇥',
        #[cfg(target_os = "macos")]
        "escape" => '⎋',
        #[cfg(target_os = "macos")]
        "shift" => '⇧',
        #[cfg(target_os = "macos")]
        "control" => '⌃',
        #[cfg(target_os = "macos")]
        "alt" => '⌥',
        #[cfg(target_os = "macos")]
        "platform" => '⌘',

        key if key.len() == 1 => key.chars().next().unwrap().to_ascii_uppercase(),
        key => return f.write_str(key),
    };
    f.write_char(key)
}

#[inline]
fn unparse(modifiers: &Modifiers, key: &str) -> String {
    let mut result = String::new();
    if modifiers.function {
        result.push_str("fn-");
    }
    if modifiers.control {
        result.push_str("ctrl-");
    }
    if modifiers.alt {
        result.push_str("alt-");
    }
    if modifiers.platform {
        #[cfg(target_os = "macos")]
        result.push_str("cmd-");

        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        result.push_str("super-");

        #[cfg(target_os = "windows")]
        result.push_str("win-");
    }
    if modifiers.shift {
        result.push_str("shift-");
    }
    result.push_str(&key);
    result
}
