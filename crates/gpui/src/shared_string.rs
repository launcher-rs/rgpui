//! GPUI 共享字符串类型。
//!
//! 提供 `SharedString` 类型，这是一个不可变的字符串，可以在 GPUI 任务中廉价地克隆。
//! 本质上是 `Arc<str>` 和 `&'static str` 的抽象，当前由 `SmolStr` 支持。

use std::{
    borrow::{Borrow, Cow},
    sync::Arc,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// 共享字符串是 GPUI 任务中可以廉价克隆的不可变字符串。
/// 本质上是 `Arc<str>` 和 `&'static str` 的抽象，
/// 当前由 [`SmolStr`] 支持。
#[derive(Eq, PartialEq, PartialOrd, Ord, Hash, Clone)]
pub struct SharedString(SmolStr);

impl std::ops::Deref for SharedString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl SharedString {
    /// 从 `&'static str` 创建静态 [`SharedString`]。
    pub const fn new_static(str: &'static str) -> Self {
        Self(SmolStr::new_static(str))
    }

    /// 创建 [`SharedString`]。
    pub fn new(str: impl AsRef<str>) -> Self {
        SharedString(SmolStr::new(str))
    }

    /// 从底层字符串获取 &str。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl JsonSchema for SharedString {
    fn inline_schema() -> bool {
        String::inline_schema()
    }

    fn schema_name() -> Cow<'static, str> {
        String::schema_name()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        String::json_schema(generator)
    }
}

impl Default for SharedString {
    fn default() -> Self {
        Self::new_static("")
    }
}

impl AsRef<str> for SharedString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for SharedString {
    fn borrow(&self) -> &str {
        self.as_ref()
    }
}

impl std::fmt::Debug for SharedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::fmt::Display for SharedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.as_str())
    }
}

impl PartialEq<String> for SharedString {
    fn eq(&self, other: &String) -> bool {
        self.as_ref() == other
    }
}

impl PartialEq<SharedString> for String {
    fn eq(&self, other: &SharedString) -> bool {
        self == other.as_ref()
    }
}

impl PartialEq<str> for SharedString {
    fn eq(&self, other: &str) -> bool {
        self.as_ref() == other
    }
}

impl<'a> PartialEq<&'a str> for SharedString {
    fn eq(&self, other: &&'a str) -> bool {
        self.as_ref() == *other
    }
}

impl From<&SharedString> for SharedString {
    #[inline]
    fn from(s: &SharedString) -> SharedString {
        s.clone()
    }
}

impl From<&str> for SharedString {
    #[inline]
    fn from(s: &str) -> SharedString {
        SharedString(SmolStr::from(s))
    }
}

impl From<&mut str> for SharedString {
    #[inline]
    fn from(s: &mut str) -> SharedString {
        SharedString(SmolStr::from(s))
    }
}

impl From<&String> for SharedString {
    #[inline]
    fn from(s: &String) -> SharedString {
        SharedString(SmolStr::from(s))
    }
}

impl From<String> for SharedString {
    #[inline(always)]
    fn from(text: String) -> Self {
        SharedString(SmolStr::from(text))
    }
}

impl From<Box<str>> for SharedString {
    #[inline]
    fn from(s: Box<str>) -> SharedString {
        SharedString(SmolStr::from(s))
    }
}

impl From<Arc<str>> for SharedString {
    #[inline]
    fn from(s: Arc<str>) -> SharedString {
        SharedString(SmolStr::from(s))
    }
}

impl From<&Arc<str>> for SharedString {
    #[inline]
    fn from(s: &Arc<str>) -> SharedString {
        SharedString(SmolStr::from(s.clone()))
    }
}

impl<'a> From<Cow<'a, str>> for SharedString {
    #[inline]
    fn from(s: Cow<'a, str>) -> SharedString {
        SharedString(SmolStr::from(s))
    }
}

impl From<SharedString> for Arc<str> {
    #[inline(always)]
    fn from(text: SharedString) -> Self {
        text.0.into()
    }
}

impl From<SharedString> for String {
    #[inline(always)]
    fn from(text: SharedString) -> Self {
        text.0.into()
    }
}

impl Serialize for SharedString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_ref())
    }
}

impl<'de> Deserialize<'de> for SharedString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(SharedString::new(&s))
    }
}
