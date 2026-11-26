# 平台契约审计：待专项设计项

> 配套文档：[`linux-api-parity-audit.md`](linux-api-parity-audit.md)（第二轮审计与主修复记录）
> 分支：`fix/linux-api-parity`
>
> 本文件记录审计中**未实现**的四项。它们都不是「静默失效的功能缺陷」，而是
> 需要特定硬件/会话/平台才能设计并验证的能力缺口。每项给出可执行的方案，
> 方便在对应环境里接手。

---

## 1. Linux 热状态（审计 #21）

### 现状

`Platform::thermal_state()` 在 Linux 走 trait 默认 `Nominal`，
`on_thermal_state_change()` 是空实现（`linux/platform.rs:503-506`）。
调用方（如 `thermal_frame_interval`）永远拿不到降档信号。

### 为什么没直接做

Linux 没有统一的「系统热状态」API。常见来源：

| 来源 | 可读性 | 问题 |
|------|--------|------|
| `/sys/class/thermal/thermal_zone*/temp` | 通用 | 温度值因硬件而异，没有标准的 Nominal/Fair/Serious/Critical 阈值 |
| `/sys/class/thermal/thermal_zone*/mode` | 部分 | 只有「被动/启用」，无等级 |
| D-Bus `org.freedesktop.thermal` | 不普遍 | 多数桌面没有 |
| `hwmon` | 通用但底层 | 需知道具体传感器语义 |

当前开发机（Ubuntu 22.04 虚拟机 + xrdp）**没有 `thermal_zone`**，无法实测。

### 方案

1. **定义口径**：以「距当前配置临界温度的百分比」映射四档。
   - 读取每个 `thermal_zoneX/temp`（毫摄氏度）与 `trip_point_*_temp`（被动/临界点）
   - `temp < passive - 10°C` → `Nominal`
   - `passive - 10°C ≤ temp < passive` → `Fair`
   - `passive ≤ temp < critical` → `Serious`
   - `temp ≥ critical` → `Critical`
   - 多个 zone 取最严重档；无 trip_point 时记 debug 并 entire 返回 `Nominal`
2. **轮询**：在 `LinuxCommon` 里加 `thermal_state: ThermalState`，用
   `smol::Timer` 每 5 秒重读 sysfs，变化时经 `on_thermal_state_change` 回调派发
   （照搬电源事件的 `power_sender` + calloop channel 模式）。
3. **API 形状**：`thermal_state()` 直接读 `common.thermal_state`；回调在状态变化时触发。
4. **测试**：构造临时 sysfs 目录（注入到读取函数，避免依赖真实硬件），
   单测覆盖阈值边界与多 zone 取最严重档。
5. **验证环境**：找一台有真实 `thermal_zone` 的笔记本，跑压力测试观察降档。

### 涉及文件

- `crates/rgpui-linux/src/linux/platform.rs`（API + 状态字段）
- `crates/rgpui-linux/src/linux/system_info.rs` 或新建 `thermal.rs`（sysfs 读取）

---

## 2. XIM preedit 反馈样式（审计 #55）

### 现状

`x11/xim_handler.rs:134-145` 的 preedit 回调拿到 `feedbacks: Vec<xim::Feedback>`
后全部忽略，只回传纯文本字符串。下划线 / 高亮 / 选中样式不生效。

### 为什么没直接做

当前开发机 `XIM` 环境变量为空，没有 XIM 会话可测。XIM 的 feedback 属性
（`XIMUnderline=2`、`XIMHighlight=4`、`XIMReverse=1`、`XIMPrimary=8` 等）
需要与终端 / 输入框的绘制层接线，改错会影响所有中文输入。

### 方案

1. **解析 feedback**：把 `Vec<xim::Feedback>` 归因到 preedit 字符串的字节/字符区间，
   产出带样式的片段序列（`Vec<StyledSegment { text, style }>`），而不是单个 `String`。
2. **数据通道**：`XimCallbackEvent::XimPreeditEvent` 当前只带 `String`；
   扩展为带片段序列，经 `PlatformInput::ImePreedit` 传给用户视图。
3. **绘制层**：终端 / `Input` 在组合期间按片段画下划线 / 反色 / 高亮
   （参考 rgpui-term 已有的 `Terminal::ime_composition` 组合期下划线预览）。
4. **兼容**：无 feedback（普通 XIM 或 Wayland IME）时退化为整段普通样式。
5. **测试**：mock `xim::Feedback` 序列，单测片段切分与样式映射；
   真机用 ibus-x11 的 XIM 模式输入中文验证。

### 涉及文件

- `crates/rgpui-linux/src/linux/x11/xim_handler.rs`
- `crates/rgpui/src/platform.rs`（`PlatformInput::ImePreedit` 形状，如需扩展）
- `crates/rgpui-term/src/`（终端组合期绘制）

---

## 3. Wayland 多 seat（审计 #56）

### 现状

`wayland/client.rs:236` 注释 `// TODO: Multi seat support`；建窗时取第一个
`wl_seat`（`:576-582`），`Globals` 只持有一个 `wl_seat`（`:139`）。
第二套鼠标 / 触控设备的输入被忽略。

### 为什么没直接做

当前开发机 `WAYLAND_DISPLAY` 为空（纯 X11 + xrdp），没有 Wayland 会话更没有多 seat。
改造涉及全局状态重构，盲改风险高。

### 方案

1. **状态改造**：`Globals.seat` 改为 `seats: HashMap<ObjectId, WlSeat>`，
   `WaylandClientState` 的 `wl_pointer` / `wl_keyboard` 改为按 seat 索引的 map。
2. **事件分派**：所有 `wl_pointer` / `wl_keyboard` 事件的回调里取 `seat` 参数，
   按 seat 分别维护焦点、 modifier、点击计数等状态（当前这些是全局单份）。
3. **data_device**：每个 seat 单独 `get_data_device` / `get_primary_selection`，
   剪贴板按 seat 区分（否则跨设备粘贴错乱）。
4. **光标**：每个指针设备单独 cursor surface（当前一个 `Cursor` 实例）。
5. **窗口焦点**：`keyboard_focused_window` 改为按 seat 记录；窗口提升仍只需一个。
6. **测试**：wayland 合成器（weston / sway）下用 `WAYLAND_DEBUG=1` 抓事件，
   接两个虚拟指针设备验证分别派发。

### 涉及文件

- `crates/rgpui-linux/src/linux/wayland/client.rs`（主要改造）
- `crates/rgpui-linux/src/linux/wayland/window.rs`（焦点 / 光标按 seat）

---

## 4. macOS `start_window_move` 重入（审计 M6）

### 现状与判断

`macos/window.rs` 的 `start_window_move()` 同步调用
`NSWindow.performWindowDragWithEvent:` 进入 AppKit 模态循环。
审计一度担心：模态循环期间派发的事件会重入 `Rc<RefCell<App>>` 借用而 panic。

**经复核，这是误报**：`performWindowDragWithEvent:` 是 AppKit 专门设计给在鼠标事件
处理中同步调用的标准 API（Zed 上游亦如此）。事件回调在该调用前已 `take()` 出
`event_callback`，模态期间的嵌套事件不会重入同一借用；其它同步回调
（如 `windowShouldClose:`）已在分支中独立做了重入防护。

### 结论

**暂不处理**。推迟到下一 run-loop 会让 `currentEvent` 失效，反而破坏拖拽。
若未来真机复现 borrowed 冲突，应先用 `RUST_LOG` + backtrace 定位具体是哪条
回调重入，再针对性加防护，而不是整体推迟 drag。

---

## 通用原则

这四项共同说明一件事：**平台能力缺口要先有验证环境，再写代码**。
本分支已修的 60+ 项都是在 Linux 本机可编译、可运行验证的；
上述四项缺少对应硬件/会话，强行实现等于盲改。建议按上面的方案在对应环境接手。
