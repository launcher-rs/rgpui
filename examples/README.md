# rgpui 示例索引

本目录是 rgpui 的示例集合。绝大多数示例已合并进 `showcase`，按大类分成 **8 个 bin**。
直接运行即可看到界面：左侧竖排 tab 切换各示例，无需传参。

```bash
cargo run -p showcase --bin <gallery>
```

少数重依赖或需要独立平台的示例保留为独立 crate（见文末）。

## showcase（8 个 gallery）

| bin | 包含（节选） | 运行 |
|-----|------|------|
| `form_gallery` | 单文件一页展示：输入框 / 表单 / 问卷向导；另可 `-- keymap` / `-- dialog` | `cargo run -p showcase --bin form_gallery` |
| `core_gallery` | hello_world / a11y / theme / animation / component_gallery / scrollable / tabs / sidebar / fps_hud / inspector / gradient / anchor / popover 等 21 个 | `cargo run -p showcase --bin core_gallery -- <demo>` |
| `text_gallery` | markdown / markdown_stream / markdown_selection / i18n / intl_text / rich_text / rich_clipboard / text / text_layout / text_wrapper | `cargo run -p showcase --bin text_gallery -- <demo>` |
| `creative_gallery` | ink / sorter / timeline / color_picker / mdi / task_pane / canvas_a11y / page_view / drag_drop / focus_visible / native_menu / grid_layout / dock / chat 等 24 个 | `cargo run -p showcase --bin creative_gallery -- <demo>` |
| `media_gallery` | image_loading / image_gallery / gif_viewer / svg / image / window_movable / window / custom_title_bar / list_example / uniform_list / data_table / tree 等 18 个 | `cargo run -p showcase --bin media_gallery -- <demo>` |
| `suite_gallery` | command_registry / ribbon / qat / mini_toolbar / backstage / viewport / ruler / canvas_objects / formula_bar / sheet_grid / app_suite / automation（需默认 feature） | `cargo run -p showcase --bin suite_gallery -- <demo>` |
| `visual_gallery` | charts / effects / qr_code（需 `--features charts,effects,qr-code`） | `cargo run -p showcase --features charts,effects,qr-code --bin visual_gallery -- <demo>` |
| `tools_gallery` | diff / editor / input_latency（需 `--features diff,editor,input-latency`） | `cargo run -p showcase --features diff,editor,input-latency --bin tools_gallery -- <demo>` |

新增/查找示例都从这张表开始。每个 bin 是**单文件**：视图、状态、入口和注释都在
`examples/showcase/src/bin/<bin>.rs` 里，不用 `#[path] mod` 引外部文件。

- `form_gallery` 是一页展示三段示例；其他 gallery 默认打开竖排 tab 导航界面，点选即可查看。
- 需要单独窗口的少数示例仍可用 `-- <demo>` 直接启动（如 `-- daemon_app`）。

## 独立示例 crate

| crate | 说明 | 运行 |
|-------|------|------|
| `view_example` | 视图 / 布局（单 bin） | `cargo run -p view_example` |
| `screen_capture` | 屏幕捕获 | `cargo run -p screen_capture` |
| `desktop_pet` | 桌面宠物（托盘、单实例） | `cargo run -p desktop_pet` |
| `desktop_pet_3d` | 3D 桌面宠物（需 `rgpui-3d`） | `cargo run -p desktop_pet_3d` |
| `rgpui_story` | 组件 Storybook（统一入口）：保留 Markdown / v1.1 特性 / 平台契约，并汇总 showcase 的 73 个示例 | `cargo run -p rgpui_story` |
| `rgpui_term_basic` | 终端基础（`--bin basic` / `--bin external_io`） | `cargo run -p rgpui_term_basic --bin basic` |
| `rgpui_term_integration` | 终端集成 | `cargo run -p rgpui_term_integration` |
| `webview` | WebView 集成（需 `--features webview`） | `cargo run -p webview --features webview` |
| `tokio_example` | Tokio 异步集成 | `cargo run -p tokio_example` |
| `tray` | 系统托盘（多 bin） | `cargo run -p tray --bin tray_simple` |

运行独立示例：`cargo run -p <crate> [--bin <名称>]`。

`examples/image_showcase/` 现在只是共享图片资产目录（无 `Cargo.toml`），供 `media_gallery`
的图片演示和 `desktop_pet_3d` 的图标使用。

## 新增/改造示例约定

- **单文件模板**：参考 `showcase/src/bin/form_gallery.rs` —— 一个 bin 文件包含全部
  视图、状态与入口；组件介绍与用法写在 `//!` / `//` 注释里，不占用界面空间。
- **多个 demo 的 gallery**：同一大类的示例放在同一个 bin 文件里，用内联 `mod <demo> { … }`
  块隔离各自的 `use` / 类型 / action，入口用 `-- <demo>` 分发。
- 不要把示例拆成多个 bin 或独立 crate，除非它依赖无法并入 showcase 的特殊平台配置。
- 每个示例文件头部用 `//!` 标注一句话说明与运行命令，函数用中文注释。
