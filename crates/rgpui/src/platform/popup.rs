use bitflags::bitflags;
use thiserror::Error;

use crate::{AnyWindowHandle, Bounds, Pixels, Point};

/// 父窗口锚定弹出窗口的选项，如菜单、下拉列表、上下文菜单或工具提示。
///
/// 弹出窗口相对于其父窗口的锚定矩形放置，而不是在绝对屏幕位置。
/// 平台解析最终位置，因此这在合成器拥有窗口放置的系统（Wayland）和具有绝对坐标的平台上
/// 都能工作。
///
/// 弹出窗口的大小来自 [`WindowOptions::window_bounds`](crate::WindowOptions)，
/// 其原点被忽略。所有坐标均以逻辑像素为单位。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopupOptions {
    /// 弹出窗口所锚定的窗口。
    pub parent: AnyWindowHandle,

    /// 弹出窗口相对定位的矩形，在父窗口的坐标空间中
    /// （与元素边界相同的空间）。例如，下拉菜单使用打开它的按钮的边界。
    pub anchor_rect: Bounds<Pixels>,

    /// [`Self::anchor_rect`] 的哪个点作为弹出窗口的锚点。
    pub anchor: PopupAnchor,

    /// 弹出窗口从锚点向外延伸的方向。下拉到其按钮下方的下拉菜单使用
    /// [`PopupAnchor::BottomLeft`] 锚点，重力为
    /// [`PopupGravity::BottomRight`]，使其向下向右增长。
    pub gravity: PopupGravity,

    /// 如果请求的位置会使弹出窗口超出屏幕，平台如何调整它。
    pub constraint_adjustment: PopupConstraintAdjustment,

    /// 锚定后应用于弹出窗口的额外偏移量。
    pub offset: Point<Pixels>,

    /// 弹出窗口是否应获取显式输入抓取。
    ///
    /// 抓取的弹出窗口行为类似菜单：它们获取键盘焦点，当用户在其外部点击
    /// 或按下关闭键时关闭。将其用于菜单和组合框，而不是工具提示或其他被动弹出窗口。
    ///
    /// 抓取必须在触发输入仍处于活动状态时请求，实际上是在打开弹出窗口的
    /// 鼠标按钮按下时。从 mouse-down 处理器而非 click 处理器打开抓取弹出窗口，
    /// 否则抓取会被拒绝。
    ///
    /// 自动关闭仅覆盖针对其他应用程序的输入。在你自己应用程序中的其他位置点击
    /// 仍会照常到达，因此关闭弹出窗口取决于你。嵌套的抓取弹出窗口必须按
    /// 打开的相反顺序关闭。
    pub grab: bool,
}

/// 弹出窗口锚定到的锚定矩形的点。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PopupAnchor {
    /// 锚定到锚定矩形的中心。
    #[default]
    Center,
    /// 锚定到顶边的中心。
    Top,
    /// 锚定到底边的中心。
    Bottom,
    /// 锚定到左边的中心。
    Left,
    /// 锚定到右边的中心。
    Right,
    /// 锚定到左上角。
    TopLeft,
    /// 锚定到左下角。
    BottomLeft,
    /// 锚定到右上角。
    TopRight,
    /// 锚定到右下角。
    BottomRight,
}

/// 弹出窗口从其锚点向外延伸的方向。
///
/// 例如，[`PopupGravity::BottomRight`] 将弹出窗口放置在锚点的下方和右侧。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PopupGravity {
    /// 弹出窗口居中于锚点上方。
    #[default]
    Center,
    /// 弹出窗口从锚点向上延伸。
    Top,
    /// 弹出窗口从锚点向下延伸。
    Bottom,
    /// 弹出窗口从锚点向左延伸。
    Left,
    /// 弹出窗口从锚点向右延伸。
    Right,
    /// 弹出窗口从锚点向左上方延伸。
    TopLeft,
    /// 弹出窗口从锚点向左下方延伸。
    BottomLeft,
    /// 弹出窗口从锚点向右上方延伸。
    TopRight,
    /// 弹出窗口从锚点向右下方延伸。
    BottomRight,
}

bitflags! {
    /// 如果请求的位置会使弹出窗口超出屏幕，平台如何调整它。
    /// 如果未设置标志，弹出窗口将按请求放置，并可能被裁剪。
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    pub struct PopupConstraintAdjustment: u32 {
        /// 弹出窗口可以水平滑动以保持在屏幕内。
        const SLIDE_X = 1;
        /// 弹出窗口可以垂直滑动以保持在屏幕内。
        const SLIDE_Y = 2;
        /// 弹出窗口的锚点和重力可以水平翻转以保持在屏幕内。
        const FLIP_X = 4;
        /// 弹出窗口的锚点和重力可以垂直翻转以保持在屏幕内。
        const FLIP_Y = 8;
        /// 弹出窗口可以水平缩小以保持在屏幕内。
        const RESIZE_X = 16;
        /// 弹出窗口可以垂直缩小以保持在屏幕内。
        const RESIZE_Y = 32;
    }
}

/// 当前平台尚无原生弹出窗口实现时返回。
///
/// 原生弹出窗口与 rgpui 的窗口内弹出层不同，后者作为元素绘制在现有窗口内。
/// 希望在所有平台上都有弹出窗口的调用者应将此错误视为回退到该窗口内渲染的提示。
#[derive(Debug, Error)]
#[error("popups are not supported on this platform")]
pub struct PopupNotSupportedError;
