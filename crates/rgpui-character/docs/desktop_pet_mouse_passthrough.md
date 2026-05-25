# 桌宠鼠标穿透问题记录与解决方案

本文记录 `rgpui-character` 桌宠示例在 Windows 上修复鼠标穿透问题的完整背景、踩坑点和最终方案。

相关代码：

- `crates/rgpui-character/examples/desktop_pet.rs`
- `crates/rgpui-windows/src/window.rs`
- `crates/rgpui-windows/src/events.rs`

## 问题现象

桌宠窗口启动后，即使示例中已经配置：

```rust
WindowOptions {
    window_background: WindowBackgroundAppearance::Transparent,
    mouse_passthrough: true,
    kind: WindowKind::Overlay,
    ..
}
```

并且运行时也调用：

```rust
window.set_mouse_passthrough(true);
```

实际表现仍然是：

- 鼠标点击桌宠所在区域时，底层窗口无法收到点击。
- 鼠标滚轮、拖拽、点击会被桌宠覆盖窗口挡住。
- `WM_NCHITTEST` 中返回 `HTTRANSPARENT` 后，部分情况下仍然不能穿透到其他应用。
- 透明背景与鼠标穿透容易被混淆：窗口看起来透明，不代表它对鼠标也是透明。

桌宠类窗口通常是一个无边框、置顶、透明覆盖窗口。它看起来只是显示一只角色，但在 Windows 窗口系统里，它仍然是一个真实 HWND。如果没有正确设置原生窗口样式，这个 HWND 会拦截它矩形区域内的鼠标输入。

## 期望行为

桌宠有两种运行状态：

1. 自动运行状态

   - 只显示角色精灵。
   - 鼠标事件应穿过桌宠窗口，落到底层应用。
   - 用户可以正常点击桌面、浏览器、编辑器等被桌宠覆盖的区域。

2. 交互状态

   - 用户按 `Ctrl` 或暂停后显示控制面板。
   - 鼠标穿透关闭。
   - 控制面板按钮可以点击。
   - 窗口可以被激活或拖动。

也就是说，鼠标穿透必须能在运行时动态切换：

```text
自动运行 -> mouse_passthrough=true  -> 穿透到底层窗口
交互面板 -> mouse_passthrough=false -> 桌宠窗口接收鼠标事件
```

## 之前方案为什么不够

最初实现主要依赖两层逻辑：

1. `WindowOptions::mouse_passthrough`

   rgpui 层记录窗口是否启用鼠标穿透。

2. `WM_NCHITTEST -> HTTRANSPARENT`

   Windows 平台消息处理里，在穿透模式下返回：

   ```rust
   Some(HTTRANSPARENT as _)
   ```

这个思路看起来合理，但在桌宠场景里不够可靠。

### `HTTRANSPARENT` 的限制

`HTTRANSPARENT` 是 `WM_NCHITTEST` 的返回值，它告诉 Windows 当前窗口的命中测试结果是透明的。它可以帮助 Windows 继续寻找其他可命中的窗口。

但是实践中它不能单独作为跨进程桌面覆盖窗口的可靠鼠标穿透方案。尤其是桌宠这种窗口：

- 是顶层窗口。
- 是置顶窗口。
- 是透明覆盖窗口。
- 底层目标窗口通常属于其他进程。
- 渲染路径可能使用 DirectComposition。

在这些条件下，仅依赖 `WM_NCHITTEST` 返回 `HTTRANSPARENT`，经常出现“命中测试像是透明了，但实际鼠标输入仍然没有落到底层应用”的情况。

### `WS_EX_TRANSPARENT` 单独也不够稳

后来尝试在穿透时同步 `WS_EX_TRANSPARENT`。这个扩展样式会影响 Windows 对窗口的绘制和命中顺序，让窗口在某些场景下跳过鼠标输入。

但对透明覆盖窗口来说，单独设置 `WS_EX_TRANSPARENT` 仍然可能不够。Windows 上大量“点击穿透透明窗口”的实现都要求组合使用：

```text
WS_EX_LAYERED | WS_EX_TRANSPARENT
```

只加 `WS_EX_TRANSPARENT`，窗口仍可能作为普通顶层窗口参与输入命中。

### 透明背景不等于鼠标穿透

`WindowBackgroundAppearance::Transparent` 只控制窗口视觉背景透明。它解决的是“看得见/看不见”的问题，不解决“点不点得到后面窗口”的问题。

这两个概念必须分开：

```text
视觉透明：背景不绘制、不遮住画面
输入透明：鼠标事件不被当前窗口吃掉
```

桌宠需要同时满足这两点。

## 最终解决方案

Windows 平台层在 `mouse_passthrough=true` 时同步设置：

```text
WS_EX_LAYERED | WS_EX_TRANSPARENT
```

并调用：

```rust
SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA)
```

这样做有三个目的：

1. `WS_EX_LAYERED`

   让窗口成为 layered window。透明覆盖窗口要可靠参与这种输入穿透行为，通常需要这个样式作为基础。

2. `WS_EX_TRANSPARENT`

   让 Windows 在鼠标命中和绘制顺序上跳过该窗口，使鼠标事件落到后面的窗口。

3. `SetLayeredWindowAttributes(..., 255, LWA_ALPHA)`

   激活 layered window 的 alpha 属性，同时保持整体不降低透明度。这里的 `255` 表示窗口整体 alpha 为完全不透明，实际的像素级透明仍由渲染内容决定。

最终平台层逻辑变成：

```rust
if passthrough {
    new_ex_style |= WS_EX_LAYERED.0 as isize | WS_EX_TRANSPARENT.0 as isize;
} else {
    new_ex_style &= !WS_EX_TRANSPARENT.0 as isize;
}
```

关闭穿透时只移除 `WS_EX_TRANSPARENT`，不主动移除 `WS_EX_LAYERED`。

原因是：在某些回退渲染路径里，`WS_EX_LAYERED` 还承担透明窗口渲染的职责。交互模式关闭鼠标穿透时，如果同时移除 `WS_EX_LAYERED`，可能导致透明窗口渲染出现新问题。

## 修改点说明

### 创建窗口时同步原生样式

文件：`crates/rgpui-windows/src/window.rs`

创建窗口时，如果 `params.mouse_passthrough == true`，扩展样式直接包含：

```rust
WS_EX_LAYERED | WS_EX_TRANSPARENT
```

这样可以保证桌宠刚启动时就处于真实穿透状态。

仅在运行时调用 `set_mouse_passthrough(true)` 不够，因为初始窗口创建阶段可能已经接收过鼠标命中或激活行为。创建时先把样式设对，启动后的状态更稳定。

### 运行时切换时同步原生样式

文件：`crates/rgpui-windows/src/window.rs`

新增内部辅助方法：

```rust
fn sync_mouse_passthrough_style(&self, passthrough: bool)
```

它负责：

- 读取当前 `GWL_EXSTYLE`。
- 根据 `passthrough` 添加或移除 `WS_EX_TRANSPARENT`。
- 穿透时确保 `WS_EX_LAYERED` 存在。
- 穿透时调用 `SetLayeredWindowAttributes`。
- 样式变化后调用 `SetWindowPos(..., SWP_FRAMECHANGED | SWP_NOACTIVATE)` 刷新窗口样式。

`PlatformWindow::set_mouse_passthrough` 现在会同时更新内部状态和 Windows 原生样式：

```rust
fn set_mouse_passthrough(&self, passthrough: bool) {
    self.0.state.mouse_passthrough.set(passthrough);
    self.0.sync_mouse_passthrough_style(passthrough);
}
```

### 保留 `WM_NCHITTEST` 逻辑

文件：`crates/rgpui-windows/src/events.rs`

`WM_NCHITTEST` 中仍然保留 `mouse_passthrough` 判断：

```rust
if self.state.mouse_passthrough.get() {
    return Some(HTTRANSPARENT as _);
}
```

它现在不是唯一的穿透机制，而是和原生扩展样式一起工作：

```text
WS_EX_LAYERED | WS_EX_TRANSPARENT -> 负责可靠跨进程输入穿透
HTTRANSPARENT                    -> 负责命中测试层面的补充语义
```

保留它的好处：

- 对同线程窗口或特殊命中测试场景仍有帮助。
- 与 Ctrl 拖动逻辑配合，允许穿透模式下临时返回 `HTCAPTION`。
- 让平台内部的鼠标命中语义保持一致。

## 桌宠示例侧的状态切换

文件：`crates/rgpui-character/examples/desktop_pet.rs`

示例中 `PetSharedState` 用 `passthrough` 表示当前是否允许鼠标穿透。

自动运行时：

```rust
self.running = true;
self.interactive = false;
self.passthrough = true;
```

进入交互模式时：

```rust
self.running = false;
self.passthrough = false;
self.interactive = true;
```

主循环里每帧同步到真实窗口：

```rust
window.set_mouse_passthrough(should_passthrough);
```

这意味着桌宠状态和 Windows 原生窗口样式之间的链路是：

```text
PetSharedState::passthrough
        ↓
window.set_mouse_passthrough(...)
        ↓
PlatformWindow::set_mouse_passthrough
        ↓
WindowsWindowInner::sync_mouse_passthrough_style
        ↓
GWL_EXSTYLE: WS_EX_LAYERED | WS_EX_TRANSPARENT
```

如果以后又出现穿透问题，排查时应沿这条链路逐层确认。

## Ctrl 拖动为什么还能工作

桌宠自动运行时鼠标是穿透的，正常来说用户无法直接拖动它。

示例通过 Windows API 检测 Ctrl 键：

```rust
GetAsyncKeyState(VK_CONTROL)
```

当 Ctrl 按下时，`WM_NCHITTEST` 会优先返回：

```rust
HTCAPTION
```

这样 Windows 会把鼠标操作当作拖动标题栏处理，从而允许拖动窗口。

注意：如果 `WS_EX_TRANSPARENT` 已经让系统完全跳过该窗口，某些机器上 Ctrl 拖动可能仍受窗口命中顺序影响。当前示例的主要交互入口是按 Ctrl 切换交互模式，关闭穿透后再拖动或点击面板，这比在穿透状态下强行拖动更可靠。

## 测试背景导致的误判

排查过程中，示例里曾临时加过：

```rust
.bg(rgb(0x333333))
```

这会让透明覆盖窗口变成一整块灰色方形区域。它虽然有助于确认窗口位置和尺寸，但会造成两个误判：

1. 看起来像桌宠仍然遮挡了一大块屏幕。
2. 容易把视觉遮挡和鼠标输入遮挡混在一起。

最终应删除这类测试背景。桌宠正常渲染时，根容器不应该设置实色背景，只绘制角色精灵和交互面板。

## 验证方法

建议按下面步骤验证鼠标穿透是否真的修好。

### 1. 启动示例

```bash
cargo run -p rgpui-character --example desktop_pet
```

### 2. 自动运行状态验证

桌宠启动后处于自动运行状态。把桌宠移动到浏览器、编辑器、文件管理器等窗口上方。

验证点：

- 点击桌宠身体所在区域，底层窗口应该收到点击。
- 在桌宠覆盖区域滚轮滚动，底层窗口应该滚动。
- 拖拽选择底层文本时，鼠标经过桌宠窗口区域不应被中断。
- 桌宠不应该抢焦点。

### 3. 交互状态验证

按 `Ctrl` 进入交互模式，或按 `Ctrl+Shift+P` 暂停。

验证点：

- 控制面板出现。
- 面板按钮可以点击。
- 鼠标事件不再穿透。
- 点击“散步”或再次切换回自动状态后，鼠标穿透恢复。

### 4. 托盘和隐藏恢复验证

关闭窗口时示例会隐藏到托盘，不会退出进程。

验证点：

- 托盘“显示窗口”恢复后，穿透状态仍符合当前桌宠状态。
- 自动运行时恢复后仍穿透。
- 暂停或交互状态恢复后面板仍可点击。

## 常见回归点

以后如果鼠标穿透再次失效，优先检查这些点。

### 是否只改了 rgpui 内部状态

只设置：

```rust
self.0.state.mouse_passthrough.set(true);
```

是不够的。必须同步 Windows 原生扩展样式。

### 是否只返回了 `HTTRANSPARENT`

只在 `WM_NCHITTEST` 返回 `HTTRANSPARENT` 不够稳定。它不能替代：

```text
WS_EX_LAYERED | WS_EX_TRANSPARENT
```

### 是否只设置了 `WS_EX_TRANSPARENT`

透明覆盖窗口建议同时设置：

```text
WS_EX_LAYERED | WS_EX_TRANSPARENT
```

### 是否在关闭穿透时移除了 `WS_EX_LAYERED`

不要轻易移除 `WS_EX_LAYERED`。它可能影响透明窗口渲染。关闭穿透时通常只移除：

```text
WS_EX_TRANSPARENT
```

### 是否忘记刷新窗口样式

修改 `GWL_EXSTYLE` 后需要调用 `SetWindowPos` 刷新：

```rust
SetWindowPos(
    hwnd,
    None,
    0,
    0,
    0,
    0,
    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE,
)
```

否则样式变化可能不会立即生效。

### 是否把测试背景留在桌宠根节点

根节点不要保留类似：

```rust
.bg(rgb(0x333333))
```

它会让透明窗口看起来像一整块遮罩，干扰判断。

## 推荐原则

桌宠透明覆盖窗口的鼠标穿透不要只靠 UI 框架层状态，也不要只靠命中测试返回值。

Windows 上推荐把它当作平台能力处理：

```text
应用状态决定是否穿透
        ↓
rgpui Window API 同步状态
        ↓
Windows 平台层同步 HWND 扩展样式
        ↓
WS_EX_LAYERED | WS_EX_TRANSPARENT 生效
```

这次修复后的关键认知是：

```text
透明渲染和鼠标穿透是两个问题。
HTTRANSPARENT 和 WS_EX_TRANSPARENT 也不是同一个层面的机制。
桌宠这类跨进程透明置顶窗口，需要 Windows 原生扩展样式兜底。
```
