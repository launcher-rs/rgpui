//! 元素扩展 trait，为所有元素提供便捷的构建方法和交互绑定。

use crate::{
    AnyElement, App, Bounds, ClickEvent, ElementSize, InteractiveElement, IntoElement,
    ParentElement, Pixels, Sizable, Stateful, Styled as _, Window, canvas,
};

/// 子元素尺寸选项。
#[derive(Default)]
struct ChildElementOptions {
    ix: usize,
    size: ElementSize,
}

/// 可携带索引的子元素 trait。
///
/// 用于表等容器组件中，为每个子元素分配其在父容器中的索引。
pub trait ChildElement: Sizable + IntoElement {
    /// 为子元素设置其在父容器中的索引。
    fn with_ix(self, ix: usize) -> Self;
}

/// 类型擦除的子元素，可在渲染前接受 [`ChildElementOptions`]。
pub struct AnyChildElement(Box<dyn FnOnce(ChildElementOptions) -> AnyElement>);

impl AnyChildElement {
    /// 将实现了 [`ChildElement`] 的元素包装为类型擦除的子元素。
    pub fn new(element: impl ChildElement + 'static) -> Self {
        Self(Box::new(|options| {
            element
                .with_ix(options.ix)
                .with_size(options.size)
                .into_any_element()
        }))
    }

    /// 将类型擦除的子元素转为实际的元素，传入索引与尺寸。
    pub fn into_any(self, ix: usize, size: ElementSize) -> AnyElement {
        (self.0)(ChildElementOptions { ix, size })
    }
}

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

/// 用于扩展 [`crate::InteractiveElement`] 的额外事件方法。
pub trait InteractiveElementExt: InteractiveElement {
    /// 设置双击事件的监听器。
    ///
    /// 在点击计数为 2 时触发给定的回调。
    fn on_double_click(
        mut self,
        listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self
    where
        Self: Sized,
    {
        self.interactivity().on_click(move |event, window, cx| {
            if event.click_count() == 2 {
                listener(event, window, cx);
            }
        });
        self
    }
}

impl<E: InteractiveElement> InteractiveElementExt for Stateful<E> {}
