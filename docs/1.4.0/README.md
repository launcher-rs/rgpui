# 1.4.0（移动端：Android 优先）

> 开发文档：`1.4.0-dev-plan.md`（实现依据，每项完成后勾选）。

- 目标：同一套 `rgpui` 应用跑到 Android 真机；`rgpui-platform` 仍是唯一门面。
- 本机只有 Android 设备：真机验收以 Android 为准；iOS 只做结构预留 + CI `check`。
- 参考实现：`temp/gpui-mobile`（只借鉴架构，不合入 `gpui-pre` 快照代码）。
- 阶段：M1 骨架可编译 + CI 可验证 → M2 Android 真机点亮 → M3 体验对齐 → M4 iOS 跟进 + 发布。

## 文档索引

- `1.4.0-dev-plan.md`：开发计划（含 §7 分 crate 决策与平台扩展指南）。
- `android-guide.md`：Android 接入全指南（环境/依赖/编译/release/图标/证书/底层 API/FAQ）。
- `ios-guide.md`：iOS 接线（M1 预留状态 + M4 清单）。
- 示例：`examples/hello_mobile/`（`README.md` 快速上手 + `android/` 最小宿主工程）。
