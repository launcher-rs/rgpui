# rgpui-character 设计开发文档（v0.1）

## 项目定位

`rgpui-character` 是基于 `rgpui` 的 UI 角色运行时系统，目标是支撑桌宠、UI 助手、动态角色 UI、轻量 Live2D-like 角色以及多角色桌面互动系统。

它当前放在 rgpui workspace 内开发，便于直接复用 rgpui 的窗口、事件和渲染能力。等角色系统稳定后，可以独立拆分成单独仓库或单独发布的 crate。

核心理念：

```text
Character = Behavior + Animation + Physics + Render
```

系统边界：

```text
rgpui -> UI / 窗口 / 渲染
rgpui-character -> 角色逻辑 / 动画 / 物理 / 渲染命令
```

## 总体架构

```text
rgpui-character
├── core        # 角色核心数据模型
├── animation   # sprite / clip 动画系统
├── behavior    # 行为系统与状态机入口
├── physics     # 轻量运动、摩擦、边界逻辑
├── asset       # 资源索引与动画资源管理
├── render      # 渲染命令与后端桥接
└── runtime     # 多角色调度器
```

主数据流：

```text
Behavior -> State -> Animation -> Physics -> RenderCommand -> rgpui
```

## Core：核心模型

`Character` 是运行时的最小角色单元，持有：

- `id`：角色标识
- `state`：当前行为阶段
- `position`：当前位置
- `velocity`：当前速度
- `animation`：动画播放状态
- `behavior`：行为状态缓存

状态只表达角色当前处于哪个行为阶段，不包含渲染逻辑，也不依赖 rgpui。

基础状态：

```rust
pub enum CharacterState {
    Idle,
    Walk,
    Sleep,
    Dragging,
    Custom(String),
}
```

## Animation：动画系统

动画系统只负责视觉帧推进，不参与行为决策。

`AnimationClip` 描述一段 sprite 动画：

- `name`：动画名称
- `frames`：纹理帧序列
- `fps`：播放帧率
- `looped`：是否循环

`AnimationPlayer` 记录播放进度，并根据 `dt` 推进当前帧。对于非循环动画，播放到末尾后停在最后一帧。

## Behavior：行为系统

行为系统表达角色“想做什么”，通过 `BehaviorAction` 输出意图：

- `Move(Vec2)`：给角色增加移动速度
- `ChangeState(CharacterState)`：切换行为阶段
- `PlayAnimation(String)`：请求播放动画
- `Idle`：本帧不做行为变化

行为系统不直接绘制、不直接持有 rgpui 对象，未来可以替换为规则系统、行为树、脚本或 AI 驱动。

## Physics：轻量物理

物理系统负责：

- 根据速度更新位置
- 应用摩擦
- 限制角色在边界内
- 可选地在碰到边界时反弹

当前 v0.1 不引入完整物理引擎，只提供桌宠需要的轻量能力。

## Render：渲染命令

runtime 不直接调用 rgpui，而是输出 `RenderCommand`：

```rust
pub enum RenderCommand {
    DrawSprite {
        texture: TextureId,
        position: Vec2,
        scale: Vec2,
        rotation: f32,
    },
}
```

rgpui 集成层实现 `RenderBackend`，再把命令翻译为 rgpui 的图片绘制 API。这样角色系统可以保持平台无关，也方便未来接入其他渲染后端或测试后端。

## Asset：资源系统

`AssetManager` 管理纹理标识和动画片段。v0.1 只做内存索引，不负责真实文件 IO：

- 纹理由调用方注册成 `TextureId`
- 动画由调用方注册成 `AnimationClip`
- 后续可以扩展 pack system、延迟加载和缓存淘汰

## Runtime：调度核心

`CharacterRuntime` 统一调度多个角色：

```text
for character in characters:
    behavior.update()
    apply action
    physics.update()
    animation.update()
    build render command
```

每帧复杂度保持在 `O(n characters)`。

## rgpui 集成策略

v0.1 的 crate 不直接依赖 rgpui 绘制细节，只定义渲染桥接 trait：

```rust
pub trait RenderBackend {
    fn draw_sprite(&mut self, cmd: &RenderCommand);
}
```

桌宠应用层可以在 rgpui 的 `Render` 实现里调用 runtime，拿到 `RenderCommand` 后执行绘制。

## 多角色与事件扩展

多角色由 `CharacterRuntime` 管理。事件系统用于鼠标互动和 UI 反馈：

- `Click`
- `Hover`
- `DragStart`
- `DragEnd`
- `Custom(String)`

事件先进入角色，再由行为系统决定是否改变状态、速度或动画。

## 性能原则

- 每帧只线性遍历角色
- 纹理资源共享缓存
- 动画播放状态保持轻量
- 不引入 ECS 和完整物理引擎
- 避免每帧大量分配

## v0.1 落地范围

本阶段实现：

- 角色核心结构
- 动画片段和播放器
- 行为 trait 与基础行为
- 轻量物理系统
- 渲染命令和后端 trait
- 资源索引
- 多角色 runtime

暂不实现：

- 3D
- ECS
- 完整 Live2D runtime
- 完整物理引擎
- 资源包格式

## v0.2 方向

下一阶段可以扩展：

- 分层伪 Live2D
- head / hair / eye rig
- sin-based deformation
- 呼吸系统
- 情绪系统
- 行为树
