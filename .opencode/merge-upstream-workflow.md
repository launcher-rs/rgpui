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
2. **自动更新上游仓库**：若 `temp/<仓库名>-upstream/` 已存在，执行 `git pull --ff-only` 获取最新代码；否则克隆仓库
3. 获取 PR 的变更文件列表，按映射规则处理路径和内容
4. 写入本地仓库，运行 `cargo check` 验证
5. 更新 `UPSTREAM-PRS.json` 中的 PR 状态

### 扫描新 PR

扫描上游仓库的新提交，发现待处理 PR。脚本会：
- 自动更新上游仓库到最新
- 根据 `UPSTREAM-PRS.json` 中该上游的最新 `merged_at` 日期过滤，只返回该日期之后的提交
- 自动跳过已在 `UPSTREAM-PRS.json` 或 `docs/merged-prs.md` 中记录的 PR

```powershell
# 扫描所有上游仓库的未合并 PR（按日期过滤）
.\scripts\merge-upstream-pr.ps1 -Scan

# 指定上游仓库扫描
.\scripts\merge-upstream-pr.ps1 -Scan -Upstream gpui-component
```

扫描结果输出后，脚本会将新发现的 PR 自动添加到 `UPSTREAM-PRS.json`，状态设为 `"pending"`。

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

## 5. 重要规则：保护 Cargo.toml 包元数据

**合并 PR 时，禁止修改 Cargo.toml 中以下包元数据字段：**

| 字段 | 说明 |
|------|------|
| `name` | crate 名称 |
| `version` | 版本号 |
| `edition` | Rust 版本 |
| `authors` | 作者信息 |
| `description` | 描述 |
| `license` | 许可证 |
| `repository` | 仓库地址 |
| `documentation` | 文档地址 |
| `readme` | 自述文件 |
| `homepage` | 主页 |
| `rust-version` | MSRV |
| `publish` | 发布设置 |

这些字段在 `upstream-rules.json` 的 `cargo_exclude_fields` 中配置，脚本应自动跳过这些字段的内容替换。

### 合并后手动检查

合并完成后，检查所有被修改的 `Cargo.toml` 文件：
1. `version` 是否被上游覆盖（应保持 rgpui 的原始版本号）
2. `name` 是否被错误修改（应保持 rgpui 前缀的 crate 名称）
3. `edition` 是否被更改（整个 workspace 统一使用 2024 edition）

若发现 Cargo.toml 包元数据被错误修改，应立即还原。使用以下命令查看变更：
```powershell
git diff -- '*/Cargo.toml'
```


## 5. 手动合并步骤

当自动脚本不适用时（如需要手动调整）：

1. 将上游 PR 中所有 `gpui` 开头的 crate 引用替换为 `rgpui` 开头
2. 检查 `Cargo.toml` 中的依赖声明是否指向正确的 rgpui crate 路径
3. 检查 `use` 语句和路径引用是否已更新
4. 运行 `cargo check --workspace` 验证无编译错误
5. 运行 `cargo check --workspace --examples` 全面验证
6. 运行 `cargo fmt` 格式化代码
