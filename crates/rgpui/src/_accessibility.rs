//! # Accessibility in GPUI
//!
//! "Accessibility" refers to the ability of your application to be used by all
//! users, regardless of disability status. One common form of accessibility is
//! screen reader support, which allows users with visual impairments to use
//! software via text-to-speech or refreshable braille displays.
//!
//! GPUI uses [AccessKit](https://accesskit.dev/) to provide accessibility
//! support on all supported platforms (macOS, Windows, Linux, and Web).
//! AccessKit is a cross-platform accessibility framework that translates your
//! UI into a platform-agnostic tree of nodes, which is then bridged to native
//! accessibility APIs (e.g., NSAccessibility on macOS, UI Automation on
//! Windows, AT-SPI on Linux).
//!
//! ## Enabling Accessibility
//!
//! Accessibility support is built into GPUI by default. When a screen reader
//! (e.g., VoiceOver, Narrator, Orca) is connected, GPUI automatically begins
//! building and updating the accessibility tree.
//!
//! ## Element IDs and the Accessibility Tree
//!
//! Every accessible node in GPUI must have a stable, unique ID. This is
//! provided by the [`ElementId`] system:
//!
//! - Use the `.id()` method on elements to assign an ID (e.g., `div().id("my-button")`).
//! - For list items or repeated elements, use a tuple like `.id(("task", i))` to
//!   create unique IDs that include an index.
//! - IDs are scoped to the window, so duplicate IDs across different views are
//!   acceptable as long as they reside in different windows.
//!
//! ## Roles and Properties
//!
//! Use the `.role()` method to assign an ARIA/compatible role to an element:
//!
//! ```ignore
//! div().id("counter").role(Role::SpinButton)
//! ```
//!
//! Supported roles include (but are not limited to):
//! - `Role::Button`
//! - `Role::Switch`
//! - `Role::SpinButton`
//! - `Role::Slider`
//! - `Role::Heading`
//! - `Role::List`
//! - `Role::ListItem`
//! - `Role::Application`
//! - `Role::TextField`
//!
//! Additional accessibility properties can be set with:
//! - `.aria_label(...)` — a human-readable label for the element
//! - `.aria_toggled(...)` — the toggle state (`Toggled::True`, `Toggled::False`, or `Toggled::Mixed`)
//! - `.aria_numeric_value(...)` — a numeric value (e.g., for sliders or spin buttons)
//! - `.aria_min_numeric_value(...)` / `.aria_max_numeric_value(...)` — numeric bounds
//! - `.aria_level(...)` — heading level (e.g., 1–6 for headings)
//! - `.aria_position_in_set(...)` / `.aria_size_of_set(...)` — for list items
//! - `.focusable()` — make the element reachable via keyboard navigation
//! - `.tab_stop(true)` — include the element in the tab order
//!
//! ## Handling Accessibility Actions
//!
//! Screen readers can request actions on elements (e.g., "increment" on a
//! spin button, "press" on a button). Use the `.on_a11y_action()` method to
//! handle these:
//!
//! ```ignore
//! .on_a11y_action(AccessibleAction::Increment, { /* handler */ })
//! ```
//!
//! The available actions include:
//! - `AccessibleAction::Increment`
//! - `AccessibleAction::Decrement`
//! - `AccessibleAction::Press`
//! - `AccessibleAction::Focus`
//! - `AccessibleAction::ScrollDown`
//! - `AccessibleAction::ScrollUp`
//! - And others as defined by the AccessKit library.
//!
//! ## Focus Management
//!
//! Accessible elements should be part of the focus hierarchy:
//! - Use `.focusable()` to make an element focusable.
//! - Use `.tab_stop(true)` to include it in the Tab key navigation order.
//! - The `on_a11y_action(AccessibleAction::Focus, ...)` handler will fire when
//!   the screen reader moves focus to the element.
//!
//! Window-level focus navigation is handled by the window itself via
//! `window.focus_next(cx)` and `window.focus_prev(cx)`.
//!
//! ## Example
//!
//! See the `a11y` example (`cargo run -p rgpui --example a11y`) for a
//! complete demonstration of accessibility features in GPUI, including:
//!
//! - A `Role::SpinButton` with increment/decrement actions
//! - A `Role::Switch` with toggle state
//! - A `Role::List` with labeled list items
//! - Keyboard navigation with Tab/Shift-Tab
//! - Proper use of `ElementId` with compound IDs for repeated elements
//!
//! ## Platform Notes
//!
//! - **macOS**: AccessKit bridges to `NSAccessibility`. VoiceOver is the
//!   primary screen reader.
//! - **Windows**: AccessKit bridges to UI Automation. Narrator and third-party
//!   screen readers are supported.
//! - **Linux**: AccessKit bridges to AT-SPI. Orca is the primary screen reader.
//! - **Web**: Accessibility is provided via the DOM, and AccessKit is not used
//!   on this platform.
