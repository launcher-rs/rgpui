# Linux 平台问题审计清单

> 分支：`fix/linux-platform-issues`
> 首次审计：2026-10-06 最近更新：2026-10-07
> 审计环境：Hyper-V 虚拟机 + Ubuntu 22.04 + GNOME + **xrdp 远程桌面会话**（`Xorg :10 -config xrdp/xorg.conf`）
> 对照基准：`Platform` / `PlatformWindow` trait（`crates/rgpui/src/platform.rs`）与 `rgpui-linux` 的实现差异

---

## 一、摘要

**渲染已经正常，tray 已经可用，Inspector 已经可用。** 剩下的是一批「返回成功但什么都没做」
的假实现和静默 no-op 的 API。

已修复并提交：

| 提交 | 内容 |
|------|------|
| `fcf70ad9f4` | `fix(linux)`：画面完全不上屏（present 缺失 + ARGB visual 误用 + feature 未开） |
| `0cd71c3ddf` | `feat(linux)`：系统托盘（StatusNotifierItem + dbusmenu） |
| `fef831e901` | `docs(linux)`：本审计文档 |
| `ac83e85218` | `fix(linux)`：关闭主窗口后托盘「显示窗口」无响应 |
| `35f4d730ed` | `chore`：忽略 `.qoder` 本地配置目录 |
| `4f782cb1fa` | `feat(linux)`：通知走门户实现 + `get_title` + X11 urgency 提醒 |
| `0fc55633cc` | `docs(linux)`：更正误诊并补记修复 |
| `3fcaa73c98` | `feat(rgpui)`：`rgpui::init_logging()` 日志初始化入口 |
| `d6f6a4c598` | `feat(linux)`：`os_info` 与 `system_idle_time` |
| `d5e4819376` | `fix(linux)`：全局热键真正 GrabKey + `on_global_hotkey` 派发 |
| `7fa8b7bf47` | `fix(linux)`：权限查询改真实现（AT-SPI 两步探测 + portal 接口盘点） |
| `ae8da6ba4c` / `bf8bb36c0a` | `docs(linux)`：同步状态、补 Linux 构建前置与权限判定口径 |
| `4bb1e1b3a2` | `fix(core)`：`keep_alive_without_windows` 状态收回核心层 |
| `651a94a618` | `feat(linux)`：`network_status`（门户优先 + `/sys/class/net` 兜底） |
| `abafc85668` | `docs(linux)`：§4.7 与 keep-alive 修复记录，更正 §4.3 调用方结论 |
| `e00ddd95aa` | `feat(core)`：权限/系统信息/自启动等能力接回 `App`，权限收敛为统一入口 |
| `79e93b058d` | `docs(linux)`：§4.8 记录 + AGENTS.md 口径 |
| `223061c8bc` | `feat(linux)`：X11 鼠标穿透 / 输入区域（X Shape）+ Wayland `set_mouse_passthrough` |

最需要记住的一句话（未变）：

> **Linux 是唯一被 feature 门控「默认关死」的平台。** Windows 的 `rgpui-windows` 后端无条件编译，
> 所以同样的依赖声明在 Windows 正常、在 Linux 静默退化成 Headless —— 这正是「Windows 基本正常、
> Linux 很多不正常」的根本原因。

第二句要记住的（**本次修正**）：

> ~~xrdp 会话下永远看不到画面，因为没有 DRI3，Mesa 的 Vulkan 呈现强制依赖 DRI3。~~
> **这个结论是误诊，见 §3.3。** 真相是 rgpui 自己的 wgpu 呈现路径漏了显式 `present()`，
> 加上 X11 无条件选 32 位 ARGB visual 导致合成器把整个窗口当透明。
> **xrdp 会话（无 DRI3）下画面完全正常**，本机的渲染验证不需要换 Wayland。

| 级别 | 问题 | 状态 |
|------|------|------|
| P0 | Linux 窗口后端 feature 未启用 → 无窗口无报错 | **已修复并验证**（`fcf70ad9f4`，见 §2.1） |
| P0 | 软件 Vulkan 驱动被 wgpu 判为「非一致性」而隐藏 → 无 GPU 适配器 | **已修复并验证**（`fcf70ad9f4`，见 §2.2） |
| P0 | 窗口能打开但画面永远不上屏 | **已修复并验证**（`fcf70ad9f4`，见 §3；~~环境限制~~ 为误诊） |
| P1 | tray 8 个 API 在 Linux 完全未实现 | **已实现并验证**（`0cd71c3ddf`，见 §4.1） |
| P1 | 关闭主窗口后托盘「显示窗口」无反应 | **已修复并验证**（`ac83e85218`，见 §2.3） |
| P1 | Inspector（F12）在 Linux 是否可用 | **已验证可用**（见 §2.4） |
| P2 | 窗口启动后 ~130 ms 纯黑，然后才出画面 | **已定位，未修**（见 §2.5） |
| P1 | GL 后端在老 Mesa 上不可用（wgpu-hal 只认 `EGL_EXT_platform_xcb`） | 已定位，上游兼容问题（见 §3.4） |
| P1 | 通知 / 全局热键 / 权限 / 应用菜单是「假实现」——返回成功但什么都没做 | **通知、全局热键、权限查询已改真实现并验证**（`4f782cb1fa`、`d5e4819376`、`7fa8b7bf47`，见 §4.2）；仅剩应用菜单 |
| P1 | 约 24 个 `Platform` 方法在 Linux 上静默 no-op | **已实现 `os_info` / `system_idle_time`（`d6f6a4c598`）、权限查询（`7fa8b7bf47`）、`network_status`（`651a94a618`，见 §4.7）**；剩约 19 个**大多是「应用层调不到」的死接口**，分诊见 §4.3 + §4.8 |
| P1 | X11/Wayland 窗口缺失 `request_attention`、`get_title` 等方法 | **`get_title` / `request_attention` 已实现并验证**（`4f782cb1fa`，见 §4.4）；`set_mouse_passthrough` + X11 `set_input_region` **已实现**（`223061c8bc`，见 §4.4）；其余（`map_window`、`render_to_image`、exclusive zone）待实现 |
| P1 | `WindowOptions.mouse_passthrough` 在 X11 被完全忽略 —— 桌面宠物类窗口只能靠 Wayland | **已修复**（`223061c8bc`）：X Shape 空输入区域，`ShapeGetRectangles` 回读 `INPUT[]`（0 rect）实测；Wayland 侧补 `set_mouse_passthrough`（仅编译验证） |
| P1 | **`App` 完全没有包装平台能力方法** → 已实现的 `os_info` / 权限判定等应用层根本调不到 | **已修复并验证**（`e00ddd95aa`，见 §4.8）：权限收敛为 `check_permission`/`request_permission`，8 个能力接回 `App` |
| P1 | `set_keep_alive_without_windows` 全链路 write-only（含 Windows） | **已修复并 A/B 验证**（`4bb1e1b3a2`）：状态收回核心层，平台侧方法删除（见 §4.1 末） |
| P2 | `cargo check --workspace` 被 webview 示例阻塞（缺 glib/gtk/webkit 系统库） | 待处理（见 §4.5） |
| P2 | rgpui 无日志初始化入口，wgpu/GPU 诊断信息全部丢失 | **已实现**（`rgpui::init_logging()`，`3fcaa73c98`，见 §4.6） |
| P3 | Inspector 面板显示「帧率 0.0 FPS · 0.0 ms」 | 待查（见 §2.4） |
| P3 | X11 窗口没有 `WM_NAME`，`wmctrl -l` 显示 `N/A` | **非平台缺陷**：`set_title` 一直会写 `WM_NAME`/`_NET_WM_NAME`，是示例没传标题（见 §2.4） |
| — | 仓库路径含 `C:` 导致 cargo 构建失败；RDP 共享盘 I/O 极慢 | 环境问题 |

---

## 二、已修复并验证

### 2.1 [P0] Linux 窗口后端根本没有被编译进来

**症状**：`cargo run -p hello_world` 退出码 0，无窗口、无输出、无任何报错。
`tray_simple` 打印一行 `Tray should be visible now.` 后一直挂着。

**根因链**（每一环都已核实）：

1. 根 `Cargo.toml` 把三个依赖都声明成 `default-features = false`：
   - `:32` `rgpui = { ..., default-features = false }`
   - `:34` `rgpui-linux = { ..., default-features = false }`
   - `:38` `rgpui-platform = { ..., default-features = false }`
2. 50 个示例都写 `rgpui-platform.workspace = true`，继承 `default-features = false`。
3. `crates/rgpui-platform/Cargo.toml:17` 的 `default = []` —— 即使不继承也不会开任何东西；
   而 `wayland = ["rgpui-linux/wayland"]`、`x11 = ["rgpui-linux/x11"]` 是**纯 opt-in，没有任何人启用**。
4. 结果：`cargo tree -p hello_world -f "{p} [{f}]"` 打印
   `rgpui-linux v1.4.0 []` —— **feature 列表为空**。
5. `rgpui-linux/src/linux.rs` 里 `mod wayland` / `mod x11` 都在 `#[cfg(feature = ...)]` 后面，
   **X11 与 Wayland 两个后端模块完全不参与编译**。
6. 同时 `rgpui` 没有 `wayland`/`x11` feature → `crates/rgpui/src/platform.rs:112-120`
   读取 `DISPLAY` / `WAYLAND_DISPLAY` 的代码被 cfg 掉 → `guess_compositor()`
   **无视 `DISPLAY=:10.0` 恒返回 `"Headless"`**。
7. `crates/rgpui-linux/src/linux.rs:70` 走 `"Headless"` 分支 → 返回 `HeadlessClient`
   + `crates/rgpui-linux/src/linux/platform.rs:146` 的 `NoopTextSystem`。

**验证**：

```
$ cargo tree -p hello_world -f "{p} [{f}]" | grep rgpui-linux
    └── rgpui-linux v1.4.0 [...]        # 修复前 []
    └── rgpui-linux v1.4.0 [wayland, x11, ...]   # 修复后
$ ./hello_world   # 修复前：退出码 0，什么都不发生
                   # 修复后：500x500 窗口 Map State = IsViewable
```

**修复**（`crates/rgpui-platform/Cargo.toml`，中央修复，50 个示例零改动）：

```toml
[target.'cfg(any(target_os = "linux", target_os = "freebsd"))'.dependencies]
rgpui-linux = { workspace = true, features = ["wayland", "x11"] }
rgpui = { workspace = true, features = ["wayland", "x11"] }
```

放在这里的原因：
- 该依赖本身就是 `cfg(target_os = "linux"/"freebsd")` 门控的，**不会影响 Windows/macOS/wasm**；
- `rgpui::guess_compositor()` 全仓库只有 `rgpui-linux` 一个调用者，开启 `wayland`/`x11`
  在其它平台不产生副作用；
- 同文件 `:34` 的 Windows 段早已有 `rgpui = { workspace = true, features = [...] }`
  跨 section 重复声明的先例，Cargo 允许这种写法。

> 顺带：修复后 `cargo build` 不再出现 `warning: field 'wake_sender' is never read` 与
> `warning: glob import doesn't reexport anything` 两条警告 —— 它们正是 feature 关闭时
> 才会暴露的死代码，是这个 bug 的旁证。

### 2.2 [P0] 软件 Vulkan 驱动被隐藏 → 枚举不到任何可用 GPU

**症状**：启用 feature 后窗口仍起不来：

```
Found 1 GPU adapter(s):
  - llvmpipe (LLVM 13.0.1, 256 bits) (backend=Gl, type=Cpu)
  Adapter llvmpipe failed: no compatible surface formats, trying next...
thread 'main' panicked: No GPU adapter found that can configure the display surface
```

**根因**（`wgpu-hal-30.0.1/src/vulkan/adapter.rs:2310-2324`）：

```rust
if driver.conformance_version.major == 0 {          // Mesa 22.0.1 的 lavapipe 上报 0
    if driver.driver_id == vk::DriverId::MOLTENVK { ... }
    else if flags.contains(InstanceFlags::ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER) { 放行 }
    else { log::debug!("Adapter is not Vulkan compliant, hiding adapter"); return None }
}
```

Debug 日志实锤：

```
DEBUG wgpu_hal::vulkan::adapter] Adapter is not Vulkan compliant, hiding adapter: llvmpipe (LLVM 13.0.1)
```

Vulkan 适配器**其实被找到了**，只是因为 Mesa 的 lavapipe 没有申报 Vulkan 一致性测试版本而被丢弃；
剩下的 GL 后端又因 §3.4 的 EGL 问题无法配置表面 → 最终 0 个适配器。

**修复**（`crates/rgpui-wgpu/src/wgpu_context.rs:226`）：

```rust
flags: wgpu::InstanceFlags::default()
    | wgpu::InstanceFlags::ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER,
```

**验证**：

```
Found 2 GPU adapter(s):
  - llvmpipe (backend=Vulkan, type=Cpu)     ← 新增
  - llvmpipe (backend=Gl,    type=Cpu)
Selected GPU (passed configuration test): llvmpipe (LLVM 13.0.1, 256 bits) (Vulkan)
WARNING: lavapipe is not a conformant vulkan implementation, testing use only.
Refreshing every 20ms                       ← 渲染循环启动
```

> ⚠️ 该 flag 的语义是「允许使用未通过 Vulkan 一致性测试的驱动」。对 VM/CI/无独显机器上的
> 软件渲染是必要开关；若担心掩盖真实驱动问题，可考虑仅在枚举不到任何其它适配器时再带此 flag
> 重试一次。

### 2.3 [P1] 关闭主窗口后，托盘「显示窗口」无反应

**症状**（用户实测）：主窗口**最小化**后托盘菜单点「显示窗口」能恢复；主窗口**关闭**后点同一项
**毫无反应**。

**根因**：不是 tray 的问题，是 X11/EWMH 语义差异。

| 路径 | 调用 | WM 眼中的状态 | 后续 `_NET_ACTIVE_WINDOW` |
|------|------|--------------|--------------------------|
| 最小化 | `X11Window::minimize()`（`x11/window.rs:1654`）发 `WM_CHANGE_STATE` + `WINDOW_ICONIC_STATE` | **Iconic**，仍在 WM 管理内 | ✅ 生效 |
| 关闭/隐藏 | `X11Window::hide()`（`x11/window.rs:1675`）就是 `unmap_window` | **Withdrawn**，mutter **停止管理该窗口** | ❌ 被直接忽略 |

ICCCM 语义：对受管顶层窗口 `UnmapWindow` → `WM_STATE = Withdrawn`，WM 放弃管理。
`activate()` 原本只发 `_NET_ACTIVE_WINDOW` 客户端消息 + `set_input_focus`，
对一个 Withdrawn 窗口两者都是空转。

**修复**（`crates/rgpui-linux/src/linux/x11/window.rs`，`activate()` 开头）：

```rust
// hide() 撤下的窗口 WM_STATE 会变成 Withdrawn，合成器不再管理它，
// 单发 _NET_ACTIVE_WINDOW 会被忽略；先重新 map 才能恢复。窗口已可见时
// MapWindow 是空操作。
check_reply(
    || "X11 MapWindow on activate failed.",
    self.0.xcb.map_window(self.0.x_window),
)
.log_err();
```

对已可见窗口 `MapWindow` 是空操作，所以原路径无回归。**Wayland 无此问题**：
`wayland/window.rs:1582` 的 `hide()` 是 `toplevel.set_minimized()`，永远是 Iconic 而非 Withdrawn。

**验证**：

```
# 关闭 → 恢复
$ wmctrl -i -c 0x2600001
  Map State: IsUnMapped      window state: Withdrawn      # 修复前卡在这里
$ gdbus call --session --dest <SNI名> --object-path /StatusNotifierItem/Menu \
    --method com.canonical.dbusmenu.Event <id> "clicked" '' 0   # 触发「显示窗口」
  Map State: IsViewable      window state: Normal         # 修复后
  → /tmp/traywin.png 内容正确渲染（"Hello from RGPUI Tray!"）

# 最小化 → 恢复（回归检查）
$ xdotool windowminimize 0x2600001     # window state: Iconic
  同样恢复为 IsViewable / Normal
```

### 2.4 [验证] Inspector 在 Linux 可用

`/tmp/rgpui-target/debug/inspector` 启动后 `xdotool key <WID> F12`，36.7% 像素发生变化，
面板**完整渲染**（截图 `/tmp/insp_after.png`）：

- 标题「元素检查器」+「拾取中」徽标，黑色横幅「拾取中，点击画布元素…」
- 「完整树 … N 个节点」
- 「运行」区：帧率 / CPU / 内存 / GPU —— GPU 一行如实报
  `llvmpipe (LLVM 13.0.1, 256 bits) (Vulkan)`，与 §2.2 的适配器选择日志一致
- 「报错 / 暂无上报错误」，以及示例内容（红/绿/蓝盒、嵌套目标、可拾取输入框、开关、计数按钮）

**结论：Inspector 在 Linux 可以正常用于排障。**

顺带发现两个独立缺陷（尚未排查，P3）：

1. 面板显示 **帧率 0.0 FPS · 0.0 ms** —— FPS 计数器在 Linux 上疑似没接上，
   而渲染循环日志明明在 `Refreshing every 20ms`。排障时会误导「是不是根本没在渲染」。
2. rgpui 的 X11 窗口在 X 树里是 `(has no name)`、`wmctrl -l` 显示 `N/A` ——
   没有设 `WM_NAME` / `_NET_WM_NAME`。这与 §4.4 的 `get_title` 缺失同源，
   并且让「按标题/按 pid 定位窗口」的脚本手段不好用。

### 2.5 [P2·未修] 窗口启动后有约 130 ms 纯黑

**症状**：窗口出现时是全黑的，随后才刷出内容。

**量化**（同一探针 `/tmp/blackprobe4.py`：轮询 `_NET_CLIENT_LIST` 找新窗口 → `xwininfo` 判
viewable → `xwd` 抓像素统计黑色占比；同一台 VM、同一 xrdp 会话）：

| 构建 | 进程启动 → 窗口可见 | 窗口可见 → 首帧内容（黑屏） |
|------|-------------------|--------------------------|
| debug | **1.491 s** | **0.250 s** |
| release | **0.202 s** | **0.131 s** |

```
=== RELEASE ===
t= 0.202s  窗口 0x2e00001 变为 IsViewable
t= 0.202s  black=100.0%  -> black
t= 0.333s  black=  0.0%  -> clean
黑屏时长 = 0.131s
```

**结论：这是两段性质完全不同的耗时，必须拆开看。**

**（a）「窗口迟迟不出现」≈ 纯性能问题，主因是 debug 构建。**
1.49 s → 0.20 s，**7.4 倍**差距。VM 无硬件 GPU，只有 llvmpipe/lavapipe 软件 Vulkan，
字体栅格化、shader 编译、首帧提交全在 CPU 上，所以软件渲染 + debug 双重放大这一段。
但它不是「黑屏」——窗口根本还没出现。

**（b）真正可见的纯黑 ≈ 结构性缺陷，与优化等级无关。**
release 把 250 ms 压到 131 ms 但**不归零**，说明它不是性能问题。两个原因叠加：

1. `crates/rgpui/src/window.rs:1905` —— `Window::new` 里直接
   `platform_window.map_window().unwrap()`，此时**一帧都还没渲染**；
2. `crates/rgpui-linux/src/linux/x11/window.rs:487-502` 的 `win_aux` 只设了
   `border_pixel` / `colormap` / `override_redirect` / `event_mask`，
   **没有设 `background_pixel`** → X server 用默认黑色填充暴露区域，
   合成器在这段窗口期拿到的就是纯黑。

**可选修法（尚未决定，用户当前指示「先不管」）**：

- **轻量**：`win_aux` 补 `background_pixel`（按 `WindowParams.window_background`，
  §2 修复后 visual 选择已依赖该字段）。只改 `rgpui-linux`，Windows/macOS/Wayland 零影响，
  把「黑闪」变成「主题底色一闪」。
- **彻底**：把 `map_window()` 推迟到首帧提交之后。动的是**全平台共用路径**，
  Windows/macOS/Wayland 的映射时机都会变，风险面大，应单独评估而非顺手改。

---

## 三、[已修复] 画面完全不上屏 —— 含一次误诊记录

保留本节是因为**误诊过程本身有价值**：证据链看起来很完整、结论是错的。

### 3.1 现象

完成 §2.1/§2.2 两个修复后，窗口正常创建、进入渲染循环，但**屏幕上是透明的、没有任何内容**
（用户原话：「测试是透明窗口，而且没有内容」）。

### 3.2 当时观察到的证据（真实，但被错误归因）

**（1）渲染管线看似正常。** 在 `WgpuRenderer::draw()` 里临时插桩（诊断代码已回滚）：

```
DEBUG-draw: 取帧成功，继续绘制
DEBUG-draw: 场景规模 quads=64 paths=0 shadows=2 underlines=0 mono_sprites=0
DEBUG-draw: 本帧已提交并释放 surface 纹理（触发呈现）
```

`get_current_texture()` 成功、场景非空、`queue.submit()` 完成、`SurfaceTexture` drop 完成，
**全程零 wgpu 错误**。

**（2）X 缓冲里什么都没有。** `xwd` 抓窗口按原始字节统计：

```
resize 前：非零像素 766 / 250000 (0.3%)，全在首两行，取值 01010101 / 02020202 …
           → 未初始化显存残渣，不是渲染结果
resize 后：非零像素 20398 / 270400，且 96.3% 的像素 alpha = 0
```

**（3）GPU 提交超时。**

```
WARN  rgpui_wgpu::wgpu_renderer] Failed to poll device during resize: Timeout
ERROR rgpui_wgpu::wgpu_renderer] GPU error during frame (failure 1 of 10): Validation Error
    In Surface::configure → Failed to wait for GPU to come idle before reconfiguring the Surface
```

**（4）换 Xephyr 复现，现象逐条一致** → 排除了 mutter/合成器/WM 因素。这一步是对的，
但它把嫌疑范围错误地收缩到了「X server 本身」。

### 3.3 当时的结论（**误诊**）与真相

**误诊结论**：xrdp Xorg 与 Xephyr 都没有 DRI3，而
`strings /usr/lib/x86_64-linux-gnu/libvulkan_lvp.so` 里有
`vulkan: No DRI3 support detected - required for presentation`，
于是判定「Mesa 的 Vulkan WSI 无法在无 DRI3 的 X server 上呈现 → 环境限制，改代码没用」。

**这条结论错在两点：**

1. **DRI3 缺失是真实事实，但它不是本机画面为空的原因。** 同一台机器、同一个 xrdp 会话
   （扩展表里依旧没有 DRI3），修完下面两处后画面**完全正常**。
2. 它把结论导向「环境问题、不可修」，从而**停止了对 rgpui 自身 present 路径的排查** ——
   而恰恰是这条路径上有两个真 bug。当时 `poll Timeout` 其实是「帧从未被 present、
   交换链图像一直被占用」的**结果**，被我当成了「present 走不通」的**证据**（因果倒置）。

**真正的根因（`fcf70ad9f4`，三处独立缺陷叠加）：**

1. **`rgpui-wgpu` 提交命令缓冲后直接 drop 交换链帧，从未显式 `present()`。**
   wgpu 30 起 `SurfaceTexture` 未显式 present 即 drop 会调用 `texture_discard`，**整帧作废**。
   Windows 走 `directx_renderer::present`、macOS 走 `drawable.present`，**只有 wgpu 路径漏了**。
   → 这一条解释 3.2(1)(2)(3) 全部现象：零错误、缓冲无内容、提交永远不空闲。
2. **X11 建窗时无条件选 32 位 ARGB visual。** 不透明窗口在该 visual 上交换链写入的 alpha 为 0
   → 合成器把整个窗口当透明。改为按 `WindowParams.window_background` 选 visual
   （新增字段，由 `WindowOptions` 传入；`WindowKind::Overlay` 仍用 ARGB）。
   → 这一条解释「除标题栏外全透明」的观感和 3.2(2) 的 96.3% alpha=0。
   注意：visual 建窗后不可更换，所以运行时切换背景外观只影响交换链 alpha 模式。
3. **feature 未开启**（即 §2.1）。

**教训（写在这里防止重犯）**：
「换 X server 现象一致」只能排除 WM，**不能证明是 X server 的锅** ——
如果 bug 在应用侧的 present 路径上，换任何 X server 都会一样。
`strings 驱动库 | grep` 找到一句匹配的诊断串只证明「驱动里有这段代码」，不证明「它被执行了」。

### 3.4 仍然成立的连带发现：GL 后端在本机完全不可用（降级路径失效）

这一条与 §3.3 的误诊无关，**仍然有效**：Vulkan 之外没有可用的降级后端。

```
Testing adapter: llvmpipe (…) (Gl)...
  Adapter (Gl) failed: no compatible surface formats, trying next...
```

根因在 `wgpu-hal-30.0.1/src/gles/egl.rs:825`：

```rust
(Some(Rdh::Xcb(xcb_display_handle)), Some(egl))
    if client_ext_str.contains("EGL_EXT_platform_xcb") => { /* XCB 平台 */ }
```

本机 Mesa 22（Ubuntu 22.04）公布的 EGL client extensions 里只有
**`EGL_MESA_platform_xcb`**，**没有** `EGL_EXT_platform_xcb`：

```
$ 只有: EGL_EXT_platform_base / EGL_EXT_platform_device / EGL_EXT_platform_x
        EGL_EXT_platform_wayland / EGL_MESA_platform_gbm / EGL_MESA_platform_surfaceless
        EGL_MESA_platform_xcb      ← 旧前缀
  没有: EGL_EXT_platform_xcb      ← wgpu-hal 只认这个
```

于是匹配落空 → 走 `EGL_MESA_platform_surfaceless` 平台 → `WindowKind::Unknown`
→ 无法为 X11 窗口创建 EGL window surface → surface format 列表为空 → GL adapter 被直接判死。

附带两个相关事实：

- 不带 `LIBGL_ALWAYS_SOFTWARE=1` 时，surfaceless 平台还会报
  `EGL 'eglInitialize' code 0x3001: DRI2: failed to load driver`；
  加上该变量后 EGL 初始化成功，但 surface format 问题依旧 → **GL 仍然不可用**。
- rgpui 侧的句柄是正确的（`XcbWindowHandle` + `XcbDisplayHandle`，
  见 `crates/rgpui-linux/src/linux/x11/window.rs:322-369`），**问题在 wgpu-hal 的扩展名匹配**
  —— 属于上游兼容性缺陷（对老 Mesa）。
  rgpui 侧可考虑的缓解：记录并暴露后端选择日志（与 §4.6 的日志入口是同一诉求）。

> ⚠️ 实际影响比原先评估的小：既然 Vulkan 路径在 §3.3 修好后正常工作，
> GL 不可用只意味着**少了一个降级备胎**，不再阻塞任何功能。优先级 P1 → P3。

### 3.5 本节待办

1. ~~在 Wayland 会话下复测 `hello_world`~~ —— 已无必要（X11 下画面已正常）。
   Wayland 复测仍有独立价值（验证 §2.3 的 Wayland 分支与 `map_window` 缺失），但不再是阻塞项。
2. 记录 §3.4 的 wgpu-hal / 老 Mesa 兼容问题，评估是否向上游提 issue（现为 P3）。

---

## 四、功能缺失与假实现

### 4.1 [已实现] tray —— StatusNotifierItem + dbusmenu

`crates/rgpui-linux/src/linux/tray.rs`（数据层，主线程）+
`tray_sni.rs`（DBus 服务，后台线程），提交 `0cd71c3ddf`。
**零新依赖**：zbus 5.19 / zvariant 5.15 经 `ashpd::zbus` 公开 re-export 传递而来；
`image` / `parking_lot` / `futures` / `smol` / `calloop` 都已在 `rgpui-linux` 无条件依赖里。

架构与线程约定：

```
主线程                                     后台线程（BackgroundExecutor + smol/zbus）
LinuxPlatform::set_tray*  ──mpsc 命令──▶  tray_sni::serve()
calloop EventSource ◀──calloop channel──  emit PropertiesChanged / LayoutUpdated
```

共享状态 `Arc<parking_lot::Mutex<SniShared>>`；**任何持锁区间都不跨 `.await`**
（`MutexGuard` 是 `!Send`，而 `BackgroundExecutor::spawn` 要求 `Future: Send`）。

8 个 `Platform` 方法的覆盖情况：

| 方法 | 状态 | 说明 |
|------|:----:|------|
| `set_tray` | ✅ | 首次调用惰性建共享状态并 spawn `serve()` |
| `set_tray_icon` | ✅ | 光栅化后 resize 出 **22 / 44** 两档，转 **ARGB32 大端 + 直通 alpha** |
| `set_tray_menu` | ✅ | dbusmenu 布局，支持分隔线 / 子菜单 / checkmark |
| `set_tray_tooltip` | ✅ | 协议已实现，但见下方 Ubuntu host 怪癖 5 |
| `set_tray_panel_mode` | ⚠️ | 映射为 `ItemIsMenu`，Ubuntu host 会忽略 |
| `get_tray_icon_bounds` | — | 恒 `None`（SNI 无坐标 API，**协议限制，不是没实现**） |
| `on_tray_icon_event` | ✅ | Activate / SecondaryActivate / ContextMenu / scroll |
| `on_tray_menu_action` | ✅ | dbusmenu `Event(id, "clicked", …)` 路由回主线程 |

**验证**（本机 GNOME/ubuntu-appindicators，协议级证据）：

```
# 1. 注册进了 watcher 列表
$ gdbus call --session --dest org.kde.StatusNotifierWatcher --object-path /StatusNotifierWatcher \
    --method org.kde.StatusNotifierWatcher.RegisteredStatusNotifierItems
  → 含本应用条目

# 2. gnome-shell 主动来拉菜单（不是我们自己发的调用）
$ dbus-monitor …   # 观察到 shell 侧发起 GetLayout / AboutToShow

# 3. 图标像素回读一致
IconPixmap a(iiay) → 解出 /tmp/sni_22.png 与 /tmp/sni_44.png，与原图相符

# 4. 点击路由
$ gdbus call … com.canonical.dbusmenu.GetLayout 0 -1 []     # 布局结构正确
$ gdbus call … com.canonical.dbusmenu.Event <id> "clicked" '' 0   # 触发回调
```

**实现期间踩到的 Ubuntu host 怪癖**（照此实现，否则图标不出现或事件丢失）：

1. `Id` 必须非空；2. `Status` 大小写敏感，`"Passive"` 会隐藏图标 → 默认 `"Active"`；
3. dbusmenu `toggle-type` 必须是 `"checkmark"`，且 `toggle-state` 是 **int**；
4. `GetLayout` 必须**忽略 `propertyNames` 过滤**、返回完整属性字典；
   子菜单靠 `children-display = "submenu"` 判定；
5. Ubuntu 的 ToolTip 渲染在扩展里是**注释掉的** —— 不要期待 tooltip 显示；
6. 左键打开菜单，只有**双击**左键才触发 `Activate`；右键永不产出 `LeftClick`；
7. `IconPixmap` 为 ARGB32 **大端 + 直通（非预乘）alpha**，且不要直发 512×512 原图；
8. 用对象路径注册 `RegisterStatusNotifierItem("/StatusNotifierItem")`，不需要 well-known name；
9. **没有** `UnregisterStatusNotifierItem` —— 清理方式就是 drop 掉 `Connection`；
10. watcher 不在时监听 `NameOwnerChanged` 重试注册。

**顺带修掉的真实缺陷**：托盘回调（如「退出」里的 `cx.quit()`）会
`RefCell already borrowed` panic —— calloop 回调持有客户端状态的 `borrow_mut()` 跨过了用户回调。
改成 `dispatch_tray_event`（先把回调 take 出来 → 释放借用 → 调用 → 再放回），
沿用既有 `handle_keyboard_layout_change`（`x11/client.rs:1528-1542`）的惯用法。

**本节遗留（独立缺陷，未随 tray 一起改）**——下列行号是「修复前」位置，已被 `4bb1e1b3a2` 删除或改写：

`set_keep_alive_without_windows`（`crates/rgpui/src/platform.rs:515`）**全链路 write-only**：
core 里没有读取方，`app.rs:2535` 只转发给平台，连 Windows 实现也只是
`AtomicBool::store`（`rgpui-windows/src/platform.rs:789-793`）而从不 load。
真正的「无窗口自动退出」在 `app.rs:1802-1810`：

```rust
QuitMode::Default => cfg!(not(target_os = "macos")),   // Linux = true
if quit_on_empty && cx.windows.is_empty() { cx.quit(); }
```

它只在**窗口关闭**时触发。所以 `tray_simple`（未开窗口）目前靠事件循环「碰巧」挡住不退出；
但任何「先开窗再全关掉以驻留托盘」的应用都会退出。这是**跨平台缺陷，不是 Linux 特有**。

> **已修复**（`4bb1e1b3a2`）：把状态收回核心层，不再让平台存一个没人读的标志。
> `App` 增加 `keep_alive_without_windows: Cell<bool>`（setter 是 `&self`，用 `Cell` 够用），
> 自动退出判据直接读它；`Platform::set_keep_alive_without_windows` 与 Windows 侧那个
> 只 `store` 从不 `load` 的 `AtomicBool` 一并删除 —— 跨平台缺陷一次改到位，
> 不是「只动 Linux 然后照样无效」。
>
> ```rust
> // 「没有窗口也要活着」优先于任何自动退出模式
> let quit_on_empty = !cx.keep_alive_without_windows.get()
>     && match cx.quit_mode {
>         QuitMode::Explicit => false,
>         QuitMode::LastWindowClosed => true,
>         QuitMode::Default => cfg!(not(target_os = "macos")),
>     };
> ```
>
> 验证（本机 X11 会话，走托盘菜单开窗口，见 §六 的 dbusmenu 驱动法）：
>
> - `daemon_app`（已 `set_keep_alive_without_windows(true)`）→ 菜单 `Settings` 开窗
>   → `wmctrl -i -c` 关掉最后一个窗口 → **进程存活**，`xdotool search --pid` 返回 0 个窗口；
> - 对照组 `hello_world`（未设该标志）关掉窗口 → 进程退出（说明不是一律不退出）；
> - 菜单 `Quit` 仍能退出（显式 `cx.quit()` 路径不受这条判据影响）。


### 4.2 [P1] 假实现 —— 返回成功，但什么都没做

这类比「没实现」更危险：调用方拿到 `Ok(())` 以为成功了。

**1) 系统通知** — `crates/rgpui-linux/src/linux/notifications.rs:25-31`

```rust
pub fn show_notification(&self, title: &str, body: &str, _icon: Option<&str>) -> Result<()> {
    log::info!("发送通知: {} - {}", title, body);
    Ok(())          // 只写日志，未接 notify-rust / XDG portal
}
```

> **已修复**（`4f782cb1fa`）：改走 XDG 门户 `org.freedesktop.portal.Notification`
> （复用已在依赖里的 `ashpd`，未新增 crate），图标名经 `Icon::with_names` 传入，
> `pollster::block_on` 在主线程安全（zbus 的 async-io 后端自带执行器线程）。
> 无头构建（既非 x11 也非 wayland）直接 `bail!`，不再假装成功。
> 验证：本机 portal 实测收到通知；`RUST_LOG=debug` 可见调用过程（见 §4.6）。

**2) 全局热键** — `crates/rgpui-linux/src/linux/global_hotkey.rs:31-45`

```rust
pub fn register(&mut self, id: i32, keystroke: &Keystroke) -> Result<()> {
    // 这里简化实现，实际需要根据显示服务器选择后端
    self.registrations.insert(id, keystroke.clone());
    Ok(())
}
```

只存进 `HashMap`，**从未向显示服务器注册**；而 `on_global_hotkey`
（`crates/rgpui/src/platform.rs:512`）在 Linux 上根本没覆盖 → **注册返回 Ok，按键永远无反应**。
`platform.rs:716-724` 的 `register_global_hotkey` / `unregister_global_hotkey` 因此是空转。

> **已修复**（`d5e4819376`）：该假实现文件删除，改由 `LinuxClient` trait 承担 ——
> X11 后端用**根窗口 `GrabKey`** 真正注册，并覆盖 `on_global_hotkey` 完成派发：
>
> - 注册时先在根窗口订阅 `KeyPress`（`GrabKey` 只登记被动抓取，抓取激活后产生的事件
>   仍按抓取窗口自身的事件掩码过滤，根窗口默认没订阅 → 不补就永远收不到）；
> - **锁定键组合必须一起抓**：`GrabKey` 的修饰键掩码是精确匹配，事件里多出的
>   CapsLock（`0x02`）/ NumLock（`0x10`）/ ScrollLock（`0x80`）任一位都会让匹配失败，
>   所以除基础组合外再抓这 3 位的全部 7 种非空叠加组合；查表前把锁定键位剥掉。
>   锁定组合被抓其它应用占用只降级为 `debug` 日志，基础组合失败才如实报「已被其它应用占用」；
> - 按键名反查键码复用 `keystroke_from_xkb`（同一份 keymap 反扫 8..=255），
>   保证注册侧与事件侧命名口径一致；
> - 根窗口的按键有两种：抓取命中的热键与普通透传按键，后者必须丢弃，否则会当成焦点窗口的
>   按键重复派发一次；
> - Wayland / 无头后端用 `LinuxClient` 默认实现返回「不支持」错误而非静默 `Ok`，
>   该契约由 `headless::client::tests::headless_backend_rejects_global_hotkey` 固定。
>
> 验证：`daemon_app`（`cmd-shift-k`，id=1）+ `xdotool key --clearmodifiers super+shift+k`
> → 回调打印一次；`xset numlock on` / `capslock on` 后仍触发；未注册的
> `super+alt+shift+k` 不触发。

**3) 权限查询** — `crates/rgpui-linux/src/linux/permissions.rs`

- `:24-27` Accessibility **恒返回 `Granted`**（实际 AT-SPI 由 `atspi` 控制，可用 portal/AT-SPI 判断）
- `:38-42` ScreenCapture 恒返回 `NotDetermined`（应走 `xdg_desktop_portal.rs` 里的 portal）

> **已修复**（`7fa8b7bf47`）：改成按「本机此刻能不能真的做成这件事」如实判定，
> Linux 上三类权限各有不同口径：
>
> | 权限 | X11 会话 | Wayland 会话 |
> |------|----------|--------------|
> | Accessibility | AT-SPI 栈在跑才 `Granted`；`NO_AT_BRIDGE=1` → `Denied`；探测失败 → `Unavailable` | 同左（与显示协议无关，只看会话总线上的 AT-SPI） |
> | ScreenCapture | `Granted`（X11 协议无按客户端的屏幕访问控制） | 有 `org.freedesktop.portal.ScreenCast` → `NotDetermined`（授权发生在真正建会话时）；门户没实现 → `Unavailable` |
> | InputMonitoring | `Granted`（根窗口 `GrabKey` 无需授权，§4.2-2 就靠它） | 有 `GlobalShortcuts` 门户 → `NotDetermined`；否则 `Unavailable` |
>
> - AT-SPI 判定分两步：会话总线 `org.a11y.Bus.GetAddress` 拿到辅助功能总线地址，
>   再连上该总线调用 `org.a11y.atspi.Registry.GetRegisteredEvents`。
>   **只查第一步会误判**：本机 `toolkit-accessibility=false` 时 `GetAddress` 依然返回地址，
>   注册表进程是否活着必须到那条私有总线上问；
> - 门户接口可用性用根对象 `/org/freedesktop/portal/desktop` 的一次 `Introspect`
>   判 XML 里有无 `interface name="…"`，**不必为 `ashpd` 打开 `screencast` /
>   `global_shortcuts` feature**（那些模块带来的会话/授权类型这里用不上）；
> - 用 `Properties.Get(<接口>, "Version")` 也能区分，但错误串受本地化影响（本机
> 「No such interface」/「No such property」），Introspect 更稳；
> - `request_permission` 不再只留一行「不需要权限」的日志：先如实回报当前状态，
>   再给出可执行指引（Linux 没有应用侧权限弹窗）。
>
> 验证（`linux::permissions::tests::query_permissions_in_live_session`，`#[ignore]`，
> 需真实会话总线；`cargo test -p rgpui-linux -- --ignored --nocapture permissions`）：
>
> ```
> 本机 X11 会话                Accessibility=Granted ScreenCapture=Granted InputMonitoring=Granted
> NO_AT_BRIDGE=1               Accessibility=Denied
> WAYLAND_DISPLAY=wayland-0    ScreenCapture=NotDetermined（门户有 ScreenCast）
>                              InputMonitoring=Unavailable（22.04 门户无 GlobalShortcuts）
> DBUS_SESSION_BUS_ADDRESS=坏  Accessibility=Unavailable（如实降级）
> ```
>
> **顺带记录**：`ashpd` 0.13 有 `desktop::global_shortcuts::GlobalShortcuts`（门户
> `org.freedesktop.portal.GlobalShortcuts`），这是 **Wayland 上做全局热键的正路**，
> 本机 portal 未实现该接口；§4.2-2 目前对 Wayland 返回「不支持」是如实的，将来要补就走这条。

**4) 应用菜单** — `crates/rgpui-linux/src/linux/platform.rs:584-596`

- `set_menus` 只把菜单存进 `common.menus`，**没有任何 UI 展示**
- `on_app_menu_action` / `on_will_open_app_menu` / `on_validate_app_menu_command`（`:560-575`）
  把回调存进 `common.callbacks` 后，**全仓库没有任何地方调用它们**
- `set_dock_menu` 就一行 `// todo(linux)`

### 4.3 [P1] 约 19 个 `Platform` 方法在 Linux 上是静默 no-op

对比 `crates/rgpui/src/platform.rs`（有默认空实现）与 `crates/rgpui-linux/src/linux/platform.rs`，
Linux 缺失（**tray 8 件套已随 §4.1 移出；`os_info`、`system_idle_time`（`d6f6a4c598`）、
`on_global_hotkey`（`d5e4819376`）、权限查询（`7fa8b7bf47`）已实现；`network_status` 已实现（`651a94a618`，见 §4.7；
`set_keep_alive_without_windows` 按 §4.1 末改为「状态收回核心层」，平台侧方法已删除**）：

```
authenticate_biometric        biometric_status         cancel_user_attention
id                            microphone_status        on_media_key_event
on_network_status_change      on_system_power_event
perform_dock_menu_action      read_from_find_pasteboard
request_microphone_permission request_user_attention   set_dock_badge
show_context_menu             show_dialog              start_power_save_blocker
stop_power_save_blocker       update_jump_list         write_to_find_pasteboard
```

其中**核心层自己会调用**的：**一个都没有**。逐个核实后的结论（此前记为
「`read/write_from_find_pasteboard`、`update_jump_list`、`perform_dock_menu_action`
会被核心调用」是**不准确的**，已更正）：

- `read/write_from_find_pasteboard`：`App` 侧的入口本身带 `#[cfg(target_os = "macos")]`
  （`crates/rgpui/src/app.rs:1403`、`:1413`），Linux 上根本不可达；
- `update_jump_list` / `perform_dock_menu_action` / `set_dock_badge`：只有 `App` 的公开方法
  在转调平台（`app.rs:2458`、`:2472`），核心内部无调用点，仅 Windows 自己用
  （`rgpui-windows/src/platform.rs:304`）；
- 其余（`show_dialog`、`show_context_menu`、`on_media_key_event`、
  `on_network_status_change` 等）同样是「应用不调用就什么都不发生」。

> 这里的 `show_context_menu` 指 `Platform` 的同名方法；
> `crates/rgpui/src/input_ui/context_menu.rs:308` 那处是**元素**的 `show_context_menu`，
> 与平台方法无关，别混为一谈。

**分诊建议**（前提见 §4.8：这些方法**在 `App` 上没有包装**，
只补 Linux 实现等于写一份应用调不到的代码，所以每一条都要「API 形状 + App 包装 + 调用点」一起做）：

- 可实现，但先要定 App 侧 API：`on_network_status_change`（门户 `NetworkMonitor` 的 `changed` 信号，
  需要像 tray 那样把事件路由回主线程再派发）、
  `on_system_power_event`（`login1`，`platform.rs` 已有 `PrepareForSleep` 监听可复用）、
  `start/stop_power_save_blocker`（`login1` 的 `Inhibit` 是**持有 fd 即生效**，
  返回 `Option<u32>` 让应用自己记 ID 的形状不合适，应改成 `Drop` 即释放的句柄）、
  `microphone_status` / `request_microphone_permission`（portal `Camera`/`Device`；
  这两个保留特例方法是因为带 `FnOnce(bool)` 回调，见 §4.8）
- 已实现：`on_global_hotkey`（§4.2-2）、`os_info`、`system_idle_time`
  （`org.freedesktop.ScreenSaver` 与 Mutter `IdleMonitor` 依次探测）、`network_status`（§4.7）、
  权限查询/请求（`check_permission` / `request_permission`，§4.2-3 + §4.8）、
  `set_keep_alive_without_windows`（§4.1 末）
- 平台语义上不需要，保持默认即可：`set_dock_badge`、`update_jump_list`、
  `perform_dock_menu_action`、`read/write_from_find_pasteboard`、`biometric_status`、
  `authenticate_biometric`、`request_user_attention` / `cancel_user_attention`
  （窗口级提醒已由 `PlatformWindow::request_attention` 承担，见 §4.4）
- 需要决定是否返回「不支持」而非静默成功：`show_dialog`、`show_context_menu`、
  `on_media_key_event`

### 4.4 [P1] 窗口层（`PlatformWindow`）方法缺失

| 方法 | X11 | Wayland | 核心层是否调用 | 说明 |
|------|:---:|:-------:|:---:|------|
| `activate` | ⚠️ | ✅ | ✅ | **X11 已修**（§2.3：先 map 再 `_NET_ACTIVE_WINDOW`）；Wayland 用 xdg-activation token |
| `request_attention` | ✅ | ❌ | ✅ | **X11 已实现**（`4f782cb1fa`）：写 ICCCM `WM_HINTS` urgency 位。Wayland 侧没有「客户端请求提醒」的协议入口，要做得靠 portal `org.freedesktop.portal.Notify` 之类的通知替代 |
| `get_title` | ✅ | ✅ | ✅ | **已实现**（`4f782cb1fa`）：优先 `_NET_WM_NAME`（UTF-8），回退 `WM_NAME`（STRING）。无头后端早就有（见下方备注） |
| `set_mouse_passthrough` | ✅ | ✅ | ✅ | **已实现**（`223061c8bc`）：X11 走 Shape 输入区域，Wayland 走空 `wl_region`（见下方口径） |
| `set_input_region` | ✅ | ✅ | ✅ | X11 已补齐（`223061c8bc`），Wayland 原本就有 |
| `set_exclusive_zone` / `set_exclusive_edge` | ❌ | ❌ | ✅ | 层叠 shell 面板区域 |
| `render_to_image` | ❌ | ❌ | ✅ | 截图/测试 |
| `map_window` | ✅ | ❌ | ✅ | Wayland 缺失 |
| `set_titlebar_visible` | ❌ | ❌ | — | X11 可用 MWM hints |
| `window_extended_style` / `set_window_extended_style` | ❌ | ❌ | — | Windows 专属语义 |
| `get_raw_handle` | ❌ | ❌ | — | 需决定 Linux 上的返回形态 |
| `set_edited` / `set_document_path` / `set_traffic_light_position` / tab 系列 / `show_character_palette` / `titlebar_double_click` / `window_controls` | ❌ | ❌ | ✅ | **macOS/Windows 专属**，有 trait 默认实现，属正常 |
| `supports_dom` / `dom_tree_update` / `on_dom_event` / `on_dom_scroll` | ❌ | ❌ | ✅ | 仅 `rgpui-web` 实现（`crates/rgpui-web/src/window.rs:746`），Linux 不需要 |

> ~~`headless/window.rs:174` 有 `get_title`，X11/Wayland 反而没有 —— 实现分布不一致。~~
> 已补齐（`4f782cb1fa`），三个后端都有 `get_title`。

**`request_attention` 的 X11 实现口径**（本机 mutter 实测，照此实现否则不生效）：

- 客户端**自发** `_NET_WM_STATE` 客户端消息请求 `_NET_WM_STATE_DEMANDS_ATTENTION` 无效 ——
  该状态是「WM 设置、客户端只读」，mutter 直接忽略，`xprop` 里始终不出现该原子；
- 正规入口是 ICCCM 的 `WM_HINTS` urgency 位（`WmHints::new()` + `urgency_hint = Some(true)`，
  同时保留 `input` / `initial_state`，别把已有字段清空），行为与
  `xdotool set_window --urgency 1` 一致：`xprop` 出现 `WmHints(... urgency ...)`，
  即 "The urgency hint bit is set"；
- 提醒是**一次性**的：WM 在窗口被激活后自行清位，因此不需要 `cancel_user_attention` 的实现。

**鼠标穿透 / 输入区域的实现口径**（`223061c8bc`）：

- **X11**：Shape 扩展的 INPUT 形状决定事件路由。`ShapeRectangles(operation=SET, kind=INPUT,
  rectangles=[])` 即空输入区域 —— 窗口照常合成显示，事件落到下层；恢复用
  `ShapeCombine(SET, INPUT, BOUNDING, win, 0, 0, win)`，省掉一次 `GetGeometry` 往返去问窗口尺寸；
  局部热区就传矩形列表（`Bounds<Pixels>` → `xproto::Rectangle`，`i16`/`u16` 夹紧）。
- X11 **没有**「创建时即穿透」的窗口属性，所以 `WindowOptions.mouse_passthrough` 的意图要在
  `params` 被移交进 `X11WindowState` 之前取出，建完窗、`set_wm_properties` 之后立刻应用。
- Shape 是**可选扩展**：客户端启动时 `prefetch_extension_information(shape::X11_EXTENSION_NAME)`，
  窗口侧用 `extension_information(...).ok().flatten().is_some()` 判定；缺扩展只记一条 debug 日志、
  不发请求（否则服务端以 `BadMatch` 拒绝）。
- **Wayland**：没有对应扩展，「空 `wl_region`」就是通用做法；`set_input_region(None)` = 无限区域
  （整个 surface）。穿透意图同样在 `WaylandWindow::new` 里先取出，在首个 `surface.commit()` 之后应用；
  region 在 `commit` 之后立即 `destroy()` 是安全的（请求已排队，服务端按顺序处理）。

**验证**（`DISPLAY=:10.0` 同一 xrdp 会话里同时跑 `desktop_pet`（`mouse_passthrough: true`，
窗口 `0x3000001`）与 `hello_world`（不穿透，`0x2a00001`），再用 x11rb 写约 30 行探针回读
`ShapeGetRectangles`；探针是仓库外的临时 crate，`x11rb = { version = "0.13.2", features = ["shape"] }`）：

```
0x03000001 320x320+524+224  BOUNDING[320x320+0+0]  INPUT[]                  (0 rect)   ← 穿透生效，外形未变
0x02a00001 500x500+10+45    BOUNDING[500x500+0+0]  INPUT[500x500+0+0]       (1 rect)   ← 对照组
mode=restore  → INPUT[320x320+0+0]     mode=partial → INPUT[100x100+50+50]  mode=empty → INPUT[]
```

未修时叠加窗口的 INPUT 会回落到外形（`ShapeGetRectangles` 对**未整形**窗口返回 BOUNDING 的内容），
也就是 `restore` 那一行的 320x320 —— 所以「0 rect vs 1 rect」是这次改动造成的真实差异，不是环境噪声。
三种请求形式（空列表 / BOUNDING 重设 / 单矩形）逐条对应实现里的三条分支，都实测有效。

**事件路由本身没能在本机实测**：xrdp 会话里 `xdotool click` 的坐标会被 `xrdpMouse` 绝对设备回弹
（`xev -id 0x2a00001` 收得到 `EnterNotify`/`FocusIn`，却收不到任何 `ButtonPress`），而
GNOME/mutter 的全屏合成覆盖窗又让 `xdotool getmouselocation` 在叠加窗区域恒返回 `0x240000a`。
INPUT 形状控制事件路由是 X 协议规范语义，回读形状即为充分证据。

Wayland 侧本次**只有编译验证**（`cargo clippy -p rgpui-linux --no-default-features --features wayland
--all-targets -- -D warnings` 干净），本机没有 Wayland 会话；空 region 的语义与 X11 空 INPUT 同构。
另：`--no-default-features --features x11` 这一变体有 `PIPE_READ_TIMEOUT` /
`read_fd_with_timeout` / portal `CursorTheme`/`CursorSize` 四条 dead-code 报错，
**属改动前既有**（这些项只在 Wayland 路径被读），与穿透无关，CI 也不构建该组合。

### 4.5 [P2] workspace 构建在 Linux 上被 webview 示例阻塞

```
error: failed to run custom build command for `glib-sys v0.18.1`
  The system library `glib-2.0` required by crate `glib-sys` was not found.
```

依赖链：`examples/webview` 启用 `rgpui/webview` → `wry` → `webkit2gtk` → `gtk` → `glib-sys`。

本机 `pkg-config` 实测：

```
MISSING glib-2.0    MISSING gtk+-3.0    MISSING webkit2gtk-4.1
OK      xkbcommon                                        （x11rb 为纯 Rust，无需 libx11-dev）
```

影响：`cargo check --workspace`、`cargo clippy --workspace --lib --bins -D warnings`
（CI 的 Linux 作业）都会失败。**这也是本次 tray / activate 改动没能跑 AGENTS.md
要求的 `cargo check --workspace` 的原因** —— 只跑了
`cargo check/clippy -p rgpui-linux --all-targets -- -D warnings` + `cargo fmt -p rgpui-linux`，
需要系统库（要用户授权 `apt install`）才能补齐这一项。

需要在开发文档中写明 Linux 构建前置包：

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libglib2.0-dev
```

### 4.6 [P2] rgpui 没有日志初始化入口 —— **已修复**（`3fcaa73c98`）

~~全仓库只有 `crates/rgpui-web/src/logging.rs:37` 有 `log::set_logger`。~~
`rgpui-wgpu` / `rgpui-linux` 里大量 `log::info!/debug!`（GPU 适配器选择、X11 初始化、
portal 调用）**在原生平台上一条都看不到** —— 这次排障被迫临时给示例加 `env_logger` 才拿到关键日志。
§3.4 的 GL 降级失效、§2.5 的黑屏定位都因此变难。

> **已实现**：`rgpui::init_logging()`（`crates/rgpui/src/logging.rs:137`，核心层无新增依赖 ——
> 直接用 `std` 写 stderr，不引入 `env_logger`）。
>
> - 级别取自 `RUST_LOG`，支持 `info` 与 `warn,rgpui_wgpu=debug` 这类按 crate 放开的写法；
>   未设置时只输出 `error`，不给正常运行刷屏；
> - **可重复调用**（后续调用被忽略），所以放在 `main` 第一行是安全的；
> - 已有输出器时不抢它的级别设置 —— `rgpui-web` 的 `set_logger` 仍优先，Web 侧行为不变。
>
> 用法（示例已在 `main` 开头接入，如 `examples/hello_world/src/main.rs:113`、
> `examples/tray/src/main.rs`）：
>
> ```rust
> fn main() {
>     rgpui::init_logging();
>     rgpui_platform::application().run(|cx| { ... });
> }
> ```
>
> 排障时用 `RUST_LOG=debug` 运行即可看到 §3.4 的后端选择、§4.2 的 portal/GrabKey 调用过程。

### 4.7 [P1] `network_status` —— **已实现**（`651a94a618`，门户优先 + `/sys/class/net` 兜底）

核心层 `Platform::network_status` 在 Linux 上原本是静默 no-op（返回默认值），
调用方拿到的「网络状态」与真实链路无关。现已实现于
`crates/rgpui-linux/src/linux/system_info.rs`，`platform.rs` 的
`network_status()` 只做转发。

**两级取数**：

1. **portal `org.freedesktop.portal.NetworkMonitor`**（`/org/freedesktop/portal/desktop`）
   —— 先 `GetAvailable`，再 `GetConnectivity`。连通性档位来自 NetworkManager：

   ```
   0 未知、1 无到互联网的线路由、2 强制门户、3 有限连通、4 完整连通
   ```

   映射为 `NetworkStatus`：`0 | 4 => Connected`、`1..=3 => ConnectedBelowRequired`、
   其余 `Connected`（`GetAvailable` 为 false 直接 `Disconnected`）。
   `0`（未知）按 `Connected` 处理是**刻意的**：门户说「有网卡可用」但没说「探到了什么」，
   此时报 `ConnectedBelowRequired` 会让调用方误判成「被门户劫持」，
   而它原本就是 §4.3 说的「假实现」行为，不如按可用上报。

2. **`/sys/class/net/*/operstate` 兜底** —— 跳过 `lo`，任一网卡 `up` 即 `Connected`。
   这条路径**只能回答「有没有 UP 的网卡」**，没有连通性探测能力，
   因此永远不会给出 `ConnectedBelowRequired`；这是设计上的取舍，不是漏实现。

取数次序是「门户可用就用门户，任一 D-Bus 调用失败即整体回落到 sysfs」——
`DBUS_SESSION_BUS_ADDRESS` 被指到坏地址时不会拖垮整个查询。

> **本机实测**（`cargo test -p rgpui-linux -- --ignored --nocapture network`）：
>
> ```
> 正常会话            network_status() => Connected / sysfs_network_status() => Connected
> DBUS_SESSION_BUS_ADDRESS=坏  走 sysfs => Connected（门户路径静默失败，不影响结果）
> ```
>
> 活体测试为 `#[ignore]`：无头 CI 上没有会话总线，跑它会误报。

### 4.8 [P1] 平台能力在应用层没有调用点 —— **已修复**（`e00ddd95aa`）

排查 §4.2-3 / §4.3 时发现一个比「没实现」更根本的问题：
**`App` 对这批能力一个包装方法都没有**，而 `App::platform` 是私有字段。
于是 `os_info`、`system_idle_time`、`network_status`、`accessibility_status`、
`set_auto_launch`、`is_auto_launch_enabled`、`focused_window_info` 即使在三平台都实现了，
**应用代码也一行都调不到** —— 与 §4.1 末的 keep-alive 是同一类缺陷的两个方向：
那边是「存了没人读」，这边是「实现了没人能调」。

> 由此定一条口径（**已写进 AGENTS.md 的「平台 trait 自有 API」**），
> 后续补 `Platform` 方法时同样适用：
> **补一个平台方法，就必须同时给 `App` 包装 + 一个真实调用点（示例或核心逻辑）**。
> 只往 `rgpui-linux` 里加实现，产出的正是本节批评的东西。

**已接回应用层**（`crates/rgpui/src/app.rs`）：

```
check_permission(PermissionType)      request_permission(PermissionType)
os_info()                             system_idle_time()
network_status()                      set_auto_launch(app_id, enabled)
is_auto_launch_enabled(app_id)        focused_window_info()
```

权限改成**统一入口**而不是逐类别加方法：

- 原来只有 `accessibility_status` / `request_accessibility_permission` 这一对特例，
  `PermissionType::ScreenCapture` / `InputMonitoring` 在 macOS 与 Linux 里都写好了判定，
  却**没有任何 trait 方法能问它们** —— 特例方法的毛病就在于每加一个类别都要再补一对；
- 现在 `check_permission(kind)` / `request_permission(kind)` 覆盖整个 `PermissionType`，
  特例对删除，macOS / Windows / Linux 三处实现同步改造（Windows 无按应用授权模型，返回 `Granted`；
  macOS 只有辅助功能有弹窗，其余交给 TCC 首次使用时自动询问）；
- `request_microphone_permission` **保留**为特例：它带 `FnOnce(bool)` 回调，
  与 `request_permission` 的「触发即返回」形状不同，且 `PermissionType` 里没有麦克风类别。

顺带修掉一个潜伏编译错误：`rgpui-macos/src/permissions.rs` 的非 macOS 分支返回
`PermissionStatus::Unknown`，而该枚举根本没有这个变体 —— 因为整条分支在
`#[cfg(not(target_os = "macos"))]` 下、三个平台都不编译它，所以一直没暴露。

**刻意没动**的（缺的是 App 侧 API 设计，不是 Linux 实现）：
`start/stop_power_save_blocker`、`on_system_power_event`、`on_network_status_change`、
`on_media_key_event`、`microphone_status`、`biometric_status` / `authenticate_biometric`、
`request/cancel_user_attention`、`set_dock_badge`、`show_dialog`、`show_context_menu`。
例如电源阻止器返回 `Option<u32>` 让应用自己记 ID 去停止，这个形状本身就值得先改
（更合理的是给出一个 `Drop` 即释放的句柄），在 Linux 上实现它只会多一份没人调的代码。

> **本机实测**（`DISPLAY=:10.0 daemon_app` 启动输出，见 §六）：
>
> ```
> OS: Ubuntu 22.04 LTS (Jammy Jellyfish)
> Network: Connected
> Idle: Some(1)
> Permission Accessibility: Granted
> Permission ScreenCapture: Granted
> Permission InputMonitoring: Granted
> Auto launch enabled: false
> ```
>
> 负路径同样如实（证明不是常量转发）：`NO_AT_BRIDGE=1` 下
> `Permission Accessibility: Denied`，其余两项不变。

---

## 五、环境问题（非 rgpui 代码缺陷）

1. **仓库路径含冒号**：`/home/abc/shared-drives/C:/code/...`
   cargo 拼接 `LD_LIBRARY_PATH` 时用 `:` 作分隔符，直接报错
   `path segment contains separator ':'`。
   绕过方式：`--target-dir` 指到无冒号路径（本次用 `/tmp/rgpui-target`、
   release 对照用 `/tmp/rgpui-target-release`）。
2. **源码在 RDP 共享盘上**（`xrdp-chansrv`），`du -sh target` 都会挂住，
   编译产物必须放本地盘；读源码也偏慢，全量构建约 2.5 分钟（release 约 5 分钟）。
   **FUSE 还有个新发现的坑**：`git add` 后 `git diff --cached` 可能读不到刚写入的索引
   （`git status` 显示干净但 HEAD 已含改动），提交后要用 `git log` / `git show` 核实，
   别急着重复提交。
3. **换行符噪音**：工作区约 242 个文件因 CRLF↔LF 被标为 modified（29875 增 / 29875 删）。
   排查改动请用 `git diff --ignore-cr-at-eol`（注意 `git status` **不支持**该选项）；
   **只 stage 具体文件名，绝不用 `git add -A`**；提交在 Windows 端进行。
4. ~~**会话为 xrdp 虚拟显示，没有 DRI3 → Vulkan 无法 present，屏幕上看不到任何画面**~~
   —— **此条已被 §3.3 推翻，删除。** 无 DRI3 的 xrdp 会话下画面完全正常。
   仍然成立的部分：本会话是**软件渲染**，性能远低于原生，所以 §2.5(a) 那段启动耗时会夸大。
5. **Mesa 是 Ubuntu 22.04 的 22.x**，EGL 只公布 `EGL_MESA_platform_xcb`，
   导致 wgpu-hal 的 GL 后端在本机不可用（§3.4）→ **没有 GL 降级备胎**，
   只能走 lavapipe Vulkan。不阻塞功能，但排查后端选择问题时少一条对照路径。
6. **GNOME 面板无法用 `xwd` 截图**：gnome-shell 面板是 GL 合成的，
   抓 root 或抓 gjs stage 窗口都是全黑；`org.gnome.Shell.Screenshot` 返回 `AccessDenied`。
   → 验证 tray 图标只能靠**协议级证据**（`RegisteredStatusNotifierItems`、
   `dbus-monitor` 观察 shell 主动调用、`IconPixmap` 回读解码），见 §4.1。
   **应用窗口可以正常抓**（`xwd -id <WID>`）。

---

## 六、验证方法备忘

```bash
# 所有 cargo 命令都要带无冒号的 target-dir
--target-dir /tmp/rgpui-target

# 检查某个示例解析出的后端 feature（修复前是 []）
cargo tree -p hello_world -f "{p} [{f}]" --target-dir /tmp/rgpui-target | grep rgpui-linux

# 打开日志看 GPU 适配器探测（示例已调 rgpui::init_logging()，见 §4.6）
RUST_LOG=info,wgpu_hal=debug ./hello_world

# 确认窗口真的创建了（不是只看进程活着）
xprop -root _NET_CLIENT_LIST | grep -o "0x[0-9a-f]*"   # 逐个查 _NET_WM_PID
xwininfo -id <WID>                                      # Map State 应为 IsViewable
wmctrl -l                                               # 注意：rgpui 窗口显示 N/A（§2.4）

# 判断是否有托盘宿主
dbus-send --session --dest=org.freedesktop.DBus --type=method_call --print-reply \
  /org/freedesktop/DBus org.freedesktop.DBus.ListNames | grep -i statusnotifier
xprop -root _NET_SYSTEM_TRAY_S0                          # XEmbed 宿主（本机无）

# SNI 端到端
gdbus call --session --dest org.kde.StatusNotifierWatcher --object-path /StatusNotifierWatcher \
  --method org.kde.StatusNotifierWatcher.RegisteredStatusNotifierItems
# 注意：这是个**属性**不是方法，按方法调会 UnknownMethod
gdbus call --session --dest org.kde.StatusNotifierWatcher --object-path /StatusNotifierWatcher \
  --method org.freedesktop.DBus.Properties.Get org.kde.StatusNotifierWatcher RegisteredStatusNotifierItems
gdbus call --session --dest <应用总线名> --object-path /StatusNotifierItem/Menu \
  --method org.kde.StatusNotifierItem... # 注意负数参数要用 -- 分隔
# dbusmenu 的 data 参数类型是 v 不是 av：传 '<>' 包起来的 ''，且第 4 个 u 参数是裸数字
# （写成 "" 会被静默忽略，写成 "u 0" 报 Error parsing parameter 4 of type "u"）
gdbus call --session --dest <应用总线名> --object-path /StatusNotifierItem/Menu \
  --method com.canonical.dbusmenu.GetLayout -- 0 -1 "[]"
gdbus call --session --dest <应用总线名> --object-path /StatusNotifierItem/Menu \
  --method com.canonical.dbusmenu.Event -- <id> "clicked" "<''>" 0

# 手工驱动 SNI 动作（绕开无法截图的面板）
gdbus call … --method org.kde.StatusNotifierItem.Activate 0 0
# 用菜单项驱动「开窗口 → 关窗口 → 再开」来验 §2.3 与 §4.1 末的 keep-alive：
# GetLayout 里拿到 Show Overlay / Quit 的 id，逐个 Event 触发，
# 用 _NET_CLIENT_LIST 增量 + xdotool search --pid <PID> 找新窗口
# （rgpui 窗口没有 WM_CLASS/_NET_WM_PID 之外的属性，wmctrl -l 显示 N/A）

# 验证「关闭后恢复」：先看 WM 状态，再触发托盘项，再看状态
wmctrl -i -c <WID>                 # 模拟关闭
xprop -id <WID> WM_STATE           # Withdrawn = 修复前会卡住
xwininfo -id <WID>                 # 修复后应回到 IsViewable / Normal

# 抓窗口像素做量化分析（xwd 文件头 25 个 CARD32，数据在窗口名补 4 字节对齐之后）
xwd -id <WID> -out /tmp/w.xwd
# 用 python 解析 bits_per_pixel / bytes_per_line，统计非零像素数与 alpha 非零占比；
# 非零占比接近 0 且出现 01010101/02020202 递增值 = 未初始化显存，即从未 present。
# 性能注意：纯 Python 逐像素遍历 1364x768 会超过 120s 工具超时，
# 改用 Image.frombuffer(..., 'BGRX', ...)（/tmp/xwd2png.py、/tmp/blackprobe4.py）。

# 黑屏/首帧时序量化（§2.5）：轮询 _NET_CLIENT_LIST 找新窗口 → xwininfo → xwd 统计黑色占比
python3 /tmp/blackprobe4.py /tmp/rgpui-target-release/release/tray

# 验证全局热键（§4.2-2）：注入按键并观察回调日志
RUST_LOG=info ./daemon_app &
xdotool key --clearmodifiers super+shift+k        # --clearmodifiers 避免 xdotool 自己带 modifiers
# 锁定键必须单独测（GrabKey 掩码精确匹配，这是最容易漏的一类）
xdotool key Num_Lock && xset q | grep -i numlock   # xset numlock on 在本机不可用
xdotool key Caps_Lock
# 反证：未注册的组合不应触发
xdotool key --clearmodifiers super+alt+shift+k
# 按键是否真到了服务器（xev 在根窗口上抓不到事件，别用它）
xinput test-xi2 --root | grep -A2 KeyPress

# 验证 urgency 提醒（§4.4）：客户端自发 _NET_WM_STATE 无效，要看 WM_HINTS
xprop -id <WID> WM_HINTS      # 应出现 "The urgency hint bit is set"

# 验证鼠标穿透（§4.4）：回读 X Shape 的 INPUT 形状，别试图用合成点击去证明
# 本机 xrdp 会话里 xdotool click 的坐标会被 xrdpMouse 绝对设备回弹：
#   xev -id <WID> 只收得到 EnterNotify/FocusIn，永远收不到 ButtonPress；
#   xdotool getmouselocation 在叠加窗区恒返回 mutter 的合成覆盖窗（本机 0x240000a）。
#   （xev 的 -id 模式不打印 banner，别把「没输出」当成「没跑起来」）
# 探针是仓库外的临时 crate（/tmp/xshape）：x11rb = { version = "0.13", features = ["shape"] }，
# 核心就三个调用 —— get_geometry / shape_get_rectangles(id, SK::BOUNDING|SK::INPUT)
DISPLAY=:10.0 /tmp/xshape-target/debug/xshape 0x3000001 0x2a00001
#   穿透窗口：BOUNDING[320x320+0+0] INPUT[]（0 rect）；普通窗口：INPUT[500x500+0+0]
#   未整形窗口 ShapeGetRectangles(INPUT) 会回落成外形，即「修复前」的样子，所以 0 vs 1 是真差异
# 三种请求形式逐条对应实现分支：empty / restore(shape_combine BOUNDING) / partial(单矩形)
/tmp/xshape-target/debug/xshape mode=restore 0x3000001     # → INPUT[320x320+0+0]
/tmp/xshape-target/debug/xshape mode=partial 0x3000001     # → INPUT[100x100+50+50]
/tmp/xshape-target/debug/xshape mode=empty   0x3000001     # → INPUT[]

# 验证权限查询（§4.2-3）：常规测试不跑，需要真实会话总线
cargo test -p rgpui-linux -- --ignored --nocapture permissions
# 负路径靠环境变量造
NO_AT_BRIDGE=1                → Accessibility=Denied
WAYLAND_DISPLAY=wayland-0     → ScreenCapture=NotDetermined / InputMonitoring=Unavailable
DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/nonexistent → Accessibility=Unavailable
# 手工对照 AT-SPI 两步探测（只查第一步会误判：toolkit-accessibility=false 时地址照样返回）
gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus --method org.a11y.Bus.GetAddress
gdbus call --address '<上一步返回的地址>' --dest org.a11y.atspi.Registry \
  --object-path /org/a11y/atspi/registry --method org.a11y.atspi.Registry.GetRegisteredEvents
# 门户接口可用性（别用 Properties.Get 的错误串判断，受本地化影响）
gdbus introspect --session --dest org.freedesktop.portal.Desktop \
  --object-path /org/freedesktop/portal/desktop --xml | grep -o 'interface name="[^"]*"'

# 验证网络状态（§4.7）：门户路径 + sysfs 兜底都会打印
cargo test -p rgpui-linux -- --ignored --nocapture network
# 对照门户返回值与兜底结果
gdbus call --session --dest org.freedesktop.portal.Desktop --object-path /org/freedesktop/portal/desktop \
  --method org.freedesktop.portal.NetworkMonitor.GetAvailable
gdbus call --session --dest org.freedesktop.portal.Desktop --object-path /org/freedesktop/portal/desktop \
  --method org.freedesktop.portal.NetworkMonitor.GetConnectivity
# 造坏总线：应静默回落到 sysfs，结果依旧可用
DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/nonexistent \
  cargo test -p rgpui-linux -- --ignored --nocapture network
cat /sys/class/net/*/operstate        # 兜底路径只看这个

# 验证 keep-alive（§4.1 末）：daemon_app 关完窗口不退，hello_world 关完即退
RUST_LOG=info ./daemon_app &
# 用托盘菜单事件关掉最后一个窗口，进程应仍存活；再触发 Quit 项才退出
pkill -x daemon_app

# 验证平台能力在应用层可达（§4.8）：daemon_app 启动即打印查询结果
DISPLAY=:10.0 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus ./daemon_app 2>&1 | head -8
# 负路径证明不是常量转发
NO_AT_BRIDGE=1 ./daemon_app 2>&1 | grep Permission   # Accessibility=Denied，其余不变

# 进程清理：用精确名，别用 -f 匹配路径（会杀掉自己所在的 shell，exit 143）
pkill -x tray ; pkill -x inspector

# 换一个 X server 做对照（排除 WM/合成器因素）
Xephyr :11 -screen 700x700x32 &      # 无 WM、无合成器、扩展表不同
# 注意：Xephyr 里没有 EWMH，找窗口要用 xdotool search --pid <PID>，不是 _NET_CLIENT_LIST
# 也注意：这条对照实验只能排除 WM，不能证明是 X server 的问题（§3.3 的教训）

# 检查 EGL client extensions 是否含 wgpu-hal 需要的 XCB 平台
RUST_LOG=debug ./hello_world 2>&1 | awk '/Client extensions: \[/,/\]/'

# DRI3 检查（保留，但**别再据此判定画面能否呈现**，见 §3.3）
xdpyinfo | sed -n '/number of extensions/,/^$/p'
```

---

## 七、后续建议顺序

渲染、tray、以及 §4.2/§4.4/§4.6/§4.8 的一批缺陷已收口，剩下按「用户能感知 → 只有开发者感知」排序：

1. ~~**§4.2 三处假实现改成真实现**~~ —— 通知（`4f782cb1fa`）、全局热键（`d5e4819376`）、
   权限查询（`7fa8b7bf47`）都已改真实现并本机验证。
   **§4.2 仅剩应用菜单（§4.2-4）**：`set_menus` 存进 `common.menus` 后无人消费，
   三个 app-menu 回调全仓库无调用点，`set_dock_menu` 还是 `// todo(linux)`。
2. ~~**§4.4 补 X11 都缺的窗口方法**：`request_attention`、`get_title`~~ —— 已实现（`4f782cb1fa`）。
   ~~`set_mouse_passthrough`（X11 Shape / Wayland input region）与 X11 的 `set_input_region`~~ ——
   已实现并本机回读验证（`223061c8bc`，口径与证据见 §4.4）。
   剩余：Wayland 的 `map_window`、`render_to_image`、`set_exclusive_zone`/`set_exclusive_edge`。
3. ~~**§4.1 末 / §4.3 的 `set_keep_alive_without_windows`**~~ —— 跨平台缺陷，已修（`4bb1e1b3a2`）：
   状态收回核心层并改掉 `app.rs` 的退出判据，`Platform` 侧方法与 Windows 的
   `AtomicBool` 一并删除。本机 A/B 验证见 §4.1 末。
4. **§2.5 启动黑屏（b）** —— 按轻量方案给 `win_aux` 补 `background_pixel`；
   彻底方案（推迟 `map_window` 到首帧 present）**单独评估**，因为是全平台路径。
   当前用户指示：先不做。
5. ~~**§4.6 日志入口**~~ —— 已提供 `rgpui::init_logging()`（`3fcaa73c98`），
   示例与排障命令均已用上（见 §六）。
6. ~~**§2.4 的两个 P3**~~ —— 示例未传窗口标题一项已定性为**非平台缺陷**（见 §2.4 与本节第 9 项）；
   Inspector 帧率恒 0 归并到第 9 项一起处理。
7. ~~**§4.5 文档补 Linux 构建前置包**~~ —— AGENTS.md 已加「Linux 端构建前置」小节，
   写明 `apt install` 列表，以及未装这些库时实际可行的验证范围
   （`-p rgpui-linux --all-targets` + `cargo fmt -p rgpui-linux`），
   并明确它**不能替代** `cargo check --workspace`。
8. **§4.3 分诊表**剩下的 `on_system_power_event` / `microphone_status` /
   `start|stop_power_save_blocker` / `on_network_status_change` ——
   **先做 App 侧 API 设计再动 Linux**（§4.8 的口径：只补平台实现会产出调不到的代码）。
   其中电源阻止器的 `Option<u32>` 返回值应改为 `Drop` 即释放的句柄。
   （`system_idle_time`、`os_info` 已随 `d6f6a4c598` 完成，`network_status` 已随 §4.7 完成，
   窗口级提醒见 §4.4）
9. **§2.4 遗留 + §4.4 的 `render_to_image`** —— Inspector 帧率恒 0 与截图能力都卡在同一个
   前置条件：需要 **lavapipe 软渲染下可回读的 surface**（本机 vulkan 只有软件驱动，
   §2.2/§3.4）。属于「只有开发者感知」，但一旦打通能同时解决两项，值得排在窗口方法之前。
10. **§3.4** 记录 wgpu-hal / 老 Mesa 兼容问题（已降为 P3），评估是否向上游提 issue。
11. **Wayland 会话复测** —— 不再是渲染验证的阻塞项，但用于覆盖 Wayland 专属分支
    （§2.3 的 `hide`/`activate` 语义、§4.4 缺失的 `map_window`、§4.4 新加的
    `set_mouse_passthrough`（本机只有 X11 会话可实测，Wayland 侧仅编译验证））仍有独立价值。
