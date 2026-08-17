//! The GPUI prelude is a collection of traits and types that are widely used
//! throughout the library. It is recommended to import this prelude into your
//! application to avoid having to import each trait individually.

pub use crate::{
    ActiveTheme, AppContext as _, BorrowAppContext, Context, Element, ElementExt,
    InteractiveElement, InteractiveElementExt, IntoElement, ParentElement, Refineable, Render,
    RenderOnce, Selectable, Sizable, StatefulInteractiveElement, Styled, StyledExt, StyledImage,
    TaskExt as _, VisualContext, util::FluentBuilder,
};
