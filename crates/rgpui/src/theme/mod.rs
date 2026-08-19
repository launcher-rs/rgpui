use crate::{App, Global, Hsla, Pixels, SharedString, Window, WindowAppearance, px};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    ops::{Deref, DerefMut},
    rc::Rc,
    sync::Arc,
};

/// 颜色定义模块。
pub mod color;
/// 代码高亮主题模块。
pub mod highlight;
/// 主题注册表模块。
pub mod registry;
/// 主题 JSON 模式定义模块。
pub mod schema;
/// 主题设置模块。
pub mod settings;
mod theme_color;

pub use color::*;
pub use highlight::*;
pub use registry::*;
pub use schema::*;
pub use settings::*;
pub use theme_color::*;

/// 初始化主题系统，注册默认主题并加载主题模式。
pub fn init(cx: &mut App) {
    registry::init(cx);

    // 启动时直接加载主题，兼容 WASM
    Theme::change(ThemeMode::Light, None, cx);
    Theme::sync_scrollbar_appearance(cx);
}

/// 提供访问当前主题的 trait。
pub trait ActiveTheme {
    /// 返回当前全局主题。
    fn theme(&self) -> &Theme;
}

impl ActiveTheme for App {
    #[inline(always)]
    fn theme(&self) -> &Theme {
        Theme::global(self)
    }
}

/// 全局主题配置。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Theme {
    /// 主题颜色。
    pub colors: ThemeColor,
    #[serde(default)]
    /// 主题令牌（颜色 + 背景）。
    pub tokens: ThemeTokens,
    /// 代码高亮主题。
    pub highlight_theme: Arc<HighlightTheme>,
    /// 亮色主题配置。
    pub light_theme: Rc<ThemeConfig>,
    /// 暗色主题配置。
    pub dark_theme: Rc<ThemeConfig>,

    /// 主题模式。
    pub mode: ThemeMode,
    /// 应用字体族，默认 `.SystemUIFont`。
    pub font_family: SharedString,
    /// 基础字号，默认 16px。
    pub font_size: Pixels,
    /// 等宽字体族。
    ///
    /// 默认值：
    ///
    /// - macOS: `Menlo`
    /// - Windows: `Consolas`
    /// - Linux: `DejaVu Sans Mono`
    pub mono_font_family: SharedString,
    /// 等宽字号，默认 13px。
    pub mono_font_size: Pixels,
    /// 常规元素圆角。
    pub radius: Pixels,
    /// 大元素圆角（如 Dialog、Notification）。
    pub radius_lg: Pixels,
    /// 是否启用阴影。
    pub shadow: bool,
    /// 透明色。
    pub transparent: Hsla,
    /// 滚动条显示模式，默认：Scrolling。
    pub scrollbar_show: ScrollbarShow,
    /// 通知设置。
    #[serde(skip)]
    pub notification: NotificationSettings,
    /// 磁贴网格尺寸，默认 4px。
    pub tile_grid_size: Pixels,
    /// 磁贴面板阴影。
    pub tile_shadow: bool,
    /// 磁贴面板圆角，默认 0px。
    pub tile_radius: Pixels,
    /// List 设置。
    pub list: crate::theme::settings::ListSettings,
    /// Sheet 设置。
    pub sheet: SheetSettings,
}

impl Default for Theme {
    fn default() -> Self {
        // 使用内置默认亮色主题的真实颜色作为回退，
        // 避免应用未显式调用 `theme::init` 时渲染为全透明（窗口显示空白）。
        let (colors, highlight_theme) = &DEFAULT_THEME_COLORS[&ThemeMode::Light];
        let mut theme = Self::from(&**colors);
        theme.highlight_theme = highlight_theme.clone();
        theme
    }
}

impl Deref for Theme {
    type Target = ThemeColor;

    fn deref(&self) -> &Self::Target {
        &self.colors
    }
}

impl DerefMut for Theme {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.colors
    }
}

impl Global for Theme {}

impl Theme {
    /// 返回全局主题引用。
    #[inline(always)]
    pub fn global(cx: &App) -> &Theme {
        cx.global::<Theme>()
    }

    /// 返回全局主题可变引用。
    #[inline(always)]
    pub fn global_mut(cx: &mut App) -> &mut Theme {
        cx.global_mut::<Theme>()
    }

    /// 返回当前是否为暗色主题。
    #[inline(always)]
    pub fn is_dark(&self) -> bool {
        self.mode.is_dark()
    }

    /// 返回当前主题名称。
    pub fn theme_name(&self) -> &SharedString {
        if self.is_dark() {
            &self.dark_theme.name
        } else {
            &self.light_theme.name
        }
    }

    /// 与系统外观同步主题。
    pub fn sync_system_appearance(window: Option<&mut Window>, cx: &mut App) {
        // 优先使用 window.appearance() 以避免 Linux 上报错。
        // https://github.com/longbridge/rgpui-component/issues/104
        let appearance = window
            .as_ref()
            .map(|window| window.appearance())
            .unwrap_or_else(|| cx.window_appearance());

        Self::change(appearance, window, cx);
    }

    /// 与系统同步滚动条显示行为。
    pub fn sync_scrollbar_appearance(cx: &mut App) {
        Theme::global_mut(cx).scrollbar_show = if cx.should_auto_hide_scrollbars() {
            ScrollbarShow::Scrolling
        } else {
            ScrollbarShow::Hover
        };
    }

    /// 切换主题模式。
    pub fn change(mode: impl Into<ThemeMode>, window: Option<&mut Window>, cx: &mut App) {
        let mode = mode.into();
        if !cx.has_global::<Theme>() {
            let mut theme = Theme::default();
            theme.light_theme = ThemeRegistry::global(cx).default_light_theme().clone();
            theme.dark_theme = ThemeRegistry::global(cx).default_dark_theme().clone();
            cx.set_global(theme);
        }

        let theme = cx.global_mut::<Theme>();
        theme.mode = mode;
        if mode.is_dark() {
            theme.apply_config(&theme.dark_theme.clone());
        } else {
            theme.apply_config(&theme.light_theme.clone());
        }

        if let Some(window) = window {
            window.refresh();
        }
    }

    /// 获取输入框背景色。
    ///
    /// 暗色模式下使用透明色与输入框边框混合：`cx.theme().input`，
    /// 亮色模式下使用 `cx.theme().background` 颜色。
    #[inline]
    pub fn input_background(&self) -> Hsla {
        if self.is_dark() {
            self.input.mix_oklab(self.transparent, 0.3)
        } else {
            self.background
        }
    }

    /// 获取编辑器背景色，未设置时使用输入框背景色。
    #[inline]
    pub(crate) fn editor_background(&self) -> Hsla {
        self.highlight_theme
            .style
            .editor_background
            .unwrap_or_else(|| self.input_background())
    }
}

impl From<&ThemeColor> for Theme {
    fn from(colors: &ThemeColor) -> Self {
        Theme {
            mode: ThemeMode::default(),
            transparent: Hsla::transparent_black(),
            font_family: ".SystemUIFont".into(),
            font_size: px(16.),
            mono_font_family: if cfg!(target_os = "macos") {
                // https://en.wikipedia.org/wiki/Menlo_(typeface)
                "Menlo".into()
            } else if cfg!(target_os = "windows") {
                "Consolas".into()
            } else {
                "DejaVu Sans Mono".into()
            },
            mono_font_size: px(13.),
            radius: px(6.),
            radius_lg: px(8.),
            shadow: true,
            scrollbar_show: ScrollbarShow::default(),
            notification: NotificationSettings::default(),
            tile_grid_size: px(8.),
            tile_shadow: true,
            tile_radius: px(0.),
            list: crate::theme::settings::ListSettings::default(),
            colors: *colors,
            tokens: ThemeTokens::from(colors),
            light_theme: Rc::new(ThemeConfig::default()),
            dark_theme: Rc::new(ThemeConfig::default()),
            highlight_theme: HighlightTheme::default_light(),
            sheet: SheetSettings::default(),
        }
    }
}

/// 主题模式枚举。
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    PartialOrd,
    Eq,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    /// 亮色模式。
    Light,
    /// 暗色模式。
    Dark,
}

impl ThemeMode {
    /// 是否为暗色模式。
    #[inline(always)]
    pub fn is_dark(&self) -> bool {
        matches!(self, Self::Dark)
    }

    /// 返回小写主题名称：`light`、`dark`。
    pub fn name(&self) -> &'static str {
        match self {
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        }
    }
}

impl From<WindowAppearance> for ThemeMode {
    fn from(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::Dark,
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::Light,
        }
    }
}
