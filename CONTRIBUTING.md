# 贡献指南

感谢你对 rgpui 项目的关注！以下是参与贡献的基本流程。

## 开发环境

- **Rust**：stable 工具链（`rustup show`）
- **操作系统**：Windows 10+、macOS 13+、Ubuntu 22.04+
- **Web 开发**（可选）：nightly 工具链 + `wasm32-unknown-unknown` + Trunk

```bash
# 安装 nightly 工具链（Web/WASM 开发）
rustup toolchain install nightly
rustup target add wasm32-unknown-unknown --toolchain nightly
rustup component add rust-src --toolchain nightly
cargo install trunk
```

## 常用命令

```bash
# 检查整个 workspace（推荐日常使用）
cargo check --workspace

# 构建并运行示例
cargo run --example tray

# 运行所有测试
cargo test --workspace

# Clippy 检查
cargo clippy --workspace

# 格式化代码
cargo fmt --all
```

## 代码规范

### 提交前检查

1. `cargo check --workspace` 通过
2. `cargo check --workspace --examples` 通过
3. `cargo fmt --all` 格式化代码
4. 所有公开 API 添加中文文档注释（`///`）

### 文档要求

- 公开函数和 trait 方法必须使用 `///` 中文注释
- 注释风格遵循 Rust 文档规范
- 避免使用英文注释，保持项目语言统一

### 跨平台注意事项

- 平台特有代码放在对应的 `rgpui-<platform>/` crate 中
- 使用 `#[cfg(target_os = "...")]` 条件编译
- 提交前检查跨平台编译：`cargo check --workspace` 至少在本机平台通过

### 禁止事项

- 禁止使用 `#[allow(dead_code)]` 压制警告
- 禁止提交密钥、密码等敏感信息
- 禁止删除中文注释

## 项目结构

```
crates/
├── rgpui/              # 核心 UI 框架（组件库、动画、手势）
├── rgpui-3d/           # 3D 渲染支持
├── rgpui-character/    # 字符/文本处理
├── rgpui-dom/          # Web DOM 后端
├── rgpui-linux/        # Linux 平台实现
├── rgpui-macos/        # macOS 平台实现
├── rgpui-macros/       # 过程宏
├── rgpui-platform/     # 平台选择入口
├── rgpui-term/         # 终端组件
├── rgpui-web/          # Web/WASM 平台实现
├── rgpui-wgpu/         # wgpu 渲染后端
└── rgpui-windows/      # Windows 平台实现
```

## 提交规范

- 提交信息使用中文
- 格式：`<类型>: <简短描述>`
- 类型：`feat`（新功能）、`fix`（修复）、`refactor`（重构）、`docs`（文档）、`test`（测试）、`chore`（构建/工具）

示例：
```
feat: 添加系统托盘图标双击事件支持
fix: 修复窗口最大化后级联定位偏移问题
docs: 为 Platform trait 补全文档注释
```

## 相关文档

- [组件整合计划](docs/component-integration-plan.md)
- [UI Crate 规划](docs/ui-crate-plan.md)
- [上游切分策略](docs/upstream-separation-strategy.md)
- [开发指南（AGENTS.md）](AGENTS.md)
