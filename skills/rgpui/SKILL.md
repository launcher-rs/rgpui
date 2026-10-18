---
name: rgpui
description: rgpui 框架知识，涵盖操作/快捷键、异步/后台任务、上下文管理（App/Window/Context<T>/AsyncApp）、自定义元素（底层 Element trait）、实体状态管理、事件系统、焦点处理、全局状态、布局/样式（flexbox/CSS 风格）以及测试。在使用 rgpui 框架概念、构建 rgpui 应用程序或需要 rgpui 特定 API 和模式指导时使用。
---

## 导航

根据任务加载相关参考文件：

| 主题 | 文件 | 何时加载 |
|------|------|---------|
| 操作与快捷键 | [action.md](references/action.md) | `actions!`、`bind_keys`、`on_action`、`key_context` |
| 命令/Ribbon/视口/标尺 | [commands-ribbon-viewport.md](references/commands-ribbon-viewport.md) | `CommandRegistry`、`Ribbon` 功能区 + `KeyTips` 按键提示、`Viewport` 缩放平移、`ruler_ticks`/`align`（Office 基座） |
| 异步与后台任务 | [async.md](references/async.md) | `cx.spawn`、`background_spawn`、`Task`、异步 I/O |
| 上下文管理 | [context.md](references/context.md) | `App`、`Window`、`Context<T>`、`AsyncApp` |
| 自定义元素（底层） | [element.md](references/element.md) | `Element` trait、`request_layout`、`prepaint`、`paint` |
| 实体状态 | [entity.md](references/entity.md) | `Entity<T>`、`WeakEntity`、状态管理 |
| 事件与订阅 | [event.md](references/event.md) | `cx.emit`、`cx.subscribe`、`cx.observe` |
| 焦点与键盘导航 | [focus-handle.md](references/focus-handle.md) | `FocusHandle`、`track_focus`、Tab 导航 |
| 全局状态 | [global.md](references/global.md) | `Global` trait、`cx.set_global`、应用级配置 |
| 布局与样式 | [layout-style.md](references/layout-style.md) | `div()`、`h_flex()`、`v_flex()`、flexbox、overflow、定位 |
| ElementId | [element-id.md](references/element-id.md) | `ElementId`、`.id()`、唯一性规则、有状态元素 |
| 测试 | [test.md](references/test.md) | `#[rgpui::test]`、`TestAppContext`、`VisualTestContext` |
| 键映射（改键） | [keymap.md](references/keymap.md) | `Keymap`、`KeyBinding`、`KeyContext`、`load_keymap_json`、`HotkeyInput` |
| 主题 | [theme.md](references/theme.md) | `Theme`、`cx.theme()`、调色板、`ThemeRegistry`、热重载 |
| 国际化 | [i18n.md](references/i18n.md) | `I18nManager`、多语言、`translate_global`、`PluralRule` |
| 检查器 | [inspector.md](references/inspector.md) | `enable_default_inspector`、`toggle_inspector`、`inspector_tree_text`、崩溃快照 |
| 系统托盘 | [tray.md](references/tray.md) | `set_tray_menu`、`TrayMenuItem`、`on_tray_menu_action`、隐藏到托盘 |
| 滚动 | [scroll.md](references/scroll.md) | `Scrollable`、`Scrollbar`、`ScrollHandle`、滚动事件 |
| 基础元素 | [widgets.md](references/widgets.md) | `Button`、`Checkbox`、`Radio`、`Switch`、`Slider`、`Badge`、`Tag`、`Avatar`、`Typography`、`Icon`、`img`/`svg`/`canvas` |
| 表单 | [form.md](references/form.md) | `Form`、`Field`、`FieldBuilder`、`v_form`/`h_form` |
| 输入 | [input.md](references/input.md) | `InputState`、`Input`、`TextArea`、`NumberInput`、`MaskPattern` |
| 编辑器 | [editor.md](references/editor.md) | `EditorState`、`Editor`、LSP、高亮注册表、代码片段 |
| 菜单 | [menu.md](references/menu.md) | `PopupMenu`、`ContextMenu`、`DropdownMenu`、`Popover`、`AppMenuBar`、`Notification` |
| 对话框 | [dialog.md](references/dialog.md) | `Dialog`、`AlertDialog`、`DialogHeader/Content/Footer`、焦点陷阱 |
| 列表/表格/标签页 | [list-table-tabs.md](references/list-table-tabs.md) | `List`、`DataTable`、`VirtualList`、`TabBar`、`Accordion` |
| 扩展组件 | [components.md](references/components.md) | `Select`、`Combobox`、`DatePicker`、`ColorPicker`、`CommandPalette`、`DockArea`、`Sidebar`、`Tree` 等 30+ |
| 标题栏 | [title-bar.md](references/title-bar.md) | `TitleBar`、`TITLE_BAR_HEIGHT`、`WindowBorder`、窗口控制 |
| 动画/手势/物理 | [animation.md](references/animation.md) | `Animation`、`Transition`、`Spring`、`GestureDetector`、`ScrollPhysics` |
| 对话/AI-Chat | [chat.md](references/chat.md) | `Message`、`ChatView`、`Sender`、`ThoughtChain`、`MessageScroller`、`Bubble` |
| 平台抽象 | [platform.md](references/platform.md) | `Platform`、`PlatformWindow`、`App` 包装、托盘/权限/电源/热键 |
| Windows 平台 | [windows.md](references/windows.md) | 托盘、鼠标穿透、Mica、自启动、焦点窗口、`WM_USER` 消息 |
| 可选功能组件 | [feature-components.md](references/feature-components.md) | `charts`/`effects`/`qr-code`/`webview`/`dom-backend` feature |
| FPS 浮层 | [fps-hud.md](references/fps-hud.md) | `FpsHudState`、`FpsHud` |
| Markdown | [markdown.md](references/markdown.md) | `rgpui-markdown`：`Markdown`、`RichBlock`、`CodeBlock`、插件 |

## 扩展参考

深入了解主题，可使用额外的参考文件：

**Element trait：**
- [element-api.md](references/element-api.md) — 完整 API、hitbox 系统、事件处理
- [element-patterns.md](references/element-patterns.md) — 文本、交互、容器、组合模式
- [element-examples.md](references/element-examples.md) — 完整示例：文本、交互、复杂元素
- [element-best-practices.md](references/element-best-practices.md) — 性能、状态、常见陷阱
- [element-advanced.md](references/element-advanced.md) — 瀑布流/圆形布局、异步更新、虚拟列表

**实体管理：**
- [entity-api.md](references/entity-api.md) — 完整 Entity API、方法、生命周期
- [entity-patterns.md](references/entity-patterns.md) — 模型-视图、跨实体通信、观察者
- [entity-best-practices.md](references/entity-best-practices.md) — 内存、性能、生命周期
- [entity-advanced.md](references/entity-advanced.md) — 集合、注册表、防抖、状态机

**测试：**
- [test-examples.md](references/test-examples.md) — 测试示例和模式
- [test-reference.md](references/test-reference.md) — 完整测试 API 参考

**桌面增强与系统集成：**
- [keymap.md](references/keymap.md) — 键映射数据结构、`load_keymap_json` 改键、快捷键录制
- [theme.md](references/theme.md) — 主题调色板、切换、注册表热重载、语法高亮主题
- [i18n.md](references/i18n.md) — 多语言文本、占位符插值、复数、全局管理器
- [inspector.md](references/inspector.md) — 元素树检查器、AI 可读导出、崩溃快照、调试帧覆盖层
- [tray.md](references/tray.md) — 系统托盘图标/菜单/事件、隐藏到托盘
- [platform.md](references/platform.md) — `Platform`/`PlatformWindow` trait、`App` 包装、托盘/权限/电源/热键/文件对话框
- [windows.md](references/windows.md) — Windows 后端：托盘、鼠标穿透、Mica、自启动、焦点窗口、`WM_USER` 消息

**组件 API：**
- [scroll.md](references/scroll.md) — `Scrollable`、`Scrollbar`、`ScrollHandle`、滚动对齐
- [widgets.md](references/widgets.md) — 基础元素、Typography、图标/图片/SVG/画布
- [form.md](references/form.md) — `Form`/`Field` 与布局
- [input.md](references/input.md) — 单行/多行/数字/掩码输入
- [editor.md](references/editor.md) — 代码编辑器、LSP、高亮、片段、gutter、minimap、Vim
- [menu.md](references/menu.md) — 弹窗/上下文/下拉菜单、Popover、应用菜单栏、通知
- [dialog.md](references/dialog.md) — 对话框与确认框、焦点陷阱
- [list-table-tabs.md](references/list-table-tabs.md) — 列表/虚拟列表/数据表/标签页/手风琴
- [components.md](references/components.md) — 30+ 扩展组件索引与构造
- [title-bar.md](references/title-bar.md) — 自定义标题栏与窗口边框/内边距
- [animation.md](references/animation.md) — 动画/过渡/弹簧、手势检测、滚动物理
- [chat.md](references/chat.md) — AI-Chat 组件族
- [feature-components.md](references/feature-components.md) — `charts`/`effects`/`qr-code`/`webview`/`dom-backend`
- [fps-hud.md](references/fps-hud.md) — FPS/CPU/内存浮层
- [markdown.md](references/markdown.md) — `rgpui-markdown` 渲染与富文本块模型
