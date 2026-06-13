---
name: rgpui
description: rgpui 框架知识，涵盖操作/快捷键、异步/后台任务、上下文管理（App/Window/Context<T>/AsyncApp）、自定义元素（底层 Element trait）、实体状态管理、事件系统、焦点处理、全局状态、布局/样式（flexbox/CSS 风格）以及测试。在使用 rgpui 框架概念、构建 rgpui 应用程序或需要 rgpui 特定 API 和模式指导时使用。
---

## 导航

根据任务加载相关参考文件：

| 主题 | 文件 | 何时加载 |
|------|------|---------|
| 操作与快捷键 | [action.md](references/action.md) | `actions!`、`bind_keys`、`on_action`、`key_context` |
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
