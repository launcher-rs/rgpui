use super::ListItem;
use crate::{IntoElement, ParentElement as _, RenderOnce, Styled};
use crate::{Skeleton, v_flex};

/// 列表加载占位视图。
#[derive(IntoElement)]
pub struct Loading;

#[derive(IntoElement)]
struct LoadingItem;

impl RenderOnce for LoadingItem {
    fn render(self, _window: &mut crate::Window, _cx: &mut crate::App) -> impl IntoElement {
        ListItem::new("skeleton").disabled(true).child(
            v_flex()
                .gap_1p5()
                .overflow_hidden()
                .child(Skeleton::new().h_5().w_48().max_w_full())
                .child(Skeleton::new().secondary().h_3().w_64().max_w_full()),
        )
    }
}

impl RenderOnce for Loading {
    fn render(self, _window: &mut crate::Window, _cx: &mut crate::App) -> impl IntoElement {
        v_flex()
            .py_2p5()
            .gap_3()
            .child(LoadingItem)
            .child(LoadingItem)
            .child(LoadingItem)
    }
}
