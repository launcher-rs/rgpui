//! Terminal element for rendering terminal content in rgpui.
//!
//! This module provides the `TerminalElement` struct that implements rgpui's `Element` trait
//! to render terminal content. It handles:
//! - Batching adjacent cells with the same style for efficient rendering
//! - Color conversion from Alacritty's color format to rgpui's Hsla
//! - Cell flag handling (bold, italic, underline, strikethrough, dim)
//! - Cursor rendering with support for block, bar, and underline shapes
//!
//! # Architecture
//!
//! The rendering pipeline consists of three phases:
//! 1. `request_layout` - Returns layout ID with the requested size
//! 2. `prepaint` - Computes layout state: batched text runs, background rects, cursor
//! 3. `paint` - Renders backgrounds, text runs, and cursor
//!
//! # Example
//!
//! ```ignore
//! let element = TerminalElement::new(terminal_entity, focus_handle, true, true, TextStyle::default());
//! ```

use std::mem;

use alacritty_terminal::{
    index::Point as AlacPoint,
    selection::SelectionRange,
    term::{TermMode, cell::Flags},
    vte::ansi::{Color as AnsiColor, CursorShape as AlacCursorShape, NamedColor},
};
use itertools::Itertools;
use rgpui::{
    AbsoluteLength, App, Bounds, ContentMask, Element, ElementId, Entity, FocusHandle, Font,
    FontStyle, FontWeight, GlobalElementId, Hitbox, Hsla, InputHandler, IntoElement, LayoutId,
    PathBuilder, Pixels, Point, Rgba, ShapedLine, StrikethroughStyle, TextRun, UTF16Selection,
    UnderlineStyle, Window, fill, point, px, size,
};

use crate::{
    IndexedCell, Terminal, TerminalBounds, TerminalConfig, TerminalContent, TerminalTheme,
};

const SCROLLBAR_WIDTH: f32 = 8.0;
const SCROLLBAR_THUMB_MIN_HEIGHT: f32 = 20.0;

/// Layout state computed during prepaint, used for painting.
pub struct LayoutState {
    hitbox: Hitbox,
    batched_text_runs: Vec<BatchedTextRun>,
    background_rects: Vec<LayoutRect>,
    block_fragments: Vec<BlockFragment>,
    polygon_fragments: Vec<PolygonFragment>,
    selection_rects: Vec<LayoutRect>,
    cursor: Option<CursorLayout>,
    background_color: Hsla,
    foreground_color: Hsla,
    dimensions: TerminalBounds,
    mode: TermMode,
    history_size: usize,
    display_offset: usize,
}

/// Helper for converting Alacritty cursor points to display coordinates.
struct DisplayCursor {
    line: i32,
    col: usize,
}

impl DisplayCursor {
    fn from(cursor_point: AlacPoint, display_offset: usize) -> Self {
        Self {
            line: cursor_point.line.0 + display_offset as i32,
            col: cursor_point.column.0,
        }
    }

    fn line(&self) -> i32 {
        self.line
    }

    fn col(&self) -> usize {
        self.col
    }
}

/// A batched text run combining adjacent cells with identical styles.
///
/// Batching reduces draw calls by combining multiple characters that share
/// the same font, color, and decoration properties into a single text shape.
#[derive(Debug)]
pub struct BatchedTextRun {
    pub start_point: AlacPoint<i32, i32>,
    pub text: String,
    pub cell_count: usize,
    pub style: TextRun,
    pub font_size: AbsoluteLength,
}

impl BatchedTextRun {
    fn new_from_char(
        start_point: AlacPoint<i32, i32>,
        c: char,
        style: TextRun,
        font_size: AbsoluteLength,
    ) -> Self {
        // Pre-allocate capacity for typical terminal line runs
        let mut text = String::with_capacity(128);
        text.push(c);
        BatchedTextRun {
            start_point,
            text,
            cell_count: 1,
            style,
            font_size,
        }
    }

    fn can_append(&self, other_style: &TextRun) -> bool {
        self.style.font == other_style.font
            && self.style.color == other_style.color
            && self.style.background_color == other_style.background_color
            && self.style.underline == other_style.underline
            && self.style.strikethrough == other_style.strikethrough
    }

    fn append_char(&mut self, c: char) {
        self.append_char_internal(c, true);
    }

    fn append_zero_width_chars(&mut self, chars: &[char]) {
        for &c in chars {
            self.append_char_internal(c, false);
        }
    }

    fn append_char_internal(&mut self, c: char, counts_cell: bool) {
        self.text.push(c);
        if counts_cell {
            self.cell_count += 1;
        }
        self.style.len += c.len_utf8();
    }

    fn paint(
        &self,
        origin: Point<Pixels>,
        dimensions: &TerminalBounds,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Pixel-snap text position to avoid subpixel rendering blur on Windows
        let pos = Point::new(
            (origin.x + self.start_point.column as f32 * dimensions.cell_width).round(),
            (origin.y + self.start_point.line as f32 * dimensions.line_height).round(),
        );

        let _ = window
            .text_system()
            .shape_line(
                self.text.clone().into(),
                self.font_size.to_pixels(window.rem_size()),
                std::slice::from_ref(&self.style),
                Some(dimensions.cell_width),
            )
            .paint(
                pos,
                dimensions.line_height,
                rgpui::TextAlign::Left,
                None,
                window,
                cx,
            );
    }
}

/// A background rectangle for cells with non-default background colors.
#[derive(Clone, Debug, Default)]
pub struct LayoutRect {
    point: AlacPoint<i32, i32>,
    num_of_cells: usize,
    color: Hsla,
}

impl LayoutRect {
    fn new(point: AlacPoint<i32, i32>, num_of_cells: usize, color: Hsla) -> LayoutRect {
        LayoutRect {
            point,
            num_of_cells,
            color,
        }
    }

    fn paint(&self, origin: Point<Pixels>, dimensions: &TerminalBounds, window: &mut Window) {
        let position = {
            let alac_point = self.point;
            point(
                (origin.x + alac_point.column as f32 * dimensions.cell_width).floor(),
                (origin.y + alac_point.line as f32 * dimensions.line_height).floor(),
            )
        };
        let rect_size = point(
            (dimensions.cell_width * self.num_of_cells as f32).ceil(),
            dimensions.line_height,
        )
        .into();

        window.paint_quad(fill(Bounds::new(position, rect_size), self.color));
    }
}

#[derive(Clone, Copy, Debug)]
struct BlockRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Clone, Copy, Debug)]
struct BlockFragment {
    point: AlacPoint<i32, i32>,
    rect: BlockRect,
    color: Hsla,
}

impl BlockFragment {
    fn paint(&self, origin: Point<Pixels>, dimensions: &TerminalBounds, window: &mut Window) {
        let cell_origin = point(
            (origin.x + self.point.column as f32 * dimensions.cell_width).floor(),
            (origin.y + self.point.line as f32 * dimensions.line_height).floor(),
        );
        let position = point(
            (cell_origin.x + self.rect.x * dimensions.cell_width).round(),
            (cell_origin.y + self.rect.y * dimensions.line_height).round(),
        );
        let rect_size = size(
            (dimensions.cell_width * self.rect.width).ceil(),
            (dimensions.line_height * self.rect.height).ceil(),
        );
        window.paint_quad(fill(Bounds::new(position, rect_size), self.color));
    }
}

#[derive(Clone, Copy, Debug)]
struct BlockPoint {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy, Debug)]
struct PolygonFragment {
    point: AlacPoint<i32, i32>,
    points: [BlockPoint; 3],
    color: Hsla,
}

impl PolygonFragment {
    fn paint(&self, origin: Point<Pixels>, dimensions: &TerminalBounds, window: &mut Window) {
        let cell_origin = point(
            (origin.x + self.point.column as f32 * dimensions.cell_width).floor(),
            (origin.y + self.point.line as f32 * dimensions.line_height).floor(),
        );
        let to_point = |p: BlockPoint| {
            point(
                (cell_origin.x + p.x * dimensions.cell_width).round(),
                (cell_origin.y + p.y * dimensions.line_height).round(),
            )
        };

        let mut builder = PathBuilder::fill();
        builder.move_to(to_point(self.points[0]));
        builder.line_to(to_point(self.points[1]));
        builder.line_to(to_point(self.points[2]));
        builder.close();

        if let Ok(path) = builder.build() {
            window.paint_path(path, self.color);
        }
    }
}

/// Cursor rendering information.
pub struct CursorLayout {
    origin: Point<Pixels>,
    block_width: Pixels,
    line_height: Pixels,
    color: Hsla,
    shape: CursorShape,
    text: Option<ShapedLine>,
}

/// Supported cursor shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    Block,
    Hollow,
    Bar,
    Underline,
}

impl CursorLayout {
    fn new(
        origin: Point<Pixels>,
        block_width: Pixels,
        line_height: Pixels,
        color: Hsla,
        shape: CursorShape,
        text: Option<ShapedLine>,
    ) -> Self {
        CursorLayout {
            origin,
            block_width,
            line_height,
            color,
            shape,
            text,
        }
    }

    fn bounds(&self, origin: Point<Pixels>) -> Bounds<Pixels> {
        Bounds::new(
            origin + self.origin,
            size(self.block_width, self.line_height),
        )
    }

    fn paint(&self, origin: Point<Pixels>, window: &mut Window, cx: &mut App) {
        let bounds = self.bounds(origin);

        match self.shape {
            CursorShape::Block => {
                window.paint_quad(fill(bounds, self.color));
                if let Some(text) = &self.text {
                    let _ = text.paint(
                        bounds.origin,
                        self.line_height,
                        rgpui::TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                }
            }
            CursorShape::Hollow => {
                window.paint_quad(rgpui::outline(
                    bounds,
                    self.color,
                    rgpui::BorderStyle::Solid,
                ));
            }
            CursorShape::Bar => {
                let bar_bounds = Bounds::new(bounds.origin, size(px(2.0), bounds.size.height));
                window.paint_quad(fill(bar_bounds, self.color));
            }
            CursorShape::Underline => {
                let underline_bounds = Bounds::new(
                    point(
                        bounds.origin.x,
                        bounds.origin.y + bounds.size.height - px(2.0),
                    ),
                    size(bounds.size.width, px(2.0)),
                );
                window.paint_quad(fill(underline_bounds, self.color));
            }
        }
    }
}

/// Rectangular region for background merging optimization.
#[derive(Debug, Clone)]
struct BackgroundRegion {
    start_line: i32,
    start_col: i32,
    end_line: i32,
    end_col: i32,
    color: Hsla,
}

impl BackgroundRegion {
    fn new(line: i32, col: i32, color: Hsla) -> Self {
        BackgroundRegion {
            start_line: line,
            start_col: col,
            end_line: line,
            end_col: col,
            color,
        }
    }

    fn can_merge_with(&self, other: &BackgroundRegion) -> bool {
        if self.color != other.color {
            return false;
        }

        if self.start_line == other.start_line && self.end_line == other.end_line {
            return self.end_col + 1 == other.start_col || other.end_col + 1 == self.start_col;
        }

        if self.start_col == other.start_col && self.end_col == other.end_col {
            return self.end_line + 1 == other.start_line || other.end_line + 1 == self.start_line;
        }

        false
    }

    fn merge_with(&mut self, other: &BackgroundRegion) {
        self.start_line = self.start_line.min(other.start_line);
        self.start_col = self.start_col.min(other.start_col);
        self.end_line = self.end_line.max(other.end_line);
        self.end_col = self.end_col.max(other.end_col);
    }
}

fn merge_background_regions(regions: Vec<BackgroundRegion>) -> Vec<BackgroundRegion> {
    if regions.is_empty() {
        return regions;
    }

    let mut merged = regions;
    let mut changed = true;

    while changed {
        changed = false;
        let mut i = 0;

        while i < merged.len() {
            let mut j = i + 1;
            while j < merged.len() {
                if merged[i].can_merge_with(&merged[j]) {
                    let other = merged.remove(j);
                    merged[i].merge_with(&other);
                    changed = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }

    merged
}

/// rgpui element for rendering terminal content.
///
/// Implements the three-phase rendering pipeline:
/// 1. `request_layout` - Requests space from the layout system
/// 2. `prepaint` - Computes batched text runs, background rects, and cursor
/// 3. `paint` - Renders all computed elements
pub struct TerminalElement {
    terminal: Entity<Terminal>,
    focus: FocusHandle,
    focused: bool,
    cursor_visible: bool,
    show_scrollbar: bool,
    cached_font_family: Option<rgpui::SharedString>,
    text_style: TextStyle,
}

impl TerminalElement {
    /// Creates a new terminal element.
    ///
    /// # Arguments
    ///
    /// * `terminal` - The terminal entity to render
    /// * `focus` - Focus handle for keyboard input
    /// * `focused` - Whether the terminal currently has focus
    /// * `cursor_visible` - Whether the cursor should be visible (for blinking)
    pub fn new(
        terminal: Entity<Terminal>,
        focus: FocusHandle,
        focused: bool,
        cursor_visible: bool,
        text_style: TextStyle,
        show_scrollbar: bool,
    ) -> Self {
        TerminalElement {
            terminal,
            focus,
            focused,
            cursor_visible,
            show_scrollbar,
            cached_font_family: None,
            text_style,
        }
    }

    /// Lays out the grid of cells, producing batched text runs and background rects.
    /// Includes viewport culling to only render visible rows for better performance.
    fn layout_grid(
        grid: impl Iterator<Item = IndexedCell>,
        text_style: &TextStyle,
        viewport_lines: Option<(i32, i32)>, // (start_line, end_line) for culling
    ) -> (
        Vec<LayoutRect>,
        Vec<BlockFragment>,
        Vec<PolygonFragment>,
        Vec<BatchedTextRun>,
    ) {
        let estimated_cells = grid.size_hint().0;
        // Improved capacity estimates based on typical terminal usage
        // Most runs are 3-5 characters, so cells/4 is a good estimate
        let estimated_runs = (estimated_cells / 4).max(50);
        // Background regions are less common, typically 5-10% of cells
        let estimated_regions = (estimated_cells / 15).max(20);

        let mut batched_runs = Vec::with_capacity(estimated_runs);
        let mut background_regions: Vec<BackgroundRegion> = Vec::with_capacity(estimated_regions);
        let mut block_fragments: Vec<BlockFragment> = Vec::with_capacity(estimated_regions);
        let mut polygon_fragments: Vec<PolygonFragment> = Vec::with_capacity(estimated_regions);
        let mut current_batch: Option<BatchedTextRun> = None;

        // Filter cells based on viewport if culling is enabled
        let filtered_grid: Box<dyn Iterator<Item = IndexedCell>> =
            if let Some((start, end)) = viewport_lines {
                Box::new(grid.filter(move |cell| {
                    let line = cell.point.line.0;
                    line >= start && line < end
                }))
            } else {
                Box::new(grid)
            };

        let linegroups = filtered_grid.into_iter().chunk_by(|i| i.point.line);
        for (line_index, (_, line)) in linegroups.into_iter().enumerate() {
            let alac_line = line_index as i32;

            if let Some(batch) = current_batch.take() {
                batched_runs.push(batch);
            }

            let mut previous_cell_had_extras = false;

            for cell in line {
                let mut fg = cell.fg;
                let mut bg = cell.bg;
                if cell.flags.contains(Flags::INVERSE) {
                    mem::swap(&mut fg, &mut bg);
                }

                let fg_color = cell_fg_color(&cell, fg, &text_style.theme);
                let bg_color = text_style.theme.resolve_color(&bg);
                if !matches!(bg, AnsiColor::Named(NamedColor::Background))
                    && !(is_block_element_char(cell.c) && bg_color == fg_color)
                {
                    let color = bg_color;
                    let col = cell.point.column.0 as i32;

                    if let Some(last_region) = background_regions.last_mut() {
                        if last_region.color == color
                            && last_region.start_line == alac_line
                            && last_region.end_line == alac_line
                            && last_region.end_col + 1 == col
                        {
                            last_region.end_col = col;
                        } else {
                            background_regions.push(BackgroundRegion::new(alac_line, col, color));
                        }
                    } else {
                        background_regions.push(BackgroundRegion::new(alac_line, col, color));
                    }
                }

                if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    continue;
                }

                if cell.c == ' ' && previous_cell_had_extras {
                    previous_cell_had_extras = false;
                    continue;
                }
                previous_cell_had_extras =
                    matches!(cell.zerowidth(), Some(chars) if !chars.is_empty());

                let cell_point = AlacPoint::new(alac_line, cell.point.column.0 as i32);
                if push_powerline_fragments(
                    &mut block_fragments,
                    &mut polygon_fragments,
                    cell_point,
                    fg_color,
                    cell.c,
                ) {
                    continue;
                }

                if push_box_fragments(&mut block_fragments, cell_point, fg_color, cell.c) {
                    continue;
                }

                if push_block_fragments(&mut block_fragments, cell_point, fg_color, cell.c) {
                    continue;
                }

                if !is_blank(&cell) {
                    let cell_style = Self::cell_style(&cell, fg, text_style);
                    let zero_width_chars = cell.zerowidth();

                    if let Some(ref mut batch) = current_batch {
                        if batch.can_append(&cell_style)
                            && batch.start_point.line == cell_point.line
                            && batch.start_point.column + batch.cell_count as i32
                                == cell_point.column
                        {
                            batch.append_char(cell.c);
                            if let Some(chars) = zero_width_chars {
                                batch.append_zero_width_chars(chars);
                            }
                        } else {
                            let old_batch = current_batch.take().unwrap();
                            batched_runs.push(old_batch);
                            let mut new_batch = BatchedTextRun::new_from_char(
                                cell_point,
                                cell.c,
                                cell_style,
                                text_style.font_size,
                            );
                            if let Some(chars) = zero_width_chars {
                                new_batch.append_zero_width_chars(chars);
                            }
                            current_batch = Some(new_batch);
                        }
                    } else {
                        let mut new_batch = BatchedTextRun::new_from_char(
                            cell_point,
                            cell.c,
                            cell_style,
                            text_style.font_size,
                        );
                        if let Some(chars) = zero_width_chars {
                            new_batch.append_zero_width_chars(chars);
                        }
                        current_batch = Some(new_batch);
                    }
                }
            }
        }

        if let Some(batch) = current_batch {
            batched_runs.push(batch);
        }

        let merged_regions = merge_background_regions(background_regions);
        let mut rects = Vec::with_capacity(merged_regions.len() * 2);

        for region in merged_regions {
            for line in region.start_line..=region.end_line {
                rects.push(LayoutRect::new(
                    AlacPoint::new(line, region.start_col),
                    (region.end_col - region.start_col + 1) as usize,
                    region.color,
                ));
            }
        }

        (rects, block_fragments, polygon_fragments, batched_runs)
    }

    /// Computes cursor position and dimensions.
    fn shape_cursor(
        cursor_point: DisplayCursor,
        size: TerminalBounds,
        text_fragment: &ShapedLine,
    ) -> Option<(Point<Pixels>, Pixels)> {
        if cursor_point.line() < size.num_lines() as i32 {
            let cursor_width = if text_fragment.width == Pixels::ZERO {
                size.cell_width
            } else {
                text_fragment.width
            };

            Some((
                point(
                    (cursor_point.col() as f32 * size.cell_width).floor(),
                    (cursor_point.line() as f32 * size.line_height).floor(),
                ),
                cursor_width.ceil(),
            ))
        } else {
            None
        }
    }

    /// Converts Alacritty cell styles to a rgpui TextRun.
    fn cell_style(indexed: &IndexedCell, fg: AnsiColor, text_style: &TextStyle) -> TextRun {
        let flags = indexed.cell.flags;
        let mut fg_color = text_style.theme.resolve_color(&fg);

        if flags.intersects(Flags::DIM) {
            fg_color.a *= 0.7;
        }

        let underline = flags
            .intersects(Flags::ALL_UNDERLINES)
            .then(|| UnderlineStyle {
                color: Some(fg_color),
                thickness: Pixels::from(1.0),
                wavy: flags.contains(Flags::UNDERCURL),
            });

        let strikethrough = flags
            .intersects(Flags::STRIKEOUT)
            .then(|| StrikethroughStyle {
                color: Some(fg_color),
                thickness: Pixels::from(1.0),
            });

        let weight = if flags.intersects(Flags::BOLD) {
            FontWeight::BOLD
        } else {
            text_style.font_weight
        };

        let style = if flags.intersects(Flags::ITALIC) {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        };

        TextRun {
            len: indexed.c.len_utf8(),
            color: fg_color,
            background_color: None,
            font: Font {
                weight,
                style,
                ..text_style.font.clone()
            },
            underline,
            strikethrough,
        }
    }
}

/// Text styling configuration for terminal rendering.
#[derive(Clone)]
pub struct TextStyle {
    pub font: Font,
    pub font_size: AbsoluteLength,
    pub font_weight: FontWeight,
    pub foreground: Hsla,
    pub background: Hsla,
    pub line_height_multiplier: f32,
    pub letter_spacing: f32,
    pub theme: TerminalTheme,
}

impl Default for TextStyle {
    fn default() -> Self {
        let theme = TerminalTheme::default();
        TextStyle {
            font: Font {
                family: "FiraCode Nerd Font".into(),
                features: rgpui::FontFeatures::default(),
                fallbacks: None, // Allow platform font fallback for missing glyphs.
                weight: FontWeight::NORMAL,
                style: FontStyle::Normal,
            },
            font_size: AbsoluteLength::Pixels(px(14.0)),
            font_weight: FontWeight::NORMAL,
            foreground: theme.foreground,
            // Semi-transparent background so the parent can decide the window fill.
            background: theme.background,
            line_height_multiplier: 1.2, // Optimized for better readability
            letter_spacing: 0.0,
            theme,
        }
    }
}

impl TextStyle {
    pub fn from_config(config: &TerminalConfig) -> Self {
        let theme = config.theme.to_theme();
        let font_size = config.font_size.max(1.0);
        let line_height = config.line_height.max(0.5);
        TextStyle {
            font: Font {
                family: config.font_family.clone().into(),
                features: rgpui::FontFeatures::default(),
                fallbacks: None,
                weight: FontWeight::NORMAL,
                style: FontStyle::Normal,
            },
            font_size: AbsoluteLength::Pixels(px(font_size)),
            font_weight: FontWeight::NORMAL,
            foreground: theme.foreground,
            background: theme.background,
            line_height_multiplier: line_height,
            letter_spacing: config.letter_spacing,
            theme,
        }
    }
}

const MONO_FONT_FAMILIES: &[&str] = &[
    "FiraCode Nerd Font",
    "FiraCode Nerd Font Mono",
    "Fira Code",
    "JetBrains Mono",
    "SF Mono",
    "Menlo",
    "Monaco",
    "Cascadia Mono",
    "Noto Sans Mono",
    "DejaVu Sans Mono",
];

fn selection_color(theme: &TerminalTheme) -> Hsla {
    theme.selection
}

fn is_block_element_char(ch: char) -> bool {
    ('\u{2580}'..='\u{259F}').contains(&ch)
}

fn is_monospace_font(
    text_system: &rgpui::WindowTextSystem,
    font: &Font,
    font_size: Pixels,
) -> bool {
    let font_id = text_system.resolve_font(font);
    let mut widths = Vec::new();

    for ch in ['i', 'm', 'W', '0', ' '] {
        if let Ok(advance) = text_system.advance(font_id, font_size, ch) {
            widths.push(f32::from(advance.width));
        }
    }

    if widths.is_empty() {
        return false;
    }

    let mut min_width = widths[0];
    let mut max_width = widths[0];

    for width in widths.iter().skip(1) {
        min_width = min_width.min(*width);
        max_width = max_width.max(*width);
    }

    (max_width - min_width) <= 0.5
}

fn select_font_family(
    text_system: &rgpui::WindowTextSystem,
    base_font: &Font,
    font_size: Pixels,
) -> rgpui::SharedString {
    let available = text_system.all_font_names();
    let mut candidates: Vec<String> = Vec::new();
    let base_family = base_font.family.as_ref();

    if available.iter().any(|name| name == base_family) {
        candidates.push(base_family.to_string());
    }

    for &family in MONO_FONT_FAMILIES {
        if family == base_family {
            continue;
        }
        if available.iter().any(|name| name == family)
            && !candidates.iter().any(|candidate| candidate == family)
        {
            candidates.push(family.to_string());
        }
    }

    if candidates.is_empty() {
        for name in &available {
            let lower = name.to_ascii_lowercase();
            if lower.contains("mono") || lower.contains("code") || lower.contains("console") {
                candidates.push(name.to_string());
            }
        }
    }

    for family in candidates {
        let mut font = base_font.clone();
        font.family = family.into();
        if is_monospace_font(text_system, &font, font_size) {
            return font.family;
        }
    }

    base_font.family.clone()
}

fn measure_cell_width(
    text_system: &rgpui::WindowTextSystem,
    font_id: rgpui::FontId,
    font_size: Pixels,
) -> Pixels {
    let mut widths = Vec::new();

    for ch in ['0', 'm', 'M', 'W', 'i', ' '] {
        if let Ok(advance) = text_system.advance(font_id, font_size, ch) {
            widths.push(f32::from(advance.width));
        }
    }

    if widths.is_empty() {
        return text_system
            .em_advance(font_id, font_size)
            .or_else(|_| text_system.ch_advance(font_id, font_size))
            .unwrap();
    }

    let mut min_width = widths[0];
    let mut max_width = widths[0];

    for width in widths.iter().skip(1) {
        min_width = min_width.min(*width);
        max_width = max_width.max(*width);
    }

    // For terminal rendering, always use max width to ensure no character clipping
    // Add a small padding (1px) to prevent edge cases where characters touch
    let width = max_width + 0.5;

    px(width.ceil())
}

fn cell_fg_color(indexed: &IndexedCell, fg: AnsiColor, theme: &TerminalTheme) -> Hsla {
    let mut fg_color = theme.resolve_color(&fg);
    if indexed.cell.flags.intersects(Flags::DIM) {
        fg_color.a *= 0.7;
    }
    fg_color
}

#[derive(Clone, Copy, Debug)]
enum BoxWeight {
    Light,
    Heavy,
    Double,
}

fn line_thickness(weight: BoxWeight) -> f32 {
    match weight {
        BoxWeight::Light => 1.0 / 8.0,
        BoxWeight::Heavy => 2.0 / 8.0,
        BoxWeight::Double => 1.0 / 10.0,
    }
}

fn push_rect_fragment(
    out: &mut Vec<BlockFragment>,
    point: AlacPoint<i32, i32>,
    color: Hsla,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    out.push(BlockFragment {
        point,
        rect: BlockRect {
            x,
            y,
            width,
            height,
        },
        color,
    });
}

fn push_horizontal_line(
    out: &mut Vec<BlockFragment>,
    point: AlacPoint<i32, i32>,
    color: Hsla,
    weight: BoxWeight,
    left: bool,
    right: bool,
) {
    if !left && !right {
        return;
    }

    let thickness = line_thickness(weight);
    let (x_start, x_end) = match (left, right) {
        (true, true) => (0.0, 1.0),
        (true, false) => (0.0, 0.5),
        (false, true) => (0.5, 1.0),
        _ => return,
    };

    let centers: [f32; 2] = if matches!(weight, BoxWeight::Double) {
        let offset = thickness * 1.5;
        [0.5 - offset, 0.5 + offset]
    } else {
        [0.5, 0.5]
    };

    for center in centers {
        let y = (center - thickness / 2.0).max(0.0);
        let height = (center + thickness / 2.0).min(1.0) - y;
        push_rect_fragment(out, point, color, x_start, y, x_end - x_start, height);
        if !matches!(weight, BoxWeight::Double) {
            break;
        }
    }
}

fn push_vertical_line(
    out: &mut Vec<BlockFragment>,
    point: AlacPoint<i32, i32>,
    color: Hsla,
    weight: BoxWeight,
    up: bool,
    down: bool,
) {
    if !up && !down {
        return;
    }

    let thickness = line_thickness(weight);
    let (y_start, y_end) = match (up, down) {
        (true, true) => (0.0, 1.0),
        (true, false) => (0.0, 0.5),
        (false, true) => (0.5, 1.0),
        _ => return,
    };

    let centers: [f32; 2] = if matches!(weight, BoxWeight::Double) {
        let offset = thickness * 1.5;
        [0.5 - offset, 0.5 + offset]
    } else {
        [0.5, 0.5]
    };

    for center in centers {
        let x = (center - thickness / 2.0).max(0.0);
        let width = (center + thickness / 2.0).min(1.0) - x;
        push_rect_fragment(out, point, color, x, y_start, width, y_end - y_start);
        if !matches!(weight, BoxWeight::Double) {
            break;
        }
    }
}

fn push_box_fragments(
    out: &mut Vec<BlockFragment>,
    point: AlacPoint<i32, i32>,
    color: Hsla,
    ch: char,
) -> bool {
    let (weight, left, right, up, down) = match ch {
        '\u{2500}' => (BoxWeight::Light, true, true, false, false),
        '\u{2502}' => (BoxWeight::Light, false, false, true, true),
        '\u{250C}' => (BoxWeight::Light, false, true, false, true),
        '\u{2510}' => (BoxWeight::Light, true, false, false, true),
        '\u{2514}' => (BoxWeight::Light, false, true, true, false),
        '\u{2518}' => (BoxWeight::Light, true, false, true, false),
        '\u{251C}' => (BoxWeight::Light, false, true, true, true),
        '\u{2524}' => (BoxWeight::Light, true, false, true, true),
        '\u{252C}' => (BoxWeight::Light, true, true, false, true),
        '\u{2534}' => (BoxWeight::Light, true, true, true, false),
        '\u{253C}' => (BoxWeight::Light, true, true, true, true),
        '\u{2574}' => (BoxWeight::Light, true, false, false, false),
        '\u{2575}' => (BoxWeight::Light, false, false, true, false),
        '\u{2576}' => (BoxWeight::Light, false, true, false, false),
        '\u{2577}' => (BoxWeight::Light, false, false, false, true),
        '\u{256D}' => (BoxWeight::Light, false, true, false, true),
        '\u{256E}' => (BoxWeight::Light, true, false, false, true),
        '\u{256F}' => (BoxWeight::Light, true, false, true, false),
        '\u{2570}' => (BoxWeight::Light, false, true, true, false),
        '\u{2501}' => (BoxWeight::Heavy, true, true, false, false),
        '\u{2503}' => (BoxWeight::Heavy, false, false, true, true),
        '\u{250F}' => (BoxWeight::Heavy, false, true, false, true),
        '\u{2513}' => (BoxWeight::Heavy, true, false, false, true),
        '\u{2517}' => (BoxWeight::Heavy, false, true, true, false),
        '\u{251B}' => (BoxWeight::Heavy, true, false, true, false),
        '\u{2523}' => (BoxWeight::Heavy, false, true, true, true),
        '\u{252B}' => (BoxWeight::Heavy, true, false, true, true),
        '\u{2533}' => (BoxWeight::Heavy, true, true, false, true),
        '\u{253B}' => (BoxWeight::Heavy, true, true, true, false),
        '\u{254B}' => (BoxWeight::Heavy, true, true, true, true),
        '\u{2550}' => (BoxWeight::Double, true, true, false, false),
        '\u{2551}' => (BoxWeight::Double, false, false, true, true),
        '\u{2554}' => (BoxWeight::Double, false, true, false, true),
        '\u{2557}' => (BoxWeight::Double, true, false, false, true),
        '\u{255A}' => (BoxWeight::Double, false, true, true, false),
        '\u{255D}' => (BoxWeight::Double, true, false, true, false),
        '\u{2560}' => (BoxWeight::Double, false, true, true, true),
        '\u{2563}' => (BoxWeight::Double, true, false, true, true),
        '\u{2566}' => (BoxWeight::Double, true, true, false, true),
        '\u{2569}' => (BoxWeight::Double, true, true, true, false),
        '\u{256C}' => (BoxWeight::Double, true, true, true, true),
        _ => return false,
    };

    push_horizontal_line(out, point, color, weight, left, right);
    push_vertical_line(out, point, color, weight, up, down);
    true
}

fn push_powerline_fragments(
    rects: &mut Vec<BlockFragment>,
    polys: &mut Vec<PolygonFragment>,
    point: AlacPoint<i32, i32>,
    color: Hsla,
    ch: char,
) -> bool {
    match ch {
        '\u{E0B0}' => {
            polys.push(PolygonFragment {
                point,
                points: [
                    BlockPoint { x: 0.0, y: 0.0 },
                    BlockPoint { x: 1.0, y: 0.5 },
                    BlockPoint { x: 0.0, y: 1.0 },
                ],
                color,
            });
            true
        }
        '\u{E0B2}' => {
            polys.push(PolygonFragment {
                point,
                points: [
                    BlockPoint { x: 1.0, y: 0.0 },
                    BlockPoint { x: 0.0, y: 0.5 },
                    BlockPoint { x: 1.0, y: 1.0 },
                ],
                color,
            });
            true
        }
        '\u{E0B1}' | '\u{E0B3}' | '\u{E0B5}' | '\u{E0B7}' => {
            let thickness = line_thickness(BoxWeight::Light);
            push_rect_fragment(
                rects,
                point,
                color,
                0.5 - thickness / 2.0,
                0.0,
                thickness,
                1.0,
            );
            true
        }
        '\u{E0B4}' => {
            polys.push(PolygonFragment {
                point,
                points: [
                    BlockPoint { x: 0.0, y: 0.0 },
                    BlockPoint { x: 1.0, y: 0.5 },
                    BlockPoint { x: 0.0, y: 1.0 },
                ],
                color,
            });
            true
        }
        '\u{E0B6}' => {
            polys.push(PolygonFragment {
                point,
                points: [
                    BlockPoint { x: 1.0, y: 0.0 },
                    BlockPoint { x: 0.0, y: 0.5 },
                    BlockPoint { x: 1.0, y: 1.0 },
                ],
                color,
            });
            true
        }
        _ => false,
    }
}

fn push_block_fragments(
    out: &mut Vec<BlockFragment>,
    point: AlacPoint<i32, i32>,
    color: Hsla,
    ch: char,
) -> bool {
    let rect = |x: f32, y: f32, width: f32, height: f32| BlockRect {
        x,
        y,
        width,
        height,
    };

    let mut push = |x: f32, y: f32, width: f32, height: f32| {
        out.push(BlockFragment {
            point,
            rect: rect(x, y, width, height),
            color,
        });
    };

    match ch {
        '\u{2591}' => {
            let mut shade = color;
            shade.a *= 0.25;
            push(0.0, 0.0, 1.0, 1.0);
            if let Some(fragment) = out.last_mut() {
                fragment.color = shade;
            }
            true
        }
        '\u{2592}' => {
            let mut shade = color;
            shade.a *= 0.5;
            push(0.0, 0.0, 1.0, 1.0);
            if let Some(fragment) = out.last_mut() {
                fragment.color = shade;
            }
            true
        }
        '\u{2593}' => {
            let mut shade = color;
            shade.a *= 0.75;
            push(0.0, 0.0, 1.0, 1.0);
            if let Some(fragment) = out.last_mut() {
                fragment.color = shade;
            }
            true
        }
        '\u{2588}' => {
            push(0.0, 0.0, 1.0, 1.0);
            true
        }
        '\u{2580}' => {
            push(0.0, 0.0, 1.0, 0.5);
            true
        }
        '\u{2584}' => {
            push(0.0, 0.5, 1.0, 0.5);
            true
        }
        '\u{2581}'..='\u{2587}' => {
            let steps = ch as u32 - 0x2580;
            let height = steps as f32 / 8.0;
            push(0.0, 1.0 - height, 1.0, height);
            true
        }
        '\u{2589}'..='\u{258F}' => {
            let steps = 0x2590 - ch as u32;
            let width = steps as f32 / 8.0;
            push(0.0, 0.0, width, 1.0);
            true
        }
        '\u{2590}' => {
            push(0.5, 0.0, 0.5, 1.0);
            true
        }
        '\u{2594}' => {
            push(0.0, 0.0, 1.0, 1.0 / 8.0);
            true
        }
        '\u{2595}' => {
            push(7.0 / 8.0, 0.0, 1.0 / 8.0, 1.0);
            true
        }
        '\u{2596}' | '\u{2597}' | '\u{2598}' | '\u{2599}' | '\u{259A}' | '\u{259B}'
        | '\u{259C}' | '\u{259D}' | '\u{259E}' | '\u{259F}' => {
            let mask = match ch {
                '\u{2596}' => 0b0010, // lower left
                '\u{2597}' => 0b0001, // lower right
                '\u{2598}' => 0b1000, // upper left
                '\u{2599}' => 0b1011, // upper left + lower left + lower right
                '\u{259A}' => 0b1001, // upper left + lower right
                '\u{259B}' => 0b1110, // upper left + upper right + lower left
                '\u{259C}' => 0b1101, // upper left + upper right + lower right
                '\u{259D}' => 0b0100, // upper right
                '\u{259E}' => 0b0110, // upper right + lower left
                '\u{259F}' => 0b0111, // upper right + lower left + lower right
                _ => 0,
            };

            if mask & 0b1000 != 0 {
                push(0.0, 0.0, 0.5, 0.5);
            }
            if mask & 0b0100 != 0 {
                push(0.5, 0.0, 0.5, 0.5);
            }
            if mask & 0b0010 != 0 {
                push(0.0, 0.5, 0.5, 0.5);
            }
            if mask & 0b0001 != 0 {
                push(0.5, 0.5, 0.5, 0.5);
            }
            true
        }
        _ => false,
    }
}

fn selection_rects(
    selection: &SelectionRange,
    display_offset: usize,
    dimensions: &TerminalBounds,
    color: Hsla,
) -> Vec<LayoutRect> {
    let num_lines = dimensions.num_lines() as i32;
    let num_columns = dimensions.num_columns();

    if num_lines <= 0 || num_columns == 0 {
        return Vec::new();
    }

    let max_line = num_lines - 1;
    let max_col = num_columns.saturating_sub(1) as i32;

    let mut start_line = selection.start.line.0 + display_offset as i32;
    let mut end_line = selection.end.line.0 + display_offset as i32;

    if start_line > end_line {
        mem::swap(&mut start_line, &mut end_line);
    }

    if end_line < 0 || start_line > max_line {
        return Vec::new();
    }

    start_line = start_line.max(0);
    end_line = end_line.min(max_line);

    let start_col = selection.start.column.0 as i32;
    let end_col = selection.end.column.0 as i32;

    let mut rects = Vec::new();
    if selection.is_block {
        let left = start_col.min(end_col).clamp(0, max_col);
        let right = start_col.max(end_col).clamp(0, max_col);
        for line in start_line..=end_line {
            rects.push(LayoutRect::new(
                AlacPoint::new(line, left),
                (right - left + 1) as usize,
                color,
            ));
        }
    } else {
        for line in start_line..=end_line {
            let (mut col_start, mut col_end) = if line == start_line && line == end_line {
                (start_col, end_col)
            } else if line == start_line {
                (start_col, max_col)
            } else if line == end_line {
                (0, end_col)
            } else {
                (0, max_col)
            };

            col_start = col_start.clamp(0, max_col);
            col_end = col_end.clamp(0, max_col);

            if col_end >= col_start {
                rects.push(LayoutRect::new(
                    AlacPoint::new(line, col_start),
                    (col_end - col_start + 1) as usize,
                    color,
                ));
            }
        }
    }

    rects
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = LayoutState;

    fn id(&self) -> Option<ElementId> {
        Some("terminal-element".into())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&rgpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = rgpui::Style::default();
        style.size.width = rgpui::Length::Definite(rgpui::DefiniteLength::Fraction(1.0));
        style.size.height = rgpui::Length::Definite(rgpui::DefiniteLength::Fraction(1.0));
        style.flex_grow = 1.0;

        let layout_id = window.request_layout(style, None, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&rgpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let hitbox = window.insert_hitbox(bounds, rgpui::HitboxBehavior::Normal);

        let mut text_style = self.text_style.clone();
        let background_color = text_style.background;
        let foreground_color = text_style.foreground;

        let font_pixels = text_style.font_size.to_pixels(window.rem_size());
        let text_system = window.text_system();

        if let Some(family) = self.cached_font_family.clone() {
            text_style.font.family = family;
        } else {
            let family = select_font_family(text_system, &text_style.font, font_pixels);
            text_style.font.family = family.clone();
            self.cached_font_family = Some(family);
        }

        let font_id = window.text_system().resolve_font(&text_style.font);
        let mut cell_width = measure_cell_width(text_system, font_id, font_pixels);
        cell_width = (cell_width + px(text_style.letter_spacing)).max(px(1.0));
        // Round line_height to whole pixels for crisp text rendering on Windows
        let line_height = px((f32::from(font_pixels) * text_style.line_height_multiplier).round());

        let dimensions = TerminalBounds::new(line_height, cell_width, bounds);

        self.terminal.update(cx, |terminal, cx| {
            terminal.set_size(dimensions);
            terminal.sync(window, cx);
        });

        let TerminalContent {
            cells,
            mode,
            display_offset,
            cursor_char,
            cursor,
            selection,
            history_size,
            ..
        } = &self.terminal.read(cx).last_content;
        let mode = *mode;
        let display_offset = *display_offset;
        let history_size = *history_size;

        let (rects, block_fragments, polygon_fragments, batched_text_runs) = Self::layout_grid(
            cells.iter().cloned(),
            &text_style,
            None, // Viewport culling disabled for now, can be enabled for large terminals
        );

        let selection_rects = selection
            .as_ref()
            .map(|selection| {
                selection_rects(
                    selection,
                    display_offset,
                    &dimensions,
                    selection_color(&text_style.theme),
                )
            })
            .unwrap_or_default();

        let cursor_layout = if let AlacCursorShape::Hidden = cursor.shape {
            None
        } else {
            let cursor_point = DisplayCursor::from(cursor.point, display_offset);
            let cursor_text = {
                let str_text = cursor_char.to_string();
                let len = str_text.len();
                window.text_system().shape_line(
                    str_text.into(),
                    font_pixels,
                    &[TextRun {
                        len,
                        font: text_style.font.clone(),
                        color: text_style.background,
                        ..Default::default()
                    }],
                    None,
                )
            };

            let focused = self.focused;
            Self::shape_cursor(cursor_point, dimensions, &cursor_text).map(
                move |(cursor_position, block_width)| {
                    let (shape, text) = match cursor.shape {
                        AlacCursorShape::Block if !focused => (CursorShape::Hollow, None),
                        AlacCursorShape::Block => (CursorShape::Block, Some(cursor_text)),
                        AlacCursorShape::Underline => (CursorShape::Underline, None),
                        AlacCursorShape::Beam => (CursorShape::Bar, None),
                        AlacCursorShape::HollowBlock => (CursorShape::Hollow, None),
                        AlacCursorShape::Hidden => unreachable!(),
                    };

                    CursorLayout::new(
                        cursor_position,
                        block_width,
                        dimensions.line_height,
                        text_style.theme.cursor,
                        shape,
                        text,
                    )
                },
            )
        };

        LayoutState {
            hitbox,
            batched_text_runs,
            background_rects: rects,
            block_fragments,
            polygon_fragments,
            selection_rects,
            cursor: cursor_layout,
            background_color,
            foreground_color,
            dimensions,
            mode,
            history_size,
            display_offset,
        }
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&rgpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        layout: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Register input handler for text input
        let input_handler = TerminalInputHandler {
            terminal: self.terminal.clone(),
            cursor_bounds: layout.cursor.as_ref().map(|c| c.bounds(bounds.origin)),
        };
        window.handle_input(&self.focus, input_handler, cx);

        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            window.paint_quad(fill(bounds, layout.background_color));
            let origin = bounds.origin;

            for rect in &layout.background_rects {
                rect.paint(origin, &layout.dimensions, window);
            }

            for fragment in &layout.block_fragments {
                fragment.paint(origin, &layout.dimensions, window);
            }

            for fragment in &layout.polygon_fragments {
                fragment.paint(origin, &layout.dimensions, window);
            }

            for rect in &layout.selection_rects {
                rect.paint(origin, &layout.dimensions, window);
            }

            for batch in &layout.batched_text_runs {
                batch.paint(origin, &layout.dimensions, window, cx);
            }

            if self.cursor_visible
                && let Some(cursor) = &layout.cursor
            {
                cursor.paint(origin, window, cx);
            }

            // Paint scrollbar on top of terminal content
            let history_size = layout.history_size;
            if history_size > 0 && self.show_scrollbar {
                let track_height = f32::from(bounds.size.height);
                let visible_lines = layout.dimensions.num_lines();
                let total_lines = history_size + visible_lines;

                let thumb_height = (visible_lines as f32 / total_lines as f32 * track_height)
                    .max(SCROLLBAR_THUMB_MIN_HEIGHT);
                let scrollable_track = track_height - thumb_height;

                let display_offset = layout.display_offset;
                let thumb_top = if history_size > 0 {
                    (1.0 - display_offset as f32 / history_size as f32) * scrollable_track
                } else {
                    0.0
                };

                let track_x = bounds.origin.x + bounds.size.width - px(SCROLLBAR_WIDTH);
                let track_bounds = Bounds::new(
                    point(track_x, bounds.origin.y),
                    size(px(SCROLLBAR_WIDTH), bounds.size.height),
                );
                let track_color = Hsla {
                    a: 0.08,
                    ..layout.foreground_color
                };
                window.paint_quad(fill(track_bounds, track_color));

                let thumb_bounds = Bounds::new(
                    point(track_x, bounds.origin.y + px(thumb_top)),
                    size(px(SCROLLBAR_WIDTH), px(thumb_height)),
                );
                let thumb_color = Hsla {
                    a: 0.45,
                    ..layout.foreground_color
                };
                window.paint_quad(rgpui::quad(
                    thumb_bounds,
                    px(SCROLLBAR_WIDTH / 2.0),
                    thumb_color,
                    rgpui::Edges::default(),
                    Hsla::transparent_black(),
                    rgpui::BorderStyle::Solid,
                ));
            }
        });
    }
}

impl IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Checks if a cell is blank and can be skipped during rendering.
fn is_blank(cell: &IndexedCell) -> bool {
    if cell.c != ' ' {
        return false;
    }

    if cell.bg != AnsiColor::Named(NamedColor::Background) {
        return false;
    }

    if cell
        .flags
        .intersects(Flags::ALL_UNDERLINES | Flags::INVERSE | Flags::STRIKEOUT)
    {
        return false;
    }

    true
}

/// Converts an Alacritty ANSI color to rgpui's Hsla format.
///
/// Supports:
/// - Named colors (16 standard ANSI colors)
/// - True colors (24-bit RGB)
/// - Indexed colors (256-color palette)
pub fn convert_color(color: &AnsiColor) -> Hsla {
    match color {
        AnsiColor::Named(named) => named_color_to_hsla(*named),
        AnsiColor::Spec(rgb) => {
            let rgba = Rgba {
                r: rgb.r as f32 / 255.0,
                g: rgb.g as f32 / 255.0,
                b: rgb.b as f32 / 255.0,
                a: 1.0,
            };
            rgba.into()
        }
        AnsiColor::Indexed(idx) => indexed_color_to_hsla(*idx),
    }
}

/// Converts a named ANSI color to Hsla with improved color accuracy.
fn named_color_to_hsla(named: NamedColor) -> Hsla {
    match named {
        // Base colors - improved for better contrast and readability
        NamedColor::Black => hsla_from_rgb(0x1E, 0x1E, 0x1E),
        NamedColor::Red => hsla_from_rgb(0xE0, 0x6C, 0x75),
        NamedColor::Green => hsla_from_rgb(0x98, 0xC3, 0x79),
        NamedColor::Yellow => hsla_from_rgb(0xE5, 0xC0, 0x7B),
        NamedColor::Blue => hsla_from_rgb(0x61, 0xAF, 0xEF),
        NamedColor::Magenta => hsla_from_rgb(0xC6, 0x78, 0xDD),
        NamedColor::Cyan => hsla_from_rgb(0x56, 0xB6, 0xC2),
        NamedColor::White => hsla_from_rgb(0xAB, 0xB2, 0xBF),

        // Bright colors - enhanced vibrancy
        NamedColor::BrightBlack => hsla_from_rgb(0x5C, 0x63, 0x70),
        NamedColor::BrightRed => hsla_from_rgb(0xE0, 0x6C, 0x75),
        NamedColor::BrightGreen => hsla_from_rgb(0x98, 0xC3, 0x79),
        NamedColor::BrightYellow => hsla_from_rgb(0xE5, 0xC0, 0x7B),
        NamedColor::BrightBlue => hsla_from_rgb(0x61, 0xAF, 0xEF),
        NamedColor::BrightMagenta => hsla_from_rgb(0xC6, 0x78, 0xDD),
        NamedColor::BrightCyan => hsla_from_rgb(0x56, 0xB6, 0xC2),
        NamedColor::BrightWhite => hsla_from_rgb(0xDF, 0xDF, 0xDF),

        // Foreground/Background
        NamedColor::Foreground => hsla_from_rgb(0xD4, 0xD4, 0xD4),
        NamedColor::Background => hsla_from_rgb(0x1E, 0x1E, 0x1E),
        NamedColor::Cursor => hsla_from_rgb(0xAE, 0xAF, 0xAD),

        // Dim colors - better visibility
        NamedColor::DimBlack => hsla_from_rgb(0x1E, 0x1E, 0x1E),
        NamedColor::DimRed => hsla_from_rgb(0xBE, 0x5B, 0x65),
        NamedColor::DimGreen => hsla_from_rgb(0x7A, 0x9F, 0x60),
        NamedColor::DimYellow => hsla_from_rgb(0xD1, 0x9A, 0x66),
        NamedColor::DimBlue => hsla_from_rgb(0x4E, 0x88, 0xB8),
        NamedColor::DimMagenta => hsla_from_rgb(0xA0, 0x61, 0xB0),
        NamedColor::DimCyan => hsla_from_rgb(0x44, 0x91, 0x9B),
        NamedColor::DimWhite => hsla_from_rgb(0x8A, 0x8F, 0x98),
        NamedColor::BrightForeground => hsla_from_rgb(0xDF, 0xDF, 0xDF),
        NamedColor::DimForeground => hsla_from_rgb(0x8A, 0x8F, 0x98),
    }
}

/// Converts a 256-color palette index to Hsla.
fn indexed_color_to_hsla(idx: u8) -> Hsla {
    match idx {
        0..=15 => {
            let named = match idx {
                0 => NamedColor::Black,
                1 => NamedColor::Red,
                2 => NamedColor::Green,
                3 => NamedColor::Yellow,
                4 => NamedColor::Blue,
                5 => NamedColor::Magenta,
                6 => NamedColor::Cyan,
                7 => NamedColor::White,
                8 => NamedColor::BrightBlack,
                9 => NamedColor::BrightRed,
                10 => NamedColor::BrightGreen,
                11 => NamedColor::BrightYellow,
                12 => NamedColor::BrightBlue,
                13 => NamedColor::BrightMagenta,
                14 => NamedColor::BrightCyan,
                15 => NamedColor::BrightWhite,
                _ => unreachable!(),
            };
            named_color_to_hsla(named)
        }
        16..=231 => {
            let idx = idx - 16;
            let r = (idx / 36) % 6;
            let g = (idx / 6) % 6;
            let b = idx % 6;

            let r = if r > 0 { r * 40 + 55 } else { 0 };
            let g = if g > 0 { g * 40 + 55 } else { 0 };
            let b = if b > 0 { b * 40 + 55 } else { 0 };

            hsla_from_rgb(r, g, b)
        }
        232..=255 => {
            let gray = (idx - 232) * 10 + 8;
            hsla_from_rgb(gray, gray, gray)
        }
    }
}

/// Helper function to create Hsla from RGB values (0-255).
fn hsla_from_rgb(r: u8, g: u8, b: u8) -> Hsla {
    let rgba = Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    };
    rgba.into()
}

/// Input handler for the terminal that processes text input.
///
/// This handles regular character input (typing), IME composition,
/// and other text-related input events via rgpui's InputHandler trait.
struct TerminalInputHandler {
    terminal: Entity<Terminal>,
    cursor_bounds: Option<Bounds<Pixels>>,
}

impl InputHandler for TerminalInputHandler {
    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        cx: &mut App,
    ) -> Option<UTF16Selection> {
        // Return empty selection when not in ALT_SCREEN mode
        // This allows text input to work
        if self
            .terminal
            .read(cx)
            .last_content
            .mode
            .contains(TermMode::ALT_SCREEN)
        {
            None
        } else {
            Some(UTF16Selection {
                range: 0..0,
                reversed: false,
            })
        }
    }

    fn marked_text_range(
        &mut self,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<std::ops::Range<usize>> {
        // No IME composition support for now
        None
    }

    fn text_for_range(
        &mut self,
        _range: std::ops::Range<usize>,
        _actual_range: &mut Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<String> {
        None
    }

    fn replace_text_in_range(
        &mut self,
        _replacement_range: Option<std::ops::Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut App,
    ) {
        // Send the typed text to the terminal
        self.terminal.update(cx, |terminal, _| {
            terminal.input_text(text);
        });
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range_utf16: Option<std::ops::Range<usize>>,
        _new_text: &str,
        _new_marked_range: Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut App,
    ) {
        // IME composition - not implemented yet
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut App) {
        // IME - not implemented yet
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: std::ops::Range<usize>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        // Return cursor bounds for IME positioning
        self.cursor_bounds
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_named_colors_returns_valid_hsla() {
        let colors = [
            NamedColor::Black,
            NamedColor::Red,
            NamedColor::Green,
            NamedColor::Yellow,
            NamedColor::Blue,
            NamedColor::Magenta,
            NamedColor::Cyan,
            NamedColor::White,
        ];

        for color in colors {
            let hsla = named_color_to_hsla(color);
            assert!(
                hsla.a >= 0.0 && hsla.a <= 1.0,
                "Alpha should be in valid range for {:?}",
                color
            );
            assert!(
                hsla.s >= 0.0 && hsla.s <= 1.0,
                "Saturation should be in valid range for {:?}",
                color
            );
            assert!(
                hsla.l >= 0.0 && hsla.l <= 1.0,
                "Lightness should be in valid range for {:?}",
                color
            );
        }
    }

    #[test]
    fn test_indexed_color_standard_colors_map_to_named() {
        let black = indexed_color_to_hsla(0);
        let named_black = named_color_to_hsla(NamedColor::Black);
        assert_eq!(black, named_black, "Index 0 should map to black");

        let bright_white = indexed_color_to_hsla(15);
        let named_bright_white = named_color_to_hsla(NamedColor::BrightWhite);
        assert_eq!(
            bright_white, named_bright_white,
            "Index 15 should map to bright white"
        );
    }

    #[test]
    fn test_indexed_color_cube_colors_produce_valid_hsla() {
        for idx in 16..=231u8 {
            let hsla = indexed_color_to_hsla(idx);
            assert!(
                hsla.a == 1.0,
                "Indexed color {} should have full alpha",
                idx
            );
        }
    }

    #[test]
    fn test_indexed_color_grayscale_produces_valid_hsla() {
        for idx in 232..=255u8 {
            let hsla = indexed_color_to_hsla(idx);
            assert!(
                hsla.a == 1.0,
                "Grayscale color {} should have full alpha",
                idx
            );
            assert!(
                hsla.s < 0.01,
                "Grayscale color {} should have near-zero saturation",
                idx
            );
        }
    }

    #[test]
    fn test_convert_spec_color_rgb_values() {
        let rgb = alacritty_terminal::vte::ansi::Rgb {
            r: 255,
            g: 128,
            b: 64,
        };
        let color = AnsiColor::Spec(rgb);
        let hsla = convert_color(&color);

        assert!(hsla.a == 1.0, "Spec color should have full alpha");
    }

    #[test]
    fn test_batched_text_run_can_append_same_style() {
        let style1 = TextRun {
            len: 1,
            font: Font::default(),
            color: Hsla::red(),
            ..Default::default()
        };

        let style2 = TextRun {
            len: 1,
            font: Font::default(),
            color: Hsla::red(),
            ..Default::default()
        };

        let font_size = AbsoluteLength::Pixels(px(12.0));
        let batch = BatchedTextRun::new_from_char(AlacPoint::new(0, 0), 'a', style1, font_size);

        assert!(
            batch.can_append(&style2),
            "Should be able to append same style"
        );
    }

    #[test]
    fn test_batched_text_run_cannot_append_different_color() {
        let style1 = TextRun {
            len: 1,
            font: Font::default(),
            color: Hsla::red(),
            ..Default::default()
        };

        let style2 = TextRun {
            len: 1,
            font: Font::default(),
            color: Hsla::blue(),
            ..Default::default()
        };

        let font_size = AbsoluteLength::Pixels(px(12.0));
        let batch = BatchedTextRun::new_from_char(AlacPoint::new(0, 0), 'a', style1, font_size);

        assert!(
            !batch.can_append(&style2),
            "Should not be able to append different color"
        );
    }

    #[test]
    fn test_batched_text_run_append_increments_cell_count() {
        let style = TextRun {
            len: 1,
            font: Font::default(),
            color: Hsla::red(),
            ..Default::default()
        };

        let font_size = AbsoluteLength::Pixels(px(12.0));
        let mut batch = BatchedTextRun::new_from_char(AlacPoint::new(0, 0), 'a', style, font_size);

        assert_eq!(batch.cell_count, 1, "Initial cell count should be 1");
        assert_eq!(batch.text, "a", "Initial text should be 'a'");

        batch.append_char('b');

        assert_eq!(
            batch.cell_count, 2,
            "Cell count should increment after append"
        );
        assert_eq!(batch.text, "ab", "Text should be 'ab' after append");
    }

    #[test]
    fn test_background_region_can_merge_horizontal_adjacent() {
        let color = Hsla::red();
        let mut region1 = BackgroundRegion::new(0, 0, color);
        region1.end_col = 2;
        let region2 = BackgroundRegion::new(0, 3, color);

        assert!(
            region1.can_merge_with(&region2),
            "Adjacent horizontal regions with same color should merge"
        );
    }

    #[test]
    fn test_background_region_cannot_merge_different_colors() {
        let region1 = BackgroundRegion::new(0, 0, Hsla::red());
        let region2 = BackgroundRegion::new(0, 1, Hsla::blue());

        assert!(
            !region1.can_merge_with(&region2),
            "Regions with different colors should not merge"
        );
    }

    #[test]
    fn test_display_cursor_calculates_offset_correctly() {
        let cursor_point = AlacPoint::new(
            alacritty_terminal::index::Line(5),
            alacritty_terminal::index::Column(10),
        );
        let display_offset = 3usize;

        let display_cursor = DisplayCursor::from(cursor_point, display_offset);

        assert_eq!(
            display_cursor.line(),
            8,
            "Line should be cursor line + display offset"
        );
        assert_eq!(
            display_cursor.col(),
            10,
            "Column should match cursor column"
        );
    }
}
