//! Scrollable component with visible scrollbars.

use super::scrollbar::{Scrollbar, ScrollbarAxis, ScrollbarState};
use rgpui::{
    AnyElement, App, Bounds, Div, Element, ElementId, GlobalElementId, InspectorElementId,
    InteractiveElement, Interactivity, IntoElement, LayoutId, ParentElement, Pixels, Position,
    ScrollHandle, SharedString, Stateful, StatefulInteractiveElement, Style, StyleRefinement,
    Styled, Window, div, relative,
};

/// A scroll view with visible scrollbars
pub struct Scrollable {
    id: ElementId,
    element: Option<AnyElement>,
    axis: ScrollbarAxis,
    always_show_scrollbars: bool,
    external_scroll_handle: Option<ScrollHandle>,
    _element: Stateful<Div>,
}

impl Scrollable {
    pub(crate) fn new(axis: ScrollbarAxis, element: impl IntoElement) -> Self {
        let element = element.into_any_element();
        let id = ElementId::Name(SharedString::from(format!("scrollable-{:?}", element.id())));

        Self {
            element: Some(element),
            _element: div().id("fake"),
            id,
            axis,
            always_show_scrollbars: false,
            external_scroll_handle: None,
        }
    }

    pub fn vertical(element: impl IntoElement) -> Self {
        Self::new(ScrollbarAxis::Vertical, element)
    }

    pub fn horizontal(element: impl IntoElement) -> Self {
        Self::new(ScrollbarAxis::Horizontal, element)
    }

    pub fn both(element: impl IntoElement) -> Self {
        Self::new(ScrollbarAxis::Both, element)
    }

    pub fn always_show_scrollbars(mut self) -> Self {
        self.always_show_scrollbars = true;
        self
    }

    pub fn with_scroll_handle(mut self, handle: ScrollHandle) -> Self {
        self.external_scroll_handle = Some(handle);
        self
    }

    fn with_element_state<R>(
        &mut self,
        id: &GlobalElementId,
        window: &mut Window,
        cx: &mut App,
        f: impl FnOnce(&mut Self, &mut ScrollViewState, &mut Window, &mut App) -> R,
    ) -> R {
        window.with_optional_element_state::<ScrollViewState, _>(
            Some(id),
            |element_state, window| {
                let mut element_state = element_state.unwrap().unwrap_or_default();
                let result = f(self, &mut element_state, window, cx);
                (result, Some(element_state))
            },
        )
    }
}

pub struct ScrollViewState {
    state: ScrollbarState,
    handle: ScrollHandle,
}

impl Default for ScrollViewState {
    fn default() -> Self {
        Self {
            handle: ScrollHandle::new(),
            state: ScrollbarState::default(),
        }
    }
}

impl ParentElement for Scrollable {
    fn extend(&mut self, _elements: impl IntoIterator<Item = AnyElement>) {
        // AnyElement doesn't support extending with children directly
        // This is a no-op for Scrollable
    }
}

impl Styled for Scrollable {
    fn style(&mut self) -> &mut StyleRefinement {
        self._element.style()
    }
}

impl InteractiveElement for Scrollable {
    fn interactivity(&mut self) -> &mut Interactivity {
        self._element.interactivity()
    }
}

impl StatefulInteractiveElement for Scrollable {}

impl IntoElement for Scrollable {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Scrollable {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ScrollViewState;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = Style {
            flex_grow: 1.0,
            position: Position::Relative,
            size: rgpui::Size {
                width: relative(1.0).into(),
                height: relative(1.0).into(),
            },
            ..Style::default()
        };

        let axis = self.axis;
        let scroll_id = self.id.clone();
        let content = self.element.take();
        let always_show = self.always_show_scrollbars;

        self.with_element_state(
            id.unwrap(),
            window,
            cx,
            |scrollable, element_state, window, cx| {
                let scroll_handle =
                    if let Some(ref external_handle) = scrollable.external_scroll_handle {
                        external_handle
                    } else {
                        &element_state.handle
                    };

                let mut scrollbar = Scrollbar::new(axis, &element_state.state, scroll_handle);
                if always_show {
                    scrollbar = scrollbar.always_visible();
                }

                let mut element = div()
                    .relative()
                    .size_full()
                    .overflow_hidden()
                    .child(
                        div()
                            .id(scroll_id)
                            .track_scroll(scroll_handle)
                            .overflow_scroll()
                            .relative()
                            .size_full()
                            .child(div().children(content)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .bottom_0()
                            .child(scrollbar),
                    )
                    .into_any_element();

                let element_id = element.request_layout(window, cx);
                let layout_id = window.request_layout(style, vec![element_id], cx);

                (layout_id, element)
            },
        )
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        element.prepaint(window, cx);
        ScrollViewState::default()
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        element.paint(window, cx)
    }
}

pub fn scrollable_vertical(element: impl IntoElement) -> Scrollable {
    Scrollable::vertical(element)
}

pub fn scrollable_horizontal(element: impl IntoElement) -> Scrollable {
    Scrollable::horizontal(element)
}

pub fn scrollable_both(element: impl IntoElement) -> Scrollable {
    Scrollable::both(element)
}
