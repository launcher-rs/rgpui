# rgpui 组件整合执行计划

> 文档日期：2026-08-16
> 策略：rgpui 缺什么，就从现有 crate 中拿什么。以 rgpui-component 为主来源（最成熟、直接依赖 rgpui），adabraka-ui / yororen-ui 为辅。分阶段、按文件逐步并入，每完成一步标记一步。

## 现状

- rgpui 核心（74.8K 行）：底层框架齐全（div / styled / style / taffy 布局 / 事件 / 渲染），**无成品 UI 组件**（button/input/checkbox 等均不存在）。
- rgpui-component（89K 行）：成熟组件库，含完整扩展样式系统（StyledExt/Sizable/Disableable/Selectable）+ 主题系统（ActiveTheme/Colorize）+ 约 60 个组件。直接依赖 rgpui，依赖干净。
- rgpui-adabraka-ui（112K 行）/ rgpui-yororen-ui（20K 行）：自研库，命名与 rgpui-component 重叠，暂不作为合并主来源。

## 整合原则

1. **按依赖顺序**：先地基（扩展样式/主题），后基础组件，最后复合组件。
2. **保持轻量**：并非所有组件都要并入 rgpui 核心。语法高亮（tree-sitter 72 包）、图表（chart/plot）、markdown、代码编辑器等高重依赖组件**不并入**；低频使用但重要的组件，将来单独做成 **rgpui UI 扩展库**（独立 crate）存放处理，保持核心轻量。
3. **保留中文注释与 rgpui 风格**：并入代码沿用 rgpui 的模块组织与命名习惯。
4. **完整性问题**：每步完成后运行 `cargo check --workspace`，AGENTS.md 中的"完整性检查清单"必须保持通过。

## 分步计划

### 阶段一：地基层（扩展样式 + 主题）

| 步骤 | 内容 | 来源 | 状态 |
|------|------|------|------|
| 1.1 | `ElementSize` 枚举（原 Size，改名避免与 geometry::Size 冲突） | rgpui-component/src/styled.rs | ☑ |
| 1.4 | `Colorize` trait + 颜色常量函数（hsl/ColorName/try_parse_*） | rgpui-component/src/theme/color.rs | ☑（依赖顺序调整为 1.4 提前） |
| 1.5 | `ActiveTheme` trait + `Theme` 结构 + 主题注册机制 | rgpui-component/src/theme/ | ☑（含 highlight.rs 纯数据结构、settings.rs 轻量设置类型） |
| 1.6 | 默认主题 JSON（default-theme.json / default-colors.json） | rgpui-component/src/theme/ | ☑（随 1.5 一并移植） |
| 1.2 | `StyledExt` trait（h_flex/v_flex/paddings/margins/corner_radii/popover_style 等） | rgpui-component/src/styled.rs | ☑（含 FocusableExt/Side/AxisExt/ElementExt 追加） |
| 1.3 | `Selectable` / `Disableable` / `Sizable` / `StyleSized` trait | rgpui-component/src/styled.rs | ☑ |
| 1.7 | 布局辅助 `h_flex()` / `v_flex()` | rgpui-component/src/styled.rs | ☑ |

### 阶段二：基础组件（轻依赖、应用必需）

| 步骤 | 内容 | 来源 | 状态 |
|------|------|------|------|
| 2.1 | `Icon` 组件 | rgpui-component/src/icon.rs | ☑ |
| 2.2 | `Label` 组件 | rgpui-component/src/label.rs | ☑ |
| 2.3 | `Button` + `ButtonIcon` | rgpui-component/src/button/ | ☑（依赖 Tooltip/Root，随 2.12 完成） |
| 2.4 | `Checkbox` | rgpui-component/src/checkbox.rs | ☑ |
| 2.5 | `Radio` | rgpui-component/src/radio.rs | ☑ |
| 2.6 | `Switch` | rgpui-component/src/switch.rs | ☑ |
| 2.7 | `Slider` | rgpui-component/src/slider.rs | ☑ |
| 2.8 | `Input`（文本输入） | rgpui-component/src/input/ | 🚧 进行中，见下方"Input 子系统移植进度" |
| 2.9 | `Select` + `Caret` | rgpui-component/src/select.rs | ☐（延后，依赖 Input） |
| 2.10 | `Spinner` / `Progress` | rgpui-component/src/spinner.rs, progress/ | ☑ Spinner（Progress 未并入） |
| 2.11 | `Skeleton` / `Badge` / `Tag` / `Separator` / `Kbd` | rgpui-component/src/ | ☑ |
| 2.12 | `Tooltip` | rgpui-component/src/tooltip.rs | ☑（含精简 Root + transition + Placement） |

### 阶段三：复合组件（依赖阶段二）

| 步骤 | 内容 | 来源 | 状态 |
|------|------|------|------|
| 3.0 | **滚动子系统**（scrollable + scrollbar + scrollable_mask + auto_scroll） | rgpui-component/src/scroll/ | ☑ 移至 `rgpui/src/elements/scroll/`，25 测试通过 |
| 3.1 | `Dialog` / `AlertDialog` | rgpui-component/src/dialog/ | ☑ 移至 `rgpui/src/dialog/` + `focus_trap.rs` + `window_ext.rs` + `window_border.rs` + `root.rs` 扩展，4 测试通过 |
| 3.2 | `Menu`（右键/下拉菜单） | rgpui-component/src/menu/ | ☑ 移至 `rgpui/src/menu/`（popup_menu + context_menu + dropdown_menu + app_menu_bar + menu_item），顺带移植 `popover.rs` 作 dropdown_menu 依赖 + 精简 `global_state.rs`（app_menus 存储），8 测试通过 |
| 3.3 | `Popover` / `HoverCard` | rgpui-component/src/popover.rs, hover_card.rs | ☑ Popover 随 3.2 移植至 `rgpui/src/menu/popover.rs`；HoverCard 移至 `rgpui/src/menu/hover_card.rs`（std::time::Duration 替代 instant），2 测试通过 |
| 3.4 | `Notification` / `Toast` | rgpui-component/src/notification.rs | ☑ 移至 `rgpui/src/menu/notification.rs`（Notification + NotificationList + NotificationType），`NotificationSettings` 复用 theme 层，`NotificationId` 公开导出，4 测试通过 |
| 3.5 | `Form`（表单容器） | rgpui-component/src/form/ | ☑ 移至 `rgpui/src/form/`（Form + Field + FieldBuilder + v_form/h_form/field 构造器），`Size`→`ElementSize`、`AxisExt` 用 `matches!` 替代，1 测试通过 |
| 3.6 | `List` / `VirtualList` | rgpui-component/src/list/ | ☐ |
| 3.7 | `Table` | rgpui-component/src/table/ | ☐ |
| 3.8 | `Tabs` / `Accordion` / `Collapsible` | rgpui-component/src/ | ☐ |
| 3.9 | `TitleBar` / `WindowBorder` | rgpui-component/src/title_bar.rs, window_border.rs | ☐ |

### 阶段四：收尾

| 步骤 | 内容 | 状态 |
|------|------|------|
| 4.1 | 更新 rgpui 预导入（prelude）暴露新组件 | ☐ |
| 4.2 | 更新 AGENTS.md 独有功能清单 | ☐ |
| 4.3 | 编写文档（组件使用说明） | ☐ |
| 4.4 | 发布 0.6.0 到 crates.io | ☐ |

### 后续（不在本计划内，独立成库或保留）

- 语法高亮 `highlighter/`（72 个 tree-sitter 语言包）
- 图表 `chart/` `plot/`
- 代码编辑器 `input/` 后半 + `text/`
- markdown 渲染
- rgpui-adabraka-ui / rgpui-yororen-ui 的融合决策

> 以上低频使用但重要的组件，将来放入独立的 **rgpui UI 扩展库**（crate），不并入 rgpui 核心。

### Input 子系统移植进度（2.8）

> 移植源：`rgpui-component-workspce/rgpui-component/src/input/`（state.rs 3943 行为核心）。目标模块：`rgpui/src/input_ui/`（避免与 rgpui 原生 `mod input` 冲突）。

**已完成（基础层，已提交 commit `6d63082c43`，43 测试通过）**：

- 依赖：rgpui 新增 `ropey` + `instant`
- `history.rs` / `auto_scroll.rs` / `word_selection.rs` / `rope_ext.rs` / `cursor.rs` / `change.rs` / `layout.rs`
- `display_map/`（folding / text_wrapper / wrap_map / display_map / fold_map）
- `mode.rs`（InputMode，已裁剪 highlighter/diagnostics/parse_task 字段，保留 CodeEditor 外壳能力）
- `indent.rs`（TabSize + is_indentable）/ `selection.rs`（TextSelector）/ `blink_cursor.rs` / `content_type.rs` / `mask_pattern.rs`
- `ShapedLine` 新增 `x_for_index`/`closest_index_for_x`/`index_for_x` 委托方法
- `menu/global_state.rs` 新增 `suppress_text_selection` 支持（供 InputState 鼠标按下抑制窗口级文本选择）

**已完成（核心移植，未挂接编译）**：

- `state.rs`（InputState 核心移植完成，约 1500 行：40 个动作 + `Enter`(no_json) + `InputEvent` + `init()` 按键绑定 + `EntityInputHandler`/`Focusable`/`Render`，已剥离 highlighter/LSP/search/popovers/NativeMenu/Root/inline-completion）
- `input.rs` / `movement.rs` / `decorations.rs` / `number_input.rs` / `clear_button.rs`（已写，随 state.rs 一起挂接）
- `selection.rs` 补充了 `InputState::select_word`/`select_line`（双击/三击选中），**但已注释掉**：`super::state` 未挂接时会导致编译错误

**待移植（下一阶段，优先项）**：

- ☑ 最高优先：`element.rs`（TextElement + EditorScrollbarSnapshot + EditorScrollbar，`RIGHT_MARGIN`/`cursor_surrounding_padding`/`BOTTOM_MARGIN_ROWS`，滚动条改用 `rgpui/src/elements/scroll/` 的 scrollable API）——`state.rs` 编译依赖
- ☑ 优先：`indent.rs` 补齐 `indent_inline`/`indent_block`/`outdent_inline`/`outdent_block`/`has_indent_guides`（`input.rs` on_action 依赖）
- ☑ 优先：恢复 `selection.rs` 中注释掉的 `InputState::select_word`/`select_line`
- `input_ui/mod.rs` 挂接模块：`mod state/element/input/number_input/movement/decorations/clear_button` + `pub(crate) use state::*`（`CONTEXT` 已改为 `pub(crate)`）
- 移植完成后：`cargo test -p rgpui --features test-support --lib input_ui` 全绿 → 更新本表 2.8 为 ☑

> 注意：`state.rs` / `input.rs` 等核心移植文件目前为未挂接状态（不参与编译），挂接前必须先完成 `element.rs` 及其余依赖，否则 `cargo check` 失败。

## 验证方式

- 每步：`cargo check --workspace` 通过
- 阶段结束：`cargo test -p rgpui`（若有测试）
- 全程：AGENTS.md 完整性检查清单通过
- 最终：`cargo check --workspace --examples` 通过

## 备注

- 合并采用**手动移植**（保留中文注释与 rgpui 风格），不使用整文件覆盖。
- 若某组件依赖尚未并入的模块，先并入其依赖（按依赖顺序微调步骤）。

## 已发现的问题（移植过程中记录，待后续修复）

### 1. 颜色函数命名冲突（已解决，属于架构约束）

rgpui 核心 `color.rs` 已定义无参标准色函数（`black()`/`white()`/`red()` 等），而 rgpui-component 的 `theme/color.rs` 定义了**带参色板函数**（`red(scale)`）+ **色阶函数**（`red_50()`~`red_950()`）。

处理结果：
- theme 的 `black()`/`white()` 与核心语义重复，已**删除**，统一用核心版本。
- theme 的裸色名带参版 `red(scale)` 设为 `pub(crate)`（不导出到根，避免与核心 `red()` 冲突），只能通过 `rgpui::theme::color::red(500)` 访问。
- 色阶函数 `red_500()` 等不与核心冲突，但**未在根命名空间导出**（通过 `theme::color` 模块路径访问）。

> 未来移植组件时，裸色名带参版（`red(500)` 这种）需改为 `theme::color::red(500)` 或色阶函数形式。建议在 4.x 收尾时统一处理。

### 2. `FakeHttpClient` 测试编译失败（既有问题，非移植引入）

`cargo test -p rgpui --lib`（不启用 `test-support` feature）会报错：

```
cannot find `FakeHttpClient` in `http_client`
```

- `FakeHttpClient` 定义在 `http_client/mod.rs:367`，受 `#[cfg(feature = "test-support")]` 门控。
- `app/test_context.rs:132` 无条件引用它。
- **结论**：既有 bug（0.5.0 发布前已存在），与本整合无关。修复方式：为 `app/test_context.rs`（及引用 FakeHttpClient 的测试文件）添加 `#[cfg(feature = "test-support")]` 门控，或让 `test-support` 成为默认 feature。

### 3. theme 模块裁剪了 notify 目录监听功能

`registry.rs` 的 `watch_dir`/`_watch_themes_dir`/`reload_themes`/`reload` 依赖 `notify` crate（主题目录热重载）。rgpui 核心无 `notify` 依赖，**已裁剪**这四个函数。

> 若未来需要主题目录热重载，可：
> 1. 为 rgpui 添加 `notify` 依赖（optional feature），或
> 2. 在 rgpui-component 层保留该功能（通过 rgpui 公开的 `ThemeRegistry` API 扩展）。

### 4. `tracing::info!` 在 init 回调中引用（`Reload active theme`）

`registry.rs::init` 中 `observe_global::<ThemeRegistry>` 回调使用了 `tracing::info!`。rgpui 已有 `tracing` 依赖，无需处理。

### 5. 主题系统的 `HighlightTheme` 为纯数据结构

`theme/highlight.rs` 只移植了 `HighlightTheme`/`HighlightThemeStyle`/`SyntaxColors`/`StatusColors`/`ThemeStyle` 等**纯 JSON schema 数据结构**（无 tree-sitter 依赖）。`LanguageRegistry`（代码高亮注册表）与高亮渲染逻辑未移植，保留在 rgpui-component。

> 这与整合原则一致：语法高亮不并入 rgpui。主题系统依赖的仅是高亮主题的**数据结构**，用于主题 JSON 解析与颜色访问。

### 6. 下一步注意：StyledExt 的 `cx.theme()` 需要 ActiveTheme 在根命名空间可见

StyledExt（1.2 步骤）使用 `cx.theme()`，依赖 `ActiveTheme` trait 已在根导出（rgpui.rs 已 `pub use theme::{ActiveTheme, ...}`）。同时 `focused_border`/`popover_style` 使用 `cx.theme().ring`/`tokens` 等 Theme 字段，均已就绪。

### 7. 既有测试失败：`elements::img` 与 `elements::list`（非移植引入）

`cargo test -p rgpui --features test-support --lib` 有 26 个失败测试，全部集中在：
- `elements::img::tests::*`（如 `stale_frame_index_is_clamped_when_image_changes`）
- `elements::list::test::*`（如 `test_autoscroll_above_item_top_renders_items_above`）

**验证**：在移植主题（1.5）之前的提交 `3c2be44057` 上，这些测试同样失败。**结论**：既有问题，与组件整合无关。可能原因：运行环境（Windows 无 GPU）或上游代码在测试环境下的行为差异。后续可单独排查。

### 8. schema.rs 测试依赖外部主题文件（已处理）

原 `test_aurora_theme_parses_gradient_backgrounds` 测试依赖 `../../../../themes/aurora.json`（存在于 rgpui-component 但不在 rgpui），已**删除**该测试。若 rgpui 未来有自定义主题目录，可恢复此测试。

### 9. Button 依赖 Tooltip → Root 全局系统（延迟处理）

Button（2.3）依赖 `managed_tooltip`（`ManagedTooltipExt`，tooltip.rs 内 pub(crate)），而 Tooltip 又依赖 Root 全局容器系统、animation（Transition）、Kbd、Text 等大量未移植组件。

**决策**：先移植独立的基础组件（Checkbox/Switch/Slider 等，不依赖 Tooltip），Tooltip/Root 系统留待基础组件完成后单独处理，再回来完成 Button 的 tooltip 集成。已移植的 Button 前置依赖：FocusableExt、Caret、Icon、StyleSized、tokens 系统均已就绪。