mod font_fallbacks;
mod font_features;
mod line;
mod line_layout;
mod line_wrapper;

pub use font_fallbacks::*;
pub use font_features::*;
pub use line::*;
pub use line_layout::*;
pub use line_wrapper::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::collections::FxHashMap;
use crate::{
    Bounds, DevicePixels, Hsla, Pixels, PlatformTextSystem, Point, Result, SharedString, Size,
    StrikethroughStyle, TextRenderingMode, UnderlineStyle, px,
};
use anyhow::{Context as _, anyhow};
use core::fmt;
use derive_more::{Add, Deref, FromStr, Sub};
use itertools::Itertools;
use parking_lot::{Mutex, RwLock, RwLockUpgradableReadGuard};
use smallvec::{SmallVec, smallvec};
use std::{
    borrow::Cow,
    cmp,
    fmt::{Debug, Display, Formatter},
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut, Range},
    sync::Arc,
};

/// 特定字体的不透明标识符。
#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
#[repr(C)]
pub struct FontId(pub usize);

/// 特定字体族的不透明标识符。
#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct FontFamilyId(pub usize);

/// X 轴方向的亚像素字形变体数量。
pub const SUBPIXEL_VARIANTS_X: u8 = 4;

/// Y 轴方向的亚像素字形变体数量。
pub const SUBPIXEL_VARIANTS_Y: u8 = 1;

/// RGPUI 文本渲染子系统。
pub struct TextSystem {
    platform_text_system: Arc<dyn PlatformTextSystem>,
    font_ids_by_font: RwLock<FxHashMap<Font, Result<FontId>>>,
    font_metrics: RwLock<FxHashMap<FontId, FontMetrics>>,
    raster_bounds: RwLock<FxHashMap<RenderGlyphParams, Bounds<DevicePixels>>>,
    wrapper_pool: Mutex<FxHashMap<FontIdWithSize, Vec<LineWrapper>>>,
    font_runs_pool: Mutex<Vec<Vec<FontRun>>>,
    fallback_font_stack: SmallVec<[Font; 2]>,
}

impl TextSystem {
    /// 使用指定的平台文本系统创建一个新的 TextSystem。
    pub fn new(platform_text_system: Arc<dyn PlatformTextSystem>) -> Self {
        TextSystem {
            platform_text_system,
            font_metrics: RwLock::default(),
            raster_bounds: RwLock::default(),
            font_ids_by_font: RwLock::default(),
            wrapper_pool: Mutex::default(),
            font_runs_pool: Mutex::default(),
            fallback_font_stack: smallvec![
                // TODO: Remove this when Linux have implemented setting fallbacks.
                font(".ZedMono"),
                font(".ZedSans"),
                font("Helvetica"),
                font("Segoe UI"),     // Windows
                font("Ubuntu"),       // Gnome (Ubuntu)
                font("Adwaita Sans"), // Gnome 47
                font("Cantarell"),    // Gnome
                font("Noto Sans"),    // KDE
                font("DejaVu Sans"),
                font("Arial"), // macOS, Windows
            ],
        }
    }

    /// 从操作系统获取所有可用字体名称的列表。
    pub fn all_font_names(&self) -> Vec<String> {
        let mut names = self.platform_text_system.all_font_names();
        names.extend(
            self.fallback_font_stack
                .iter()
                .map(|font| font.family.to_string()),
        );
        names.push(".SystemUIFont".to_string());
        names.sort_unstable();
        names.dedup();
        names
    }

    /// 向文本系统添加字体数据。
    pub fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        self.platform_text_system.add_fonts(fonts)
    }

    /// 获取指定字体族和样式的 FontId。
    fn font_id(&self, font: &Font) -> Result<FontId> {
        fn clone_font_id_result(font_id: &Result<FontId>) -> Result<FontId> {
            match font_id {
                Ok(font_id) => Ok(*font_id),
                Err(err) => Err(anyhow!("{err}")),
            }
        }

        let font_id = self
            .font_ids_by_font
            .read()
            .get(font)
            .map(clone_font_id_result);
        if let Some(font_id) = font_id {
            font_id
        } else {
            let font_id = self.platform_text_system.font_id(font);
            self.font_ids_by_font
                .write()
                .insert(font.clone(), clone_font_id_result(&font_id));
            font_id
        }
    }

    /// 根据 FontId 获取对应的 Font。
    pub fn get_font_for_id(&self, id: FontId) -> Option<Font> {
        let lock = self.font_ids_by_font.read();
        lock.iter()
            .filter_map(|(font, result)| match result {
                Ok(font_id) if *font_id == id => Some(font.clone()),
                _ => None,
            })
            .next()
    }

    /// 解析指定的字体，如果字体加载失败则回退到默认字体栈。
    ///
    /// # Panics
    ///
    /// 如果字体和所有回退字体都无法解析，则会 panic。
    pub fn resolve_font(&self, font: &Font) -> FontId {
        if let Ok(font_id) = self.font_id(font) {
            return font_id;
        }
        for fallback in &self.fallback_font_stack {
            if let Ok(font_id) = self.font_id(fallback) {
                return font_id;
            }
        }

        panic!(
            "failed to resolve font '{}' or any of the fallbacks: {}",
            font.family,
            self.fallback_font_stack
                .iter()
                .map(|fallback| &fallback.family)
                .join(", ")
        );
    }

    /// 获取指定字体和字号的边界框。
    /// 字体的边界框是能够包含该字体中所有字形的最小矩形（将所有字形叠加在一起）。
    pub fn bounding_box(&self, font_id: FontId, font_size: Pixels) -> Bounds<Pixels> {
        self.read_metrics(font_id, |metrics| metrics.bounding_box(font_size))
    }

    /// 获取指定字符在给定字体和字号下的排版边界。
    pub fn typographic_bounds(
        &self,
        font_id: FontId,
        font_size: Pixels,
        character: char,
    ) -> Result<Bounds<Pixels>> {
        let glyph_id = self
            .platform_text_system
            .glyph_for_char(font_id, character)
            .with_context(|| format!("glyph not found for character '{character}'"))?;
        let bounds = self
            .platform_text_system
            .typographic_bounds(font_id, glyph_id)?;
        Ok(self.read_metrics(font_id, |metrics| {
            (bounds / metrics.units_per_em as f32 * font_size.0).map(px)
        }))
    }

    /// 获取指定字符在给定字体和字号下的前进宽度。
    pub fn advance(&self, font_id: FontId, font_size: Pixels, ch: char) -> Result<Size<Pixels>> {
        let glyph_id = self
            .platform_text_system
            .glyph_for_char(font_id, ch)
            .with_context(|| format!("glyph not found for character '{ch}'"))?;
        let result = self.platform_text_system.advance(font_id, glyph_id)?
            / self.units_per_em(font_id) as f32;

        Ok(result * font_size)
    }

    // Consider removing this?
    /// Returns the shaped layout width of for the given character, in the given font and size.
    pub fn layout_width(&self, font_id: FontId, font_size: Pixels, ch: char) -> Pixels {
        let mut buffer = [0; 4];
        let buffer = ch.encode_utf8(&mut buffer);
        self.platform_text_system
            .layout_line(
                buffer,
                font_size,
                &[FontRun {
                    len: buffer.len(),
                    font_id,
                }],
            )
            .width
    }

    /// 返回一个 `em` 的宽度。
    ///
    /// 使用给定字体和字号下 `m` 字符的宽度。
    pub fn em_width(&self, font_id: FontId, font_size: Pixels) -> Result<Pixels> {
        Ok(self.typographic_bounds(font_id, font_size, 'm')?.size.width)
    }

    /// 返回一个 `em` 的前进宽度。
    ///
    /// 使用给定字体和字号下 `m` 字符的前进宽度。
    pub fn em_advance(&self, font_id: FontId, font_size: Pixels) -> Result<Pixels> {
        Ok(self.advance(font_id, font_size, 'm')?.width)
    }

    /// 返回一个 `ch` 的宽度。
    ///
    /// 使用给定字体和字号下 `0` 字符的宽度。
    pub fn ch_width(&self, font_id: FontId, font_size: Pixels) -> Result<Pixels> {
        Ok(self.typographic_bounds(font_id, font_size, '0')?.size.width)
    }

    /// 返回一个 `ch` 的前进宽度。
    ///
    /// 使用给定字体和字号下 `0` 字符的前进宽度。
    pub fn ch_advance(&self, font_id: FontId, font_size: Pixels) -> Result<Pixels> {
        Ok(self.advance(font_id, font_size, '0')?.width)
    }

    /// 获取每个 'em 方块' 的字体大小单位数，
    /// 根据 MDN："一个抽象方块，其高度是相同字号下行间距的预期距离"
    pub fn units_per_em(&self, font_id: FontId) -> u32 {
        self.read_metrics(font_id, |metrics| metrics.units_per_em)
    }

    /// 获取指定字体和字号下大写字母的高度。
    pub fn cap_height(&self, font_id: FontId, font_size: Pixels) -> Pixels {
        self.read_metrics(font_id, |metrics| metrics.cap_height(font_size))
    }

    /// 获取指定字体和字号下 x 字符的高度。
    pub fn x_height(&self, font_id: FontId, font_size: Pixels) -> Pixels {
        self.read_metrics(font_id, |metrics| metrics.x_height(font_size))
    }

    /// 获取指定字体的推荐基线上方距离
    pub fn ascent(&self, font_id: FontId, font_size: Pixels) -> Pixels {
        self.read_metrics(font_id, |metrics| metrics.ascent(font_size))
    }

    /// 获取指定字体在单倍行距文本中的推荐基线下方距离。
    pub fn descent(&self, font_id: FontId, font_size: Pixels) -> Pixels {
        self.read_metrics(font_id, |metrics| metrics.descent(font_size))
    }

    /// 获取指定字体和行高的推荐基线偏移量。
    pub fn baseline_offset(
        &self,
        font_id: FontId,
        font_size: Pixels,
        line_height: Pixels,
    ) -> Pixels {
        let ascent = self.ascent(font_id, font_size);
        let descent = self.descent(font_id, font_size);
        let padding_top = (line_height - ascent - descent) / 2.;
        padding_top + ascent
    }

    fn read_metrics<T>(&self, font_id: FontId, read: impl FnOnce(&FontMetrics) -> T) -> T {
        let lock = self.font_metrics.upgradable_read();

        if let Some(metrics) = lock.get(&font_id) {
            read(metrics)
        } else {
            let mut lock = RwLockUpgradableReadGuard::upgrade(lock);
            let metrics = lock
                .entry(font_id)
                .or_insert_with(|| self.platform_text_system.font_metrics(font_id));
            read(metrics)
        }
    }

    /// 返回指定字体和字号的行包装器句柄。
    pub fn line_wrapper(self: &Arc<Self>, font: Font, font_size: Pixels) -> LineWrapperHandle {
        let lock = &mut self.wrapper_pool.lock();
        let font_id = self.resolve_font(&font);
        let wrappers = lock
            .entry(FontIdWithSize { font_id, font_size })
            .or_default();
        let wrapper = wrappers
            .pop()
            .unwrap_or_else(|| LineWrapper::new(font_id, font_size, self.clone()));

        LineWrapperHandle {
            wrapper: Some(wrapper),
            text_system: self.clone(),
        }
    }

    /// 获取特定已渲染字形的光栅化大小和位置。
    pub(crate) fn raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        let raster_bounds = self.raster_bounds.upgradable_read();
        if let Some(bounds) = raster_bounds.get(params) {
            Ok(*bounds)
        } else {
            let mut raster_bounds = RwLockUpgradableReadGuard::upgrade(raster_bounds);
            let bounds = self.platform_text_system.glyph_raster_bounds(params)?;
            raster_bounds.insert(params.clone(), bounds);
            Ok(bounds)
        }
    }

    pub(crate) fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        let raster_bounds = self.raster_bounds(params)?;
        self.platform_text_system
            .rasterize_glyph(params, raster_bounds)
    }

    /// 返回以给定颜色绘制字形时使用的膨胀级别。
    pub(crate) fn glyph_dilation_for_color(&self, color: Hsla) -> u8 {
        self.platform_text_system.glyph_dilation_for_color(color)
    }

    /// 返回平台为给定字体和字号推荐的文本渲染模式。
    /// 返回值永远不会是 [`TextRenderingMode::PlatformDefault`]。
    pub(crate) fn recommended_rendering_mode(
        &self,
        font_id: FontId,
        font_size: Pixels,
    ) -> TextRenderingMode {
        self.platform_text_system
            .recommended_rendering_mode(font_id, font_size)
    }
}

/// RGPUI 文本布局子系统。
#[derive(Deref)]
pub struct WindowTextSystem {
    line_layout_cache: LineLayoutCache,
    #[deref]
    text_system: Arc<TextSystem>,
}

impl WindowTextSystem {
    /// 使用指定的 TextSystem 创建一个新的 WindowTextSystem。
    pub fn new(text_system: Arc<TextSystem>) -> Self {
        Self {
            line_layout_cache: LineLayoutCache::new(text_system.platform_text_system.clone()),
            text_system,
        }
    }

    pub(crate) fn layout_index(&self) -> LineLayoutIndex {
        self.line_layout_cache.layout_index()
    }

    pub(crate) fn reuse_layouts(&self, index: Range<LineLayoutIndex>) {
        self.line_layout_cache.reuse_layouts(index)
    }

    pub(crate) fn truncate_layouts(&self, index: LineLayoutIndex) {
        self.line_layout_cache.truncate_layouts(index)
    }

    /// 对给定行进行排版，使用指定字号，用于屏幕绘制。
    /// 可通过 `runs` 参数对行的子集独立设置样式。
    ///
    /// 注意此方法只能排版单行文本。如果文本包含换行符则会 panic。
    /// 如需排版多行文本，请使用 [`Self::shape_text`]。
    pub fn shape_line(
        &self,
        text: SharedString,
        font_size: Pixels,
        runs: &[TextRun],
        force_width: Option<Pixels>,
    ) -> ShapedLine {
        debug_assert!(
            text.find('\n').is_none(),
            "text argument should not contain newlines"
        );

        let mut decoration_runs = SmallVec::<[DecorationRun; 32]>::new();
        for run in runs {
            if let Some(last_run) = decoration_runs.last_mut()
                && last_run.color == run.color
                && last_run.underline == run.underline
                && last_run.strikethrough == run.strikethrough
                && last_run.background_color == run.background_color
            {
                last_run.len += run.len as u32;
                continue;
            }
            decoration_runs.push(DecorationRun {
                len: run.len as u32,
                color: run.color,
                background_color: run.background_color,
                underline: run.underline,
                strikethrough: run.strikethrough,
            });
        }

        let layout = self.layout_line(&text, font_size, runs, force_width);

        ShapedLine {
            layout,
            text,
            decoration_runs,
        }
    }

    /// 使用调用者提供的内容哈希作为缓存键对给定行进行排版。
    ///
    /// 这使得无需实例化连续的 `SharedString` 即可命中缓存。
    /// 如果缓存未命中，将调用 `materialize_text` 来生成用于排版的 `SharedString`。
    ///
    /// 契约（调用者保证）：
    /// - 相同的 `text_hash` 意味着相同的文本内容（碰撞风险由调用者承担）。
    /// - `text_len` 应为文本的 UTF-8 字节长度（有助于减少意外碰撞）。
    ///
    /// 与 [`Self::shape_line`] 一样，此方法只能用于单行文本（无 `\n`）。
    pub fn shape_line_by_hash(
        &self,
        text_hash: u64,
        text_len: usize,
        font_size: Pixels,
        runs: &[TextRun],
        force_width: Option<Pixels>,
        materialize_text: impl FnOnce() -> SharedString,
    ) -> ShapedLine {
        let mut decoration_runs = SmallVec::<[DecorationRun; 32]>::new();
        for run in runs {
            if let Some(last_run) = decoration_runs.last_mut()
                && last_run.color == run.color
                && last_run.underline == run.underline
                && last_run.strikethrough == run.strikethrough
                && last_run.background_color == run.background_color
            {
                last_run.len += run.len as u32;
                continue;
            }
            decoration_runs.push(DecorationRun {
                len: run.len as u32,
                color: run.color,
                background_color: run.background_color,
                underline: run.underline,
                strikethrough: run.strikethrough,
            });
        }

        let mut used_force_width = force_width;
        let layout = self.layout_line_by_hash(
            text_hash,
            text_len,
            font_size,
            runs,
            used_force_width,
            || {
                let text = materialize_text();
                debug_assert!(
                    text.find('\n').is_none(),
                    "text argument should not contain newlines"
                );
                text
            },
        );

        // We only materialize actual text on cache miss; on hit we avoid allocations.
        // Since `ShapedLine` carries a `SharedString`, use an empty placeholder for hits.
        // NOTE: Callers must not rely on `ShapedLine.text` for content when using this API.
        let text: SharedString = SharedString::new_static("");

        ShapedLine {
            layout,
            text,
            decoration_runs,
        }
    }

    /// 对多行文本字符串进行排版，使用指定字号，用于屏幕绘制。
    /// 可通过 `runs` 参数对文本的子集独立设置样式。
    /// 如果提供了 `wrap_width`，将调整换行以适应指定宽度。
    pub fn shape_text(
        &self,
        text: SharedString,
        font_size: Pixels,
        runs: &[TextRun],
        wrap_width: Option<Pixels>,
        line_clamp: Option<usize>,
    ) -> Result<SmallVec<[WrappedLine; 1]>> {
        let mut runs = runs.iter().filter(|run| run.len > 0).cloned().peekable();
        let mut font_runs = self.font_runs_pool.lock().pop().unwrap_or_default();

        let mut lines = SmallVec::new();
        let mut max_wrap_lines = line_clamp;
        let mut wrapped_lines = 0;

        let mut process_line = |line_text: SharedString, line_start, line_end| {
            font_runs.clear();

            let mut decoration_runs = <Vec<DecorationRun>>::with_capacity(32);
            let mut run_start = line_start;
            while run_start < line_end {
                let Some(run) = runs.peek_mut() else {
                    log::warn!("`TextRun`s do not cover the entire to be shaped text");
                    break;
                };

                let run_len_within_line = cmp::min(line_end - run_start, run.len);

                let decoration_changed = if let Some(last_run) = decoration_runs.last_mut()
                    && last_run.color == run.color
                    && last_run.underline == run.underline
                    && last_run.strikethrough == run.strikethrough
                    && last_run.background_color == run.background_color
                {
                    last_run.len += run_len_within_line as u32;
                    false
                } else {
                    decoration_runs.push(DecorationRun {
                        len: run_len_within_line as u32,
                        color: run.color,
                        background_color: run.background_color,
                        underline: run.underline,
                        strikethrough: run.strikethrough,
                    });
                    true
                };

                let font_id = self.resolve_font(&run.font);
                if let Some(font_run) = font_runs.last_mut()
                    && font_id == font_run.font_id
                    && !decoration_changed
                {
                    font_run.len += run_len_within_line;
                } else {
                    font_runs.push(FontRun {
                        len: run_len_within_line,
                        font_id,
                    });
                }

                // Preserve the remainder of the run for the next line
                run.len -= run_len_within_line;
                if run.len == 0 {
                    runs.next();
                }
                run_start += run_len_within_line;
            }

            let layout = self.line_layout_cache.layout_wrapped_line(
                &line_text,
                font_size,
                &font_runs,
                wrap_width,
                max_wrap_lines.map(|max| max.saturating_sub(wrapped_lines)),
            );
            wrapped_lines += layout.wrap_boundaries.len();

            lines.push(WrappedLine {
                layout,
                decoration_runs,
                text: line_text,
            });

            // Skip `\n` character.
            if let Some(run) = runs.peek_mut() {
                run.len -= 1;
                if run.len == 0 {
                    runs.next();
                }
            }
        };

        let mut split_lines = text.split('\n');

        // Special case single lines to prevent allocating a sharedstring
        if let Some(first_line) = split_lines.next()
            && let Some(second_line) = split_lines.next()
        {
            let mut line_start = 0;
            process_line(
                SharedString::new(first_line),
                line_start,
                line_start + first_line.len(),
            );
            line_start += first_line.len() + '\n'.len_utf8();
            process_line(
                SharedString::new(second_line),
                line_start,
                line_start + second_line.len(),
            );
            for line_text in split_lines {
                line_start += line_text.len() + '\n'.len_utf8();
                process_line(
                    SharedString::new(line_text),
                    line_start,
                    line_start + line_text.len(),
                );
            }
        } else {
            let end = text.len();
            process_line(text, 0, end);
        }

        self.font_runs_pool.lock().push(font_runs);

        Ok(lines)
    }

    pub(crate) fn finish_frame(&self) {
        self.line_layout_cache.finish_frame()
    }

    /// 对给定行文本进行布局，使用指定字号。
    /// 可通过 `runs` 参数对行的子集独立设置样式。
    /// 通常应优先使用 [`Self::shape_line`]，它可直接用于绘制。
    pub fn layout_line(
        &self,
        text: &str,
        font_size: Pixels,
        runs: &[TextRun],
        force_width: Option<Pixels>,
    ) -> Arc<LineLayout> {
        let mut last_run = None::<&TextRun>;
        let mut font_runs = self.font_runs_pool.lock().pop().unwrap_or_default();
        font_runs.clear();

        for run in runs.iter() {
            let decoration_changed = if let Some(last_run) = last_run
                && last_run.color == run.color
                && last_run.underline == run.underline
                && last_run.strikethrough == run.strikethrough
            // we do not consider differing background color relevant, as it does not affect glyphs
            // && last_run.background_color == run.background_color
            {
                false
            } else {
                last_run = Some(run);
                true
            };

            let font_id = self.resolve_font(&run.font);
            if let Some(font_run) = font_runs.last_mut()
                && font_id == font_run.font_id
                && !decoration_changed
            {
                font_run.len += run.len;
            } else {
                font_runs.push(FontRun {
                    len: run.len,
                    font_id,
                });
            }
        }

        let layout = self.line_layout_cache.layout_line(
            &SharedString::new(text),
            font_size,
            &font_runs,
            force_width,
        );

        self.font_runs_pool.lock().push(font_runs);

        layout
    }

    /// 返回指定字符在给定字体和字号下的排版布局宽度。
    pub fn layout_width(&self, font_id: FontId, font_size: Pixels, ch: char) -> Pixels {
        let mut buffer = [0; 4];
        let buffer: &_ = ch.encode_utf8(&mut buffer);
        self.line_layout_cache
            .layout_line(
                buffer,
                font_size,
                &[FontRun {
                    len: buffer.len(),
                    font_id,
                }],
                None,
            )
            .width
    }

    /// 返回一个 `em` 的排版布局宽度。
    pub fn em_layout_width(&self, font_id: FontId, font_size: Pixels) -> Pixels {
        self.layout_width(font_id, font_size, 'm')
    }

    /// 使用调用者提供的内容哈希探测行布局缓存，无需分配内存。
    ///
    /// 如果布局已在当前帧或上一帧中缓存，则返回 `Some(layout)`。未缓存则返回 `None`。
    ///
    /// 契约（调用者保证）：
    /// - 相同的 `text_hash` 意味着相同的文本内容（碰撞风险由调用者承担）。
    /// - `text_len` 应为文本的 UTF-8 字节长度（有助于减少意外碰撞）。
    pub fn try_layout_line_by_hash(
        &self,
        text_hash: u64,
        text_len: usize,
        font_size: Pixels,
        runs: &[TextRun],
        force_width: Option<Pixels>,
    ) -> Option<Arc<LineLayout>> {
        let mut last_run = None::<&TextRun>;
        let mut font_runs = self.font_runs_pool.lock().pop().unwrap_or_default();
        font_runs.clear();

        for run in runs.iter() {
            let decoration_changed = if let Some(last_run) = last_run
                && last_run.color == run.color
                && last_run.underline == run.underline
                && last_run.strikethrough == run.strikethrough
            // we do not consider differing background color relevant, as it does not affect glyphs
            // && last_run.background_color == run.background_color
            {
                false
            } else {
                last_run = Some(run);
                true
            };

            let font_id = self.resolve_font(&run.font);
            if let Some(font_run) = font_runs.last_mut()
                && font_id == font_run.font_id
                && !decoration_changed
            {
                font_run.len += run.len;
            } else {
                font_runs.push(FontRun {
                    len: run.len,
                    font_id,
                });
            }
        }

        let layout = self.line_layout_cache.try_layout_line_by_hash(
            text_hash,
            text_len,
            font_size,
            &font_runs,
            force_width,
        );

        self.font_runs_pool.lock().push(font_runs);

        layout
    }

    /// 使用调用者提供的内容哈希作为缓存键对给定行文本进行布局。
    ///
    /// 这使得无需实例化连续的 `SharedString` 即可命中缓存。
    /// 如果缓存未命中，将调用 `materialize_text` 来生成用于排版的 `SharedString`。
    ///
    /// 契约（调用者保证）：
    /// - 相同的 `text_hash` 意味着相同的文本内容（碰撞风险由调用者承担）。
    /// - `text_len` 应为文本的 UTF-8 字节长度（有助于减少意外碰撞）。
    pub fn layout_line_by_hash(
        &self,
        text_hash: u64,
        text_len: usize,
        font_size: Pixels,
        runs: &[TextRun],
        force_width: Option<Pixels>,
        materialize_text: impl FnOnce() -> SharedString,
    ) -> Arc<LineLayout> {
        let mut last_run = None::<&TextRun>;
        let mut font_runs = self.font_runs_pool.lock().pop().unwrap_or_default();
        font_runs.clear();

        for run in runs.iter() {
            let decoration_changed = if let Some(last_run) = last_run
                && last_run.color == run.color
                && last_run.underline == run.underline
                && last_run.strikethrough == run.strikethrough
            // we do not consider differing background color relevant, as it does not affect glyphs
            // && last_run.background_color == run.background_color
            {
                false
            } else {
                last_run = Some(run);
                true
            };

            let font_id = self.resolve_font(&run.font);
            if let Some(font_run) = font_runs.last_mut()
                && font_id == font_run.font_id
                && !decoration_changed
            {
                font_run.len += run.len;
            } else {
                font_runs.push(FontRun {
                    len: run.len,
                    font_id,
                });
            }
        }

        let layout = self.line_layout_cache.layout_line_by_hash(
            text_hash,
            text_len,
            font_size,
            &font_runs,
            force_width,
            materialize_text,
        );

        self.font_runs_pool.lock().push(font_runs);

        layout
    }
}

#[derive(Hash, Eq, PartialEq)]
struct FontIdWithSize {
    font_id: FontId,
    font_size: Pixels,
}

/// 文本系统句柄，可用于计算文本的换行布局
pub struct LineWrapperHandle {
    wrapper: Option<LineWrapper>,
    text_system: Arc<TextSystem>,
}

impl Drop for LineWrapperHandle {
    fn drop(&mut self) {
        let mut state = self.text_system.wrapper_pool.lock();
        let wrapper = self.wrapper.take().unwrap();
        state
            .get_mut(&FontIdWithSize {
                font_id: wrapper.font_id,
                font_size: wrapper.font_size,
            })
            .unwrap()
            .push(wrapper);
    }
}

impl Deref for LineWrapperHandle {
    type Target = LineWrapper;

    fn deref(&self) -> &Self::Target {
        self.wrapper.as_ref().unwrap()
    }
}

impl DerefMut for LineWrapperHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.wrapper.as_mut().unwrap()
    }
}

/// 字体的黑度或笔画粗细程度。该值范围为 100.0 到 900.0，
/// 400.0 为常规值。
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize, Add, Sub, FromStr)]
#[serde(transparent)]
pub struct FontWeight(pub f32);

impl Display for FontWeight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<f32> for FontWeight {
    fn from(weight: f32) -> Self {
        FontWeight(weight)
    }
}

impl Default for FontWeight {
    #[inline]
    fn default() -> FontWeight {
        FontWeight::NORMAL
    }
}

impl Hash for FontWeight {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(u32::from_be_bytes(self.0.to_be_bytes()));
    }
}

impl Eq for FontWeight {}

impl FontWeight {
    /// 细体 (100)，最细的值。
    pub const THIN: FontWeight = FontWeight(100.0);
    /// 特细体 (200)。
    pub const EXTRA_LIGHT: FontWeight = FontWeight(200.0);
    /// 浅细体 (300)。
    pub const LIGHT: FontWeight = FontWeight(300.0);
    /// 常规 (400)。
    pub const NORMAL: FontWeight = FontWeight(400.0);
    /// 中粗体 (500，比常规稍粗)。
    pub const MEDIUM: FontWeight = FontWeight(500.0);
    /// 半粗体 (600)。
    pub const SEMIBOLD: FontWeight = FontWeight(600.0);
    /// 粗体 (700)。
    pub const BOLD: FontWeight = FontWeight(700.0);
    /// 特粗体 (800)。
    pub const EXTRA_BOLD: FontWeight = FontWeight(800.0);
    /// 黑体 (900)，最粗的值。
    pub const BLACK: FontWeight = FontWeight(900.0);

    /// 所有字重，按从细到粗排列。
    pub const ALL: [FontWeight; 9] = [
        Self::THIN,
        Self::EXTRA_LIGHT,
        Self::LIGHT,
        Self::NORMAL,
        Self::MEDIUM,
        Self::SEMIBOLD,
        Self::BOLD,
        Self::EXTRA_BOLD,
        Self::BLACK,
    ];
}

impl schemars::JsonSchema for FontWeight {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "FontWeight".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        use schemars::json_schema;
        json_schema!({
            "type": "number",
            "minimum": Self::THIN,
            "maximum": Self::BLACK,
            "default": Self::default(),
            "description": "Font weight value between 100 (thin) and 900 (black)"
        })
    }
}

/// 允许选择斜体或倾斜字体。
#[derive(Clone, Copy, Eq, PartialEq, Debug, Hash, Default, Serialize, Deserialize, JsonSchema)]
pub enum FontStyle {
    /// 既非斜体也非倾斜的字体。
    #[default]
    Normal,
    /// 通常为草书体的字体形式。
    Italic,
    /// 常规字体的倾斜版本。
    Oblique,
}

impl Display for FontStyle {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

/// 带样式的文本段，用于 [`crate::TextLayout`]。
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TextRun {
    /// UTF-8 字节数
    pub len: usize,
    /// 本段使用的字体。
    pub font: Font,
    /// 文本颜色
    pub color: Hsla,
    /// 背景颜色（如果有）
    pub background_color: Option<Hsla>,
    /// 下划线样式（如果有）
    pub underline: Option<UnderlineStyle>,
    /// 删除线样式（如果有）
    pub strikethrough: Option<StrikethroughStyle>,
}

#[cfg(all(target_os = "macos", test))]
impl TextRun {
    fn with_len(&self, len: usize) -> Self {
        let mut this = self.clone();
        this.len = len;
        this
    }
}

/// 特定字形的标识符，由 [`WindowTextSystem::layout_line`] 返回。
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct GlyphId(pub u32);

/// 字形渲染参数，用作光栅化边界和精灵图集的缓存键。
///
/// 标识特定的字形渲染配置，包括字体、大小、亚像素定位和缩放因子。
#[derive(Clone, Debug, PartialEq)]
pub struct RenderGlyphParams {
    /// 字体 ID。
    pub font_id: FontId,
    /// 字形 ID。
    pub glyph_id: GlyphId,
    /// 字体大小（逻辑像素）。
    pub font_size: Pixels,
    /// 亚像素位置变体（用于缓存不同亚像素位置的光栅化结果）。
    pub subpixel_variant: Point<u8>,
    /// 设备缩放因子。
    pub scale_factor: f32,
    /// 是否为 Emoji 字形。
    pub is_emoji: bool,
    /// 是否启用亚像素渲染（LCD 抗锯齿）。
    pub subpixel_rendering: bool,
    /// 字形膨胀级别（用于彩色渲染）。
    pub dilation: u8,
}

impl Eq for RenderGlyphParams {}

impl Hash for RenderGlyphParams {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.font_id.0.hash(state);
        self.glyph_id.0.hash(state);
        self.font_size.0.to_bits().hash(state);
        self.subpixel_variant.hash(state);
        self.scale_factor.to_bits().hash(state);
        self.is_emoji.hash(state);
        self.subpixel_rendering.hash(state);
        self.dilation.hash(state);
    }
}

/// 用于标识特定字体的配置详情。
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Font {
    /// 字体族名称。
    ///
    /// 特殊名称 ".SystemUIFont" 用于标识系统 UI 字体，因平台而异。
    pub family: SharedString,

    /// 使用的字体特性。
    pub features: FontFeatures,

    /// 使用的回退字体。
    pub fallbacks: Option<FontFallbacks>,

    /// 字体粗细。
    pub weight: FontWeight,

    /// 字体样式。
    pub style: FontStyle,
}

impl Default for Font {
    fn default() -> Self {
        font(".SystemUIFont")
    }
}

/// 根据给定名称获取 [`Font`]。
pub fn font(family: impl Into<SharedString>) -> Font {
    Font {
        family: family.into(),
        features: FontFeatures::default(),
        weight: FontWeight::default(),
        style: FontStyle::default(),
        fallbacks: None,
    }
}

impl Font {
    /// 将此字体设为粗体
    pub fn bold(mut self) -> Self {
        self.weight = FontWeight::BOLD;
        self
    }

    /// 将此字体设为斜体
    pub fn italic(mut self) -> Self {
        self.style = FontStyle::Italic;
        self
    }
}

/// 用于存储字体度量信息的结构体。
/// 用于定义字体的度量尺寸。
#[derive(Clone, Copy, Debug)]
pub struct FontMetrics {
    /// 组成 "em 方块" 的字体单位数，
    /// 一个用于确定字体大小的可伸缩网格。
    pub units_per_em: u32,

    /// 从字体基线到字形顶部的垂直距离。
    pub ascent: f32,

    /// 从字体基线到底部的垂直距离。
    pub descent: f32,

    /// 推荐的行间额外间距。
    pub line_gap: f32,

    /// 下划线的建议位置。
    pub underline_position: f32,

    /// 下划线的建议粗细。
    pub underline_thickness: f32,

    /// 从字体基线测量的大写字母高度。
    pub cap_height: f32,

    /// 小写 x 的高度。
    pub x_height: f32,

    /// 字体覆盖区域的外部边界。
    /// 对应 OpenType `head` 表中的 xMin / xMax / yMin / yMax 值
    pub bounding_box: Bounds<f32>,
}

impl FontMetrics {
    /// 以像素为单位返回从字体基线到字形顶部的垂直距离。
    pub fn ascent(&self, font_size: Pixels) -> Pixels {
        Pixels((self.ascent / self.units_per_em as f32) * font_size.0)
    }

    /// 以像素为单位返回从字体基线到底部的垂直距离。
    pub fn descent(&self, font_size: Pixels) -> Pixels {
        Pixels((self.descent / self.units_per_em as f32) * font_size.0)
    }

    /// 以像素为单位返回推荐的行间额外间距。
    pub fn line_gap(&self, font_size: Pixels) -> Pixels {
        Pixels((self.line_gap / self.units_per_em as f32) * font_size.0)
    }

    /// 以像素为单位返回下划线的建议位置。
    pub fn underline_position(&self, font_size: Pixels) -> Pixels {
        Pixels((self.underline_position / self.units_per_em as f32) * font_size.0)
    }

    /// 以像素为单位返回下划线的建议粗细。
    pub fn underline_thickness(&self, font_size: Pixels) -> Pixels {
        Pixels((self.underline_thickness / self.units_per_em as f32) * font_size.0)
    }

    /// 以像素为单位返回从字体基线测量的大写字母高度。
    pub fn cap_height(&self, font_size: Pixels) -> Pixels {
        Pixels((self.cap_height / self.units_per_em as f32) * font_size.0)
    }

    /// 以像素为单位返回小写 x 的高度。
    pub fn x_height(&self, font_size: Pixels) -> Pixels {
        Pixels((self.x_height / self.units_per_em as f32) * font_size.0)
    }

    /// 以像素为单位返回字体覆盖区域的外部边界。
    pub fn bounding_box(&self, font_size: Pixels) -> Bounds<Pixels> {
        (self.bounding_box / self.units_per_em as f32 * font_size.0).map(px)
    }
}

/// 将已知的虚拟字体名称映射到其实际等效字体。
#[allow(unused)]
pub fn font_name_with_fallbacks<'a>(name: &'a str, system: &'a str) -> &'a str {
    // Note: the "Zed Plex" fonts were deprecated as we are not allowed to use "Plex"
    // in a derived font name. They are essentially indistinguishable from IBM Plex/Lilex,
    // and so retained here for backward compatibility.
    match name {
        ".SystemUIFont" => system,
        ".ZedSans" | "Zed Plex Sans" => "IBM Plex Sans",
        ".ZedMono" | "Zed Plex Mono" => "Lilex",
        _ => name,
    }
}

/// 类似 [`font_name_with_fallbacks`]，但接受和返回 [`SharedString`] 引用。
#[allow(unused)]
pub fn font_name_with_fallbacks_shared<'a>(
    name: &'a SharedString,
    system: &'a SharedString,
) -> &'a SharedString {
    // Note: the "Zed Plex" fonts were deprecated as we are not allowed to use "Plex"
    // in a derived font name. They are essentially indistinguishable from IBM Plex/Lilex,
    // and so retained here for backward compatibility.
    match name.as_str() {
        ".SystemUIFont" => system,
        ".ZedSans" | "Zed Plex Sans" => const { &SharedString::new_static("IBM Plex Sans") },
        ".ZedMono" | "Zed Plex Mono" => const { &SharedString::new_static("Lilex") },
        _ => name,
    }
}
