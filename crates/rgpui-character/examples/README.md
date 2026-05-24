# rgpui-character 桌面宠物示例

这个目录提供一个完整的 `rgpui-character` crate 示例，用于演示如何把角色运行时接入 rgpui 透明桌面窗口。

## 运行

```bash
cargo run -p rgpui-character --example desktop_pet
```

## 示例内容

- `desktop_pet.rs`：完整桌宠示例代码。
- `assets/tray-icon.png`：系统托盘图标。
- `assets/cat/*.png`：小猫 sprite 帧素材。

当前包含五组动画：

- `walk`：小猫散步。
- `sleep`：小猫睡觉。
- `eat`：小猫吃东西。
- `spin`：小猫转圈圈。
- `yawn`：小猫打哈欠。

每组动画有 4 张透明 PNG 帧。示例把这些路径注册为 `TextureId`，再由 `AnimationClip` 组织成动画。

## rgpui-character 使用流程

1. 创建 `CharacterRuntime`。
2. 通过 `runtime.assets.register_animation(...)` 注册动画。
3. 创建 `Character`，设置初始位置和初始动画。
4. 实现 `Behavior`，在 `update` 中返回 `BehaviorAction`。
5. 每帧调用 `runtime.update(dt, behavior)`。
6. 从返回的 `RenderCommand` 中取出 `TextureId`。
7. 在 rgpui 中用 `img(texture_path)` 绘制当前 sprite。

核心代码形状：

```rust
let mut runtime = CharacterRuntime::new();
runtime.assets.register_animation(AnimationClip::new(
    "walk",
    vec![TextureId::new("assets/cat/walk_0.png")],
    8.0,
    true,
));

let mut pet = Character::new("desktop-cat");
pet.animation.play("walk");
runtime.add_character(pet);

let commands = runtime.update(dt, behavior);
```

## 交互方式

- 默认自动运行，小猫会随机散步、吃东西、转圈圈、打哈欠。
- 按一下 `Ctrl` 会进入交互模式，关闭鼠标穿透并显示控制面板。
- `Ctrl+Shift+P` 暂停或恢复。
- 暂停时小猫睡觉，控制面板可点击不同动作。
- 点击“散步”会退出交互模式，恢复鼠标穿透和自动移动。
- 关闭窗口会隐藏到系统托盘。
- 托盘左键或菜单“显示窗口”会恢复窗口。

## 设计要点

`rgpui-character` 不直接依赖 rgpui 的绘制 API，它只输出 `RenderCommand`。这个示例中的 rgpui 层只负责把 `TextureId` 解释为资源路径并绘制图片。这样角色运行时以后可以继续独立演进，也可以迁移到其他渲染后端。
