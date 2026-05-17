/// 返回 `true`，用作 serde 默认值。
pub const fn default_true() -> bool {
    true
}

/// 判断给定值是否等于其类型的默认值。
pub fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}
