# Examples 独立化 + PR 工作流实施方案

## 背景与目标

### 问题 1：发布时循环依赖

`rgpui` 发布到 crates.io 时，examples 需要 `rgpui-platform` 作为 dev-dependency，
但 `rgpui-platform` 又依赖 `rgpui`，形成循环依赖。`cargo publish` 解析 dev-dependencies
时会报错，只能用 `--no-verify` 绕过。

### 问题 2：开发流程不规范

当前直接推送到 main 分支，没有 code review 和 CI 预检，代码质量和历史追溯性差。

### 目标

1. 将 examples 从 `rgpui` crate 中独立出来，彻底消除循环依赖
2. 建立 PR-based 开发工作流，所有变更通过 PR 合并

---

## 第一部分：Examples 独立化

### 现状

```
crates/rgpui/examples/          # 49 个内联 .rs + 1 个 view_example 目录
  ├── hello_world.rs
  ├── tray.rs
  ├── view_example/             # 多文件 example（5 个文件）
  ├── image/                    # 含 assets 的 example
  └── ...（共 49 个 .rs 文件）

examples/                       # 7 个已独立的 crate
  ├── desktop_pet/
  ├── desktop_pet_3d/
  ├── extended_components/
  ├── rgpui_story/
  ├── rgpui_term_basic/
  ├── rgpui_term_integration/
  └── screen_capture/
```

**关键发现：**
- 49 个内联 examples **100% 使用 `rgpui_platform`**
- **没有 examples 使用私有/未导出 API**
- 6 个 examples 需要 feature gates：`charts`、`effects`、`qr_code`、`input-latency-histogram`、`tokio`、`reqwest`

### 方案：顶层 `examples/` 目录统一管理

#### 目标结构

```
rgpui/
├── Cargo.toml                    # 主 workspace（exclude = ["examples"]）
├── crates/                       # 库代码
│   └── rgpui/
│       └── examples/
│           └── README.md         # 保留说明文档，指引到 examples/
├── examples/                     # 所有 examples 统一存放
│   ├── Cargo.toml                # [workspace] 隔离，不属于主 workspace
│   ├── README.md                 # examples 总览
│   ├── hello_world/              # 每个 example 一个目录
│   │   ├── Cargo.toml
│   │   └── main.rs
│   ├── tray/
│   ├── desktop_pet/              # 已有，直接移动
│   ├── ...（共约 56 个目录）
│   └── shared/                   # 共享工具（如有需要）
│       ├── Cargo.toml
│       └── lib.rs
```

#### 关键设计

**1. examples 独立 workspace**

```toml
# examples/Cargo.toml
[workspace]
members = ["*"]
# 不继承主 workspace，避免依赖冲突
```

**2. 路径依赖，不依赖 crates.io**

```toml
# examples/hello_world/Cargo.toml
[package]
name = "example-hello-world"
version = "0.0.0"
publish = false
edition = "2024"

[dependencies]
rgpui = { path = "../../crates/rgpui" }
rgpui-platform = { path = "../../crates/rgpui-platform" }
```

**3. Feature gates 保持不变**

```toml
# examples/charts/Cargo.toml
[dependencies]
rgpui = { path = "../../crates/rgpui", features = ["charts"] }
```

**4. 主 workspace 排除 examples**

```toml
# 根 Cargo.toml
[workspace]
exclude = ["examples"]
```

#### 迁移策略

**分批迁移，不一次性移动所有文件：**

| 批次 | 内容 | 数量 | 优先级 |
|------|------|------|--------|
| 第 1 批 | 已有独立 crate 位置调整 | 7 | 高（已在 examples/ 下） |
| 第 2 批 | 高频使用的 examples | ~15 | 高（hello_world, tray, animation 等） |
| 第 3 批 | 带 feature gates 的 examples | 6 | 中（charts, effects 等） |
| 第 4 批 | 剩余 examples | ~21 | 低（可逐步迁移） |

**命名规范：**
- 目录名：kebab-case（`hello-world`、`window-shadow`）
- crate 名：`example-{目录名}`（`example-hello-world`）
- 所有 example crate 设置 `publish = false`

#### 迁移后清理

**rgpui/Cargo.toml 变更：**

1. 移除所有 `[[example]]` 条目（约 30 个）
2. 移除 `[dev-dependencies]` 中的 `rgpui-platform`
3. 保留 `rgpui` 自身的测试所需 dev-dependencies（`backtrace`、`env_logger` 等）
4. 添加 `exclude = ["examples/"]` 到 `[package]`（防止打包）

**rgpui-macros/Cargo.toml：**
- dev-dependency `rgpui` 已移除，无需再改

**发布命令恢复正常：**
```bash
cargo publish -p rgpui           # 不再需要 --no-verify
cargo publish -p rgpui-platform  # 不再有循环依赖
```

#### 特殊处理

**view_example（多文件 example）：**
- 作为独立 crate 迁移，保留 `src/` 目录结构
- `mod example_*` 改为 `mod` 声明或内联

**image/ 和 svg/（含 assets）：**
- 将 assets 放在 example 目录下
- 用 `std::path::Path` 相对路径引用，或 `include_bytes!`

**带 `#[cfg(test)]` 的 examples（如 testing.rs）：**
- 迁移后作为独立 crate 的测试，需添加 `rgpui = { ..., features = ["test-support"] }`

---

## 第二部分：PR 工作流

### 分支保护设置

**GitHub Settings → Branches → Add rule：**

```
Branch name pattern: main

✅ Require a pull request before merging
   - Required approvals: 1
   - Dismiss stale pull request approvals when new commits are pushed

✅ Require status checks to pass before merging
   - Required: check (CI job name)
   - Require branches to be up to date before merging

✅ Require conversation resolution before merging（可选，小团队可不开）

❌ Do not require status checks for administrators（保持一致）
❌ Allow force pushes（禁止）
❌ Allow deletions（禁止）
```

### 开发工作流

```
main ──────────────────────────────────────────────── 受保护
  │
  ├── feat/examples-extraction ── PR ── Squash Merge ──→ main
  │
  ├── fix/tab-group-ordering ─── PR ── Squash Merge ──→ main
  │
  └── chore/upgrade-spin ─────── PR ── Squash Merge ──→ main
```

#### 步骤详解

**1. 创建功能分支**
```bash
git checkout main
git pull origin main
git checkout -b feat/examples-extraction
```

**2. 开发 + 本地验证**
```bash
# 必须全部通过
cargo check --workspace
cargo clippy --workspace
cargo test --workspace
cargo fmt --all --check
```

**3. 推送功能分支**
```bash
git add -A
git commit -m "feat: 将 examples 独立为 workspace crate"
git push origin feat/examples-extraction
```

**4. 创建 PR**
```bash
gh pr create \
  --title "feat: 将 examples 独立为 workspace crate" \
  --body-file .github/pr_template.md
```

**5. CI 自动运行**
- push 后 GitHub Actions 自动触发
- 跑 fmt、clippy、check、test
- 状态显示在 PR 页面

**6. Review + 合并**
- 至少 1 人 approve
- CI 全绿
- **Squash Merge**（推荐）：一个 PR = 一个 commit
- 合并后自动删除功能分支

#### 分支命名规范

```
feat/xxx       # 新功能
fix/xxx        # Bug 修复
chore/xxx      # 杂务（依赖升级、CI 调整）
refactor/xxx   # 重构
docs/xxx       # 文档
test/xxx       # 测试
```

#### PR 描述模板

创建 `.github/pull_request_template.md`：

```markdown
## 变更内容
- 

## 测试验证
- [ ] `cargo check --workspace` 通过
- [ ] `cargo clippy --workspace` 通过
- [ ] `cargo test --workspace` 通过
- [ ] `cargo fmt --all --check` 通过

## 相关 Issue
Closes #

## 截图/录屏（如适用）
```

#### 合并策略

| 方式 | 历史效果 | 适用场景 |
|------|---------|---------|
| **Squash Merge** ⭐ | 线性，一个 PR 一个 commit | 日常开发，推荐默认使用 |
| Merge Commit | 保留完整分支历史 | 大特性，多 commit 有意义 |
| Rebase | 线性，但改写 commit hash | 追求极致线性（不推荐） |

**建议：默认 Squash Merge**，GitHub Settings 中设置 "Allow squash merging" 为唯一选项。

### CI 配置调整

当前 CI 在 push 到 main 时运行。PR 工作流下需要同时在 PR 上运行：

```yaml
# .github/workflows/ci.yml
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
```

确保 PR 提交前就能看到 CI 结果。

---

## 执行计划

### Phase 1：建立 PR 工作流（Day 1）

| 步骤 | 内容 | 耗时 |
|------|------|------|
| 1.1 | 设置 GitHub branch protection rules | 10 min |
| 1.2 | 创建 `.github/pull_request_template.md` | 5 min |
| 1.3 | 更新 CI 配置支持 PR 触发 | 5 min |
| 1.4 | 测试：创建一个 PR 验证流程 | 15 min |

### Phase 2：迁移已有独立 crate（Day 1）

| 步骤 | 内容 | 耗时 |
|------|------|------|
| 2.1 | 将 `crates/rgpui/examples/desktop_pet/` 等 7 个已独立 crate 移到 `examples/` | 15 min |
| 2.2 | 更新路径引用（`path = "../../crates/rgpui"`） | 10 min |
| 2.3 | 更新主 workspace Cargo.toml（exclude examples） | 5 min |
| 2.4 | 验证编译 | 5 min |
| 2.5 | 通过 PR 合并 | - |

### Phase 3：迁移内联 examples（Day 2-3，分批）

| 步骤 | 内容 | 耗时 |
|------|------|------|
| 3.1 | 迁移高频 examples（hello_world, tray, animation 等 ~15 个） | 2 h |
| 3.2 | 迁移带 feature gates 的 examples（~6 个） | 1 h |
| 3.3 | 迁移剩余 examples（~21 个） | 2 h |
| 3.4 | 处理 view_example（多文件） | 30 min |
| 3.5 | 处理 image/ 和 svg/（含 assets） | 30 min |
| 3.6 | 每批通过 PR 合并 | - |

### Phase 4：清理发布流程（Day 3）

| 步骤 | 内容 | 耗时 |
|------|------|------|
| 4.1 | 从 rgpui 移除所有 `[[example]]` 条目 | 10 min |
| 4.2 | 从 rgpui 移除 `rgpui-platform` dev-dependency | 5 min |
| 4.3 | 更新 `crates/rgpui/examples/README.md` 指引到 `examples/` | 10 min |
| 4.4 | 验证 `cargo publish -p rgpui --dry-run` | 5 min |
| 4.5 | 验证 `cargo publish -p rgpui-platform --dry-run` | 5 min |
| 4.6 | 通过 PR 合并 | - |

### 验收标准

- [ ] `cargo check --workspace` 通过
- [ ] `cargo check --workspace --examples`（examples 目录）通过
- [ ] `cargo clippy --workspace` 无警告
- [ ] `cargo test --workspace` 通过
- [ ] `cargo publish -p rgpui --dry-run` 成功（不需 --no-verify）
- [ ] `cargo publish -p rgpui-platform --dry-run` 成功
- [ ] GitHub PR 流程可正常运行
- [ ] main 分支受保护，无法直接推送
