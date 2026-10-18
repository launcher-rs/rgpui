# 主题（Theme）

**目录：** [概述](#概述) · [快速入门](#快速入门) · [读取主题色](#读取主题色) · [切换与自定义](#切换与自定义) · [主题注册表与热重载](#主题注册表与热重载) · [主题 JSON 格式](#主题-json-格式) · [语法高亮主题](#语法高亮主题) · [主题监听（theme_watcher）](#主题监听theme_watcher) · [组件设置](#组件设置) · [最佳实践](#最佳实践)

## 概述

rgpui 主题系统以全局单例 `Theme`（实现 `Global`）承载调色板、字体、圆角、
阴影、滚动条与语法高亮配置；组件经 `ActiveTheme::theme()` 读取当前主题。

**导出：** `pub mod theme;`（`rgpui.rs:74`）。crate 根重导出核心类型
（`rgpui.rs:408-412`）：`ActiveTheme`、`HighlightTheme`、`HighlightThemeStyle`、
`ListSettings`、`NotificationSettings`、`ScrollbarShow`、`SheetSettings`、
`StatusColors`、`SyntaxColors`、`TITLE_BAR_HEIGHT`、`Theme`、`ThemeColor`、
`ThemeConfig`、`ThemeConfigColors`、`ThemeMode`、`ThemeRegistry`、`ThemeSet`、
`ThemeToken`、`ThemeTokens`、`DEFAULT_THEME_COLORS`（`:407`），以及色板函数
（`:413-443`，`blue_500` 之类）。

**prelude 只含 `ActiveTheme`**（`prelude.rs:13`）。`theme_watcher` 的类型
（`ThemeColors`、`ThemeEvent`、`ThemeManager`、`ThemeMode`、`ThemeWatcher`）
也在 prelude（`prelude.rs:73`）。

> **命名冲突：** 根 `rgpui::ThemeMode` 只有 `Light`/`Dark` 两值
> （`theme/mod.rs:304`）；prelude 里的 `theme_watcher::ThemeMode` 有三值
> `Light`/`Dark`/`System`。同时用两者时需写全限定路径。

## 快速入门

```rust
fn main() {
    application().run(|cx: &mut App| {
        rgpui::theme::init(cx);        // 注册默认主题 + 加载 Light（theme/mod.rs:30）
        cx.open_window(/* ... */);
    });
}
```

在 `Render` 中读取主题（`ActiveTheme` 在 prelude）：

```rust
impl Render for MyView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .border_color(cx.theme().border)
    }
}
```

`Context<T>` 经 `Deref` 获得 `theme()`，所以 `cx.theme()` 直接可用
（`impl ActiveTheme for App`，`theme/mod.rs:44`）。

## 读取主题色

`Theme` 实现 `Deref<Target = ThemeColor>`（`theme/mod.rs:118`），因此
`cx.theme().background` 等价于 `cx.theme().colors.background`。

`ThemeColor`（`theme/theme_color.rs:62`）有 **140 个 `pub Hsla` 字段**，
常见的有：

```text
background  foreground  border  accent  accent_foreground
primary  primary_hover  primary_active  primary_foreground
danger  warning  info  success  muted  muted_foreground
popover  popover_foreground  selection  ring  link  caret
list  list_hover  list_active  list_active_border  list_even  list_head
table  table_hover  table_active  table_head  table_row_border
tab  tab_active  tab_bar  title_bar  status_bar  scrollbar  scrollbar_thumb
sidebar  sidebar_accent  switch  slider_thumb  skeleton  overlay
red  green  blue  yellow  magenta  cyan（及各自 _light 变体）
```

`Theme` 上的便捷方法：

| 方法 | 位置 | 说明 |
|------|------|------|
| `Theme::global(cx) -> &Theme` | `theme/mod.rs:137` | 全局引用 |
| `Theme::global_mut(cx) -> &mut Theme` | `:143` | 全局可变引用 |
| `is_dark()` | `:149` | 是否暗色 |
| `theme_name()` | `:154` | 当前主题名 |
| `input_background()` | `:235` | 输入框背景（暗色下混合 `transparent`） |

## 切换与自定义

```rust
// 按模式切换（Light/Dark），并刷新窗口
Theme::change(ThemeMode::Dark, Some(window), cx);   // theme/mod.rs:183

// 与系统外观同步
Theme::sync_system_appearance(Some(window), cx);    // :163

// 直接改全局主题字段
Theme::global_mut(cx).radius = px(8.);
cx.refresh_windows();  // 或 window.refresh()
```

运行时改色示例见 `examples/showcase/src/bin/theme.rs`
（`cargo run -p showcase --bin theme`）。

## 主题注册表与热重载

`ThemeRegistry`（`theme/registry.rs:166`，实现 `Global`）管理默认主题与从
JSON 加载的自定义主题。

```rust
// 从字符串加载主题集（名称冲突跳过）
ThemeRegistry::global_mut(cx).load_themes_from_str(json)?;   // :240

// 从目录加载全部 *.json（同名覆盖，返回加载数量）
let n = ThemeRegistry::global_mut(cx).load_themes_from_dir(Path::new("./themes")); // :247

// 按名称应用（未知名称返回 false，全局主题不动）
Theme::apply_named("Ayu Dark", Some(window), cx);            // theme/mod.rs:209

// 监视目录热重载（新增/修改 *.json 自动加载并重应用）
ThemeRegistry::watch_themes_dir(cx);                         // :191
```

| 方法 | 位置 |
|------|------|
| `ThemeRegistry::global(cx)` / `global_mut(cx)` | `registry.rs:177`/`:182` |
| `themes()` / `sorted_themes()` | `:197`/`:202` |
| `default_light_theme()` / `default_dark_theme()` | `:220`/`:225` |
| `themes_dir()` / `set_themes_dir(dir)` | `:230`/`:235` |
| `load_themes_from_str(content)` | `:240` |
| `load_themes_from_dir(dir) -> usize` | `:247` |
| `reload_themes_from_dir() -> usize` | `:274` |
| `watch_themes_dir(cx)` | `:191` |

## 主题 JSON 格式

一个 JSON 文件是一个 `ThemeSet`（`theme/schema.rs:24`），含多个 `ThemeConfig`：

```json
{
  "name": "Pack",
  "themes": [
    { "name": "Pack Dark", "mode": "dark", "colors": { "background": "#1e1e1e" } },
    { "name": "Pack Light", "mode": "light" }
  ]
}
```

- `ThemeConfig`（`schema.rs:39`）：`is_default`、`name`、`mode`，字体
  （`font.size`/`font.family`/`mono_font.family`/`mono_font.size`）、
  圆角（`radius`/`radius.lg`）、`shadow`、`colors`、`highlight`。
- `ThemeConfigColors`（`schema.rs:83`）：按 `"accent.background"` 之类键名
  覆盖 `ThemeColor`，全部字段 `Option`，未设的走默认。
- 生成 JSON Schema：`theme_json_schema() -> String`（`:947`）、
  `write_theme_json_schema(path)`（`:953`）。

## 语法高亮主题

`HighlightTheme`（`theme/highlight.rs:499`，兼容 Zed 主题格式）实现
`Deref<Target = SyntaxColors>`：

```rust
cx.theme().highlight_theme.style.editor_background   // Option<Hsla>
cx.theme().highlight_theme.style.syntax.keyword      // Option<ThemeStyle>
```

- `HighlightThemeStyle`（`highlight.rs:467`）：`editor_background`、
  `editor_foreground`、`editor_active_line`、`editor_line_number`、
  `editor_gutter_background`、`status: StatusColors`、`syntax: SyntaxColors`。
- `HighlightTheme::default_light()` / `default_dark()`（`:519`/`:524`）。
- `SyntaxColors::style(name) -> Option<HighlightStyle>`（`:239`）按 token 名取样式。
- `StatusColors`（`:315`）提供 `error()`/`warning()`/`info()`/`success()`/`hint()`
  及各自 `_background`/`_border` 方法（均需 `&App`，如 `status.error(cx)`）。

## 主题监听（theme_watcher）

`rgpui::theme_watcher`（`pub mod theme_watcher;`，`rgpui.rs:287`）提供独立于
`Theme` 全局的轻量监听器：

```rust
use rgpui::theme_watcher::{ThemeManager, ThemeMode, ThemeEvent};

let mut manager = ThemeManager::with_theme(ThemeMode::System);
manager.on_change(|event: &ThemeEvent| {
    // 响应主题变化
});
```

| 类型 | 位置 | 说明 |
|------|------|------|
| `ThemeMode` | `theme_watcher/mod.rs:18` | **三值** `Light`/`Dark`/`System` |
| `ThemeEvent` | `:29` | 主题变化事件 |
| `ThemeWatcher` | `:37` | `new()`/`with_theme()`/`current_theme()`/`set_theme()`/`toggle_theme()`/`on_change()`/`set_enabled()`/`is_enabled()` |
| `ThemeColors` | `:125` | `light()`/`dark()`/`for_theme(mode)` |
| `ThemeManager` | `:176` | `new()`/`with_theme()`/`colors()`/`watcher()`/`watcher_mut()`/`set_theme()`/`on_change()` |

## 组件设置

| 类型 | 位置 | 说明 |
|------|------|------|
| `ScrollbarShow` | `theme/settings.rs:11` | `Scrolling`（默认）/`Hover`/`Always`，含 `is_hover()`/`is_always()` |
| `ListSettings` | `:35` | `active_highlight: bool`（默认 true） |
| `SheetSettings` | `:50` | `margin_top: Pixels`（默认 `TITLE_BAR_HEIGHT`） |
| `NotificationSettings` | `:65` | `placement: Anchor`、`margins: Edges<Pixels>`、`max_items`（默认 10） |
| `TITLE_BAR_HEIGHT` | `:6` | `px(34.)` 常量 |

颜色工具（`rgpui::theme::color`，`rgpui.rs:384`）：
`hsl(h, s, l)`（`color.rs:16`）、`Colorize` trait（`:21`）、
`ColorName` 枚举（`:365`）、`try_parse_color(&str)`（`:706`）、
`try_parse_background(&str)`（`:776`）。

## 最佳实践

- **先 `theme::init(cx)`**：应用启动时调用，否则窗口可能全透明（`Theme::default`
  用内置亮色主题兜底，但仍应显式初始化）。
- **读色统一走 `cx.theme()`**：不要硬编码颜色，组件才能随主题/系统外观切换。
- **自定义主题用 JSON + 注册表**：放 `./themes/*.json`，`watch_themes_dir` 热重载。
- **注意 `ThemeMode` 二义**：框架主题用根 `ThemeMode`（两值），系统跟随用
  `theme_watcher::ThemeMode`（三值）。
