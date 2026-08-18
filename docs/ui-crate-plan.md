# rgpui UI 库重组计划（rgpui-ui）

> 文档日期：2026-08-18
> 状态：战略探讨，待评审决策（含未决项）
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

### 3.2 明确不迁入

- 核心已有的全部基础组件（component/adabraka/yororen 三库中的 button、input、checkbox、radio、switch、slider、select、dialog、menu、tabs、table、avatar、alert、pagination、color_picker、rating、stepper、resizable、dock、status_bar、tree、calendar、date_picker、time_picker、sheet、badge、kbd、tooltip、separator、progress、spinner、skeleton、form、textarea 等）。
- yororen 独有的 keybinding_input/display、file_path_input、button_group、combo_box、dropdown_menu、disclosure、focus_ring —— **评估迁移或放弃**（部分可能并入核心或 rgpui-ui，待定）。

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

## 6. 其他待决项与优劣

### 6.1 图表（charts/）

| 方案 | 优点 | 缺点 |
|------|------|------|
| 独立成库 `rgpui-chart` | 专业可视化独立演进；依赖树干净 | 多一个 crate |
| rgpui-ui 的 feature | 与组件生态共享 | 编译矩阵膨胀；图表依赖较重 |
| 暂缓 | 避免过早设计 | 图表能力缺失 |

**倾向**：先随 rgpui-ui 以 feature 门控迁入（保持可选），待图表需求明确后再拆独立库。**未定**。

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

## 7. 执行步骤（草稿）

> 当前为探讨阶段，**尚未开始迁移**。用户确认架构后方可动手。

1. 编写/确认本计划文档，逐项拍板未决项（§6）。
2. 删除三个旧库：改根 Cargo.toml、删 7 个受影响 examples、清理失效 web 示例、评估 22 套主题去向、清理文档/skills。验证 `cargo check --workspace`（首次全绿）+ 测试基线。
3. 新建 `crates/rgpui-ui`（先搭骨架 + 动画子模块：Spring/animate/动画组件）。
4. 分批迁入组件（动画组件 → 特效 → 媒体/显示 → 高级输入 → 布局 → 通知/命令）。
5. 独立库论证与落地：`rgpui-markdown`（§5）、`rgpui-chart`（§6.1）。
6. 更新 AGENTS.md、完整性检查清单、rgpui-book 文档。

## 8. 待拍板清单（汇总）

| # | 事项 | 推荐 | 状态 |
|---|------|------|------|
| 1 | 动画归属 | 方案 A：时间驱动留核心，弹簧+动画组件进 rgpui-ui | 待定 |
| 2 | markdown | 独立成库 `rgpui-markdown` | 待定 |
| 3 | charts | 先随 rgpui-ui 作 feature，需求明确后拆独立库 | 待定 |
| 4 | 特效类 | 精选保留 | 待定 |
| 5 | 手势/滚动物理 | 查重后迁入或丢弃 | 待定 |
| 6 | yororen 独有组件 | 评估迁移或放弃（keybinding 倾向并入核心） | 待定 |
| 7 | 22 套主题 JSON | 拷入核心/新库或放弃 | 待定 |
| 8 | 受影响 examples | 删除或重写（term 两示例评估用核心组件） | 待定 |