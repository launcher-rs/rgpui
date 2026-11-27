# 上游借鉴开发计划（1.8.0 版本分支）

> 分支：`feat/1.8.0`（基于 `feat/diff`；版本分支直接提交推送，完成后开 PR 合入 main）
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

## 剩余待办总览（2026-11-20 收尾盘点）

批次 A/B/C/D 与两批轻量项均已落地（各节标注「已完成」）。下面是**尚未完成**
或**需要做但被阻塞**的全部条目，按「能否立刻动手」分三组，末尾附技术债清单。

### 一、可立刻动手（无外部前置）

| 项 | 来源 | 说明与做法 |
|---|---|---|
| **D3 / D4 端到端实测** | zed / gpui-fast | D3（路径渲染）与 D4（软件适配器部分重绘）需在本机实际场景确认效果；性能验收一律以本机口径为准。 |

**已完成（2026-11-21）**：动画改用 executor 时钟（zed `99e70ee4a0`，`956454d8`）、
borrow / window 错误诊断（zed `c408f8efc4`，`50e1936f`——含上游前置的 borrow_origin
追踪机制，window.rs 21 处 log_err 站点加操作与窗口 id 上下文）、D4c 像素对拍
（gpui-fast `3268abe`，`df1790af`——通过后软件适配器默认开启 partial 重绘）、
`fast/` 依赖快照哈希去重（gpui-fast `0f5bf7f` 思想，`5ad60a83`——上游修的是
`unique_states`/`OffsetReads::of` 的二次方回溯；rgpui 的对应问题在 `absorb_*`/
`merge` 完全不去重、保留视图依赖集无界膨胀，改为吸收点哈希去重：完全相同的
重复只保留首次，不同版本 / 读取方式并存，is_stale 语义逐位不变）、
B3 UAX #14 接线（zed `f0210f1621`，见下文批次 B 落地纪要）。
SVG 场景层变换动画（zed `aff99ecc32`，见下文「SVG 场景层变换动画」落地纪要）。

### 二、需要前置条件或无法本机验证

| 项 | 阻塞原因 |
|---|---|
| **D5 动态纹理**（gpui-fast `9235ef5` / `b533574`） | core 没有 `DynamicTexturePlatform` / `DynamicTextureId` 抽象，Windows `PaintSurface` 无帧字段，也没有视频示例——现在实现只会造出零调用点的 API，违反「能力必须有真实调用点」。等真实需求（视频 / 摄像头 / 流式画布）再引入。 |
| **macOS display-link 遮挡复用**（gpui-fast `184e2f3`） | 只在 macOS 生效，本机（Windows）编不到也验不了，靠 CI 矩阵兜底；且依赖 `fast/` 视图复用（见上表 P2）先落地。 |
| **GLES 3.0 无 compute 路径** | 只影响 web / 移动后端；`rgpui-web` 当前目标平台与验证环境都不在本机。 |
| **`Primitives/finish` 基准组** | rgpui 核心 crate 没有 `benches/` 与 `[[bench]]`，补基准要先引入整套 criterion 设施。 |

### 三、已判定不适用（登记，不实施）

| 项 | 结论 |
|---|---|
| notification 高于模态层（gpui-kit `f1d1f5a6`） | rgpui 的 `NotificationCenter` 是普通 `IntoElement`，层级由调用方布局决定，无框架级 z-order 可调。 |
| 行注解卡片（gpui-kit annotation cards） | rgpui 无多文件 diff 场景，等真实需求再引入。 |

### 四、技术债（不新增能力，但要清）

1. ~~**`crates/rgpui/Cargo.toml` 有一条未使用的 `tiny-skia-path = "0.11"`**~~
   已清：D3 落地后 `path_builder.rs` 的描边/虚线展开正式使用
   `tiny_skia_path`（`lyon` → `lyon_path` 一并完成）。
2. ~~**报告里还有两条次要项未定位到确切提交**~~ 已定位并处理：
   - 手势 MouseUp 被吞 → gpui-kit `5bece8fc`（"window: End mouse gestures when
     the compositor consumes MouseUp"），已落地：见下文「输入拖选手势取消」。
   - 行注解卡片视口钉住 → gpui-kit `cfab7b21`（annotation cards 视口钉住），
     **不适用**：rgpui 的 DiffView 没有行注解卡片，等真实需求再引入。
3. **`docs/borrow-upstream-plan.md` B1 方案与实际实现不一致**：方案写「引入
   `unicode-linebreak` crate」，实际落地用的是 `icu_segmenter`
   （`LineSegmenter::new_for_non_complex_scripts`，compiled_data 编进二进制）。
   已在下文 B1 处加注，历史方案段保留以免丢失决策脉络。

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
  等真实需求再引入（已收进「剩余待办总览 · 三、已判定不适用」）。

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

   > **落地修正（B1 实际实现）**：最终用的是 `icu_segmenter` 而不是
   > `unicode-linebreak`——`LineSegmenter::new_for_non_complex_scripts` +
   > `compiled_data`（规则表编进二进制、无运行时数据加载），non-complex
   > 变体跳过天城文等复杂文字的例外规则，与 zed 同款取舍，对拉丁 / CJK /
   > 谚文场景足够。上面的 `unicode-linebreak` 是当时的备选，保留以存决策脉络。
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

### B3 接线（2026-11-21，已完成，对齐 zed `f0210f1621`）

两条软换行路径的候选断点从「拉丁空格 + 禁则否决」改为 **UAX #14 断点集合查询**：

- `LineWrapper` 新增 `text_buffer`（fragment 拼接成行文本，内联元素占位为等长
  `a`，与相邻单词在分段器眼中粘结）+ `break_indices(&self, text)`（每次构造
  `LineSegmenter`——`segment_str` 按值消费分段器且不实现 `Clone`，compiled_data
  下构造只是取静态表指针，代价可忽略）。`wrap_line_measured` 拆成「重建文本」
  与「扫描」两半（`wrap_line_measured_with_text`），文本临时 `mem::take` 出
  self，避免与 `width_for_char(&mut self)` 的借用冲突。
- 候选判定：断点 **且** 字素边界（`grapheme_indices` 游标对齐，组合标记 /
  ZWJ 序列 / 肤色修饰不被切开）且行首已有非空白；对**所有**候选生效（含元素
  候选，这点曾放错到 Char 分支，混合 fragment 用例抓出）。
- 换行触发条件从 `ix > last_wrap_ix` 改为 `grapheme_index > last_wrap_ix`，
  新增 `wrap_at_candidate = 有候选 && 触发宽 - 候选前宽 <= wrap_width`：否则
  按「超宽字素整体换行」处理（`width -= width_before_grapheme`），缩进计入后
  随新行起算。C2 的 `valid_from/valid_to` 区间结构保留，下界口径随之改成
  「候选前宽 / 字素前宽」二选一。
- `truncate_wrapped_line`（多行截断那条路径）同步改断点集合查询，并在换行符
  位置也消费断点（跳过空行时迭代器不滞留）。
- `LineLayout::compute_wrap_boundaries`：断点 ∩ 「shaping 未拆开的字素起点」
  （`glyph.index` 恰为断点位置），避免把连字 / 组合标记切断；原先的
  `invariant` 字符提取改为 `text.as_bytes()[index]` 字节判定（上游同款）。
- **删除** `LineWrapper::is_word_char`（约 60 行类别表）与其测试，替换为
  `test_break_indices_follow_uax14`（拉丁空格 / 连字符 / CJK 逐字 / 行首禁则 /
  字素边界不参与断开）。
- **行为变化**（上游已声明）：连字符后可断（LB21）、URL / 路径中 `/` `?` 可断、
  emoji 序列不再被拆开、半角 ASCII 标点的「单词字符」豁免取消。`can_break_between`
  保留为公开局部判定助手，但软换行不再逐对调用（UAX #14 LB13 等已涵盖禁则）。

### 风险

- 本批未改元素样式层（计划原先设想的「模式开关」没有引入——上游也是直接替换
  而非开关）；回归靠 `line_wrapper` 19 条 + 全量库测试兜住。
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
| D4 | Windows 软适配器部分重绘 | `directx_renderer.rs` | Windows 本地 | 中（D4a/D4b/D4c 已完成） |
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

### D2 深度缓冲 anti-overdraw（已完成，对齐 zed `be07e43ea0`）

- 上游机制（WebRender 式双通道）：
  - 深度函数 `depth = saturate((quad_index + 1) × depth_scale)`（越大越近），
    深度比较用 **Greater**（0 为最远，pass 开始 Clear 0、结束 Discard）。
  - **不透明通道**：把「实心不透明、无圆角、无边框」的矩形 quad **从前往后**
    画（实例索引反向映射），开深度测试 + 深度写、无混合；非候选实例在顶点
    着色器里退化成退化三角形（CULLED_POSITION）。
  - **混合通道**（既有 back-to-front 循环）：quad 着色器剔除不透明矩形并写
    自己的深度；其余图元（阴影/路径/精灵/下划线）用视口 min=max=quad 游标
    深度把整批压到一个深度平面，落在不透明 quad 后面直接深度测试失败、不进
    片元着色器。
  - `paint_quad` 侧：圆角矩形拆「矩形内部 + 四条边带」，内部实心不透明时进
    不透明通道；纯直角边框的边带退化为边框色矩形；`largest_border_interior`
    改为沿角弧的更大内部（±1%）。
- rgpui 现状差异（移植必须适配，不能照搬 diff）：
  - 三后端 shader 都停在旧版：无 `clip_to_mask`/`is_shaped` 助手（rgpui 用
    `clip_distances` FS 裁剪），quad 顶点着色器带 `transform` 路径；
  - **实例索引 ≠ quad 全局索引**：wgpu 是旧式 `instance_offset` 顺序写（每批
    独立 bind group），拿不到全局 quad 号；DirectX 的 `draw_range` 用
    `StartInstanceLocation` 倒是等价全局索引。上游「shader 里由 instance_id
    反推深度」在 wgpu 侧不成立。
- 适配决策：深度**逐实例携带**——核心 `Quad` 的尾部 `pad_quad: u32` 换成
  `depth: f32`（大小/对齐不变，`gpu_struct_layout` 守卫测试同步），`finish()`
  排序后统一赋值（排序后下标即绘制序）；三后端 Quad 着色器结构同步该字段，
  不透明通道只需再加一个全局 `quad_count` 做反向索引；非 quad 批的视口深度
  游标用核心导出的 `quad_depth_scale`/`quad_depth` 在 CPU 算。
  `is_opaque_rectangle` 谓词沿用上游（tag==0 实心、a≥1、无圆角、无边框；
  rgpui 当前核心 quad 一律 transform 恒等、continuous=0，谓词无需加项），
  不透明通道的 content_mask 裁剪用从上游回移的 `clip_to_mask` 几何裁剪
  （深度写入不能靠 FS 丢弃）。
- 范围：核心（scene/geometry/window）+ rgpui-wgpu（WGSL + renderer + 深度纹理）
  + rgpui-windows（HLSL + D3D11 深度态 + ID3D11DeviceContext1 DiscardView）
  + rgpui-macos（shaders.metal + metal_renderer，本地编不到靠 CI）一次提交
  ——核心 Quad 布局一改，三后端必须同步，拆开会有中间态渲染错位。
- 测试：finish 深度单调/量化断言；paint_quad 拆分（圆角内部独立实心矩形、
  纯边框边带退化矩形、border-only 语义不回退）；三端布局守卫。
- 落地纪要（本批实际实现与上游的差异）：
  - 深度逐实例：核心 `Quad.depth: f32`（`finish()` 排序后按
    `quad_depth(index)` 赋值），wgpu/DX/Metal 三端 shader 的 Quad 结构同步
    `pad → depth`；`gpu_struct_layout` 守卫断言 `offset_of!(Quad, depth) == 260`。
  - 不透明通道：三端都是「批处理循环之前，全量 `scene.quads` 反序
    （`quad_count - 1 - instance_id`）画一遍」，非候选实例在顶点级几何退化；
    wgpu 用 `clip_to_mask` + `CULLED_POSITION`，HLSL/Metal 用 clip distance
    几何裁剪（FS discard 会连深度一起写，不能用）。
  - 深度态：wgpu 三管线各自 `DepthStencilState`（新式 Option 字段）；DX 三个
    `ID3D11DepthStencilState`（禁用 / Greater 不写 / Greater 写）随
    `PipelineState` 携带；Metal（metal-rs 0.33）管线只能声明
    `depth_attachment_pixel_format`，深度态在 `RenderCommandEncoder`
    上按通道设，非 quad 图元走 Metal 默认「深度禁用」态。
  - **视口深度游标已补齐（2026-11-21）**：非 quad 批按 quad 游标把视口深度钉在
    `quad_depth(cursor)`，深度态从「禁用」改为 Greater 不写——既拿到非 quad 的
    overdraw 收益，也修正了「插入更晚的不透明矩形被先画的非 quad 图元覆盖」的
    z-order 错误。三后端同步（wgpu `set_viewport` / DX `RSSetViewports` 的
    MinDepth=MaxDepth / Metal `MTLViewport.znear=zfar`）；各端加了不透明矩形
    遮挡下划线的端到端像素测试。DX 的 `DiscardView` 仍未移植（rgpui 每帧 Clear，
    无需丢弃）。
  - 验证：wgpu 侧 `crates/rgpui-wgpu/tests/headless_quad_smoke.rs`（无头
    16×16 真渲染回读，不透明覆盖 + 半透明叠加）；DX 侧软件适配器端到端测试
    `opaque_channel_occludes_and_blended_channel_culls_opaque_rects`（不透明
    通道遮挡 + 混合通道剔除断言）；Metal 本地编不到，靠 CI 矩阵。

### D4 现状摸底（2026-11-20，对齐 gpui-fast `3268abe`）

**上游改动**：5 文件 +2137/−6 —— `fast/partial.rs`（+1541）、
`fast/layers/tests.rs`（+579）、`directx_renderer.rs`（+14）、
`directx_atlas.rs`（+8）、`fast/mod.rs`（+1）。hooks 很小，算法本体很大。

**机制**：两 canvas 交替，画完拷回 swap chain；每帧图元压成 `Sig`
（外观 hash + 边界 + mask + 可达范围）与上一帧比对得 `Plan`——无变化直接
显示上一 canvas；有变化则拷贝上一 canvas 并对 damage 矩形开 scissor 重绘；
垂直整像素滚动且同 clip 内 ≥4 个图元同幅移动时，`CopySubresourceRegion`
搬移该 clip，damage 只剩暴露条带 + 未跟随的静态图元 + 其它变化。安全阀：
非整像素滚动/非像素对齐 clip、scroll layer、surface、damage > 60%、
绘制顺序不一致、变形 sprite、图集 tile 重传、clear 色变化 → 整帧绘制。

**rgpui 现状差异（移植必须适配）**：

1. rgpui 的 `fast/` 只有 `dependencies`（P1）/`layout_key`（P3a）/`stats`，
   没有上游的 `frame::FrameState`、`composition::DirectXComposition`、
   `device_recovery::RecoveryState`、`Drawer` trait 与 `layers` 像素测试
   harness——`partial.rs` 的渲染半段（`Canvas`/`Scissor`/`RendererDrawer`，
   约 500 行）无法直接复用，需要按 rgpui 的 `DirectXRenderer` 重写。
2. `DirectXRenderer::draw_into` 直接按 `scene.batches()` 调 `draw_*`，没有
   Drawer 抽象；damage 剪裁要么包 device context 的 scissor 状态（上游的
   `Scissor` guard，含失败恢复），要么给每个 `draw_*` 加区域参数（侵入更大）。
3. `DirectXAtlas` 是本地 `DirectXAtlasState`（非上游泛型 `AtlasState<B>`），
   `Uploads` 加字段比上游更轻，但要在 `get_or_insert_with` 的上传路径记录 tile。
4. **最大技术风险**：D2 深度缓冲刚落地，`pre_draw` 每帧 `Clear(0)` 并绑定
   DepthStencil；拷贝上一 canvas 后，damage 区重绘必须同步清空对应深度，
   否则深度测试会误剔除。上游 partial 基于它自己的 anti-overdraw，深度口径
   （shader 反推 instance_id）与 rgpui 的逐实例 `Quad.depth` 不同，不能直接照搬。
5. `Quad` 已加 `depth: f32`（D2），`Sig` 必须排除该字段，否则排序变化会被
   误判成内容变化而丢掉整帧复用。
6. rgpui 有 `render_to_image`（test-support）可作像素对拍基座，但上游 579 行
   测试依赖 `fast::layers` harness，需新写等价 harness。

**建议拆分（每个独立提交）**：

- D4a `Sig`/`Plan` 纯算法 + 单测（约 900 行）：先只做判定，不接绘制，
  以环境变量默认关闭。
- D4b 双 canvas + scissor + 滚动块搬移（约 500 行）：含 D2 深度同步。
- D4c atlas uploads + 像素对拍测试（约 600 行）：复用 `render_to_image`。

**状态**：摸底完成；D4a 已落地——`crates/rgpui-windows/src/fast/partial.rs`
实现 `Area` / `Sig` / `signatures` / `Plan` / `plan`，按 rgpui 差异修正
（`Quad.depth` 不进签名；quad 的 `transform`/`blend_mode`/`continuous_corners`
与多色精灵的 `transformation` 进签名并按矩阵算可达范围；有 `surface` 直接不比较）。
渲染器加默认关闭的判定 hook（`RGPUI_PARTIAL_REDRAW=1` 开启，只记 debug 日志、
不改绘制），设备丢失时清空上一帧签名。8 个单测、`clippy -p rgpui-windows
--all-targets -D warnings`、`cargo check --workspace` 全绿。

**D4b 已落地**（2026-11-20）：

- `fast::partial` 增加 `Drawer` trait + `Scissor` guard + `Canvas`（颜色/深度
  各两张纹理交替）+ `Partial::frame`。damage 区用 `ID3D11DeviceContext1::ClearView`
  只清颜色与深度（**深度清 0.0**，对齐 rgpui「越大越近」），scissor 下按矩形
  重画；滚动用 `CopySubresourceRegion` 把颜色与深度一起搬，保持 anti-overdraw
  的深度与像素对应；最后整张 canvas `CopyResource` 回交换链 buffer。
- 渲染器：`draw_batches` 从 `draw_into` 抽出供 partial 复用；`PartialDrawer`
  实现 `Drawer`，`begin` 临时把 canvas 视图换进 `resources` 并在 `Drop` 恢复
  （路径栅格化会读回 `render_target_view`）；`RENDER_TARGET_FORMAT` /
  `render_target` 提为 `pub(crate)`；设备丢失时 `partial.reset()`。
- 图集：`DirectXAtlasState` 记录自上次以来的瓦片上传，`take_uploads()` 在
  溢出时返回 `None` 强制整帧（对齐上游 `Uploads` 的 4096 上限）。

**与上游的关键差异（历史口径，已随 D4c 收敛）**：D4b 落地时因缺像素对拍
兜底，rgpui 只有显式 `RGPUI_PARTIAL_REDRAW=1` 才启用；D4c 对拍通过后改为
对齐上游——软件适配器默认开启，`=0` 强制关、`=1` 硬件上也强制开。

**验证**：`fast::partial` 9 个单测（含软件适配器真设备上的 canvas 控制流：
整帧画一次 / 无变化不画 / damage 按矩形画）、`cargo test -p rgpui-windows`
23 passed、`clippy -p rgpui-windows --all-targets -D warnings` 全绿、
`cargo check --workspace` 通过。

**仍未做**：硬件 GPU 实测（软件适配器已默认开启，见 D4c 落地纪要）。

**D4c 已落地**（2026-11-21，`df1790af`）：像素对拍门槛补齐——同一串场景
分别走 partial canvas 与整帧参考路径，回读像素**逐字节**比对。场景 =
不透明绿块 + 红色三角形路径（D3 环绕数管线）+ 蓝色下划线；帧 A 全量绘制、
帧 B 只移动三角形（其余像素从 canvas 原样拷贝、damage 区重画），并断言
帧 B 的 damage 计划非空（防退化成整帧对拍自证）。绘制代码抽成
`BatchRenderer`（device/context/pipelines/globals/viewport/atlas 闭包）
与 `upload_scene_buffers_into`，渲染器与对拍测试共用同一份实现，消除测试
侧复制漂移；软件适配器设备上运行，CI 无显卡环境可用。**验收通过后按约定
翻开默认开关**：软件适配器上自动开启（对齐上游 wanted()），
`RGPUI_PARTIAL_REDRAW=0` 强制关闭、`=1` 在硬件适配器上也强制开启。

### D3 现状摸底（2026-11-20，对齐 zed `ffe0fa29e4`）

**上游改动**：23 文件 +2470/−2095。核心三件：`path_winding.rs`（+842 新模块）、
`path_builder.rs`（+432 重写）、`scene.rs`（±266）；三后端 shader 全改
（wgsl ±222 / hlsl ±225 / metal ±274），渲染器大幅简化
（metal −721 / wgpu −717 / directx −565 净删为主）；`window.rs` 仅 ±19。
上游声明的取舍：**描边 CPU 侧变慢 2–8×**（lyon 描边镶嵌换成 tiny-skia 轮廓展开），
笔迹密集场景需在本机实测确认。

**机制（Slug 式逐像素环绕数）**：`PathBuilder` 把轮廓转成闭合的单调二次曲线链
（三次贝塞尔按 0.25px 容差近似成二次、按极值点拆单调段）；CPU 按显示比例把曲线
分桶到方形 cell（cell 尺寸按成本模型选幂次），未列入 cell 的曲线贡献折进 backdrop；
相邻同列表同 backdrop 的 cell 合并成矩形 `PathTile`（大矩形 → 9 tile：4 角 4 边 1 内部）。
每 tile 一个 GPU 实例，shader 只遍历该 tile 的曲线：向左/向上射线数交叉算环绕数，
半像素内按 Slug 公式计部分覆盖；even-odd / non-zero 两种填充规则都在 shader 侧实现。
**不再有 MSAA、中间纹理、额外 render pass、CPU 镶嵌。**

**rgpui 现状差异（移植必须适配）**：

1. rgpui `Path` 是泛型 `Path<P>` + 预镶嵌三角形（`vertices: Vec<PathVertex>`），
   由完整 `lyon`（tessellation）产出；上游 `Path` 非泛型 = `Arc<[PathCurve]>` +
   even_odd 标志，`Path::new`/`push_triangle`/`scale`/`build_path` 全删。
   公开 API 面其实不大：仓内 `push_triangle`/`build_path` 无外部调用点，
   `Path<Pixels>` 类型标注只在 ink.rs / anim_timeline.rs / window.rs / path_builder.rs。
2. rgpui 的 `StrokeOptions`/`FillOptions` 直接重导出 lyon tessellation 类型，
   ink.rs / anim_timeline.rs / painting 例用了 `.with_line_width/.with_line_cap/.with_line_join`
   builder 链；上游换成自有精简 struct（无 builder 方法）。**有意偏差**：rgpui 版
   自有 struct 保留同款 builder 方法，6 处调用点不动。
3. 依赖变化：`lyon = "1.0"` → `lyon_path = "1.0"`（只留 path 构建，去掉 tessellation）
   + `tiny-skia-path`（描边展开：dash + stroke→outline，**预置依赖正是为此**）。
   ink.rs / anim_timeline.rs 直接用的 `lyon::path::{LineCap, LineJoin}` 改由
   path_builder 重导出（lyon_path 同名类型，语义一致）。
4. **rgpui `Background` 是 136 字节**（上游 72：无 PatternSlash/Checkerboard/Conic 等
   pattern 档），`PathPaint` 相应为 176 字节（bounds 16 + mask 16 + Background 136 +
   even_odd 4 + pad 4）——三端 shader 结构体**按 rgpui 口径写，不能照抄上游 HLSL**。
   `PathCurve` 40 字节、`PathTile` 56 字节与上游一致。
5. 场景层：`Scene::paths: Vec<PaintedPath>`（含 `Rc<PathTiles>`，`append_to` 把
   paints/curves/tiles 三数组平铺给渲染器）；分桶发生在 `PaintedPath::new`
   （即 `paint_path` 调用点，按当时 scale_factor），`retain_visible(content_mask)`
   先裁掉 mask 外 tile。批处理仍是 `PrimitiveKind::Path` 按 order 分批。
6. 与 D2 的交互：tile 实例直接进主通道，走非 quad 批的既有口径（深度禁用态，
   与 sprites/shadow/underline 一致——rgpui 未移植非 quad 视口深度的部分），
   不再需要 path 专用的 rasterization/resolve 两个管线与中间纹理。
   wgpu 侧 `path_globals_bind_group` / gamma（Loop-Blinn 残留）一并删除。
7. 与 D4 的交互：`fast::partial` 现在用 `.debug(p)` 保守签整条路径；`PaintedPath`
   的 Debug 会展开全部 tile+curve（大且无谓），改成结构化键：
   paint 字段 + tile 数 + tile 几何 + 曲线字节 hash（binning 确定性保证同输入同输出）。
8. 验证基座：wgpu 有 D2 的 headless 真渲染回读测试、DX 有软件适配器端到端测试，
   两端各加一条 path smoke（画三角形路径，读像素断言内部全 coverage、边缘半覆盖）；
   Metal 本地编不到，shader 翻译靠 CI 矩阵兜底。上游 `randomized_element_tree.rs`
   是 test-support 的随机对拍，rgpui 无此文件，不引入。
9. `examples/showcase` 的 `painting` / `paths_bench` bin 用 PathBuilder API，
   编译应无感；`render_to_image`（test-support）走 wgpu 渲染路径自动获得新管线。

**预计规模**：核心 ~1100 行（path_builder 重写 + path_winding 移植 + scene/window
改动）+ wgpu ~400 + DX ~350 + Metal ~250 + 三端 shader ~450 + 测试 ~300，比 D2 大。
上游 `path_winding.rs` 的 5 条单测（九宫格 tile / 超大路径只画可见 tile / 溢出跳过 /
非有限点 fail 整条路径 / 端点平衡）随模块移植。

**D3 已落地**（2026-11-20/21）：

- **核心**：`path_builder.rs` 重写（lyon_path 构建 + tiny-skia-path 描边/虚线展开
  → `Outline` → 闭合单调二次曲线链，三次贝塞尔按 0.25px 容差近似成二次、按极值点
  拆单调段）；新模块 `path_winding.rs`（BINNER thread_local 分桶：cell 尺寸成本模型、
  backdrop 步进、run 合并成 `PathTile`，5 条单测全移植）；`scene.rs` 的 `Path` 去泛型
  去三角形，新增 `PathCurve`/`PathPaint`/`PathTile`/`PaintedPath`（`Rc<PathTiles>`
  缓存，`append_to` 平铺三数组），`window.rs::paint_path` 改走 `PaintedPath::new`
  （分桶发生在绘制时、按当时 scale_factor，`retain_visible` 先裁 mask 外瓦片）。
  依赖 `lyon = "1.0"` → `lyon_path = "1.0"`（dev-dep 里未用的 `lyon` 一并删除），
  预置的 tiny-skia-path 正式启用——技术债 §四.1 就此清账。
- **公开 API 变化**（对齐上游，有意为之）：`Path<Pixels>` → `Path`（非泛型，
  只能由 `PathBuilder` 构建）；`Path::new`/`push_triangle`/`scale`/`build_path`
  删除（仓内无外部调用点）；自有 `StrokeOptions`/`FillOptions`（**有意偏差**：保留
  `with_line_width/with_line_cap/with_line_join` builder 方法，ink.rs / anim_timeline.rs /
  painting 例 6 处调用点不动）；`LineCap`/`LineJoin`/`FillRule`/`Transform` 改由
  path_builder 重导出（lyon_path 同名类型）。`showcase` 的 `painting`/`paths_bench`
  仅 `Path<Pixels>` 类型标注两处改动。
- **三后端**：删旧 path_rasterization/resolve 双管线与中间纹理（wgpu 连同
  Loop-Blinn 残留的 path_globals/gamma 缓冲一起删），改「瓦片实例直接进主通道」：
  wgpu 每批一个三槽 bind group（tiles/curves/paints 存储缓冲，批内平铺、批内下标）；
  DX 复用核心的 `PathPaint`/`PathCurve`/`PathTile` 类型（布局即 GPU 布局），
  t1=本批瓦片区间视图、t2/t3=全量曲线/绘制参数视图（**首次提交时槽位交叉写反，
  软件适配器测试抓出后修正**）；Metal 三缓冲共用实例池、按 256 对齐分段绑定，
  三角带 4 顶点、角点按 `vertex_id` 位选（与上游一致，避免 origin+size 插值的
  瓦片接缝）。三端 shader 的 `PathPaint` 按 rgpui 136 字节 `Background` 口径
  （176 字节）写，不照抄上游 112 字节版本；布局守卫
  `gpu_struct_layout.rs` 新增 PathCurve(40)/PathTile(56)/PathPaint(176) 三条。
- **顺手修复 D2 遗留回归**：wgpu 共享渲染通道挂深度附件后，`depth_stencil: None`
  的管线（阴影/下划线/全部文本精灵）被 wgpu 验证拒绝、整批静默丢画
  （无头回读实验证实：D2 之后下划线/精灵在本分支就不出图）。给全部非 quad 管线
  配「`Always` + 不写深度」的深度态（语义等价关闭深度测试），DX/Metal 无此问题
  （DX 显式禁用深度态、Metal 走 PSO 默认禁用）。回读测试
  `dbg_underline` 式最小用例验证后删除。
- **验证**：`path_winding` 5 条单测；wgpu 无头真渲染回读
  `headless_path_smoke.rs` 2 条（三角形填充内部/外部/边缘 AA、宽 2 横线描边——
  三角形回读逐像素画出正确形状且边缘有半透明抗锯齿）；DX 软件适配器端到端
  `path_tile_pipeline_fills_triangle_with_winding`；Metal 本地编不到，shader 随
  CI 矩阵兜底。`cargo test -p rgpui --lib` 921、`-p rgpui-windows` 24（含 partial
  23 + path 1）、`-p rgpui-wgpu --features test-support` 29、guards 9、
  `cargo clippy --workspace --all-targets -- -D warnings` 退出 0。
- **验证口径**：GPU 侧收益以本机端到端实测为准（见 `docs/local-perf-baseline.md`）；
  CPU 侧描边变慢 2–8× 为上游已声明取舍，笔迹（ink）的长笔画场景待本机确认。

---

## 输入拖选手势取消（2026-11-21，对齐 gpui-kit `5bece8fc`）

鼠标释放被上层吞掉（弹出菜单、模态层、窗口失焦等）时，输入框会一直停在
拖选状态，之后每次鼠标移动都继续改选区。

- `InputState::cancel_mouse_gesture(cx)`（公开 API）：`selecting` 复位、
  清 `selected_word_range`、停 `auto_scroll`， was_selecting 时 notify；
  **保留已有选区**（失焦复制文本的语义不受影响）。rgpui 无列选，对应上游
  删掉的 `column_select_start` 无操作。
- 接线三处（`input_ui/input/element.rs` / `state.rs`）：
  1. 窗口级 MouseMove 的 **capture 阶段**（无按键拖拽中）到来 → 合成器接管了
     输入路由，取消手势；
  2. `MouseExitEvent` capture → 指针离开输入框时取消（防在框外继续拖）；
  3. `InputState::on_blur` → 失焦时取消手势（保留选区与 IME 语义）。
- 测试：`gesture_tests::cancel_mouse_gesture_ends_drag_but_keeps_selection`。

---

## SVG 场景层变换动画（2026-11-21，对齐 zed `aff99ecc32`，已完成）

场景级精灵动画：SVG 只绘制一次，之后由场景在每次呈现前按已过时间推进矩阵与
alpha，不重绘视图，避免每帧重绘整个视图树。

- **核心**（`scene.rs`）：`SpriteAnimationId`（MonochromeSprite 原 `pad` 字段位，
  `NONE = u32::MAX`）+ `SpriteAnimation`（变换、中心、scale_factor、底色、精灵下标）
  + `Scene::animations`；`insert_animated_sprite`（插入失败时不登记动画）、
  `advance_animations(elapsed)`（只改矩阵与 alpha）、`animation_frame_interval()`
  （各动画 `min_frame_interval` 取最小）；`finish()` 排序后修正 sprite_index；
  `replay` 从源场景取回动画定义。
- **API**（`elements/svg.rs`）：`Transformation` 加 `alpha` 字段 + `with_alpha` /
  `alpha_factor`；`TimedTransformation`（period/from/to、四个分量各自
  `EasingFn = fn(f32)->f32`（rgpui 的缓动是自由函数，不用上游的 Easing 枚举）、
  `alternate`、`with_max_fps`）+ `Svg::with_timed_transformation`；静态
  `with_transformation` 与动画共用同一字段（`Transformation` → `TimedTransformation`
  的 From 转换）。
- **窗口接线**（`window.rs`）：`paint_animated_svg`（有动画走
  `insert_animated_sprite`，否则普通插入）；`advance_scene_animations` 在
  present 前调用（同步动画时钟 `synced_animation_epoch`）；draw 后把
  `scene_animation_interval` 写进 Rc<Cell>，帧循环把它并进 `needs_present`
  与 `min_frame_interval`（动画档与温控/失焦档取最长）；帧末若仍有动画则
  `Window::request_frame()`。
- **平台**：`PlatformWindow::request_frame` 新方法（默认空实现）；
  Windows 用 `RedrawWindow(RDW_INVALIDATE|RDW_NOERASE|RDW_NOFRAME)`。
  **macOS / Linux 未实现**（默认空操作）——那两个平台目前只在其它失效
  引起出帧时推进动画，CI 矩阵也无法本地验证；登记为后续。
- **reduce motion**：`Svg::paint` 检查 `cx.reduce_motion()`，开启时按起始变换
  静态绘制（不登记动画）。
  `with_timed_transformation` 旋转的 SVG，与原有 `with_animation` 版本并列对照。
- **验证**：`elements::svg::tests` 4 条（旋转插值/回绕、缩放+alpha+alternate、
  max_fps、period=0）、`scene::animation_tests` 4 条（advance 只改矩阵与 alpha、
  alpha 合成、越界动画安全跳过、frame_interval）；rgpui --lib 932、
  --features editor 1038、guards 9、wgpu 29、workspace clippy 全绿。

---

## Scene 排序加速（2026-11-20，对齐 zed `d3fc053d5b`，已完成）

**上游做法**：`Scene::finish` 原来对图元本身 `sort_by_key`，每次交换都整体拷贝
一个实例结构体（`Quad` 100+ 字节）。改成先排小的 `(键, 下标)` 对，再按置换的环
把图元搬到位，每个图元只搬一次；已有序的数组提前返回、不分配；一个键缓冲在
`finish` 内跨图元种类复用。`paths` / `surfaces` 不是 `Copy` 且数量少，保持
`sort_by_key`。

**rgpui 移植**（`crates/rgpui/src/scene.rs` 新增 `sort_in_place`）：

- 键打包成 `u64`：`(major as u64) << 32 | minor as u64`，低 32 位放**插入下标**，
  因此相等键保持插入顺序——与原来的稳定 `sort_by_key` 结果逐位一致，也正因为
  有这条保证才敢用 `sort_unstable`。
- 覆盖 `shadows` / `quads` / `underlines` / 三种精灵；D2 的深度赋值循环原样保留
  在排序之后（下标即绘制序，不能提前）。
- 精灵键仍是 `(order, texture_id.index)`（D1 合批口径），未改成 `tile_id`。

**验证**：`scene::tests` 新增 4 条——`test_finish_sort_matches_stable_sort`
（6 组输入逐位对比稳定排序，含重复 order / 已有序 / 空数组）、
`test_finish_is_idempotent`（连调两次结果与深度不变）、
`test_sort_in_place_follows_multiple_cycles`（直接验证多环置换搬移）、
`test_sprite_sort_is_stable_across_texture_groups`（同键精灵保持插入序）。
`cargo test -p rgpui --lib` 全绿、`cargo clippy -p rgpui --all-targets` 退出 0。

**未做**：上游同提交带的 `benches/primitives.rs` 里 `Primitives/finish` 基准组。
rgpui 核心 crate 目前没有 bench 目标（无 `[[bench]]`、无 `benches/`），补基准要
先引入整套 criterion 设施，与本次改动无关，登记待有基准框架时再补。

---

## 调研依据（2026-11-20，temp/ 三仓库均活跃至前一天）

- **gpui-fast**（性能特化下游，`fast/` 保留层）：Windows 软适配器部分重绘
  （`3268abe`，partial.rs +1541）、动态纹理后端（`9235ef5`/`b533574`）、
  哈希定位重复状态（`0f5bf7f`）、macOS 遮挡复用 display links（`184e2f3`）、
  app 被借用时推迟帧请求（`32c506f`）等。
- **zed `crates/gpui`**：11-12 同日渲染管线三连（`be07e43ea0` / `1d8eda94bd`
  / `ffe0fa29e4`）+ 排序加速（`d3fc053d5b`，已完成，见「Scene 排序加速」）；
  UAX #14 断行（`f0210f1621`）；电源/热帧间隔
  可配置（`7c8dbadcb5`，已完成，见「帧间隔可配置」）；SVG 变换动画
  （`aff99ecc32`）；arena 包 `Rc` 防 UB（`2529740084`）等。
- **gpui-kit**（组件库下游）：diff `span_highlights`（`fa6c72d3`）、
  editor `GutterStyle`（`d758da0f`）、大文档软换行加速（`514f243d`）、
  f64 坐标 + 9 位行号（`e028806a`）、input 折叠 API / IME 失焦组合 /
  RTL 段落起始边等。
- 未纳入本轮但可后续评估：zed SVG 变换动画、gpui-kit RTL 段落起始边
  （与 rgpui bidi 同源）。（zed 电源/热帧间隔与 gpui-kit input 折叠 API
  已在后续两批完成。）

---

## 轻量借鉴项（2026-11-20 第二批，已完成）

三项低风险、可独立验证的 gpui-kit 改动，均未改公开 API 语义：

1. **input 公开折叠 API**（`6898b31c`）：`InputState::fold_all` / `unfold_all` /
   `fold_at_level` / `fold_candidates`。`fold_at_level` 用栈单遍算候选嵌套深度
   （level 1 = 不被其它候选包含），只改 folded 集合、候选保留。底座新增
   `FoldMap::set_folded_lines` / `DisplayMap::set_folded_lines` / `DisplayMap::fold_candidates`。
   测试：`fold_map::tests` 3 条（只折候选、整体替换、clear 保留候选）。
2. **IME 失焦结束组合**（`39540772`）：`TextCore::end_ime_composition()` 清
   `ime_marked_range` 但保留文本与选区；`InputState::on_blur` 调用它，避免焦点
   切走后留下"组合中"的悬挂下划线。测试：`core::tests` 1 条。
3. **Markdown 解析 panic 降级**（`249a963c`）：`parse_markdown_with_urls` 用
   `catch_unwind` 包住解析，panic 时降级为「保留换行的纯文本段落」
   （`plain_text_fallback`，区间覆盖全文）。rgpui 用 pulldown-cmark，上游报的
   崩溃输入本来就不 panic，这层是外部来源 Markdown 的防御兜底。
   测试：`markdown::tests` 3 条（换行保留、空行保留、上游崩溃输入不 panic）。

验证：`cargo test -p rgpui-markdown` 20 passed、`cargo test -p rgpui --lib`
相关 8 条、`cargo clippy --workspace --all-targets -D warnings` 无 error、
`cargo check -p rgpui --features editor` 通过。

**不适用（登记，不实施）**：gpui-kit `f1d1f5a6`「notification 高于模态层」。
上游有自动挂载的 toast 栈（`WindowState` 里 `deferred(11 + dialogs)`），rgpui 的
`NotificationCenter` 是普通 `IntoElement`，层级由调用方布局决定，没有框架级
z-order 可调。等真实需求（需要框架统一托管 toast）再引入。

---

## 帧间隔可配置（2026-11-20，对齐 zed `7c8dbadcb5`，已完成）

**上游做法**：`WindowOptions` 增加 `inactive_frame_interval` /
`low_power_frame_interval` / `thermal_frame_interval`，`window.rs` 把命中的限制
收进一个数组再 `.max()`（**最长间隔胜出**），强制呈现绕过全部限制。

**rgpui 的差异（有意）**：

- rgpui 没有 `PowerMode` 抽象（`grep -rn 'PowerMode\|low_power\|power_mode'`
  全仓为空），没有电源状态来源就加 low-power 档只会得到永远不命中的死配置，
  因此**只搬失焦档与温控档**，low-power 档登记在案，等有真实电源来源再补。
- 判定逻辑没有塞进 `window.rs` 的帧回调，而是抽到独立模块
  `crates/rgpui/src/frame_throttle.rs`（`FrameThrottle { inactive, thermal }`
  + `interval(active, high_rate_input, thermal_state) -> Option<Duration>`），
  这样「取最长」「输入速率豁免」「`None` 只关自己那一档」都能直接单测，
  不必起真窗口。帧回调只剩「强制呈现 / 有下一帧回调 → 不限帧」这层门。

**默认值不变**：`Some(33_333µs)`（失焦 ≈30fps）/ `Some(16_667µs)`
（温控 ≈60fps），与原硬编码完全一致；`WindowOptions::default()` 从
`frame_throttle` 的两个常量取，单测 `window_options_defaults_feed_this_module`
把这条接线钉住，防止两处各改各的。

**语义要点**（写进字段文档）：

- `None` = 该档关闭（不限帧），不是「回退默认值」；`Some(ZERO)` 等价不限帧。
- 两档同时命中取较长间隔（失焦 30fps + 温控 60fps → 30fps，与原「先命中先返回」
  的顺序结果一致）。
- 温控档只认 `Serious` / `Critical`；`Fair` 只是「减少非必要工作」的提示，
  不构成限帧依据。
- 高速率输入（≥60fps）只暂停失焦档，不暂停温控档。

**真实调用点**：`examples/desktop_pet` 的桌宠是永不获得焦点的 Overlay 穿透窗，
原先一直被失焦档压到 30fps，而它自己按 60fps 驱动动画循环——现在显式设
`inactive_frame_interval: None`，温控档保持默认（过热仍降频）。

**验证**：`frame_throttle::tests` 7 条（默认值等价旧行为、`Fair` 不限帧、
取最长、输入速率豁免、单档关闭、热状态未知、`WindowOptions` 默认接线）、
`cargo test -p rgpui --lib` 912 passed、`cargo clippy --workspace --all-targets
-- -D warnings` 退出 0、`python .github/scripts/check_platform_calls.py` 全通过。

