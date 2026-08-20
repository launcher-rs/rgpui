# rgpui Web DOM Backend 深入分析（参考项目调研）

> 文档日期：2026-08-19
> 状态：调研分析，作为 `docs/web-dom-backend-plan.md`（规划）的支撑材料
>
> 调研对象：`dioxuslabs/dioxus`、`tokio-rs/topcoat`、Flutter Web（HTML renderer 演进 + HtmlElementView）、leptos、react 等
> 事实依据：对 rgpui 核心与 `rgpui-web` 的代码勘察（关键位置见 §6 与各节引注）

## 0. TL;DR（结论先行）

现有规划（`docs/web-dom-backend-plan.md`）的**总体方向正确**，且与业界"核心/渲染器分离 + 增量 patch"的主流做法一致。但参考 Flutter Web 十年演进后，本分析给出三条**必须修正**的结论：

1. **DOM 后端只能做"语义增强的增量层"，不能做默认或唯一渲染路径。** Flutter 在 2024-2025 年正式废弃并移除了 HTML renderer，理由是"复杂、性能差、图形表达力弱、维护成本高"。rgpui 的 DOM 路径必须定位为**可选的、面向文本/表单/无障碍的混合层**，canvas（wgpu）保持为主渲染路径与兜底。规划中"第一版就建成 Hybrid"的判断因此**更加关键**，应上升为首要原则而非阶段性目标。

2. **DOM 树不会天然获得"语义"。** Flutter 的 HTML renderer 输出的是"div 堆 + 小 canvas 贴片"，`<button>` 并不输出为 `<button>`，无障碍靠**独立的 semantics tree**。rgpui 若只做"div/span 的绝对定位映射"，文本选择/IME 免费，但屏幕阅读器、SEO 拿到的仍然是"div 汤"。要拿到真正的语义，必须走规划中的**方案 B（语义优先）**：显式把 `button→<button>`、`input→<input>`、`img→<img>`、`aria-*` 写进 DOM。这同时意味着：**DOM 层本身就是 rgpui-web 缺失的 accessibility 层的替代方案**（现状 `rgpui-web` 完全没有 AccessKit 代码，见 §6.5）。

3. **混合分层（DOM 与 canvas 共存）的难点不在建树，而在 z-order 合成与事件策略。** Flutter HtmlElementView 的经验表明：DOM 元素天然"浮"在 canvas 之上，要让 canvas 内容穿插到 DOM 层之间，浏览器需要额外合成多个 canvas 层（成本随数量上升，官方建议尽量少）。rgpui 的 paint 顺序是唯一 z-order 依据，第一版应**禁止 DOM/canvas 细粒度交错**，只允许"DOM 整体在下 / 整体在上 / 少量 canvas 切片"，否则合成复杂度失控。

## 1. 横向对比：主流"DOM 化"路线的四种范式

| 范式 | 代表 | 核心机制 | 对 rgpui 的启示 |
|------|------|----------|----------------|
| **VDOM + Mutation 流** | Dioxus、React、Brick | 保留虚拟树 → diff → 生成序列化 mutation → 渲染器 patch 真实 DOM | rgpui 无保留树，改以 `GlobalElementId` 复用做"每帧快照对账"，见 §2、§6.2 |
| **DOM-as-唯一渲染器** | Flutter HTML renderer（已废弃） | 整个 UI 用 DOM/CSS 呈现 | **反面教材**：不把 DOM 当默认路径，见 §3.1 |
| **混合（DOM-in-canvas）** | Flutter HtmlElementView、游戏 HUD | 主渲染走 canvas，个别组件嵌入原生 DOM | 正是规划要的 Hybrid；合成与事件是难点，见 §3.2、§6.4 |
| **SSR + 客户端反应性** | Topcoat | 服务端渲染全部 HTML，`$(...)` 闭包编译成 JS 客户端重跑 | 为 Phase 4（SSR/Hydration）指明了一条低门槛路线，见 §4 |
| **细粒度响应式（非 VDOM）** | Leptos、Sycamore | Signal + 局部 DOM patch，无 diff | 与 rgpui"每帧重渲染"模型差异太大，仅作参考，见 §5 |

## 2. Dioxus 深度剖析（核心机制与 rgpui 最接近）

### 2.1 架构：VirtualDom + Mutation 流 + Renderer

Dioxus 把渲染拆成三层，`dioxus-core`（纯逻辑，不依赖 web）产出 **Mutation 流**，各渲染器（web/tui/desktop/mobile）只做两件事：**消费 mutation** 与 **上报 UserEvent**：

```
VirtualDom（保留虚拟树）
   → diff → Mutation 流（序列化 enum，栈机器语义）
   → renderer（web-sys / tui 等）apply_mutations
   → 用户事件 → UserEvent → VirtualDom 处理 → 新一轮 diff
```

Mutation 是**序列化**的：`CreateTextNode / LoadTemplate / SetAttribute / AppendChildren / InsertBefore / Remove / HydrateText / NewEventListener ...`。两条直接借鉴：

- **"diff 与 patch 分离"**：先算出全部变更再一次性提交，patch 阶段耗时极小、可被更高优先级打断。rgpui 的 DOM 后端应同样**把"计算变更"（core）与"应用变更"（rgpui-web）分开**，core 只产出"该 DOM 层增删改哪些节点"的抽象变更集，web 端用 web-sys 应用。这样 core 可测试、可跨目标复用（甚至未来序列化做 SSR/远程驱动）。
- **节点的"栈机器"管理**：`LoadTemplate` 压栈、`AppendChildren` 弹栈，配合 `ElementId(u64)` 在稀疏 Vec 里索引真实节点。rgpui 无需照搬栈机器（因为我们有现成的 element 树结构），但"**数值 ID + 稀疏数组**存储 DOM 节点"值得借鉴（见 §6.1）。

### 2.2 Template：静态结构不参与 diff

Dioxus 的 `rsx!` 在编译期把 UI 结构编译成 `Template`：静态部分（标签、静态属性、结构）只建一次并克隆复用，动态部分（文本、动态属性、事件）以占位符 + `node_paths` 定位。**静态节点永不参与 diff**——这是 Dioxus 性能的关键之一。

对 rgpui 的映射：rgpui 的 element 树每帧重建，**没有编译期静态结构可省**，但 `GlobalElementId` 稳定复用 + 逐节点等值检查，在"应用变更"这一层达到类似效果：结构没变 → 不触碰 DOM。因此 rgpui 需要的是**"每帧对账、变更才 patch"**，而不是 Dioxus 的"模板预构建 + 克隆"（后者是编译器红利，rgpui 拿不到，也不必强求）。

### 2.3 事件循环

Dioxus web 渲染器的事件循环是 `select(等用户事件, 等内部 work)`，先 `rebuild` 一次，之后每次事件或内部更新就 `work_with_deadline` 产出一批 mutation 应用。**Dioxus 是事件驱动**（没事不重渲染）。

rgpui 的 WebWindow 是**rAF 驱动、每帧全量 draw**（`create_raf_closure` 无条件重新调度 rAF，`crates/rgpui-web/src/window.rs:314-341`）。DOM 后端不能假设"只在有变更时唤醒"，必须接受"每帧可能被调、但内部短路"的现实——见 §6.2 的脏跟踪设计。

### 2.4 对 rgpui 的借鉴与差异总结

| 维度 | Dioxus | rgpui（DOM 后端） |
|------|--------|-------------------|
| 树模型 | 保留的 VirtualDOM | element 树每帧重建（`window.rs:3010 draw_roots`） |
| 复用依据 | 位置 + type + key | `GlobalElementId`（`element.rs:213`，跨帧稳定） |
| 变更输出 | 序列化 Mutation 栈机器 | 抽象变更集（create/update/remove，按 GlobalElementId 索引） |
| 渲染器职责 | 消费 mutation + 上报事件 | 消费变更集 + 上报事件（可复用现有 events.rs 转换） |
| 触发 | 事件驱动 | rAF 驱动，需内部短路 |

**结论**：rgpui 的 DOM 后端 = "**无保留树的、按 GlobalElementId 对账的 Mutation 模型**"。规划 §4 的描述（RetainedDomTree + reconcile + 增量 patch）与之一致，方向可确认。

## 3. Flutter Web 深度剖析（最大警示）

### 3.1 HTML renderer 的兴衰：为什么不把 DOM 当默认渲染器

Flutter Web 曾提供三种渲染器：HTML（DOM）、CanvasKit（Skia→wasm/WebGL）、skwasm。2024 年官方发布"Intent to deprecate and remove the HTML renderer"（[docs.flutter.dev/to/web-html-renderer-deprecation](https://docs.flutter.dev/to/web-html-renderer-deprecation)），2024.08 正式宣布移除计划，默认改为 CanvasKit。官方给出的理由：

- **复杂**：同时维护两套渲染栈，双倍工作量。
- **性能差**：交互与图形表达力全面落后于 WebGL 方案。
- **表达力有限**：复杂的合成、滤镜、混合在 DOM/CSS 里难以表达或代价极高。
- **语义并不免费**：HTML renderer 输出的 DOM"从来不是地道 HTML"，一个按钮输出为"一堆 div 和 2D canvas 贴片"的混合体；无障碍靠独立的 semantics tree 而非 DOM 本身。

对 rgpui 的启示（这是本分析最重要的外部教训）：

1. **DOM 后端永远只是"增强层"，不是"替代层"**。canvas 路径保留为默认、为兜底、为精确渲染路径。规划 §4.1 的"新增一条 DOM 影子树、不替换现有渲染"与这条教训完全吻合，但需要把"Hybrid"从路线图阶段**提升为第一原则**。
2. **图形表达力弱的元素（图表、编辑器、3D、粒子）永远留在 canvas**。规划 §4.9 已正确划界，本分析确认这条边界必须**严格执行**，防止 DOM 层被滥用导致性能与维护双双恶化。
3. **"语义"要显式给，不能指望 DOM 自动生成**。哪怕 rgpui 的 div/span 绝对定位能 1:1 落地，屏幕阅读器读到的是 `role` 缺失的 div 堆。真正有价值的语义来自"有意义的元素映射 + ARIA"——这正是方案 B 的论据（见 §6.5）。

### 3.2 HtmlElementView：DOM-in-canvas 混合模型（Hybrid 的直接参照）

Flutter 在 canvas 渲染下用 `HtmlElementView`（platform view）嵌入原生 DOM（视频、地图、WebView 等）。官方文档明确：**"嵌入 HTML 是潜在的高开销操作，能用 Flutter 等价物实现就避免使用"**。在 CanvasKit 下，DOM 内容要穿插到 Flutter 内容之间时，Flutter 需要**创建额外 canvas 层做合成**，平台视图越多合成成本越高，官方建议：**减少 canvas 层数量、尽量把 DOM 内容聚拢**。

对 rgpui 的 Hybrid 分层（规划 §4.9、第三阶段）的直接借鉴：

- **DOM 节点天然在 canvas 之上**（DOM 是独立的元素，浏览器按 document 顺序与 z-index 合成）。要让 canvas 内容显示在某个 DOM 节点之上，就必须把该 canvas 拆成独立层并设更高 z-index——即 Flutter 的"extra canvas"。
- **第一版禁止细粒度交错**：只允许两种稳定配置——(a) DOM 层整体在下、canvas 层整体在上；(b) DOM 层整体在上、canvas 层垫底（此时 canvas 区域是"画布窗口"）。细粒度交错（DOM、canvas、DOM、canvas 交替）留给后续用"paint 顺序切片 → 每片一个 canvas"支持，且要设数量上限。具体见 §6.4。

### 3.3 语义树与渲染树分离

Flutter 的语义（无障碍）来自独立的 **semantics tree**，HTML renderer 的 DOM 不承担无障碍职责；Screen reader / 爬虫 / 自动化工具都该走 semantics 而非 DOM。这一点与 rgpui 现状（核心已有 AccessKit a11y 树，`element.rs:365-401` 每帧登记）同构。

对 rgpui 的意义：**DOM 后端上线后，rgpui-web 的无障碍可以直接从"DOM 语义元素 + ARIA"免费获得，这是现状缺失的（rgpui-web 零 a11y 代码）**。但要做到"免费"，前提是方案 B 的语义映射足够完整；若只做 div 映射，则仍需把 AccessKit 树桥接成 DOM role，成本反而更高。因此：**方案 B 的语义映射投入，本质上是对 a11y 的一次性投资**，一举两得。

### 3.4 加载性能的教训

Flutter 社区反对废弃 HTML renderer 的最有力论据是**初始加载时间**：Slow3G 下 CanvasKit 75s vs HTML 45s，wasm 体积 ~4MB vs ~1MB。rgpui 现状的 3xx MB debug wasm + WebGPU 初始化正是同一痛点（规划 §1.1 已列）。**DOM 后端最直接的价值不是渲染更快，而是"文本/表单页无需完整 GPU 管线也能渲染"**——这为"轻量模式"（无 WebGPU 时的纯 DOM 降级）留了口子，是规划未点破的一个潜在卖点。

## 4. Topcoat 深度剖析（SSR 与无-wasm 反应性）

### 4.1 架构

Topcoat（tokio 团队）是"batteries-included 全栈框架"：**全部 HTML 在服务端渲染**，组件可 async、可直接查数据库；交互用 `$(...)` 表达式——它是**类型检查过的 Rust**，服务端用它做首屏渲染，同时**编译翻译成 JavaScript**，在浏览器里重跑，无需 wasm、无客户端构建：

```
view! {
    signal open = false;
    <button @click=$(|e| open.set(!open.get()))>"What is Topcoat?"</button>
    <p :hidden=$(!open.get())>"A full-stack Rust framework."</p>
}
```

需要服务端数据时用 `#[shard]` 标记组件，参数变化时服务端重渲染该组件并**原地替换 HTML**。

### 4.2 对 rgpui 的启示（Phase 4：SSR / Hydration）

规划 §6 第四阶段写"SSR / Hydration（DOM 后端天然更接近可 SSR 形态）"。Topcoat 提供了一条具体佐证与技术路线：

- **SSR 的前提是"语义化、可序列化的树"**。rgpui 的 `RetainedDomTree`（按 GlobalElementId 索引、含语义 tag/attr/文本）天然可序列化为 HTML 字符串——这比 Dioxus 的"mutation 序列化"（在远端重建 DOM）更直接。
- **Taffy 布局是纯 Rust**，服务端可先行算出全部绝对定位结果，DOM 树带上 bounds 序列化，客户端 hydration 只需对账补上事件与动态部分。**这使 rgpui 的 SSR 比多数 DOM 框架更自然**（多数框架依赖浏览器重排，rgpui 的绝对定位方案服务端可完整复算）。
- Topcoat 的"闭包编译成 JS"路线与 rgpui 的架构（wasm 为主）冲突，**不值得引入**；但"SSR 首屏 + 客户端水合"的方向成立，且 Topcoat 证明该路线的工程成本可被一个成熟团队消化。

**建议**：Phase 4 的 SSR 可行性论证应写入规划，明确"RetainedDomTree 可序列化 + Taffy 可服务端复算"是核心依据。

## 5. 其他参考（简要）

- **React（Fiber）**：`key` 机制与 rgpui 的 `GlobalElementId` 概念同构（React 用 key 在列表重排时保身份；rgpui 的 `ElementId::NamedInteger/Integer/Name` 栈天然就是这种 key）。React 还证明了"中断式、可优先级的 diff 调度"，rgpui 每帧全量对账可借鉴其"按优先级取消"思想，但实现成本高，**不建议引入**。
- **Leptos / Sycamore**：Signal 细粒度 patch，无 diff。与 rgpui"每帧重渲染 element 树"的模型不同源，**只作背景参照**；如果未来 rgpui 要降低重渲染成本，可参考其"仅订阅变化的部分写 DOM"的思路，但那属于更大的架构变更，超出本方案范围。
- **egui / 现状 canvas 路径**：作为"反例"参照——egui 证明了纯 canvas 即时模式能做完整 UI，但其 web 体验（无文本选择/IME 原生性）正是 rgpui 要改善的，也与 Flutter 的 CanvasKit 结论一致（图形强、文本/表单弱）。

## 6. 基于代码事实的关键决策深化

> 本节引用的行号来自对 rgpui 核心与 `rgpui-web` 的实际勘察。

### 6.1 GlobalElementId → 数值 DOM id：事件桥接的两种路线

规划 §4.8 的事件桥接写的是"DOM 事件 → 读 `data-gpui-id` → 解码为 GlobalElementId"。这里需要细化，因为 `GlobalElementId` 是 `Arc<[ElementId]>`（`element.rs:213`），其中 `ElementId` 枚举含 `Path / CodeLocation / Uuid / FocusHandle` 等变体（`window.rs:6447-6468`），**直接字符串化既不稳定也不便宜**。

更优做法：**DOM 属性只放一个数值 id**——沿用现有 `GlobalElementId::accesskit_node_id`（`element.rs:228-233`，DefaultHasher 哈希成 u64）的思路，在 `RetainedDomTree` 里维护 `HashMap<GlobalElementId, u64>` 分配/复用数值 id，DOM 节点只写 `data-gpui-id="<u64>"`。事件侧 `u64 → GlobalElementId` 反向查表（O(1)）。哈希跨构建是否稳定仅影响 SSR/hydration，而 hydration 的 DOM 由同一份代码在服务端/客户端生成，天然一致，不受影响。

**更进一步的简化（推荐）**：指针/鼠标事件其实**不需要 data-gpui-id**。因为 DOM 布局与 Taffy 结果 1:1（absolute 定位），DOM 事件的 `clientX/Y` 换算成窗口坐标后，**直接走现有坐标命中管线**——`draw_roots` 每帧已算好 `mouse_hit_test`（`window.rs:3077`），事件分发走 `Window::dispatch_event`（`window.rs:4884`）。也就是说鼠标类事件**复用现有 events.rs 的坐标转换即可，零改动命中逻辑**。`data-gpui-id` 只对"键盘事件要定位到具体元素"（DOM 原生输入焦点）或"无坐标的合成事件"有意义。这大幅缩小了第一版的事件桥接工作量。

### 6.2 每帧重建的代价：脏跟踪与短路（规划未覆盖）

WebWindow 每 rAF 无条件回调 `request_frame`（`window.rs:322-327`），核心每帧走 `draw_roots` 重建 element 树并 `present`（`window.rs:3010/2984`）。DOM 后端若每帧都做"全量对账 + patch"，等于每帧扫描整棵 DOM。

必须加**两级短路**：

1. **变更检测**：`RetainedDomTree` 生成时逐节点比较（bounds、style、text、kind），**只有变化的节点进入变更集**；结构相同则零 DOM 触碰。这利用了 GlobalElementId 稳定 + element 树每帧重建但结果稳定的特性（render 返回结构不变 → id 不变 → 等值 → 跳过）。这正是"对账"比 Dioxus diff 便宜的地方：Dioxus 要 diff 两棵树，rgpui 只需"新树节点 → 查旧树 → 比等值"。
2. **帧级短路**：核心已有 `needs_present`（`window.rs:2944`）。DOM 后端只在 `needs_present` 或内容实际变化时才执行 reconcile；纯 canvas 动画帧（scene 变了但 DOM 层没变）只重绘 canvas，不碰 DOM。

同时注意：**DOM 路径比 canvas 更便宜**（canvas 每帧全量提交 GPU，DOM 短路后可为零开销），这应作为验收指标写入第一阶段。

### 6.3 文本字体度量一致性：用 @font-face 对齐

规划 §4.5 接受"浏览器排版与 Taffy/cosmic-text 度量不一致"。这里有一个可以**显著缩小差异**的具体手段：cosmic-text 与浏览器各自用不同的字体栈，直接后果是同样的字号行高、字形宽度都可能不同。让 DOM 文本与 canvas 文本共用同一字体（把 rgpui 内嵌/加载的字体用 `@font-face` 注入，字重/字号一致）可把差异压到亚像素级。第一版可只对齐字族与字号，行高差异按规划接受；若需要像素级一致，对 span 逐个覆盖 line-height（Taffy 行高已知）。**建议把"字体对齐"写进第一阶段验收项**，否则"文本可选中"的体验会因为字体突变而显得突兀。

### 6.4 Hybrid z-order：paint order → z-index 切片

规划 §4.9 说"两者 z-order 由 paint 顺序合并"，但没给出机制。结合 Flutter 教训（§3.2），建议：

- 单一 `<div>` 覆盖层（absolute，全窗口）承载所有 DOM 节点，每个节点 `z-index` = 其 paint order。
- 单一 canvas 设基准 z-index（0 或最大），DOM 节点统一在其上/下。
- **第一版限制为两种稳定拓扑**（DOM 在下 / DOM 在上），禁止交错；交错（如"模态框在 DOM 层之上、canvas 图表穿插其中"）由"paint 顺序切片"支持：把 paint 序列切成 [DOM 段 | canvas 段 | DOM 段 ...]，每段 canvas 对应一个 `<canvas>`，段与段之间按 z-index 排布。**canvas 切片数量设上限**（Flutter 建议尽量少）。
- 与 §6.1 结合：命中测试仍用坐标（对齐），DOM 节点的 `pointer-events` 由"该坐标是否落在 canvas 层元素之上"决定，避免 DOM 遮挡 canvas 层元素的事件——但因为 hitbox 是权威分发（坐标命中后由 rgpui 决定谁接收），DOM 元素只需**不 stopPropagation**，事件仍按 rgpui 的 hitbox 顺序分发，视觉层级与命中层级天然一致。

### 6.5 语义映射的层级：组合式元素无需单独实现（对方案 B 的关键修正）

规划 §4.3 方案 B 说"要给内置元素逐个实现 dom()，约 10~20 个"。代码勘察给出一个**省力事实**：

- `button`（`elements/button.rs`）、`scrollable`（`elements/scroll/scrollable.rs`）是**组合式 `RenderOnce`，自身没有 paint**——它们最终都分解为 `div` + 文本/滚动容器（button 分解为 `Stateful<Div>` + label，`button.rs:255`）。也就是说，**DOM 后端只需覆盖有 paint 的叶子**：`div`（`elements/div.rs`，含 Interactivity 语义）、文本（`elements/text.rs`）、`img`（`elements/img.rs`）、`svg`（`elements/svg.rs`）、`scrollbar`、`input_ui`（`input_ui/element.rs`）。核心元素约 **5~6 个而非 10~20 个**。
- 语义（button/aria-role）不必在 button 层补：`Interactivity`（`div.rs:2306-2439`）已有 role/aria 数据（a11y 动作注册在 `2400-2413`），方案 B 的 `dom()` 只需**在 div 叶子层读出 role/aria 并转成 HTML 属性**。组合式元素（button/scrollable）的语义自然由叶子层组合得出。
- 这也印证 §3.3：**DOM 语义与 AccessKit a11y 登记是同源数据**（都在 Interactivity 层），方案 B 的一次实现同时补齐 web 端缺失的 a11y。

### 6.6 文本拦截点

规划 §2.2 已正确判断"DOM 必须在 element 层拿文本"。代码事实补充：文本的 paint 在 `TextLayout::paint`（`text.rs:792-827`）逐行调 `paint_glyph`，**shaping 结果在调用 paint_glyph 前已存在**（`StyledText`/`Text` 保存原始字符串与逐段样式）。因此方案 B 里文本的 `dom()` 应在 `Text`/`StyledText` 层**拦截原始文本 + 分段样式**，输出 `<span>`/`textContent`，**不要让 glyph 信息泄漏到 DOM**（`paint_glyph` 的图集坐标对 DOM 无意义）。这一拦截点的成本集中在 `elements/text.rs` 一处。

## 7. 对现有规划的修正清单

| 规划条款 | 验证/修正 |
|----------|-----------|
| §1.2 目标"新增 DOM 渲染后端，让浏览器交还能力" | ✅ 成立。修正措辞：定位为**语义增强的混合层**，不是渲染后端替换（Flutter 教训） |
| §2.2 结论"DOM 后端必须在 element paint 阶段做" | ✅ 成立，代码勘察确认（paint 阶段可拿 `GlobalElementId+Bounds`，`element.rs:95-104`） |
| §4.2 `RetainedDomTree`/`DomNode`/`reconcile` | ✅ 方向正确，与 Dioxus"核心产出变更、渲染器应用"同构。补充：**数值 id 索引**（§6.1）、**两级脏短路**（§6.2） |
| §4.3 方案 B"给内置元素逐个实现 dom()" | ⚠️ 修正：只需覆盖有 paint 的叶子 5~6 个（div/text/img/svg/scrollbar/input），组合式元素（button/scrollable）自动由叶子组合出语义（§6.5） |
| §4.5 文本"接受字体度量差异" | ⚠️ 补充：第一版用 `@font-face` 对齐字体族/字号，差异可压到亚像素；行高差异按接受处理（§6.3） |
| §4.8 事件桥接"data-gpui-id 解码 GlobalElementId" | ⚠️ 简化：指针/鼠标事件走**坐标 + 现有 hitbox 命中**（`window.rs:3077`），零改命中逻辑；data-gpui-id 仅用于键盘/合成事件，且用**数值 id**（§6.1） |
| §4.9 Hybrid 分层"z-order 由 paint 顺序合并" | ⚠️ 补充机制：单一 DOM 覆盖层 + z-index=paint order；第一版只允许两种稳定拓扑（DOM 上/下），交错靠"paint 切片多 canvas"且设数量上限（§6.4） |
| §6 第四阶段 SSR/Hydration | ✅ 可行性强化：`RetainedDomTree` 可序列化 + Taffy 可服务端复算，是 SSR 的核心依据（§4.2） |
| §7 风险"浏览器排版不一致" | ✅ 保留；补充字体对齐缓解（§6.3） |
| 未覆盖项 | 新增：**每帧对账的性能与短路**（§6.2）、**DOM 层即 web 端 a11y 层**（§3.3/§6.5）、**加载期无 GPU 的轻量 DOM 降级**（§3.4） |

## 8. 路线图修订建议

- **第一阶段**：加入三项硬性验收——(a) 无变化帧 DOM 零触碰（短路生效）；(b) DOM 文本与 canvas 文本字体一致（@font-face）；(c) rgpui_story 侧边栏/标题栏/基础组件 DOM 化后，鼠标事件仍按现有 hitbox 坐标命中。事件桥接按 §6.1 简化路线（坐标复用）。
- **第二阶段**：`scrollable` 原生滚动时，确认滚动后 Taffy bounds 与 DOM 布局同步回填 `ScrollHandle` 的路径（规划 §4.7）；`input_ui` 中简单输入映射 `<input>` 时，用原生 input 事件回灌 rgpui 值，复杂输入保持 canvas。
- **第三阶段**：z-order 切片（§6.4）作为专项任务，明确"canvas 切片数量上限"与事件策略；DOM 层与 hitbox 层级一致性做自动化校验。
- **长期（SSR）**：按 §4.2 路线，先做"RetainedDomTree → HTML 字符串"的序列化原型，验证 Taffy 服务端复算。

## 9. 风险清单更新（在规划 §7 基础上）

| 风险 | 说明 | 缓解 |
|------|------|------|
| DOM 层被滥用导致维护/性能恶化 | Flutter 废弃 HTML renderer 的主因是双栈维护成本 | DOM 层仅覆盖语义/文本/表单核心元素；图形元素强制留在 canvas（§3.1） |
| DOM 树语义不足（div 汤） | 屏幕阅读器/SEO 得不到真语义 | 方案 B 显式映射 tag + ARIA（§3.3、§6.5） |
| 混合 z-order 合成复杂 | DOM 与 canvas 穿插需要多 canvas 合成层 | 第一版限定两种稳定拓扑；交错走切片且设数量上限（§6.4） |
| 每帧对账开销 | rAF 驱动下每帧都可能触发 reconcile | 两级短路：逐节点等值 + 帧级 needs_present（§6.2） |
| 字体度量不一致 | 浏览器与 cosmic-text 字体栈不同 | @font-face 对齐字族/字号（§6.3） |
| 事件双通道歧义 | DOM 原生激活（click/submit）与 rgpui 事件并存 | 指针事件统一走坐标命中；原生激活仅补键盘/无障碍通道（§6.1） |

## 10. 结论

1. 规划总体成立，且与 Dioxus（核心/渲染器分离 + 变更集）、Flutter HtmlElementView（DOM-in-canvas 混合）、Topcoat（语义树 SSR）的成熟路线一致。
2. 三处必须修正：**DOM 定位为混合增强层而非替代层**（Flutter 教训）；**语义靠显式映射而非 DOM 自动生成**（方案 B 缩小到叶子元素 5~6 个）；**z-order 与事件策略需要在第一版就定死拓扑**（否则混合合成失控）。
3. 三处最有价值的补充：**坐标命中复用**大幅缩小事件桥接工作量、**两级短路**保证每帧对账不伤性能、**DOM 层同时补齐 rgpui-web 缺失的无障碍**。