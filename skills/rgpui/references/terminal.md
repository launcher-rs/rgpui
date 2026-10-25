# 终端（rgpui-term）

**目录：** [概述](#概述) · [两种字节来源](#两种字节来源) · [内容搜索](#内容搜索) · [OSC 8 超链接](#osc-8-超链接) · [IME 组合输入](#ime-组合输入) · [其余能力](#其余能力) · [渲染接线：element vs view](#渲染接线element-vs-view) · [示例](#示例) · [平台与依赖](#平台与依赖)

## 概述

`rgpui-term` 是**独立库 crate**（`crates/rgpui-term/`），把 Alacritty 仿真内核
（`alacritty_terminal`）接到 rgpui 的渲染与事件循环。`lib.rs` 重导出：
`Terminal`/`TerminalBuilder`/`TerminalView`/`TerminalElement`/`Event`/`SearchStatus`/
`IndexedCell`/`TerminalContent`/`TerminalBounds`/`ZedListener`、外接后端
`ExternalHandles`/`FeedHandle`/`PtyBoundMessage`/`PtyBoundReceiver`、以及
`Block`/`BlockDetection`、`InputOrigin`/`TerminalMiddleware`、`TextStyle`/`convert_color`、
`TerminalConfig`/`TerminalTheme`/`TerminalThemeConfig`/`hsla_from_rgb`、
`ThemeManager`/`ThemeDefinition`、各类 action（`Copy`/`Paste`/`Clear`/`SelectAll`/
`Scroll*`/`SearchNext`/`SearchPrevious`/`ClearSearch`/`ChangeTheme`）。
无 feature 门控（`Cargo.toml` 无 `[features]`）。

## 两种字节来源

字节来源与去向都抽象成一个 `PtyBackend`（`terminal.rs:63`，`Local`/`External` 两态），
模拟内核完全一致，仅差别在通道。

- **本地 PTY**：`TerminalBuilder::new(working_directory, shell, env, max_scroll_history_lines, window_id, cx)`（`terminal.rs:449`）返回 `Task<Result<TerminalBuilder>>`，内部拉起 shell 进程 + 读写线程。
- **外接 IO**：`TerminalBuilder::new_external(max_scroll_history_lines)`（`terminal.rs:591`）无进程，直接返回 `Result<TerminalBuilder>`；再用 `take_external_handles()`（`terminal.rs:650`）取走 `Option<ExternalHandles>`。

两条路径都以 `cx.new(|cx| builder.subscribe(cx))`（`subscribe` `terminal.rs:658`）汇入
RGPUI 事件循环。`ExternalHandles`（`external.rs:105`）公开两个字段，各由一侧持有：

| 方向 | 句柄 | 载体 |
|------|------|------|
| 下行（远端 → 终端） | `ExternalHandles::feed: FeedHandle` | `FeedHandle::feed(bytes: impl Into<Vec<u8>>) -> bool`（`external.rs:73`），返回 `false` 表示会话已结束 |
| 上行（终端 → 远端） | `ExternalHandles::rx: PtyBoundReceiver` | `recv()`/`try_recv()`（`external.rs:92`/`:99`）取 `PtyBoundMessage` |

`PtyBoundMessage`（`external.rs:46`）：`Input(Vec<u8>)`（键入、鼠标报告、OSC 应答、
焦点上报）与 `Resize(WindowSize)`（行列数 + 单元格像素，远端据此 resize 其 PTY）。
会话结束由传输层调 `FeedHandle::end_session()`（`external.rs:79`），UI 侧收
`Event::CloseTerminal`，表现与本地进程退出一致。

```rust
let mut builder = TerminalBuilder::new_external(Some(2000))?;
let handles = builder.take_external_handles().expect("外联终端必有句柄");
let terminal = cx.new(|cx| builder.subscribe(cx));
// 传输层线程：远端字节 → handles.feed.feed(..)；handles.rx.recv() → 写 SSH channel
```

## 内容搜索

搜索是排队执行的（`search*` 只入队，实际编译/定位在 `Terminal::sync` 期间完成），
状态经 `Event::SearchUpdated(SearchStatus)`（`terminal.rs:112`）回传。

| `Terminal` 方法（`terminal.rs`） | 说明 |
|------|------|
| `search(query)` `:1023` | 按**正则**设关键词；全小写忽略大小写；空串清除 |
| `search_literal(query)` `:1034` | 按**字面量**（先 `escape_literal` 转义元字符，无需额外 `regex` 依赖） |
| `search_next()` / `search_previous()` `:1042` / `:1048` | 覆盖全部滚动历史、自动回绕 |
| `clear_search()` `:1054` | 清除高亮与状态 |
| `search_status() -> SearchStatus` `:1062` | `Idle`/`Invalid`/`NotFound`/`Matched`（`terminal.rs:122`） |
| `search_query()` / `search_is_literal()` `:1072` / `:1077` | 回显当前关键词 / 是否字面量 |
| `search_current() -> Option<SelectionRange>` `:1082` | 当前匹配的网格坐标 |

`TerminalView` 侧透传 `search`/`search_literal`/`search_status`
（`terminal_view.rs:495`/`:504`/`:514`），并绑定 action `SearchNext`/`SearchPrevious`/
`ClearSearch`（`terminal_view.rs:34` 的 `actions!`）。搜索条这类直接吃用户输入的入口
应用 `search_literal`，避免 `.` `(` `[` 被当成正则。

## OSC 8 超链接

`Terminal::hyperlink_at(point: AlacPoint) -> Option<String>`（`terminal.rs:1137`）读单元格
携带的 URI。触发在 `mouse_down`（`terminal.rs:1177`）：**`MouseButton::Left` + （`control` 或
`platform`）修饰键**，且不在鼠标上报模式（`mouse_mode` `terminal.rs:1132`）时，命中即发
`Event::HyperlinkClick(uri)`（`terminal.rs:110`），跳过本击的选择逻辑；打开方式由应用侧决定。
渲染侧在 `terminal_element.rs` 的 `cell_style`（`:700`）给链接单元格着色 + 强制下划线：
颜色取主题蓝 `theme.ansi[4]`（`:710`），色调优先于 `DIM`/`BOLD` 视觉。

## IME 组合输入

组合文本存 `Terminal::ime_composition`（`terminal.rs:748`）， setter/getter
`set_ime_composition(Option<String>)`/`ime_composition()`（`:854`/`:859`）。
组合期间 `TerminalView::on_key_down`（`terminal_view.rs:184`）检测到
`ime_composition().is_some()` 直接 return —— 按键归输入法，不直送 PTY。
上屏由 `terminal_element.rs` 的私有 `TerminalInputHandler`（`:1885`，
`impl InputHandler` `:1890`）按 **UTF-16 区间**应答：`marked_text_range`（`:1915`，
组合文本永远是一段前缀 `0..len_utf16`）、`text_for_range`（`:1928`，经 `utf16_slice`
`:2008` 截取并回填 `actual_range`，代理对/越界安全）、`replace_text_in_range`（`:1941`，
清预览后 `input_text` 上屏给 PTY）。组合期间元素层在光标处画下划线预览
（`LayoutState::composition`，`terminal_element.rs:1574`）。

## 其余能力

- **代码块**（`block.rs`）：`Block`（`:21`，`{id, command, output, timestamp, exit_code, detection}`）、
  `BlockDetection`（`:14`，`ShellIntegration`/`Heuristic`）；`BlockTracker`（`:42`）在 Enter 与
  下一个提示符之间分块。`Terminal` 侧 `blocks()`/`current_block()`/`block_running()`（`terminal.rs:1097`/`:1102`/`:1107`）。
- **中间件**（`middleware.rs`）：`TerminalMiddleware` trait（`:20`，`on_input`/`on_event`/`on_output`/`on_raw_pty_output`）、
  `InputOrigin`（`:7`，`Keystroke`/`Text`/`Paste`/`Mouse`/`Scroll`/`Focus`/`Clipboard`/`System`/`Programmatic`）；
  注册走 `Terminal::add_middleware(Arc<dyn TerminalMiddleware>)`（`terminal.rs:757`），`on_input` 返回 `None` 丢弃输入。
- **配置/主题**：`TerminalConfig`（`config.rs:229`，`config_path`/`load_or_create`/`save`）、
  `TerminalTheme`（`config.rs:11`，`resolve_color`）、`TerminalThemeConfig`（`:166`，`to_theme`）、
  `hsla_from_rgb`（`:354`）；`ThemeManager`（`theme_manager.rs:43`，`Global`，`new`/`get_theme`/
  `set_theme`/`available_themes`/`add_theme`/`current_theme`）与 `ThemeDefinition`（`:33`，`{name, theme}`）；
  `TerminalView::set_theme(name, cx)`（`terminal_view.rs:142`）。
- **内容快照**：`TerminalContent`（`terminal.rs:300`，含 `cells`/`cursor`/`selection`/`mode`/
  `display_offset`/`history_size`/`search_match`/`terminal_bounds`）、`IndexedCell`（`:280`，
  `{point, cell}`，`Deref` 到 `Cell`）、`TerminalBounds`（`:194`，像素↔网格坐标换算）。

## 渲染接线：element vs view

- **`TerminalElement`**（`terminal_element.rs:463`，低层 `Element`）：`new(terminal, focus, focused, cursor_visible, text_style, show_scrollbar)`（`:482`）；只做布局/绘制（视口裁剪 + 批量文本运行），自行接事件。直接嵌进 `div` 时用。
- **`TerminalView`**（`terminal_view.rs:59`，`Entity` 组件，`impl Focusable + Render`）：`new(terminal, window, cx)`（`:74`）或 `new_with_style(..., text_style, ...)`（`:79`），内部订阅事件、管焦点、处理键鼠/滚动条/剪贴板 action，并转发给 `TerminalElement`。**需要键盘输入/选区/搜索/主题切换的完整终端就选它。**
- 文本样式统一走 `TextStyle`（`terminal_element.rs:768`，`Default` 或 `from_config(&TerminalConfig)` `:811`）。

## 示例

`examples/rgpui_term_basic/`（`publish = false`，两个 bin）：

```text
cargo run -p rgpui_term_basic --bin basic         # 本地 PTY：多标签 + 搜索条 + OSC 8 + IME + 中间件
cargo run -p rgpui_term_basic --bin external_io   # 外接 IO：无本地 PTY，模拟远端双向通道
```

`examples/README.md`（`:135`）对该 crate 的描述（`--bin basic` 多标签/搜索条/OSC 8/IME、
`--bin external_io` 外接 IO 后端）与代码一致 —— `basic` 的 `main.rs` 确实经
`TerminalBuilder::new`（`main.rs:619`/`:1166`）拉起真实 shell。

## 平台与依赖

- 本地 PTY 依赖 `alacritty_terminal::tty`（跨平台，`terminal.rs` 内 `#[cfg(windows)]`
  仅两处：工作目录缺失时回退 `USERPROFILE`（`:485`）、`tty::Options.escape_args = false`
  （`:507`）；`drain_on_exit` 等其余选项各平台一致）。
- `new_external` 路径**不产生任何进程、无 OS 轮询**（`external.rs:19`），三平台行为一致，
  适合受限/无 PTY 环境或测试注入。
