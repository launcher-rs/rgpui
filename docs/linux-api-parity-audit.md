# Linux API 契约对等性审计（第二轮）

> 分支：`fix/linux-api-parity`
> 审计日期：2026-11-26
> 范围：**元素层 / 核心层跨平台 API 在 Linux 的静默失效**、后端能力不对等、文档与守卫缺口
> 上一轮：`docs/linux-platform-audit.md`（渲染、tray、feature 门控、`Platform` 假实现、窗口层方法缺失）

---

## 一、摘要

### 触发事件

RuTerm（自绘标题栏）在 Linux/Wayland 上「拖拽、最小化、最大化、关闭全部点不动」。
定位结论：它用的 `WindowControlArea` 看起来是跨平台 API，实际只在 Windows 后端接到了
`WM_NCHITTEST`；Wayland / X11 / headless 三个后端的 `on_hit_test_window_control`
都是**空函数体**，回调被直接丢弃。框架自带的 `TitleBar` 靠 `.when(is_windows, ...)`
绕开，但这个约束只存在于框架内部，API 层没有任何文档或编译期提示。

### 三层根因

1. **跨平台 API 的 Linux 静默 no-op**：`WindowControlArea` / `on_hit_test_window_control`
   命名与签名都不带平台限定，Linux 实现丢弃回调，调用方拿到的是「注册成功」的错觉。
2. **装饰模式回退造成「两边都不管」**：请求服务端装饰在 Wayland 上静默回退为客户端
   装饰，而应用侧又以为系统会处理拖拽/按钮，结果是系统边框没有、应用处理也没有。
3. **框架已有正确做法但没有外化**：`TitleBar::window_options()` + Linux 分支的
   `on_click` / `start_window_move` / `window_border` 是既定解法，普通调用者无从得知。

### 与上一轮审计的分工

`docs/linux-platform-audit.md` 已覆盖并大部修复：`Platform` 假实现（通知/热键/权限/电源/
网络状态）、窗口层方法缺失（titlebar 可见性、鼠标穿透、独占区域、离屏渲染）、渲染链路、
feature 未开启、托盘。**本轮不重复这些**，聚焦上一轮没有系统审计的维度：

- 元素层 / 核心层 API（`WindowControlArea`、`WindowOptions` 字段、能力查询）的契约泄漏；
- Wayland 与 X11 后端的能力不对等（一个实现、一个空/撒谎）；
- 「返回成功 / 返回 true 但什么都没做」的**能力撒谎**；
- 启动期硬 panic 与 feature 组合不一致；
- 文档、示例与自动化守卫的缺口。

> ⚠️ 上一轮文档自身也有过期项：`docs/linux-platform-audit.md` §4.3 列的
> `show_dialog` / `on_media_key_event` / `read_write_find_pasteboard` 等已在 1.5.0
> 删除（`CHANGELOG.md:463-466`）。读那份清单时以当前 `platform.rs` 为准，本清单第 38 项已登记。

### 严重度口径

| 级别 | 含义 |
|------|------|
| P0 | 启动崩溃、核心交互完全失效、能力明确撒谎且无提示 |
| P1 | 功能在某一后端静默缺失或行为不对等，用户可感知但不知原因 |
| P2 | 降级路径只写日志、文档缺失、边缘场景行为不一致 |
| P3 | 日志误导、非关键细节、多 seat / 输入法样式等长尾 |

---

## 二、问题清单

> 状态列：`新` = 本轮首次记录；`已核实` = 已对照源码确认；`待验证` = 仅有扫描结果，
> 修复前需再核对。行号以审计当时的 `main`（`37ee741d`）为准。

### P0

| # | 问题 | 位置 | 证据 / 影响 | 状态 |
|---|------|------|-------------|------|
| 1 | `WindowControlArea` / `on_hit_test_window_control` 在 Linux 三个后端全部空实现 | `crates/rgpui/src/window.rs:2001`；`wayland/window.rs:1880`、`x11/window.rs:2242`、`headless/window.rs:211`；对照 `rgpui-windows/src/window.rs:1128` | 自绘标题栏的拖拽区与三个按钮命中测试永远返回不了区域；API 名跨平台、语义仅 Windows | 已核实 |
| 2 | `current_platform()` 按 feature 生成匹配分支，兜底 `unreachable!()` | `crates/rgpui-linux/src/linux.rs:58-83` | `rgpui` 与 `rgpui-linux` 的 wayland/x11 feature 不一致时启动直接 panic | 已核实 |
| 3 | Wayland 宣称支持屏幕捕获，实际 `screen_capture_sources()` 直接返回「not yet implemented」 | `crates/rgpui-linux/src/linux/platform.rs:75-77`；`wayland/client.rs:838-855` | `App::is_screen_capture_supported()` 暴露给 UI 为 true，点击后列表空/报错 | 已核实 |
| 4 | Wayland 启动期多处硬 `unwrap!` / 低版本 `panic!` | `wayland/client.rs:649`、`562/564`、`530-557` | 无 `wl_seat`、老合成器、连接失败时直接崩溃，不回退其它后端或 headless | 待验证 |
| 5 | `on_open_urls` / `on_reopen` 回调在 Linux 只存不派发 | `crates/rgpui-linux/src/linux/platform.rs:593-596`、`753-756`；对照 macOS `platform.rs:1471-1492`、`971+1446` | 注册 URL scheme 返回 `Ok`，但以该 scheme 启动应用时回调永不触发；Dock 重开事件同理 | 已核实（全局无派发点） |
| 6 | `WindowOptions::is_movable` / `is_resizable` / `is_minimizable` 在 Linux 建窗时从不读取 | 定义 `platform.rs:2210-2217`；核心转发 `window.rs:1522-1526`、`2107-2108`；X11 `window.rs:601/731/961/976/1037/1144`、Wayland `window.rs:285/538/746` 均未读；对照 Windows `rgpui-windows/src/window.rs:508-521` 真设 WS 位 | 设 `is_minimizable: false` 的窗口在 Linux 仍显示并响应最小化/最大化按钮，直接违反应用意图；`is_movable: false` 完全无效 | 已核实 |
| 7 | 装饰模式双向静默回退：Wayland `Server→Client`、X11 `Client→Server` | `wayland/window.rs:2118-2126`（只 `log::info!`）；`x11/window.rs:2440-2447`；`window_decorations()` 在 X11 恒 `Server`（`:2364-2367`） | 这正是 RuTerm 踩坑的第二层根因：请求与实际生效不一致，调用方查不到真实值；X11 无合成器时「原生框架 + 自绘标题栏」重复 | 已核实 |
| 8 | README 虚报「媒体键、生物识别」支持，对应 API 已在 1.5.0 删除 | `readme.md:85`；`CHANGELOG.md:463-466` | 明面上的功能虚报，Linux 读者会去找根本不存在的接口 | 已核实 |
| 9 | README / 入门文档宣称 Linux 支持，却不提 feature 门控与构建前置 | `readme.md:8/96/144-151`；`docs/rgpui-book/01-getting-started.md:8/20-25`；根 `Cargo.toml:30` + `crates/rgpui/Cargo.toml:47/49` | 直接依赖 `rgpui` 的下游会「编译成功、无窗口、零报错」（静默 Headless）；缺 glib/gtk/webkit 时 workspace 构建失败 | 已核实 |

### P1

| # | 问题 | 位置 | 证据 / 影响 | 状态 |
|---|------|------|-------------|------|
| 10 | Wayland `primary_display` / `window_stack` 恒 `None`，X11 已实现 | `wayland/client.rs:835-837`、`1082-1084`；`x11/client.rs:1631-1640`、`1944-1984` | 多屏定位、窗口栈枚举类功能在 Wayland 静默失效 | 待验证 |
| 11 | Wayland 权限查询在 portal 有 `GlobalShortcuts` 时返回 `NotDetermined`，但全局热键注册恒失败 | `permissions.rs:126-134`；`wayland/client.rs:530-557`、`platform.rs:112-118` | UI 显示「可申请」，注册总失败或注册后永不触发 | 待验证 |
| 12 | X11 不处理光标主题/大小变更事件，Wayland 会更新 | `x11/client.rs:551-553`；`wayland/client.rs:708-713` | 运行时切主题后应用内光标不更新，后端行为不对等 | 待验证 |
| 13 | X11 `request_frame` 缺失，纯定时动画卡住；Wayland 靠 `commit` 能推进 | `crates/rgpui/src/platform.rs:1404-1410`；`x11/window.rs` 的 `PlatformWindow` 实现 | 同一段动画代码在两个后端表现不同 | 待验证 |
| 14 | `focused_window` 只要编了 x11 feature 就无条件连 X11，即使在 Wayland 会话 | `focused_window.rs:17-22` | Wayland + XWayland 下返回 XWayland 客户端信息，不是当前原生窗口 | 待验证 |
| 15 | X11 缩放因子不读 XSettings，RandR 失败直接硬编码 1.0 | `x11/client.rs:2772`、`2766` | XFCE / GNOME X11 下 UI 尺寸偏小或偏大 | 待验证 |
| 16 | 剪贴板交接给 manager 超时只 warn + `Ok(())` | `x11/clipboard.rs:852-857` | 复制后退出应用，粘贴板变空，调用方无错误 | 待验证 |
| 17 | `rgpui-platform` 的 wayland/x11 feature 在 Linux 上是空操作（依赖硬编码双后端） | `crates/rgpui-platform/Cargo.toml:25-26`、`43-44` | 用户以为关掉了某后端，实际仍编进来；或以为选了 wayland 仍走 X11 | 待验证 |
| 18 | 一批 `Platform` 方法持续空实现且无能力查询 | `platform.rs:829-833`、`952`、`802-806`、`824-827`、`544-551` | 最近文档 / dock 跳转列表 / 热状态等 UI 静默无效果 | 待验证 |
| 19 | X11 `window_controls()` 未覆盖，走默认「四项全支持」 | `crates/rgpui/src/platform.rs:940-949`（`WindowControls::default()` 全 `true`）；X11 `window.rs` 无该方法；Wayland `window.rs:2130` 有真实能力上报 | `TitleBar` 据该值绘制按钮；平铺 WM / 不支持 min/max 的 WM 上会画出点了没反应的按钮（与本轮 #1 同类） | 已核实 |
| 20 | `should_auto_hide_scrollbars` 在 Linux 永远 `false`（字段无写入点） | `crates/rgpui-linux/src/linux/platform.rs:190/245`、`847-848`；对照 Windows `platform.rs:962/1628` | 主题 `scrollbar_show` 恒为 `Hover`，不跟随 GNOME/KDE 系统设置 | 待验证 |
| 21 | `thermal_state()` 恒 `Nominal`，`on_thermal_state_change` 空 | `crates/rgpui-linux/src/linux/platform.rs:457-461`；对照 macOS `platform.rs:991-1005` | 性能自适应应用永不降级；调用方无法区分「真正常」与「不支持」 | 待验证 |
| 22 | Wayland `set_position()` 只改内部 geometry，合成器可不尊重 | `wayland/window.rs:1681-1705`（源码注释已承认） | 记忆窗口位置 / 多窗口布局在 Wayland 上出现偏差，API 无能力提示 | 待验证 |
| 23 | Wayland `focused_window_info()` 恒 `None` | `focused_window.rs:20-24`；对照 X11 同文件 `:26+` | 无法区分「Wayland 不支持」与「当前无焦点窗口」 | 待验证 |
| 24 | `show_window_menu` 注释称「Wayland 和 X11」，实际有强前置条件 | `window.rs:2900-2903`；X11 `window.rs:2315-2348` 只认 GTK 的 `_GTK_SHOW_WINDOW_MENU`；Wayland `window.rs:2009-2019` 需 MousePress serial | 右键标题栏在 KDE/i3/多数 Wayland 合成器无菜单，且无法区分「不支持」与「没点」 | 待验证 |
| 25 | `Dialog` 遮罩层也挂了 `window_control_area(Drag)`，Linux 无效 | `dialog/dialog.rs:486`（与 #1 同源） | Windows 上点遮罩会被系统当拖拽，Linux 只走点击关闭，行为分叉 | 已核实 |
| 26 | `appears_transparent` / `traffic_light_position` / `app_owns_titlebar_drag` 在 Linux 被丢弃，且后者文档误写「macOS 专用」 | `platform.rs:2395-2400`、`2242-2244`；X11 `window.rs:731-754`、Wayland `window.rs:538` 只读 `title`；`app_owns_titlebar_drag` 实际 Windows 也用（`rgpui-windows/src/window.rs:503`） | 照抄 `TitleBar::window_options()` 的调用者带着三个 Linux 无意义字段，且文档口径自相矛盾 | 已核实 |
| 27 | 一批 macOS-only API 无平台标注：`set_window_edited`、`show_character_palette`、`titlebar_double_click`、tab 系列、`set_text_content_type` | `window.rs:2992-3014`、`platform.rs:1276-1317`、`1429`；对照 `set_document_path`（`window.rs:2997` 已写 macOS） | 调用方以为平台级语义已生效，Linux 全是空操作 | 已核实 |
| 28 | `Tray::visible` 三个平台都不读 | `system/tray.rs:59-60`、`125-129`；Linux `platform.rs:382-405`、Windows `platform.rs:726-754`；SNI `tray_sni.rs:282/486` 恒 `true` | 设 `visible: false` 仍显示托盘图标 | 待验证 |
| 29 | `Input::content_type` 的「密码管理器/自动填充」原生部分只实现了一半 | `input_ui/input/widget.rs:180-188`；`content_type.rs:152-171` 的 `#[cfg(not(macos))]` 分支是 `let _ = (window, content_type);` | 无障碍 Role 生效，但原生自动填充/输入法提示在 Linux 不发生 | 待验证 |
| 30 | `set_menus` 在 Linux 只存不显示，且与自绘 `AppMenuBar` 数据源不同步 | `app.rs:2612-2621`；`linux/platform.rs:814-822`；`menu/app_menu_bar.rs:47-63` 读 `GlobalState::app_menus()` | 只调 `cx.set_menus(...)` 在 Linux 看不到任何菜单栏 | 待验证 |
| 31 | `gpu_specs` 文档与实现相反 | `window.rs:6633-6637` 写「Mac 和 Windows 上返回 None」，但三个 Linux 后端都实现了（`x11/window.rs:2491`、`wayland/window.rs:2151`、`headless/window.rs:228`） | 文档误导，应用可能因此跳过 Linux 的 GPU 信息展示 | 已核实 |
| 32 | `map_window` 在 Wayland 走默认 `Ok(())`，文档未说明 | `platform.rs:1343-1346`；Wayland 无 override（建窗时 `surface.commit()` 已隐式映射） | 调用方无法区分「已映射」与「平台隐式完成」，排障时误导 | 已核实 |
| 33 | `Platform` 默认实现 rustdoc 把「返回 `Ok(())` / 空值」写成成功 —— 1.4.1 假实现模式残留 | `platform.rs:514-526`、`528-535`（`register_global_hotkey` / `show_notification` / `set_auto_launch` 默认 `Ok(())` 但什么都没做） | 任何未 override 的后端都会骗调用方；这是本轮一大批问题的**系统性根源** | 已核实 |
| 34 | 技能索引没有 Linux 入口，只有 Windows 平台条目 | `skills/rgpui/SKILL.md:6-54/77-84`；`readme.md:144-151` 未链接 `docs/linux-platform-audit.md` | 1600+ 行的 Linux 审计文档对加载技能的调用者完全不可发现，**放大本清单每一条** | 已核实 |
| 35 | 托盘技能把 `set_tray_panel_mode` / `tray_icon_bounds` 写成可用 | `skills/rgpui/references/tray.md:100-102/162-163`；`app.rs:2713-2734`；Linux `platform.rs:433-441`（Ubuntu host 忽略）、`get_tray_icon_bounds` 恒 `None` | 「托盘图标下方定位窗口」在 Linux 直接失效 | 已核实 |
| 36 | `TrayIconEvent` 文档按 Windows/macOS 语义写，Ubuntu 主机实际不同 | `tray.md:38-42/93`；`system/tray.rs:9-16`；SNI `tray_sni.rs:120/129` | Ubuntu/appindicators：左键打开菜单、**双击**才触发 `Activate`、右键永不产出 `LeftClick` | 已核实 |
| 37 | `title-bar.md` 未标 `on_close_window` 为 Linux-only，且示例指向不存在的 bin | `title-bar.md:49/37`；`title_bar.rs:85-95` 明写 Linux-only；真实示例是 `window_showcase --bin custom_title_bar` | 标题栏入门者照抄无效 API / 跑不存在的命令 | 已核实 |
| 38 | 剪贴板技能表未标 `find_pasteboard` 仅 macOS、`primary` 仅 Linux/FreeBSD | `platform.md:48`；`app.rs:1541-1570` 的 cfg 门控 | Linux 上调用 `find_pasteboard` 直接编译失败 | 已核实 |
| 39 | `window_decorations` 文档写「仅 Wayland」，实际 X11 已实现且 `TitleBar` 在 Linux 默认开 CSD | `platform.rs:2232-2234`；`window.rs:2477-2479`；X11 `window.rs:2437-2467`；`title_bar.rs:79-81` | 文档口径与实现不一致，调用者误判适用范围 | 已核实 |
| 40 | `docs/linux-platform-audit.md` 自身「已知问题清单」过期 | 该文件 `:3`、`:922-949` 列了 `show_dialog` / `on_media_key_event` / `read_write_find_pasteboard` 等 | 其中多项已在 1.5.0 删除（`CHANGELOG.md:463-466`），照它排查会去找不存在的接口 | 已核实 |
| 41 | `docs/rgpui-book` 窗口管理 / 入门 / 托盘三章 API 全面过期 | `01-getting-started.md:8/20-25/31-45`、`07-window-management.md:7-11/17-45/152-166/203-243` | `Window::default()` / `App::new()` / `gpui::blue()` 等当前 API 下任何平台都编译不过 | 已核实 |

### P2 / P3（静默降级与日志缺失）

> 这一类的共同形态：`log::debug!/warn!` 之后 `return`，调用方拿到 `Ok(())` / 默认值，
> 只能看日志才知道功能丢了。默认日志级别是 `error`，所以多数用户完全看不见。

| # | 问题 | 位置 | 状态 |
|---|------|------|------|
| 42 | portal Settings 连接失败后整个 future 静默结束，外观/光标主题/减少动效全留默认值 | `xdg_desktop_portal.rs:44-47`、`177-186` | 待验证 |
| 43 | reduce-motion 变更只更新 `common`，已有窗口不重读，需重启应用 | `xdg_desktop_portal.rs:258-259`；`wayland/client.rs:708-713`、`x11/client.rs:548-550` | 待验证 |
| 44 | Wayland 光标主题加载失败直接 `return`，指针完全消失（无默认兜底） | `wayland/cursor.rs:118-121` | 待验证 |
| 45 | 托盘 watcher 初始注册失败只 `debug!`，后续只靠命令流重试，shell 重启后可能永不注册 | `tray_sni.rs:396-416` | 待验证 |
| 46 | 电源事件监听失败后只 warn 且 `power_listener_started` 仍置 true，永不重试 | `platform.rs:266-283` | 待验证 |
| 47 | `open_uri` / `reveal_path` 全失败只 `log::error!`，函数返回 `()` | `platform.rs:1027-1071`、`1080-1098` | 待验证 |
| 48 | `set_exclusive_zone` 对非 layer-shell 窗口只 `debug!` 后返回 | `wayland/window.rs:2078-2097` | 待验证 |
| 49 | `headless/window.rs` 的 frame / close / input 回调全空，等一帧的 Future 永不 resolve | `headless/window.rs:193-216` | 待验证 |
| 50 | X11 无效 `GPUI_X11_SCALE_FACTOR` 环境变量直接 panic | `x11/client.rs:2732`、`2740` | 待验证 |
| 51 | `auto_launch` 硬编码 `$HOME/.config/autostart`，忽略 `XDG_CONFIG_HOME` | `auto_launch.rs:79-82` | 待验证 |
| 52 | 剪贴板不支持的转换目标 / 缺 selection owner 时只 warn + `return` | `x11/clipboard.rs:362/379`；`wayland/client.rs:1022-1049` | 待验证 |
| 53 | X11 光标加载失败日志分支条件写反（误导排查方向） | `x11/client.rs:2229-2236` | 已核实 |
| 54 | Wayland `wl_pointer::Axis` 未知值 `unreachable!()` | `wayland/client.rs:2166/2193/2209/2221` | 待验证 |
| 55 | XIM preedit 的下划线/高亮/选中反馈属性被忽略，只传纯文本 | `x11/xim_handler.rs:141` | 待验证 |
| 56 | 多 seat 只取第一个，`// TODO: Multi seat support` | `wayland/client.rs:236`、`576-582`、`649` | 待验证 |
| 57 | `examples/README.md:59` 把 `set_menus` 简写为「原生菜单」 | 示例自身已如实说明 Windows/Linux 只存储不渲染，README 没同步 | 待验证 |
| 58 | `window_showcase` 的 Hide Titlebar 按钮在 Wayland / CSD 下必然无效 | `examples/window_showcase/src/bin/window.rs:254-267`；`TitleBar::window_options()` 默认 CSD | 待验证 |
| 59 | `examples/README.md:143` 把 layer_shell 写成「Wayland layer shell」 | 实际 X11 走 EWMH DOCK + strut（`examples/layer_shell/README.md:3-5`） | 已核实 |
| 60 | 透明窗口示例无 X11/软件渲染环境限制提示 | `window_showcase/README.md:13`；`docs/linux-platform-audit.md:588-589` | 待验证 |
| 61 | `platform.md:185-206` 能力矩阵只列 8 项，给人「其余已对齐」错觉 | 恰好漏掉本清单的 `show_native_menu`、`prompt`、托盘 bounds/panel mode、`request_attention`、`window_control_area` 等 | 已核实 |
| 62 | `start_window_move` / `start_window_resize` 文档未提 Wayland 需 MousePress serial | `platform.rs:1331-1334`、`window.rs:2925-2931`；`wayland/window.rs:2022-2038` | 待验证 |

---

## 三、系统性根因

### 3.1 跨平台 API 没有平台契约

`WindowControlArea` 的名字、枚举变体、rustdoc 都不带「仅 Windows 有效」的语义。
框架内部知道（自绘 `TitleBar` 用 `.when(is_windows, ...)` 限定），但这是**实现细节**，
不是 API 契约。调用者只能读框架源码才不会踩坑。

### 3.2 「返回成功但什么都没做」比「没实现」更危险

本轮至少三类：
- 回调被丢弃（`on_hit_test_window_control`）；
- 能力查询返回 `true` / 空实现返回 `Ok(())`（screen capture、剪贴板交接、通知早期版本）；
- 只 `log::debug!/warn!` 就 `return`（portal 设置、电源抑制、托盘注册重试）。

调用方拿到的是成功信号，无法区分「做成了」和「这个平台不支持」。

### 3.3 feature 组合缺乏编译期约束

`rgpui` 与 `rgpui-linux` 两侧的 wayland/x11 feature 可以不一致，运行时靠字符串匹配选后端，
不匹配就 `unreachable!()`。这是把编译期能发现的问题推迟到运行时崩溃。

### 3.4 没有自动化守卫

`crates/rgpui/tests/guards.rs` 现有 7 条守卫，覆盖「平台方法必须有 App 包装 + 真实调用点」、
「生产代码禁止 `todo!`/`unimplemented!`」等，但**没有**任何守卫检查「某个 trait 方法在
Linux 后端是否只是空实现」。新出现的静默 no-op 不会被 CI 拦住。

---

## 四、修复计划

> 逐项推进，每项包含：代码修复 + 验证方式 + 必要的文档/技能同步。状态随修复更新。

| # | 问题 | 修复方案 | 验证方式 | 状态 |
|---|------|----------|----------|------|
| 1 | `WindowControlArea` Linux 静默 no-op | ① rustdoc 明确「仅 Windows 有效」；② Linux 后端 debug 下记 warn，避免静默；③ 文档给出 `TitleBar` / `start_window_move` 的正确做法 | 文档单测 + Linux 手动验证自绘标题栏 | 待开始 |
| 2 | `current_platform()` `unreachable!` | 按实际编译后端 fallback，返回可读错误而不是 panic | feature 组合矩阵 + 单元测试 | 待开始 |
| 3 | Wayland 屏幕撒谎 | 覆盖 `is_screen_capture_supported() -> false`（实现 portal 前） | 现有能力测试 | 待开始 |
| 4 | Wayland 启动期硬 panic | 连接/绑定失败改 `Result` 并逐后端 fallback | 无 seat / 老合成器模拟 | 待开始 |
| 5 | URL scheme / 重开回调不派发 | Linux 启动路径解析 argv 并调用 `callbacks.open_urls`；桌面入口激活补 `on_reopen` | 带 scheme 启动示例 + 测试 | 待开始 |
| 6 | `is_movable` / `is_resizable` / `is_minimizable` 被丢弃 | X11 用 `_MOTIF_WM_HINTS` functions 位 + min/max size hints 落地；`window_controls()` 反映真实值；Wayland 文档化尽力而为 | 建窗后读属性 + 按钮可见性验证 | 待开始 |
| 7 | 装饰模式双向静默回退 | `window_decorations()` 暴露协商后的真实值；回退日志从 info 提到 warn；同步包装层文档 | Wayland/X11 两种组合的窗口外观验证 | 待开始 |
| … | （其余见第二节） | | | |

---

## 五、附：上一轮已修问题的回归提示

上一轮审计中有几类问题反复出现，修复本轮问题时要避免重蹈覆辙：

- **只修 Linux 不改 API 形状**：`keep_alive_without_windows` 的教训 —— 平台侧实现了但核心层
  没人读，等于没修。新能力要同时有 `App` 包装 + 真实调用点。
- **静默降级**：`show_notification` 早期只 `Ok(())`，后来才改 portal；同类问题要一次给调用方
  明确信号。
- **误诊为环境问题**：画面不上屏曾被误判为「xrdp 无 DRI3」，真相是 rgpui 自己漏了 `present()`。
  遇到「某平台就是不行」的结论，先核对代码路径。
