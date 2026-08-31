# rgpui-3d

rgpui 的 3D 渲染支持模块，提供 OpenGL/wgpu 集成、MSAA 抗锯齿和离线渲染能力。

## 功能

- 3D 场景上下文管理（`GpuContext`）
- MSAA 多重采样抗锯齿（可配置采样数）
- 离线渲染路径（无 swapchain，通过 resolve texture 读回 CPU）
- 支持加载 3D 模型（glTF 格式）

## 示例

```rust
use rgpui_3d::{GpuContext, Scene3d};

let context = GpuContext::new(window_handle)?;
let mut scene = Scene3d::new(&context)?;
scene.set_msaa_sample_count(4);
```

## 依赖

- `wgpu`（GPU 抽象层）
- `winit`（窗口创建）
- `bytemuck`（数据类型转换）
