# rgpui Web DOM Backend 使用指南

> 文档日期：2026-08-20
> 状态：**第一版已落地（Phase 1）**
>
> 配套文档：`docs/web-dom-backend-plan.md`（规划）、`docs/web-dom-backend-analysis.md`（调研）、`docs/research-text-selection.md`（文本选择调研）

## 1. 两种"渲染后端"的含义

rgpui 的 Web 平台（`rgpui-web`）目前有两条渲染路径，可并存：

| 后端 | 实现 | 特点 |
|------|------|------|
| **Canvas（默认）** | element → Taffy 布局 → `Scene` → wgpu 画到 `<canvas>` | 与桌面完全同源；性能确定；但没有浏览器原生文本能力 |
| **DOM 覆盖层（可选）** | paint 阶段并行构建保留的 DOM 树 → `rgpui-dom` 对账 → 绝对定位 `<div>/<span>` 覆盖层 | 浏览器原生提供**文本选择 / 复制 / IME / 无障碍**；**不做双重绘制** |

> **关键概念**：DOM 后端**不是替换 canvas**，而是在 canvas 之上叠加一层绝对定位的 DOM。**canvas 继续负责全部形状绘制（背景/边框/图标/输入框等），DOM 层只负责文本**：开启 DOM 层后，`Text` 元素的字形不再绘制到 canvas，改由 DOM `<span>` 渲染（因此可选中/复制）；DOM 的元素/容器节点是**透明的定位结构**，不重复画背景。两者各司其职、互不重叠。

布局不依赖浏览器重排：DOM 节点一律用 Taffy 的 `position:absolute + left/top/width/height` 结果 1:1 落地，与 canvas 像素对齐。

## 2. 依赖与代码结构

| crate | 作用 |
|-------|------|
| `rgpui`（feature `dom-backend`） | DOM 数据模型：`DomTree` / `DomNode` / `DomNodeKey` / `DomStyle` / `DomTreeBuilder`，以及运行时开关 |
| `rgpui-dom` | 增量对账（`reconcile` → `DomPatch`）、HTML 序列化（`to_html`）、CSS 序列化、wasm 端 `WebDomBackend` |
| `rgpui-web` | 在 `WebWindow::supports_dom` / `dom_tree_update` 接入 DOM 层 |

调用链：核心每帧 `draw_roots` 里由 `Element::dom()` 登记的节点构建一棵新鲜 DOM 树 → `PlatformWindow::dom_tree_update` 交给 `rgpui-dom` → 与上一帧树对账、增量应用到真实 DOM。

## 3. 如何启用

### 3.1 Cargo 侧：开启 feature

`rgpui-web`（及其平台入口 `rgpui-platform`）已**无条件依赖** `rgpui-dom` 与 `rgpui` 的 `dom-backend` feature，因此 Web 应用**不需要额外引入 rgpui-dom**，只要让代码能访问 `set_dom_layer_enabled`：

- 若你的应用直接依赖 `rgpui`，在其 Cargo.toml 中打开 feature：

```toml
[dependencies]
rgpui = { workspace = true, features = ["dom-backend"] }
```

- 桌面端：不开启该 feature 即完全不受影响（桌面平台不实现 `supports_dom`）。

### 3.2 运行时：打开 DOM 层开关

DOM 层默认**关闭**（保持纯 canvas 行为）。在**打开窗口之前**调用：

```rust
// main() 中、application().run(...) 之前
rgpui::set_dom_layer_enabled(true);
```

Web 窗口的 `supports_dom()` 会读取该开关：`true` 时核心开始构建并交付 DOM 树，`false` 时完全走纯 canvas 路径。

> 桌面平台不实现 `supports_dom`，此开关对桌面无影响，可在 `#[cfg(target_family = "wasm")]` 下调用以免桌面构建告警。

## 4. 如何定制后端（Canvas 还是 DOM）

一句话：**DOM 层是运行时开关，随时可以在「纯 canvas」与「canvas + DOM 覆盖层」之间切换**。

```rust
// 纯 canvas（默认）——文本由 wgpu 绘制，无浏览器原生文本能力
rgpui::set_dom_layer_enabled(false);

// canvas + DOM 覆盖层——文本可选中/复制/IME，接受双重绘制
rgpui::set_dom_layer_enabled(true);
```

选择建议：

- **需要文本选择 / 复制 / 输入法（IME）**：开启 DOM 层。
- **追求极致性能 / 纯动画或图表场景**：保持关闭，走纯 canvas。
- **动态切换**：目前开关在窗口创建前读取一次；同一窗口运行时切换尚未支持（后续版本可做成 `Window` 级配置）。

## 5. 字体对齐（消除"重影"）

开启 DOM 层后，文本字形由 DOM 渲染，但浏览器并不知道应用内嵌的字体。
若 DOM 层文本的 `font-family` 浏览器里不存在，会回退到默认字体，与应用预期不符。
解决办法是让浏览器加载与应用**同一份**字体字节：

```rust
// 打开窗口之前，把应用内嵌的字体按 DOM 层使用的字族名注册给 DOM 后端
rgpui::set_dom_font_face("Inter Variable", include_bytes!("fonts/Inter-Regular.ttf"));
rgpui::set_dom_font_face("JetBrains Mono", include_bytes!("fonts/JetBrainsMono-Regular.ttf"));
```

- 注册表是**线程局部、进程级**的；`rgpui-dom` 挂载覆盖层时一次性读取，注入为 `@font-face`（base64 data URI）。
- 字族名必须与**主题的 `font_family` / `mono_font_family`**（即 DOM 样式输出的 `font-family`）一致；`rgpui-web` 平台已默认注册 `.SystemUIFont`（→ 内嵌 IBM Plex Sans），覆盖未自定义字体的应用。
- CSS 序列化会对 `font-family` 统一加引号（`font-family:"Inter Variable"`），否则多词字族会被解析成多个族名、以 `.` 开头的字族不是合法 CSS ident，都会静默失效。
- 未注册的字体（如个别 CJK/Emoji 回退）仍可能显示为浏览器默认字体，属已知限制。

## 6. 文本选择与事件转发

开启 DOM 层后，**文本 `<span>` 会开启 `pointer-events:auto`**（其余覆盖层区域保持穿透），
因此可以用鼠标**拖选 / 复制 DOM 文本**。为了让应用交互不受覆盖层遮挡：

- `rgpui-dom` 在 `document` 上注册**捕获阶段转发器**：落在覆盖层内的 `pointerdown` / `pointerup` /
  `pointermove` / `wheel` 会以同坐标合成事件**转发到 canvas**，应用按原来的 hitbox 体系处理（点击按钮、
  悬停、滚轮滚动均不受影响）。
- 转发发生在浏览器对 span 的默认选择动作**之前**，且不拦截原始事件，因此文本选择与交互可共存。

## 7. 示例

| 示例 | 路径 | 说明 |
|------|------|------|
| `hello_web` | `crates/rgpui-web/examples/hello_web/` | 最小可运行示例，`main()` 中演示了开启 DOM 层的写法 |
| `rgpui_story` | `examples/rgpui_story/` | 组件示例大全（侧边栏 + 大量文本），wasm 下已开启 DOM 层并注册 Inter 字体 |

运行方式（Web）：

```bash
# 前置：nightly 工具链 + wasm 目标 + trunk
rustup toolchain install nightly
rustup target add wasm32-unknown-unknown --toolchain nightly
rustup component add rust-src --toolchain nightly
cargo install trunk

cd crates/rgpui-web/examples/hello_web && trunk serve
cd examples/rgpui_story && trunk serve
```

## 8. 当前能力与限制（v1）

DOM 化的内容（canvas **不再**绘制这些文本）：

- 文本（`Text` / `&'static str` / `SharedString` / `StyledText`）→ `<span>`：颜色、字号、字体族（加引号输出）、字重/样式、行高、对齐、空白处理。
- `div` 等元素/容器 → **透明的**定位 `<div>`（仅用于对账结构与 `overflow` 裁剪），视觉由 canvas 绘制。

仍在 canvas 绘制的（未 DOM 化，`Element::dom()` 默认返回 `None`）：

- `button`、`img`、`svg`、`scrollable`、图表、编辑器、输入框（`Input`/`TextArea`）等——canvas 负责其全部绘制与文本。

已知差异与注意：

- 开启 DOM 层后，`Text` 元素的字形不再绘制到 canvas，改由 DOM 渲染（可选中/复制）；行背景（高亮）仍由 canvas 绘制。
- `StyledText` 的多段样式（runs，如粗体/彩色片段）v1 会**退化为统一基础样式**（浏览器只支持单 span）。
- 文本装饰（下划线/删除线）、输入框内文本的 IME 合成、编辑器（tree-sitter 高亮等）属后续阶段。
- 拖选文本时若起始点落在可点击元素（如按钮）的标签上，松开时可能同时触发该元素，属 v1 取舍。
- DOM 元素节点透明、不拦截事件，**所有交互事件仍由 canvas 的 hitbox 体系处理**，覆盖层 div 区域保持 `pointer-events: none`，不影响点击/悬停。
- wasm 体积：debug 构建约 3xx MB（`trunk.toml` 已配置 `data-wasm-opt` 等裁减项，release 明显更小）。

## 9. 相关 API

| API | 位置 | 说明 |
|-----|------|------|
| `set_dom_layer_enabled(bool)` / `dom_layer_enabled()` | `crates/rgpui/src/dom.rs` | Web DOM 层运行时开关（默认关） |
| `set_dom_font_face(family, data)` / `dom_font_faces()` | `crates/rgpui/src/dom.rs` | 注册/读取 DOM 覆盖层字体面（注入 `@font-face`） |
| `PlatformWindow::supports_dom()` / `dom_tree_update(&DomTree)` | `crates/rgpui/src/platform.rs` | 平台窗口接入点（cfg 门控默认实现） |
| `Element::dom()` | `crates/rgpui/src/element.rs` | 元素声明 DOM 化语义，默认 `None` |
| `DomTree` / `DomTreeBuilder` | `crates/rgpui/src/dom.rs` | 保留的 DOM 树与构建器 |
| `reconcile` / `DomPatch` / `DomBackend` / `WebDomBackend` / `to_html` | `crates/rgpui-dom/src/` | 对账、平台后端、HTML/样式序列化 |