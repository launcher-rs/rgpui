# 1.4.2 借鉴 gpui-kit 落地计划

> 起因：对照 `temp/gpui-kit`（longbridge GPUI Kit）盘点 rgpui 值得借鉴的东西。
> 结论是**不全面学习**——组件层同质、抄过来价值最低；只挑 rgpui 自己有 machinery
> 却没接线的缺口。本文档记录范围、每项状态（已实现 / 未实现）、涉及文件与已知边界，
> 供 PR 描述与后续版本归档用。

## 一、时间线与判断依据

- gpui-kit 在 **2026-08-13** 引入 `crates/base` 三层拆分；rgpui 的组件移植落在
  **2026-08-16**。两者同源，此后 gpui-kit 的 487 次提交才是"可借鉴"的真实范围
  （热点：input 37、website+docs 58、shell 23、base 21、text_view 21）。
- 反向核对桌面平台能力（mouse_passthrough / request_attention / single_instance /
  global_hotkey / AutoLaunch / 媒体键 / 电源事件 / 生物识别 / 托盘）：gpui-kit **全部为 0**，
  这是 rgpui 的护城河，不在借鉴范围内。
- 编辑器子系统（vim / minimap / codelens / snippets / inlay / sticky / gutter / stdio LSP）
  rgpui 已超出 gpui-kit，同样不抄。

## 二、五项清单与状态

| # | 项 | 状态 | 说明 |
|---|----|------|------|
| 1 | a11y 标注补齐（checkbox / radio / slider 等） | **未做** | 基础设施 rgpui 已有（`write_a11y_info` / `A11ySubtreeBuilder`），实测已标注文件 **7** 个（gpui-kit base 为 31）；纯增量，另开 PR |
| 2 | 尊重系统「减少动态效果」 | **已做** | 四平台读取 + 核心层写入点，详见下节 |
| 3 | 编辑器接通 Code Action | **进行中** | 模块 + 键盘 + 浮层 + 示例调用点已落地；四个单测当前**未过**，详见下节"当前卡点" |
| 4 | `scroll_physics` 接进滚动层 | **未做** |  Overscroll 拉伸回弹；参考 gpui-kit `base/src/scroll_bounce.rs`（1058 行，挂 touch + OngoingScroll），rgpui 侧只有 `scroll_physics.rs` 没人调 |
| 5 | CI 卫生（typos / cargo-machete / `.rustfmt.toml` / feature 组合矩阵 / "Platform 方法必须有调用点"检查） | **未做** | 另开 PR；最后一项是针对本仓库反复出现的"实现了没人调"缺陷 |

明确**不借鉴**：`base`/`component` crate 物理拆分、gpui-shell（QuickJS）、分版本文档站、
headless UI 测试层、PaneTree dock 引擎（只抄概念不抄体量）、自动更新与打包。
`accessibility_id` 也不抄——它只在 gpui-kit `crates/component`（5 文件），`crates/base` 为 0。

## 三、第 2 项：reduced-motion（已完成）

**根因**：`App::reduce_motion` 只有读方（装饰性动画元素绘制期读），**没有任何写方**——
`set_reduce_motion` 存在但没人调用，系统设置完全没接。

**实现**：

- `Platform::reduce_motion_enabled()`（默认 `false`，未覆写的平台保持动画）。
- 四平台读取：
  - Windows：`SPI_GETCLIENTAREAANIMATION`（`rgpui-windows/src/system_settings.rs`
    `client_area_animation_enabled`，读取失败按"保持动画"处理，不误判成用户要求减动）。
  - macOS：`NSWorkspace.accessibilityDisplayShouldReduceMotion`。
  - Web：`match_media("(prefers-reduced-motion: reduce)")`。
  - Linux：XDG 门户 `org.freedesktop.appearance`/`animations-disabled`，
    缺失时回退 GNOME `enable-animations`；门户事件新增 `Event::ReduceMotion`，
    x11 / wayland 两个 client 各接一条臂写入 `LinuxCommon::reduce_motion`。
- 核心层写入点：`App::sync_reduce_motion_from_platform()`（私有），**每次创建窗口时调用**；
  `App::set_reduce_motion` 置 `reduce_motion_override` 标记，应用显式指定后不再被系统值改写。

**已知边界**：Linux 门户值异步到达，首窗可能仍带动画；Windows/macOS 改设置需新建窗口才生效
（不做运行时热切，那要求订阅 WM_SETTINGCHANGE / DistributedNotificationCenter 并刷动画）。

**踩坑记录**：第一版无条件同步，把 `set_reduce_motion(true)` 的测试用例覆盖掉了
（`test_reduce_motion_renders_single_static_frame` 失败）——加 override 锁修复，
本地 `cargo test -p rgpui` 638 passed。

## 四、第 3 项：Code Action（已接线，收尾中）

**根因**：`LspClient::code_actions`（`lsp/types.rs:101`）与 stdio 传输
（`lsp/stdio.rs:544`）**都在**，缺的是编辑器侧调用点——全仓 0 处调用。
（此前有一次调研把它误报成"rgpui 无 code_actions"，据实更正。）

**实现**：

- 新模块 `crates/rgpui/src/input_ui/editor/code_actions.rs`，与 `codelens.rs` 同款结构：
  - `CodeActionProvider` trait（默认空实现，不断编译）：入参当前文档快照、选区
    （UTF-8 字节区间）、反查成 LSP 形态的诊断列表。
  - `EditorState`：`set_code_action_provider` / `request_code_actions` /
    `toggle_code_actions` / `accept_code_action` / `dismiss_code_actions` /
    `select_next_code_action` / `select_previous_code_action` /
    `code_action_menu_active` / `code_actions()`。
  - epoch 作废在途请求（用户显式触发，不设防抖）。
  - 编辑应用：按起始偏移**倒序**落笔（改后面的不挪前面的字节位置），
    LSP 行列按 UTF-8 字节列换算（与诊断下划线同口径），应用后光标回原位。
  - 浮层 `render_code_action_menu` 由 `Editor` **自带渲染**（`deferred` + `anchored` +
    `snap_to_window_with_margin`，补全弹窗同款定位）；不可见时零开销。
- 键盘：新增动作 `ToggleCodeActions` + 键位 `alt-enter`（`Input` 上下文）；
  `editor_ui.rs` 的 ↑↓ / Enter / Esc 四个 capture 处理器改为
  "补全菜单优先，其次修复菜单"，命中才 `stop_propagation`。
- 文本变更即收起菜单（`EditorState::new` 的 `subscribe_in(Change)` 链里补一步）。
- `lsp_attach.rs` 补公开访问器 `EditorState::document_uri()`（修复应用要按 URI 挑本文档的编辑）。
- 示例调用点：`examples/v1_2_showcase/src/bin/editor.rs` 的 `DemoCodeActionProvider`
  （行首加注释 / 选区替换 / 有诊断时首选「标注首条诊断」），满足
  "加能力必须有真实调用点"的仓库规矩。
- 测试：`code_action_request_shows_menu` / `code_action_accept_edits_text` /
  `code_action_disconnect_clears` / `code_action_text_change_dismisses`。

**v1 边界（注释即契约）**：

- 只应用**当前文档**的文本编辑；多文件 `documentChanges` 里别家文件的改动忽略；
  未设 `document_uri` 时，仅当编辑只涉及一个文档才应用。
- 命令型动作（只有 `command`、没有 `edit`）不进框架执行：列出标题，应用时等于收起；
  server 自定义命令由应用层自行分发。
- 文件创建/重命名/删除类 `documentChanges` 操作不接。
- 布局未就绪（首绘前）请求直接忽略，避免菜单弹到左上角。
- 诊断反查为 LSP 形态时 `code` 以字符串回传，数字编码原形丢失。

**当前卡点（本次推送未完成的部分）**：

- `cargo test -p rgpui --features editor --lib code_action` 四条全红：
  `code_action_request_shows_menu` / `code_action_accept_edits_text` /
  `code_action_disconnect_clears` / `code_action_text_change_dismisses`。
- 症状一致——请求后 `code_action_menu_active()` 仍为 `false`，菜单从没弹开过。
- 判断：`request_code_actions` 先取光标处布局边界当锚点，**取不到就 return**
  （防首绘前弹到左上角）。`cx.add_window_view` 建的测试窗口在显式绘制前
  `last_layout` 为空，于是请求被自己拦掉。实现路径大概率是对的，是测试没 draw。
- 修法（下次第一件事）：测试里先 `cx.draw()`（`VisualTestContext` 有）再请求；
  顺手确认 `Probe` 视图把 `Editor` 挂进渲染树——现在 `Probe::render` 只返回空 `div`，
  内部 `Input` 根本没参与布局，这可能才是 `last_layout` 为空的真因。
  两条都验证后，`cargo test -p rgpui` 全绿再进 PR。
- 其余待办：`cargo check --workspace` / `cargo fmt --all` / 改动包 clippy 复跑；
  CHANGELOG 第 3 项条目已写但**未经全绿验证**，合入前复核。

## 五、第 4 项：scroll_physics（未开始）

- 现状：`rgpui/src/scroll_physics.rs` 有 `ScrollPhysics` 实现，滚动层零调用。
- 目标：overscroll 拉伸 + 回弹接到 `elements/scroll/`，触摸与滚轮两条路径。
- 参考：gpui-kit `crates/base/src/scroll_bounce.rs`（1058 行，挂在 touch 事件 +
  `OngoingScroll` 上）——**只借思路，不搬体量**，rgpui 已有自己的滚动元素与物理模块。
- 风险：改滚动层牵动所有可滚动组件，需要三平台矩阵验证；本项建议单独 PR。

## 六、验证口径

- Windows 本机：`cargo check --workspace`、`cargo test -p rgpui`、改动包 clippy、`cargo fmt --all`。
- macOS / Linux / Web 的 `cfg` 代码本机不编译，靠 CI 三平台矩阵兜（AGENTS.md 已注明）。
- 分支 `feat/a11y-motion-and-code-action`，main 受保护，合入走 PR（Squash + Conventional Commits）。
