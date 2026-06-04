# 上游 PR 合并工作流

当用户要求合并上游 PR 时，执行以下流程：

## 1. 自动合并（优先）

使用 `scripts/merge-upstream-pr.ps1` 脚本：

```powershell
# 合并指定 PR（自动识别 PR 所属的上游仓库）
.\scripts\merge-upstream-pr.ps1 -PR 58291

# 合并所有待处理 PR（UPSTREAM-PRS.json 中 status = "pending" 的）
.\scripts\merge-upstream-pr.ps1 -All

# 指定上游仓库合并
.\scripts\merge-upstream-pr.ps1 -PR 123 -Upstream gpui-component

# 仅分析，不写入文件
.\scripts\merge-upstream-pr.ps1 -PR 58291 -DryRun
```

脚本自动执行：
1. 从 `UPSTREAM-PRS.json` 读取 PR 所属的上游仓库配置
2. 克隆/拉取对应仓库到 `temp/<仓库名>-upstream/`
3. 获取 PR 的变更文件列表，按映射规则处理路径和内容
4. 写入本地仓库，运行 `cargo check` 验证
5. 更新 `UPSTREAM-PRS.json` 中的 PR 状态

### 扫描新 PR

先扫描上游仓库的新提交，发现待处理 PR：

```powershell
# 扫描所有上游仓库最近 200 个提交，输出未合并的 PR
.\scripts\merge-upstream-pr.ps1 -Scan

# 指定上游仓库扫描
.\scripts\merge-upstream-pr.ps1 -Scan -Upstream zed
```

扫描结果会自动更新 `UPSTREAM-PRS.json`，新增的 PR 状态设为 `"pending"`。

## 2. 支持的上游仓库

| 名称 | 仓库 | 映射的 crate |
|------|------|-------------|
| `zed` | zed-industries/zed | rgpui, rgpui_macos, rgpui_windows, rgpui_linux, rgpui_web, rgpui_wgpu, rgpui_macros, rgpui_tokio, rgpui_platform |
| `gpui-component` | longbridge/gpui-component | rgpui-component, rgpui-component-macros, rgpui-component-assets, rgpui-webview |
| `yororen-ui` | MeowLynxSea/yororen-ui | rgpui-yororen-ui |

## 3. 包名映射规则

| Zed 原 crate | rgpui 对应 crate |
|--------------|------------------|
| `gpui` | `rgpui` |
| `gpui_platform` | `rgpui_platform` |
| `gpui_windows` | `rgpui_windows` |
| `gpui_macos` | `rgpui_macos` |
| `gpui_linux` | `rgpui_linux` |
| `gpui_web` | `rgpui_web` |
| `gpui_wgpu` | `rgpui_wgpu` |
| `gpui_macros` | `rgpui_macros` |
| `gpui_tokio` | `rgpui_tokio` |
| `gpui_component` | `rgpui_component` |
| `gpui_component_macros` | `rgpui_component_macros` |
| `gpui_component_assets` | `rgpui_component_assets` |
| `gpui_webview` | `rgpui_webview` |
| `yororen_ui` | `rgpui_yororen_ui` |

## 4. 配置文件

- `.opencode/upstream-rules.json` — 上游仓库配置（URL、分支、路径映射、内容替换规则）
- `UPSTREAM-PRS.json` — PR 状态追踪（status: pending / merged / check-failed / analyzed）

## 5. 手动合并步骤

当自动脚本不适用时（如需要手动调整）：

1. 将上游 PR 中所有 `gpui` 开头的 crate 引用替换为 `rgpui` 开头
2. 检查 `Cargo.toml` 中的依赖声明是否指向正确的 rgpui crate 路径
3. 检查 `use` 语句和路径引用是否已更新
4. 运行 `cargo check --workspace` 验证无编译错误
5. 运行 `cargo check --workspace --examples` 全面验证
6. 运行 `cargo fmt` 格式化代码
