# 上游借鉴开发计划（borrow-upstream）

> 分支：`feat/borrow-upstream`（基于 `feat/diff`；PR #85 合入 main 后同步 main）
>
> 背景与调研结论见本文末尾「调研依据」；本计划是 temp/ 三仓库（gpui-fast /
> gpui-kit / zed）近期演进的借鉴落地路线。

## 总策略

- **顺序**：批次 A（DiffView 增强）→ 批次 B（UAX #14 断行）→ 批次 C（editor
  大文档三件套）→ 批次 D（渲染管线性能专项）。从小到大，A/B/C 每批一次
  独立提交；D 每个子项单独评估、独立提交，进入前与维护者确认单项范围。
- **分支策略**：全部在本分支开发；PR #85（feat/diff → main）合并完成后
  `git fetch && git merge origin/main` 同步一次，解决冲突后再继续。
- **每批验收（硬性）**：`cargo check --workspace`、
  `cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all`、
  `cargo test -p rgpui --test guards`；涉及到的包跑 `cargo test -p <pkg>`。
  改到 feature 门控文件时按 AGENTS.md 跑全量口径。
- **约束**：核心 crate 禁 `unwrap/expect/panic`；所有公开 API 中文注释；
  改公开 API 同步更新 `skills/rgpui/references/`；禁 `#[allow(dead_code)]`。

---

## 批次 A：DiffView 增强（对齐 gpui-kit）

对照提交：`fa6c72d3`（diff `span_highlights` 字符级高亮）、
`cfab7b21` / `7eb9f997`（水平滚动视口保持）。

现状（`crates/rgpui/src/components/diff.rs`，433 行）：`DiffState` 行级 diff
（`similar` crate）+ 行号 + `uniform_list` 虚拟化；**无字符级高亮、无滚动定位
API**。rgpui 是单文件 unified 视图，gpui-kit 的「文件头/注解卡片视口保持」
没有直接对应场景，转译为下面 A2。

### A1 字符级高亮 `span_highlights`（高优先）

- `DiffState` 新增字段 `span_highlights: Vec<DiffSpanHighlight>`，
  `DiffSpanHighlight { line: usize, range: Range<usize> }`（行 ix + 行内
  **字节**区间，语义与 gpui-kit 一致：强调搜索命中等字符区间）。
- API：
  - `pub fn set_span_highlights(&mut self, highlights: impl IntoIterator<Item = DiffSpanHighlight>)`
  - `pub fn clear_span_highlights(&mut self)`
  - `pub fn span_highlights(&self) -> &[DiffSpanHighlight]`
  - 便捷构造：`DiffState::highlights_for_query(&self, query: &str) -> Vec<DiffSpanHighlight>`（大小写不敏感的字节查找，供搜索联动）。
- 渲染：行内容文本按区间切段，命中段加主题强调背景
  （`theme.tokens.accent.color.opacity(0.35)` 左右）与加粗；区间落在
  `content_text`（含 `+ `/`- ` 前缀偏移 +2）需要平移；按字节区间切段时
  必须**沿 UTF-8 字符边界对齐**（`floor_char_boundary` 语义，参照
  `utf8-slice-safety.md`），非法区间忽略不炸。
- 测试：区间切段正确（含 CJK 多字节）、前缀偏移、非法区间忽略、
  `highlights_for_query` 命中统计；draw 测试（有高亮时 bounds 正常）。

### A2 滚动定位 `scroll_to_line`（次优先）

- `DiffState` 新增 `scroll_target: Option<usize>`；
  `pub fn scroll_to_line(&mut self, line: usize)`（写目标 + notify）。
- `DiffView` 内部创建 `ScrollHandle` 传给 `uniform_list`
  （参照 `sheet_grid.rs` 的 `.track_scroll(&vertical)` 用法），prepaint 后
  消费 `scroll_target` 调 `handle.scroll_to_item`，消费后清空（一次性语义，
  避免每帧重复定位）。
- 用途：与 A1 的搜索联动——下一个命中 → `scroll_to_line` + `set_span_highlights`。
- 测试：目标消费后清空；`scroll_to_item` 越界 clamp。

### 暂不做（登记 backlog）

- 行注解卡片（gpui-kit 的 annotation cards）：rgpui 无多文件 diff 场景，
  等真实需求再引入。

---

## 批次 B：UAX #14 断行规则补齐（对齐 zed）

对照提交：zed `f0210f1621`（`line_wrapper.rs` ±497 行，接入 UAX #14）。

现状（`crates/rgpui/src/line_break.rs`，361 行）：自研 `BreakClass` 10 类 +
CJK 禁则（行首/行末/胶连），**明确不改半角 ASCII 标点行为**（产品约束）；
两条换行路径在候选断点处按禁则否决。

### 方案

1. **引入 `unicode-linebreak` crate**（zed 同款、纯 Rust、UAX #14 完整
   pair 表）。决策理由：自建 40+ 类的完整规则表工程量大且需跟 Unicode
   版本；该 crate 无传递依赖拖累。注意仓库有 `binary-size-optimization.md`
   专项——落地时确认表体积影响，若显著则用 feature 门控（默认开启，
   `no_std` 场景不受影响——本仓库不做 no_std）。
2. **新增 API（不改旧行为）**：
   - `pub fn uax14_break_class(c: char) -> Uax14Class`（透出 crate 的类枚举，
     或转成自有枚举——落地时定）。
   - `pub fn uax14_break_opportunities(text: &str) -> impl Iterator<Item = usize>`
     （返回可断位置的字节偏移，LB2–LB31 规则由 crate 的 pair 表给出）。
3. **接线（登记为后续批次 B3，暂不动）**：`text_system/line_wrapper.rs`
   （editor 软换行 `input_ui/display_map/text_wrapper.rs` 调用）与
   `line_layout.rs::compute_wrap_boundaries`（div 文本布局）两条路径目前是
   「拉丁空格换行 + 禁则否决」；接 UAX #14 需要贯通元素样式层到 text_system
   的模式开关（候选断点改为预计算 `uax14_break_indices` 集合查询），改动
   影响全部软换行文本，回归面大。B1 的 API 已就绪，接线单独立批次。
4. **互验测试（已完成）**：`uax14_agrees_with_kinsoku_on_prohibited_breaks`
   断言禁则否决的位置 UAX #14 同样不产出断点（两口径在重叠规则上一致）；
   `uax14_break_indices_basic_rules` 覆盖拉丁空格（LB18）、连字符（LB21）、
   货币（LB25）、千分位（LB29/30）、NBSP（WJ）、CJK、强制断点（LB2/5）。

### 风险

- UAX #14 与现有 `is_word_char` 拉丁换行口径的边界（半角标点行为保持）。
- crate 表体积（见 binary-size 文档约束）。

---

## 批次 C：editor 大文档三件套（对齐 gpui-kit）

对照提交：`d758da0f`（editor `GutterStyle`）、`514f243d`（大文档/非 ASCII
软换行加速）、`e028806a`（f64 垂直坐标 + 9 位行号）。

### C1 `GutterStyle`：行号区样式可配（优先）

- rgpui 的行号绘制在 `input_ui/editor/editor_ui.rs`（`line_number_width`
  来自 `LastLayout`），与 `gutter.rs` 的**自定义标记 gutter**是两回事——
  本项是给**行号区**加样式。
- 新类型 `GutterStyle { width: Option<Pixels>, color: Option<Color>,
  active_color: Option<Color>, font_size: Option<Pixels>, right_margin: Pixels }`
  （全部 Option，None = 用主题默认；`Default` 即现状）。
- `EditorState` 加 `set_gutter_style` / `gutter_style`；`LastLayout` 的
  `line_number_width` 计算与行号绘制读取该样式（9 位行号自适应宽度顺带
  在 C3 处理）。
- 测试：样式生效（宽度/颜色绘制断言）、None 回落主题默认。

### C2 大文档软换行加速（已完成）

- gpui-kit 思路（`514f243d`）：大文档、非 ASCII 文本的软换行把测量成本
  压到 O(视口)。本项按「先测量后动手」执行，测量结论与落地：
  - **基线数据**：新增 bench `cargo bench -p list_showcase --bench text_wrap`
    （拉丁/CJK 各 1 万行，走 `LineWrapper::wrap_line` 全路径）：
    拉丁 ≈ **7.98 ms/万行**、CJK ≈ **7.68 ms/万行**（约 0.8 µs/行）。
  - **上游 Part A/B（monospace 宽度表 / CharWidths 前缀和）不落地**：
    rgpui 的 `wrap_line` 本就是逐字符增量扫描 + 字符宽度缓存（ASCII 数组 +
    HashMap），没有上游那种「每次探测都问 shaper」的 shaped probe 路径，
    宽度表思路没有收益。
  - **落地的是 Part A7（resize 只重换受影响的行）**：`LineWrapper::wrap_line_measured`
    换行同时产出每断点的 `wrap_width` 稳定半开区间（`WrapSegment`/`WrapMeasurement`），
    `LineItem` 附带行级区间 `[valid_from, valid_to)`；`TextWrapper::set_wrap_width`
    改为 `rewrap_changed_widths` 增量重换——拖分隔条/缩放窗口时，断点结构不变的行
    （绝大多数）原样克隆。区间保守方向只会多换行不会错换行（恰在上界可能仍一致）。
- 验收：增量重换与全量重换逐宽度对照测试（`test_set_wrap_width_incremental_matches_full`）
  + 区间性质测试（`test_wrap_line_measured_valid_widths`）。

### C3 垂直坐标精度 + 9 位行号（已完成）

- 对照 `e028806a`（编辑器内部坐标 f64 + 9 位行号）。rgpui **范围收缩**
  （`Pixels` 是框架级 f32 类型，不动）：摸底确认行顶 y 的逐行累加只发生在
  绘制循环——`TextElement`（选区边框、活动行背景、装饰背景、文本、行号，
  `input/element.rs`）与 editor 叠层（折叠图标、行内提示 `editor_ui.rs`，
  gutter 标记列 `gutter.rs`）共 8 处；滚动偏移、行长度、`visible_top` 都是
  单次乘法，不在改动范围。
- 落地：`input_ui/layout.rs` 新增 `RowTopY`（f64 行顶累加器：
  `new` / `advance` / `advance_rows` / `pixels`），8 处累加循环全部换用，
  绘制输出时才转回 `Pixels`（单次舍入）；长文档（>2^24 px）行顶不再跨行漂移。
- 9 位行号：行号宽度公式本就按最大行号位数自适应（`ilog10 + 2`，亿级行
  天然覆盖），提为可测函数 `line_number_digits`；自定义宽度走 C1 的
  `GutterStyle.width`。
- 测试：`test_row_top_y_precision_beyond_f32_integer`（100 万行 × 17.3px
  越过 2^24，累加器误差 ≤ 1 个 f32 ulp，朴素 f32 累加明显更差）、
  `test_line_number_digits_adaptive`（9/10 位行号位数）。

---

## 批次 D：渲染管线性能专项（对齐 zed / gpui-fast，逐项确认后进入）

对照提交：
- zed `1d8eda94bd` 图元批处理（`scene.rs` +318、`bounds_tree.rs` 重构）
- zed `be07e43ea0` 深度缓冲 anti-overdraw（Metal + wgpu shader 双后端）
- zed `ffe0fa29e4` 路径渲染重写（新 `path_winding.rs` +842，环绕数算法）
- zed `d3fc053d5b` 场景图元排序加速
- gpui-fast `3268abe` Windows 软适配器部分重绘（`fast/partial.rs` +1541）
- gpui-fast `9235ef5` / `b533574` 动态纹理后端（DirectX / wgpu CPU 上传）

### 子项顺序与范围

| D1 | 图元批处理改进 | `scene.rs`（1114 行）+ bounds 相关 | benches 对比 | 高（已完成） |
| D2 | 深度缓冲 anti-overdraw | `rgpui-wgpu` WGSL 先行；DirectX 次之；Metal 本地编不到靠 CI | wgpu 示例目测 + 帧统计 | 高 |
| D3 | path_winding 环绕数重写 | rgpui Path 实现（落地前摸底） | paths bench + 示例 | 中 |
| D4 | Windows 软适配器部分重绘 | `directx_renderer.rs` | Windows 本地 | 中 |
| D5 | 动态纹理后端 | DirectX / wgpu 动态上传路径 | Windows 本地 | 中 |

- 进入方式：**每个子项开独立提交**；D 批次动工前写单项现状摸底
  （对照上游 diff 的关键文件清单），与维护者确认后实施。
- 风险：渲染行为回归只能靠示例目测 + bench 数字；Metal 端 rgpui-macos
  本地编译不了（AGENTS.md），shader 改动依赖 CI 矩阵。

### D1 图元批处理（已完成，对齐 zed `1d8eda94bd`）

- 摸底结论：rgpui 的 `Scene`/`BoundsTree` 与上游改动前形态一致——每个图元
  插入排序树取「最大相交顺序 +1」，`paint_layer(bounds)` 开层即占一个顺序；
  合批在 `finish()` 后按 (order, 种类[, 纹理]) 分组。
- 落地：
  - `bounds_tree.rs`：叶子 key 从 `u32` 换成 `{ ordering, kind }`，
    `insert(bounds, kind, max_shared_kind)`——相交最大 key 的种类不超过
    `max_shared_kind` 时直接复用其排序值（同顺序内按种类排序保证绘制次序）。
  - `scene.rs`：`PrimitiveKind::max_shared_kind`（精灵特例：同顺序内还按
    图集纹理排序，只能与排在自己之前的种类共享）；`Primitive::kind()`；
    **图层改挂起落地**——`push_layer()` 只记挂起层（并集边界、种类聚合、
    图元区间），最外层关闭时整层按并集边界插入排序树，层内图元共用顺序；
    `paint_layer`/`StartLayer` 不再带边界参数。
  - 公开 API 变化（有意为之，同上游）：`Window::paint_layer` 去掉 `bounds`
    参数、`Scene::push_layer` 去掉 `bounds` 参数；仓内三处调用点同步
    （`ShapedLine::paint` / `paint_background`、滚动条）。
- 测试：`bounds_tree` 随机对拍升级为带种类口径（1000 种子）；scene 级新增
  重叠 Quad 共享顺序合 1 批、精灵不与精灵共享、挂起图层落地与嵌套、
  空层不占顺序。
- 测量：`scene_build` bench 在共享机器上噪声极大（同代码 3.3–11ms 波动），
  交错 A/B 下 uniform_list（合批最重路径）D1 稳定更快（≈407–448 →
  366–388 µs，约 8–10%）；divs 组噪声内不可分辨。批次收敛的直接证据由
  单测断言（依次重叠的三个 Quad：旧口径顺序 1/2/3 → 新口径全 1、批次
  3 → 1；上游同改动 Zed 启动帧批次 20 → 4）。

---

## 调研依据（2026-11-20，temp/ 三仓库均活跃至前一天）

- **gpui-fast**（性能特化下游，`fast/` 保留层）：Windows 软适配器部分重绘
  （`3268abe`，partial.rs +1541）、动态纹理后端（`9235ef5`/`b533574`）、
  哈希定位重复状态（`0f5bf7f`）、macOS 遮挡复用 display links（`184e2f3`）、
  app 被借用时推迟帧请求（`32c506f`）等。
- **zed `crates/gpui`**：11-12 同日渲染管线三连（`be07e43ea0` / `1d8eda94bd`
  / `ffe0fa29e4`）+ 排序加速；UAX #14 断行（`f0210f1621`）；电源/热帧间隔
  可配置（`7c8dbadcb5`，后续可评估给 `Platform` trait）；SVG 变换动画
  （`aff99ecc32`）；arena 包 `Rc` 防 UB（`2529740084`）等。
- **gpui-kit**（组件库下游）：diff `span_highlights`（`fa6c72d3`）、
  editor `GutterStyle`（`d758da0f`）、大文档软换行加速（`514f243d`）、
  f64 坐标 + 9 位行号（`e028806a`）、input 折叠 API / IME 失焦组合 /
  RTL 段落起始边等。
- 未纳入本轮但可后续评估：zed 电源/热帧间隔 API、gpui-kit 的 input 折叠
  API 与 RTL 段落起始边（与 rgpui bidi 同源）、zed SVG 变换动画。
