## todo

* 系统托盘 完成
* 鼠标穿透 完成
* 全局快捷键（Global Hotkeys）
* 原生通知（Native Notifications）
* Overlay 悬浮窗: 
  * always-on-top
  * overlay window
  * 透明层窗口
* 无窗口保持运行:
  ```
    cx.set_keep_alive_without_windows(true);
    ```
  
* 开机自启动
* 单实例锁（Single Instance）
* 聚焦窗口信息（Focused Window Info）
* 权限查询（Permission Query）
* 内置 Toast 系统

### AI原文
```
相对于官方的 [GPUI（Zed 的 gpui）](https://github.com/zed-industries/zed/tree/main/crates/gpui?utm_source=chatgpt.com)，[adabraka-gpui](https://docs.rs/adabraka-gpui?utm_source=chatgpt.com) 的核心定位是：

> “把原本偏编辑器/普通桌面 UI 的 GPUI，扩展成适合后台常驻、桌宠、菜单栏工具、悬浮层工具 的框架。”

它本质上是 GPUI 的 fork，但新增了很多“系统级桌面能力”。

---

# 一、最核心新增：后台常驻能力（Daemon Mode）

官方 GPUI 更偏：

* 普通 GUI 应用
* 编辑器类应用
* 窗口驱动

而 adabraka-gpui 新增：

* 无窗口常驻
* 托盘应用
* 热键后台监听
* 悬浮窗
* 桌宠类能力

这是最大的方向差异。 ([Docs.rs][1])

---

# 二、具体新增功能

下面是 adabraka-gpui 相对官方 GPUI 的新增能力。

---

## 1. 系统托盘（System Tray）

这是官方 GPUI 没有的。

新增：

* 托盘图标
* 托盘 tooltip
* 托盘菜单
* 菜单回调

示例 API：

```rust
cx.set_tray_tooltip("My App");

cx.set_tray_menu(vec![
    TrayMenuItem::Action {
        label: "Settings".into(),
        id: "settings".into(),
    },
]);
```

适合：

* 输入法
* AI 助手
* 桌宠
* 后台工具
* Claude Desktop 类程序

([Docs.rs][1])

---

## 2. 全局快捷键（Global Hotkeys）

官方 GPUI：

* 只能窗口内快捷键
* 类似 Web 的 focus shortcut

adabraka-gpui：

* 系统级热键
* 即使窗口不聚焦也能触发

支持：

* Windows `RegisterHotKey`
* X11 `XGrabKey`
* macOS 原生热键

你之前做的：

```rust
GlobalHotKeyEvent
```

其实就是这部分能力。

非常适合：

* AI 呼出
* 截图工具
* 桌宠控制
* Overlay 开关

([Docs.rs][1])

---

## 3. 原生通知（Native Notifications）

官方 GPUI 没有系统通知封装。

adabraka-gpui 增加：

* Windows 通知
* macOS Notification
* Linux notify-rust

适合：

* AI 回复提醒
* 后台任务通知
* 消息提醒

([Docs.rs][1])

---

## 4. Overlay 悬浮窗

这是桌宠最关键的能力之一。

新增：

* always-on-top
* overlay window
* 透明层窗口

类似：

* FPS overlay
* 桌面宠物
* AI assistant overlay
* 屏幕翻译层

官方 GPUI 原本并不强调这个方向。

([Docs.rs][1])

---

## 5. Click-through 鼠标穿透

你最近一直在研究的：

```rust
WS_EX_TRANSPARENT
```

这个就是 adabraka-gpui 新增重点。

支持：

* 鼠标穿透
* 点击透传
* overlay 不阻挡操作

官方 GPUI 基本没有完整封装。

adabraka-gpui 明显是为：

* 桌宠
* Overlay
* HUD

专门做过适配。

([Docs.rs][1])

---

## 6. 无窗口保持运行

官方 GPUI：

* 关闭窗口 ≈ 退出应用

adabraka-gpui：

```rust
cx.set_keep_alive_without_windows(true);
```

即：

* 没有窗口
* 进程仍存活
* 托盘仍可运行

这是典型：

* Tray App
* Menubar App
* 后台 AI Agent

的需求。

([Docs.rs][1])

---

## 7. 开机自启动（Auto Launch）

新增：

* Windows Registry
* macOS SMAppService
* Linux XDG autostart

官方 GPUI 没有。

([Docs.rs][1])

---

## 8. 单实例锁（Single Instance）

新增：

* Unix socket
* Windows named mutex

防止：

* 启动多个桌宠
* 多个托盘程序

类似：

```txt
QQ 已经运行
微信已打开
```

([Docs.rs][1])

---

## 9. 聚焦窗口信息（Focused Window Info）

能获取：

* 当前活动窗口
* 用户正在操作哪个程序

适合：

* AI 助手
* 自动翻译
* OCR
* 上下文 AI

例如：

```txt
当前用户正在 VSCode
当前焦点窗口是 Chrome
```

官方 GPUI 没有这种系统集成。

([Docs.rs][1])

---

## 10. 权限查询（Permission Query）

尤其 macOS：

* Accessibility
* Microphone

官方 GPUI 没有。

([Docs.rs][1])

---

## 11. 内置 Toast 系统

新增：

* stackable toast
* auto dismiss

类似：

```txt
保存成功
AI 已复制
截图完成
```

官方 GPUI 只有底层能力，没有完整 toast 系统。

([Docs.rs][1])

---

# 三、它本质上在向什么方向演化？

官方 GPUI：

更像：

```txt
高性能 GUI Framework
```

目标：

* 编辑器
* IDE
* 专业桌面应用

---

adabraka-gpui：

更像：

```txt
Electron + Tauri + Overlay SDK + Tray SDK
```

目标：

* 桌宠
* AI 助手
* Menubar App
* Tray App
* Overlay
* HUD
* 系统增强工具

---

# 四、为什么你最近的问题都和它有关

你最近研究：

* 鼠标穿透
* 透明窗口
* overlay
* hotkey
* 桌宠
* daemon
* tray
* gpui

其实正是 adabraka-gpui 的设计方向。

它基本已经：

> 把“桌宠/悬浮 AI 工具”需要的系统能力补齐了。

---

# 五、它和官方 GPUI 的关系

当前：

```txt
adabraka-gpui = fork(zed gpui)
```

即：

* 保留官方 GPUI 渲染架构
* 保留 entity/state 模型
* 保留 GPU 渲染
* 保留 declarative UI

但增加：

* OS integration
* desktop utility capability

---

# 六、你可以把它理解成

| 框架            | 定位            |
| ------------- | ------------- |
| 官方 GPUI       | 编辑器 UI 引擎     |
| adabraka-gpui | 系统级桌面工具 UI 引擎 |

---

# 七、对于 Rust 桌宠来说它为什么特别合适

因为桌宠真正难的不是 UI。

而是：

* 透明窗
* 鼠标穿透
* 全局热键
* overlay
* 托盘
* 后台运行
* 单实例
* 系统通知

这些 adabraka-gpui 已经补了。

```