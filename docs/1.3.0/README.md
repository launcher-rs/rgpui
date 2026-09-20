# v1.3.0 开发文档

## 文件索引

| 文件 | 说明 |
|------|------|
| `1.3.0-dev-plan.md` | 开发计划与验收清单 |
| `../migration-guide-1.2-to-1.3.md` | 1.2 → 1.3 迁移指南（回调签名统一为主） |

## 1.3.0 变更总览

### 检查器完善（Inspector）—— I1–I4

- **I1** 树节点 ↔ 界面双向映射（点击树选中画布 + 橙框高亮）
- **I2** 完整元素树 + 逐节点折叠/展开（prepaint 嵌套记录，关闭零开销）
- **I3** 检查面板内置化（`App::enable_default_inspector()` 两行启用）
- **I4** 自定义接口文档化（book 检查器章节 + 双示例 recipe）

### 回调签名统一（C1 + C2，breaking）

- 纯点击一律 `on_click: Fn(&ClickEvent, &mut Window, &mut App)`
- 值变更一律 `on_change: Fn(Value, &mut Window, &mut App)`，值按值传递
- 存储统一 `Arc + Send + Sync + 'static`
- 新增 `Context::listener_value`（按值版 listener）

### 应用联动落地（F1–F7）

- `TITLE_BAR_LEFT_PADDING` 公开、`HotkeyInput::recording_text()`、`TreeEvent::Selected`
- `StatusBar` 数据驱动重写、`Theme::apply_named`、`HotkeyListInput` 多值、`PopupMenuItem::danger`

### 应用实战回流（G1–G7）

- `DialogId` + `close_dialog_by(id)` 按标识关闭
- App 回调回实体 recipe、`TreeEvent::Confirmed`、无状态 `Tabs`/`TabsItem`
- 菜单位置约束、`I18nManager` Global + `load_locale_dir` + `I18nSnapshot`
- `Dialog::overlay_visible(bool)` setter

### 1.2.0 结转（Z1）

- 删除 `chat_ui` deprecated 别名

### 第二批加菜（H1–H8，breaking 一次收完）

- **H1** C2 回调残留统一（`InteractiveText`/`CompletionPopup`/`Link`/`StatusBarItem`/`OTPInput` 等）
- **H2** 全局动作 helper（`App::on_global_action`，F12 沉淀）
- **H3** i18n 补强（`I18nText::translate_global`）
- **H4** story 加 Tabs 演示
- **H5** 面板 Header/Section 插槽化
- **H6** v1_3_showcase 新增示例
- **H7** 检查器 Chrome 化（盒模型、运行卡、报错卡、AI 可读树文本）
- **H8** 崩溃快照（`InspectorSnapshot` + `last.json` + panic hook）

## 验证清单

```bash
cargo check --workspace
cargo fmt --all
cargo clippy --workspace --lib --bins -- -D warnings
cargo test --workspace --lib
cargo run -p inspector              # 默认面板
cargo run -p inspector_custom       # 自定义面板
cargo run -p v1_3_showcase          # 新 API 集中演示
```
