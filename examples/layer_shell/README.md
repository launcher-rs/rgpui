# layer_shell（Linux 面板窗口，仅 Linux）

演示常驻面板类窗口：Wayland 上走 layer-shell 协议（`WindowKind::LayerShell`），
X11 上走 EWMH（DOCK 窗口 + `_NET_WM_STRUT_PARTIAL`）。窗口锚定屏幕上方并
通过 `Window::set_exclusive_edge` / `set_exclusive_zone` 让出顶部区域，
其他窗口不会压到面板上。

## 运行（Linux）

```text
cargo run -p layer_shell
```

X11 下可用 `xprop` 回读验证：

```text
xprop -notype -id <窗口 ID> _NET_WM_STRUT _NET_WM_STRUT_PARTIAL _NET_WM_WINDOW_TYPE
xprop -root _NET_WORKAREA      # 面板出现前后对比
```

`_NET_WM_STRUT_PARTIAL` 的 12 个值依次是 left、right、top、bottom、
left_start_y、left_end_y、right_start_y、right_end_y、top_start_x、top_end_x、
bottom_start_x、bottom_end_x（物理像素）。`set_exclusive_zone` 传非正值即撤销
保留区域（两个属性都会被删除）。

注意两点 X11 与 Wayland 的差异：

- X11 上 strut 必须由**被 WM 接管**的窗口提出，所以这里不设置 `override_redirect`；
- 保留宽度按「离屏幕边缘多远」计算（与 Wayland 的 `exclusive_zone` 同口径），
  WM 若不按请求位置摆放窗口，让出的区域可能与面板不重合。
