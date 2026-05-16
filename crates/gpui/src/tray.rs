use crate::{App, Image, MenuItem, SharedString, SvgRenderer};
use anyhow::Result;
use std::rc::Rc;

/// System tray icon.
#[derive(Clone)]
pub struct Tray {
    /// Tooltip text.
    pub tooltip: Option<SharedString>,
    /// Tray icon image.
    pub icon: Option<Rc<Image>>,
    /// Rendered icon data for platform use.
    pub icon_data: Option<TrayIconData>,
    /// Function to build the context menu.
    pub menu_builder: Option<Rc<dyn Fn(&mut App) -> Vec<MenuItem>>>,
    /// Visibility of the tray icon.
    pub visible: bool,
}

impl Tray {
    /// Render the icon to platform-compatible icon data.
    pub fn render_icon(&mut self, svg_renderer: SvgRenderer) -> Result<()> {
        if let Some(icon) = &self.icon {
            let image = icon.to_image_data(svg_renderer)?;
            let bytes = image.as_bytes(0).unwrap_or_default();
            let size = image.size(0);

            self.icon_data = Some(TrayIconData {
                data: Rc::new(bytes.to_vec()),
                width: size.width.0 as u32,
                height: size.height.0 as u32,
            })
        }
        Ok(())
    }
}

/// Rendered icon data for the tray.
#[derive(Clone)]
pub struct TrayIconData {
    /// Raw RGBA image data.
    pub data: Rc<Vec<u8>>,
    /// Width of the icon in pixels.
    pub width: u32,
    /// Height of the icon in pixels.
    pub height: u32,
}

impl Tray {
    /// Create a new tray icon with default properties.
    pub fn new() -> Self {
        Self {
            tooltip: None,
            icon: None,
            icon_data: None,
            menu_builder: None,
            visible: true,
        }
    }

    /// Set the tooltip text, defaults to None.
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Set the icon image, defaults to None.
    pub fn icon(mut self, icon: impl Into<Image>) -> Self {
        self.icon = Some(Rc::new(icon.into()));
        self
    }

    /// Set the context menu.
    pub fn menu<F>(mut self, builder: F) -> Self
    where
        F: Fn(&mut App) -> Vec<MenuItem> + 'static,
    {
        self.menu_builder = Some(Rc::new(builder));
        self
    }

    /// Set visibility of the tray icon, default is true.
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
}
