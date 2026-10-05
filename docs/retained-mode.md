# Retained Mode（保留模式）

> 状态：已合入 `main`（PR #23，squash）。本文描述保留模式相对合入前的行为变化、
> 实测收益与应用层契约。数字来自 headless release 基准（见 §5），
> 与机器有关，读趋势不读绝对值。

## 1. 背景：原来每一帧都在全量重做

合入前，绘制是 immediate mode：每一帧走完三个阶段——

1. **request_layout**：渲染所有视图（含 `render()`），申请布局；
2. **prepaint**：计算布局、放置元素、注册 hitbox／dispatch／监听器；
3. **paint**：把图元写入场景交给 GPU。

即使只有一个标签的文本变了，整棵树三个阶段全部重跑，开销随场景规模线性增长。
上游 GPUI 唯一的例外是显式 `cached` 视图。实现时借鉴 gpui-fast
（https://github.com/longbridge/gpui-fast）的思路，
把“干净子树直接复用上帧输出”做成框架默认行为，无需应用逐个标注 `cached`。

## 2. 做了什么

| 阶段 | 内容 |
|------|------|
| P0 | `fast/` 隔离模块 + `FrameStats` 单窗帧统计（计数零行为变更） |
| P1 | 依赖追踪：实体 `Generation`、全局版本号／存在性（`has_global` 只记存在性）、`StateVersion`（滚动句柄等 Rc 共享状态） |
| P1b | `ScrollHandle`／`ListState` 等状态写入递增版本号，读路径打戳 |
| P2a | `ViewElement` prepaint／paint 复用：命中即丢弃本帧渲染产物，重放上帧区间（hitbox、dispatch 子树、监听器、场景、文本布局、调试边界） |
| P2c | oracle 对等校验（双窗逐帧比对 `Scene`＋hitbox＋调试边界）＋ `retention_override` 开关 |
| P2d | headless 双窗对照基准（60 面板 × 64 标签仪表盘，见 §5） |
| P3a | Taffy 布局节点跨帧保留（路径键 `LayoutKey`，样式／子节点／测量三重比对，命中即不写，Taffy 自身缓存存活）＋ 文本测量 carry（指纹一致直接交还旧测量） |
| 加固 | `frame_seq` 范围守卫（只信任上一帧区间）、sweep 按认领节点存活、`focus_generation` 焦点守卫 |

复用判定链（`crates/rgpui/src/view.rs`，全真才复用，只会多重建、不会复用过期帧）：

- 包围盒／content mask／文本样式一致；
- 不在 `dirty_views`（被 `notify` 脏标记的视图及祖先除外）；
- 非 refresh-draw（`refresh()` 语义即全量重执行，见 §4）；
- 焦点代际一致（`focus`／`blur` 即重建）；
- 保留总开关开启（默认开，可用 `RGPUI_VIEW_RETENTION=0` 关闭）；
- 无障碍未激活、检查器未打开；
- 区间来自上一帧（`frame_seq + 1 == 当前帧`）；
- 依赖快照新鲜（实体／全局版本号、Rc 状态计数器均未变）。

## 3. 提升了什么

60 面板 × 64 标签仪表盘（3840 文本），A 窗保留开启、B 窗强制全量，同场景交替绘制：

| 每帧变更面板数 | 保留模式均值 | 全量均值 | 提升 |
|---|---|---|---|
| 0 | 19.5ms | 55.0ms | **+64.6%** |
| 1 | 18.6ms | 50.6ms | **+63.2%** |
| 6 | 19.0ms | 52.0ms | **+63.6%** |
| 60（全变） | 19.4ms | 50.4ms | **+61.5%** |

收益来自两处（分档计数口径验证，见 `retained_bench.rs`）：

1. **显式绘制跳过子树**：干净视图复用时整个子树不走 prepaint／paint（row0：70 次绘制、70 次复用、0 重建）。
2. **重建绘制中的布局保留**：脏视图仍会 `render`，但 Taffy 节点命中即不写（change=60 全变档仍有 +61.5%，即主要来自此处）。

口径说明（不夸大）：测试线束每次 `update()` 会触发一次 refresh-draw（`refreshing=true`，
整窗重建）加一次显式绘制；生产环境事件循环中 notify 驱动的绘制多为非 refresh，
干净子树复用更频繁，实际收益只会比基准更乐观。但反过来说，频繁 `refresh()` 的
应用（如逐帧滚动）主要吃到的是第 2 项收益。

正确性：`cargo test -p rgpui` 全量 636 通过（含 oracle 对等、键盘激活、焦点、
滚动、菜单回归），CI 三平台（Windows／macOS／Linux）绿。

## 4. 语义变化与应用层契约

- **`render()` 仍然每帧执行**。复用只丢弃渲染产物、重放上帧 prepaint／paint 输出。
  因此 render 必须是“可重放”的：可以在 render 里读状态、做纯计算，但不要指望
  render 的副作用每帧只发生一次（此前如此，之后亦然——P3b 证伪见 §6）。
- **`notify()` 契约不变**：自身变更必须 `notify`，否则依赖快照不脏、复用会显示旧帧。
  这与 cached 视图既有契约一致。
- **`refresh()` 语义明确为全量重执行**：`window.refresh()` 触发的绘制不复用任何视图。
  滚动偏移等 Rc 状态、prepaint 期 processor（如 `uniform_list` 可见区计算）都不进
  代际系统，全靠这条门保证正确。只调 `refresh()` 不 `notify()` 的状态变更能正确显示，
  只是享受不到视图复用（仍有布局保留）。
- **焦点变化即整窗重建一次**：`focus`／`blur` 递增焦点代际，保证 `is_focused` 门控的
  按键处理器注册与聚焦外观不过期（`keyboard_activation` 回归覆盖）。
- **悬停变化走 `notify`**：hover 切换通知所在视图重建，复用不受影响。
- **已知缺口**：render 期直接读 `window.mouse_position()`／`modifiers()` 的视图，
  在“鼠标动了、但绘制由别处触发”的按需绘制中，输出可能滞后一帧（下一次重建即自纠正，
  无脏数据）。gpui-fast 中的做法是在窗口读路径挂记录器，改造成本高，
  暂列为已知问题。

开关：默认开启，无需改应用代码。`RGPUI_VIEW_RETENTION=0`（兼容 `GPUI_VIEW_RETENTION=0`）
可全局关闭；测试可用 `window.set_retention_override(Some(false))` 对单窗关闭
（oracle 对照基线用法）。

验证生效：设 `rgpui_MEASUREMENTS=1`（或 `ZED_MEASUREMENTS=1`）再跑，日志里会有
`[fast] ... reused=N rebuilt=M ...` 单行快照，看 `reused` 是否随静止帧增长；
严格对照跑两次（开／关环境变量）对比帧耗时即可。

公有 API 变更（仅一处）：`Window::request_measured_layout` 新增文本指纹／状态交还
参数（P3a 文本 carry 所需），返回值变为 `(LayoutId, Option<TextLayout>)`；
不需要文本复用的调用方走新增的 `Window::request_measured_layout_simple`，
签名与改动前一致。仓库内调用方（`text`／`list`／`uniform_list`／`virtual_list`）
已全部迁移，示例全编译通过。

## 5. 如何复现测量

```sh
# 对照基准（仅 release，debug 数字无意义），约 40 秒
cargo test -p rgpui --lib --release fast::tests::retained_bench -- --ignored --nocapture

# 对等校验与单包测试
cargo test -p rgpui --lib
```

输出列 `A(draws,reused,rebuilt)` 即本档内显式＋refresh 绘制次数、复用视图数、
重建视图数；`delta` 为 B 窗（全量）相对 A 窗（保留）的耗时下降百分比。

## 6. 证伪记录（同样重要的结论）

- **P3b render-skip（干净视图跳过 `render`）**：回退。render 附带不可判定的副作用
  （定时器泵、`use_keyed_state` 生命周期、Rc 簿记、可见区回写），跳过即挂 scroll／
  oracle。教训：render 必须跑，复用只能发生在 render 之后。
- **P5 refresh-draw 放开复用**：回退。去掉 `!window.refreshing` 门后 5 个测试挂掉，
  定位到两条独立机制：`is_focused` 门控处理器注册（跨焦点复用重放无处理器输出）、
  prepaint 期 processor（`uniform_list` 可见区）与 Rc 滚动状态不进代际系统。
  结论：`refresh()` 全量重执行语义是正确的，不应放开；焦点问题用代际守卫单独解决。
- 调查副产品：逐 draw 追踪确认测试线束每迭代含一次 refresh-draw，
  解释了此前计数器“63 重建 + 1 复用”的表象（61 视图全重建＋根复用跳过子树）。

## 7. 代码地图

- `crates/rgpui/src/fast/` —— `mod.rs`（开关 discipline）、`dependencies.rs`（代际／记录器／
  快照判定）、`layout_key.rs`（路径键）、`stats.rs`（帧统计）、`tests/oracle.rs`（对等）、
  `tests/retained_bench.rs`（基准）。
- `crates/rgpui/src/view.rs` —— `ViewElementState`（区间＋依赖＋帧序号＋焦点代际）、
  复用判定链、重放／重建两分支。
- `crates/rgpui/src/taffy.rs` —— `TaffyLayoutEngine` 节点保留／指纹／认领／sweep、
  `measure_fingerprint` 文本 carry。
- `crates/rgpui/src/window.rs` —— `frame_seq`、`fast_stats`、`reuse_prepaint`／`reuse_paint`
  区间重放、`retention_override`。
- `crates/rgpui/src/element.rs` —— `Drawable::request_layout` 统一钩点。
