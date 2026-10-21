# AGENTS.md - rgpui 开发指南

## 项目概述

rgpui 是一个**独立演进**的 GPU 加速跨平台 UI 框架（历史合并 Zed 的 `gpui` +
`gpui-component`，现已与上游切分，不再跟随）。Rust workspace 架构，核心库
`rgpui` 通过 `Platform` trait 抽象各平台实现。

## 本机约定（Windows + PowerShell 7）

opencode 等工具按 UTF-8 解码 pwsh 输出，而本机控制台默认 GB2312 —— 任何含中文
输出的命令都先执行这四行：

```powershell
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::InputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$PSStyle.OutputRendering = 'PlainText'
```

- 文本结果优先用专用工具（Read / Glob / Grep，不受代码页影响）。
- 程序化解析命令输出时尽量用英文输出或 `--no-ansi`。
- 读中文文件加 `-Encoding UTF8`。

## 架构与包边界

```
rgpui（核心：框架 + 基础组件 + 扩展组件/动画/手势/滚动物理，charts/effects/qr-code/tokio/webview feature 门控）
  ├── rgpui-markdown（独立 Markdown 库）└── rgpui-term / rgpui-3d（专业集成库）

crates/                 # 12 个库 crate（rgpui / rgpui-{3d,character,linux,macos,macros,markdown,platform,term,web,wgpu,windows}）
examples/               # 17 个示例 crate（同根 workspace；showcase 为核心示例集合，一个功能一个 bin，见 examples/README.md）
```

- `rgpui/src/platform.rs` 定义 `Platform` / `PlatformWindow` trait；示例经
  `rgpui_platform::application()` 启动；平台特有代码在对应 `rgpui-<platform>/`。
- Tokio 已并入核心（`rgpui::tokio`，feature 门控），WebView 同理（`rgpui::webview`）。

## 开发命令

```bash
cargo check --workspace            # 日常检查（比 build 快）
cargo run -p <pkg>                 # 运行示例；showcase 加 --bin，如 -p image_showcase --bin image
cargo test --workspace             # 所有测试
cargo test -p rgpui                # 单包测试
cargo clippy -p <pkg> --all-targets -- -D warnings   # 提交前对改动包执行
cargo fmt --all                    # 提交前格式化
```

## Clippy 规则

workspace 级 `deny`：`dbg_macro`、`todo`、`declare_interior_mutable_const`、
`redundant_clone`、`disallowed_methods`；`style` 为 `allow`。

根目录 `clippy.toml` 允许测试代码用 `unwrap` / `expect` / `panic!`（断言失败就是要炸）。
生产代码按模块收紧：`rgpui.rs` 里 24 个套件模块声明带
`#[deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`，
新增代码在这些模块里用 `unwrap` 会直接编译失败；其余历史模块按同一口径逐步迁移。

## 托盘与窗口隐藏

- 类型：`TrayMenuItem` / `TrayIconEvent`（`rgpui::tray`）；App 方法：
  `set_tray_icon/menu/tooltip`、`on_tray_menu_action`、`on_tray_icon_event`。
- Windows 实现：`rgpui-windows/src/tray.rs`（`Shell_NotifyIconW`）+
  `platform.rs`（`WM_GPUI_TRAY_ICON` / `WM_COMMAND`）。
- macOS 实现：`rgpui-macos/src/tray.rs`（`NSStatusItem` + `NSMenu`，图标按 18×18 点）；
  菜单挂在状态栏按钮的 action 上而不是 `NSStatusItem.setMenu:`，这样点击事件与菜单
  能同时工作；只设菜单没设图标时用 SF Symbol 齿轮兜底。
- Linux 实现：`rgpui-linux/src/linux/tray.rs` + `tray_sni.rs`（StatusNotifierItem，
  X11 / Wayland 共用）。
- `hide_window()`（任务栏移除）vs `minimize_window()`（保留任务栏图标）；
  托盘恢复用 `activate_window()`。图标接受 PNG/ICO 字节（macOS 只认 NSImage 支持的
  格式，ICO 会记警告并留空）。
- Linux 的应用级 `Platform::activate/hide`：X11 发 EWMH `_NET_ACTIVE_WINDOW`
  （先 map 回被 `hide()` 撤下的窗口，否则窗管会忽略），Wayland 走 xdg-activation
  （合成器可拒绝）；`hide()` 在 X11 是 unmap、Wayland 是最小化。`hide_other_apps` /
  `unhide_other_apps` 在 Linux 没有对应协议能力，保留空实现。

## 提交与 PR 规范

- 提交前：`cargo check --workspace` + `cargo fmt --all` + 相关包 clippy，无错误警告。
- **禁止 `#[allow(dead_code)]`**，无用代码删除或重构。平台专属字段仅在部分目标上使用
  时，用 `#[cfg_attr(<目标>, allow(dead_code))]` 精确限定（只对不编译其用途的目标生效），
  不得整块放宽。
- 开发阶段废弃代码直接删除，不留 `#[deprecated]` 兼容（不为未发布的 API 做兼容）。
- **合入 main 的改动走 PR**（分支命名 `feat|fix|refactor|chore/xxx`，Squash 合并，
  标题 Conventional Commits）。`main` **未开** GitHub 分支保护（单人维护、主动去掉），
  所以规则靠自觉：**必须等 CI 全绿再合**。注意无保护时 `gh pr merge --squash --auto`
  不会等待检查、会立刻合并，别用 `--auto`。
  版本开发分支（如 `feat/1.2.0`）上直接提交推送，**不为合入版本分支开中间 PR**；
  版本分支完成后再开一个 PR 合入 main。
- `gh pr create / merge --squash`；CI 全绿再合并；等 CI 时不要反复 push 刷运行。
- CI（`.github/workflows/ci.yml`）：三平台矩阵，跑 fmt（逐库）+ clippy
 （`--workspace --lib --bins -D warnings`）+ check + test；同分支 push/PR 共用
  concurrency 组，后触发自动取消先触发的。

## 跨平台注意

- Windows 的 `cargo check` 不编译 macOS/Linux `cfg` 代码，跨平台改动靠 CI 矩阵验证。
- `cargo hack check --each-feature` 可用；`scap` / `screen-capture` feature 已可以启用
  （旧的「与 `windows-capture` API 不兼容」结论已过期），Windows 本地已验证编译，
  跨平台由 CI 矩阵兜底。

### Linux 端构建前置

`examples/webview` 启用 `rgpui/webview` → `wry` → `webkit2gtk` → `gtk` → `glib-sys`，
缺系统库时 `cargo check --workspace` 会卡在 `glib-sys` 的 build script
（`The system library 'glib-2.0' ... was not found`），连带 CI 用的
`cargo clippy --workspace --lib --bins` 一起失败。先装齐：

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libglib2.0-dev
```

装不了（无授权/无网络）时，Linux 端的实际可验证范围是
`cargo check/clippy -p rgpui-linux --all-targets -- -D warnings` + `cargo fmt -p rgpui-linux`，
**这不能替代 `cargo check --workspace`**，跨 crate 影响要靠 CI 矩阵兜住。
`x11rb` 是纯 Rust，不需要 `libx11-dev`。诊断日志用 `rgpui::init_logging()` + `RUST_LOG`。

## rgpui 独有功能（重构不得移除）

### 组件库索引

| 子系统 | 模块 | 关键类型 |
|--------|------|----------|
| 滚动 | `elements/scroll/` | `Scrollable`、`Scrollbar`、`ScrollHandle` |
| 基础元素 | `elements/` | `Button`、`Checkbox`、`Radio`、`Switch`、`Slider`、`Spinner`、`Skeleton`、`Badge`、`Tag`、`Separator`、`Kbd`、`Tooltip`、`Icon`、`Avatar`、`Alert`、`Breadcrumb`、`Card`、`Typography`、`Toggle` |
| 表单 | `form/` | `Form`、`Field`、`FieldBuilder` |
| 输入 | `input_ui/` | `Input`、`MaskedInput`、`NumberInput`、`PasswordInput`、`TextArea`；`InputState::reveal_offset/reveal_range`、`TextDecorationCollection`、`Debouncer`（见 `docs/1.2.0/1.2.0-dev-plan.md` §E） |
| 编辑器 | `input_ui/editor/`（`editor` feature 门控，蕴含 `lsp`） | `EditorState`（包 `Entity<InputState>` 编排外壳 + 大纲缓存；透传 `set_value`/`reveal_*`/`set_read_only`/`set_line_comment_prefix`/`set_highlighter`/`goto_symbol`）、`Editor`（消费 `EditorState` 实体 + 粘性大纲顶栏 + 状态行）；行操作/多光标/自动闭合/括号匹配/当前行（`impl InputState`）；LSP 接线（`lsp_attach.rs`：provider 注入 + 防抖 + 诊断下划线）+ 片段（`snippets.rs`：子集解析 + 会话跳转）+ 行内提示（`inlay_hints.rs`：provider + 防抖 + paint overlay）+ 粘性顶栏（`sticky_scroll.rs`）+ 多语言注册表（`highlight::{register_highlighter,highlighter_for}` + `set_language`）+ 自定义 gutter（`gutter.rs`：具名 provider 注册/开关 + overlay，见 `docs/1.2.0/1.2.0-dev-plan.md` §M）；stdio LSP 传输（`lsp/stdio.rs`：子进程 + JSON-RPC）、Minimap（`minimap.rs`：缩略+跳转）、Vim（`vim.rs`：三态）、CodeLens（`codelens.rs`：行上透镜）、标尺+括号彩虹（`rulers`/`bracket_rainbow`）、JSON+TOML grammar（`tree_sitter.rs`）、extensions+on_edit 表（`extensions.rs`） |
| 菜单 | `menu/` | `PopupMenu`、`ContextMenu`、`DropdownMenu`、`Popconfirm`、`MenuBar`、`HoverCard`、`Notification`、`Toast` |
| 对话框 | `dialog/` | `Dialog`、`AlertDialog`、`DialogHeader/Content/Footer`、`FocusTrapElement` |
| 列表/表格/标签页 | `list/`、`table/`、`tabs/` | `List`、`VirtualList`、`DataTable`、`TabBar`、`Accordion` |
| 标题栏/扩展 | `title_bar/`、`components/` | `TitleBar`、动画 13 组件、`SplitPane`、`CommandPalette`、`ImageViewer`、`SearchPanelState`、`Sidebar`/`SidebarSection`、`Select`、`Combobox`、`DatePicker`、`ColorPicker`、`Pagination`、`Steps`、`Timeline`、`Rate`、`Upload`、`Carousel`、`MermaidDiagram`、`DockArea` 等（`charts`/`effects`/`qr-code` feature 门控） |
| 动画/手势/物理 | `animation/`、`mouse_gestures.rs`、`scroll_physics.rs` | `Spring`、`GestureDetector`、`ScrollPhysics` |
| 对话（AI-chat） | `chat/` | `Message`/`MessageGroup`/`ChatView`、`Prompts`、`Suggestion`、`ThoughtChain`、`Attachments`/`FileCard`、`Sources`、`Actions`、`Sender`、`Bubble`、`MessageScroller`、`Marker` |
| 剪贴板（核心，无 feature 门控） | `platform.rs`（`Platform::read/write_to_clipboard`，各平台 `clipboard.rs`/`pasteboard.rs`） | `ClipboardItem`/`ClipboardEntry`/`ClipboardString`/`ClipboardFormat`；富负载条目 `ClipboardEntry::Format`（HTML/RTF/CSV/自定义 MIME，MIME 常量见 `ClipboardFormat::{HTML,RTF,PLAIN_TEXT,CSV,TSV,MARKDOWN,URI_LIST}`），构造器 `new_html`/`new_rtf`/`new_with_format`/`with_string`/`with_format`，读取 `text`/`html`/`rtf`/`format`/`formats` |
| 富文本（核心，无 feature 门控） | `rich_text.rs` | 文档模型 `RichTextDoc`/`Paragraph`/`Run`/`RunProps`/`ParagraphStyle`/`ListKind`；字符坐标编辑 `insert_text`/`delete`/`split_paragraph`/`merge_paragraph`/`apply_props`/`normalize`/`locate`；纯文本 `plain_text`/`from_plain_text`，HTML 双向 `to_html`/`from_html`（含列表/对齐/颜色）；只读渲染元素 `RichText`（映射到 `StyledText`/`TextRun`） |
| Ink 笔迹（核心，无 feature 门控） | `ink.rs` | `InkLayer`/`Stroke`/`InkPoint`/`InkStyle`/`InkTool`（Pen/Highlighter/Eraser/Lasso）；`begin_stroke`/`begin_stroke_with_pressure`/`extend_stroke`/`extend_stroke_with_pressure`/`end_stroke`/`cancel_stroke`/`undo`/`clear`、`erase_at`/`hit_test`/`lasso`/`remove`/`simplify`/`bounds`/`semantics`；多图层 `InkDocument`/`InkDocumentLayer`（`layers`/`layer`/`visible_layers`/`active`/`set_active`/`add_layer`/`remove_layer`/`move_layer`/`set_visible`/`set_locked`/`set_opacity`/`is_editable`/`fade`，编辑落在激活层）；几何 `point_segment_distance`/`point_in_polygon`/`polygon_bounds`/`simplify_points`（Douglas–Peucker）/`stroke_path`/`pressure_width`/`stroke_paths`（逐段压感变宽）；绘制元素 `InkLayerElement`（`.highlight`/`.lasso`/`.opacity`/`.a11y`，笔画经画布语义暴露给辅助技术） |
| 缩略图排序器（核心，无 feature 门控） | `sorter.rs` | `Sorter`/`SorterState`/`SorterDelegate`/`SorterEvent`；`select`/`toggle`/`select_range`/`select_all`/`clear_selection`、`begin_drag`/`set_drop_slot`/`update_drag`/`drag_autoscroll`/`finish_drag`/`cancel_drag`、`set_zoom`/`zoom_by`/`scroll_to_item`；几何 `cell_size`/`grid_columns`/`drop_slot`/`move_dest`/`move_group_dest`/`autoscroll_velocity`/`clamp_zoom`/`normalize_aspect_ratio` |
| 动画时间线（核心，无 feature 门控） | `anim_timeline.rs` | 模型 `Timeline`/`Track`/`Keyframe`/`Interpolation`（`Linear`/`Hold`/`Smooth`）；`normalize`/`set_keyframe`/`set_keyframe_value`/`remove_keyframe`/`keyframe_index_at`/`move_keyframe`/`duration`/`value_at`/`value_range`/`to_keyframe_animation`、`Track` 的 `collapsed`/`row_height`/`with_collapsed`/`set_collapsed`、`Timeline` 的 `add_track`/`remove_track`/`move_track`/`duration`/`values_at`；控件 `TimelineState`/`TimelineView`/`TimelineEvent`/`TimelineHit`（`set_time`/`advance`/`play`/`pause`/`toggle_play`/`set_looping`、`set_px_per_second`/`zoom_by`/`set_scroll`/`vscroll`/`set_vscroll`/`max_vscroll`/`scroll_to_track`/`tracks_height`/`track_top`/`set_fps`/`snapped`、`select_keyframe`/`remove_selected_keyframe`/`move_track`/`toggle_track_collapsed`/`hit_test`）；几何 `time_to_x`/`x_to_time`/`snap_time`/`ruler_step`/`format_time`/`clamp_px_per_second`/`value_to_y`/`y_to_value`/`curve_points`（值曲线，关键帧水平改时间、垂直改值，`KeyframeMoved` 带 `value`） |
| 色板（核心，无 feature 门控） | `components/color_palette.rs` | `ColorPalette`（`new`/`colors`/`selected`/`swatch_size`/`gap`/`on_select`/`on_select_arc`）；`theme_palette`（主题语义色）/`color_name_palette`（命名色阶）/`push_recent`（最近颜色去重置顶 + 截断）；`ColorPickerState` 增强：`show_palette`/`recent_limit`/`recents`/`push_recent`/`clear_recents`/`set_color`/`begin_eyedropper`（吸管） |
| 屏幕取色（`screen-capture` feature 门控） | `eyedropper.rs` | `EyedropperResult`（`Picked`/`Cancelled`/`Failed`）、`open_eyedropper`（推流取首帧全屏冻结 + 十字线 + 放大镜，`ColorPickerState::begin_eyedropper` 接线） |
| 多文档界面 MDI（核心，无 feature 门控） | `mdi.rs` | `MdiState`/`MdiArea`/`MdiEvent`/`MdiWindow`/`MdiFrame`/`MdiTab`/`MdiHit`/`MdiLayout`/`MdiWindowFlags`/`WindowState`/`WindowButton`/`ResizeEdge`；`open`/`open_with`/`close`/`close_active`/`close_all`/`activate`/`cycle`/`minimize`/`maximize`/`restore`/`toggle_maximize`/`move_by`/`snap_active`/`set_bounds`/`arrange`/`hit_test`/`is_tabbed`/`tabs`/`tab_bar_rect`/`tabbed_content_rect`；几何 `cascade_rects`/`tile_rects`/`tab_rects`/`tab_at`/`clamp_rect`/`snap_rect`/`icon_rect`/`title_button_rects`/`resize_edge_at`/`resize_edge_rects`/`resize_bounds`（`with_snap` 打开后 `on_pointer_up` 自动吸附；`MdiLayout::Tabbed` 页签式只渲染激活窗口） |
| 任务窗格（核心，无 feature 门控） | `task_pane.rs` | `TaskPaneState`/`TaskPane`/`TaskPaneEvent`/`TaskPaneHit`/`TaskPaneSide`；`show`/`hide`/`toggle`/`set_visible`/`set_side`/`set_width`/`set_closable`/`with_tabs`/`set_tabs`/`tabs`/`active_tab`/`active_tab_title`/`set_active_tab`/`cycle_tab`/`tab_rects`/`tab_at`/`float`/`dock`/`toggle_float`/`set_rect`/`set_auto_hide`/`set_collapsed`/`toggle_collapsed`/`set_hovered`/`effective_width`/`hit_test`/`on_pointer_*`/`on_key_down`（`Esc` 取消拖拽、`Ctrl+Tab` 切页签）；自动隐藏收起为 `RAIL_WIDTH` 竖排标题轨道、悬停展开；页签组（激活页签高亮下划线，`TaskPaneEvent::TabSelected`）；几何 `clamp_width`/`resize_width`/`grip_rect`/`close_button_rect`/`tab_rects`/`floating_default_bounds`（浮动态复用 `mdi` 的 `ResizeEdge`/`resize_bounds`/`resize_edge_rects`）；多窗格堆叠 `TaskPaneStack`/`TaskPaneStackState`（`with_weights`/`weights`/`weight`/`set_weights`/`set_weight`/`begin_drag`/`update_drag`/`end_drag`/`usable_height`、`resize_stack_weights`/`normalize_stack_weight`、`STACK_SEPARATOR_HEIGHT`/`MIN_STACK_WEIGHT`） |
| 画布语义（核心，无 feature 门控） | `canvas_a11y.rs`（+ `elements/canvas.rs` 接线） | `CanvasNodeId`/`CanvasNode`/`CanvasSemantics`/`CanvasActionHandler`；`CanvasNode::{new,with_value,with_description,with_bounds,disabled,selected,focused,with_action,with_actions,set_*}`、`CanvasSemantics::{new,push,extend,clear,get,get_mut,index_of,hit_test,reading_order,first_in_reading_order,next_in_reading_order,window_bounds,with_row_tolerance,fill_empty_bounds}`；几何 `reading_order_indices`/`node_at_point`/`next_in_order`/`window_bounds`；无障碍桥 `accesskit_node`/`push_canvas_a11y`/`register_canvas_a11y_actions`（框架侧 `A11ySubtreeBuilder::{push_child_with_bounds,scale_factor}`、`synthetic_child_node_id`）；元素 `Canvas::{id,a11y,a11y_label,on_canvas_a11y_action}`（零尺寸语义节点发布时按画布大小填充）；接线组件 `InkLayerElement::a11y`、`Sparkline::a11y`、`Waveform::a11y`、`PieChart::a11y`、`Gauge::a11y`、`RadarChart::a11y`、`TreeMap::a11y`、`LineChart::a11y`、`AreaChart::a11y`、`Chart::a11y`（后七个随 `charts` feature，整体摘要节点） |
| 双向文字 bidi（核心，无 feature 门控） | `bidi.rs`（+ `style.rs` 的 `TextAlign`、`text_system/line.rs` 绘制入口、`rgpui-dom` CSS 映射） | `TextDirection`（`Ltr`/`Rtl`）、`TextDirection::{resolve,is_rtl,for_locale}`（`resolve` 为 HTML `dir=auto` 口径：首个强方向字符，无强方向字符回退 LTR；`for_locale` 按语言标签给基准方向）、`primary_language`/`is_rtl_locale`（CLDR 常用 RTL 语言、主语言子标签、大小写不敏感）、`strong_direction`/`is_rtl_char`（UBA `L`/`R`/`AL`，基于 `unicode-bidi`）；`TextAlign::{Start,End}` + `TextAlign::resolve(TextDirection)`（按段落自身方向在绘制入口解析；`rich_text`/DOM 后端分别透传 `text-align:start\|end`） |
| 东亚断行禁则（核心，无 feature 门控） | `line_break.rs`（+ `text_system/line_wrapper.rs`、`text_system/line_layout.rs` 接线） | `BreakClass`/`break_class`、`is_ideographic`、`is_line_start_prohibited`（行首禁则）/`is_line_end_prohibited`（行末禁则）/`is_no_break`（胶连字符）/`can_break_between`；两条换行路径在候选断点处按禁则否决（不改半角 ASCII 标点行为） |
| 应用套件基座（`ribbon` / `viewport` / `spreadsheet` / `document` / `control-channel` feature；`commands.rs` 为核心，其余默认关闭） | `commands.rs`、`ribbon.rs`、`keytips.rs`、`qat.rs`、`mini_toolbar.rs`、`backstage.rs`、`viewport.rs`、`guides.rs`、`formula_bar.rs`、`sheet_grid.rs`、`page_view.rs`、`object_toolkit.rs`、`control_channel.rs`、`mcp.rs` | `CommandDescriptor`/`CommandRegistry`/`register_command!`/`App::dispatch_command`/`label_for`/`CommandRegistry::validate`、`Ribbon`/`RibbonState`/`RibbonTab`/`RibbonGroup`/`RibbonItem`/`RibbonSplitButton`/`RibbonGallery`/`RibbonComboBox`、`KeyTip`/`KeyTipScope`/`resolve_key`、`QuickAccessToolbar`/`QatItem`/`QatButton`、`MiniToolbar`、`Backstage`/`BackstageState`/`BackstageTab`、`Viewport`、`ruler_ticks`/`align`、`FormulaBar`/`FormulaBarState`/`FormulaBarEvent`（`spreadsheet`）、`SheetGrid`/`SheetGridState`/`SheetGridEvent`/`SheetDelegate`/`CellRef`/`CellRange`/`column_label`（`spreadsheet`）、`PageView`/`PageLayout`/`PageSetup`/`PageSize`/`PageMargins`/`PageArrangement`/`PageLabels`（`document`）、`ResizeHandle`/`SelectionHandles`/`AlignMode`/`handle_point`/`handle_point_rotated`/`hit_handle`/`hit_handle_rotated`/`marquee_bounds`/`marquee_hits`/`MarqueeMode`/`resize_bounds`/`rotate_point`/`angle_between`/`union_bounds`/`align`/`distribute`（`viewport`）、`ControlChannel`/`ControlChannelOptions`/`App::start_control_channel`/`McpServer`/`tool_name`（`control-channel`） |
| 国际化 i18n（核心，无 feature 门控） | `i18n/mod.rs`（+ `bidi.rs` 的语言方向查表） | `I18nManager`（`new`/`with_fallback`/`load_translations`/`load_translations_map`/`load_locale_dir`/`t`/`set_locale`/`available_locales`/`snapshot`/`restore`）、`Locale`/`Translation`/`I18nSnapshot`；`direction`/`is_rtl`（语言标签 → 基准方向，走 `TextDirection::for_locale`）、`validate`（目录审计：空目录 / 当前或回退语言未加载 / 逐语言缺 key）；`bidi::primary_language`/`is_rtl_locale` |
| 终端（`rgpui-term` 独立 crate） | `terminal.rs`、`terminal_element.rs`、`terminal_view.rs` | `Terminal`/`TerminalView`；IME 中文/日文输入：组合文本存 `Terminal::ime_composition`（`set_ime_composition`/`ime_composition`），`TerminalInputHandler` 按 UTF-16 区间（`utf16_slice`）应答 `marked_text_range`/`text_for_range`，组合期间在光标处画下划线预览（`LayoutState::composition`），`replace_text_in_range` 上屏到 PTY；组合期间 `on_key_down` 不直送终端 |
| Markdown 渲染（`rgpui-markdown` 独立 crate；公式 / 图表随 feature 门控） | `crates/rgpui-markdown/src/{markdown,rich_text,code_block,latex,mermaid,svg_image}.rs` | `Markdown`（解析 + 渲染）、`RichBlock`/`RichInline`（块模型，含 `Math` 变体）、`render_blocks`/`render_blocks_with`/`render_blocks_with_svg`、`CodeBlock`；公式管线 `render_latex_svg`/`latex_image`/`LatexImage`（`latex` feature：ratex 解析 → 排版 → SVG（字形转路径）→ usvg 光栅化；`$...$` 行内公式 / `$$...$$` 展示公式，失败降级为源码文本）；图表管线 `render_mermaid_svg`/`mermaid_image`/`is_mermaid_language`（`mermaid` feature：`mermaid-rs-renderer` → SVG → usvg；` ```mermaid ` 围栏自动成图，配色跟随主题深浅，失败降级为代码块） |

`prelude` 含：`ActiveTheme`、`ElementExt`、`InteractiveElementExt`、`Selectable`、
`Sizable`、`StyledExt`、`FluentBuilder`。

### 平台 trait 自有 API（platform.rs）

`PlatformWindow`：`hide`、`set_mouse_passthrough`、`set_position`、
`set_titlebar_visible`、`set_input_region`、`request_attention`、
`get_raw_handle`（仅 Windows，后端 `open_window` 登记 HWND 用）。
`Platform` 约 35+ 自有方法：托盘 8 件套、全局热键、通知、电源、权限（统一走
`check_permission` / `request_permission`，类别见 `PermissionType`）、
网络状态、系统信息、Dock 菜单、上下文菜单、`reduce_motion_enabled`（系统「减少
动态效果」，四平台各自读取，核心层每次建窗同步一次）；
另 `WindowOptions.mouse_passthrough`、`WindowKind::Overlay`、Mica 材质、
`tray.rs` / `single_instance/mod.rs`。

**加 `Platform` 方法必须同时加 `App` 包装 + 一个真实调用点**（示例或核心逻辑）。
`App::platform` 是私有字段，没有 `App` 包装的能力应用层一行都调不到 ——
`set_keep_alive_without_windows`（存了没人读）与 `os_info`/权限查询（实现了没人调）
两起缺陷都是这一条没做到导致的。权限类能力统一走
`App::check_permission(PermissionType)` / `request_permission(kind)`，不要再为单个类别加特例方法。

### rgpui-windows 特有

托盘（`tray.rs`）、NCHITTEST 鼠标穿透（`events.rs`）、Mica（`window.rs`）、
自启动（`auto_launch.rs`）、焦点窗口查询、9 个 `WM_USER` 自定义消息。
DComp 以 `CreateTargetForHwnd(hwnd, false)` 创建，子 HWND（如 WebView2）显示在
DComp 内容之上——改回 `true` 会遮挡 WebView。

### 完整性检查清单（重构/提交前）

1. `cargo check --workspace` 通过
2. `mouse_passthrough`（`WindowOptions`/`WindowParams`）、`WindowKind::Overlay`、
   Mica 变体、`tray.rs` / `single_instance/mod.rs` 都在
3. `Platform` / `PlatformWindow` 自有方法都在（对照上表）
4. `tray`、`desktop_pet`（`_3d`）示例可编译
5. 中文注释未被删除；无 `#[allow(dead_code)]`
6. 组件子系统目录都在（`form input_ui menu dialog list table tabs title_bar elements/scroll`）
7. 应用套件基座模块都在（`commands.rs`（核心）/ `ribbon.rs` / `keytips.rs` / `qat.rs` /
   `mini_toolbar.rs` / `backstage.rs` / `viewport.rs` / `guides.rs` / `formula_bar.rs` /
   `sheet_grid.rs` / `page_view.rs` / `object_toolkit.rs` / `control_channel.rs` / `mcp.rs`），且
   `ribbon`、`viewport`、`spreadsheet`、`document`、`control-channel` feature 开关两侧都能编译
8. `crates/rgpui-ui`、`rgpui-tokio` 等旧 crate 未复活；`rgpui-markdown` 在，
   `pulldown-cmark = "0.13"`
9. feature 组合按需通过：`charts,effects,qr-code` / `tokio` / `control-channel` / `webview` /
   `screen-capture`（吸管，Windows 本地验证）/ `dom-backend`（+ `cargo test -p rgpui-dom`）/
   `ribbon,viewport` / `spreadsheet` / `document` / `rgpui-markdown` 的 `latex`（公式）、
   `mermaid`（图表）
10. `cargo publish -p rgpui --dry-run --registry crates-io` 通过
11. 发布要求无 dev 循环依赖（测试放消费方 `tests/`，见 1.1.1 教训）
12. 命令清单 gate 通过（`CommandRegistry::validate()` 为空）；改控制通道 / MCP 后跑
    `cargo test -p rgpui --features control-channel --lib` 与
    `cargo test -p showcase --bin automation`

## Web/WASM 开发

前置：nightly 工具链 + `wasm32-unknown-unknown` + `rust-src` + `trunk`。
Web 示例是独立子 crate（含 `index.html`、`trunk.toml`、`.cargo/config.toml`、
`rust-toolchain.toml`），入口模式：`#![cfg_attr(target_family = "wasm", no_main)]` +
`#[wasm_bindgen(start)]` 中先调 `rgpui_platform::web_init()`。

限制：剪贴板 None、文件对话框/托盘/原生菜单不可用、tree-sitter 不可用、图标 CDN 下载。
原生文本能力：`dom-backend` feature + `set_dom_layer_enabled(true)`（见
`docs/web-dom-backend-usage.md`，`hello_web` 与 `rgpui_story` 已开启）。

## 代码规范

- 所有函数中文注释（`///` 公开 API，`//` 内部逻辑），不写英文注释。
