# Linux 平台问题审计清单

> 分支：`fix/linux-platform-issues`
> 审计日期：2026-10-06
> 审计环境：Hyper-V 虚拟机 + Ubuntu 22.04 + GNOME + **xrdp 远程桌面会话**（`Xorg :10 -config xrdp/xorg.conf`）
> 对照基准：`Platform` / `PlatformWindow` trait（`crates/rgpui/src/platform.rs`）与 `rgpui-linux` 的实现差异

---

## 一、摘要

在 Linux 上，rgpui 存在 **2 个已修复的致命问题**、**1 个已定位的画面问题**、
以及 **1 个功能缺失（tray）+ 一批静默失效的 API**。

最需要记住的一句话：

> **Linux 是唯一被 feature 门控「默认关死」的平台。** Windows 的 `rgpui-windows` 后端无条件编译，
> 所以同样的依赖声明在 Windows 正常、在 Linux 静默退化成 Headless —— 这正是「Windows 基本正常、
> Linux 很多不正常」的根本原因。

第二句要记住的：

> **xrdp 会话下永远看不到画面，因为 xrdp/Xephyr 都没有 DRI3，而 Mesa 的 Vulkan 呈现强制依赖 DRI3。**
> 这是环境限制，不是 rgpui 的 bug —— 换 Wayland 或原生 Xorg 才能验证渲染。

| 级别 | 问题 | 状态 |
|------|------|------|
| P0 | Linux 窗口后端 feature 未启用 → 无窗口无报错 | **已修复并验证** |
| P0 | 软件 Vulkan 驱动被 wgpu 判为「非一致性」而隐藏 → 无 GPU 适配器 | **已修复并验证** |
| P0 | 窗口能打开但画面永远不上屏 → **根因已定位：X server 缺 DRI3** | **已定位为环境限制**（见 §3） |
| P1 | GL 后端在老 Mesa 上不可用（wgpu-hal 只认 `EGL_EXT_platform_xcb`） | 已定位，上游兼容问题（见 §3.4） |
| P1 | tray 8 个 API 在 Linux 完全未实现 | 待实现 |
| P1 | 通知 / 全局热键 / 权限是「假实现」——返回成功但什么都没做 | 待实现 |
| P1 | 32 个 `Platform` 方法在 Linux 上静默 no-op | 待分诊 |
| P1 | X11/Wayland 窗口缺失 `request_attention` 等方法 | 待实现 |
| P2 | `cargo check --workspace` 被 webview 示例阻塞（缺 glib/gtk/webkit 系统库） | 待处理 |
| P2 | rgpui 无日志初始化入口，wgpu/GPU 诊断信息全部丢失 | 待处理 |
| — | 仓库路径含 `C:` 导致 cargo 构建失败；RDP 共享盘 I/O 极慢 | 环境问题 |
| — | xrdp 会话无 DRI3、EGL 缺驱动 → 无法在本会话看到渲染 | 环境问题 |

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
剩下的 GL 后端又因下节的 EGL 问题无法配置表面 → 最终 0 个适配器。

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
> 重试一次（见 §5 建议）。

---

## 三、[P0] 窗口能开但画面永远到不了屏幕上（已定位）

### 3.1 现象

完成 §2 两个修复后，窗口正常创建、进入渲染循环，但**屏幕上是透明的、没有任何内容**
（用户原话：「测试是透明窗口，而且没有内容」）。

### 3.2 证据链

**（1）渲染管线本身是正常的。** 在 `WgpuRenderer::draw()` 里临时插桩（诊断代码已回滚）：

```
DEBUG-draw: 取帧成功，继续绘制
DEBUG-draw: 场景规模 quads=64 paths=0 shadows=2 underlines=0 mono_sprites=0
DEBUG-draw: 本帧已提交并释放 surface 纹理（触发呈现）
```

`get_current_texture()` 成功、场景非空（64 个 quad + 2 个阴影）、
`queue.submit()` 完成、`SurfaceTexture` drop（即触发 present）完成，**全程零 wgpu 错误**。

**（2）但 X 缓冲里什么都没有。** 用 `xwd` 抓窗口后按原始字节统计：

```
resize 前：非零像素 766 / 250000 (0.3%)，全在首两行，取值为 01010101 / 02020202 …
           → 典型的未初始化显存残渣，不是渲染结果
resize 后：非零像素 20398 / 270400，且 96.3% 的像素 alpha = 0
           → 仍然不含 hello_world 的灰底 0x505050 与六个色块
```

即**提交的帧从未到达 X11 drawable**。窗口对合成器而言 alpha=0，所以显示为透明。

**（3）GPU 提交根本没完成。** 打开日志后，resize 时立刻暴露：

```
WARN  rgpui_wgpu::wgpu_renderer] Failed to poll device during resize: Timeout
ERROR rgpui_wgpu::wgpu_renderer] GPU error during frame (failure 1 of 10): Validation Error
    Caused by:
      In Surface::configure
        Failed to wait for GPU to come idle before reconfiguring the Surface
```

`device.poll(PollType::Wait { timeout: None })` 超时 —— **首帧的队列提交永远没有 signalled**，
present 因此永远不发生。

**（4）换 X 服务器复现，排除 WM/合成器因素。** 在同一台机器上另起一个
`Xephyr :11`（X.Org 1.21.1.3、无 WM、无合成器、扩展表完全不同）跑同一个 `hello_world`：

```
Failed to poll device during resize: Timeout
In Surface::configure → Failed to wait for GPU to come idle
两次 draw 之后窗口内容依旧未变（alpha 非零仅 0.1%）
```

两个 X server 现象**逐条一致** → 不是 mutter/合成器/WM 的问题。

### 3.3 根因：X 服务器没有 DRI3，Mesa 的 Vulkan WSI 无法呈现

两个 X server 的扩展表都**没有 DRI3**（xrdp Xorg 有 DRI2+Present，Xephyr 只有 Present）。
直接在 Mesa 的 Vulkan 驱动库里找到了它自己的诊断串：

```
$ strings /usr/lib/x86_64-linux-gnu/libvulkan_lvp.so | grep -i dri3
xcb_dri3_query_version
xcb_dri3_open
xcb_dri3_pixmap_from_buffers
DRI3
vulkan: No DRI3 support detected - required for presentation
Note: you can probably enable DRI3 in your Xorg config
```

**Mesa 明确要求 DRI3 才能做 present。** 缺失时 `vkAcquireNextImageKHR` 照常返回
（所以 wgpu 侧看不到任何错误、`get_current_texture()` 成功），但呈现链路走不通，
队列提交无法完成 → `poll` 超时 → 屏幕无内容。这与 3.2 的每一条证据都吻合。

**定性：环境限制，不是 rgpui 代码缺陷。** 本测试会话是 xrdp 提供的虚拟显示
（`Xorg :10 -config xrdp/xorg.conf`，驱动 `xrdpdev`），天然没有 DRI3；
`Xephyr` 这类嵌套 X server 同样没有。xrdp 会话下**无法**通过改 rgpui 代码看到画面。

**要看到画面，需要满足以下任一条件**：
1. **Wayland 会话**（`VK_KHR_wayland_surface` 走 dmabuf，不依赖 DRI3）—— 首选验证路径；
2. 用 `modesetting` 驱动跑在真实 `/dev/dri/card0`（本机 `hyperv_drm` 已加载）上的原生 Xorg，
   由其提供 DRI3；
3. 任何提供 DRI3 的 X 服务器。

### 3.4 连带发现：GL 后端在本机也完全不可用（降级路径失效）

既然 Vulkan 走不通，自然要问「为什么不能降级到 GL」。答案是 **GL 也起不来**：

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

于是匹配落空 → 走 `EGL_MESA_platform_surfaceless` 平台 →
`WindowKind::Unknown` → 无法为 X11 窗口创建 EGL window surface
→ surface format 列表为空 → GL adapter 被直接判死。

附带两个相关事实：

- 不带 `LIBGL_ALWAYS_SOFTWARE=1` 时，surfaceless 平台还会报
  `EGL 'eglInitialize' code 0x3001: DRI2: failed to load driver`；
  加上该变量后 EGL 初始化成功，但 surface format 问题依旧 → **GL 仍然不可用**。
- rgpui 侧的句柄是正确的（`XcbWindowHandle` + `XcbDisplayHandle`，
  见 `crates/rgpui-linux/src/linux/x11/window.rs:322-369`），**问题在 wgpu-hal 的扩展名匹配**
  —— 属于上游兼容性缺陷（对老 Mesa），可在审计/上游 issue 中记录；
  rgpui 侧可考虑的缓解是记录并暴露后端选择日志，方便用户诊断。

### 3.5 本节待办

1. **在 Wayland 会话下复测 `hello_world`** —— 这是唯一能在不换机器的前提下看到画面的路径，
   也是判定 §3.3 结论的最终交叉验证。
2. 若 Wayland 正常 → 在 `docs/` 中写明「Linux X11 需要 DRI3，xrdp/嵌套 X server 不支持，
   请用 Wayland 或原生 Xorg」。
3. 若 Wayland 也异常 → 才需要回头排查 rgpui 的 present 路径
   （重点：`alpha_mode` 选择、首帧提交时机）。
4. 记录 §3.4 的 wgpu-hal/老 Mesa 兼容问题，评估是否向上游提 issue。

---

## 四、功能缺失与假实现

### 4.1 [P1] tray 在 Linux 上完全没有实现

`grep -rn tray crates/rgpui-linux/` → **0 匹配**。

`crates/rgpui/src/platform.rs:496-512` 中 tray 全部 8 个方法都是默认空实现，
`rgpui-linux` 一个都没有覆盖，因此调用**静默无效果、回调永不触发**：

| 方法 | 行号 | 表现 |
|------|------|------|
| `set_tray` | 496 | 静默忽略 |
| `set_tray_icon` | 498 | 图标不显示 |
| `set_tray_menu` | 500 | 菜单不显示 |
| `set_tray_tooltip` | 502 | 无效 |
| `set_tray_panel_mode` | 504 | 无效 |
| `get_tray_icon_bounds` | 506 | 恒 `None` |
| `on_tray_icon_event` | 510 | 回调永不触发 |
| `on_tray_menu_action` | 512 | 回调永不触发 |

连带 `set_keep_alive_without_windows`（`:515`）也是 no-op，
`examples/tray/src/bin/tray_simple.rs:14` 正依赖它。

**实现路线（已确定：StatusNotifierItem via zbus）**，环境已验证可行：

```
$ dbus-send --session ... ListNames | grep -i statusnotifier
      string "org.kde.StatusNotifierWatcher"                    ← GNOME Shell 提供宿主
      string "org.fcitx.Fcitx5.StatusNotifierItem-33656-4"      ← 同协议应用正常显示

$ xprop -root _NET_SYSTEM_TRAY_S0
_NET_SYSTEM_TRAY_S0:  not found                                  ← XEmbed 老式托盘无宿主
```

- ✅ SNI 宿主存在 → SNI 路线可行；`ashpd` 已依赖 `zbus`，无需引入重型新依赖
- ❌ XEmbed（`_NET_SYSTEM_TRAY`）无宿主 → 走 XEmbed 的实现必然失败
- 桌面：`XDG_CURRENT_DESKTOP=ubuntu:GNOME`，面板进程仅 `gnome-shell`

**修复 §2 后 tray 示例的实测行为**：进程能起来并阻塞在 `LinuxClient::run` 的事件循环里
（`timeout` 返回 124，说明进程存活），**但托盘图标不出现、菜单不可用、只能 kill 结束**。
即保活是「碰巧」由事件循环挡住的 —— `set_keep_alive_without_windows`
（`crates/rgpui/src/platform.rs:515`）本身仍是 no-op，全仓库**只有写入、没有任何读取点**
（`crates/rgpui/src/app.rs:2535` 只是转发给平台）。
真正的「无窗口自动退出」逻辑在 `crates/rgpui/src/app.rs:1802-1810`：

```rust
QuitMode::Default => cfg!(not(target_os = "macos")),   // Linux = true
if quit_on_empty && cx.windows.is_empty() { cx.quit(); }
```

它只在**窗口关闭**时触发，因此当前未开窗口的 tray 示例不会命中；但任何
「先开窗再全关掉以驻留托盘」的应用都会因此退出 —— 实现 tray 时必须一并处理。

### 4.2 [P1] 假实现 —— 返回成功，但什么都没做

这类比「没实现」更危险：调用方拿到 `Ok(())` 以为成功了。

**1) 系统通知** — `crates/rgpui-linux/src/linux/notifications.rs:25-31`

```rust
pub fn show_notification(&self, title: &str, body: &str, _icon: Option<&str>) -> Result<()> {
    log::info!("发送通知: {} - {}", title, body);
    Ok(())          // 只写日志，未接 notify-rust / XDG portal
}
```

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

**3) 权限查询** — `crates/rgpui-linux/src/linux/permissions.rs`

- `:24-27` Accessibility **恒返回 `Granted`**（实际 AT-SPI 由 `atspi` 控制，可用 portal/AT-SPI 判断）
- `:38-42` ScreenCapture 恒返回 `NotDetermined`（应走 `xdg_desktop_portal.rs` 里的 portal）

**4) 应用菜单** — `crates/rgpui-linux/src/linux/platform.rs:584-596`

- `set_menus` 只把菜单存进 `common.menus`，**没有任何 UI 展示**
- `on_app_menu_action` / `on_will_open_app_menu` / `on_validate_app_menu_command`（`:560-575`）
  把回调存进 `common.callbacks` 后，**全仓库没有任何地方调用它们**
- `set_dock_menu` 就一行 `// todo(linux)`

### 4.3 [P1] 32 个 `Platform` 方法在 Linux 上是静默 no-op

对比 `crates/rgpui/src/platform.rs`（有默认空实现）与 `crates/rgpui-linux/src/linux/platform.rs`，
Linux 缺失：

```
authenticate_biometric        biometric_status         cancel_user_attention
get_tray_icon_bounds          id                       microphone_status
network_status                on_global_hotkey         on_media_key_event
on_network_status_change      on_system_power_event    on_tray_icon_event
on_tray_menu_action           os_info                  perform_dock_menu_action
read_from_find_pasteboard     request_microphone_permission
request_user_attention        set_dock_badge           set_keep_alive_without_windows
set_tray                      set_tray_icon            set_tray_menu
set_tray_panel_mode           set_tray_tooltip         show_context_menu
show_dialog                   start_power_save_blocker stop_power_save_blocker
system_idle_time              update_jump_list         write_to_find_pasteboard
```

其中**核心层确实会调用**的 14 个（其余为 macOS/Windows 专属或仅面向应用层）：

```
tray 8 件套、set_keep_alive_without_windows、on_global_hotkey、
read/write_from_find_pasteboard、update_jump_list、perform_dock_menu_action
```

**分诊建议**：

- 必须实现：`set_keep_alive_without_windows`、`on_global_hotkey`、tray 8 件套
- 可用 Linux 等价物实现：`network_status`（portal / `NetworkManager`）、`system_idle_time`
  （`org.freedesktop.ScreenSaver`）、`on_system_power_event`（`login1`，`platform.rs:196` 已有
  `PrepareForSleep` 监听可复用）、`microphone_status`（portal）、`request_user_attention`
  （X11 `_NET_WM_STATE_DEMANDS_ATTENTION`）
- 平台语义上不需要，保持默认即可：`set_dock_badge`、`update_jump_list`、
  `perform_dock_menu_action`、`read/write_from_find_pasteboard`、`biometric_status`
- 需要决定是否返回「不支持」而非静默成功：`show_dialog`、`show_context_menu`、`os_info`

### 4.4 [P1] 窗口层（`PlatformWindow`）方法缺失

| 方法 | X11 | Wayland | 核心层是否调用 | 说明 |
|------|:---:|:-------:|:---:|------|
| `request_attention` | ❌ | ❌ | ✅ | 任务栏提醒，X11 可用原子实现 |
| `get_title` | ❌ | ❌ | ✅ | 标题读取 |
| `set_mouse_passthrough` | ❌ | ❌ | ✅ | X11 可用 Shape 扩展 |
| `set_input_region` | ❌ | ✅ | ✅ | X11 缺失，Wayland 已实现 |
| `set_exclusive_zone` / `set_exclusive_edge` | ❌ | ❌ | ✅ | 层叠 shell 面板区域 |
| `render_to_image` | ❌ | ❌ | ✅ | 截图/测试 |
| `map_window` | ✅ | ❌ | ✅ | Wayland 缺失 |
| `set_titlebar_visible` | ❌ | ❌ | — | X11 可用 MWM hints |
| `window_extended_style` / `set_window_extended_style` | ❌ | ❌ | — | Windows 专属语义 |
| `get_raw_handle` | ❌ | ❌ | — | 需决定 Linux 上的返回形态 |
| `set_edited` / `set_document_path` / `set_traffic_light_position` / tab 系列 / `show_character_palette` / `titlebar_double_click` / `window_controls` | ❌ | ❌ | ✅ | **macOS/Windows 专属**，有 trait 默认实现，属正常 |
| `supports_dom` / `dom_tree_update` / `on_dom_event` / `on_dom_scroll` | ❌ | ❌ | ✅ | 仅 `rgpui-web` 实现（`crates/rgpui-web/src/window.rs:746`），Linux 不需要 |

> `headless/window.rs:174` 有 `get_title`，X11/Wayland 反而没有 —— 实现分布不一致。

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
（CI 的 Linux 作业）都会失败。需要在开发文档中写明 Linux 构建前置包：

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libglib2.0-dev
```

### 4.6 [P2] rgpui 没有日志初始化入口

全仓库只有 `crates/rgpui-web/src/logging.rs:37` 有 `log::set_logger`。
`rgpui-wgpu` / `rgpui-linux` 里大量 `log::info!/debug!`（GPU 适配器选择、X11 初始化、
portal 调用）**在原生平台上一条都看不到** —— 这次排障被迫临时给示例加 `env_logger` 才拿到关键日志。

建议提供 `rgpui::init_logger()` 之类的公开入口，或在示例模板中统一接入。

---

## 五、环境问题（非 rgpui 代码缺陷）

这些是本机/本仓库环境的限制，会干扰开发与验证，但不改代码也绕不开：

1. **仓库路径含冒号**：`/home/abc/shared-drives/C:/code/...`
   cargo 拼接 `LD_LIBRARY_PATH` 时用 `:` 作分隔符，直接报错
   `path segment contains separator ':'`。
   绕过方式：`CARGO_TARGET_DIR` 指到无冒号路径（本次用 `/tmp/opencode/rgpui-target`）。
2. **源码在 RDP 共享盘上**（`xrdp-chansrv`），`du -sh target` 都会挂住，
   编译产物必须放本地盘；读源码也偏慢，全量构建约 2.5 分钟。
3. **换行符噪音**：工作区 242 个文件因 CRLF↔LF 被标为 modified（29875 增 / 29875 删）。
   排查改动请用 `git diff --ignore-cr-at-eol`；提交在 Windows 端进行。
4. **会话为 xrdp 虚拟显示，没有 DRI3** → Vulkan 无法 present，屏幕上看不到任何画面。
   详见 §3.3。这是本机验证渲染时最大的障碍：**当前会话只能验证「窗口开没开」，
   验证不了「画面对不对」**。必须换 Wayland 或原生 Xorg 才能验证渲染结果。
5. **Mesa 是 Ubuntu 22.04 的 22.x**，EGL 只公布 `EGL_MESA_platform_xcb`，
   导致 wgpu-hal 的 GL 后端在本机不可用（§3.4），Vulkan/GL 两条路在本会话都不通。

---

## 六、验证方法备忘

```bash
# 检查某个示例解析出的后端 feature（修复前是 []）
CARGO_TARGET_DIR=/tmp/opencode/rgpui-target cargo tree -p hello_world -f "{p} [{f}]" | grep rgpui-linux

# 打开日志看 GPU 适配器探测（需示例临时接入 env_logger）
RUST_LOG=info,wgpu_hal=debug ./hello_world

# 确认窗口真的创建了（不是只看进程活着）
xprop -root _NET_CLIENT_LIST | grep -o "0x[0-9a-f]*"   # 逐个查 _NET_WM_PID
xwininfo -id <WID>                                      # Map State 应为 IsViewable

# 判断是否有托盘宿主
dbus-send --session --dest=org.freedesktop.DBus --type=method_call --print-reply \
  /org/freedesktop/DBus org.freedesktop.DBus.ListNames | grep -i statusnotifier
xprop -root _NET_SYSTEM_TRAY_S0                          # XEmbed 宿主
xdpyinfo | grep -ioE "DRI3|Present"                      # 软渲染呈现能力

# 确认当前 X server 到底有没有 DRI3（决定 Vulkan 能不能呈现）
xdpyinfo | sed -n '/number of extensions/,/^$/p'
strings /usr/lib/x86_64-linux-gnu/libvulkan_lvp.so | grep -i "DRI3"

# 抓窗口像素做量化分析（xwd 文件头 25 个 CARD32，数据在窗口名补 4 字节对齐之后）
xwd -id <WID> -out /tmp/w.xwd
# 用 python 解析 bits_per_pixel / bytes_per_line，统计非零像素数与 alpha 非零占比；
# 非零占比接近 0 且出现 01010101/02020202 递增值 = 未初始化显存，即从未 present。

# 换一个 X server 做对照（排除 WM/合成器因素）
Xephyr :11 -screen 700x700x32 &      # 无 WM、无合成器、扩展表不同
# 注意：Xephyr 里没有 EWMH，找窗口要用 xdotool search --pid <PID>，不是 _NET_CLIENT_LIST

# 检查 EGL client extensions 是否含 wgpu-hal 需要的 XCB 平台
RUST_LOG=debug ./hello_world 2>&1 | awk '/Client extensions: \[/,/\]/'
```

---

## 七、后续建议顺序

1. **换 Wayland 会话复测 `hello_world`** —— 这是判定 §3 结论的最终交叉验证，
   也是当前唯一能看到画面的路径。xrdp/X11 下已确认无法呈现（缺 DRI3）。
2. 实现 tray（StatusNotifierItem via zbus），同时补 `set_keep_alive_without_windows`
3. 把 §4.2 的三处假实现改成真实现，或至少改成返回明确的「不支持」错误
4. 按 §4.3 分诊表补 `on_global_hotkey`、`network_status` 等
5. 补 §4.4 中 X11/Wayland 都缺的 `request_attention`、`get_title`、`set_mouse_passthrough`
6. 文档补 Linux 构建前置包（§4.5）与日志入口（§4.6）
7. 记录 §3.4 的 wgpu-hal / 老 Mesa 兼容问题，评估向上游提 issue
8. 在 `docs/` 中写明 Linux 渲染前提：**X11 需要 DRI3，xrdp / 嵌套 X server 不支持，
   请用 Wayland 或原生 Xorg**
