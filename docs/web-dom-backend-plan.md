# rgpui Web DOM Backend 规划

> 文档日期：2026-08-19
> 状态：**已落地（2026-08-20，纯 DOM 渲染模式）**，本文档保留战略思考与路线，实际实现见 §9
>
> 上游文档：`docs/upstream-separation-strategy.md`（切割战略）、`docs/ui-crate-plan.md`（UI 重组）、`docs/research-text-selection.md`（文本选择调研）
>
> 支撑分析：`docs/web-dom-backend-analysis.md`（参考 dioxus / topcoat / Flutter Web 的深入调研，2026-08-19）

## 1. 背景与动机

### 1.1 现状

`rgpui-web` 目前与桌面端共用同一渲染路径：element 树 → Taffy 布局 → 低层 `Scene`（quad/path/glyph 原语）→ `WgpuRenderer` 画到 `<canvas>`（WebGPU）。`WebWindow::draw(&Scene)`（`crates/rgpui-web/src/window.rs:689`）走 wgpu 路径。

这套方案的现实问题：

1. **容易出问题**：WebGPU 兼容性、多线程 fallback、3xx MB 的 debug wasm、wasm-bindgen 闭包生命周期等，都是运行时风险的来源（已多次踩坑）。
2. **丢失浏览器原生能力**：文本选择、复制、IME、无障碍、RTL、链接、表单、滚动、图片懒加载全部要自研或不可用。
3. **与桌面共享带来的负担**：为跨平台而抽象的 `Scene` 原语语义过低，web 端拿到的是"画一个圆角矩形"而不是"这是一个按钮"。

### 1.2 目标

为 web 端增加一条 **DOM 渲染后端**，让浏览器把文本、布局、表单、无障碍等能力免费交还给浏览器，同时**尽量保留**桌面端代码与组件复用。本方案遵循的核心原则：

> **rgpui 保持 Widget（element）+ Entity + Context 不变，只替换/追加渲染后端。**

桌面端完全不动。Web 端在既有 canvas 路径之外，新增一条 `element → DOM` 路径，两者可按需共存（Hybrid）。

## 2. 现有渲染管线事实（先讲清楚，方案才有依据）

### 2.1 帧流程

```
平台请求帧 (on_request_frame)
  → Window::draw(&mut App)            crates/rgpui/src/window.rs:2834
  → draw_roots → 各 View.render()     重建 element 树（一次性，非保留）
  → request_layout(Taffy) → prepaint → paint
  → 生成 Scene（扁平原语列表）         crates/rgpui/src/scene.rs:27
  → Window::present()                  crates/rgpui/src/window.rs:2984
  → platform_window.draw(&Scene)       各平台消费 Scene
```

### 2.2 关键事实（决定方案可行性）

| 事实 | 位置 | 影响 |
|------|------|------|
| **element 树是"每帧重建"的，不是保留树** | `Window::draw_roots` 每帧重建 | 用户方案里"Old Tree → New Tree → Diff"不能直接套用：没有可持久 diff 的 widget 树 |
| `Scene` 是**扁平原语列表**（quad/path/glyph），语义已丢失 | `scene.rs:27` | 不能"Scene → DOM"直译：DOM 需要语义（button/label/img）与原始文本，原语里没有 |
| 文本在 paint 阶段被 **shaping 成 glyph**（cosmic-text → atlas） | `elements/text.rs:711`、`window.rs:4191 paint_glyph` | DOM 后端必须**在 element 层**拿原始文本 + 样式，而不是从 Scene 提取 |
| element 有跨帧稳定标识 `GlobalElementId` | `element.rs:213`（由 `ElementId` 栈构成） | 这是 DOM 节点复用的**天然 key** |
| `Element::paint` 能拿到 `GlobalElementId` + `Bounds` | `element.rs:95` | 在 paint 阶段把 element 登记进 DOM 树是可行的挂点 |
| 布局由 Taffy 计算，结果是一组 `Bounds<Pixels>` | 全库统一 | DOM 端用 `position:absolute + left/top/w/h` 即可 1:1 落地，不依赖浏览器 Flex 重排 |
| 平台层 `PlatformWindow::draw(&Scene)` 是唯一绘制入口 | `platform.rs:1143` | 想绕过 Scene，就需要在**更早的 element 阶段**介入 |

**结论**：DOM 后端不能在"消费 Scene"这一层做，必须在 element paint 阶段做——即**并行构建一棵保留的 DOM 树**，key 用 `GlobalElementId`。

## 3. 对用户 12 层方案的评估

用户方案总体方向正确（保留 Widget/Entity/Context，替换 Render Backend），但有几处与 rgpui 现实冲突，需修正：

| 用户方案 | 评估 | 修正 |
|----------|------|------|
| 保留 Widget 树，只换后端 | ✅ 正确 | rgpui 的 element 是每帧重建的，需新增**保留层**（见 §4） |
| 继续用 Taffy 布局，绝对定位 | ✅ 正确 | 与现状一致；DOM 用 absolute 定位即可，不用浏览器 flex |
| 新增 `ElementNode/TextNode` + `Renderer trait` | ✅ 方向对 | 更贴合 rgpui 的做法是：DOM 节点**按 `GlobalElementId` 复用**，不做通用 diff 引擎 |
| Widget → DOM 直接映射（button→`<button>`） | ⚠️ 部分 | 内置元素（div/text/button/img/svg）可映射；**自定义元素无法穷举**，必须保留 canvas 兜底 |
| Label → `<span>`，文本交给浏览器 | ⚠️ 可行但有代价 | 浏览器排版（字体度量）与 Taffy/cosmic-text 结果**不完全一致**，视觉可能有像素级差异；需接受或做子像素对齐 |
| Input → `<input>` | ⚠️ 部分 | rgpui 有自绘 `input_ui`（Masked/Number/Password 等），映射原生 `<input>` 会丢失自定义渲染。**第一版建议 hybrid**：纯文本输入映射 DOM，复杂输入保持 canvas |
| 事件 `DOM → WidgetId → Context → Callback` | ✅ 可行 | 用 DOM `data-gpui-id`（GlobalElementId 编码）+ 事件委托 |
| 第一版不支持 CSS，全用 Taffy 结果 | ✅ 正确 | 绝对定位即足够 |
| `Html()`/Markdown 组件 | ✅ 后期加 | html5ever 解析到 DOM 树，是独立 feature，不与核心渲染耦合 |
| 完全不做 VirtualDOM，直接 diff | ⚠️ 需澄清 | rgpui 本来就没有 widget 树可 diff；是"用 GlobalElementId 做 DOM 节点复用"，不是"diff 两棵树" |

## 4. 推荐架构

### 4.1 总体分层

```
                 View.render() → element 树（每帧重建）
                            │
               request_layout（Taffy）→ Bounds
                            │
                  ┌─────────┴──────────┐
                  │                    │
         Desktop / Web 现状           Web DOM 后端（新）
             paint → Scene        paint 阶段并行构建 DOM 树
                  │                    │
          PlatformWindow::draw     RetainedDomTree（GlobalElementId 为 key）
                  │                    │
            WGPU / 平台渲染         diff → web-sys 应用变更
```

核心思路：**不替换、不改造现有渲染**，而是新增一条"DOM 影子树"。paint 阶段每个 element 在画原语的同时（或 web 专属路径下**只**），把自己登记进 `DomTree`。

### 4.2 新增抽象

```rust
// rgpui 核心（feature "dom-backend" 门控，默认关，桌面不受影响）
pub struct DomNode {
    key: GlobalElementId,          // 跨帧稳定标识
    kind: DomNodeKind,
    bounds: Bounds<Pixels>,        // Taffy 结果
    // 样式映射：由 rgpui Styled 样式转成浏览器可直接消费的 style
    style: DomStyle,
}

pub enum DomNodeKind {
    Element { tag: &'static str, attrs: Vec<(String, String)> },
    Text { text: SharedString },
    // 兜底：无法 DOM 化的自定义内容，留坑位给 canvas 层
    Foreign { layer: CanvasLayerId },
}

pub struct RetainedDomTree {
    nodes: HashMap<GlobalElementId, DomNode>,
    children: HashMap<GlobalElementId, Vec<GlobalElementId>>,
}

// rgpui-web 实现：消费 RetainedDomTree，做增量变更
impl DomBackend for WebDomBackend {
    fn reconcile(&mut self, tree: &RetainedDomTree) { /* create/update/remove */ }
}
```

### 4.3 挂点：在哪儿把 element 登记进 DOM 树

不新增 trait，**复用现有 `Element::paint` 的 `GlobalElementId + Bounds` 参数**。两个可选方案：

- **方案 A（最小侵入）**：`Window` 新增 `dom_builder: Option<DomTreeBuilder>`。paint 阶段 window 上的 `paint_quad / paint_glyph` 等方法**旁路**登记 DOM 节点（div→div、text→span、button→button…）。内置元素无需改动（它们只是调用 window 的 paint 原语）。
  - 优点：改动集中、内置组件全自动覆盖。
  - 缺点：`paint_quad` 只拿到"矩形+颜色"，拿不到语义（"这是按钮还是卡片？"）。需要 window 提供**语义化的高一层登记 API**，由各内置元素调用。

- **方案 B（语义优先）**：新增 `Element::dom(&self, window, cx) -> Option<DomNode>` 默认返回 `None`，`div/text/button/img/icon/svg/scrollable` 等内置元素实现它。
  - 优点：语义完整（`<button>`、`<input>`、`<img>`、`aria-*` 都能给出）。
  - 缺点：要给内置元素逐个实现（约 10~20 个核心元素），但这是**一次性投入且都是标准 HTML 映射**。

**建议**：方案 B，但只覆盖**核心元素子集**；未实现 `dom()` 的元素走 Foreign 层（canvas 兜底）。这样第一版就是 Hybrid，而不是"全 DOM 或全 canvas"。

### 4.4 节点复用与更新

- 每帧 `View.render()` 重建 element 树，但 **`GlobalElementId` 稳定**（由 `.id("...")` 和层级构成）。
- `RetainedDomTree` 以 `GlobalElementId` 为 key：存在则更新 bounds/style/text，不存在则新增，上帧有本帧无则删除。
- 变更提交给 `WebDomBackend::reconcile`：只对变化的节点做 `setProperty`/`setAttribute`/`textContent`/`appendChild`/`remove`，**不做全量重建**（对比现状：canvas 每帧全量重画）。

### 4.5 文本

- `Label` / `div().child("...")` 等文本在 DOM 后端走 `<span>` + `textContent`，由浏览器负责选择/复制/IME/RTL/Emoji/无障碍。
- **已知差异**：浏览器字体度量与 cosmic-text shaping 不完全一致，视觉存在亚像素/行高差异。第一版接受此差异；如需像素级一致，可对 span 逐个覆盖行高（line-height 由 Taffy 行高给出），或者对精确排版场景回退 canvas 层。

### 4.6 输入（第一版策略）

- 简单文本输入（`Input`）→ 映射 `<input>`/`<textarea>`，浏览器提供 IME/剪贴板/拼写。
- 复杂输入（`MaskedInput`/`NumberInput`/`PasswordInput`）→ 第一版保持 canvas（Foreign 层），后续再逐步 DOM 化。
- 输入事件走 `data-gpui-id` 事件委托回 rgpui 的 `PlatformInput`，复用现有 `dispatch_input` 链路。

### 4.7 滚动

- `scrollable` 元素 DOM 化：容器 `<div>` 设 `overflow:scroll` + 内容用 `position:absolute` 由 Taffy 布局，滚动由浏览器管理（含惯性、滚动条、触控板）。rgpui 的 `ScrollHandle` 状态仍由滚动事件回填。

### 4.8 事件桥接

```
DOM 事件（事件委托到 body）
  → 读 event.target 的 data-gpui-id
  → 解码为 GlobalElementId
  → 在 RetainedDomTree 命中后换算成坐标/目标
  → 转成 rgpui PlatformInput（复用现有转换：events.rs 已有 DOM→gpui 事件映射）
```

与现状的差异仅是"命中检测"从 hitbox 树换成 DOM 委托，事件**结构**不变。

### 4.9 Hybrid 分层（关键）

DOM 与 canvas 在一个窗口内共存：

```
DomBackend
  ├── dom layer  （div/text/button/img/svg/scroll 等核心元素）
  └── canvas layer（图表/编辑器/3D/自定义 paint 的元素）
```

- 第一版就建成 Hybrid 而非纯 DOM，是**因为 element 无法穷举**——任何自绘 element（`img.rs` 的自定义绘制、`canvas.rs`、编辑器、图表）都天然属于 canvas 层。
- 两者 z-order 由 paint 顺序合并（DOM 层按 bounds 绝对定位，canvas 层作为单独 `<canvas>` 叠加在对应区域）。
- 结构上可复用现有 `Scene`：canvas 层继续走 wgpu，DOM 层只在 reconcile 时更新。

## 5. 落地方式与依赖

| 功能 | 技术 | 说明 |
|------|------|------|
| 布局 | Taffy（现状） | 不动 |
| DOM 树构建/复用 | 核心新增 `dom_backend` 模块（feature 门控） | 不依赖外部 crate |
| DOM 操作 | `web-sys` | 现状已有 |
| JS/WASM 绑定 | `wasm-bindgen` | 现状已有 |
| HTML/Markdown（后期） | `html5ever` / `pulldown-cmark`（工作区已用 0.12） | 独立 feature，不与核心耦合 |
| 图片 | 浏览器 `<img>` | 懒加载/缓存免费 |
| SVG | 浏览器 `<svg>` | 复用现有 `svg` 元素数据 |
| 无障碍 | 浏览器原生（DOM 语义） | AccessKit 桌面路径不动 |

**门控与隔离**：核心新增全部用 `dom-backend` feature 隔离，默认关；`rgpui-web` 启用。桌面四平台零影响（AGENTS.md 完整性清单 1~18 全保）。

## 6. 分阶段路线

### 第一阶段：跑通最小 DOM Backend（约 2~4 周）

- 核心新增 `dom_backend` 模块：`DomNode`/`RetainedDomTree`/`reconcile`（feature 门控）。
- `rgpui-web` 新增 `WebDomBackend`：`web-sys` 创建节点、按 `GlobalElementId` 增量更新。
- 首批 DOM 化元素：`div` → `<div>`、`label`/文本 → `<span>`、`button` → `<button>`、`img` → `<img>`。
- Taffy 结果以 absolute 定位落地；事件委托 `data-gpui-id` → `PlatformInput`。
- 验收：rgpui_story 的侧边栏/标题栏/基础组件以 DOM 呈现，文本可选中可复制，IME 正常。

### 第二阶段：复杂文档与表单（约 1~2 个月）

- `scrollable` → 原生滚动；`Input`/`TextArea` → 原生输入（IME/剪贴板免费获得）。
- 图片/链接/SVG 完整化；表格/列表滚动 DOM 化。
- Markdown/`Html()` widget（html5ever）作为独立 feature。
- Hybrid 分层完善：canvas 层只覆盖图表/编辑器等自绘元素。

### 第三阶段：Hybrid 与性能（约 2~3 个月）

- 显式 `CanvasLayer` / `DomLayer` 分层渲染，z-order 合并。
- 事件命中、焦点、无障碍与 DOM 树对齐。
- 体积/性能优化：DOM 更新批量提交、减少重建、wasm 体积裁剪（替代当前 3xx MB debug 产物）。

### 第四阶段（长期）

- CSS 样式系统（可选，仍以绝对定位为默认）。
- SSR / Hydration（DOM 后端天然更接近可 SSR 形态）。
- Web Components / Shadow DOM（可选）。

## 7. 风险与权衡

| 风险 | 说明 | 缓解 |
|------|------|------|
| 浏览器排版与 Taffy 度量不一致 | 视觉像素级差异 | 第一版接受；精确排版走 canvas 层 |
| 元素无法穷举 | 自定义元素无法 DOM 化 | 第一版即 Hybrid，Foreign 层兜底 |
| 每帧重建 element 树 | 节点复用靠 GlobalElementId，需保证 key 稳定 | 与现有 hitbox/accesskit 复用机制同理，成熟模式 |
| 双路径维护成本 | DOM 路径与 Scene 路径并行 | DOM 路径只覆盖核心元素；两路径共享 Taffy 布局与事件结构 |
| 输入体验回退 | 原生 `<input>` 与自绘输入渲染不同 | 复杂输入第一版保持 canvas |

## 8. 明确不做的事

- **不引入 Dioxus/Yew 等前端框架**：本方案是给 rgpui 增加 DOM 后端，不重做生态。
- **不做通用 VirtualDOM diff 引擎**：rgpui 没有保留 widget 树，DOM 复用靠 `GlobalElementId` 映射，复杂度更低。
- **第一版不支持 CSS**：全部样式来自 rgpui Styled，映射为内联 style（absolute 定位）。
- **不修改 Entity/Context/事件模型**：桌面与 DOM 后端共用同一套。

## 9. 实际实现记录（2026-08-20）

> 本文档前半部分（§1~§8）是规划；本节约定**已落地的实际形态**。实施过程中方案有两处偏离规划：

### 9.1 偏离一：叠加层（canvas + DOM 文本）→ 纯 DOM 渲染

初版按 §4.9 的 Hybrid 叠加层思路实现（`00ca144`）：canvas 画全部形状，DOM 层只叠加文本 span。
实测暴露根因——**坐标复合错误**：DOM 节点写入 Taffy 的**窗口绝对坐标**，但 CSS `position:absolute`
子元素相对最近 positioned 祖先定位，嵌套节点（host → button div → label div → text span）的偏移逐层累加，
导致按钮文字丢失、布局错乱、canvas 与 DOM 双重渲染。

**决策**（与用户确认）：放弃叠加层，重建为**纯 DOM 渲染**——DOM 层是主渲染器，canvas 视觉隐藏
（`opacity:0`），`WgpuRenderer::draw` 被跳过。DOM 节点坐标统一换算为**父相对坐标**（`origins` 栈）。

### 9.2 偏离二：容器节点从"透明结构"改为"完整视觉"

规划 §4.9 中 DOM 容器是透明定位结构。纯 DOM 模式下 canvas 被隐藏，容器必须自带视觉：
`Div::dom` 现映射背景（纯色 / 线性 / 径向 / 锥形渐变）、圆角、边框（颜色 + 统一宽度）、
盒阴影（内 / 外）、透明度、光标、`overflow`。`DomStyle` 相应新增 `background_gradient` /
`border_color` / `border_width` / `box_shadows` 字段及 `DomGradient` / `DomBoxShadow` 类型。

### 9.3 落地清单

| 项 | 位置 | 说明 |
|----|------|------|
| 坐标修复 | `crates/rgpui/src/dom.rs`（`DomTreeBuilder::register`） | `origins` 栈与节点栈平行，父相对坐标换算 |
| 样式模型扩展 | `crates/rgpui/src/dom.rs` | `DomGradient`（Linear/Radial/Conic）、`DomBoxShadow`、`DomStyle` 新字段 |
| 完整形状映射 | `crates/rgpui/src/elements/div.rs`（`Div::dom`） | 渐变/边框/阴影/圆角/透明度/光标/overflow |
| CSS 序列化 | `crates/rgpui-dom/src/css.rs` | `linear-gradient(...)` / `radial-gradient(...)` / `conic-gradient(...)`、`border`、`box-shadow`；删除无用的 `dom_structure_to_css` |
| 纯 DOM 模式 | `crates/rgpui-web/src/window.rs` | `dom_tree_update` 时隐藏 canvas（`opacity:0`），`draw` 跳过 `renderer.draw` |
| 节点样式统一 | `crates/rgpui-dom/src/web.rs` | 元素节点改用完整视觉样式（`dom_style_to_css`），不再区分结构/文本样式 |
| 形状映射到 Stateful | `crates/rgpui/src/elements/div.rs`（`Stateful<E>::dom`） | `button`/`checkbox`/`radio` 的根元素是 `Stateful<Div>`，此前未实现 `dom()`，纯 DOM 模式下形状不显示；现委托给内部元素 |
| 事件转发防递归 | `crates/rgpui-dom/src/web.rs` | 合成事件设 `bubbles:false` 且顶部检查 `FORWARD_MARK`，避免合成事件冒泡回 `document` 重入同一 wasm-bindgen 闭包（"closure invoked recursively"） |
| 样式串去重缓存 | `crates/rgpui-dom/src/web.rs` | `WebDomBackend.styles` 缓存节点 CSS 串，未变化时跳过 `setAttribute`，减少 DOM 写 |
| 同 id 兄弟消歧 | `crates/rgpui/src/dom.rs`（`DomTreeBuilder::register`） | 多个元素复用同一 `ElementId`（如 story 三个 Input 共享同一 entity）导致 `GlobalElementId` 碰撞时，重复实例回退为匿名式 `dom_path` 消歧，首个实例保留干净 key |

### 9.4 验证状态

- `cargo check --workspace` ✅
- `cargo check -p rgpui --features dom-backend` ✅
- `cargo test -p rgpui-dom`（14 项，含新增渐变/边框/阴影序列化用例）✅
- `cargo +nightly check -p rgpui-web --target wasm32-unknown-unknown` ✅
- `cargo +nightly build -p rgpui_story --target wasm32-unknown-unknown` ✅
- clippy：本次改动零新增告警（`tag_input.rs:301` 冗余 clone 为存量问题）
- 浏览器视觉验收：**进行中**（`trunk serve` 后确认按钮文字/布局正常、无双渲染）
- 已修复运行时问题：
  - 按钮形状不显示（`Stateful::dom` 委托）
  - 事件转发递归崩溃（`FORWARD_MARK` + `bubbles:false`）
  - `DOM key 重复` 崩溃（同 id 兄弟消歧，根因：story 三个 Input 复用同一 entity）
  - **右键菜单崩溃**（`rgpui-web/src/events.rs` `register_context_menu`：document 级 `doc_closure`
    注册后未被持有就 drop，右键 `contextmenu` 触发已释放的 wasm-bindgen 闭包报 "closure invoked
    recursively or after being dropped"；现改为返回闭包向量一并保活）

### 9.5 下一步（后续迭代）

- `button` / `img` / `svg` / `scrollable` 等核心元素 DOM 化（纯 DOM 模式下 canvas 已隐藏，这些元素 v1 不显示）。
- 图标（`Icon`）SVG 映射，替代 canvas 精灵图。
- 输入组件（`Input`/`TextArea`）DOM 化，获得原生 IME/剪贴板。
- 文本选择与点按交互的边界（起始点在可点元素标签上的取舍）打磨。

### 9.6 DOM 事件委托（点击错位修复）

**背景**：纯 DOM 模式下 canvas 已隐藏（`opacity:0`）但保留为不可见命中传感器，点击事件
走 `canvas` 监听器按坐标 hit-test 命中。滚动容器由浏览器原生 `overflow:auto` 滚动，而 gpui
hitbox 使用自己的滚动偏移，DOM 层与 canvas 命中空间在滚动/缩放下错位，导致"点滑块却打开
/选中的是选择类控件"。

**方案**：点击 DOM 覆盖层上的元素时，改按 DOM 树身份（`data-gpui-id` → `DomNodeKey`）直接
命中对应 hitbox，绕过坐标 hit-test。

**改动清单**：

| 项 | 位置 | 说明 |
|----|------|------|
| DOM key 提前到 prepaint | `crates/rgpui/src/element.rs`（prepaint） | 在 bounds 计算后、`element.prepaint` 前调用 `window.dom_element` 压栈，prepaint 后 `dom_exit` 配对出栈；paint 阶段移除原 `dom_element`/`dom_exit` |
| 覆盖层全可命中 | `crates/rgpui-dom/src/web.rs`（`attach_default`） | 宿主样式 `pointer-events:none` → `pointer-events:auto`，全部节点可命中（纯 DOM 模式下 DOM 层是主渲染器）；文本节点保留 `user-select:text` |
| key→hitbox 映射 | `crates/rgpui/src/window.rs`（`Frame.dom_key_hitboxes`） | `insert_hitbox` 时用 `current_dom_key()`（DOM 栈顶）把 hitbox 记入 `Frame.dom_key_hitboxes`；`Frame::new`/`clear` 同步初始化/清理 |
| 显式命中派发 | `crates/rgpui/src/window.rs`（`dispatch_event_for_dom`） | 新增 `dispatch_event_for_dom(keys, event, cx)`，`dispatch_event` 重构为内部 `dispatch_event_inner` 支持可选的 key 覆盖；`dispatch_mouse_event` 按 key 链构造 HitTest（`dom_keys_hit_test`） |
| 平台回调 | `crates/rgpui/src/platform.rs`（`PlatformWindow::on_dom_event`） | 新增默认空实现；`Window::new` 在 `on_input` 后注册，转发到 `dispatch_event_for_dom` |
| id→key 反查表 | `crates/rgpui-dom/src/web.rs`（`WebDomBackend.id_to_key`） | 每帧 `update`/`rebuild` 时从树重建 `data-gpui-id` → `DomNodeKey` 映射 |
| 委托回调注入 | `crates/rgpui-dom/src/web.rs`（`set_dom_event_handler`） | `rgpui-web` 首次创建后端时注入，转发器命中 `data-gpui-id` 链时调用 |
| 事件转换 | `crates/rgpui-web/src/events.rs`（`dispatch_dom_event`） | 把 DOM 事件按类型转换为 `PlatformInput`（位置用 client 坐标减 canvas 矩形，与 canvas 监听器坐标空间一致），连同 key 链交给核心 |
| 平台注册 | `crates/rgpui-web/src/window.rs` | `WebWindowCallbacks.dom_event` 字段 + `on_dom_event` 实现 + `dom_tree_update` 首次创建后端时注入回调 |

**验证状态**：

- `cargo check --workspace` ✅
- `cargo check -p rgpui --features dom-backend` ✅
- `cargo test -p rgpui-dom` ✅
- `cargo +nightly check -p rgpui-web --target wasm32-unknown-unknown` ✅
- `cargo +nightly build -p rgpui_story --target wasm32-unknown-unknown` ✅
- clippy：本次改动零新增告警（`tag_input.rs:301`、`div.rs:1964`、`util/mod.rs:203` 为存量问题）
- 浏览器复测：待用户验证（`trunk serve` 侧边栏点击、滑块、选择控件、右键菜单）

**已知限制**：覆盖层全可命中后，canvas 不再收到原生 `pointerenter/leave`，窗口级 hover 状态
（`on_hover_status_change`）在纯 DOM 模式下不再更新（DOM 层本就不渲染 hover 样式，视觉无感）。
若后续需要窗口级 hover 感知，可在宿主上补监听 pointerenter/leave 并合成状态变更。