# rgpui-term

终端模拟组件，为 rgpui 提供终端仿真和交互能力。

## 功能

- VT100/xterm 终端仿真
- ANSI 颜色支持（256 色和 TrueColor）
- 终端元素渲染（`TerminalElement`）
- 键盘输入映射
- 终端滚动和选择

## 示例

```rust
use rgpui_term::TerminalElement;

let terminal = TerminalElement::new(shell_command, window, cx);
```

## 依赖

- `rgpui`（核心框架）
- `vte`（终端状态机）
