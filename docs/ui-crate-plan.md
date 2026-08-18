# rgpui UI 库重组计划（rgpui-ui）

> 文档日期：2026-08-18
> 状态：**执行中**。战略已定稿，§8 全部决策项已拍板（2026-08-18 用户确认），开始 §7 执行步骤。执行进度见文末「§9 执行记录」。
> 上游文档：`docs/upstream-separation-strategy.md`（切割战略）、`docs/component-integration-plan.md`（组件整合，已全部完成）

## 1. 背景与决策动机

组件整合计划（阶段一~四）已全部完成，`rgpui` 核心已自带完整基础组件库（elements/、form/、input_ui/、menu/、dialog/、list/、table/、tabs/、title_bar/、theme/）。

剩下的三个旧 UI 库定位尴尬：

| 旧库 | 规模 | 来源 | 问题 |
|------|------|------|------|
| rgpui-component | 260 文件 / 89K 行 | 移植自上游 gpui-component | 名称带"移植"烙印；大部分组件已并入核心，剩余为高频重复 |
| rgpui-adabraka-ui | 319 文件 / 112K 行 | 自研（shadcn 风格） | 约 150 个组件中大量与核心重叠；冗余度高 |
| rgpui-yororen-ui | 100 文件 / 20K 行 | 自研 | 命名独立，架构另起炉灶 |

**决策**（2026-08-18 用户拍板）：

1. **三个旧 UI 库全部废弃、彻底删除**（含 rgpui-component 子工作区、rgpui-adabraka-ui、rgpui-yororen-ui、rgpui-webview）。
2. **新建自有 UI 库 `rgpui-ui`**：将"核心没有的剩余组件"集成到新库中，**基于核心构建，不重复实现核心已有的基础组件**。
3. 不沿用 component / yororen / adabraka 这些外来或品牌化名称。

## 2. 删除旧库的影响范围（已勘察）

勘察结论：`rgpui` 核心、`rgpui-term`、`rgpui-tokio`、`rgpui-web` 及所有平台/渲染 crate **完全不依赖**这三个旧库，删除无框架风险。

### 2.1 各旧库被引用情况

| 旧库 | publish | 被谁引用 |
|------|---------|----------|
| rgpui-component | true | rgpui-component-story/story-web（同删）；rgpui-webview（同删）；9 个组件示例 crate；examples/rgpui_async_demo、rgpui_term_basic、rgpui_term_component_integration |
| rgpui-component-macros | true | 仅 rgpui-component |
| rgpui-component-assets | true | rgpui-component、story、webview、部分组件示例、2 个 examples、2 个 web 示例 |
| rgpui-component-story / story-web | false | story-web 依赖 story，其余无 |
| rgpui-webview | true | 仅根 workspace.dependencies（无其他依赖方） |
| rgpui-adabraka-ui | true | 仅根 workspace 成员 + workspace.dependencies |
| rgpui-yororen-ui | true | 4 个 examples（counter/todolist/file_browser/toast_notification） |

### 2.2 需同步处理的构建影响

- **根 `Cargo.toml`**：`members` 移除 `crates/rgpui-adabraka-ui`、`crates/rgpui-yororen-ui`、`crates/rgpui-component-workspce/*`、`crates/rgpui-component-workspce/rgpui-component/examples/*`；`exclude` 清理过期项（themes、两个已不存在的 web 示例路径）；`[workspace.dependencies]` 移除 6 个 crate 引用。
- **受影响的 examples**（7 个，依赖被删 crate）：`rgpui_async_demo`、`rgpui_term_basic`、`rgpui_term_component_integration`、`rgpui_yororen_ui_counter/todolist/file_browser/toast_notification` —— 需删除或重写（其中 term 两个示例评估是否改用核心组件）。
- **已失效的 web 示例**：`crates/rgpui-web/examples/components_web`、`hello_world_web` 引用已不存在的 `rgpui-component` 路径，本就坏掉，一并清理。
- **主题资源**：`crates/rgpui-component-workspce/themes/` 的 22 套 JSON 主题随删除丢失。核心 `theme/registry.rs` 内嵌自己的 `default-theme.json`（`include_str!`），**不读取**该目录（`themes_dir` 字段已废弃未用），故无构建影响；但需评估 22 套主题是否有保留价值（可拷入核心或 rgpui-ui 作为额外主题资源）。
- **CI**：无 crate 名硬引用，仅跑 `cargo check --workspace` / `--workspace --examples` / clippy / fmt，删除后自然通过。

### 2.3 附带收益

当前 `cargo check --workspace` 的已知失败（组件示例 Button/Root/h_flex/v_flex 歧义错误、rgpui-component-story 57 错、adabraka 示例 115 错）**全部来自旧库**。删除后 workspace 与 examples 将**首次全绿**。

### 2.4 文档与技能清理（不构建影响）

- 更新：`AGENTS.md`（架构图、命令说明、完整性检查清单）、`docs/upstream-separation-strategy.md`、`docs/rgpui-book/08-components.md`。
- 删除：`skills/rgpui-component/`、`docs/linux-dependencies.md`（GTK/WebView 依赖说明）。

## 3. 新库 rgpui-ui 定位

```
rgpui（核心：框架 + 基础组件 + 时间驱动动画原语）
  └── rgpui-ui（新：核心没有的组件 + 动画组件/特效 + 精选自建能力）
        └── 独立专业库：rgpui-markdown（重依赖，另行论证）
```

**原则：`rgpui-ui` 只收核心没有的**。核心已有的（elements/、form/、input_ui/、menu/、dialog/、list/、table/、tabs/、title_bar/）一律不再实现，直接用核心的。

### 3.1 拟迁入内容（核心没有的）

| 分类 | 候选组件 | 判断 |
|------|----------|------|
| 动画组件 | animated_switch/counter/presence/progress/text、number_ticker、type_writer、ripple、shimmer、marquee、pulse_indicator、countdown、text_reveal、carousel、tilt_card、magnetic_button | 精选迁入（是否全要待定） |
| 特效 | aurora、confetti、particle_emitter、meteors、dot_pattern、noise、gradient_text、glass_morphism、gradient_border | **装饰性特效，是否保留待定** |
| 媒体 | video_player、audio_player、waveform | 迁入（注意平台依赖） |
| 显示 | qr_code、sparkline、code_block、image_viewer、svg_renderer、rich_text | 迁入 |
| 高级输入 | mention_input、tag_input、otp_input、inline_edit、search_input、hotkey_input、file_upload | 迁入 |
| 布局 | split_pane、resizable、expandable_card、empty_state、canvas_component、drag_drop、sortable_list、infinite_scroll、virtual_list、view_router、segmented_nav | 迁入 |
| 通知/命令 | notification_center、spotlight、command_palette、drawer_navigation、navigation_menu、app_menu、bottom_sheet | 迁入 |
| 工具能力 | gestures（手势识别）、scroll_physics（滚动惯性） | 迁入（需与核心 InteractiveElement/scroll 查重） |
| 图表 | charts/（10+ 图，自成体系） | **待定：独立库 or feature** |
| markdown | display/markdown | **待定：独立库**（见 §5） |

**共享辅助层随组件一起迁**：adabraka 的独有组件并非孤立文件，依赖其 `styled_ext.rs`、`gpui_ext.rs`、`layout.rs`、`responsive.rs`、`util.rs`、`icon_config.rs` 等共享模块。迁移时必须连带处理——与核心 `Styled`/`Responsive` 等冲突部分并入核心，其余进 rgpui-ui 内部 `util/`、`layout/` 子模块，避免"只迁组件、丢了地基"。

### 3.2 明确不迁入

- 核心已有的全部基础组件（component/adabraka/yororen 三库中的 button、input、checkbox、radio、switch、slider、select、dialog、menu、tabs、table、avatar、alert、pagination、color_picker、rating、stepper、resizable、dock、status_bar、tree、calendar、date_picker、time_picker、sheet、badge、kbd、tooltip、separator、progress、spinner、skeleton、form、textarea 等）。
- yororen 独有的 keybinding_input/display、file_path_input、button_group、combo_box、dropdown_menu、disclosure、focus_ring —— **评估迁移或放弃**（部分可能并入核心或 rgpui-ui，待定）。
- adabraka 的 `editor.rs`（ropey + tree-sitter 的完整代码编辑器）—— 计划新增决策项：重依赖且与 rgpui-term 相关，**建议丢弃或预留为未来独立 `rgpui-editor` 专业库，不并入 rgpui-ui**（见 §8 #9）。

## 4. 动画归属探讨

### 4.1 现状盘点

**核心已有（自洽、有测试）：**
- `elements/animation.rs`：`Animation`（时长/缓动/循环/同步相位/节流帧/reduce_motion）+ `AnimationExt::with_animation` + `AnimationElement`。
- `transition.rs`：easing 函数族（cubic_bezier/ease_out_cubic/ease_in_out_cubic）+ `Lerp` trait + `Transition`（slide/fade/width/height 组合过渡，`.apply()` 到元素）。
- 已被核心组件实际使用：tabs 滑动指示条、dialog、notification、switch、checkbox、spinner、skeleton、window。

**adabraka 动画层（与核心互补、不重叠）：**
- `spring.rs`（137 行）：**弹簧物理，值驱动**（刚度/阻尼/质量，惯性回弹）——核心没有的唯一"新原语"。
- `animate.rs`：`AnimationPreset`（fade_in/slide_up/scale_in/bounce_in...）、`KeyframeAnimation`、`StaggerConfig`、`Transition` —— 基于核心 `Animation` 的**上层便捷构造器**，部分与核心 easing 冗余。
- `animation_coordinator.rs`：命名动画注册表 + 完成回调 —— 能力弱于核心同步相位机制，价值不大。
- `animated_state.rs`、`content_transition.rs`、`transitions.rs`、`scroll_physics.rs`、`gestures.rs`。
- 动画组件：`animated_*`、`number_ticker`、`type_writer`、`ripple`、`shimmer`、`marquee`、`meteors`、`confetti`、`particle_emitter`、`aurora`、`pulse_indicator`、`countdown`、`text_reveal` 等。

**关键结论：核心是"时间驱动"，adabraka 补的是"值驱动（弹簧）"+ 动画组件，二者互补而非重复。**

**桥接点（落地时第一个技术点）**：弹簧是 **dt 驱动**的（`spring.tick(dt)`），而核心 `Animation` 是**进度驱动**（0~1 delta）。rgpui-ui 的动画组件无法直接用 `with_animation` 驱动弹簧，需要一个桥接方案，例如：
- 在 `with_animation` 的 animator 里把 delta 换算成 dt（如 `dt = delta * duration`，配合 `repeat_synced` 或每帧重渲染），或
- 动画组件自建 `request_animation_frame` 状态机，按真实帧间隔 `tick` 弹簧。
建议在动画子模块迁入时先写好一个 spring→animation 桥接的测试用例（类似核心 `animation.rs` 的 `#[rgpui::test]`），再逐组件套用。

### 4.2 动画归属的三种方案与优劣

#### 方案 A：分层安置（当前推荐）

- 时间驱动原语 → **留核心**（现状不动）
- 弹簧 `Spring` + 动画 DSL/交错 → **进 rgpui-ui 内部 `animation/` 子模块**（跟随唯一使用者）
- 动画组件 → **归 rgpui-ui**
- 不建独立 `rgpui-animation` crate

| 优点 | 缺点 |
|------|------|
| 职责清晰：核心管基础，rgpui-ui 管组件层动画 | 若未来其他库也要弹簧，需依赖 rgpui-ui |
| 无 crate 碎片化 | 弹簧若做得通用，放核心更"正统" |
| 核心稳定性不受动画范式变更影响 | —— |

#### 方案 B：弹簧并入核心

把 `Spring` 弹簧物理并入核心 `transition.rs`，核心所有组件可选使用。

| 优点 | 缺点 |
|------|------|
| 弹簧是"值动画原语"，与 UI 无关，属基础能力 | 核心膨胀（虽仅 137 行） |
| 未来核心组件（tabs 指示条、dialog 缩放）可选用回弹手感 | 核心组件不迁移则成死代码 |
| 所有下游库免依赖直接可用 | 引入新的动画范式，动摇核心稳定性 |

#### 方案 C：独立 crate `rgpui-animation`

弹簧 + DSL + 协调器单独成库，rgpui-ui 依赖之。

| 优点 | 缺点 |
|------|------|
| 动画系统与组件彻底解耦 | crate 碎片化，多一个库要维护 |
| 任何库/应用可只依赖动画 | 当前唯一使用者就是 rgpui-ui，属于过度设计 |
| 未来重依赖动画时天然独立 | 与核心既有动画原语形成两个动画体系，割裂 |

**倾向**：方案 A。若弹簧演进为"核心组件也需要"或"多库复用"，届时再从 rgpui-ui 抽出（Spring 保持纯 Rust 无 UI 依赖即可轻松抽出）。

## 5. markdown 归属探讨

### 方案 A：独立成库 `rgpui-markdown`（当前推荐）

| 优点 | 缺点 |
|------|------|
| 重依赖（markdown 解析 + html5ever）不污染 rgpui-ui 默认构建 | 多一个 crate |
| 单用途自包含，符合 rgpui-term/rgpui-3d 独立库模式 | 若无人用则闲置 |
| 将来并入代码高亮（tree-sitter）时依赖树清晰 | 初始 API 设计需一次到位 |
| 不进核心（AGENTS.md 已定高重依赖不并入） | —— |

### 方案 B：作为 rgpui-ui 的 feature

| 优点 | 缺点 |
|------|------|
| 少一个 crate，使用者只需依赖一个库 | 默认构建若开启则拖重 rgpui-ui |
| 与组件生态共享上下文 | feature 门控管理复杂，跨 feature 编译矩阵膨胀 |
| —— | 与代码高亮集成时依赖纠缠 |

**倾向**：方案 A（独立成库）。rgpui-ui 定位轻量组件集合，被 markdown 重依赖污染不划算。

> **已落地**（2026-08-18）：按方案 A 新建 `crates/rgpui-markdown`，见 §9 执行记录 §7.6。

## 6. 其他待决项与优劣

### 6.1 图表（charts/）

| 方案 | 优点 | 缺点 |
|------|------|------|
| 独立成库 `rgpui-chart` | 专业可视化独立演进；依赖树干净 | 多一个 crate |
| rgpui-ui 的 feature | 与组件生态共享 | 编译矩阵膨胀；图表依赖较重 |
| 暂缓 | 避免过早设计 | 图表能力缺失 |

**倾向**：先随 rgpui-ui 以 feature 门控迁入（保持可选），待图表需求明确后再拆独立库。**未定**。

> **已落地**（2026-08-18）：按用户决策**全量迁入** `rgpui-ui` 的 `charts` feature（`[features] charts = []`，11 文件 ~5.1k 行），见 §9 执行记录 §7.6。

### 6.2 特效类（aurora/confetti/particle_emitter/meteors/dot_pattern/noise 等装饰性组件）

| 方案 | 优点 | 缺点 |
|------|------|------|
| 精选保留 | 差异化竞争力（桌面宠物/展示场景） | 维护成本，性能风险 |
| 全部丢弃 | rgpui-ui 轻量 | 丢失差异化价值，未来要重写 |

**倾向**：精选保留（aurora/confetti/particle_emitter/marquee/shadow 高频），其余按需。**未定，待用户拍板**。

### 6.3 手势与滚动物理（gestures.rs / scroll_physics.rs）

- 需先与核心 `InteractiveElement`（点击/悬停/拖拽）与 `elements/scroll/`（Scrollable）查重。
- 若核心无惯性滚动/完整手势层 → 迁入 rgpui-ui 作辅助模块；若核心交互已覆盖 → 丢弃。
- **未定**（需勘察后决定）。

### 6.4 yororen 独有组件

keybinding_input/display、file_path_input、button_group 等 → **评估迁移或放弃**。其中 keybinding（热键录制）与核心 keymap 强相关，可能更适合并入核心（待定）。

### 6.5 代码编辑器归属（rgpui-editor）

> **现状核实（2026-08-18）**：编写 rgpui-editor 前先做了三方勘察，结论如下，**留档待议**，暂不创建 crate。

**已集成到核心**（`crates/rgpui/src/input_ui/`）：rgpui-editor → rgpui-component → 核心这条线的输入编辑器**已经全部在核心里**——`InputState` 完整文本编辑（多行、`code_editor(language)` 模式、行号、软换行、自动滚动、undo/redo、光标/选区/词移动、缩进、掩码）与 `display_map`（wrap_map 换行映射 + fold_map 折叠映射）。历史上 crates.io 发布的 `rgpui-editor 0.3.0` 因**不好维护**而合并回 rgpui-component，rgpui-component 又并回核心，其 `input/` 子系统即今日核心 `input_ui/`（core 裁剪了 otp_input/search/lsp/popovers）。

**核心未集成（缺口）**：
- **tree-sitter 语法高亮**——`display_map/folding.rs` 明示 stub（"rgpui 不引入 tree-sitter"），折叠与着色为空；但核心 `theme/highlight.rs` 已有语法高亮主题基建，可作为高亮器的样式挂钩。
- **LSP**（completions/hover/definitions/code_actions/semantic_tokens/document_colors）
- **popovers**（补全菜单/completion_menu、诊断弹窗、悬停弹窗）
- 搜索面板（search.rs）、文件读写

#### 候选路线（优略）

| 方案 | 优点 | 缺点 |
|------|------|------|
| A. 以核心 `input_ui` 为底座，**只补 tree-sitter 高亮 + 真实折叠** | 编辑器功能全在核心、零重复；增量最小；可维护 | 高亮器仍是一笔重依赖；input_ui 需开一个高亮接入点替代 folding.rs stub |
| B. 移植 adabraka `editor.rs`（4511 行，ropey+tree-sitter） | 功能全（高亮/折叠/诊断/括号匹配/补全/面包屑/文件 IO）；tree-sitter 高亮真实可用 | 与核心 input_ui 是**两套重复体系**；单文件 4511 行难维护；tree-sitter + 数十个语法 grammar 重依赖；中文注释/API 适配量大 |
| C. 整体恢复 crates.io `rgpui-editor 0.3.0`（highlighter + input + lsp + popovers） | `highlighter/` 设计成熟、34 语言 feature 门控；lsp/popovers 曾是完整方案 | 与"合并回归"历史决策相悖；input 部分与核心重复；等于重造一个重库，维护成本已被实践证明很高 |

#### 未来可能思路（推荐优先级）

1. **短期**：以核心 `input_ui` 为编辑器底座，**不新建独立 crate**。若确有语法高亮需求，做一个**独立高亮器 crate**（复用 crates.io rgpui-editor 的 `highlighter/` 模块 + tree-sitter，语言按 feature 门控），产出高亮样式喂给 input_ui 的 decoration/样式接口；核心 input_ui 补一个高亮器接入点替代 folding.rs 的 stub。这是最小增量、可维护性最好。
2. **中期**：需要 LSP/补全/诊断时，把 `lsp` + `popovers` 做成挂在 input_ui 之上的独立 crate（或 rgpui-editor 扩展），**不要恢复单体编辑器**。
3. **adabraka `editor.rs` 不再考虑**：与 input_ui 体系重复、难维护，且已有"合并回归"先例。
4. **未定**：待编辑器/代码展示需求明确后再逐项拍板。

## 7. 执行步骤（草稿）

> 当前为探讨阶段，**尚未开始迁移**。用户确认架构后方可动手。

1. 编写/确认本计划文档，逐项拍板未决项（§6）。
2. 删除三个旧库（第一阶段）：改根 Cargo.toml、删 7 个受影响 examples、清理失效 web 示例、评估 22 套主题去向、清理文档/skills。验证 `cargo check --workspace`（首次全绿）+ 测试基线。
3. 受影响 examples 处理（第二阶段）：term 两示例重写为用核心组件（独立提交，避免与删除混在一起难定位）。
4. 新建 `crates/rgpui-ui`（先搭骨架 + 动画子模块：Spring/桥接测试/animate/动画组件）。
5. 分批迁入组件（动画组件 → 特效 → 媒体/显示 → 高级输入 → 布局 → 通知/命令）。
6. 独立库论证与落地：`rgpui-markdown`（§5）、`rgpui-chart`（§6.1）、`rgpui-editor`（§8 #9）。
7. 更新 AGENTS.md、完整性检查清单、rgpui-book 文档。

## 8. 待拍板清单（汇总）

> 评审意见（2026-08-18）：依据代码勘察，各推荐项已核实（见 §4 桥接点、§3.1 共享层、§8 #9），下表"推荐"列为当前定稿倾向，**状态列待用户逐项确认**。

| # | 事项 | 推荐 | 状态 |
|---|------|------|------|
| 1 | 动画归属 | 方案 A：时间驱动留核心，弹簧+动画组件进 rgpui-ui | **已定** |
| 2 | markdown | 独立成库 `rgpui-markdown`（复用 adabraka pulldown-cmark 轻量路线） | **已定** |
| 3 | charts | 先随 rgpui-ui 作 feature，需求明确后拆独立库 | **已定** |
| 4 | 特效类 | 精选保留 aurora/confetti/particle_emitter/marquee/ripple/shimmer；dot_pattern/noise/gradient_* 静态装饰丢弃 | **已定** |
| 5 | 手势/滚动物理 | 勘察结论：核心 `InteractiveElement` 仅有点击/双击/拖拽，swipe/pan 手势与惯性滚动核心没有 → 迁入 rgpui-ui；scroll_physics 查核心 `elements/scroll/` 后决定 | **已定** |
| 6 | yororen 独有组件 | keybinding_input/display 并入核心（与 `keymap` 强相关）；button_group/combo_box 等迁 rgpui-ui 或放弃 | **已定**（实际：**不迁入**——display 被核心 `Kbd::format`、input 被 rgpui-ui `hotkey_input` 覆盖，且强依赖 yororen i18n） |
| 7 | 22 套主题 JSON | 拷贝进核心 `theme/registry.rs` 作为可选内置主题表（catppuccin/tokyonight/gruvbox 等实用性强） | **已定** |
| 8 | 受影响 examples | term 两示例重写为用核心组件；yororen 四例直接删除 | **已定** |
| 9 | adabraka `editor.rs` | **丢弃或预留独立 `rgpui-editor` 专业库，不并入 rgpui-ui**（ropey+tree-sitter 重依赖） | **已定** |

> **已预留**（2026-08-18）：`rgpui-editor` 仅作文档预留（§7.6），不创建 crate、不构建；待编辑器需求明确后单独论证落地。
> **补充留档（2026-08-18）**：论证分析已写入 §6.5（三方勘察：核心 input_ui 已集成输入编辑器、缺口仅为 tree-sitter 高亮 + 折叠 + LSP/popovers；三条候选路线优略与未来思路，推荐"以核心 input_ui 为底座、独立高亮器 crate 补 tree-sitter 高亮"）。**保持预留不构建**。

## 9. 执行记录

> 每完成一个执行步骤在此打 ☑，并简述验证结果。

| 步骤 | 内容 | 状态 |
|------|------|------|
| §7.2 | 删除三个旧库（Cargo.toml、examples、web 示例、主题、文档/skills），`cargo check --workspace` 首次全绿 | **☑ 已完成**（2026-08-18：Cargo.toml 清理、三个旧库目录删除、7 个 affected examples + 2 个失效 web 示例删除、主题拷入核心 `bundled-themes` feature、AGENTS.md/readme/上游战略/08-components/skills 清理，`cargo check --workspace` 与 `--workspace --examples` 通过） |
| §7.3 | term 两示例重写为用核心组件（独立提交） | **☑ 已完成**（2026-08-18：从 git 恢复 `examples/rgpui_term_basic` 与 `rgpui_term_component_integration`（§7.2 误删），改用核心组件并编译通过——去 `rgpui-component`/`rgpui-component-assets` 依赖；导入全部切到 `rgpui`（`Tab`/`TabBar` 在 `rgpui::tabs`，`Button`/`ButtonVariants`/`Sizable`/`AxisExt`/`IconName` 在根）；basic 移除 `init(cx)` 与 `with_assets`，用 `DefiniteLength::Fraction` + 自实现分割条替换 `h_resizable`/`v_resizable`/`resizable_panel`（`pane_ratios` + `resizing_split` 状态、`cursor_ew_resize`/`cursor_ns_resize`）；integration 改用核心主题 API（`rgpui::theme::init`、`Theme`/`ThemeMode`/`ThemeRegistry`/`ThemeColor`/`Colorize`，并开启 `bundled-themes` feature），`overflow_y_scrollbar` 用核心 `ScrollableElement`。`cargo check --workspace --examples` 全绿，示例零警告） |
| §7.4 | 新建 `crates/rgpui-ui`（骨架 + 动画子模块：Spring/桥接测试/animate/动画组件） | **进行中**（2026-08-18：骨架已建——`spring.rs`（3 个物理测试）、`easing.rs`（back/elastic 系列）、`animate.rs`（Preset/Keyframe/Stagger）、`bridge.rs`（spring→animation 桥接）；已迁 13 个动画组件：`pulse_indicator`/`shimmer`/`marquee`/`number_ticker`/`type_writer`/`text_reveal`/`animated_switch`/`animated_text`/`animated_collapsible`/`ripple`/`animated_counter`/`animated_presence`/`animated_list`/`countdown`，13 个测试全过；经验：render 返回 `AnyElement` 截断类型深度，测试用定向导入避免 recursion_limit（默认 128 足够，无需该属性）） |
| §7.5 | 分批迁入组件（动画组件 → 特效 → 媒体/显示 → 高级输入 → 布局 → 通知/命令） | **进行中**（2026-08-18：**特效批已完成**——从 git 恢复 adabraka 源码，迁入 `aurora`（`ThemeTokens.primary/accent` 是 `ThemeToken`（Deref 到 `Hsla`），用 `..*primary` 解引用；`relative(f32)`/`ease_in_out`/`with_animation` 均用核心）、`confetti`、`particle_emitter`（`canvas` 用核心 `FnOnce` 签名、`paint_quad`/`Corners::all`/`Edges::default` 均存在；删除旧 `.map()` 模式改用 `root.style().refine(&user_style)`；meteors/dot_pattern/noise 按 §8#4 决策不保留）；另修复此前批次遗留警告：`countdown`/`number_ticker` 的 Render/RenderOnce 返回 `impl trait` 与 trait 签名不匹配，已改回 `impl IntoElement`（`div().into_any_element()` → `div()`），13 测试仍全过、零警告。**媒体批（部分）**：迁入 `waveform`（155 行，纯 canvas 自绘，无外部依赖；`FluentBuilder::when` 需 `prelude::FluentBuilder as _` 导入）。**video_player（1150 行）/audio_player（899 行，依赖 rodio）暂缓**：按 §8#4 重依赖/大组件原则与用户决策，留作后续**专门视频/音频播放器组件**单独设计落地（涉及核心 svg/img/actions!/KeyBinding 等 API 适配，优先级不高）。**高级输入批已完成**——迁入 `tag_input`（284 行）、`otp_input`（505 行，`actions!`+`KeyBinding` 注册、render 体内**先提取主题值再 `self.state.update`** 避免 `cx.theme()` 借用冲突、error 用 `theme.highlight_theme.style.status.error_border(cx)`、聚焦外发光用 `BoxShadow::new(..).blur_radius` 替代旧库 `focus_ring_light`）、`hotkey_input`（328 行，`HotkeyValue::format_display` 平台相关分支保留）、`inline_edit`（815 行，含自定义 `Element`：`ElementInputHandler`/`EntityInputHandler`/`UTF16Selection`/`ShapedLine::paint` 等核心文本输入 API 全套可用；`rgpui-ui` 新增 `unicode-segmentation` 工作区依赖用于字素边界）。**暂缓**：`mention_input`（需 Avatar + scrollable + unicode_segmentation）、`search_input`/`file_upload`（依赖 adabraka 自家 Icon/Input 子系统与 `SpinnerSize`/`SpinnerVariant` 枚举，核心无）。移植经验：`BoxShadow::new`/`.bg()` 等函数参数不触发 Deref，`ThemeToken` 需 `*tok`/`tok.color` 显式转 `Hsla`；`if/else` 两分支类型必须统一（ThemeToken vs Hsla）；`root.child(..)` 消费 root 需 `root = root.child(..)`；多个组件导出 `init` 时 `pub use module::*` 触发 `ambiguous_glob_reexports`，改显式重导出 + components/mod.rs 聚合 `init(cx)`。**布局批已完成**——迁入 `split_pane`（528 行）、`resizable`（801 行，自定义 `Element`：`ResizeHandle`/`ResizePanelGroupElement`，`ResizeHandle::request_layout` 的 `with_element_state` 闭包内无 cx，须**提前提取 `theme.tokens.accent/border` 所有权值**）、`drag_drop`（344 行，`DragData`/`Draggable`/`DropZone`；match 分支统一 `(Pixels, Hsla, Hsla)`——ThemeToken 是 struct（Deref 到 Hsla），`muted` 作 `bg` 需 `theme.tokens.muted.color`）、`sortable_list`（214 行）、`view_router`（292 行）、`canvas_component`（82 行）、`segmented_nav`（248 行，render 体内先 `self.state.update` 再 `cx.theme()` 避免借用冲突）、`expandable_card`（194 行）、`layout_transition`（144 行）、`empty_state`（215 行，`IconSource`→核心 `Icon`、`Button::new(id).label(..)`、无 `.variant()`/`.color()` 方法，改 `.ghost()` 便捷方法与 `.with_size()`/`.text_color()`）、`infinite_scroll`（234 行，旧库 `Spinner`→核心 `Spinner::new()`、`theme.tokens.destructive`→`theme.highlight_theme.style.status.error_border(cx)`）。核心 API 核实用法：`canvas` 为 `FnOnce`（prepaint 3 参、paint 4 参）、`on_drag` 闭包签名 `(&T, Point<Pixels>, &mut Window, &mut App)`、`drag_over::<S>` 闭包签名 `(StyleRefinement, &S, &mut Window, &mut App)`、`relative(f32)`（geometry.rs:3705）、`px()` 是 const fn、`Animation::new(duration).with_easing(easings::ease_out_cubic)`（`use crate::animation::easing::easings`）。`cargo check -p rgpui-ui` 零警告、13 测试全过、`cargo check --workspace` 与 `--workspace --examples` 全绿。**通知/命令批已完成**——迁入 `spotlight`（聚光灯，跟随鼠标的圆形高亮光斑）、`app_menu`（`AppMenuBar`/`AppMenu`/`StandardMacMenuBar` + file_menu/edit_menu/view_menu/window_menu/help_menu 便捷函数；核心 `Menu{name,items,disabled}` 字段直接构造、`MenuItem::action/separator/submenu/os_submenu`、`SystemMenuType::Services`；与核心 `rgpui::AppMenuBar`（macOS 系统菜单）重名，下游同时 glob 两 crate 时需路径限定）、`drawer_navigation`（抽屉导航，`deferred(child).with_priority(usize)` 延迟渲染提升层级）、`bottom_sheet`（底部弹层；rgpui-ui `animate.rs` 无 `slide_in_bottom`/`presets` 模块，改用 `Animation::new(Duration::from_millis(250)).with_easing(rgpui::ease_out_cubic)`）、`navigation_menu`（泛型递归侧边菜单；`IconSource`→核心 `Icon`、`IconName::ArrowDown/ArrowRight` 枚举变体、`&rgpui::Theme` 参数传递）、`command_palette`（命令面板；核心 `input_ui::Input/InputState/InputEvent` 全套：`InputState::new(window, cx).placeholder(..)` 构造、`cx.subscribe` 订阅 `InputEvent::Change` 实时过滤、`actions!` 快捷键导航；RenderOnce 无 `cx.listener`（仅 `Context` 有），改克隆 state/on_close 后闭包捕获；`App::notify` 需 `EntityId` 参数，改在 `Entity::update` 闭包内用 `Context::notify()` 无参版本）、`notification_center`（`NotificationCenter`/`NotificationBell`/`NotificationItem` 通知中心与铃铛徽标；复用本批迁入的 `EmptyState`；`Button::new(id).label(..).ghost()/.outline().small()`；徽标红用 `theme.tokens.danger/danger_foreground`）。核心差异记录：`Icon::new` 收 1 参（`IconName`/`Icon`）、`Button::new` 只收 id + `.label(..)`、`popover` 令牌替代 `card`、`theme.radius/radius_lg` 替代 `radius_sm/md/xl` 令牌、`theme.font_family` 为字段非令牌、`rgba(hex)`→`Background`、大阴影手写 `BoxShadow{color,offset,blur_radius,spread_radius,inset}`。`cargo check -p rgpui-ui` 零警告、13 测试全过、`cargo fmt -p rgpui-ui`、`cargo check --workspace` 与 `--workspace --examples` 全绿。**显示批已完成**——迁入 `qr_code`（workspace 依赖加 `qrcode = "0.14"`，canvas 绘制，`*theme.tokens.foreground/background` 解引用）、`sparkline`（Line/Bar/Area 变体 + 趋势，纯 canvas）、`svg_renderer`（解析 M/L/H/V/C/Q/Z path 数据经核心 PathBuilder 绘制）、`image_viewer`（ImageViewer/ImageViewerState/ImageItem，适配核心 Button/IconName——zoom 图标无、映射 Plus/Minus，箭头 ArrowLeft/ArrowRight，x→Close；`init_image_viewer` 注册 9 条 KeyBinding）；**放弃 code_block/rich_text**——与 `rgpui-markdown` 公开 API（`CodeBlock`/`RichBlock`/`ListItem` 等）完全重叠。commit `95362b6358`。**工具批已完成**——`gestures`（GestureDetector 状态机 Tap/双击/Pan/Swipe/LongPress + GestureEvent；原文件无测试，尝试补测触发 rustc 宏递归栈溢出，按原样不设测试）与 `scroll_physics`（ScrollPhysics，补 4 个物理测试，修复原缺陷：overscroll 回弹几何级数不收敛 → 越界 <0.5 钳制到边界清零速度）迁到 crate 根模块（非 components/），lib.rs 注册 `pub mod` + re-export。**余项拍板**：yororen keybinding_input/display **不迁移**——display 被核心 `Kbd::format`（macOS/Win 全符号映射）、input 被 rgpui-ui `hotkey_input`（录制 + format_display）覆盖且两者强依赖 yororen i18n；`animated_progress` **迁入**（自包含，核心无 Progress，自带 AnimatedProgressSize/Variant，`theme.radius_lg` + `tokens.danger`）；`carousel`/`tilt_card`/`magnetic_button` **放弃**——tilt/magnetic 为含 `#[allow(dead_code)]` 的视觉玩具、carousel 依赖 core 无的 `focus_ring_light`/RenderOnce spawn 模式，适配成本高且核心 tabs/动画已覆盖基本交互。**§7.5 组件分批迁入全部完成**） |
| §7.6 | 独立库落地：`rgpui-markdown`、`rgpui-chart`（feature 门控）、`rgpui-editor`（预留） | **进行中**（2026-08-18：**rgpui-markdown 已落地**——新建 `crates/rgpui-markdown`（`src/lib.rs`+`markdown.rs`+`rich_text.rs`+`code_block.rs`），复用 adabraka pulldown-cmark 0.12 轻量路线；工作区依赖加 `pulldown-cmark = "0.12"`（`Tag::Table(Vec<Alignment>)` 签名匹配）；移植适配：去全部 `#[cfg(feature="markdown")]` 门控、`use_theme()`→`cx.theme()`、`theme.tokens.font_family/font_mono`→`theme.font_family`/`theme.mono_font_family` 字段、`radius_md/sm`→`theme.radius`、`Separator::new()`→`Separator::horizontal()`、`text_right()`→`.text_align(TextAlign::Right)`、TextVariant→本地 `heading_style()` 尺寸换算（H1=32 BOLD…H6=16 MEDIUM）、rich_text 渲染函数 thread `theme: &Theme` 参数（核心无全局 `use_theme()`）；code_block 用 `StyledText`/`InteractiveText::on_click`+`cx.open_url` 处理链接、`ClipboardItem::new_string`+`cx.write_to_clipboard` 复制、简易 Rust 关键字/字符串/注释/数字分词。`cargo check -p rgpui-markdown`、`cargo check --workspace`、`cargo check --workspace --examples` 全绿零警告。**rgpui-chart 已落地**——按用户决策**全量迁入** `rgpui-ui` 的 `charts` feature（非独立 crate）：`Cargo.toml` 加 `[features] charts = []`，`components/mod.rs` 加 `#[cfg(feature="charts")] pub mod charts;` + `pub use charts::*;`；迁入 11 文件共 ~5.1k 行：`chart.rs`（地基，Axis/Chart/Series/Legend/Tooltip + canvas 绘制与悬停 tooltip）、`line_chart`、`bar_chart`（单/多系列、分组/堆叠、横/纵）、`area_chart`（叠加/堆叠）、`pie_chart`（点阵近似扇区 + 图例）、`gauge`、`heatmap`、`radar_chart`、`treemap`（squarify 布局 + `window.paint_quad` + `shape_line`/`ShapedLine::paint`）、`donut_chart`（点阵环 + 中心标签，复用 `super::pie_chart::PieChartSegment`）。移植统一适配：去 `use_theme()`→`cx.theme()`、自由渲染函数 thread `&Theme` 参数、`theme.tokens.border` 需 Hsla 处取 `.color`、`radius_sm`→`theme.radius`、`text_right()`→`.text_align(TextAlign::Right)`、删除未用字段/下划线 `_id` 保留（禁 `#[allow(dead_code)]`）；treemap 矩形描边 `PaintQuad.border_color` 用 `theme.tokens.background.color.into()`、反色对比 `rect.color.l > 0.5`、`ShapedLine::paint` 6 参签名（origin, font_size, align, align_width=None, window, cx）。`cargo check -p rgpui-ui --features charts`/默认/`--all-features`/`--workspace`/`--workspace --examples` 全绿零警告、`cargo fmt -p rgpui-ui`。**rgpui-editor 保持预留不构建**——论证留档于 §6.5：核实核心 `input_ui`（源自 crates.io `rgpui-editor 0.3.0` → rgpui-component → 核心）已含完整输入编辑器（`InputState`/display_map/undo 等），缺口仅为 tree-sitter 语法高亮 + 真实折叠（folding.rs 为 stub）+ LSP/popovers；adabraka `editor.rs` 与 input_ui 为两套重复体系且 4511 行难维护、已有"合并回归"先例，不再考虑；推荐"以核心 input_ui 为底座、独立高亮器 crate 补 tree-sitter 高亮"，待需求明确后处理） |
| §7.7 | 更新 AGENTS.md、完整性检查清单、rgpui-book 文档 | **☑ 已完成**（2026-08-18：AGENTS.md 完整性清单补 #19（rgpui-ui 迁移全部完成，含显示/工具批、animated_progress 迁入、yororen keybinding 不迁、carousel/tilt_card/magnetic_button 放弃）；`docs/rgpui-book/08-components.md` 补「扩展库（rgpui-ui / rgpui-markdown）」章节（内容/引入方式表、init(cx) 快捷键注册、迁移规则、gestures/scroll_physics 位于 crate 根）；§9 §7.5 行补显示批 + 工具批 + 余项拍板详情） |
| §7.8 | **rgpui-ui 并入核心（删除该 crate）** | **☑ 已完成**（2026-08-19：按用户决策将 `crates/rgpui-ui` 全部内容并入 `crates/rgpui`，删除 rgpui-ui crate。执行：① `git mv` 保留历史——`components/`→`rgpui/src/components/`（含 charts/）、`animation/`→`rgpui/src/animation/`、`gestures.rs`→`rgpui/src/mouse_gestures.rs`、`scroll_physics.rs`→`rgpui/src/scroll_physics.rs`；② 全局替换 `use rgpui::`→`use crate::` 及 `rgpui::`→`crate::`；③ `rgpui.rs` 注册 `pub mod animation/components/mouse_gestures/scroll_physics`（与 `input_ui`/`table` 一致用 `pub mod` + 内部 `pub use`，不在根级 glob 展开，规避 AppMenuBar/Axis/GestureEvent/SVGRenderer/TreeMap/Command/init 七项命名冲突）；④ feature 平移：核心 `Cargo.toml` 加 `charts`/`effects`/`qr-code`（`qr-code = ["dep:qrcode"]`，`qrcode` 改 optional），`components/mod.rs` 加 `#[cfg(feature = "effects")]`（aurora/confetti/particle_emitter/ripple/shimmer/marquee/pulse_indicator）与 `#[cfg(feature = "qr-code")]`（qr_code）；⑤ `git rm` 删除 `crates/rgpui-ui`，清理 workspace members 与依赖；⑥ 补文档注释修复核心 `missing_docs` 警告（otp_input 6 变体、animation/spring、image_viewer 未用导入）；⑦ 更新 AGENTS.md（架构图/整合原则/组件库表/完整性清单 #17、#19）与 `docs/rgpui-book/08-components.md`（「扩展库」→「扩展组件（并入核心）」）。验证：`cargo check -p rgpui`（默认/`--features charts,effects,qr-code`）零警告、`cargo check --workspace` 全绿零警告） |