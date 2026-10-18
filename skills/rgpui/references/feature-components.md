# 可选功能组件（Charts / Effects / QR-Code / WebView / DOM）

**目录：** [概述](#概述) · [Charts](#charts) · [Effects](#effects) · [QR-Code](#qr-code) · [WebView](#webview) · [DOM 后端](#dom-后端) · [最佳实践](#最佳实践)

## 概述

以下组件默认**不启用**，按 feature 门控（`crates/rgpui/Cargo.toml`）：

| feature | 依赖 | 内容 |
|---------|------|------|
| `charts` | — | 11 个图表（`components/charts/`） |
| `effects` | — | 装饰特效 7 个 |
| `qr-code` | `qrcode` | `QRCodeComponent` |
| `webview` | `wry` | `WebView`/`WebViewElement` |
| `dom-backend` | — | 原生文本 DOM 层（见 `rgpui-dom`） |

showcase 对应 bin 由 `required-features` 门控（`examples/showcase/Cargo.toml:109-122`）。

## Charts

`#[cfg(feature = "charts")] pub mod charts;`（`components/mod.rs:86`）+
`pub use charts::*`（`:167`）。11 个图表：`area_chart`、`bar_chart`、`chart`、
`donut_chart`、`gauge`、`heatmap`、`line_chart`、`pie_chart`、`radar_chart`、`treemap`。

`charts/mod.rs` 重导出：
- `bar_chart::{BarChart, BarChartData, BarChartMode, BarChartOrientation, BarChartSeries}`
- `chart::{Axis, AxisPosition, Chart, ChartArea, ChartPadding, DataPoint, DataRange, Legend, LegendPosition, Series, SeriesType, TooltipConfig}`
- `line_chart::{LineChart, LineChartPoint, LineChartSeries}`
- `pie_chart::{PieChart, PieChartLabelPosition, PieChartSegment, PieChartSize, PieChartVariant}`

其余模块为 `pub mod`（**未重导出**），经完整路径访问：`components::charts::gauge::Gauge`、
`heatmap`、`radar_chart::{RadarChart, RadarChartSize}`、
`treemap::{Treemap, TreemapItem}`、`area_chart::{AreaChart, AreaChartSize, AreaChartMode}`、
`donut_chart::{DonutChart, DonutChartSize}`。

`Gauge::new(id)`（`gauge.rs:66`）、`GaugeSize`（`:8`）。

## Effects

`#[cfg(feature = "effects")]`。`components/mod.rs` 声明并重导出的特效：
`Aurora`（`aurora.rs:9`/`new()` :24）、`Shimmer`（`shimmer.rs:9`/`new()` :16）、
`Ripple`（`ripple.rs:9`/`new(id, origin: Point<Pixels>, color: Hsla)` :19）、
`PulseIndicator`（`pulse_indicator.rs:9`/`new(id)` :18）、
`Marquee`（`marquee.rs:19`/`new(...)` :31）。

状态式：`Confetti`（`confetti.rs:200`/`new(id, Entity<ConfettiState>)` :208，
状态 `ConfettiState::new(cx)` :48）、
`ParticleEmitter`（`particle_emitter.rs:236`/`new(id, Entity<ParticleEmitterState>)` :244，
配置 `ParticleEmitterConfig` :25、`Particle` :8、状态 `ParticleEmitterState::new(cx)` :75）。

## QR-Code

`#[cfg(feature = "qr-code")] pub mod qr_code;`（`mod.rs:59`）+ `pub use qr_code::*`（`:140`）。
`QRCodeComponent`（`qr_code.rs:19`）：`new(data: impl Into<SharedString>)`（`:36`）。

## WebView

`#[cfg(feature = "webview")] pub mod webview;`（`rgpui.rs:253`），prelude 重导出
`WebView`/`WebViewHandle`（`prelude.rs:91`）。

`WebView`（`webview/mod.rs:55`）：`new(wry::WebView, window, cx)`（`:79`）、`show()`（`:91`）、
`hide()`（`:97`）、`is_visible()`（`:104`）、`bounds()`（`:109`）、`back()`（`:114`）、
`forward()`（`:119`）、`reload()`（`:124`）、`load_url(&str)`（`:129`）、`load_html(&str)`（`:134`）、
`handle()`（`:139`，返回 `WebViewHandle(Rc<wry::WebView>)` :41）、`eval_script(&str)`（`:149`）。
`WebViewElement`（`:191`）：`new(Rc<wry::WebView>, Entity<WebView>, window, cx)`（`:200`，`IntoElement`）。

```rust
let webview = wry::WebViewBuilder::new()
    .with_url("https://example.com")
    .build(&window)?; // 平台相关，示例见 examples/webview
let view = cx.new(|cx| WebView::new(webview, window, cx));
```

> **限制**：Web 端不可用；Windows 上 WebView2 子 HWND 显示在 DComp 内容之上
> （见 `windows.md`）；Linux 构建需 `libwebkit2gtk-4.1-dev`（AGENTS.md）。

## DOM 后端

`dom-backend` feature + `set_dom_layer_enabled(true)` 启用原生文本。
`PlatformWindow::supports_dom`/`dom_tree_update`/`on_dom_event`/`on_dom_scroll`
（`platform.rs:1205`-`:1231`）；`components/dom_svg`（`mod.rs:35`，私有）。
测试 `cargo test -p rgpui-dom`；示例 `hello_web`、`rgpui_story` 已开启。
详见 `docs/web-dom-backend-usage.md`。

## 最佳实践

- **默认特性不含这些组件**：用 `rgpui/{charts,effects,qr-code,webview}` 或
  `cargo run -p showcase --bin <bin> --features ...`。
- **Charts 图标模块部分需完整路径**（`charts::gauge::Gauge`）。
- **`scap`/`screen-capture` feature 已知编译失败**（`zed-scap` 与 `windows-capture`
  不兼容），不要启用。
