use crate::{App, Bounds, IntoElement, ParentElement, Pixels, Styled as _, Window, canvas};

/// 用于扩展 [`crate::ParentElement`] 元素的额外功能。
pub trait ElementExt: ParentElement + Sized {
    /// 添加一个 prepaint 回调到元素。
    ///
    /// 这是一个辅助方法，用于在元素绘制后获取其边界。
    ///
    /// 第一个参数为元素的像素边界。
    ///
    /// 参见 [`canvas`](crate::canvas)。
    fn on_prepaint<F>(self, f: F) -> Self
    where
        F: FnOnce(Bounds<Pixels>, &mut Window, &mut App) + 'static,
    {
        self.child(
            canvas(
                move |bounds, window, cx| f(bounds, window, cx),
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
    }
}

impl<T: ParentElement> ElementExt for T {}