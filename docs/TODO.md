# rgpui 开发 TODO

更新：2026-11-04

状态图例：✅ 已完成 / 🚧 进行中 / ⬜ 未开始 / — 不适用（已判定无需改动）。
完成项直接给出落地文件与回归测试；本文件不引用仓库外的临时材料。

## 状态总览

### 性能与文本管线

| ID | 任务 | 优先级 | 状态 |
|---|---|---|---|
| T1 | 长列表虚拟化写成硬约定（文档 + 示例） | P0 | ✅ |
| T2 | 仓库内基准 `text_shaping` / `scene_build` | P6 | ✅ |
| T3 | DirectWrite `IDWriteTextFormat1` 缓存 | P1 | ✅ |
| T4 | `paint_line` 行级 + 字形级裁剪 | P2 | ✅ |
| T5 | 公开 `Window::frame_stats()` + HUD 接线 | P5 | ✅ |
| T12 | 纯 ASCII 单字体快路径 | 可选 | —（不实现：见下） |
| T13 | 终端 `layout_grid` 去整屏克隆 + 背景合并线性化 | 高 | ✅ |

### Markdown 与组件

| ID | 任务 | 优先级 | 状态 |
|---|---|---|---|
| T6 | Markdown 流式增量 | 高 | ✅ |
| T7 | Markdown 文档级选区 + 复制 | 高 | ✅ |
| T8 | guard 测试三件套 | 中 | ✅ |
| T9 | Calendar / DatePicker 键盘导航 | 中 | ✅ |
| T10 | SplitPane 键盘契约 | 中 | ✅ |
| T11 | 图标能力文档化 | 低 | ✅ |

### 上游修复 P0 批

| ID | 缺陷 | 状态 |
|---|---|---|
| P0-1 | Windows 着色器 sRGB 转换方向反了 | ✅ |
| P0-2 | 未配置 font features 时连字被关掉 | ✅ |
| P0-3 | 精灵排序键不是纹理索引，批次被切碎 | ✅ |
| P0-4 | `uniform_list` 深处 f32 精度错乱 | ✅ |
| P0-5 | 换行器池引用环泄漏 `TextSystem` | ✅ |
| P0-6 | `element_arena` 裸指针别名 UB | ✅ |
| P0-7 | 未校验来源的 `WM_USER` 消息 | ✅ |

### 上游修复核对批

| ID | 上游提交 | 缺陷 | 状态 |
|---|---|---|---|
| U1 | `96e95edac4` | 图集纹理已释放时精灵跳过而不是 panic | ✅ |
| U2 | `e71963a599` | 命中测试超过换行行宽时 panic | ✅ |
| U3 | `979cd3fdf0` | Windows 上 Unicode 分隔符被拆行 | ✅ |
| U4 | `5218009a2d` | Web IME 竞态 | —（我们的 Web 后端无该镜像层） |
| U5 | `1d7e5f1d01` | 副屏不同 DPI 时窗口定位错误 | ✅ |
| U6 | `7040aa5669` | 合成 COLR emoji 前未清空 render target | ✅ |
| U7 | `9cb5f90e3c` | 重绘期间未抑制合成的 `WM_MOUSEMOVE` | ✅ |
| U8 | `bc7041c600` | 独立修饰键序列超时后未 flush | ✅ |

## 未完成项

（无）

## 已判定不做

### T12 纯 ASCII 单字体快路径（不做）

- 复测：`text_shaping` 1000 行唯一 ASCII 为 1.36–1.44 ms（≈1.39 µs/行），与 T2 基线 1.35 µs/行一致；这是**缓存全 miss** 下的纯排版成本。`WindowTextSystem::shape_line` 按「文本 + 样式」缓存，终端每帧对未变化的行命中的是缓存，快路径的实际收益上限只是「每帧变化行数 × 30%」。
- 语义代价：绕过 `CreateTextLayout` 只能用 `GetDesignGlyphMetrics` 累加 advance，等于放弃 kerning 与连字（P0-2 刚把 DirectWrite 默认的 liga/clig/calt 恢复回来），同时要自行重建 fallback 与字形聚类。
- 验收要求「抽样 1000 行与完整 shaping 逐字节比对」依赖 GPU/COM 的 DirectWrite，CI 矩阵无法执行，只能本地人工比对，回归无保障。
- 结论：收益场景窄、语义风险与维护成本高，标记 —（不实现）。

### 终端与文本管线候选三项（2026-11-04 评估，不做）

1. **终端 prepaint 整屏重排缓存**：12,000 格（60×200）`layout_grid` 实测 0.63–0.69 ms/帧。核对重绘来源后确认收益场景过窄：本 crate 没有光标闪烁定时器（`CursorBlinkingChange` 只在应用切换该设置时发一次），空闲不重绘；`Wakeup`（新输出）与滚动分别改变 `output_revision` / `display_offset`，缓存必然 miss，真正命中的只有选区拖拽、搜索词输入、焦点切换这几类交互突发帧。实现上 `TerminalView::render` 每帧新建 `TerminalElement`，缓存得寄存在视图里跨帧共享，还要在 `set_theme` / `text_style_mut` / 字号变化时失效，并把缓存的 `Vec`（含每行 `String`）复制进元素——为不足 4% 的帧预算引入跨帧状态与失效风险不划算；若将来 profiling 显示拖选帧超预算再重启。
2. **`Terminal::make_content` 的整屏克隆**：每次同步克隆 12,000 个 `Cell` 实测 51 µs；复用 `Vec` 缓冲只省约 0.7 µs（分配占比 <2%），克隆本身才是成本。要真正消除得让渲染直接读网格，会把 alacritty 锁带进 prepaint，暂不划算。
3. **`shape_line_by_hash` 的按哈希缓存**：`try_layout_line_by_hash` / `layout_line_by_hash` 里按内容哈希查找本帧/上帧布局确实是 `iter().find` 的 O(n) 扫描（`FxHashMap` 的键是 `Arc<HashedCacheKey>`，按内容探测只能扫）。但 `crates/` 与 `examples/` 里只有 `shape_line_by_hash` 的定义、没有调用方，不构成热点；等有实际调用时把键换成可借用探测的 `HashedCacheKeyRef` 即可 O(1)。

## 已完成记录

### 代码复查修正（2026-11-04）

- `crates/rgpui-term/src/terminal.rs`：`InternalEvent::Clear` 清屏会丢弃滚动历史并重排网格，但没有递增 `output_revision`，`refresh_search` 会误判「网格未变」而保留旧坐标，搜索高亮继续盖在已清空的格子上。补一次递增，复用既有的重定位路径。
- 验证：`cargo test -p rgpui-term --lib`（44 条）、`cargo clippy -p rgpui-term --all-targets -- -D warnings`、`cargo check --workspace`、`cargo fmt --all -- --check`、`cargo test -p rgpui --test guards`（9 条）均通过。

### T13 终端 `layout_grid` 去整屏克隆 + 背景合并线性化

- `crates/rgpui-term/src/terminal_element.rs`
  - `layout_grid` 改为借用 `&IndexedCell`（`impl Iterator<Item = &'a IndexedCell>`）：`prepaint` 不再 `cells.iter().cloned()` 造临时整屏副本，逐格克隆（含可选 `Arc` 扩展数据）随之消失。
  - 删掉从未启用的 `viewport_lines` 参数与 `Box<dyn Iterator>` 动态派发，行分组改为 `chunk_by(point.line)`（显示行号的归零见下方复查修正）。
  - `merge_background_regions` 从「两两比较、反复扫描到不动点」的 O(n²) 改为单趟线性：`layout_grid` 按行生成、行内已是最大同色跨度，只需沿「上一行同列跨度 + 同色」向下延伸；上一行映射用两张 `HashMap` 逐行滚动，避免拿旧实现的 `can_merge_with` / `merge_with`。
  - 新增空白格快速路径：默认底色、无装饰、无零宽附标的空格直接跳过颜色解析与三次形状匹配（这类格子不产生背景矩形、也进不了文本运行）。同进程 A/B 实测典型 shell 输出（约 6/7 为空格）整屏布局 619 µs → 512 µs（−17.3%），全文本行与带底色空格场景在噪声内（−1 ~ −2%）。
  - 回归测试：`test_background_region_merges_vertical_same_span`、`test_background_region_keeps_different_colors_separate`（改成直接验合并结果）、`test_background_region_merge_preserves_cell_coverage`（逐格交错底色，校验不重复覆盖、不漏覆盖、颜色一致）、`test_layout_grid_batches_text_and_merges_background`（文本运行被换行切断、相邻同色空格合并成一个矩形、制表框字符走块片段而非文本）。
  - 复查修正：行分组曾直接用 `point.line` 当绘制行号，但 alacritty 的网格坐标以回滚缓冲为基准，回滚时首行是 `-display_offset`，会导致整屏上移、底部留空。`layout_grid` 增加 `display_offset` 参数换算成 0 基显示行；新增 `test_layout_grid_normalizes_scrollback_line_numbers`（临时回退该行会得到 `-6` 并失败）。
- 实测（Windows / DirectWrite，release，60×200 = 12,000 格）：
  - 纯文本：旧（克隆 42.6 µs + 布局 774.6 µs）≈ 817 µs → 新 627.6 µs（约 −23%）。
  - 整行同底色：742.7 µs → 645.3 µs（约 −13%）。
  - 逐格交错底色：240.9 ms → 1.85 ms；单独测合并 12,000 个不可合并矩形 241.9 ms → 1.10 ms，60 行整行同色合并为 1 个矩形耗时 13.6 µs。旧实现在这类 TUI 配色下会直接掉帧。
- 验证：`cargo fmt -p rgpui-term`、`cargo clippy -p rgpui-term --all-targets -- -D warnings`、`cargo test -p rgpui-term`（最终 44 条通过）；`cargo clippy --workspace --all-targets -- -D warnings` 通过。实机运行 `external_io` 演示并注入蓝/红底色、反色、制表线与方块字符，截图确认背景矩形、块片段与文本运行渲染正确。

### T11 图标能力文档化

- `examples/showcase/src/bin/icons.rs`（新增）：按变体名排序渲染全部 99 个内置 `IconName`（每格图标 + 变体名，单元格宽 136 防止长变体名折行）；顶部三张用法卡片分别演示 `Icon::new(IconName::Check)`、`Icon::default().path(..).data(include_bytes!(..))`、自定义 `IconNamed`（`StarGlyph`）。
- `examples/showcase/Cargo.toml` 登记 `[[bin]] name = "icons"`（该包 `autobins = false`）；`examples/README.md` 组件表补一行 `cargo run -p showcase --bin icons`。
- `skills/rgpui/references/widgets.md`：Icon 小节补齐 `new` / `path` / `data` / `transform` / `rotate` / `empty` / `view` / `Sizable` / `Styled` 的源码行号与三种用法，并列出全部 `IconName` 变体（与 `crates/rgpui/assets/icons/*.svg` 一一对应）。
- `crates/rgpui/tests/guards.rs` 新增守卫四 `icon_catalog_matches_assets`：按 `icon_named!` 同款 PascalCase 规则从 `crates/rgpui/assets/icons/*.svg` 生成期望集合，与技能文档、`icons.rs` 中出现的 `IconName::Variant` 逐一比对，缺漏与多余都报错；发布包缺输入时跳过、CI 缺失时失败（与既有三条守卫同口径）。
- `widgets.md` 最佳实践里原有的 `IconName::Xxx` 占位写法改为真实变体 `IconName::Check`，避免被新守卫判为「引用了不存在的变体」。
- 验证（Windows，2026-11-01）：`cargo test -p rgpui --test guards` 5 条全绿（含新守卫）；`cargo clippy -p showcase --bin icons -- -D warnings` 通过；`cargo run -p showcase --bin icons` 实机截图确认顶部三张用法卡片与 99 个图标全部渲染、无空白格，底部 `WindowClose` / `WindowMaximize` / `WindowMinimize` / `WindowRestore` 齐全；`find crates/rgpui/assets/icons -name '*.svg' -size 0` 为空。

### T10 SplitPane 键盘契约

- `crates/rgpui/src/components/split_pane.rs`：焦点句柄在 `new` 里 `tab_stop(true)`；分隔条 `track_focus` 可 Tab 聚焦，新增 `focus(window, cx)` 与 `on_key_down`：沿分隔方向的方向键按 1%（`Shift` 10%）调整、`Home` / `End` 到夹紧后的最小 / 最大比例、`Enter` / `Space` 折叠 / 展开第一个面板。
- 键盘与鼠标共用新增的 `apply_ratio`（`set_ratio` 夹紧后变化才派发 `SplitPaneEvent::Resized`），`update_from_position` 改走同一入口；元素侧键盘监听在比例变化后触发 `on_resize`，与拖拽回调一致。
- 折叠后分隔条消失，展开按钮以同一个焦点句柄 `track_focus` 并接管按键，`Enter` / `Space` 可继续展开，键盘焦点不中断。
- 无障碍：分隔条 `Role::Splitter` + `aria_label` + `aria_numeric_value`（百分比）/ `aria_min_numeric_value` / `aria_max_numeric_value` / `aria_numeric_value_step(1)` + `aria_orientation`（随分隔方向）。各实例的分隔条、折叠 / 展开按钮 ID 补上 `EntityId`，避免多实例冲突。
- 测试（`split_pane::tests`，5 条）：方向键 1% / `Shift` 10% 与水平分割忽略上下键、垂直分割用上下键、`Home` / `End` 夹紧、`Enter` / `Space` 折叠展开（折叠后键盘仍可展开）、无障碍树（`Splitter` 角色 + 数值 50 + 方向）。
- `skills/rgpui/references/components.md` 更新 SplitPane 键盘与无障碍契约。
- 验证（Windows，2026-11-01）：`cargo test -p rgpui --lib split_pane` 5 条全绿。

### T9 Calendar / DatePicker 键盘导航

- `crates/rgpui/src/components/date_picker.rs`：`DatePickerState` 新增 `focus_handle` / `active_day` / `on_close`；构造改为 `new(cx)`，新增 `focus(window, cx)`（弹窗打开后由父组件调用）、`active_day()`、`on_close`；`select` 同时把活动日移到该天，`pick` 聚焦面板并统一触发 `on_change`（鼠标与键盘共用一条路径）。
- 键盘：`←/→` ±1 天、`↑/↓` ±7 天（跨月时展示月份跟随）、`Home` / `End` 到本周一 / 周日、`PageUp` / `PageDown` 换月并按目标月份天数钳制（1 月 31 日 → 2 月 29 日）、`Enter` / `Space` 选中、`Esc` 触发 `on_close`。
- 无障碍：面板 `Role::Grid` + `key_context("DatePicker")` + `track_focus` + `on_key_down`；日期格 `Role::GridCell` + 完整日期 `aria_label` + `aria_selected`，活动日经 `aria_active_descendant` 报告；活动日描边、选中日填充可同时出现。导航按钮 ID 带上实例序号，避免同页多实例冲突。
- 测试钩子：`A11y::force_active_for_test`（`window/a11y.rs`，解除 `new_inaccessible` 的强制禁用并打开活跃标志）与 `Window::force_a11y_active_for_test`（`window.rs`，仅 `test` / `test-support` 编译），让测试平台也能用 `debug_a11y_tree_json` 断言树结构。
- 测试（`date_picker::tests`，8 条）：方向键 / Home / End、跨月移动、PageUp / PageDown、闰年钳制、Enter / Space 选中 + 回调、Esc 关闭、鼠标点击选中、无障碍树（`Grid` 根、`GridCell` + `selected`、`active_descendant_focus` 指向选中日）。
- `examples/showcase/src/bin/component_gallery.rs` 适配 `new(cx)`；`skills/rgpui/references/components.md` 更新 DatePicker API、键盘表与无障碍语义。
- 验证（Windows，2026-11-01）：`cargo test -p rgpui --lib date_picker` 8 条全绿；`cargo check -p rgpui --all-targets` 通过。

### T8 guard 测试三件套

- `crates/rgpui/tests/guards.rs`（新增）：三条仓库级守卫 + 围栏扫描器自测，`cargo test -p rgpui --test guards` 运行。仓库根按 `CARGO_MANIFEST_DIR` 的上两级定位；发布包里 `AGENTS.md` / `examples/` / `skills/` 不存在时跳过，`CI` 环境变量存在时改为失败，避免守卫静默失效。
- doctest fence：扫描 `crates/*/src/**/*.rs` 的 `///` / `//!` 文档注释与 `skills/**/*.md`，按 CommonMark 规则识别 ` ``` ` / `~~~` 围栏（支持嵌套与 `rust,ignore` 信息串），禁止 `ignore` / `no_run`。两个口径：doctest 真正会跑的 crate 逐条校验，现有 16 处片段无法独立编译（`rgpui-3d` 1、`rgpui-term` 3、`rgpui-macros` 12），登记在 `ALLOWED_FENCES`（按「文件 + 围栏首行」匹配，附理由，条目失效即报错）；`[lib] doctest = false` 的 crate（当前只有 `rgpui`）围栏本就不参与编译，改为整包登记 `NO_DOCTEST_CRATES` 并注明理由 —— 新关闭 doctest 且带围栏的 crate 会被拦下。
- showcase coverage：新增 `examples/showcase/coverage.toml`，把 `AGENTS.md`「组件库索引」28 个子系统逐个登记到源码模块（相对仓库根）与示例 bin（`<package>:<bin>`）。守卫解析 AGENTS.md 表格第一列（去括注），校验两侧一一对应、模块路径存在、bin 在对应示例包的清单或 `src/bin/<bin>.rs` 中存在；`rgpui-term` / `rgpui-markdown` 指向各自示例包。
- required-features：遍历 `examples/*/Cargo.toml`，校验 `[[bin]].required-features` 是本包声明的 feature、`[[bin]].path` 存在、所有 `rgpui/<f>`（含 feature 定义与静态依赖 features）都能在 `crates/rgpui/Cargo.toml` 的 `[features]` 里找到。
- 顺带补齐覆盖表里唯一没有示例的「屏幕取色」：`showcase` 新增 `screen-capture` feature（并入 `default`，保证 CI 的 `--all-targets` 会编译这条路径），`color_picker` 动作行在 feature 下多出「吸管取色」按钮，调 `ColorPickerState::begin_eyedropper`。
- `crates/rgpui/Cargo.toml`：`[dev-dependencies]` 加 `toml.workspace = true`（守卫解析清单与覆盖表）。
- 验证（Windows，2026-11-01）：`cargo test -p rgpui --test guards` 4 条全绿；故意在 `rgpui-3d/src/lib.rs` 新增 `ignore` 围栏 → 报 `crates/rgpui-3d/src/lib.rs:23`；把 coverage.toml 的 bin 改成 `showcase:not_a_bin` → 报 `coverage.toml:9`；把 `charts` 的 required-features 改成 `chartz` → 报 `examples/showcase/Cargo.toml:141`；三处均已还原。`cargo clippy -p rgpui --all-targets -- -D warnings` 与 `cargo clippy -p showcase --all-targets -- -D warnings` 通过。

### T2 仓库内基准：text_shaping / scene_build

- `crates/rgpui/Cargo.toml` 新增 `bench` feature（`dep:criterion` + `test-support` + `input-latency-histogram`）：`bench_context.rs` 与 `#[rgpui::bench]` 早已存在，但此前没有对应的 Cargo feature，消费方无法启用；criterion 从 dev-dependency 改为可选正常依赖（普通构建不编译）。
- `examples/list_showcase/Cargo.toml`：`[dev-dependencies]` 加 `criterion`、`rgpui`（`features = ["bench"]`）、`rgpui-platform`（`features = ["test-support"]`，宏生成的无头渲染器入口在该 feature 下），两个 `[[bench]]`（`harness = false`）。bench 不放 `crates/rgpui/benches/`：会形成发布禁止的 dev 循环依赖。
- `examples/list_showcase/benches/text_shaping.rs`：1000 行唯一 ASCII（约 90 字符/行），`#[rgpui::bench]` + `cx.bench_iter`；每次迭代新建 `WindowTextSystem`（行布局缓存为空）并逐行 `layout_line`，背后的字体表仍共享。
- `examples/list_showcase/benches/scene_build.rs`：`inputs = [Divs, Uniform]`，`Window::replace_root` 挂基准视图；bench 构建下 `cx.notify()` 的 effect flush 同步 `Window::draw` 但不 `present`，只覆盖 CPU 侧（布局 + shaping + 绘制 + 场景构建）。
- `examples/README.md` 新增「基准（list_showcase/benches）」章节，记录跑法与基线。
- 实测（Windows / DirectWrite，release，2026-11-01）：`text_shaping` 1.33–1.38 ms / 1000 行（约 1.35 µs/行）；`scene_build` 逐行 `div` 4.87 ms/帧、`uniform_list` 394 µs/帧（约 12×）。
- 验证：两条 `cargo bench -p list_showcase --bench ...` 可跑；`cargo clippy -p list_showcase --all-targets -- -D warnings` 通过。

### T7 Markdown 文档级选区 + 复制

- `crates/rgpui-markdown/src/selection.rs`（新增）：`SelectionPoint { leaf, byte }` / `MarkdownSelection` / `MarkdownSelectionState`，叶子以「全文顺序索引 + 叶内字节偏移」定位，`range_for_leaf` 端点按字符边界向下吸附；`selected_text()` 按叶子顺序拼接（叶子间 `\n`，列表项保留 `• ` / `1. ` 前缀）；`copy_to_clipboard()` 写 `ClipboardItem`；`select_all` / `clear` / `begin` / `extend` / `end` 完整拖选状态机。`SelectableText` 元素代理 `StyledText` 的布局与绘制，按 `TextLayout::index_for_position` 命中字节索引，按下置锚点、移动推进 head、抬起结束，未拖动的单击落在链接区间时打开链接；I 形光标在 paint 阶段设置。
- `crates/rgpui-markdown/src/rich_text.rs`：`render_blocks_selected` 把可选渲染上下文穿透到段落/标题/引用/列表/表格单元格；选区高亮按 `TextRun` 字节区间切分并替换背景色（`apply_selection_highlight`）；代码块整块作为叶子（按下置锚点，拖过时按方向扩到块首/块尾，整块选中时容器着色），表格单元格逐格命中。
- `crates/rgpui-markdown/src/markdown.rs`：`Markdown::selection(Entity<MarkdownSelectionState>)`；每帧清空并重建叶子登记表，根节点在叶子外抬起鼠标时结束拖选。
- `crates/rgpui-markdown/src/lib.rs` 导出 `MarkdownSelection` / `MarkdownSelectionState` / `SelectionPoint`。
- 示例 `examples/showcase/src/bin/markdown_selection.rs`：`Ctrl+C` 复制、`Ctrl+A` 全选、`Esc` 清空、按钮清空，状态行显示已选字符数；`examples/README.md` 已登记。
- 文档：`skills/rgpui/references/markdown.md` 新增「文档级选区（MarkdownSelectionState）」章节。
- 验证：`selection::tests::{selected_text_spans_leaves_in_document_order,range_clamps_at_leaf_boundaries,range_snaps_to_char_boundaries}`；实机拖选跨「标题 + 段落 + 列表标题」，`Ctrl+C` 后剪贴板内容与文档一致；`Ctrl+A` 全选复制含列表标记、代码块与表格单元格（按阅读顺序），截图确认高亮与代码块整块着色。

### T6 Markdown 流式增量

- `crates/rgpui-markdown/src/state.rs` 新增 `MarkdownState`：`new` 同步完成首次解析（首帧即有内容）；`append` / `set_source` 只更新源码，按 16 ms 防抖在后台整篇重解析；解析期间 `parsed()` 继续返回上一版块列表；每次调度递增 `generation`，后台结果回来时比对丢弃过期结果，新调度丢弃旧 `Task` 即取消旧解析。
- `crates/rgpui-markdown/src/markdown.rs`：`ParsedMarkdown` 类型别名；全局单条缓存改为容量 8 的 LRU（`ParseCache`，命中提升为最近使用），静态 `Markdown::new` 不再因多文档轮流渲染互相踢缓存；新增 `Markdown::new_state(Entity<MarkdownState>)` 并订阅实体，解析完成自动重绘。
- `crates/rgpui-markdown/src/lib.rs` 导出 `MarkdownState` / `ParsedMarkdown` / `parse_markdown`。
- 示例 `examples/showcase/src/bin/markdown_stream.rs`：每 50 ms 注入 32 字节，右上角挂 `FpsHud`，含“重新播放”；`examples/README.md` 已登记。
- 文档：`skills/rgpui/references/markdown.md` 新增「流式增量（MarkdownState）」章节与最佳实践。
- 验证：`state::tests::append_debounces_then_publishes_parsed_blocks`（防抖窗口内保留旧块）、`independent_states_do_not_reparse_each_other`（两条流式消息互不影响）、`set_source_replaces_and_reparses`；`markdown::tests::parse_cache_is_bounded_lru`（容量与提升语义）。实机截图确认逐块渲染完成、HUD 与宽度约束正常。
- 实测（735 字节按 32 字节 / 50 ms 注入，输出完成后取 `FpsHud`）：release 构建 **FPS 59.7、DRAW 2.14 ms**（验收线 < 8 ms）；debug 构建 DRAW 约 31.7 ms，仅作开发观察。

### T1 长列表虚拟化写成硬约定

- `AGENTS.md`：组件索引补 `UniformList` / `uniform_list`；完整性检查清单第 13 条要求 >100 行必须用 `uniform_list` / `v_virtual_list`，且必须撑满确定高度的父容器。
- `skills/rgpui/references/scroll.md` 新增「长列表：虚拟化是默认选择」正反例；`list-table-tabs.md` 同步口径。
- 示例 `examples/list_showcase/src/bin/uniform_list.rs` 保持 `.h_full()` 写法。

### T3 DirectWrite `IDWriteTextFormat1` 缓存

- `DirectWriteState` 增加 `text_format_cache: HashMap<(FontId, u32), IDWriteTextFormat1>`；`layout_line` 命中复用，miss 时建好并 `SetFontFallback` 后插入；`CreateTextLayout` 仍每行创建。
- 实测 1000 行动态刷新：avg 46.3 ms/帧 → 38.5–38.9 ms/帧（约 −16%）。

### T4 `paint_line` 行级裁剪

- `crates/rgpui/src/text_system/line.rs`：`line_is_visible`（外扩 `max(line_height, font_size)` 后与 content mask 求交），`paint_line` / `paint_line_background` 早退。
- `window.rs`：`paint_glyph` / `paint_emoji` 在 `raster_bounds` 之后按真实边界早退。
- 静态帧 avg 7.7 → 7.42 ms（约 −4%）；已验证单字形 mask 原本就存在，剩余收益取决于每行绘制开销。

### T5 `Window::frame_stats()` + HUD

- `FrameStatsSnapshot` 公开 + `#[non_exhaustive]`，经 `rgpui::fast` 重导出；新增 `Window::frame_stats()`。
- `FpsHudState` 新增 `draw_ms` / `update_draw_ms`；`FpsHud` 未手动喂入时读取 `window.frame_stats().avg_ms`，新增 `DRAW` 行（≤8 ms 绿 / ≤16 ms 黄 / 更高红）。
- `skills/rgpui/references/fps-hud.md` 已同步。

### P0 上游修复七项

| ID | 落地位置 | 验证 |
|---|---|---|
| P0-1 | `crates/rgpui-windows/src/shaders.hlsl`（sRGB 转换方向） | sRGB 与 Oklab 渐变实机平滑无跳变 |
| P0-2 | `crates/rgpui-windows/src/direct_write.rs`（`apply_font_features` 保留默认 liga/clig/calt） | Cascadia Code 默认显示连字，`disable_ligatures()` 生效 |
| P0-3 | `crates/rgpui/src/scene.rs`（精灵排序键改为 `(order, texture_id.index)`） | `scene::tests::test_sprite_sort_keeps_texture_batches_contiguous`，回退旧键立即失败 |
| P0-4 | `crates/rgpui/src/elements/uniform_list.rs`（索引与偏移改 f64） | 900 万行处单测，回退 f32 立即失败 |
| P0-5 | `crates/rgpui/src/text_system/line_wrapper.rs`、`text_system.rs`（池只存字符宽度） | `test_wrapper_pool_does_not_retain_text_system` |
| P0-6 | `crates/rgpui/src/app.rs`、`window.rs`（`Rc<RefCell<Arena>>` 消除别名 UB） | `cargo test -p rgpui --lib` 通过 |
| P0-7 | `crates/rgpui-windows/src/events.rs`、`platform.rs`（`WM_USER` 消息统一 token 校验） | `cargo clippy --workspace --all-targets -- -D warnings` 通过 |

### U 上游修复核对八项

| ID | 落地位置 | 验证 |
|---|---|---|
| U1 | Windows `directx_atlas.rs` / `directx_renderer.rs`；WGPU `wgpu_atlas.rs` / `wgpu_renderer.rs`；macOS `metal_atlas.rs` / `metal_renderer.rs`：取纹理返回 `Option`，缺失时跳过该批精灵 | `cargo test -p rgpui-windows --lib`；macOS 由 CI 矩阵编译 |
| U2 | `crates/rgpui/src/text_system/line_layout.rs`：`index_for_x` 返回 `None` 时回退行尾索引 | `line_layout::tests::test_index_for_position_past_wrapped_line_width`，回退旧代码按预期 panic |
| U3 | `crates/rgpui-windows/src/direct_write.rs`：按 U+2028 / U+2029 分段排版再水平拼接；原实现改名 `layout_line_without_separators` | 两条 `unicode_*_separators_preserve_shaped_line` 测试，绕过分段时宽度比对失败 |
| U4 | 不改代码：`rgpui-web` 无隐藏 `ime_mirror` 元素与选择同步层，组合事件直连分发，竞态不存在 | `grep mirror/selectionchange` 为空 |
| U5 | `crates/rgpui-windows/src/display.rs` 新增 `WindowsDisplay::scale_factor()`；`window.rs::retrieve_window_placement` 用目标显示器 DPI 换算物理矩形 | 多显示器异构 DPI 无法单测，代码路径核对 + CI 编译 |
| U6 | `crates/rgpui-windows/src/direct_write.rs`：抽出 `composite_color_layers`，绑定后 `ClearRenderTargetView` | `color_emoji_composites_over_cleared_texture`，去掉 clear 读回 `[255,255,255,255]` |
| U7 | `crates/rgpui-windows/src/events.rs`：位置未变且无按键按下时忽略合成 `WM_MOUSEMOVE`；`window.rs` 记录最近真实位置 | 需真实窗口重绘复现，逻辑同上游客径 |
| U8 | `crates/rgpui/src/platform/keystroke.rs` 新增 `is_modifier_key`；`window.rs` 纯修饰键待定序列也置 `needs_timeout` | `key_dispatch.rs` 两条测试，回退 `window.rs` 改动后超时测试失败 |

## 建议顺序

本轮待办已清空。上一轮列出的三个实测候选已在 2026-11-04 评估并全部判定不做，理由见「已判定不做 → 终端与文本管线候选三项」；下次继续找优化时重新按实测场景占比立项。

## 明确不做（本轮）

- **`SelectionModel` 抽象**：等 T7 实现选区后，把实际用到的 anchor/head/extend 逻辑沉淀出来，再决定是否抽公共模型；现在抽是凭空设计。
- **API 文档覆盖率门禁**：中文注释已覆盖，量化与门禁成本高、收益低；图标清单已在 T11 补齐并加了守卫，其余 API 按需增量更新。
- **把 `rgpui` 拆成更小的 crate 以“追上 gpuikit 速度”**：差距来自范围而非执行力，拆分不解决文本/布局/平台层的问题。

## 复测口径

```bash
# 日常门禁
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p rgpui --lib
cargo test -p rgpui-windows --lib
cargo test -p rgpui-markdown --lib
```

性能对比统一用：

```bash
cargo bench -p list_showcase --bench text_shaping
cargo bench -p list_showcase --bench scene_build
```

每次优化前后把 `results-*.json` 与帧耗时拆解写进 PR 描述，作为收益证据。
