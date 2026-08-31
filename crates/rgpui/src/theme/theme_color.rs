use std::{ops::Deref, sync::Arc};

use crate::{ThemeMode, theme::DEFAULT_THEME_COLORS};

use rgpui::{Background, Fill, Hsla};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 主题令牌：持有代表颜色及其可渲染背景。
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ThemeToken {
    /// 代表颜色。
    pub color: Hsla,
    /// 可渲染背景（纯色或渐变）。
    pub background: Background,
}

impl ThemeToken {
    /// 从颜色与背景创建主题令牌。
    pub fn new(color: Hsla, background: Background) -> Self {
        Self { color, background }
    }
}

impl Deref for ThemeToken {
    type Target = Hsla;

    fn deref(&self) -> &Self::Target {
        &self.color
    }
}

impl From<Hsla> for ThemeToken {
    fn from(color: Hsla) -> Self {
        Self {
            color,
            background: color.into(),
        }
    }
}

impl From<ThemeToken> for Hsla {
    fn from(token: ThemeToken) -> Self {
        token.color
    }
}

impl From<ThemeToken> for Background {
    fn from(token: ThemeToken) -> Self {
        token.background
    }
}

impl From<ThemeToken> for Fill {
    fn from(token: ThemeToken) -> Self {
        Fill::Color(token.background)
    }
}

/// UI 组件中使用的主题颜色。
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct ThemeColor {
    /// 强调色，用于 MenuItem、ListItem 等的悬停背景。
    pub accent: Hsla,
    /// 强调文字颜色。
    pub accent_foreground: Hsla,
    /// 折叠面板背景颜色。
    pub accordion: Hsla,
    /// 默认背景颜色。
    pub background: Hsla,
    /// 默认边框颜色。
    pub border: Hsla,
    /// 默认按钮背景颜色。
    pub button: Hsla,
    /// 默认按钮激活背景颜色。
    pub button_active: Hsla,
    /// 默认按钮文字颜色。
    pub button_foreground: Hsla,
    /// 默认按钮悬停背景颜色。
    pub button_hover: Hsla,
    /// 危险按钮背景颜色，回退至 `danger`。
    pub button_danger: Hsla,
    /// 危险按钮激活背景颜色，回退至 `danger_active`。
    pub button_danger_active: Hsla,
    /// 危险按钮文字颜色，回退至 `danger_foreground`。
    pub button_danger_foreground: Hsla,
    /// 危险按钮悬停背景颜色，回退至 `danger_hover`。
    pub button_danger_hover: Hsla,
    /// 信息按钮背景颜色，回退至 `info`。
    pub button_info: Hsla,
    /// 信息按钮激活背景颜色，回退至 `info_active`。
    pub button_info_active: Hsla,
    /// 信息按钮文字颜色，回退至 `info_foreground`。
    pub button_info_foreground: Hsla,
    /// 信息按钮悬停背景颜色，回退至 `info_hover`。
    pub button_info_hover: Hsla,
    /// 主要按钮背景颜色，回退至 `primary`。
    pub button_primary: Hsla,
    /// 主要按钮激活背景颜色，回退至 `primary_active`。
    pub button_primary_active: Hsla,
    /// 主要按钮文字颜色，回退至 `primary_foreground`。
    pub button_primary_foreground: Hsla,
    /// 主要按钮悬停背景颜色，回退至 `primary_hover`。
    pub button_primary_hover: Hsla,
    /// 次要按钮背景颜色，回退至 `secondary`。
    pub button_secondary: Hsla,
    /// 次要按钮激活背景颜色，回退至 `secondary_active`。
    pub button_secondary_active: Hsla,
    /// 次要按钮文字颜色，回退至 `secondary_foreground`。
    pub button_secondary_foreground: Hsla,
    /// 次要按钮悬停背景颜色，回退至 `secondary_hover`。
    pub button_secondary_hover: Hsla,
    /// 成功按钮背景颜色，回退至 `success`。
    pub button_success: Hsla,
    /// 成功按钮激活背景颜色，回退至 `success_active`。
    pub button_success_active: Hsla,
    /// 成功按钮文字颜色，回退至 `success_foreground`。
    pub button_success_foreground: Hsla,
    /// 成功按钮悬停背景颜色，回退至 `success_hover`。
    pub button_success_hover: Hsla,
    /// 警告按钮背景颜色，回退至 `warning`。
    pub button_warning: Hsla,
    /// 警告按钮激活背景颜色，回退至 `warning_active`。
    pub button_warning_active: Hsla,
    /// 警告按钮文字颜色，回退至 `warning_foreground`。
    pub button_warning_foreground: Hsla,
    /// 警告按钮悬停背景颜色，回退至 `warning_hover`。
    pub button_warning_hover: Hsla,
    /// 分组框背景颜色。
    pub group_box: Hsla,
    /// 分组框文字颜色。
    pub group_box_foreground: Hsla,
    /// 输入框光标颜色（闪烁光标）。
    pub caret: Hsla,
    /// 图表 1 颜色。
    pub chart_1: Hsla,
    /// 图表 2 颜色。
    pub chart_2: Hsla,
    /// 图表 3 颜色。
    pub chart_3: Hsla,
    /// 图表 4 颜色。
    pub chart_4: Hsla,
    /// 图表 5 颜色。
    pub chart_5: Hsla,
    /// K 线图阳线颜色（价格上涨）。
    pub chart_bullish: Hsla,
    /// K 线图阴线颜色（价格下跌）。
    pub chart_bearish: Hsla,
    /// 危险背景颜色。
    pub danger: Hsla,
    /// 危险激活背景颜色。
    pub danger_active: Hsla,
    /// 危险文字颜色。
    pub danger_foreground: Hsla,
    /// 危险悬停背景颜色。
    pub danger_hover: Hsla,
    /// 描述列表标签背景颜色。
    pub description_list_label: Hsla,
    /// 描述列表标签前景颜色。
    pub description_list_label_foreground: Hsla,
    /// 拖拽边框颜色。
    pub drag_border: Hsla,
    /// 拖放目标背景颜色。
    pub drop_target: Hsla,
    /// 默认文字颜色。
    pub foreground: Hsla,
    /// 信息背景颜色。
    pub info: Hsla,
    /// 信息激活背景颜色。
    pub info_active: Hsla,
    /// 信息文字颜色。
    pub info_foreground: Hsla,
    /// 信息悬停背景颜色。
    pub info_hover: Hsla,
    /// 输入框（如 Input、Select 等）的边框颜色。
    pub input: Hsla,
    /// 链接文字颜色。
    pub link: Hsla,
    /// 激活链接文字颜色。
    pub link_active: Hsla,
    /// 悬停链接文字颜色。
    pub link_hover: Hsla,
    /// 列表和列表项的背景颜色。
    pub list: Hsla,
    /// 激活列表项的背景颜色。
    pub list_active: Hsla,
    /// 激活列表项的边框颜色。
    pub list_active_border: Hsla,
    /// 偶数列表项的条纹背景颜色。
    pub list_even: Hsla,
    /// 列表头部背景颜色。
    pub list_head: Hsla,
    /// 列表项悬停背景颜色。
    pub list_hover: Hsla,
    /// 柔和背景颜色，用于骨架屏和开关等。
    pub muted: Hsla,
    /// 柔和文字颜色，用于禁用文本。
    pub muted_foreground: Hsla,
    /// 弹出框背景颜色。
    pub popover: Hsla,
    /// 弹出框文字颜色。
    pub popover_foreground: Hsla,
    /// 主要背景颜色。
    pub primary: Hsla,
    /// 主要激活背景颜色。
    pub primary_active: Hsla,
    /// 主要文字颜色。
    pub primary_foreground: Hsla,
    /// 主要悬停背景颜色。
    pub primary_hover: Hsla,
    /// 进度条背景颜色。
    pub progress_bar: Hsla,
    /// 用于焦点环。
    pub ring: Hsla,
    /// 滚动条背景颜色。
    pub scrollbar: Hsla,
    /// 滚动条滑块背景颜色。
    pub scrollbar_thumb: Hsla,
    /// 滚动条滑块悬停背景颜色。
    pub scrollbar_thumb_hover: Hsla,
    /// 次要背景颜色。
    pub secondary: Hsla,
    /// 次要激活背景颜色。
    pub secondary_active: Hsla,
    /// 次要文字颜色，用于次要按钮文字或次要文本。
    pub secondary_foreground: Hsla,
    /// 次要悬停背景颜色。
    pub secondary_hover: Hsla,
    /// 输入框选中背景颜色。
    pub selection: Hsla,
    /// 侧边栏背景颜色。
    pub sidebar: Hsla,
    /// 侧边栏强调背景颜色。
    pub sidebar_accent: Hsla,
    /// 侧边栏强调文字颜色。
    pub sidebar_accent_foreground: Hsla,
    /// 侧边栏边框颜色。
    pub sidebar_border: Hsla,
    /// 侧边栏文字颜色。
    pub sidebar_foreground: Hsla,
    /// 侧边栏主要背景颜色。
    pub sidebar_primary: Hsla,
    /// 侧边栏主要文字颜色。
    pub sidebar_primary_foreground: Hsla,
    /// 骨架屏背景颜色。
    pub skeleton: Hsla,
    /// 滑块条背景颜色。
    pub slider_bar: Hsla,
    /// 滑块手柄背景颜色。
    pub slider_thumb: Hsla,
    /// 成功背景颜色。
    pub success: Hsla,
    /// 成功文字颜色。
    pub success_foreground: Hsla,
    /// 成功悬停背景颜色。
    pub success_hover: Hsla,
    /// 成功激活背景颜色。
    pub success_active: Hsla,
    /// 开关背景颜色。
    pub switch: Hsla,
    /// 开关手柄背景颜色。
    pub switch_thumb: Hsla,
    /// 标签页背景颜色。
    pub tab: Hsla,
    /// 标签页激活背景颜色。
    pub tab_active: Hsla,
    /// 标签页激活文字颜色。
    pub tab_active_foreground: Hsla,
    /// 标签栏背景颜色。
    pub tab_bar: Hsla,
    /// 标签栏分段背景颜色。
    pub tab_bar_segmented: Hsla,
    /// 标签页文字颜色。
    pub tab_foreground: Hsla,
    /// 表格背景颜色。
    pub table: Hsla,
    /// 表格激活项背景颜色。
    pub table_active: Hsla,
    /// 表格激活项边框颜色。
    pub table_active_border: Hsla,
    /// 偶数表格行的条纹背景颜色。
    pub table_even: Hsla,
    /// 表格表头背景颜色。
    pub table_head: Hsla,
    /// 表格表头文字颜色。
    pub table_head_foreground: Hsla,
    /// 表格表尾背景颜色。
    pub table_foot: Hsla,
    /// 表格表尾文字颜色。
    pub table_foot_foreground: Hsla,
    /// 表格项悬停背景颜色。
    pub table_hover: Hsla,
    /// 表格行边框颜色。
    pub table_row_border: Hsla,
    /// 标题栏背景颜色，用于窗口标题栏。
    pub title_bar: Hsla,
    /// 标题栏边框颜色。
    pub title_bar_border: Hsla,
    /// 状态栏背景颜色，用于底部状态栏。
    pub status_bar: Hsla,
    /// 状态栏边框颜色。
    pub status_bar_border: Hsla,
    /// 磁贴背景颜色。
    pub tiles: Hsla,
    /// 警告背景颜色。
    pub warning: Hsla,
    /// 警告激活背景颜色。
    pub warning_active: Hsla,
    /// 警告悬停背景颜色。
    pub warning_hover: Hsla,
    /// 警告前景颜色。
    pub warning_foreground: Hsla,
    /// 覆盖层背景颜色。
    pub overlay: Hsla,
    /// 窗口边框颜色。
    ///
    /// # 平台特定：
    ///
    /// 仅在 Linux 上有效，其他平台无法更改窗口边框颜色。
    pub window_border: Hsla,

    /// 基础红色。
    pub red: Hsla,
    /// 基础浅红色。
    pub red_light: Hsla,
    /// 基础绿色。
    pub green: Hsla,
    /// 基础浅绿色。
    pub green_light: Hsla,
    /// 基础蓝色。
    pub blue: Hsla,
    /// 基础浅蓝色。
    pub blue_light: Hsla,
    /// 基础黄色。
    pub yellow: Hsla,
    /// 基础浅黄色。
    pub yellow_light: Hsla,
    /// 基础品红色。
    pub magenta: Hsla,
    /// 基础浅品红色。
    pub magenta_light: Hsla,
    /// 基础青色。
    pub cyan: Hsla,
    /// 基础浅青色。
    pub cyan_light: Hsla,
}

macro_rules! define_theme_tokens {
    ($($field:ident),+ $(,)?) => {
        /// 已解析的主题令牌：同时持有代表颜色与配置的绘制背景。
        #[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, JsonSchema)]
        pub struct ThemeTokens {
            $(/// 主题令牌（颜色 + 背景）。
            pub $field: ThemeToken,)+
        }

        impl From<ThemeColor> for ThemeTokens {
            fn from(colors: ThemeColor) -> Self {
                Self {
                    $($field: colors.$field.into(),)+
                }
            }
        }

        impl From<&ThemeColor> for ThemeTokens {
            fn from(colors: &ThemeColor) -> Self {
                Self::from(*colors)
            }
        }
    };
}

define_theme_tokens! {
    accent,
    accent_foreground,
    accordion,
    background,
    border,
    button,
    button_active,
    button_foreground,
    button_hover,
    button_danger,
    button_danger_active,
    button_danger_foreground,
    button_danger_hover,
    button_info,
    button_info_active,
    button_info_foreground,
    button_info_hover,
    button_primary,
    button_primary_active,
    button_primary_foreground,
    button_primary_hover,
    button_secondary,
    button_secondary_active,
    button_secondary_foreground,
    button_secondary_hover,
    button_success,
    button_success_active,
    button_success_foreground,
    button_success_hover,
    button_warning,
    button_warning_active,
    button_warning_foreground,
    button_warning_hover,
    group_box,
    group_box_foreground,
    caret,
    chart_1,
    chart_2,
    chart_3,
    chart_4,
    chart_5,
    chart_bullish,
    chart_bearish,
    danger,
    danger_active,
    danger_foreground,
    danger_hover,
    description_list_label,
    description_list_label_foreground,
    drag_border,
    drop_target,
    foreground,
    info,
    info_active,
    info_foreground,
    info_hover,
    input,
    link,
    link_active,
    link_hover,
    list,
    list_active,
    list_active_border,
    list_even,
    list_head,
    list_hover,
    muted,
    muted_foreground,
    popover,
    popover_foreground,
    primary,
    primary_active,
    primary_foreground,
    primary_hover,
    progress_bar,
    ring,
    scrollbar,
    scrollbar_thumb,
    scrollbar_thumb_hover,
    secondary,
    secondary_active,
    secondary_foreground,
    secondary_hover,
    selection,
    sidebar,
    sidebar_accent,
    sidebar_accent_foreground,
    sidebar_border,
    sidebar_foreground,
    sidebar_primary,
    sidebar_primary_foreground,
    skeleton,
    slider_bar,
    slider_thumb,
    success,
    success_foreground,
    success_hover,
    success_active,
    switch,
    switch_thumb,
    tab,
    tab_active,
    tab_active_foreground,
    tab_bar,
    tab_bar_segmented,
    tab_foreground,
    table,
    table_active,
    table_active_border,
    table_even,
    table_head,
    table_head_foreground,
    table_foot,
    table_foot_foreground,
    table_hover,
    table_row_border,
    title_bar,
    title_bar_border,
    status_bar,
    status_bar_border,
    tiles,
    warning,
    warning_active,
    warning_hover,
    warning_foreground,
    overlay,
    window_border,
    red,
    red_light,
    green,
    green_light,
    blue,
    blue_light,
    yellow,
    yellow_light,
    magenta,
    magenta_light,
    cyan,
    cyan_light,
}

impl ThemeColor {
    /// 获取默认浅色主题颜色。
    pub fn light() -> Arc<Self> {
        DEFAULT_THEME_COLORS[&ThemeMode::Light].0.clone()
    }

    /// 获取默认深色主题颜色。
    pub fn dark() -> Arc<Self> {
        DEFAULT_THEME_COLORS[&ThemeMode::Dark].0.clone()
    }
}
