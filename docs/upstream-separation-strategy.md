# rgpui 与上游切割及整合战略

> 文档日期：2026-08-15
> 状态：战略思考，待评审决策
>
> **执行状态（2026-08-18 更新）**：本战略的核心决策已落地——三大 UI 库（`rgpui-component`、`rgpui-adabraka-ui`、`rgpui-yororen-ui` 及 `rgpui-webview`）已全部删除，基础组件已并入 rgpui 核心，后续 UI 库发展以 `docs/ui-crate-plan.md`（rgpui-ui 重组计划）为准。本文档的 C/D 类组件划分及依赖关系图仅作历史记录。

## 1. 背景与决策动机

rgpui 基于 Zed 的 GPU 加速 UI 框架 `gpui` 移植，并同步移植了 `gpui-component` 第三方组件库。经过长期跟随上游合并 PR 后，我们决定**与上游切割，不再跟随 gpui / gpui-component 合并最新 PR**。

切割动机：

1. **上游架构重构**：`gpui-component` 已完成 `gpui-base` 架构重构（PR #2677），与本地代码架构彻底分叉，继续合并几乎不可能。
2. **合并成本持续上升**：rgpui 本地代码已比上游 gpui 最新源码多出约 1.5 万行，每次手工 diff 合并 PR 的代价越来越高。
3. **自有差异化功能**：rgpui 拥有大量独有功能（系统托盘、桌面宠物、3D MSAA、鼠标穿透、Mica 材质等），不应被上游架构绑架。
4. **生态自治**：0.5.0 版本已成功独立发布，具备完全自主演进的能力。

## 2. rgpui 生态全景

### 2.1 Crate 规模统计

| 分层 | Crate | 文件数 | 代码行数 | 说明 |
|------|-------|--------|----------|------|
| 平台 | rgpui-macros | 17 | 4,311 | 过程宏 |
| 平台 | rgpui-windows | 24 | 11,993 | Windows 平台实现 |
| 平台 | rgpui-linux | 31 | 13,258 | Linux 平台实现 |
| 平台 | rgpui-macos | 23 | 11,334 | macOS 平台实现 |
| 平台 | rgpui-web | 12 | 3,000 | Web/WASM 平台实现 |
| 平台 | rgpui-wgpu | 7 | 4,061 | wgpu 渲染后端 |
| 平台 | rgpui-platform | 1 | 157 | 平台选择入口 |
| 核心 | rgpui | 141 | 74,768 | 核心 UI 框架 |
| 扩展 | rgpui-3d | 9 | 5,057 | 3D 渲染引擎集成 |
| 扩展 | rgpui-character | 8 | 647 | 桌宠角色运行时系统 |
| 扩展 | rgpui-term | 11 | 5,600 | 终端模拟器组件 |
| ~~扩展~~ | ~~rgpui-tokio~~ | — | — | 已并入 rgpui 核心（feature `tokio` 门控，`rgpui::tokio` 模块） |
| UI | rgpui-component | 260 | 89,281 | 移植自 gpui-component |
| UI | rgpui-adabraka-ui | 319 | 112,268 | 自研组件库（shadcn 风格） |
| UI | rgpui-yororen-ui | 100 | 19,775 | 独立 UI 组件库 |
| 组件辅助 | rgpui-component-assets | 4 | 224 | 组件资源文件 |
| 组件辅助 | rgpui-component-macros | 2 | 362 | 组件过程宏 |
| 组件辅助 | rgpui-webview | 2 | 367 | WebView 集成 |

> 注：`rgpui-component-story` / `rgpui-component-story-web` 为 Storybook 演示项目（publish=false），不计入发布范围。

### 2.2 依赖关系

- **核心** `rgpui` 依赖 `rgpui-macros`（正式依赖）+ `rgpui-platform` / `rgpui-web`（仅 dev-dependencies）
- **平台层**：`rgpui-platform` 通过 `cfg(target_os)` 条件依赖各平台实现
- **UI 层**：三个 UI 库均直接依赖 `rgpui` 核心，互不依赖

### 2.3 关键观察

1. **三大 UI 库功能高度重叠**：rgpui-component、rgpui-adabraka-ui、rgpui-yororen-ui 各自实现了 button/input/select/table/dialog/tooltip/menu 等基础组件，命名与行为不统一。
2. **只有 rgpui-component 来自上游**：adabraka-ui 与 yororen-ui 本就是自有资产，与上游切割无关。
3. **rgpui-component 依赖极重**：携带 72 个 tree-sitter 语法高亮语言包 + resvg + objc2 等平台绑定依赖，但其 `tree-sitter-languages` feature 为**可选且非默认启用**，具备轻量使用的前提。

## 3. 整合思路

**核心原则：不是将所有第三方库折叠进 rgpui，而是将"必要的、常用的"合并进 rgpui 核心；"重要但不常用"的成立我们自己的第三方库，自主演进。**

### 3.1 模块分类

#### A 类：留在 rgpui 核心（框架骨架，天然内聚）

`app / window / element / style / scene / text_system / platform / executor / scheduler / input / keymap / assets / subscription / gestures / tab_stop / tray / single_instance`

约 60K 行，属于 UI 框架骨架，无争议。

#### B 类：rgpui 内部"寄生"的独立工具

| 模块 | 行数 | 判断 |
|------|------|------|
| http_client | 1,274 | 被 app/executor/img 深度引用，**留** |
| keymap | 912 | 与 input/action 强耦合，**留** |
| profiler / perf | 581 | 轻量，**留** |
| collections / sum_tree / refineable | 已内部化 | 保持现状 |

结论：rgpui 核心内部没有明显需要迁出的模块。

#### C 类：rgpui-component 中的"必要常用"（并入 rgpui）

基础组件：`button / input / checkbox / radio / select / dialog / tooltip / menu / label / tag / icon / notification / progress / slider / switch / spinner / skeleton / badge / breadcrumb / pagination / separator / table / list`

主题体系：`theme/`

判断依据：轻依赖、UI 应用必需、无平台绑定。

#### D 类：rgpui-component 中的"重要少用"（→ 自有第三方库）

- **语法高亮 `highlighter/`**：拖 72 个 tree-sitter 语言包（optional feature，非默认启用）
- **代码编辑器 `input/` 后半 + `text/`**：依赖 highlighter 与 LSP
- **图表 `chart/` `plot/`**：专业可视化组件
- **markdown 渲染**：依赖 markdown + html5ever 重依赖

### 3.2 目标形态

```
rgpui（核心框架 + 必要常用组件）
├── 框架核心（现有约 75K 行）
├── 基础组件层（从 rgpui-component 精选并入的轻依赖组件）
└── 可选 feature：语法高亮 / 代码编辑器 / markdown（feature 门控，默认关闭）

自有第三方库（独立 crate，自主演进）
├── rgpui-highlighter   （语法高亮，tree-sitter 全家桶）
├── rgpui-plot / rgpui-chart（可视化图表）
├── rgpui-markdown
├── rgpui-3d            （已有）
├── rgpui-term          （已有）
└── 未来按需新增
```

### 3.3 技术可行性

- rgpui-component 的 `tree-sitter-languages` 等重依赖 feature 已是 **optional + 非默认**，可直接基于此进行拆分，无需重写。
- 核心内模块已通过 `rgpui.rs` 入口统一管理，新增组件模块可沿用 `elements/` 结构。
- 切割后合并脚本与上游仓库已清理：`scripts/merge-upstream-pr.ps1`、`.opencode/merge-upstream-workflow.md`、`.opencode/upstream-rules.json`、`UPSTREAM-PRS.json`、`docs/merged-prs.md` 及 `temp/` 上游 worktree 均已删除（见提交记录）。

## 4. 组件重叠盘点

### 4.1 三个 UI 库共有组件

`checkbox / icon / label / radio / select / skeleton / slider / tooltip`

### 4.2 rgpui-component 与 adabraka-ui 共有

`alert / checkbox / collapsible / color_picker / combobox / icon / kbd / label / pagination / radio / rating / select / separator / skeleton / slider / spinner / tooltip`

### 4.3 命名空间差异

- rgpui-component：`button / input / dialog / tooltip / select`（移植自上游，命名贴近 gpui-component）
- rgpui-adabraka-ui：`button / input / dialog / tooltip / select` + 大量动画/图表组件（shadcn 风格，命名空间为 `components/`）
- rgpui-yororen-ui：`button_group / combo_box / dropdown_menu / keybinding_input` 等（独立命名空间）

> 三个库的组件存在显著重叠，且命名规范不统一，合并前需确定主来源。

## 5. 待决策问题

1. **adabraka-ui（112K 行）与 yororen-ui（20K 行）的定位**：两个自研库与并入后的 rgpui 组件层会重叠，需确定是"融合为一个"还是"分层共存"（作为 rgpui 组件层的上层封装）。
2. **rgpui-component 并入的核心组件来源**：以哪个库为主来源（adabraka 更现代，component 更成熟）。
3. **上游资源处置**：已删除合并脚本、工作流文档、PR 追踪文件与上游 worktree（已完成，见 §3.3）。
4. **feature 门控粒度**：语法高亮 / 代码编辑器 / markdown 等重功能作为 rgpui 的 feature 还是直接独立成库。

## 6. 后续步骤

1. 盘点 adabraka / yororen 与 rgpui-component 的重叠组件清单，确定内嵌组件主来源。
2. 制定 rgpui-component 拆分计划（C 类并入核心，D 类独立成库）。
3. 制定自有第三方库的命名与发布规范。
4. 冻结 `UPSTREAM-PRS.json` 更新（该文件已删除，切割日期 2026-08-15）。