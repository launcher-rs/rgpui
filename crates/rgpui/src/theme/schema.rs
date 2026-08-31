use std::{rc::Rc, sync::Arc};

use crate::{Background, Hsla, SharedString, px};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::highlight::{HighlightTheme, HighlightThemeStyle};

use super::color::{
    try_parse_background, try_parse_background_clamped, try_parse_color, try_parse_theme_color,
};
use super::{Colorize, Theme, ThemeColor, ThemeMode, ThemeToken, ThemeTokens};

fn try_parse_theme_token(value: &str) -> anyhow::Result<ThemeToken> {
    Ok(ThemeToken::new(
        try_parse_theme_color(value)?,
        try_parse_background(value)?,
    ))
}

/// 表示主题配置。
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ThemeSet {
    /// 主题集的名称。
    pub name: SharedString,
    /// 主题的作者。
    pub author: Option<SharedString>,
    /// 主题的 URL。
    pub url: Option<SharedString>,
    /// 主题集的主题列表。
    #[serde(rename = "themes")]
    pub themes: Vec<ThemeConfig>,
}

/// 主题配置，定义字体、圆角、阴影、颜色与高亮等主题属性。
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ThemeConfig {
    /// 此主题是否为默认主题。
    pub is_default: bool,
    /// 主题的名称。
    pub name: SharedString,
    /// 主题的模式，默认为浅色。
    pub mode: ThemeMode,

    /// 基础字体大小，默认为 16。
    #[serde(rename = "font.size")]
    pub font_size: Option<f32>,
    /// 基础字体族，默认为系统字体：`.SystemUIFont`。
    #[serde(rename = "font.family")]
    pub font_family: Option<SharedString>,
    /// 等宽字体族，默认为平台特定：
    /// - macOS: `Menlo`
    /// - Windows: `Consolas`
    /// - Linux: `DejaVu Sans Mono`
    #[serde(rename = "mono_font.family")]
    pub mono_font_family: Option<SharedString>,
    /// 等宽字体大小，默认为 13。
    #[serde(rename = "mono_font.size")]
    pub mono_font_size: Option<f32>,

    /// 通用元素的边框半径，默认为 6。
    #[serde(rename = "radius")]
    pub radius: Option<usize>,
    /// 大型元素（如对话框和通知）的边框半径，默认为 8。
    #[serde(rename = "radius.lg")]
    pub radius_lg: Option<usize>,
    /// 在主题中设置阴影，例如输入框和按钮，默认为 true。
    #[serde(rename = "shadow")]
    pub shadow: Option<bool>,

    /// 主题的颜色。
    pub colors: ThemeConfigColors,
    /// 高亮主题，此部分与 Zed 主题中的 `style` 部分兼容。
    ///
    /// https://github.com/zed-industries/zed/blob/f50041779dcfd7a76c8aec293361c60c53f02d51/assets/themes/ayu/ayu.json#L9
    pub highlight: Option<HighlightThemeStyle>,
}

/// 主题颜色配置，覆盖 `ThemeColor` 中对应名称的颜色值。
#[derive(Debug, Default, Clone, JsonSchema, Serialize, Deserialize)]
pub struct ThemeConfigColors {
    /// 用于悬停背景色的强调色，如 MenuItem、ListItem 等上的悬停背景。
    #[serde(rename = "accent.background")]
    pub accent: Option<SharedString>,
    /// 用于强调文本颜色。
    #[serde(rename = "accent.foreground")]
    pub accent_foreground: Option<SharedString>,
    /// 手风琴背景色。
    #[serde(rename = "accordion.background")]
    pub accordion: Option<SharedString>,
    /// 默认背景色。
    #[serde(rename = "background")]
    pub background: Option<SharedString>,
    /// 默认边框颜色。
    #[serde(rename = "border")]
    pub border: Option<SharedString>,
    /// 默认按钮背景色。
    #[serde(rename = "button.background")]
    pub button: Option<SharedString>,
    /// 默认按钮激活背景色。
    #[serde(rename = "button.active.background")]
    pub button_active: Option<SharedString>,
    /// 默认按钮文本颜色。
    #[serde(rename = "button.foreground")]
    pub button_foreground: Option<SharedString>,
    /// 默认按钮悬停背景色。
    #[serde(rename = "button.hover.background")]
    pub button_hover: Option<SharedString>,
    /// 危险按钮背景色，回退到 `danger`。
    #[serde(rename = "button.danger.background")]
    pub button_danger: Option<SharedString>,
    /// 危险按钮激活背景色，回退到 `danger_active`。
    #[serde(rename = "button.danger.active.background")]
    pub button_danger_active: Option<SharedString>,
    /// 危险按钮文本颜色，回退到 `danger_foreground`。
    #[serde(rename = "button.danger.foreground")]
    pub button_danger_foreground: Option<SharedString>,
    /// 危险按钮悬停背景色，回退到 `danger_hover`。
    #[serde(rename = "button.danger.hover.background")]
    pub button_danger_hover: Option<SharedString>,
    /// 信息按钮背景色，回退到 `info`。
    #[serde(rename = "button.info.background")]
    pub button_info: Option<SharedString>,
    /// 信息按钮激活背景色，回退到 `info_active`。
    #[serde(rename = "button.info.active.background")]
    pub button_info_active: Option<SharedString>,
    /// 信息按钮文本颜色，回退到 `info_foreground`。
    #[serde(rename = "button.info.foreground")]
    pub button_info_foreground: Option<SharedString>,
    /// 信息按钮悬停背景色，回退到 `info_hover`。
    #[serde(rename = "button.info.hover.background")]
    pub button_info_hover: Option<SharedString>,
    /// 主要按钮背景色，回退到 `primary`。
    #[serde(rename = "button.primary.background")]
    pub button_primary: Option<SharedString>,
    /// 主要按钮激活背景色，回退到 `primary_active`。
    #[serde(rename = "button.primary.active.background")]
    pub button_primary_active: Option<SharedString>,
    /// 主要按钮文本颜色，回退到 `primary_foreground`。
    #[serde(rename = "button.primary.foreground")]
    pub button_primary_foreground: Option<SharedString>,
    /// 主要按钮悬停背景色，回退到 `primary_hover`。
    #[serde(rename = "button.primary.hover.background")]
    pub button_primary_hover: Option<SharedString>,
    /// 次要按钮背景色，回退到 `secondary`。
    #[serde(rename = "button.secondary.background")]
    pub button_secondary: Option<SharedString>,
    /// 次要按钮激活背景色，回退到 `secondary_active`。
    #[serde(rename = "button.secondary.active.background")]
    pub button_secondary_active: Option<SharedString>,
    /// 次要按钮文本颜色，回退到 `secondary_foreground`。
    #[serde(rename = "button.secondary.foreground")]
    pub button_secondary_foreground: Option<SharedString>,
    /// 次要按钮悬停背景色，回退到 `secondary_hover`。
    #[serde(rename = "button.secondary.hover.background")]
    pub button_secondary_hover: Option<SharedString>,
    /// 成功按钮背景色，回退到 `success`。
    #[serde(rename = "button.success.background")]
    pub button_success: Option<SharedString>,
    /// 成功按钮激活背景色，回退到 `success_active`。
    #[serde(rename = "button.success.active.background")]
    pub button_success_active: Option<SharedString>,
    /// 成功按钮文本颜色，回退到 `success_foreground`。
    #[serde(rename = "button.success.foreground")]
    pub button_success_foreground: Option<SharedString>,
    /// 成功按钮悬停背景色，回退到 `success_hover`。
    #[serde(rename = "button.success.hover.background")]
    pub button_success_hover: Option<SharedString>,
    /// 警告按钮背景色，回退到 `warning`。
    #[serde(rename = "button.warning.background")]
    pub button_warning: Option<SharedString>,
    /// 警告按钮激活背景色，回退到 `warning_active`。
    #[serde(rename = "button.warning.active.background")]
    pub button_warning_active: Option<SharedString>,
    /// 警告按钮文本颜色，回退到 `warning_foreground`。
    #[serde(rename = "button.warning.foreground")]
    pub button_warning_foreground: Option<SharedString>,
    /// 警告按钮悬停背景色，回退到 `warning_hover`。
    #[serde(rename = "button.warning.hover.background")]
    pub button_warning_hover: Option<SharedString>,
    /// 分组框背景色。
    #[serde(rename = "group_box.background")]
    pub group_box: Option<SharedString>,
    /// 分组框文本颜色。
    #[serde(rename = "group_box.foreground")]
    pub group_box_foreground: Option<SharedString>,
    /// 分组框标题文本颜色。
    #[serde(rename = "group_box.title.foreground")]
    pub group_box_title_foreground: Option<SharedString>,
    /// 输入框插入符颜色（闪烁光标）。
    #[serde(rename = "caret")]
    pub caret: Option<SharedString>,
    /// 图表 1 颜色。
    #[serde(rename = "chart.1")]
    pub chart_1: Option<SharedString>,
    /// 图表 2 颜色。
    #[serde(rename = "chart.2")]
    pub chart_2: Option<SharedString>,
    /// 图表 3 颜色。
    #[serde(rename = "chart.3")]
    pub chart_3: Option<SharedString>,
    /// 图表 4 颜色。
    #[serde(rename = "chart.4")]
    pub chart_4: Option<SharedString>,
    /// 图表 5 颜色。
    #[serde(rename = "chart.5")]
    pub chart_5: Option<SharedString>,
    /// K 线图看涨颜色（价格上涨）。
    #[serde(rename = "chart_bullish")]
    pub chart_bullish: Option<SharedString>,
    /// K 线图看跌颜色（价格下跌）。
    #[serde(rename = "chart_bearish")]
    pub chart_bearish: Option<SharedString>,
    /// 危险背景色。
    #[serde(rename = "danger.background")]
    pub danger: Option<SharedString>,
    /// 危险激活背景色。
    #[serde(rename = "danger.active.background")]
    pub danger_active: Option<SharedString>,
    /// 危险文本颜色。
    #[serde(rename = "danger.foreground")]
    pub danger_foreground: Option<SharedString>,
    /// 危险悬停背景色。
    #[serde(rename = "danger.hover.background")]
    pub danger_hover: Option<SharedString>,
    /// 描述列表标签背景色。
    #[serde(rename = "description_list.label.background")]
    pub description_list_label: Option<SharedString>,
    /// 描述列表标签前景色。
    #[serde(rename = "description_list.label.foreground")]
    pub description_list_label_foreground: Option<SharedString>,
    /// 拖拽边框颜色。
    #[serde(rename = "drag.border")]
    pub drag_border: Option<SharedString>,
    /// 放置目标背景色。
    #[serde(rename = "drop_target.background")]
    pub drop_target: Option<SharedString>,
    /// 默认文本颜色。
    #[serde(rename = "foreground")]
    pub foreground: Option<SharedString>,
    /// 信息背景色。
    #[serde(rename = "info.background")]
    pub info: Option<SharedString>,
    /// 信息激活背景色。
    #[serde(rename = "info.active.background")]
    pub info_active: Option<SharedString>,
    /// 信息文本颜色。
    #[serde(rename = "info.foreground")]
    pub info_foreground: Option<SharedString>,
    /// 信息悬停背景色。
    #[serde(rename = "info.hover.background")]
    pub info_hover: Option<SharedString>,
    /// 输入框（如 Input、Select 等）的边框颜色。
    #[serde(rename = "input.border")]
    pub input: Option<SharedString>,
    /// 链接文本颜色。
    #[serde(rename = "link")]
    pub link: Option<SharedString>,
    /// 激活链接文本颜色。
    #[serde(rename = "link.active")]
    pub link_active: Option<SharedString>,
    /// 悬停链接文本颜色。
    #[serde(rename = "link.hover")]
    pub link_hover: Option<SharedString>,
    /// 列表和列表项的背景色。
    #[serde(rename = "list.background")]
    pub list: Option<SharedString>,
    /// 激活列表项的背景色。
    #[serde(rename = "list.active.background")]
    pub list_active: Option<SharedString>,
    /// 激活列表项的边框颜色。
    #[serde(rename = "list.active.border")]
    pub list_active_border: Option<SharedString>,
    /// 偶数列表项的条纹背景色。
    #[serde(rename = "list.even.background")]
    pub list_even: Option<SharedString>,
    /// 列表头部的背景色。
    #[serde(rename = "list.head.background")]
    pub list_head: Option<SharedString>,
    /// 列表项的悬停背景色。
    #[serde(rename = "list.hover.background")]
    pub list_hover: Option<SharedString>,
    /// 柔和背景色，如骨架屏和开关。
    #[serde(rename = "muted.background")]
    pub muted: Option<SharedString>,
    /// 柔和文本颜色，用于禁用文本。
    #[serde(rename = "muted.foreground")]
    pub muted_foreground: Option<SharedString>,
    /// 弹出层背景色。
    #[serde(rename = "popover.background")]
    pub popover: Option<SharedString>,
    /// 弹出层文本颜色。
    #[serde(rename = "popover.foreground")]
    pub popover_foreground: Option<SharedString>,
    /// 主要背景色。
    #[serde(rename = "primary.background")]
    pub primary: Option<SharedString>,
    /// 激活主要背景色。
    #[serde(rename = "primary.active.background")]
    pub primary_active: Option<SharedString>,
    /// 主要文本颜色。
    #[serde(rename = "primary.foreground")]
    pub primary_foreground: Option<SharedString>,
    /// 悬停主要背景色。
    #[serde(rename = "primary.hover.background")]
    pub primary_hover: Option<SharedString>,
    /// 进度条背景色。
    #[serde(rename = "progress.bar.background")]
    pub progress_bar: Option<SharedString>,
    /// 用于焦点环。
    #[serde(rename = "ring")]
    pub ring: Option<SharedString>,
    /// 滚动条背景色。
    #[serde(rename = "scrollbar.background")]
    pub scrollbar: Option<SharedString>,
    /// 滚动条滑块背景色。
    #[serde(rename = "scrollbar.thumb.background")]
    pub scrollbar_thumb: Option<SharedString>,
    /// 滚动条滑块悬停背景色。
    #[serde(rename = "scrollbar.thumb.hover.background")]
    pub scrollbar_thumb_hover: Option<SharedString>,
    /// 次要背景色。
    #[serde(rename = "secondary.background")]
    pub secondary: Option<SharedString>,
    /// 激活次要背景色。
    #[serde(rename = "secondary.active.background")]
    pub secondary_active: Option<SharedString>,
    /// 次要文本颜色，用于次要按钮文本颜色或次要文本。
    #[serde(rename = "secondary.foreground")]
    pub secondary_foreground: Option<SharedString>,
    /// 悬停次要背景色。
    #[serde(rename = "secondary.hover.background")]
    pub secondary_hover: Option<SharedString>,
    /// 输入框选中背景色。
    #[serde(rename = "selection.background")]
    pub selection: Option<SharedString>,
    /// 侧边栏背景色。
    #[serde(rename = "sidebar.background")]
    pub sidebar: Option<SharedString>,
    /// 侧边栏强调背景色。
    #[serde(rename = "sidebar.accent.background")]
    pub sidebar_accent: Option<SharedString>,
    /// 侧边栏强调文本颜色。
    #[serde(rename = "sidebar.accent.foreground")]
    pub sidebar_accent_foreground: Option<SharedString>,
    /// 侧边栏边框颜色。
    #[serde(rename = "sidebar.border")]
    pub sidebar_border: Option<SharedString>,
    /// 侧边栏文本颜色。
    #[serde(rename = "sidebar.foreground")]
    pub sidebar_foreground: Option<SharedString>,
    /// 侧边栏主要背景色。
    #[serde(rename = "sidebar.primary.background")]
    pub sidebar_primary: Option<SharedString>,
    /// 侧边栏主要文本颜色。
    #[serde(rename = "sidebar.primary.foreground")]
    pub sidebar_primary_foreground: Option<SharedString>,
    /// 骨架屏背景色。
    #[serde(rename = "skeleton.background")]
    pub skeleton: Option<SharedString>,
    /// 滑块条背景色。
    #[serde(rename = "slider.background")]
    pub slider_bar: Option<SharedString>,
    /// 滑块滑块背景色。
    #[serde(rename = "slider.thumb.background")]
    pub slider_thumb: Option<SharedString>,
    /// 成功背景色。
    #[serde(rename = "success.background")]
    pub success: Option<SharedString>,
    /// 成功文本颜色。
    #[serde(rename = "success.foreground")]
    pub success_foreground: Option<SharedString>,
    /// 成功悬停背景色。
    #[serde(rename = "success.hover.background")]
    pub success_hover: Option<SharedString>,
    /// 成功激活背景色。
    #[serde(rename = "success.active.background")]
    pub success_active: Option<SharedString>,
    /// 开关背景色。
    #[serde(rename = "switch.background")]
    pub switch: Option<SharedString>,
    /// 开关滑块背景色。
    #[serde(rename = "switch.thumb.background")]
    pub switch_thumb: Option<SharedString>,
    /// 标签页背景色。
    #[serde(rename = "tab.background")]
    pub tab: Option<SharedString>,
    /// 标签页激活背景色。
    #[serde(rename = "tab.active.background")]
    pub tab_active: Option<SharedString>,
    /// 标签页激活文本颜色。
    #[serde(rename = "tab.active.foreground")]
    pub tab_active_foreground: Option<SharedString>,
    /// 标签栏背景色。
    #[serde(rename = "tab_bar.background")]
    pub tab_bar: Option<SharedString>,
    /// 标签栏分段背景色。
    #[serde(rename = "tab_bar.segmented.background")]
    pub tab_bar_segmented: Option<SharedString>,
    /// 标签页文本颜色。
    #[serde(rename = "tab.foreground")]
    pub tab_foreground: Option<SharedString>,
    /// 表格背景色。
    #[serde(rename = "table.background")]
    pub table: Option<SharedString>,
    /// 表格激活项背景色。
    #[serde(rename = "table.active.background")]
    pub table_active: Option<SharedString>,
    /// 表格激活项边框颜色。
    #[serde(rename = "table.active.border")]
    pub table_active_border: Option<SharedString>,
    /// 偶数表格行的条纹背景色。
    #[serde(rename = "table.even.background")]
    pub table_even: Option<SharedString>,
    /// 表格头部背景色。
    #[serde(rename = "table.head.background")]
    pub table_head: Option<SharedString>,
    /// 表格头部文本颜色。
    #[serde(rename = "table.head.foreground")]
    pub table_head_foreground: Option<SharedString>,
    /// 表格页脚背景色。
    #[serde(rename = "table.foot.background")]
    pub table_foot: Option<SharedString>,
    /// 表格页脚文本颜色。
    #[serde(rename = "table.foot.foreground")]
    pub table_foot_foreground: Option<SharedString>,
    /// 表格项悬停背景色。
    #[serde(rename = "table.hover.background")]
    pub table_hover: Option<SharedString>,
    /// 表格行边框颜色。
    #[serde(rename = "table.row.border")]
    pub table_row_border: Option<SharedString>,
    /// 标题栏背景色，用于窗口标题栏。
    #[serde(rename = "title_bar.background")]
    pub title_bar: Option<SharedString>,
    /// 标题栏边框颜色。
    #[serde(rename = "title_bar.border")]
    pub title_bar_border: Option<SharedString>,
    /// 状态栏背景色，用于底部状态栏。
    #[serde(rename = "status_bar.background")]
    pub status_bar: Option<SharedString>,
    /// 状态栏边框颜色。
    #[serde(rename = "status_bar.border")]
    pub status_bar_border: Option<SharedString>,
    /// 磁贴背景色。
    #[serde(rename = "tiles.background")]
    pub tiles: Option<SharedString>,
    /// 警告背景色。
    #[serde(rename = "warning.background")]
    pub warning: Option<SharedString>,
    /// 警告激活背景色。
    #[serde(rename = "warning.active.background")]
    pub warning_active: Option<SharedString>,
    /// 警告悬停背景色。
    #[serde(rename = "warning.hover.background")]
    pub warning_hover: Option<SharedString>,
    /// 警告前景色。
    #[serde(rename = "warning.foreground")]
    pub warning_foreground: Option<SharedString>,
    /// 覆盖层背景色。
    #[serde(rename = "overlay")]
    pub overlay: Option<SharedString>,
    /// 窗口边框颜色。
    ///
    /// # 平台特定：
    ///
    /// 此选项仅在 Linux 上有效，其他平台无法更改窗口边框颜色。
    #[serde(rename = "window.border")]
    pub window_border: Option<SharedString>,

    /// 基础蓝色。
    #[serde(rename = "base.blue")]
    blue: Option<String>,
    /// 基础浅蓝色。
    #[serde(rename = "base.blue.light")]
    blue_light: Option<String>,
    /// 基础青色。
    #[serde(rename = "base.cyan")]
    cyan: Option<String>,
    /// 基础浅青色。
    #[serde(rename = "base.cyan.light")]
    cyan_light: Option<String>,
    /// 基础绿色。
    #[serde(rename = "base.green")]
    green: Option<String>,
    /// 基础浅绿色。
    #[serde(rename = "base.green.light")]
    green_light: Option<String>,
    /// 基础品红色。
    #[serde(rename = "base.magenta")]
    magenta: Option<String>,
    #[serde(rename = "base.magenta.light")]
    magenta_light: Option<String>,
    /// 基础红色。
    #[serde(rename = "base.red")]
    red: Option<String>,
    /// 基础浅红色。
    #[serde(rename = "base.red.light")]
    red_light: Option<String>,
    /// 基础黄色。
    #[serde(rename = "base.yellow")]
    yellow: Option<String>,
    /// 基础浅黄色。
    #[serde(rename = "base.yellow.light")]
    yellow_light: Option<String>,
}

impl ThemeColor {
    /// 从 `ThemeConfig` 创建新的 `ThemeColor`。
    pub(crate) fn apply_config(
        &mut self,
        config: &ThemeConfig,
        default_theme: &ThemeColor,
    ) -> ThemeTokens {
        let colors = config.colors.clone();
        let default_tokens = ThemeTokens::from(default_theme);
        let mut tokens = default_tokens;

        macro_rules! apply_color {
            ($config_field:ident) => {
                if let Some(value) = &colors.$config_field {
                    self.$config_field =
                        try_parse_color(value).unwrap_or(default_theme.$config_field);
                } else {
                    self.$config_field = default_theme.$config_field;
                }
                tokens.$config_field = self.$config_field.into();
            };
            // With fallback
            ($config_field:ident, fallback = $fallback:expr) => {
                let fallback: rgpui::Hsla = ($fallback).into();
                if let Some(value) = &colors.$config_field {
                    self.$config_field = try_parse_color(value).unwrap_or(fallback);
                } else {
                    self.$config_field = fallback;
                }
                tokens.$config_field = self.$config_field.into();
            };
        }

        macro_rules! apply_background_color {
            ($config_field:ident) => {
                let token = if let Some(value) = &colors.$config_field {
                    if let Ok(token) = try_parse_theme_token(&value) {
                        token
                    } else {
                        default_tokens.$config_field
                    }
                } else {
                    default_tokens.$config_field
                };
                self.$config_field = token.color;
                tokens.$config_field = token;
            };
            ($config_field:ident, fallback = $fallback:expr) => {
                let fallback: ThemeToken = ($fallback).into();
                let token = if let Some(value) = &colors.$config_field {
                    if let Ok(token) = try_parse_theme_token(&value) {
                        token
                    } else {
                        fallback
                    }
                } else {
                    fallback
                };
                self.$config_field = token.color;
                tokens.$config_field = token;
            };
        }

        apply_background_color!(background);

        // Base colors for fallback
        apply_color!(red);
        apply_color!(
            red_light,
            fallback = self.background.blend(self.red.opacity(0.8))
        );
        apply_color!(green);
        apply_color!(
            green_light,
            fallback = self.background.blend(self.green.opacity(0.8))
        );
        apply_color!(blue);
        apply_color!(
            blue_light,
            fallback = self.background.blend(self.blue.opacity(0.8))
        );
        apply_color!(magenta);
        apply_color!(
            magenta_light,
            fallback = self.background.blend(self.magenta.opacity(0.8))
        );
        apply_color!(yellow);
        apply_color!(
            yellow_light,
            fallback = self.background.blend(self.yellow.opacity(0.8))
        );
        apply_color!(cyan);
        apply_color!(
            cyan_light,
            fallback = self.background.blend(self.cyan.opacity(0.8))
        );

        apply_color!(border);
        apply_color!(foreground);
        apply_color!(input, fallback = self.border);
        apply_background_color!(muted);
        apply_color!(
            muted_foreground,
            fallback = self.muted.blend(self.foreground.opacity(0.7))
        );

        // Button colors
        let active_darken = if config.mode.is_dark() { 0.2 } else { 0.1 };
        let hover_opacity = 0.9;
        let transparent = rgpui::transparent_black();
        let button_background = if config.mode.is_dark() {
            self.input.mix_oklab(transparent, 0.3)
        } else {
            self.background
        };
        apply_background_color!(button, fallback = button_background);
        apply_color!(button_foreground, fallback = self.foreground);
        apply_background_color!(
            button_hover,
            fallback = self.input.mix_oklab(transparent, 0.5)
        );
        apply_background_color!(
            button_active,
            fallback = self.input.mix_oklab(transparent, 0.7)
        );
        apply_background_color!(primary);
        apply_color!(primary_foreground, fallback = self.foreground);
        apply_background_color!(
            primary_hover,
            fallback = self.background.blend(self.primary.opacity(hover_opacity))
        );
        apply_background_color!(
            primary_active,
            fallback = self.primary.darken(active_darken)
        );
        apply_background_color!(button_primary, fallback = tokens.primary);
        apply_color!(
            button_primary_foreground,
            fallback = self.primary_foreground
        );
        apply_background_color!(button_primary_hover, fallback = tokens.primary_hover);
        apply_background_color!(button_primary_active, fallback = tokens.primary_active);
        apply_background_color!(secondary);
        apply_color!(secondary_foreground, fallback = self.foreground);
        apply_background_color!(
            secondary_hover,
            fallback = self.background.blend(self.secondary.opacity(hover_opacity))
        );
        apply_background_color!(
            secondary_active,
            fallback = self.secondary.darken(active_darken)
        );
        apply_background_color!(button_secondary, fallback = tokens.secondary);
        apply_color!(
            button_secondary_foreground,
            fallback = self.secondary_foreground
        );
        apply_background_color!(button_secondary_hover, fallback = tokens.secondary_hover);
        apply_background_color!(button_secondary_active, fallback = tokens.secondary_active);
        apply_background_color!(success, fallback = self.green);
        apply_color!(success_foreground, fallback = self.primary_foreground);
        apply_background_color!(
            success_hover,
            fallback = self.background.blend(self.success.opacity(hover_opacity))
        );
        apply_background_color!(
            success_active,
            fallback = self.success.darken(active_darken)
        );
        apply_background_color!(
            button_success,
            fallback = self.success.mix_oklab(transparent, 0.2)
        );
        apply_color!(button_success_foreground, fallback = self.success);
        apply_background_color!(
            button_success_hover,
            fallback = self.success.mix_oklab(transparent, 0.3)
        );
        apply_background_color!(
            button_success_active,
            fallback = self.success.mix_oklab(transparent, 0.4)
        );
        apply_background_color!(info, fallback = self.cyan);
        apply_color!(info_foreground, fallback = self.primary_foreground);
        apply_background_color!(
            info_hover,
            fallback = self.background.blend(self.info.opacity(hover_opacity))
        );
        apply_background_color!(info_active, fallback = self.info.darken(active_darken));
        apply_background_color!(
            button_info,
            fallback = self.info.mix_oklab(transparent, 0.2)
        );
        apply_color!(button_info_foreground, fallback = self.info);
        apply_background_color!(
            button_info_hover,
            fallback = self.info.mix_oklab(transparent, 0.3)
        );
        apply_background_color!(
            button_info_active,
            fallback = self.info.mix_oklab(transparent, 0.4)
        );
        apply_background_color!(warning, fallback = self.yellow);
        apply_color!(warning_foreground, fallback = self.primary_foreground);
        apply_background_color!(
            warning_hover,
            fallback = self.background.blend(self.warning.opacity(0.9))
        );
        apply_background_color!(
            warning_active,
            fallback = self.background.blend(self.warning.darken(active_darken))
        );
        apply_background_color!(
            button_warning,
            fallback = self.warning.mix_oklab(transparent, 0.2)
        );
        apply_color!(button_warning_foreground, fallback = self.warning);
        apply_background_color!(
            button_warning_hover,
            fallback = self.warning.mix_oklab(transparent, 0.3)
        );
        apply_background_color!(
            button_warning_active,
            fallback = self.warning.mix_oklab(transparent, 0.4)
        );

        // Other colors
        apply_background_color!(accent, fallback = tokens.secondary);
        apply_color!(accent_foreground, fallback = self.foreground);
        apply_background_color!(accordion, fallback = tokens.background);
        apply_background_color!(
            group_box,
            fallback = self
                .background
                .blend(
                    self.secondary
                        .opacity(if config.mode.is_dark() { 0.3 } else { 0.4 })
                )
        );
        apply_color!(group_box_foreground, fallback = self.foreground);
        apply_color!(caret, fallback = self.primary);
        apply_color!(chart_1, fallback = self.blue.lighten(0.4));
        apply_color!(chart_2, fallback = self.blue.lighten(0.2));
        apply_color!(chart_3, fallback = self.blue);
        apply_color!(chart_4, fallback = self.blue.darken(0.2));
        apply_color!(chart_5, fallback = self.blue.darken(0.4));
        apply_color!(chart_bullish, fallback = self.green);
        apply_color!(chart_bearish, fallback = self.red);
        apply_background_color!(danger, fallback = self.red);
        apply_background_color!(danger_active, fallback = self.danger.darken(active_darken));
        apply_color!(danger_foreground, fallback = self.primary_foreground);
        apply_background_color!(
            danger_hover,
            fallback = self.background.blend(self.danger.opacity(0.9))
        );
        apply_background_color!(
            button_danger,
            fallback = self.danger.mix_oklab(transparent, 0.2)
        );
        apply_color!(button_danger_foreground, fallback = self.danger);
        apply_background_color!(
            button_danger_hover,
            fallback = self.danger.mix_oklab(transparent, 0.3)
        );
        apply_background_color!(
            button_danger_active,
            fallback = self.danger.mix_oklab(transparent, 0.4)
        );
        apply_background_color!(
            description_list_label,
            fallback = self.background.blend(self.border.opacity(0.2))
        );
        apply_color!(
            description_list_label_foreground,
            fallback = self.muted_foreground
        );
        apply_color!(drag_border, fallback = self.primary.opacity(0.65));
        apply_background_color!(drop_target, fallback = self.primary.opacity(0.2));
        apply_color!(link, fallback = self.primary);
        apply_color!(link_active, fallback = self.link);
        apply_color!(link_hover, fallback = self.link);
        apply_background_color!(list, fallback = tokens.background);
        apply_background_color!(
            list_active,
            fallback = self.background.blend(self.primary.opacity(0.1))
        );
        apply_color!(
            list_active_border,
            fallback = self.background.blend(self.primary.opacity(0.6))
        );
        apply_background_color!(list_even, fallback = tokens.list);
        apply_background_color!(list_head, fallback = tokens.list);
        apply_background_color!(list_hover, fallback = self.accent.opacity(0.6));
        apply_background_color!(popover, fallback = tokens.background);
        apply_color!(popover_foreground, fallback = self.foreground);
        apply_background_color!(progress_bar, fallback = tokens.primary);
        apply_color!(ring, fallback = self.blue);
        apply_background_color!(scrollbar, fallback = tokens.background);
        apply_background_color!(scrollbar_thumb, fallback = tokens.accent);
        apply_background_color!(scrollbar_thumb_hover, fallback = tokens.scrollbar_thumb);
        apply_background_color!(selection, fallback = tokens.primary);
        apply_background_color!(
            sidebar,
            fallback = self.background.blend(self.border.opacity(0.15))
        );
        apply_background_color!(sidebar_accent, fallback = tokens.accent);
        apply_color!(sidebar_accent_foreground, fallback = self.accent_foreground);
        apply_color!(sidebar_border, fallback = self.border);
        apply_color!(sidebar_foreground, fallback = self.foreground);
        apply_background_color!(sidebar_primary, fallback = tokens.primary);
        apply_color!(
            sidebar_primary_foreground,
            fallback = self.primary_foreground
        );
        apply_background_color!(skeleton, fallback = tokens.secondary);
        apply_background_color!(slider_bar, fallback = tokens.primary);
        apply_background_color!(slider_thumb, fallback = self.primary_foreground);
        apply_background_color!(switch, fallback = tokens.secondary_active);
        apply_background_color!(switch_thumb, fallback = tokens.background);
        apply_background_color!(tab, fallback = tokens.background);
        apply_background_color!(tab_active, fallback = tokens.background);
        apply_color!(tab_active_foreground, fallback = self.foreground);
        apply_background_color!(tab_bar, fallback = tokens.background);
        apply_background_color!(tab_bar_segmented, fallback = tokens.secondary);
        apply_color!(tab_foreground, fallback = self.foreground);
        apply_background_color!(table, fallback = tokens.list);
        apply_background_color!(table_active, fallback = tokens.list_active);
        apply_color!(table_active_border, fallback = self.list_active_border);
        apply_background_color!(table_even, fallback = tokens.list_even);
        apply_background_color!(table_head, fallback = tokens.list_head);
        apply_color!(table_head_foreground, fallback = self.muted_foreground);
        apply_background_color!(table_foot, fallback = tokens.list_head);
        apply_color!(table_foot_foreground, fallback = self.muted_foreground);
        apply_background_color!(table_hover, fallback = tokens.list_hover);
        apply_color!(table_row_border, fallback = self.border);
        apply_background_color!(title_bar, fallback = tokens.background);
        apply_color!(title_bar_border, fallback = self.border);
        apply_background_color!(status_bar, fallback = tokens.title_bar);
        apply_color!(status_bar_border, fallback = self.title_bar_border);
        apply_background_color!(tiles, fallback = tokens.background);
        apply_background_color!(overlay);
        apply_color!(window_border, fallback = self.border);

        // TODO: Apply default fallback colors to highlight.

        // Ensure opacity for list_active, table_active, selection.
        let clamp_alpha = |raw: Option<&str>, color: Hsla, background: Background, max: f32| {
            let base = color.a;
            let target = base.min(max);
            let color = color.alpha(target);
            let background = raw
                .and_then(|value| try_parse_background_clamped(value, max).ok())
                .unwrap_or_else(|| {
                    let factor = if base > 0. { target / base } else { 1. };
                    background.opacity(factor)
                });
            (color, ThemeToken::new(color, background))
        };

        (self.list_active, tokens.list_active) = clamp_alpha(
            colors.list_active.as_deref(),
            self.list_active,
            tokens.list_active.background,
            0.2,
        );
        (self.table_active, tokens.table_active) = clamp_alpha(
            colors.table_active.as_deref(),
            self.table_active,
            tokens.table_active.background,
            0.2,
        );
        (self.selection, tokens.selection) = clamp_alpha(
            colors.selection.as_deref(),
            self.selection,
            tokens.selection.background,
            0.3,
        );

        tokens
    }
}

impl Theme {
    /// 将给定的主题配置应用到当前主题。
    pub fn apply_config(&mut self, config: &Rc<ThemeConfig>) {
        if config.mode.is_dark() {
            self.dark_theme = config.clone();
        } else {
            self.light_theme = config.clone();
        }
        if let Some(style) = &config.highlight {
            let highlight_theme = Arc::new(HighlightTheme {
                name: config.name.to_string(),
                appearance: config.mode,
                style: style.clone(),
            });
            self.highlight_theme = highlight_theme;
        }

        let default_colors = if config.mode.is_dark() {
            ThemeColor::dark()
        } else {
            ThemeColor::light()
        };

        if let Some(font_size) = config.font_size {
            self.font_size = px(font_size);
        }
        if let Some(font_family) = &config.font_family {
            self.font_family = font_family.clone();
        }
        if let Some(mono_font_family) = &config.mono_font_family {
            self.mono_font_family = mono_font_family.clone();
        }
        if let Some(mono_font_size) = config.mono_font_size {
            self.mono_font_size = px(mono_font_size);
        }
        if let Some(radius) = config.radius {
            self.radius = px(radius as f32);
        }
        if let Some(radius_lg) = config.radius_lg {
            self.radius_lg = px(radius_lg as f32);
        }
        if let Some(shadow) = config.shadow {
            self.shadow = shadow;
        }

        self.tokens = self.colors.apply_config(&config, &default_colors);
        self.mode = config.mode;
    }
}

#[cfg(test)]
mod tests {
    use rgpui::{linear_color_stop, linear_gradient};

    use crate::theme::color::{blue_600, red_500};
    use crate::{Theme, ThemeConfig, ThemeMode, try_parse_color};

    #[test]
    fn test_apply_config_preserves_gradient_background_and_solid_color_fallback() {
        let config = serde_json::from_value::<ThemeConfig>(serde_json::json!({
            "name": "Gradient",
            "mode": "light",
            "colors": {
                "primary.background": "linear-gradient(135deg, #4F46E5, #06B6D4)",
                "button.primary.hover.background": "linear-gradient(to right, red-500 25%, blue-600 75%)"
            }
        }))
        .unwrap();

        let mut theme = Theme::default();
        theme.apply_config(&std::rc::Rc::new(config));

        let primary_from = try_parse_color("#4F46E5").unwrap();
        let primary_to = try_parse_color("#06B6D4").unwrap();
        assert_eq!(theme.primary, primary_from);
        assert_eq!(theme.tokens.primary.color, primary_from);
        assert_eq!(
            theme.tokens.primary.background,
            linear_gradient(
                135.,
                linear_color_stop(primary_from, 0.),
                linear_color_stop(primary_to, 1.)
            )
        );
        assert_eq!(
            theme.tokens.button_primary.background,
            theme.tokens.primary.background
        );
        assert_eq!(
            theme.tokens.button_primary_hover.background,
            linear_gradient(
                90.,
                linear_color_stop(red_500(), 0.25),
                linear_color_stop(blue_600(), 0.75)
            )
        );
        assert_eq!(theme.mode, ThemeMode::Light);
    }

    #[test]
    fn test_apply_config_clamps_highlight_alpha_per_gradient_stop() {
        let config = serde_json::from_value::<ThemeConfig>(serde_json::json!({
            "name": "Highlight",
            "mode": "light",
            "colors": {
                // Solid above the cap: must be capped to 0.2, not attenuated twice.
                "list.active.background": "#3b82f6",
                // Gradient with a faint `from` stop and an opaque `to` stop: the
                // `to` stop must be clamped independently, not left at full alpha.
                "table.active.background": "linear-gradient(#bfdbfe33, #3b82f6)",
                // Gradient with a transparent `from` stop: the opaque `to` stop
                // must still be clamped (the `base == 0` factor fallback used to
                // leave it untouched).
                "selection.background": "linear-gradient(#3b82f600, #3b82f6)",
            }
        }))
        .unwrap();

        let mut theme = Theme::default();
        theme.apply_config(&std::rc::Rc::new(config));

        // Solid: representative color and rendered background both capped at 0.2.
        let blue = try_parse_color("#3b82f6").unwrap();
        assert_eq!(theme.list_active, blue.alpha(0.2));
        assert_eq!(theme.tokens.list_active.background, blue.alpha(0.2).into());

        // Gradient: the opaque `to` stop is clamped to 0.2, not left fully opaque.
        let faint = try_parse_color("#bfdbfe33").unwrap();
        assert_eq!(
            theme.tokens.table_active.background,
            linear_gradient(
                180.,
                linear_color_stop(faint.alpha(faint.a.min(0.2)), 0.),
                linear_color_stop(blue.alpha(0.2), 1.),
            )
        );

        // Gradient: a transparent `from` stop stays transparent while the opaque
        // `to` stop is still clamped to 0.3 (selection cap).
        let clear = try_parse_color("#3b82f600").unwrap();
        assert_eq!(
            theme.tokens.selection.background,
            linear_gradient(
                180.,
                linear_color_stop(clear.alpha(clear.a.min(0.3)), 0.),
                linear_color_stop(blue.alpha(0.3), 1.),
            )
        );
    }
}
