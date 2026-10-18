# rgpui 示例索引

本目录是 rgpui 的示例集合。为了便于查找，**常用/高频功能都是独立命名的 binary**，
统一放在 `showcase` 包里，一个功能一个 bin：

```bash
cargo run -p showcase --bin <名称>
```

少数重依赖或需要独立平台的示例保留为独立 crate（见文末）。

> 全部的 `showcase` bin 清单也可以随时查看：`cargo run -p showcase --bin` 或
> 打开 `examples/showcase/Cargo.toml`。

## showcase（一个功能一个 bin）

### 快速上手 / 通用

| bin | 说明 | 运行 |
|-----|------|------|
| `hello_world` | 最小可运行窗口 | `cargo run -p showcase --bin hello_world` |
| `a11y` | 无障碍（AccessKit）标注 | `cargo run -p showcase --bin a11y` |
| `animation` | 动画 / 过渡 | `cargo run -p showcase --bin animation` |
| `theme` | 主题（亮/暗、自定义配色） | `cargo run -p showcase --bin theme` |
| `daemon_app` | 常驻后台应用骨架 | `cargo run -p showcase --bin daemon_app` |
| `active_state_bug` | active 状态回归复现 | `cargo run -p showcase --bin active_state_bug` |

### 组件

| bin | 说明 | 运行 |
|-----|------|------|
| `component_gallery` | 组件总览（各类基础组件） | `cargo run -p showcase --bin component_gallery` |
| `form` | 表单 `Form`/`Field` 与校验 | `cargo run -p showcase --bin form` |
| `input` | 输入组件（Input/TextArea 等） | `cargo run -p showcase --bin input` |
| `extended_components` | 扩展组件集合 | `cargo run -p showcase --bin extended_components` |
| `dialog` | 对话框 / 焦点陷阱 | `cargo run -p showcase --bin dialog` |
| `sidebar` | 侧边栏 | `cargo run -p showcase --bin sidebar` |
| `scrollable` | 滚动 / 滚动条 | `cargo run -p showcase --bin scrollable` |
| `grid_layout` | 网格布局 | `cargo run -p showcase --bin grid_layout` |
| `gradient` | 渐变填充 | `cargo run -p showcase --bin gradient` |
| `pattern` | 图案 / 纹理绘制 | `cargo run -p showcase --bin pattern` |
| `painting` | 自定义绘制 | `cargo run -p showcase --bin painting` |
| `charts` | 图表（Bar/Line/Pie 等，需 feature） | `cargo run -p showcase --bin charts --features charts` |
| `effects` | 封装动效（Aurora/Confetti 等，需 feature） | `cargo run -p showcase --bin effects --features effects` |
| `qr_code` | 二维码（需 feature） | `cargo run -p showcase --bin qr_code --features qr-code` |

### 交互与事件

| bin | 说明 | 运行 |
|-----|------|------|
| `drag_drop` | 拖拽 | `cargo run -p showcase --bin drag_drop` |
| `focus_visible` | 焦点可见性（focus ring） | `cargo run -p showcase --bin focus_visible` |
| `tab_stop` | Tab 焦点遍历 | `cargo run -p showcase --bin tab_stop` |
| `mouse_pressure` | 鼠标压力/倾斜输入 | `cargo run -p showcase --bin mouse_pressure` |
| `move_entity_between_windows` | 实体跨窗口迁移 | `cargo run -p showcase --bin move_entity_between_windows` |
| `on_window_close_quit` | 关窗退出策略 | `cargo run -p showcase --bin on_window_close_quit` |
| `ownership_post` | 实体所有权传递 | `cargo run -p showcase --bin ownership_post` |
| `set_menus` | 原生菜单 | `cargo run -p showcase --bin set_menus` |
| `native_notification` | 系统通知 | `cargo run -p showcase --bin native_notification` |

### 性能与调试

| bin | 说明 | 运行 |
|-----|------|------|
| `fps_hud` | FPS / 性能 HUD 叠层 | `cargo run -p showcase --bin fps_hud` |
| `input_latency` | 输入延迟直方图（需 feature） | `cargo run -p showcase --bin input_latency --features input-latency` |
| `paths_bench` | 路径绘制基准 | `cargo run -p showcase --bin paths_bench` |
| `inspector` | 元素检查器默认面板 | `cargo run -p showcase --bin inspector`（`--release --features inspector` 亦可） |
| `inspector_custom` | 检查器自定义面板 | `cargo run -p showcase --bin inspector_custom` |
| `testing` | 测试基建（TestAppContext / VisualTestContext） | `cargo test -p showcase --bin testing` |

### 国际化 / 富文本 / 结构化展示

| bin | 说明 | 运行 |
|-----|------|------|
| `i18n` | 国际化（`I18nManager` / `I18nText` / 复数规则） | `cargo run -p showcase --bin i18n` |
| `markdown` | Markdown 渲染（含代码高亮/数学/任务列表插件） | `cargo run -p showcase --bin markdown` |
| `tabs` | 标签页（含拖拽重排） | `cargo run -p showcase --bin tabs` |
| `tree` | 树形控件 | `cargo run -p showcase --bin tree` |
| `status_bar` | 状态栏 | `cargo run -p showcase --bin status_bar` |

### 编辑器 / 工作区

| bin | 说明 | 运行 |
|-----|------|------|
| `editor` | 代码编辑器（tree-sitter / LSP / 片段 / inlay，需 feature） | `cargo run -p showcase --bin editor --features editor` |
| `keymap` | 快捷键映射 | `cargo run -p showcase --bin keymap` |
| `dock` | 停靠面板布局 | `cargo run -p showcase --bin dock` |
| `chat` | AI 对话视图（ChatView 等） | `cargo run -p showcase --bin chat` |

### 应用套件基座（`ribbon` / `viewport` feature）

| bin | 说明 | 运行 |
|-----|------|------|
| `command_registry` | 命令注册表：按 id 派发 + 命令面板接入 | `cargo run -p showcase --bin command_registry` |
| `ribbon` | Ribbon 功能区（选项卡 / 分组 / 大·小·切换·下拉按钮 / 上下文选项卡）+ Alt/F10 按键提示 | `cargo run -p showcase --bin ribbon` |
| `qat` | 快速访问工具栏（QAT）：按命令 id 绑定的紧凑图标按钮条 | `cargo run -p showcase --bin qat` |
| `mini_toolbar` | 迷你工具栏：拖动选择后就近浮出命令条（B/I/U/高亮/复制） | `cargo run -p showcase --bin mini_toolbar` |
| `backstage` | 应用菜单：覆盖整窗的文件面板（左导航 + 内容页/命令页） | `cargo run -p showcase --bin backstage` |
| `viewport` | 视口：可缩放/平移的 A4 文档页视图（屏幕↔文档坐标映射） | `cargo run -p showcase --bin viewport` |
| `ruler` | 标尺刻度 + 拖拽方块吸附并显示参考线 | `cargo run -p showcase --bin ruler` |

## 独立示例 crate

以下示例因重依赖或平台相关，保留为独立包：

| crate | 说明 |
|-------|------|
| `image_showcase` | 图片加载 / 查看（多 bin） |
| `list_showcase` | 列表 / 虚拟列表（多 bin） |
| `text_showcase` | 文本 / 编辑相关（多 bin） |
| `overlay_showcase` | 浮层 / Overlay（多 bin） |
| `window_showcase` | 窗口能力（多 bin） |
| `tray` | 系统托盘（多 bin） |
| `view_example` | 视图 / 布局（多 bin） |
| `rgpui_story` | 组件 Storybook |
| `desktop_pet` | 桌面宠物（托盘、单实例） |
| `desktop_pet_3d` | 3D 桌面宠物（需 `rgpui-3d`） |
| `rgpui_term_basic` | 终端基础 |
| `rgpui_term_integration` | 终端集成 |
| `screen_capture` | 屏幕捕获 |
| `webview` | WebView 集成 |
| `layer_shell` | Wayland layer shell |
| `tokio_example` | Tokio 异步集成 |

运行独立示例：`cargo run -p <crate> [--bin <名称>]`。

## 新增示例约定

- 只需要 `rgpui` + `rgpui-platform` 的单文件示例，放进 `showcase`，在
  `examples/showcase/Cargo.toml` 增加一条 `[[bin]]` 并在本文件登记。
- 若依赖额外 feature，给该 bin 加 `required-features`，并在运行命令中带上 `--features`。
- 重依赖、或需要独立平台配置的，另建独立 crate。
- 每个示例文件头部用 `//!` 标注一句话说明与运行命令。
