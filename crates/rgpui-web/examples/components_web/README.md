# components_web

rgpui-component Web 组件总览示例，展示常用 UI 组件在浏览器中的运行效果，包括按钮、输入框、复选框、开关、标签、徽章、头像、警告条、骨架屏、加载器、进度条等。

## 前置条件

```bash
# 安装 nightly 工具链
rustup toolchain install nightly
rustup target add wasm32-unknown-unknown --toolchain nightly
rustup component add rust-src --toolchain nightly

# 安装 Trunk 构建工具
cargo install trunk
```

## 开发

```bash
cd crates/rgpui-component/examples/components_web
trunk serve
```

浏览器自动打开 `http://127.0.0.1:8080`，修改代码后自动热重载。

## 构建生产版本

```bash
trunk build --release
```

产物输出到 `dist/` 目录，可部署到任何静态文件服务器（Nginx、GitHub Pages、Vercel 等）。

## 部署注意事项

部署时需要配置 HTTP 响应头，否则 Web Worker 和 WebGPU 不可用：

```
Cross-Origin-Embedder-Policy: require-corp
Cross-Origin-Opener-Policy: same-origin
```

Nginx 示例配置：

```nginx
location / {
    add_header Cross-Origin-Embedder-Policy "require-corp";
    add_header Cross-Origin-Opener-Policy "same-origin";
}
```

## 示例组件

| 组件 | 说明 |
|------|------|
| Button | 主要/次要/危险/幽灵/描边变体，禁用/加载状态，计数器交互 |
| Input | 文本输入、可清除输入 |
| Checkbox | 复选框，支持选中状态 |
| Switch | 开关，支持选中状态 |
| Tag | 主要/次要/成功/警告/危险标签 |
| Badge | 计数徽章、溢出省略、点状徽章 |
| Avatar | 头像、大号头像 |
| Alert | 信息/成功/警告/错误提示条 |
| Skeleton | 骨架屏加载占位 |
| Spinner | 加载指示器 |
| Progress | 进度条、进度圈 |

## 项目结构

```
components_web/
├── .cargo/config.toml      # WASM 编译 flags（atomics、shared-memory）
├── src/main.rs             # 应用代码
├── Cargo.toml              # 独立 crate 配置
├── index.html              # HTML 外壳
├── trunk.toml              # Trunk 配置（含 COOP/COEP 头）
└── rust-toolchain.toml     # 指定 nightly + wasm32 目标
```
