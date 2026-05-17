use bitflags::bitflags;
use thiserror::Error;

use crate::Pixels;

/// 表面渲染的层级。多个表面可以共享同一层级，单个层级内的排序是未定义的。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Layer {
    /// 背景层级，通常用于壁纸。
    Background,

    /// 底部层级。
    Bottom,

    /// 顶部层级，通常用于全屏窗口。
    Top,

    /// 覆盖层级，用于应始终在最上面的表面。
    #[default]
    Overlay,
}

bitflags! {
    /// layer_shell 表面的屏幕锚点。这些可以任意组合使用，例如
    /// 指定 `Anchor::LEFT | Anchor::RIGHT` 将使表面横跨屏幕宽度。
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    pub struct Anchor: u32 {
        /// 锚定到屏幕顶部边缘。
        const TOP = 1;
        /// 锚定到屏幕底部边缘。
        const BOTTOM = 2;
        /// 锚定到屏幕左侧边缘。
        const LEFT = 4;
        /// 锚定到屏幕右侧边缘。
        const RIGHT = 8;
    }
}

/// layer_shell 表面的键盘交互模式。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum KeyboardInteractivity {
    /// 不会向表面传递任何键盘输入，且无法接收键盘焦点。
    None,

    /// 只要表面位于 shell 表面层级之上，且没有其他 layer_shell 表面在其上方，
    /// 表面将接收独占键盘焦点。
    Exclusive,

    /// 表面可以像普通窗口一样被聚焦。
    #[default]
    OnDemand,
}

/// 创建 layer_shell 窗口的选项。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LayerShellOptions {
    /// 表面的命名空间，主要由合成器用于应用规则，表面创建后无法更改。
    pub namespace: String,
    /// 表面渲染的层级。
    pub layer: Layer,
    /// 表面的锚点。
    pub anchor: Anchor,
    /// 请求合成器避免用其他表面遮挡某个区域。
    pub exclusive_zone: Option<Pixels>,
    /// 独占区域的锚点，如果未指定将根据 anchor 确定。
    pub exclusive_edge: Option<Anchor>,
    /// 表面与其锚点之间的边距。
    /// 按 CSS 顺序指定：上、右、下、左。
    pub margin: Option<(Pixels, Pixels, Pixels, Pixels)>,
    /// 键盘事件应如何传递给表面。
    pub keyboard_interactivity: KeyboardInteractivity,
}

/// 表示由于合成器不支持所需的 layer_shell 协议而导致操作失败的错误。
#[derive(Debug, Error)]
#[error("Compositor doesn't support zwlr_layer_shell_v1")]
pub struct LayerShellNotSupportedError;
