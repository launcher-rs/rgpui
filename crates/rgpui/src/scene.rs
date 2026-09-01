//! 渲染场景：描述需要绘制的几何图元（圆角矩形、文本、阴影等）。

// todo("windows"): remove

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    AtlasTextureId, AtlasTile, Background, Bounds, ContentMask, Corners, Edges, Hsla, Pixels,
    Point, Radians, ScaledPixels, Size, bounds_tree::BoundsTree, point,
};
use std::{
    fmt::Debug,
    iter::Peekable,
    ops::{Add, Range, Sub},
    slice,
};

/// 路径顶点的缩放像素类型别名（已废弃，使用 `PathVertex<ScaledPixels>`）。
#[allow(non_camel_case_types, unused)]
pub type PathVertex_ScaledPixels = PathVertex<ScaledPixels>;

/// 绘制顺序值，数值越小越先绘制。
pub type DrawOrder = u32;

/// 渲染场景 — 收集所有绘制图元（四边形、路径、精灵、阴影等），供 GPU 批量渲染。
#[derive(Default)]
pub struct Scene {
    pub(crate) paint_operations: Vec<PaintOperation>,
    primitive_bounds: BoundsTree<ScaledPixels>,
    layer_stack: Vec<DrawOrder>,
    /// 阴影图元列表。
    pub shadows: Vec<Shadow>,
    /// 四边形图元列表。
    pub quads: Vec<Quad>,
    /// 矢量路径图元列表。
    pub paths: Vec<Path<ScaledPixels>>,
    /// 下划线图元列表。
    pub underlines: Vec<Underline>,
    /// 单色精灵图元列表。
    pub monochrome_sprites: Vec<MonochromeSprite>,
    /// 亚像素精灵图元列表。
    pub subpixel_sprites: Vec<SubpixelSprite>,
    /// 多色精灵图元列表。
    pub polychrome_sprites: Vec<PolychromeSprite>,
    /// 平台原生绘制表面列表。
    pub surfaces: Vec<PaintSurface>,
}

/// 渲染场景的构建和管理方法。
impl Scene {
    /// 清空场景中的所有绘制操作。
    pub fn clear(&mut self) {
        self.paint_operations.clear();
        self.primitive_bounds.clear();
        self.layer_stack.clear();
        self.paths.clear();
        self.shadows.clear();
        self.quads.clear();
        self.underlines.clear();
        self.monochrome_sprites.clear();
        self.subpixel_sprites.clear();
        self.polychrome_sprites.clear();
        self.surfaces.clear();
    }

    /// 返回场景中的绘制操作数量。
    pub fn len(&self) -> usize {
        self.paint_operations.len()
    }

    /// 推入一个新的绘制图层，后续图元将在此图层内绘制。
    pub fn push_layer(&mut self, bounds: Bounds<ScaledPixels>) {
        let order = self.primitive_bounds.insert(bounds);
        self.layer_stack.push(order);
        self.paint_operations
            .push(PaintOperation::StartLayer(bounds));
    }

    /// 弹出当前绘制图层，返回上一层。
    pub fn pop_layer(&mut self) {
        self.layer_stack.pop();
        self.paint_operations.push(PaintOperation::EndLayer);
    }

    /// 插入一个渲染图元到场景中。
    pub fn insert_primitive(&mut self, primitive: impl Into<Primitive>) {
        let mut primitive = primitive.into();
        let clipped_bounds = primitive
            .bounds()
            .intersect(&primitive.content_mask().bounds);

        if clipped_bounds.is_empty() {
            return;
        }

        let order = self
            .layer_stack
            .last()
            .copied()
            .unwrap_or_else(|| self.primitive_bounds.insert(clipped_bounds));
        match &mut primitive {
            Primitive::Shadow(shadow) => {
                shadow.order = order;
                self.shadows.push(*shadow);
            }
            Primitive::Quad(quad) => {
                quad.order = order;
                self.quads.push(*quad);
            }
            Primitive::Path(path) => {
                path.order = order;
                path.id = PathId(self.paths.len());
                self.paths.push(path.clone());
            }
            Primitive::Underline(underline) => {
                underline.order = order;
                self.underlines.push(*underline);
            }
            Primitive::MonochromeSprite(sprite) => {
                sprite.order = order;
                self.monochrome_sprites.push(*sprite);
            }
            Primitive::SubpixelSprite(sprite) => {
                sprite.order = order;
                self.subpixel_sprites.push(*sprite);
            }
            Primitive::PolychromeSprite(sprite) => {
                sprite.order = order;
                self.polychrome_sprites.push(*sprite);
            }
            Primitive::Surface(surface) => {
                surface.order = order;
                self.surfaces.push(surface.clone());
            }
        }
        self.paint_operations
            .push(PaintOperation::Primitive(primitive));
    }

    /// 重放指定范围内的绘制操作，从另一个场景复制图元。
    pub fn replay(&mut self, range: Range<usize>, prev_scene: &Scene) {
        for operation in &prev_scene.paint_operations[range] {
            match operation {
                PaintOperation::Primitive(primitive) => self.insert_primitive(primitive.clone()),
                PaintOperation::StartLayer(bounds) => self.push_layer(*bounds),
                PaintOperation::EndLayer => self.pop_layer(),
            }
        }
    }

    /// 完成场景构建，按绘制顺序排序所有图元。
    pub fn finish(&mut self) {
        self.shadows.sort_by_key(|shadow| shadow.order);
        self.quads.sort_by_key(|quad| quad.order);
        self.paths.sort_by_key(|path| path.order);
        self.underlines.sort_by_key(|underline| underline.order);
        self.monochrome_sprites
            .sort_by_key(|sprite| (sprite.order, sprite.tile.tile_id));
        self.subpixel_sprites
            .sort_by_key(|sprite| (sprite.order, sprite.tile.tile_id));
        self.polychrome_sprites
            .sort_by_key(|sprite| (sprite.order, sprite.tile.tile_id));
        self.surfaces.sort_by_key(|surface| surface.order);
    }

    /// 返回按绘制顺序分批的图元迭代器。
    pub fn batches(&self) -> impl Iterator<Item = PrimitiveBatch> + '_ {
        BatchIterator {
            shadows_start: 0,
            shadows_iter: self.shadows.iter().peekable(),
            quads_start: 0,
            quads_iter: self.quads.iter().peekable(),
            paths_start: 0,
            paths_iter: self.paths.iter().peekable(),
            underlines_start: 0,
            underlines_iter: self.underlines.iter().peekable(),
            monochrome_sprites_start: 0,
            monochrome_sprites_iter: self.monochrome_sprites.iter().peekable(),
            subpixel_sprites_start: 0,
            subpixel_sprites_iter: self.subpixel_sprites.iter().peekable(),
            polychrome_sprites_start: 0,
            polychrome_sprites_iter: self.polychrome_sprites.iter().peekable(),
            surfaces_start: 0,
            surfaces_iter: self.surfaces.iter().peekable(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Default)]
pub(crate) enum PrimitiveKind {
    Shadow,
    #[default]
    Quad,
    Path,
    Underline,
    MonochromeSprite,
    SubpixelSprite,
    PolychromeSprite,
    Surface,
}

pub(crate) enum PaintOperation {
    Primitive(Primitive),
    StartLayer(Bounds<ScaledPixels>),
    EndLayer,
}

/// 渲染图元 — 场景中所有可绘制元素的枚举。
#[derive(Clone)]
pub enum Primitive {
    /// 阴影
    Shadow(Shadow),
    /// 四边形
    Quad(Quad),
    /// 矢量路径
    Path(Path<ScaledPixels>),
    /// 下划线
    Underline(Underline),
    /// 单色精灵（灰度字形/图标）
    MonochromeSprite(MonochromeSprite),
    /// 亚像素精灵（LCD 抗锯齿字形）
    SubpixelSprite(SubpixelSprite),
    /// 多色精灵（彩色图像/Emoji）
    PolychromeSprite(PolychromeSprite),
    /// 平台原生绘制表面
    Surface(PaintSurface),
}

impl Primitive {
    /// 获取此图元的边界框。
    pub fn bounds(&self) -> &Bounds<ScaledPixels> {
        match self {
            Primitive::Shadow(shadow) => &shadow.bounds,
            Primitive::Quad(quad) => &quad.bounds,
            Primitive::Path(path) => &path.bounds,
            Primitive::Underline(underline) => &underline.bounds,
            Primitive::MonochromeSprite(sprite) => &sprite.bounds,
            Primitive::SubpixelSprite(sprite) => &sprite.bounds,
            Primitive::PolychromeSprite(sprite) => &sprite.bounds,
            Primitive::Surface(surface) => &surface.bounds,
        }
    }

    /// 获取此图元的内容裁剪蒙版。
    pub fn content_mask(&self) -> &ContentMask<ScaledPixels> {
        match self {
            Primitive::Shadow(shadow) => &shadow.content_mask,
            Primitive::Quad(quad) => &quad.content_mask,
            Primitive::Path(path) => &path.content_mask,
            Primitive::Underline(underline) => &underline.content_mask,
            Primitive::MonochromeSprite(sprite) => &sprite.content_mask,
            Primitive::SubpixelSprite(sprite) => &sprite.content_mask,
            Primitive::PolychromeSprite(sprite) => &sprite.content_mask,
            Primitive::Surface(surface) => &surface.content_mask,
        }
    }
}

struct BatchIterator<'a> {
    shadows_start: usize,
    shadows_iter: Peekable<slice::Iter<'a, Shadow>>,
    quads_start: usize,
    quads_iter: Peekable<slice::Iter<'a, Quad>>,
    paths_start: usize,
    paths_iter: Peekable<slice::Iter<'a, Path<ScaledPixels>>>,
    underlines_start: usize,
    underlines_iter: Peekable<slice::Iter<'a, Underline>>,
    monochrome_sprites_start: usize,
    monochrome_sprites_iter: Peekable<slice::Iter<'a, MonochromeSprite>>,
    subpixel_sprites_start: usize,
    subpixel_sprites_iter: Peekable<slice::Iter<'a, SubpixelSprite>>,
    polychrome_sprites_start: usize,
    polychrome_sprites_iter: Peekable<slice::Iter<'a, PolychromeSprite>>,
    surfaces_start: usize,
    surfaces_iter: Peekable<slice::Iter<'a, PaintSurface>>,
}

impl<'a> Iterator for BatchIterator<'a> {
    type Item = PrimitiveBatch;

    fn next(&mut self) -> Option<Self::Item> {
        let mut orders_and_kinds = [
            (
                self.shadows_iter.peek().map(|s| s.order),
                PrimitiveKind::Shadow,
            ),
            (self.quads_iter.peek().map(|q| q.order), PrimitiveKind::Quad),
            (self.paths_iter.peek().map(|q| q.order), PrimitiveKind::Path),
            (
                self.underlines_iter.peek().map(|u| u.order),
                PrimitiveKind::Underline,
            ),
            (
                self.monochrome_sprites_iter.peek().map(|s| s.order),
                PrimitiveKind::MonochromeSprite,
            ),
            (
                self.subpixel_sprites_iter.peek().map(|s| s.order),
                PrimitiveKind::SubpixelSprite,
            ),
            (
                self.polychrome_sprites_iter.peek().map(|s| s.order),
                PrimitiveKind::PolychromeSprite,
            ),
            (
                self.surfaces_iter.peek().map(|s| s.order),
                PrimitiveKind::Surface,
            ),
        ];
        orders_and_kinds.sort_by_key(|(order, kind)| (order.unwrap_or(u32::MAX), *kind));

        let first = orders_and_kinds[0];
        let second = orders_and_kinds[1];
        let (batch_kind, max_order_and_kind) = if first.0.is_some() {
            (first.1, (second.0.unwrap_or(u32::MAX), second.1))
        } else {
            return None;
        };

        match batch_kind {
            PrimitiveKind::Shadow => {
                let shadows_start = self.shadows_start;
                let mut shadows_end = shadows_start + 1;
                self.shadows_iter.next();
                while self
                    .shadows_iter
                    .next_if(|shadow| (shadow.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    shadows_end += 1;
                }
                self.shadows_start = shadows_end;
                Some(PrimitiveBatch::Shadows(shadows_start..shadows_end))
            }
            PrimitiveKind::Quad => {
                let quads_start = self.quads_start;
                let mut quads_end = quads_start + 1;
                self.quads_iter.next();
                while self
                    .quads_iter
                    .next_if(|quad| (quad.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    quads_end += 1;
                }
                self.quads_start = quads_end;
                Some(PrimitiveBatch::Quads(quads_start..quads_end))
            }
            PrimitiveKind::Path => {
                let paths_start = self.paths_start;
                let mut paths_end = paths_start + 1;
                self.paths_iter.next();
                while self
                    .paths_iter
                    .next_if(|path| (path.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    paths_end += 1;
                }
                self.paths_start = paths_end;
                Some(PrimitiveBatch::Paths(paths_start..paths_end))
            }
            PrimitiveKind::Underline => {
                let underlines_start = self.underlines_start;
                let mut underlines_end = underlines_start + 1;
                self.underlines_iter.next();
                while self
                    .underlines_iter
                    .next_if(|underline| (underline.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    underlines_end += 1;
                }
                self.underlines_start = underlines_end;
                Some(PrimitiveBatch::Underlines(underlines_start..underlines_end))
            }
            PrimitiveKind::MonochromeSprite => {
                let texture_id = self.monochrome_sprites_iter.peek().unwrap().tile.texture_id;
                let sprites_start = self.monochrome_sprites_start;
                let mut sprites_end = sprites_start + 1;
                self.monochrome_sprites_iter.next();
                while self
                    .monochrome_sprites_iter
                    .next_if(|sprite| {
                        (sprite.order, batch_kind) < max_order_and_kind
                            && sprite.tile.texture_id == texture_id
                    })
                    .is_some()
                {
                    sprites_end += 1;
                }
                self.monochrome_sprites_start = sprites_end;
                Some(PrimitiveBatch::MonochromeSprites {
                    texture_id,
                    range: sprites_start..sprites_end,
                })
            }
            PrimitiveKind::SubpixelSprite => {
                let texture_id = self.subpixel_sprites_iter.peek().unwrap().tile.texture_id;
                let sprites_start = self.subpixel_sprites_start;
                let mut sprites_end = sprites_start + 1;
                self.subpixel_sprites_iter.next();
                while self
                    .subpixel_sprites_iter
                    .next_if(|sprite| {
                        (sprite.order, batch_kind) < max_order_and_kind
                            && sprite.tile.texture_id == texture_id
                    })
                    .is_some()
                {
                    sprites_end += 1;
                }
                self.subpixel_sprites_start = sprites_end;
                Some(PrimitiveBatch::SubpixelSprites {
                    texture_id,
                    range: sprites_start..sprites_end,
                })
            }
            PrimitiveKind::PolychromeSprite => {
                let texture_id = self.polychrome_sprites_iter.peek().unwrap().tile.texture_id;
                let sprites_start = self.polychrome_sprites_start;
                let mut sprites_end = sprites_start + 1;
                self.polychrome_sprites_iter.next();
                while self
                    .polychrome_sprites_iter
                    .next_if(|sprite| {
                        (sprite.order, batch_kind) < max_order_and_kind
                            && sprite.tile.texture_id == texture_id
                    })
                    .is_some()
                {
                    sprites_end += 1;
                }
                self.polychrome_sprites_start = sprites_end;
                Some(PrimitiveBatch::PolychromeSprites {
                    texture_id,
                    range: sprites_start..sprites_end,
                })
            }
            PrimitiveKind::Surface => {
                let surfaces_start = self.surfaces_start;
                let mut surfaces_end = surfaces_start + 1;
                self.surfaces_iter.next();
                while self
                    .surfaces_iter
                    .next_if(|surface| (surface.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    surfaces_end += 1;
                }
                self.surfaces_start = surfaces_end;
                Some(PrimitiveBatch::Surfaces(surfaces_start..surfaces_end))
            }
        }
    }
}

/// 图元批处理 — 按类型分组的图元索引范围，用于 GPU 批量绘制。
#[derive(Debug)]
pub enum PrimitiveBatch {
    /// 阴影批
    Shadows(Range<usize>),
    /// 四边形批
    Quads(Range<usize>),
    /// 路径批
    Paths(Range<usize>),
    /// 下划线批
    Underlines(Range<usize>),
    /// 单色精灵批
    MonochromeSprites {
        /// 图集纹理 ID
        texture_id: AtlasTextureId,
        /// 索引范围
        range: Range<usize>,
    },
    /// 亚像素精灵批
    SubpixelSprites {
        /// 图集纹理 ID
        texture_id: AtlasTextureId,
        /// 索引范围
        range: Range<usize>,
    },
    /// 多色精灵批
    PolychromeSprites {
        /// 图集纹理 ID
        texture_id: AtlasTextureId,
        /// 索引范围
        range: Range<usize>,
    },
    /// 平台原生表面批
    Surfaces(Range<usize>),
}

/// 四边形图元 — 矩形区域，支持背景色、边框、圆角和变换。
#[derive(Default, Debug, Copy, Clone)]
#[repr(C)]
pub struct Quad {
    /// 绘制顺序。
    pub order: DrawOrder,
    /// 边框样式。
    pub border_style: BorderStyle,
    /// 边界矩形。
    pub bounds: Bounds<ScaledPixels>,
    /// 内容裁剪蒙版。
    pub content_mask: ContentMask<ScaledPixels>,
    /// 背景填充。
    pub background: Background,
    /// 边框颜色。
    pub border_color: Hsla,
    /// 圆角半径。
    pub corner_radii: Corners<ScaledPixels>,
    /// 边框宽度（上、右、下、左）。
    pub border_widths: Edges<ScaledPixels>,
    /// 是否使用连续圆角（0 = 普通，1 = 连续圆角）。
    pub continuous_corners: u32,
    /// 二维变换矩阵。
    pub transform: TransformationMatrix,
    /// 混合模式索引。
    pub blend_mode: u32,
    /// 8 字节对齐填充。
    pub pad_quad: u32,
}

impl From<Quad> for Primitive {
    fn from(quad: Quad) -> Self {
        Primitive::Quad(quad)
    }
}

/// 下划线图元 — 支持直线和波浪线样式。
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Underline {
    /// 绘制顺序。
    pub order: DrawOrder,
    /// 8 字节对齐填充。
    pub pad: u32, // align to 8 bytes
    /// 边界矩形。
    pub bounds: Bounds<ScaledPixels>,
    /// 内容裁剪蒙版。
    pub content_mask: ContentMask<ScaledPixels>,
    /// 颜色。
    pub color: Hsla,
    /// 线条粗细。
    pub thickness: ScaledPixels,
    /// 是否为波浪线（0 = 直线，1 = 波浪线）。
    pub wavy: u32,
}

impl From<Underline> for Primitive {
    fn from(underline: Underline) -> Self {
        Primitive::Underline(underline)
    }
}

/// 阴影图元 — 支持外阴影和内阴影，可配置模糊半径和颜色。
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Shadow {
    /// 绘制顺序。
    pub order: DrawOrder,
    /// 模糊半径。
    pub blur_radius: ScaledPixels,
    /// 边界矩形。
    pub bounds: Bounds<ScaledPixels>,
    /// 圆角半径。
    pub corner_radii: Corners<ScaledPixels>,
    /// 内容裁剪蒙版。
    pub content_mask: ContentMask<ScaledPixels>,
    /// 颜色。
    pub color: Hsla,
    /// 元素边界。
    pub element_bounds: Bounds<ScaledPixels>,
    /// 元素圆角半径。
    pub element_corner_radii: Corners<ScaledPixels>,
    /// 0 = 外阴影（绘制在元素外部），1 = 内阴影（绘制在元素内部）。
    pub inset: u32,
    /// 8 字节对齐填充。
    pub pad: u32,
}

impl From<Shadow> for Primitive {
    fn from(shadow: Shadow) -> Self {
        Primitive::Shadow(shadow)
    }
}

/// 边框的样式。
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[repr(C)]
pub enum BorderStyle {
    /// 实线边框。
    #[default]
    Solid = 0,
    /// 虚线边框。
    Dashed = 1,
}

/// 渲染四边形时应用的混合模式。
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[repr(C)]
pub enum BlendMode {
    /// 标准 alpha 混合（源覆盖目标）。
    #[default]
    Normal = 0,
    /// 通过将源颜色与自身相乘来变暗。
    Multiply = 1,
    /// 通过应用屏幕公式使源颜色变亮。
    Screen = 2,
    /// 基于源亮度结合乘法和屏幕。
    Overlay = 3,
    /// 覆盖的柔和版本，产生更柔和的对比度。
    SoftLight = 4,
    /// 从较亮的颜色中减去较暗的颜色。
    Difference = 5,
}

/// 表示可应用于元素的二维变换的数据类型。
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C, align(8))]
pub struct TransformationMatrix {
    /// 2x2 矩阵，包含旋转和缩放，
    /// 按行主序存储
    pub rotation_scale: [[f32; 2]; 2],
    /// 平移向量
    pub translation: [f32; 2],
}

impl Eq for TransformationMatrix {}

impl TransformationMatrix {
    /// 单位矩阵，无效果。
    pub fn unit() -> Self {
        Self {
            rotation_scale: [[1.0, 0.0], [0.0, 1.0]],
            translation: [0.0, 0.0],
        }
    }

    /// 将原点移动给定点
    pub fn translate(mut self, point: Point<ScaledPixels>) -> Self {
        self.compose(Self {
            rotation_scale: [[1.0, 0.0], [0.0, 1.0]],
            translation: [point.x.0, point.y.0],
        })
    }

    /// 绕原点顺时针旋转（以弧度为单位）
    pub fn rotate(self, angle: Radians) -> Self {
        self.compose(Self {
            rotation_scale: [
                [angle.0.cos(), -angle.0.sin()],
                [angle.0.sin(), angle.0.cos()],
            ],
            translation: [0.0, 0.0],
        })
    }

    /// 绕原点缩放
    pub fn scale(self, size: Size<f32>) -> Self {
        self.compose(Self {
            rotation_scale: [[size.width, 0.0], [0.0, size.height]],
            translation: [0.0, 0.0],
        })
    }

    /// 与另一个变换执行矩阵乘法
    /// 以产生一个新的变换，它是
    /// 应用两个变换的结果：首先 `other`，然后 `self`。
    #[inline]
    pub fn compose(self, other: TransformationMatrix) -> TransformationMatrix {
        if other == Self::unit() {
            return self;
        }
        // 执行矩阵乘法
        TransformationMatrix {
            rotation_scale: [
                [
                    self.rotation_scale[0][0] * other.rotation_scale[0][0]
                        + self.rotation_scale[0][1] * other.rotation_scale[1][0],
                    self.rotation_scale[0][0] * other.rotation_scale[0][1]
                        + self.rotation_scale[0][1] * other.rotation_scale[1][1],
                ],
                [
                    self.rotation_scale[1][0] * other.rotation_scale[0][0]
                        + self.rotation_scale[1][1] * other.rotation_scale[1][0],
                    self.rotation_scale[1][0] * other.rotation_scale[0][1]
                        + self.rotation_scale[1][1] * other.rotation_scale[1][1],
                ],
            ],
            translation: [
                self.translation[0]
                    + self.rotation_scale[0][0] * other.translation[0]
                    + self.rotation_scale[0][1] * other.translation[1],
                self.translation[1]
                    + self.rotation_scale[1][0] * other.translation[0]
                    + self.rotation_scale[1][1] * other.translation[1],
            ],
        }
    }

    /// 对点应用变换，主要用于调试
    pub fn apply(&self, point: Point<Pixels>) -> Point<Pixels> {
        let input = [point.x.0, point.y.0];
        let mut output = self.translation;
        for (i, output_cell) in output.iter_mut().enumerate() {
            for (k, input_cell) in input.iter().enumerate() {
                *output_cell += self.rotation_scale[i][k] * *input_cell;
            }
        }
        Point::new(output[0].into(), output[1].into())
    }
}

impl Default for TransformationMatrix {
    fn default() -> Self {
        Self::unit()
    }
}

/// 单色精灵 — 从图集纹理中采样的灰度字形或图标。
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct MonochromeSprite {
    /// 绘制顺序。
    pub order: DrawOrder,
    /// 8 字节对齐填充。
    pub pad: u32,
    /// 边界矩形。
    pub bounds: Bounds<ScaledPixels>,
    /// 内容裁剪蒙版。
    pub content_mask: ContentMask<ScaledPixels>,
    /// 颜色。
    pub color: Hsla,
    /// 图集纹理块。
    pub tile: AtlasTile,
    /// 二维变换矩阵。
    pub transformation: TransformationMatrix,
}

impl From<MonochromeSprite> for Primitive {
    fn from(sprite: MonochromeSprite) -> Self {
        Primitive::MonochromeSprite(sprite)
    }
}

/// 亚像素精灵 — 使用 LCD 抗锯齿渲染的字形，需要 RGB 三通道采样。
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct SubpixelSprite {
    /// 绘制顺序。
    pub order: DrawOrder,
    /// 8 字节对齐填充。
    pub pad: u32, // align to 8 bytes
    /// 边界矩形。
    pub bounds: Bounds<ScaledPixels>,
    /// 内容裁剪蒙版。
    pub content_mask: ContentMask<ScaledPixels>,
    /// 颜色。
    pub color: Hsla,
    /// 图集纹理块。
    pub tile: AtlasTile,
    /// 二维变换矩阵。
    pub transformation: TransformationMatrix,
}

impl From<SubpixelSprite> for Primitive {
    fn from(sprite: SubpixelSprite) -> Self {
        Primitive::SubpixelSprite(sprite)
    }
}

/// 多色精灵 — 从图集纹理中采样的彩色图像（Emoji、图标等）。
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct PolychromeSprite {
    /// 绘制顺序。
    pub order: DrawOrder,
    /// 8 字节对齐填充。
    pub pad: u32,
    /// 是否为灰度模式。
    pub grayscale: bool,
    /// 不透明度。
    pub opacity: f32,
    /// 边界矩形。
    pub bounds: Bounds<ScaledPixels>,
    /// 内容裁剪蒙版。
    pub content_mask: ContentMask<ScaledPixels>,
    /// 圆角半径。
    pub corner_radii: Corners<ScaledPixels>,
    /// 图集纹理块。
    pub tile: AtlasTile,
    /// 二维变换矩阵。
    pub transformation: TransformationMatrix,
}

impl From<PolychromeSprite> for Primitive {
    fn from(sprite: PolychromeSprite) -> Self {
        Primitive::PolychromeSprite(sprite)
    }
}

/// 绘制表面 — 用于平台原生表面渲染（如 macOS CVPixelBuffer）。
#[derive(Clone, Debug)]
pub struct PaintSurface {
    /// 绘制顺序。
    pub order: DrawOrder,
    /// 边界矩形。
    pub bounds: Bounds<ScaledPixels>,
    /// 内容裁剪蒙版。
    pub content_mask: ContentMask<ScaledPixels>,
    #[cfg(target_os = "macos")]
    /// 平台原生图像缓冲区。
    pub image_buffer: core_video::pixel_buffer::CVPixelBuffer,
}

impl From<PaintSurface> for Primitive {
    fn from(surface: PaintSurface) -> Self {
        Primitive::Surface(surface)
    }
}

/// 路径的唯一标识符。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PathId(pub usize);

/// 由一系列顶点和控制点组成的矢量路径，支持填充和变换。
#[derive(Clone, Debug)]
pub struct Path<P: Clone + Debug + Default + PartialEq> {
    /// 路径的唯一标识符。
    pub id: PathId,
    /// 绘制顺序。
    pub order: DrawOrder,
    /// 路径的边界矩形。
    pub bounds: Bounds<P>,
    /// 内容裁剪蒙版。
    pub content_mask: ContentMask<P>,
    /// 路径的顶点列表。
    pub vertices: Vec<PathVertex<P>>,
    /// 路径填充颜色。
    pub color: Background,
    start: Point<P>,
    current: Point<P>,
    contour_count: usize,
}

impl Path<Pixels> {
    /// 使用给定的起始点创建新路径。
    pub fn new(start: Point<Pixels>) -> Self {
        Self {
            id: PathId(0),
            order: DrawOrder::default(),
            vertices: Vec::new(),
            start,
            current: start,
            bounds: Bounds {
                origin: start,
                size: Default::default(),
            },
            content_mask: Default::default(),
            color: Default::default(),
            contour_count: 0,
        }
    }

    /// 按给定因子缩放此路径。
    pub fn scale(&self, factor: f32) -> Path<ScaledPixels> {
        Path {
            id: self.id,
            order: self.order,
            bounds: self.bounds.scale(factor),
            content_mask: self.content_mask.scale(factor),
            vertices: self
                .vertices
                .iter()
                .map(|vertex| vertex.scale(factor))
                .collect(),
            start: self.start.map(|start| start.scale(factor)),
            current: self.current.scale(factor),
            contour_count: self.contour_count,
            color: self.color,
        }
    }

    /// 将起点、当前点移动到给定点。
    pub fn move_to(&mut self, to: Point<Pixels>) {
        self.contour_count += 1;
        self.start = to;
        self.current = to;
    }

    /// 从当前点到给定点绘制直线。
    pub fn line_to(&mut self, to: Point<Pixels>) {
        self.contour_count += 1;
        if self.contour_count > 1 {
            self.push_triangle(
                (self.start, self.current, to),
                (point(0., 1.), point(0., 1.), point(0., 1.)),
            );
        }
        self.current = to;
    }

    /// 从当前点到给定点绘制曲线，使用给定的控制点。
    pub fn curve_to(&mut self, to: Point<Pixels>, ctrl: Point<Pixels>) {
        self.contour_count += 1;
        if self.contour_count > 1 {
            self.push_triangle(
                (self.start, self.current, to),
                (point(0., 1.), point(0., 1.), point(0., 1.)),
            );
        }

        self.push_triangle(
            (self.current, ctrl, to),
            (point(0., 0.), point(0.5, 0.), point(1., 1.)),
        );
        self.current = to;
    }

    /// 向 Path 添加三角形。
    pub fn push_triangle(
        &mut self,
        xy: (Point<Pixels>, Point<Pixels>, Point<Pixels>),
        st: (Point<f32>, Point<f32>, Point<f32>),
    ) {
        self.bounds = self
            .bounds
            .union(&Bounds {
                origin: xy.0,
                size: Default::default(),
            })
            .union(&Bounds {
                origin: xy.1,
                size: Default::default(),
            })
            .union(&Bounds {
                origin: xy.2,
                size: Default::default(),
            });

        self.vertices.push(PathVertex {
            xy_position: xy.0,
            st_position: st.0,
            content_mask: Default::default(),
        });
        self.vertices.push(PathVertex {
            xy_position: xy.1,
            st_position: st.1,
            content_mask: Default::default(),
        });
        self.vertices.push(PathVertex {
            xy_position: xy.2,
            st_position: st.2,
            content_mask: Default::default(),
        });
    }
}

impl<T> Path<T>
where
    T: Clone + Debug + Default + PartialEq + PartialOrd + Add<T, Output = T> + Sub<Output = T>,
{
    /// 返回路径与内容遮罩的交集边界。
    #[allow(unused)]
    pub fn clipped_bounds(&self) -> Bounds<T> {
        self.bounds.intersect(&self.content_mask.bounds)
    }
}

impl From<Path<ScaledPixels>> for Primitive {
    fn from(path: Path<ScaledPixels>) -> Self {
        Primitive::Path(path)
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
#[expect(missing_docs)]
pub struct PathVertex<P: Clone + Debug + Default + PartialEq> {
    pub xy_position: Point<P>,
    pub st_position: Point<f32>,
    pub content_mask: ContentMask<P>,
}

#[expect(missing_docs)]
impl PathVertex<Pixels> {
    pub fn scale(&self, factor: f32) -> PathVertex<ScaledPixels> {
        PathVertex {
            xy_position: self.xy_position.scale(factor),
            st_position: self.st_position,
            content_mask: self.content_mask.scale(factor),
        }
    }
}
