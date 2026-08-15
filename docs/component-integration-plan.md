# rgpui 组件整合执行计划

> 文档日期：2026-08-16
> 策略：rgpui 缺什么，就从现有 crate 中拿什么。以 rgpui-component 为主来源（最成熟、直接依赖 rgpui），adabraka-ui / yororen-ui 为辅。分阶段、按文件逐步并入，每完成一步标记一步。

## 现状

- rgpui 核心（74.8K 行）：底层框架齐全（div / styled / style / taffy 布局 / 事件 / 渲染），**无成品 UI 组件**（button/input/checkbox 等均不存在）。
- rgpui-component（89K 行）：成熟组件库，含完整扩展样式系统（StyledExt/Sizable/Disableable/Selectable）+ 主题系统（ActiveTheme/Colorize）+ 约 60 个组件。直接依赖 rgpui，依赖干净。
- rgpui-adabraka-ui（112K 行）/ rgpui-yororen-ui（20K 行）：自研库，命名与 rgpui-component 重叠，暂不作为合并主来源。

## 整合原则

1. **按依赖顺序**：先地基（扩展样式/主题），后基础组件，最后复合组件。
2. **保持轻量**：语法高亮（tree-sitter 72 包）、图表（chart/plot）、markdown、代码编辑器等高重依赖组件**不并入**，留在 rgpui-component 或未来独立成库。
3. **保留中文注释与 rgpui 风格**：并入代码沿用 rgpui 的模块组织与命名习惯。
4. **完整性问题**：每步完成后运行 `cargo check --workspace`，AGENTS.md 中的"完整性检查清单"必须保持通过。

## 分步计划

### 阶段一：地基层（扩展样式 + 主题）

| 步骤 | 内容 | 来源 | 状态 |
|------|------|------|------|
| 1.1 | `Size` 枚举（xsmall/small/medium/large） | rgpui-component/src/styled.rs | ☑（改名 `ElementSize`，避免与 geometry::Size 冲突） |
| 1.2 | `StyledExt` trait（h_flex/v_flex/paddings/margins/corner_radii/popover_style 等） | rgpui-component/src/styled.rs | ☐ |
| 1.3 | `Selectable` / `Disableable` / `Sizable` / `StyleSized` trait | rgpui-component/src/styled.rs | ☐ |
| 1.4 | `Colorize` trait（opacity/lighten/darken/mix/mix_oklab/to_hex 等颜色工具） | rgpui-component/src/theme/color.rs | ☐ |
| 1.5 | `ActiveTheme` trait + `Theme` 结构 + 主题注册机制 | rgpui-component/src/theme/mod.rs | ☐ |
| 1.6 | 默认主题 JSON（default-theme.json / default-colors.json） | rgpui-component/src/theme/ | ☐ |
| 1.7 | 布局辅助 `h_flex()` / `v_flex()` | rgpui-component/src/styled.rs | ☐ |

### 阶段二：基础组件（轻依赖、应用必需）

| 步骤 | 内容 | 来源 | 状态 |
|------|------|------|------|
| 2.1 | `Icon` 组件 | rgpui-component/src/icon.rs | ☐ |
| 2.2 | `Label` 组件 | rgpui-component/src/label.rs | ☐ |
| 2.3 | `Button` + `ButtonIcon` | rgpui-component/src/button/ | ☐ |
| 2.4 | `Checkbox` | rgpui-component/src/checkbox.rs | ☐ |
| 2.5 | `Radio` | rgpui-component/src/radio.rs | ☐ |
| 2.6 | `Switch` | rgpui-component/src/switch.rs | ☐ |
| 2.7 | `Slider` | rgpui-component/src/slider.rs | ☐ |
| 2.8 | `Input`（文本输入） | rgpui-component/src/input/ | ☐ |
| 2.9 | `Select` + `Caret` | rgpui-component/src/select.rs | ☐ |
| 2.10 | `Spinner` / `Progress` | rgpui-component/src/spinner.rs, progress/ | ☐ |
| 2.11 | `Skeleton` / `Badge` / `Tag` / `Separator` / `Kbd` | rgpui-component/src/ | ☐ |
| 2.12 | `Tooltip` | rgpui-component/src/tooltip.rs | ☐ |

### 阶段三：复合组件（依赖阶段二）

| 步骤 | 内容 | 来源 | 状态 |
|------|------|------|------|
| 3.1 | `Dialog` / `AlertDialog` | rgpui-component/src/dialog/ | ☐ |
| 3.2 | `Menu`（右键/下拉菜单） | rgpui-component/src/menu/ | ☐ |
| 3.3 | `Popover` / `HoverCard` | rgpui-component/src/popover.rs, hover_card.rs | ☐ |
| 3.4 | `Notification` / `Toast` | rgpui-component/src/notification.rs | ☐ |
| 3.5 | `Form`（表单容器） | rgpui-component/src/form/ | ☐ |
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

## 验证方式

- 每步：`cargo check --workspace` 通过
- 阶段结束：`cargo test -p rgpui`（若有测试）
- 全程：AGENTS.md 完整性检查清单通过
- 最终：`cargo check --workspace --examples` 通过

## 备注

- 合并采用**手动移植**（保留中文注释与 rgpui 风格），不使用整文件覆盖。
- 若某组件依赖尚未并入的模块，先并入其依赖（按依赖顺序微调步骤）。