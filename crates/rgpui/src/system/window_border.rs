//! 窗口边框工具 - 计算客户端装饰窗口的内容内边距。

use crate::{Decorations, Edges, Pixels, Tiling, Window, px};

/// 客户端装饰的阴影尺寸（像素）。非 Linux 平台为 0，Linux 上为 20。
#[cfg(not(target_os = "linux"))]
pub(crate) const SHADOW_SIZE: Pixels = px(0.0);
#[cfg(target_os = "linux")]
pub(crate) const SHADOW_SIZE: Pixels = px(20.0);

/// 计算可见框架距离窗口外边界的各边内边距。
fn client_frame_insets(shadow_size: Pixels, tiling: &Tiling) -> Edges<Pixels> {
    let mut insets = Edges::all(shadow_size);
    if tiling.top {
        insets.top = px(0.0);
    }
    if tiling.bottom {
        insets.bottom = px(0.0);
    }
    if tiling.left {
        insets.left = px(0.0);
    }
    if tiling.right {
        insets.right = px(0.0);
    }
    insets
}

/// 获取窗口内容的内边距。
///
/// 服务端装饰返回全零；客户端装饰根据窗口平铺状态计算各边阴影尺寸。
pub fn window_paddings(window: &Window) -> Edges<Pixels> {
    let shadow_size = window.client_inset().unwrap_or(SHADOW_SIZE);
    match window.window_decorations() {
        Decorations::Server => Edges::all(px(0.0)),
        Decorations::Client { tiling } => client_frame_insets(shadow_size, &tiling),
    }
}
