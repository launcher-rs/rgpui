use crate::{App, Global, Rgba, Window, WindowAppearance, rgb};
use std::ops::Deref;
use std::sync::Arc;

/// GPUI 的默认颜色集。
///
/// 这些用于样式化基础组件、示例等。
#[derive(Clone, Debug)]
pub struct Colors {
    /// 文本颜色
    pub text: Rgba,
    /// 选中文本颜色
    pub selected_text: Rgba,
    /// 背景颜色
    pub background: Rgba,
    /// 禁用状态颜色
    pub disabled: Rgba,
    /// 选中状态颜色
    pub selected: Rgba,
    /// 边框颜色
    pub border: Rgba,
    /// 分隔线颜色
    pub separator: Rgba,
    /// 容器颜色
    pub container: Rgba,
}

impl Default for Colors {
    fn default() -> Self {
        Self::light()
    }
}

impl Colors {
    /// 返回给定窗口外观的默认颜色。
    pub fn for_appearance(window: &Window) -> Self {
        match window.appearance() {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::light(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::dark(),
        }
    }

    /// 返回默认的深色颜色集。
    pub fn dark() -> Self {
        Self {
            text: rgb(0xffffff),
            selected_text: rgb(0xffffff),
            disabled: rgb(0x565656),
            selected: rgb(0x2457ca),
            background: rgb(0x222222),
            border: rgb(0x000000),
            separator: rgb(0xd9d9d9),
            container: rgb(0x262626),
        }
    }

    /// 返回默认的浅色颜色集。
    pub fn light() -> Self {
        Self {
            text: rgb(0x252525),
            selected_text: rgb(0xffffff),
            background: rgb(0xffffff),
            disabled: rgb(0xb0b0b0),
            selected: rgb(0x2a63d9),
            border: rgb(0xd9d9d9),
            separator: rgb(0xe6e6e6),
            container: rgb(0xf4f5f5),
        }
    }

    /// 从全局状态获取 [Colors]
    pub fn get_global(cx: &App) -> &Arc<Colors> {
        &cx.global::<GlobalColors>().0
    }
}

/// 从全局状态获取 [Colors]
#[derive(Clone, Debug)]
pub struct GlobalColors(pub Arc<Colors>);

impl Deref for GlobalColors {
    type Target = Arc<Colors>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Global for GlobalColors {}

/// 实现此 trait 以允许通过 `cx.default_colors()` 全局访问 [Colors]。
pub trait DefaultColors {
    /// Returns the default [`Colors`]
    fn default_colors(&self) -> &Arc<Colors>;
}

impl DefaultColors for App {
    fn default_colors(&self) -> &Arc<Colors> {
        &self.global::<GlobalColors>().0
    }
}

/// 用于样式化 GPUI 元素的基础 GPUI 颜色外观
///
/// 根据系统当前的 [`WindowAppearance`] 而变化。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DefaultAppearance {
    /// 对浅色外观使用一组颜色。
    #[default]
    Light,
    /// 对深色外观使用一组颜色。
    Dark,
}

impl From<WindowAppearance> for DefaultAppearance {
    fn from(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::Light,
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::Dark,
        }
    }
}
