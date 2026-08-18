//! 直接绘图画布：支持 Styled 样式的底层绘制表面。

use crate::{prelude::FluentBuilder as _, *};
use std::rc::Rc;

/// 绘制回调类型。
type PaintCallback = Rc<dyn Fn(Bounds<Pixels>, &mut Window, &mut App)>;
/// 预备绘制回调类型。
type PrepareCallback = Rc<dyn Fn(Bounds<Pixels>, &mut Window, &mut App)>;

/// 画布组件：通过回调提供直接绘制能力。
#[derive(IntoElement)]
pub struct CanvasComponent {
    /// 元素 ID。
    id: ElementId,
    /// 绘制回调。
    on_paint: Option<PaintCallback>,
    /// 预备绘制回调。
    on_prepare: Option<PrepareCallback>,
    /// 用户样式。
    style: StyleRefinement,
}

impl CanvasComponent {
    /// 创建画布组件。
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            on_paint: None,
            on_prepare: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置绘制回调。
    pub fn on_paint(
        mut self,
        callback: impl Fn(Bounds<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_paint = Some(Rc::new(callback));
        self
    }

    /// 设置预备绘制回调。
    pub fn on_prepare(
        mut self,
        callback: impl Fn(Bounds<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_prepare = Some(Rc::new(callback));
        self
    }
}

impl Styled for CanvasComponent {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for CanvasComponent {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let user_style = self.style;
        let paint_cb = self.on_paint;
        let prepare_cb = self.on_prepare;

        div()
            .id(self.id)
            .relative()
            .child(
                canvas(
                    move |bounds, window, cx| {
                        if let Some(ref cb) = prepare_cb {
                            cb(bounds, window, cx);
                        }
                    },
                    move |bounds, _, window, cx| {
                        if let Some(ref cb) = paint_cb {
                            cb(bounds, window, cx);
                        }
                    },
                )
                .absolute()
                .inset_0()
                .size_full(),
            )
            .map(|this| {
                let mut el = this;
                el.style().refine(&user_style);
                el
            })
    }
}
