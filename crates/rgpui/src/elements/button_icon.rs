//! 按钮图标元素，为按钮提供图标或加载动画的内部组件。

use crate::{
    App, ElementSize, Icon, IntoElement, RenderOnce, Sizable, Spinner, Window,
    prelude::FluentBuilder,
};

/// 按钮图标，可以是 Icon 或 Spinner，用于 Button 的 `icon` 方法。
#[doc(hidden)]
#[derive(IntoElement)]
pub struct ButtonIcon {
    icon: ButtonIconVariant,
    loading_icon: Option<Icon>,
    loading: bool,
    size: ElementSize,
}

impl<T> From<T> for ButtonIcon
where
    T: Into<ButtonIconVariant>,
{
    fn from(icon: T) -> Self {
        ButtonIcon::new(icon)
    }
}

impl ButtonIcon {
    /// 使用给定的图标创建新的 ButtonIcon。
    pub fn new(icon: impl Into<ButtonIconVariant>) -> Self {
        Self {
            icon: icon.into(),
            loading_icon: None,
            loading: false,
            size: ElementSize::Medium,
        }
    }

    /// 设置加载图标。
    pub(crate) fn loading_icon(mut self, icon: Option<Icon>) -> Self {
        self.loading_icon = icon;
        self
    }

    /// 设置是否显示加载状态。
    pub(crate) fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }
}

impl Sizable for ButtonIcon {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

/// 按钮图标变体，可以是 Icon 或 Spinner，用于 Button 的 `icon` 方法。
#[doc(hidden)]
#[derive(IntoElement)]
pub enum ButtonIconVariant {
    Icon(Icon),
    Spinner(Spinner),
}

impl<T> From<T> for ButtonIconVariant
where
    T: Into<Icon>,
{
    fn from(icon: T) -> Self {
        Self::Icon(icon.into())
    }
}

impl From<Spinner> for ButtonIconVariant {
    fn from(spinner: Spinner) -> Self {
        Self::Spinner(spinner)
    }
}

impl ButtonIconVariant {
    /// 是否为 Spinner 类型。
    #[inline]
    pub(crate) fn is_spinner(&self) -> bool {
        matches!(self, Self::Spinner(_))
    }
}

impl Sizable for ButtonIconVariant {
    fn with_size(self, size: impl Into<ElementSize>) -> Self {
        match self {
            Self::Icon(icon) => Self::Icon(icon.with_size(size)),
            Self::Spinner(spinner) => Self::Spinner(spinner.with_size(size)),
        }
    }
}

impl RenderOnce for ButtonIconVariant {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        match self {
            Self::Icon(icon) => icon.into_any_element(),
            Self::Spinner(spinner) => spinner.into_any_element(),
        }
    }
}

impl RenderOnce for ButtonIcon {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        if self.loading {
            if self.icon.is_spinner() {
                self.icon.with_size(self.size).into_any_element()
            } else {
                Spinner::new()
                    .when_some(self.loading_icon, |this, icon| this.icon(icon))
                    .with_size(self.size)
                    .into_any_element()
            }
        } else {
            self.icon.with_size(self.size).into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IconName;

    /// 测试 ButtonIcon 构造
    #[test]
    fn test_button_icon_builder() {
        let custom_icon = Icon::new(IconName::Loader);
        let icon = ButtonIcon::new(IconName::Plus)
            .loading(true)
            .loading_icon(Some(custom_icon))
            .large();

        assert!(icon.loading);
        assert!(icon.loading_icon.is_some());
        assert_eq!(icon.size, ElementSize::Large);
    }

    /// 测试 ButtonIconVariant 类型
    #[test]
    fn test_button_icon_variant_types() {
        let icon_variant = ButtonIconVariant::Icon(Icon::new(IconName::Plus));
        assert!(!icon_variant.is_spinner());

        let spinner_variant = ButtonIconVariant::Spinner(Spinner::new());
        assert!(spinner_variant.is_spinner());
    }
}
