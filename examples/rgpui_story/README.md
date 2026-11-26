# rgpui_story（组件大全 Storybook）

rgpui 组件示例大全：左侧侧边栏分类导航，右侧内容区展示对应组件用法。
覆盖核心基础组件、输入/表单、菜单、对话框、列表、表格、标签页、
扩展组件与特效/图表（feature 门控）。找组件用法时先看这里。

「平台契约」分组展示本框架**看似跨平台、实际有平台差异**的 API 实时读数：
- 平台能力：OS / 合成器 / 网络 / 显卡 / 主显示器 / 按钮布局 / 聚焦窗口 / 权限 /
  滚动条自动隐藏（GNOME overlay-scrolling 实时同步）/ 热状态（Linux 未做，如实标注）
- 窗口契约：`window_controls` / `window_decorations` / 可调整大小 / 可最小化，
  可点按钮请求 Client / Server 装饰并重新读取实际值，观察回退与标题栏收敛

每项附平台差异说明；完整审计见 `docs/linux-api-parity-audit.md`，
待专项设计项见 `docs/linux-parity-followups.md`。

## 运行

```text
cargo run -p rgpui_story
```
