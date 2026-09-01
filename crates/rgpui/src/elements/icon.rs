//! 图标组件，支持命名图标、SVG 渲染和旋转等变换效果。

use crate::icon_named;
use crate::prelude::FluentBuilder as _;
use crate::{
    ActiveTheme, AnyElement, App, AppContext, Context, ElementSize, Entity, Hsla, IntoElement,
    Radians, Render, RenderOnce, SharedString, StyleRefinement, Styled, Svg, Transformation,
    Window, svg,
};

/// 实现该 trait 的类型可以自动转换为 [`Icon`]。
///
/// 这允许你实现自定义的 [`IconName`] 版本，作为其他 UI 组件的即插即用替代品。
pub trait IconNamed {
    /// 返回图标的资源路径。
    fn path(&self) -> SharedString;

    /// 返回图标的编译期嵌入字节数据（SVG 原始内容）。
    fn bytes(&self) -> &'static [u8];
}

impl<T: IconNamed> From<T> for Icon {
    fn from(value: T) -> Self {
        Icon::build(value)
    }
}

// 从 `rgpui` 随附的图标资源生成 `IconName` 枚举。
icon_named!(IconName, "assets/icons");

impl IconName {
    /// 将图标作为 `Entity<Icon>` 返回。
    pub fn view(self, cx: &mut App) -> Entity<Icon> {
        Icon::build(self).view(cx)
    }
}

impl From<IconName> for AnyElement {
    fn from(val: IconName) -> Self {
        Icon::build(val).into_any_element()
    }
}

impl RenderOnce for IconName {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        Icon::build(self)
    }
}

/// 图标元素，用于渲染 SVG 矢量图标。
#[derive(IntoElement)]
pub struct Icon {
    /// 基础 SVG 元素
    base: Svg,
    /// 样式精炼
    style: StyleRefinement,
    /// 图标路径
    path: SharedString,
    /// 图标字节数据（编译期嵌入）
    data: Option<&'static [u8]>,
    /// 文字颜色
    text_color: Option<Hsla>,
    /// 尺寸
    size: Option<ElementSize>,
    /// 旋转角度
    rotation: Option<Radians>,
}

impl Default for Icon {
    fn default() -> Self {
        Self {
            base: svg().flex_none().size_4(),
            style: StyleRefinement::default(),
            path: "".into(),
            data: None,
            text_color: None,
            size: None,
            rotation: None,
        }
    }
}

impl Clone for Icon {
    fn clone(&self) -> Self {
        let mut this = Self::default().path(self.path.clone());
        this.style = self.style.clone();
        this.rotation = self.rotation;
        this.size = self.size;
        this.text_color = self.text_color;
        this.data = self.data;
        this
    }
}

impl Icon {
    /// 创建一个新的图标。
    pub fn new(icon: impl Into<Icon>) -> Self {
        icon.into()
    }

    fn build(name: impl IconNamed) -> Self {
        let path = name.path();
        let bytes = name.bytes();
        Self::default().path(path).data(bytes)
    }

    /// 设置图标的资源路径。
    ///
    /// 例如：`icons/foo.svg`
    pub fn path(mut self, path: impl Into<SharedString>) -> Self {
        self.path = path.into();
        self
    }

    /// 设置图标的编译期嵌入字节数据。
    pub fn data(mut self, data: &'static [u8]) -> Self {
        self.data = Some(data);
        self
    }

    /// 创建图标的新视图。
    pub fn view(self, cx: &mut App) -> Entity<Icon> {
        cx.new(|_| self)
    }

    /// 应用变换。
    pub fn transform(mut self, transformation: Transformation) -> Self {
        self.base = self.base.with_transformation(transformation);
        self
    }

    /// 创建一个空图标。
    pub fn empty() -> Self {
        Self::default()
    }

    /// 按给定角度旋转图标。
    pub fn rotate(mut self, radians: impl Into<Radians>) -> Self {
        self.base = self
            .base
            .with_transformation(Transformation::rotate(radians));
        self
    }
}

impl Styled for Icon {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }

    fn text_color(mut self, color: impl Into<Hsla>) -> Self {
        self.text_color = Some(color.into());
        self
    }
}

impl crate::Sizable for Icon {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = Some(size.into());
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let text_color = self.text_color.unwrap_or_else(|| window.text_style().color);
        let text_size = window.text_style().font_size.to_pixels(window.rem_size());
        let has_base_size = self.style.size.width.is_some() || self.style.size.height.is_some();

        let mut base = self.base;
        *base.style() = self.style;

        base.flex_shrink_0()
            .text_color(text_color)
            .when(!has_base_size, |this| this.size(text_size))
            .when_some(self.size, |this, size| match size {
                ElementSize::ElementSize(px) => this.size(px),
                ElementSize::XSmall => this.size_3(),
                ElementSize::Small => this.size_3p5(),
                ElementSize::Medium => this.size_4(),
                ElementSize::Large => this.size_6(),
            })
            .path(self.path)
            .when_some(self.data, |this, data| this.data(data))
    }
}

impl From<Icon> for AnyElement {
    fn from(val: Icon) -> Self {
        val.into_any_element()
    }
}

impl Render for Icon {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text_color = self.text_color.unwrap_or_else(|| cx.theme().foreground);
        let text_size = window.text_style().font_size.to_pixels(window.rem_size());
        let has_base_size = self.style.size.width.is_some() || self.style.size.height.is_some();

        let mut base = svg().flex_none();
        *base.style() = self.style.clone();

        base.flex_shrink_0()
            .text_color(text_color)
            .when(!has_base_size, |this| this.size(text_size))
            .when_some(self.size, |this, size| match size {
                ElementSize::ElementSize(px) => this.size(px),
                ElementSize::XSmall => this.size_3(),
                ElementSize::Small => this.size_3p5(),
                ElementSize::Medium => this.size_4(),
                ElementSize::Large => this.size_6(),
            })
            .path(self.path.clone())
            .when_some(self.data, |this, data| this.data(data))
            .when_some(self.rotation, |this, rotation| {
                this.with_transformation(Transformation::rotate(rotation))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Sizable;

    #[test]
    fn test_icon_name_generated() {
        // 验证宏生成的枚举变体与路径
        let icon = IconName::Check;
        assert_eq!(icon.path(), "icons/check.svg");
        assert!(!icon.bytes().is_empty());
    }

    #[test]
    fn test_icon_build() {
        let icon = Icon::new(IconName::Check);
        assert_eq!(icon.path, "icons/check.svg");
        assert!(icon.data.is_some());
    }

    #[test]
    fn test_icon_style() {
        let icon = Icon::new(IconName::Check).small();
        assert_eq!(icon.size, Some(ElementSize::Small));
    }
}
