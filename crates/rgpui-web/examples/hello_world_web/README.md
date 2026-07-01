# hello_world_web

rgpui-component 最简 Web 示例，展示基本按钮和布局在浏览器中的运行效果。

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
cd crates/rgpui-component/examples/hello_world_web
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

## 项目结构

```
hello_world_web/
├── .cargo/config.toml      # WASM 编译 flags（atomics、shared-memory）
├── src/main.rs             # 应用代码
├── Cargo.toml              # 独立 crate 配置
├── index.html              # HTML 外壳
├── trunk.toml              # Trunk 配置（含 COOP/COEP 头）
└── rust-toolchain.toml     # 指定 nightly + wasm32 目标
```
