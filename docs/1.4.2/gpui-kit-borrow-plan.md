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
| 1 | a11y 标注补齐（checkbox / radio / slider 等） | **已做** | `CheckBox` / `RadioButton`+`RadioGroup` / `Slider` 四件，gpui-kit 同款口径，详见新§七 |
| 2 | 尊重系统「减少动态效果」 | **已做** | 四平台读取 + 核心层写入点，详见下节 |
| 3 | 编辑器接通 Code Action | **已做** | 模块 + 键盘 + 浮层 + 示例调用点已落地；四个单测已过，详见下节 |
| 4 | `scroll_physics` 接进滚动层 | **已做** | opt-in `Overscroll` 包装 + `Scrollable::overscroll`，只做视觉位移，详见新§八 |
| 5 | CI 卫生（typos / cargo-machete / feature 组合矩阵 / "Platform 方法必须有调用点"检查） | **已做** | 缺 `.rustfmt.toml`（故意不加，见§九）；另顺手删掉 13 个无调用点死方法 |

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

## 四、第 3 项：Code Action（已完成）

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

**收尾记录（卡点已解）**：

- 症状：`cargo test -p rgpui --features editor --lib code_action` 四条全红，
  请求后 `code_action_menu_active()` 仍为 `false`。根因是 `request_code_actions`
  先取光标处布局边界当锚点、取不到就 return（防首绘前弹到左上角），而
  `cx.add_window_view` 建的测试窗口在显式绘制前 `last_layout` 为空；且 `Probe::render`
  只返回空 `div`，内部 `Input` 根本没参与布局。
- 修法（已落地）：测试 `Probe` 改为渲染 `Editor`（minimap 测试同款
  `Editor::new(&state).render(window, cx).into_element()`），`probe_with_provider`
  里加 `input_ui::init` + `theme::init` 并经 `cx.update(|window, cx| window.draw(cx))`
  强制绘制一帧后再请求。注意 `VisualTestContext` 没有无参 `draw()`，
  正确姿势是 `window.draw(cx)`（`input/widget.rs` 的 a11y 测试即此写法）。
- 顺手修：`cargo check --workspace` 暴露示例 `editor.rs` 的 `is_preferred` 类型错
  （闭包参数缺 `Option<bool>` 标注，两处 `false` 改 `None`）；`cargo clippy -p
  v1_2_showcase`（该特性组合才编译到 editor 模块）报 `sort_by` 改 `sort_by_key` +
  `editor_ui.rs` 末次 `clone` 改 move。`cargo test -p rgpui --lib` 638 passed，
  `cargo check --workspace`、`cargo fmt --all`、两包 clippy 全绿。

## 五、第 4 项：scroll_physics（已完成）

- 现状曾是：`scroll_physics.rs` 有 `ScrollPhysics` 实现，滚动层零调用。
- 落地（opt-in，只做视觉位移，不动逻辑钳制）：
  - 新文件 `elements/scroll/overscroll.rs`：`Overscroll<E>` 包装器，根为
    `size_full` + `relative` 偏移 `div`；状态 `Rc<RefCell>`（抄 `scrollbar.rs`
    的 `ScrollbarState` 模板，滚轮监听里只有 `&mut App`），经
    `use_keyed_state` 跨帧存活。
  - 物理即 `ScrollPhysics` 本体：每轴一个，边界 `[0, 0]`，位置即视觉位移。
    冒泡监听里内层已先消费，用"钳制后逻辑位没动"判定到边（事件时刻的偏移是
    瞬时超界值，下次 prepaint 才钳回——此前按原始偏移判定永远到不了边）；
    到边且增量指向界外时 `apply_delta(增量×0.35)` 并钳制 ±96px。
  - 回弹：render 里 `settling()` 为真即 `tick(1/60)` 一帧 +
    `request_animation_frame()`，收敛自动停；`reduce_motion` 下跳过拉伸
    （与第 2 项联动，gpui-kit 同款语义）。
  - `Scrollable::overscroll(bool)`（默认关，零行为变化）把滚动区包一层，
    共享跟踪句柄做边缘判定；`gradient` 示例开了一处当真实调用点。
  - 触摸与滚轮走同一条 `ScrollWheelEvent` 路径（触摸以带 `touch_phase` 的
    滚轮事件呈现）；`Cancelled` 直接清零。
- v1 边界（模块头注明）：固定步长回弹；无方向锁/动量抑制；逻辑滚回去时视觉
  直接清零（有一帧跳变，以后再做跟手释放）；包装根固定 `size_full`。
- 测试：`overscroll_pulls_and_settles`（拉伸→方向/上限→300 帧归位）、
  `overscroll_disabled_by_default`、`overscroll_keeps_normal_scroll`
  （逻辑位移不被回弹吃掉）。注意测试里逐帧推进必须经
  `window.simulate_next_frame` 交付 `request_animation_frame` 回调，
  光 `window.draw` 的话 view 不脏、render 不重跑（踩坑记录）。
- 踩坑记录：初版按事件时刻原始偏移判定到边，内层瞬时超界（如 +50）导致
  `moved` 恒为真，拉伸永远触发不了——打印 `offset/max` 一眼看到。

## 六、第 1 项：a11y 标注补齐（已完成）

- 口径照抄 gpui-kit（`temp/gpui-kit/crates/base/src/{checkbox,radio,radio_group,slider}.rs`）：
  - `Checkbox`：`Role::CheckBox` + `aria_toggled` + label；禁用态不挂点击，
    辅助技术就不提供激活动作（与 gpui-kit 断言一致）。
  - `Radio`：`Role::RadioButton` + `aria_toggled` + `aria_selected`（gpui-kit
    注释：不同辅助技术各读一种，两边都报）+ label；`RadioGroup` 容器报
    `Role::RadioGroup` + 布局方向。
  - `Slider`：`Role::Slider` + 数值/最小/最大/步长 + 方向 +
    `Increment`/`Decrement` 动作（按步长改值，走 `set_value`）。
- 新增 `SliderState::min_value/max_value/step_value` 取值器（gpui-kit 同名，
  避开 builder 风格的 `min/max/step`  setters）。
- 渲染树零变化：checkbox/radio 只是把原来外层 `div().child(内层链)` 拆成
  `render() → div().child(render_box())`，`render_box` 返回带标注的内层盒子，
  单测直断 role/label/toggled 不必钻树。
- 测试写法抄 gpui-kit：`canvas` 探针 + `window.draw`（直接调 render 会撞
  `current_view()` 的 prepaint/paint 断言，`use_keyed_state` 要渲染栈）。
- `Switch` 本来就有，`Toggle` 包 `Button` 间接继承，都不动。

## 七、第 5 项：CI 卫生（已完成）

- `typos`：根 `_typos.toml`（`scap/ags/lod/tme/ptd/nd/ba/numer/sur` 九个
  人工核对过的合法词 + `temp/*`、`target/*` 排除）+ CI hygiene job。
  落地时顺手修了三处拼写（directx_renderer 注释、sticky_scroll 注释、screen_capture 注释）。
- `cargo-machete`：CI hygiene job；落地把 13 个死依赖全删了
  （`rgpui-dom` 的 anyhow/log、`rgpui-term` 的 parking_lot/portable-pty/
  smallvec/thiserror，以及 6 个示例包的零散依赖），本地复跑干净。
- feature 组合：CI 矩阵 job 内加 `cargo check -p rgpui --features
  editor,tokio,charts,effects,qr-code,dom-backend,tree-sitter,tree-sitter-json,tree-sitter-toml`
  （三平台）+ `webview`（仅 Linux，要系统库）+ `cargo test -p rgpui-dom`。
  `scap`/`screen-capture` 照 AGENTS.md 不加（已知编译失败）。
- "Platform 方法必须有调用点"：`.github/scripts/check_platform_calls.py`
  （1.6s）+ CI hygiene job + 空 allowlist。口径是"除定义行外至少一处调用"
  （包装/core 内调/示例/后端内调都算，`temp/` 不算）。
  - triage 干掉了 13 个无调用点死方法 + 5 个连带孤儿类型
    （`AttentionType`/`DialogOptions`/`DialogType`/`BiometricStatus`/`MediaKeyEvent`）：
    麦克风 ×2、生物识别 ×2、媒体键、网络回调、注意力 ×2（真接线走
    `PlatformWindow::request_attention`，不是这俩）、Dock 徽章、原生弹窗、
    扩展样式 ×2、`a11y_update_window_bounds`（含 x11 那段 30 行实现）。
    全是当年批量补 trait 凑完整性塞进来的（`git log -S` 可查），从没人调。
  - `get_raw_handle` 一度误删：Windows `open_window` 登记 HWND 时真在调，
    属于后端内部正当调用——已恢复，检查语义也因此从"后端不算"改成
    "定义行不算"，并补了该条注释。
  - AGENTS.md 平台 API 清单同步更新（删掉已删项；麦克风/生物识别特例条款
    作废——特例方法本身已删，统一权限口径现在完全成立）。
- `.rustfmt.toml` 故意不加：加了等于全仓重排，噪音巨大；现有逐包
  `cargo fmt --check` 已够用。
- 计划外附带：全量验证时发现无头 Windows 上 `rgpui-wgpu` 测试进程崩溃
  （`STATUS_ACCESS_VIOLATION`，干净树同样复现，非本分支引入）。二分到
  `wgpu_atlas` 两测试 → `test_device_and_queue` → `request_device` 内：
  本机 Intel 核显走 Vulkan 即崩，走 DX12 正常。修法：测试 helper 在 Windows
  固定 `Backends::DX12`，其它平台不动（CI 全绿处不 churn），另留
  `RGPUI_TEST_BACKENDS` 环境变量逃生口。26 passed。

## 八、验证口径

- Windows 本机：`cargo check --workspace`（根 + examples 双 workspace）、
  `cargo test -p rgpui --lib`（644 passed）、改动包 clippy、`cargo fmt --all`、
  `typos` / `cargo machete` / `check_platform_calls.py` 全绿。
- macOS / Linux 的 `cfg` 代码（macOS 后端、Linux x11/wayland 删除点）本机不编译，
  靠 CI 三平台矩阵兜（AGENTS.md 已注明）；feature 组合 CI 覆盖见§七。
- 分支 `feat/a11y-motion-and-code-action`，main 受保护，合入走 PR（Squash + Conventional Commits）。
