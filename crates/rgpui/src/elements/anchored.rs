//! 锚定定位元素，将子元素相对于父元素或指定锚点进行精确定位。

use smallvec::SmallVec;

use crate::{
    Anchor, AnyElement, App, Axis, Bounds, Display, Edges, Element, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, ParentElement, Pixels, Point, Position, Size, Style,
    Window, point, px,
};

#[cfg(feature = "dom-backend")]
use crate::DomNode;

/// 锚定元素用于跟踪其子元素的状态。
pub struct AnchoredState {
    child_layout_ids: SmallVec<[LayoutId; 4]>,
}

/// 一个锚定元素，可用于显示会避免超出窗口边界的 UI。
pub struct Anchored {
    children: SmallVec<[AnyElement; 2]>,
    anchor: Anchor,
    fit_mode: AnchoredFitMode,
    anchor_position: Option<Point<Pixels>>,
    position_mode: AnchoredPositionMode,
    offset: Option<Point<Pixels>>,
}

/// 创建一个会避免超出窗口边界的锚定元素。
/// 子元素不应有 margin，以避免测量问题。
pub fn anchored() -> Anchored {
    Anchored {
        children: SmallVec::new(),
        anchor: Anchor::TopLeft,
        fit_mode: AnchoredFitMode::SwitchAnchor,
        anchor_position: None,
        position_mode: AnchoredPositionMode::Window,
        offset: None,
    }
}

impl Anchored {
    /// 设置锚定元素的哪个角应锚定到当前位置。
    pub fn anchor(mut self, anchor: Anchor) -> Self {
        self.anchor = anchor;
        self
    }

    /// 设置窗口坐标中的位置
    /// （否则使用锚定元素渲染的位置）
    pub fn position(mut self, anchor: Point<Pixels>) -> Self {
        self.anchor_position = Some(anchor);
        self
    }

    /// 按指定量偏移最终位置。
    /// 当需要锚定到某个元素但偏移一定距离时很有用，例如在 PopoverMenu 中。
    pub fn offset(mut self, offset: Point<Pixels>) -> Self {
        self.offset = Some(offset);
        self
    }

    /// 设置此锚定元素的位置模式。Local 模式将
    /// [`Anchored::position`] 解释为相对于父元素。
    /// Window 模式将位置解释为相对于窗口。
    pub fn position_mode(mut self, mode: AnchoredPositionMode) -> Self {
        self.position_mode = mode;
        self
    }

    /// 当溢出发生时，吸附到窗口边缘而不是切换锚定角。
    pub fn snap_to_window(mut self) -> Self {
        self.fit_mode = AnchoredFitMode::SnapToWindow;
        self
    }

    /// Snap to window edge and leave some margins.
    pub fn snap_to_window_with_margin(mut self, edges: impl Into<Edges<Pixels>>) -> Self {
        self.fit_mode = AnchoredFitMode::SnapToWindowWithMargin(edges.into());
        self
    }
}

impl ParentElement for Anchored {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements)
    }
}

impl Element for Anchored {
    type RequestLayoutState = AnchoredState;
    type PrepaintState = ();

    fn id(&self) -> Option<crate::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    #[cfg(feature = "dom-backend")]
    fn dom(&self, _bounds: Bounds<Pixels>, _window: &mut Window, _cx: &mut App) -> Option<DomNode> {
        // Anchored 仅做窗口内防溢出定位（canvas 模式通过 `with_element_offset`
        // 偏移子元素）。DOM 模式下子元素各自基于自身布局 bounds 与 `occlude`/
        // 绝对定位自行落位，外层包裹节点无需（也不应）占据 Taffy 原始 bounds——
        // 那会是一个位于视口外（如 y=2500）的空透明容器。故这里返回 None，
        // 让子元素直接挂到上一层 DOM 节点，避免产生离屏空节点。
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (crate::LayoutId, Self::RequestLayoutState) {
        let child_layout_ids = self
            .children
            .iter_mut()
            .map(|child| child.request_layout(window, cx))
            .collect::<SmallVec<_>>();

        let anchored_style = Style {
            position: Position::Absolute,
            display: Display::Flex,
            ..Style::default()
        };

        let layout_id = window.request_layout(anchored_style, child_layout_ids.iter().copied(), cx);

        (layout_id, AnchoredState { child_layout_ids })
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if request_layout.child_layout_ids.is_empty() {
            return;
        }

        let children_bounds = request_layout
            .child_layout_ids
            .iter()
            .map(|id| window.layout_bounds(*id))
            .reduce(|acc, bounds| acc.union(&bounds))
            .unwrap();

        let (origin, mut desired) = self.position_mode.get_position_and_bounds(
            self.anchor_position,
            self.anchor,
            children_bounds.size,
            bounds,
            self.offset,
        );

        let limits = Bounds {
            origin: Point::default(),
            size: window.viewport_size(),
        };

        if self.fit_mode == AnchoredFitMode::SwitchAnchor {
            let mut anchor = self.anchor;

            if desired.left() < limits.left() || desired.right() > limits.right() {
                let switched = Bounds::from_anchor_and_size(
                    anchor.other_side_along(Axis::Horizontal),
                    origin,
                    children_bounds.size,
                );
                if !(switched.left() < limits.left() || switched.right() > limits.right()) {
                    anchor = anchor.other_side_along(Axis::Horizontal);
                    desired = switched
                }
            }

            if desired.top() < limits.top() || desired.bottom() > limits.bottom() {
                let switched = Bounds::from_anchor_and_size(
                    anchor.other_side_along(Axis::Vertical),
                    origin,
                    children_bounds.size,
                );
                if !(switched.top() < limits.top() || switched.bottom() > limits.bottom()) {
                    desired = switched;
                }
            }
        }

        let client_inset = window.client_inset.unwrap_or(px(0.));
        let edges = match self.fit_mode {
            AnchoredFitMode::SnapToWindowWithMargin(edges) => edges,
            _ => Edges::default(),
        }
        .map(|edge| *edge + client_inset);

        // Snap the horizontal edges of the anchored element to the horizontal edges of the window if
        // its horizontal bounds overflow, aligning to the left if it is wider than the limits.
        if desired.right() > limits.right() {
            desired.origin.x -= desired.right() - limits.right() + edges.right;
        }
        if desired.left() < limits.left() {
            desired.origin.x = limits.origin.x + edges.left;
        }

        // Snap the vertical edges of the anchored element to the vertical edges of the window if
        // its vertical bounds overflow, aligning to the top if it is taller than the limits.
        if desired.bottom() > limits.bottom() {
            desired.origin.y -= desired.bottom() - limits.bottom() + edges.bottom;
        }
        if desired.top() < limits.top() {
            desired.origin.y = limits.origin.y + edges.top;
        }

        let offset = desired.origin - bounds.origin;
        let offset = point(offset.x.round(), offset.y.round());

        window.with_element_offset(offset, |window| {
            for child in &mut self.children {
                child.prepaint(window, cx);
            }
        })
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: crate::Bounds<crate::Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        for child in &mut self.children {
            child.paint(window, cx);
        }
    }
}

impl IntoElement for Anchored {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// 将锚定元素适配到窗口内时使用的算法。
#[derive(Copy, Clone, PartialEq)]
pub enum AnchoredFitMode {
    /// 将锚定元素吸附到窗口边缘。
    SnapToWindow,
    /// 吸附到窗口边缘并保留一定边距。
    SnapToWindowWithMargin(Edges<Pixels>),
    /// 切换此锚定元素所连接的锚定角。
    SwitchAnchor,
}

/// 定位锚定元素时使用的算法。
#[derive(Copy, Clone, PartialEq)]
pub enum AnchoredPositionMode {
    /// 相对于窗口定位锚定元素。
    Window,
    /// 相对于父元素定位锚定元素。
    Local,
}

impl AnchoredPositionMode {
    fn get_position_and_bounds(
        &self,
        anchor_position: Option<Point<Pixels>>,
        anchor: Anchor,
        size: Size<Pixels>,
        bounds: Bounds<Pixels>,
        offset: Option<Point<Pixels>>,
    ) -> (Point<Pixels>, Bounds<Pixels>) {
        let offset = offset.unwrap_or_default();

        match self {
            AnchoredPositionMode::Window => {
                let anchor_position = anchor_position.unwrap_or(bounds.origin);
                let bounds = Bounds::from_anchor_and_size(anchor, anchor_position + offset, size);
                (anchor_position, bounds)
            }
            AnchoredPositionMode::Local => {
                let anchor_position = anchor_position.unwrap_or_default();
                let bounds = Bounds::from_anchor_and_size(
                    anchor,
                    bounds.origin + anchor_position + offset,
                    size,
                );
                (anchor_position, bounds)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Context, Pixels, PlatformInput, Point, TestAppContext, Window, deferred, div, point,
        prelude::*, px, size,
    };

    struct AnchoredTestView {
        position: Point<Pixels>,
    }

    impl Render for AnchoredTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                div()
                    .id("scroll-container")
                    .overflow_y_scroll()
                    .size_full()
                    .child(div().h(px(2000.)).w_full())
                    .child(
                        deferred(
                            super::anchored()
                                .snap_to_window()
                                .position(self.position)
                                .child(
                                    div()
                                        .id("menu")
                                        .debug_selector(|| "MENU".into())
                                        .w(px(200.))
                                        .h(px(300.)),
                                ),
                        )
                        .with_priority(1),
                    ),
            )
        }
    }

    #[rgpui::test]
    fn test_anchored_position_without_scroll(cx: &mut TestAppContext) {
        let window = cx.open_window(size(px(800.), px(600.)), |_, _| AnchoredTestView {
            position: point(px(100.), px(100.)),
        });

        cx.run_until_parked();

        let menu_bounds = window
            .update(cx, |_, window, _| {
                window.rendered_frame.debug_bounds.get("MENU").copied()
            })
            .unwrap()
            .expect("MENU debug bounds not found");

        assert_eq!(menu_bounds.origin, point(px(100.), px(100.)));
        assert_eq!(menu_bounds.size, size(px(200.), px(300.)));
    }

    #[rgpui::test]
    fn test_anchored_position_when_scrolled(cx: &mut TestAppContext) {
        let window = cx.open_window(size(px(800.), px(600.)), |_, _| AnchoredTestView {
            position: point(px(100.), px(100.)),
        });

        cx.run_until_parked();

        window
            .update(cx, |_, window, cx| {
                let event = rgpui::ScrollWheelEvent {
                    position: point(px(400.), px(300.)),
                    delta: rgpui::ScrollDelta::Pixels(point(px(0.), px(-1000.))),
                    ..Default::default()
                };
                window.dispatch_event(PlatformInput::ScrollWheel(event), cx);
            })
            .unwrap();

        cx.run_until_parked();

        let menu_bounds = window
            .update(cx, |_, window, _| {
                window.rendered_frame.debug_bounds.get("MENU").copied()
            })
            .unwrap()
            .expect("MENU debug bounds not found");

        assert_eq!(menu_bounds.origin, point(px(100.), px(100.)));
        assert_eq!(menu_bounds.size, size(px(200.), px(300.)));
    }

    #[rgpui::test]
    fn test_anchored_snaps_to_window(cx: &mut TestAppContext) {
        let window = cx.open_window(size(px(800.), px(600.)), |_, _| AnchoredTestView {
            position: point(px(100.), px(500.)),
        });

        cx.run_until_parked();

        let menu_bounds = window
            .update(cx, |_, window, _| {
                window.rendered_frame.debug_bounds.get("MENU").copied()
            })
            .unwrap()
            .expect("MENU debug bounds not found");

        assert_eq!(menu_bounds.origin, point(px(100.), px(300.)));
        assert_eq!(menu_bounds.size, size(px(200.), px(300.)));
    }
}
