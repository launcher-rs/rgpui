//! 渲染场景：描述需要绘制的几何图元（圆角矩形、文本、阴影等）。

// todo("windows"): remove

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    AtlasTextureId, AtlasTile, Background, Bounds, ContentMask, Corners, Edges, Hsla, Path, Pixels,
    Point, Radians, ScaledPixels, Size, TimedTransformation, bounds_tree::BoundsTree,
    path_winding::PathTiles,
};
use smallvec::SmallVec;
use std::{
    collections::VecDeque, fmt::Debug, iter::Peekable, ops::Range, rc::Rc, slice, time::Duration,
};

/// 内部不变量：值为 `None` 说明前提被破坏（批处理迭代器与已排序的种类表不一致等），
/// 必须立刻暴露。
///
/// 统一走这里而不是散落的 `unwrap`：语义与原先一致（同样 panic），但把「这是不变量
/// 断言」的意图写进调用点，模块级 `deny` 仍能拦住随手写的 unwrap。
#[inline]
#[allow(clippy::panic)] // 不变量断言：前提被破坏必须立刻暴露
fn invariant<T>(value: Option<T>, message: &str) -> T {
    match value {
        Some(value) => value,
        None => panic!("{message}"),
    }
}

/// 绘制顺序值，数值越小越先绘制。
pub type DrawOrder = u32;

/// 挂起中的绘制图层：记录图层内图元在 `paint_operations` 中的区间、并集边界与
/// 种类聚合；图层关闭时才统一插入排序树分配绘制顺序（对标上游图元批处理改进）。
struct Layer {
    /// 图层内图元在 `paint_operations` 里的连续区间（可多段）。
    primitives: SmallVec<[Range<usize>; 1]>,
    /// 图层内所有图元的并集边界（空边界 = 还没有图元，union 以空为单位元）。
    bounds: Bounds<ScaledPixels>,
    /// 图层内最大图元种类（作为插入排序树的 key 种类）。
    max_kind: PrimitiveKind,
    /// 图层内最严的 `max_shared_kind`（图层整体能否共享他方排序值）。
    max_shared_kind: PrimitiveKind,
}

/// 渲染场景 — 收集所有绘制图元（四边形、文本、阴影等），供 GPU 批量渲染。
#[derive(Default)]
pub struct Scene {
    pub(crate) paint_operations: Vec<PaintOperation>,
    primitive_bounds: BoundsTree<ScaledPixels>,
    /// 图层在 `layers` 队列中的下标栈。
    layer_stack: Vec<usize>,
    /// 挂起图层队列（先进先出，最外层关闭时统一落地）。
    layers: VecDeque<Layer>,
    /// 阴影图元列表。
    pub shadows: Vec<Shadow>,
    /// 四边形图元列表。
    pub quads: Vec<Quad>,
    /// 矢量路径图元列表。
    pub paths: Vec<PaintedPath>,
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
    /// 场景级精灵动画表：每项指向 `monochrome_sprites` 的一个下标，由
    /// `advance_animations` 在呈现前推进（不重绘视图）。
    animations: Vec<SpriteAnimation>,
}

/// 渲染场景的构建和管理方法。
impl Scene {
    /// 清空场景中的所有绘制操作。
    pub fn clear(&mut self) {
        self.paint_operations.clear();
        self.primitive_bounds.clear();
        self.layer_stack.clear();
        self.layers.clear();
        self.paths.clear();
        self.shadows.clear();
        self.quads.clear();
        self.underlines.clear();
        self.monochrome_sprites.clear();
        self.subpixel_sprites.clear();
        self.polychrome_sprites.clear();
        self.surfaces.clear();
        self.animations.clear();
    }

    /// 返回场景中的绘制操作数量。
    pub fn len(&self) -> usize {
        self.paint_operations.len()
    }

    /// 推入一个新的绘制图层，后续图元将在此图层内绘制。
    ///
    /// 图层不立刻占用绘制顺序：关闭时以「图层内图元的并集边界」插入排序树，
    /// 按与其他图元相同的规则共享或新建绘制顺序；图层内所有图元共用该顺序。
    pub fn push_layer(&mut self) {
        self.layer_stack.push(self.layers.len());
        self.layers.push_back(Layer {
            primitives: SmallVec::new(),
            bounds: Bounds::default(),
            max_kind: PrimitiveKind::Shadow,
            max_shared_kind: PrimitiveKind::Surface,
        });
        self.paint_operations.push(PaintOperation::StartLayer);
    }

    /// 弹出当前绘制图层，返回上一层。
    ///
    /// 最外层图层关闭时，挂起队列里的所有图层（含嵌套）统一插入排序树。
    pub fn pop_layer(&mut self) {
        self.layer_stack.pop();
        if self.layer_stack.is_empty() {
            while let Some(layer) = self.layers.pop_front() {
                self.insert_layer(layer);
            }
        }
        self.paint_operations.push(PaintOperation::EndLayer);
    }

    /// 落地一个挂起图层：按并集边界分配绘制顺序，图层内图元共用该顺序。
    fn insert_layer(&mut self, layer: Layer) {
        if layer.primitives.is_empty() {
            return;
        }
        let order =
            self.primitive_bounds
                .insert(layer.bounds, layer.max_kind, layer.max_shared_kind);
        for index in layer.primitives.into_iter().flatten() {
            if let PaintOperation::Primitive(primitive) = &self.paint_operations[index] {
                let primitive = primitive.clone();
                self.push_primitive(primitive, order);
            }
        }
    }

    /// 插入一个渲染图元到场景中。
    ///
    /// 不在图层内时立即插入排序树分配绘制顺序；在图层内则并入图层，
    /// 关闭时统一落地。
    pub fn insert_primitive(&mut self, primitive: impl Into<Primitive>) {
        self.try_insert_primitive(primitive.into());
    }

    /// 插入一个渲染图元，返回它是否被保留（被裁剪掉时为 `false`）。
    ///
    /// 动画精灵插入失败时不能登记动画项，否则动画表会指向不存在的图元。
    fn try_insert_primitive(&mut self, primitive: Primitive) -> bool {
        let clipped_bounds = primitive
            .bounds()
            .intersect(&primitive.content_mask().bounds);

        if clipped_bounds.is_empty() {
            return false;
        }

        let kind = primitive.kind();
        let active_layer = self.layer_stack.last().copied();
        match active_layer.and_then(|layer_ix| self.layers.get_mut(layer_ix)) {
            Some(layer) => {
                // 首个图元直接取自身边界。不能用 `Bounds::default()` 做并集起点：
                // 它的原点是 (0,0)，会把每个图层都撑成从窗口左上角开始的大矩形，
                // 导致文本行图层互相相交、绘制顺序被错误抬高（弹窗会被下层文字盖住）。
                if layer.primitives.is_empty() {
                    layer.bounds = clipped_bounds;
                } else {
                    layer.bounds = layer.bounds.union(&clipped_bounds);
                }
                layer.max_kind = layer.max_kind.max(kind);
                layer.max_shared_kind = layer.max_shared_kind.min(kind.max_shared_kind());
                let index = self.paint_operations.len();
                match layer.primitives.last_mut() {
                    Some(range) if range.end == index => range.end += 1,
                    _ => layer.primitives.push(index..index + 1),
                }
            }
            None => {
                let order =
                    self.primitive_bounds
                        .insert(clipped_bounds, kind, kind.max_shared_kind());
                self.push_primitive(primitive.clone(), order);
            }
        }
        self.paint_operations
            .push(PaintOperation::Primitive(primitive));
        true
    }

    /// 插入一个由 [`Scene::advance_animations`] 持续变换的单色精灵。
    ///
    /// 精灵的颜色取动画 alpha 之前的底色；动画项记录它的下标、变换与中心，
    /// 每次呈现前按已过时间推进矩阵与 alpha，无须重绘视图。
    pub(crate) fn insert_animated_sprite(
        &mut self,
        mut sprite: MonochromeSprite,
        mut animation: SpriteAnimation,
    ) {
        sprite.animation = SpriteAnimationId(self.animations.len() as u32);
        animation.base_color = sprite.color;
        if self.try_insert_primitive(sprite.into()) {
            self.animations.push(animation);
        }
    }

    /// 场景里所有动画都要求呈现时，相邻两次呈现之间可以等待的最长时间；
    /// 场景里没有动画时为 `None`，某个动画要求每帧都画时为 `Some(ZERO)`。
    pub(crate) fn animation_frame_interval(&self) -> Option<Duration> {
        self.animations
            .iter()
            .map(|animation| {
                animation
                    .transformation
                    .min_frame_interval()
                    .unwrap_or_default()
            })
            .min()
    }

    /// 把每个动画精灵推进到动画时钟 `elapsed` 时刻的姿态，供呈现前调用。
    ///
    /// 只改场景中已有精灵的矩阵与 alpha，不触碰视图、不重建场景。
    pub(crate) fn advance_animations(&mut self, elapsed: Duration) {
        for animation in &self.animations {
            let Some(sprite) = self.monochrome_sprites.get_mut(animation.sprite_index) else {
                continue;
            };
            let pose = animation.transformation.at(elapsed);
            sprite.transformation = pose
                .transformation
                .into_matrix(animation.center, animation.scale_factor);
            sprite.color = animation.base_color.opacity(pose.alpha_factor());
        }
    }

    /// 按给定绘制顺序把图元放入对应的种类列表。
    fn push_primitive(&mut self, primitive: Primitive, order: DrawOrder) {
        match primitive {
            Primitive::Shadow(mut shadow) => {
                shadow.order = order;
                self.shadows.push(shadow);
            }
            Primitive::Quad(mut quad) => {
                quad.order = order;
                self.quads.push(quad);
            }
            Primitive::Path(mut path) => {
                path.order = order;
                self.paths.push(path);
            }
            Primitive::Underline(mut underline) => {
                underline.order = order;
                self.underlines.push(underline);
            }
            Primitive::MonochromeSprite(mut sprite) => {
                sprite.order = order;
                self.monochrome_sprites.push(sprite);
            }
            Primitive::SubpixelSprite(mut sprite) => {
                sprite.order = order;
                self.subpixel_sprites.push(sprite);
            }
            Primitive::PolychromeSprite(mut sprite) => {
                sprite.order = order;
                self.polychrome_sprites.push(sprite);
            }
            Primitive::Surface(mut surface) => {
                surface.order = order;
                self.surfaces.push(surface);
            }
        }
    }

    /// 重放指定范围内的绘制操作，从另一个场景复制图元。
    pub fn replay(&mut self, range: Range<usize>, prev_scene: &Scene) {
        for operation in &prev_scene.paint_operations[range] {
            match operation {
                PaintOperation::Primitive(primitive) => {
                    // 动画 id 是「插入时的场景」内的下标，重放时要取回动画定义。
                    if let Primitive::MonochromeSprite(sprite) = primitive
                        && let Some(animation) =
                            prev_scene.animations.get(sprite.animation.0 as usize)
                    {
                        self.insert_animated_sprite(*sprite, *animation);
                    } else {
                        self.insert_primitive(primitive.clone());
                    }
                }
                PaintOperation::StartLayer => self.push_layer(),
                PaintOperation::EndLayer => self.pop_layer(),
            }
        }
    }

    /// 完成场景构建，按绘制顺序排序所有图元。
    pub fn finish(&mut self) {
        debug_assert!(
            self.layer_stack.is_empty(),
            "场景图层未平衡（push/pop 不成对）"
        );
        // 排序策略见 `sort_in_place`：排小的 `(键, 下标)` 对，再把图元按置换
        // 逐环搬到位，每个图元只搬一次。`paths` / `surfaces` 不是 `Copy` 且数量少，
        // 继续用 `sort_by_key`。
        let keys = &mut Vec::new();
        sort_in_place(&mut self.shadows, keys, |shadow| (shadow.order, 0));
        sort_in_place(&mut self.quads, keys, |quad| (quad.order, 0));
        self.paths.sort_by_key(|path| path.order);
        sort_in_place(&mut self.underlines, keys, |underline| (underline.order, 0));
        // 合批以 texture_id 为界（见 `Scene::batches`），排序键必须与之一致：
        // 按 tile_id 排序会把同一张纹理的精灵打散成多个批次。
        sort_in_place(&mut self.monochrome_sprites, keys, |sprite| {
            (sprite.order, sprite.tile.texture_id.index)
        });
        // 排序打乱了精灵顺序，动画项要跟着找到自己的精灵下标。
        if !self.animations.is_empty() {
            for (index, sprite) in self.monochrome_sprites.iter().enumerate() {
                if let Some(animation) = self.animations.get_mut(sprite.animation.0 as usize) {
                    animation.sprite_index = index;
                }
            }
        }
        sort_in_place(&mut self.subpixel_sprites, keys, |sprite| {
            (sprite.order, sprite.tile.texture_id.index)
        });
        sort_in_place(&mut self.polychrome_sprites, keys, |sprite| {
            (sprite.order, sprite.tile.texture_id.index)
        });
        self.surfaces.sort_by_key(|surface| surface.order);
        // 深度缓冲 anti-overdraw：排序后的下标即绘制序，按「越大越近」给每个
        // quad 分配单调深度，渲染器的不透明通道据此跳过被遮住像素。
        let depth_scale = quad_depth_scale(self.quads.len());
        for (index, quad) in self.quads.iter_mut().enumerate() {
            quad.depth = quad_depth(index as u32, depth_scale);
        }
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

/// 按 `(主键, 次键)` 就地排序图元，但**不搬图元本身**。
///
/// 图元结构体很大（`Quad` 100+ 字节），`sort_by_key` 的每次交换都要整体拷贝，
/// 而交换次数是 O(n log n)。这里改排小的 `(键, 下标)` 对（16 字节），排完再按
/// 置换的环把图元搬到位，每个图元只搬一次。`keys` 由调用方复用，跨图元种类
/// 只分配一次。
///
/// 稳定性：下标是排序键的低 32 位，因此相等键保持插入顺序，与原来的
/// `sort_by_key`（稳定排序）结果逐位一致；有了这条保证才能用 `sort_unstable`。
///
/// 已经有序的数组直接返回，不分配也不搬动（网格 / 条纹这类常见场景）。
fn sort_in_place<T: Copy>(
    primitives: &mut [T],
    keys: &mut Vec<(u64, u32)>,
    key: impl Fn(&T) -> (u32, u32),
) {
    if primitives.is_sorted_by_key(&key) {
        return;
    }
    keys.clear();
    keys.extend(primitives.iter().enumerate().map(|(index, primitive)| {
        let (major, minor) = key(primitive);
        ((major as u64) << 32 | minor as u64, index as u32)
    }));
    keys.sort_unstable();

    // `keys[target].1` 是 target 位置应放入的源下标。沿着置换的每个环搬一圈，
    // 搬过的槽位把下标指向自己作为「已处理」标记。
    for start in 0..primitives.len() {
        if keys[start].1 as usize == start {
            continue;
        }
        let displaced = primitives[start];
        let mut target = start;
        loop {
            let source = keys[target].1 as usize;
            keys[target].1 = target as u32;
            if source == start {
                primitives[target] = displaced;
                break;
            }
            primitives[target] = primitives[source];
            target = source;
        }
    }
}

/// 一帧含 `quad_count` 个 quad 时，相邻两个 quad 之间的深度步长。
///
/// 分母取（数量 + 1）向上取幂并封顶在 2^24（24 位深度缓冲的整数精度边界），
/// 保证深度在缓冲精度内单调可分。
pub fn quad_depth_scale(quad_count: usize) -> f32 {
    let denominator = (quad_count.min((1 << 24) - 1) + 1).next_power_of_two();
    1.0 / denominator as f32
}

/// 把 quad 在 [`Scene::quads`] 中的下标映射为深度，值越大越靠近观察者。
pub fn quad_depth(quad_id: u32, scale: f32) -> f32 {
    ((quad_id + 1) as f32 * scale).min(1.0)
}

/// 非 quad 图元插在「第 `quad_cursor` 个 quad 之前」时应使用的深度。
///
/// 取 `quad_cursor - 1` 与 `quad_cursor` 两个 quad 深度的中点：严格大于已经画过的
/// 前一个 quad、严格小于还没画的第 `quad_cursor` 个 quad，所以它既不会被前面的
/// 不透明矩形挡住，又会（正确地）被后面那个不透明矩形挡住。
///
/// 不能直接钉在 [`quad_depth`]`(quad_cursor)`：那是**下一个** quad 自己的深度，
/// 两者完全相等。深度比较在相等时不会把图元判为「被遮住」，于是弹窗这类不透明
/// 底板（在帧首的不透明通道里就已写下同一深度）反而压不住下层文字——下层文字会
/// 浮在弹窗之上。
pub fn quad_depth_between(quad_cursor: u32, scale: f32) -> f32 {
    ((quad_cursor as f32 + 0.5) * scale).min(1.0)
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

impl PrimitiveKind {
    /// 重叠图元的绘制顺序保证按绘制次序；但为了 GPU 合批，渲染器在同一绘制顺序内
    /// 先按种类、再按图集纹理排序，最后才按绘制次序。因此一个图元只有在它重叠的
    /// 最上层图元种类不超过这里返回的种类时，才能共享其绘制顺序；否则需要新的顺序。
    /// 精灵特例：同顺序内还要按图集纹理排序，不能与任何精灵共享顺序（只能与排在
    /// 自己之前的种类共享）。
    fn max_shared_kind(self) -> Self {
        match self {
            Self::MonochromeSprite => Self::Underline,
            Self::SubpixelSprite => Self::MonochromeSprite,
            Self::PolychromeSprite => Self::SubpixelSprite,
            kind => kind,
        }
    }
}

pub(crate) enum PaintOperation {
    Primitive(Primitive),
    StartLayer,
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
    Path(PaintedPath),
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

    /// 获取此图元的种类。
    pub(crate) fn kind(&self) -> PrimitiveKind {
        match self {
            Primitive::Shadow(_) => PrimitiveKind::Shadow,
            Primitive::Quad(_) => PrimitiveKind::Quad,
            Primitive::Path(_) => PrimitiveKind::Path,
            Primitive::Underline(_) => PrimitiveKind::Underline,
            Primitive::MonochromeSprite(_) => PrimitiveKind::MonochromeSprite,
            Primitive::SubpixelSprite(_) => PrimitiveKind::SubpixelSprite,
            Primitive::PolychromeSprite(_) => PrimitiveKind::PolychromeSprite,
            Primitive::Surface(_) => PrimitiveKind::Surface,
        }
    }

    /// 获取此图元的内容裁剪蒙版。
    pub fn content_mask(&self) -> &ContentMask<ScaledPixels> {
        match self {
            Primitive::Shadow(shadow) => &shadow.content_mask,
            Primitive::Quad(quad) => &quad.content_mask,
            Primitive::Path(path) => &path.paint.content_mask,
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
    paths_iter: Peekable<slice::Iter<'a, PaintedPath>>,
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
                // `batch_kind` 取自 orders_and_kinds 里 order 为 Some 的首项，
                // 排序后该种类的 peek 必然存在
                let texture_id = invariant(
                    self.monochrome_sprites_iter.peek(),
                    "场景批处理：monochrome 批次必有待处理精灵",
                )
                .tile
                .texture_id;
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
                // `batch_kind` 取自 orders_and_kinds 里 order 为 Some 的首项，
                // 排序后该种类的 peek 必然存在
                let texture_id = invariant(
                    self.subpixel_sprites_iter.peek(),
                    "场景批处理：subpixel 批次必有待处理精灵",
                )
                .tile
                .texture_id;
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
                // `batch_kind` 取自 orders_and_kinds 里 order 为 Some 的首项，
                // 排序后该种类的 peek 必然存在
                let texture_id = invariant(
                    self.polychrome_sprites_iter.peek(),
                    "场景批处理：polychrome 批次必有待处理精灵",
                )
                .tile
                .texture_id;
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
    /// 深度缓冲 anti-overdraw 用的深度值（「越大越近」，由 `Scene::finish`
    /// 按排序后的绘制序统一赋值；占据原 `pad_quad` 填充槽，布局大小不变）。
    pub depth: f32,
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
    /// 完整绘制边界；外阴影包含高斯扩散范围。
    pub bounds: Bounds<ScaledPixels>,
    /// SDF 使用的阴影形状边界，不含高斯扩散范围。
    pub shape_bounds: Bounds<ScaledPixels>,
    /// 圆角半径。
    pub corner_radii: Corners<ScaledPixels>,
    /// 内容裁剪蒙版。
    pub content_mask: ContentMask<ScaledPixels>,
    /// 颜色。
    pub color: Hsla,
    /// 元素边框盒；所有外阴影统一裁剪到此边界外侧（与 CSS box-shadow 一致）。
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

/// 指向所属场景动画表某一条目的下标；`NONE` 表示这个精灵不带动画。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct SpriteAnimationId(u32);

impl SpriteAnimationId {
    /// 无动画。
    pub const NONE: Self = Self(u32::MAX);
}

/// 由窗口在每次呈现前应用到精灵上的动画定义（场景级，不触发视图重绘）。
#[derive(Copy, Clone, Debug)]
pub(crate) struct SpriteAnimation {
    transformation: TimedTransformation,
    /// 变换矩阵的中心（元素中心，逻辑像素）。
    center: Point<Pixels>,
    /// 构建时的显示比例。
    scale_factor: f32,
    /// 动画 alpha 之前的底色。
    base_color: Hsla,
    /// 精灵在 `Scene::monochrome_sprites` 中的下标（`finish` 排序后修正）。
    sprite_index: usize,
}

impl SpriteAnimation {
    pub(crate) fn new(
        transformation: TimedTransformation,
        center: Point<Pixels>,
        scale_factor: f32,
    ) -> Self {
        Self {
            transformation,
            center,
            scale_factor,
            base_color: Hsla::default(),
            sprite_index: 0,
        }
    }
}

/// 单色精灵 — 从图集纹理中采样的灰度字形或图标。
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct MonochromeSprite {
    /// 绘制顺序。
    pub order: DrawOrder,
    /// 所属场景动画表的条目（`SpriteAnimationId::NONE` 表示不带动画）。
    pub animation: SpriteAnimationId,
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

/// 一条路径曲线：xy 各为二次多项式的单调二次贝塞尔（Slug 路径渲染的最小单元）。
///
/// 参数形式 `x(t) = (a.x * t + b.x) * t + p0.x`、`y(t) = (a.y * t + b.y) * t + p0.y`，
/// 且 y 随 t 单调（t: 0→1 时 y 从 `p0.y` 走到 `p1.y`）；`sign` 记录 y 的走向，
/// 供环绕数步进使用。直线是二次项为零的特例。
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct PathCurve {
    /// 起点坐标。
    pub p0: Point<f32>,
    /// 终点坐标。
    pub p1: Point<f32>,
    /// 二次项系数。
    pub a: Point<f32>,
    /// 一次项系数。
    pub b: Point<f32>,
    /// y 走向符号（+1 向下 / -1 向上）。
    pub sign: f32,
    /// 8 字节对齐填充。
    pub pad: u32, // align to 8 bytes
}

/// 路径的绘制参数（GPU 传输结构，随曲线/瓦片数组一起上传）。
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct PathPaint {
    /// 路径边界（设备像素）。
    pub bounds: Bounds<ScaledPixels>,
    /// 内容裁剪蒙版。
    pub content_mask: ContentMask<ScaledPixels>,
    /// 填充背景。
    pub color: Background,
    /// 填充规则：非 0 为奇偶（even-odd），0 为非零环绕（non-zero）。
    pub even_odd: u32,
    /// 8 字节对齐填充。
    pub pad: u32, // align to 8 bytes
}

/// 路径瓦片：一组合并的 cell，GPU 实例按其曲线列表逐像素计算环绕数与覆盖。
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct PathTile {
    /// 本帧 `PathPaint` 数组的下标。
    pub paint: u32,
    /// 本帧 `PathCurve` 数组中的起始下标。
    pub curve_start: u32,
    /// 本瓦片列出的曲线数量。
    pub curve_count: u32,
    /// 未列入本瓦片的曲线沿参考角向左射线的环绕数贡献。
    pub backdrop: i32,
    /// 瓦片矩形左上角（设备像素）。
    pub top_left: Point<f32>,
    /// 瓦片矩形右下角（设备像素）。
    pub bottom_right: Point<f32>,
    /// backdrop 的参考点（所在 cell 的左上角）。
    pub reference: Point<f32>,
    /// 列表曲线包围盒左上角（供 shader 早退与越界判定）。
    pub curves_top_left: Point<f32>,
    /// 列表曲线包围盒右下角。
    pub curves_bottom_right: Point<f32>,
}

/// 一条待绘制的填充路径：分桶瓦片 + 绘制参数。
///
/// 由 [`crate::Window::paint_path`] 构建（分桶依赖显示比例，发生在绘制时）；
/// `Rc<PathTiles>` 让缓存复用的 `Path` 克隆成本与曲线数量无关。
#[derive(Clone, Debug)]
pub struct PaintedPath {
    /// 绘制顺序（场景分配，渲染侧只读）。
    order: DrawOrder,
    /// 涂刷边界（瓦片边界外扩抗锯齿边距，设备像素）。
    bounds: Bounds<ScaledPixels>,
    /// 分桶结果。
    tiles: Rc<PathTiles>,
    /// 绘制参数。
    paint: PathPaint,
}

impl PaintedPath {
    /// 把 `path` 按显示比例分桶成瓦片，在 `content_mask` 内以 `color` 填充。
    pub fn new(
        path: Path,
        scale_factor: f32,
        content_mask: ContentMask<Pixels>,
        color: Background,
    ) -> Self {
        let content_mask = content_mask.scale(scale_factor);
        let mut tiles = path.bin(scale_factor);
        tiles.retain_visible(content_mask.bounds);
        let paint = PathPaint {
            bounds: tiles.bounds(),
            content_mask,
            color,
            even_odd: path.even_odd().into(),
            pad: 0,
        };
        Self {
            order: 0,
            bounds: tiles.painted_bounds(),
            tiles: Rc::new(tiles),
            paint,
        }
    }

    /// 本路径的瓦片数量。
    pub fn tile_count(&self) -> usize {
        self.tiles.tiles().len()
    }

    /// 绘制顺序（由场景在插入时分配）。
    pub fn order(&self) -> DrawOrder {
        self.order
    }

    /// 涂刷边界（设备像素）。
    pub fn bounds(&self) -> Bounds<ScaledPixels> {
        self.bounds
    }

    /// 内容裁剪蒙版（设备像素）。
    pub fn content_mask(&self) -> &ContentMask<ScaledPixels> {
        &self.paint.content_mask
    }

    /// 把绘制参数、曲线与瓦片平铺进本帧的三条 GPU 数组。
    ///
    /// 渲染器按批调用（批内路径依次追加，`paint_index` / `curve_start` 随之为批内下标）。
    pub fn append_to(
        &self,
        paints: &mut Vec<PathPaint>,
        curves: &mut Vec<PathCurve>,
        tiles: &mut Vec<PathTile>,
    ) {
        let paint_index = paints.len() as u32;
        let curve_start = curves.len() as u32;
        paints.push(self.paint);
        curves.extend_from_slice(self.tiles.curves());
        tiles.extend(self.tiles.tiles().iter().map(|tile| PathTile {
            paint: paint_index,
            curve_start: curve_start + tile.curve_start,
            ..*tile
        }));
    }
}

impl From<PaintedPath> for Primitive {
    fn from(path: PaintedPath) -> Self {
        Primitive::Path(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AtlasTextureKind, DevicePixels, TileId, bounds, point, size};

    /// 构造一个只关心 order 与所属纹理/瓦片 ID 的单色精灵。
    fn sprite(order: DrawOrder, texture_index: u32, tile_id: u32) -> MonochromeSprite {
        MonochromeSprite {
            order,
            animation: SpriteAnimationId::NONE,
            bounds: bounds(
                point(ScaledPixels(0.0), ScaledPixels(0.0)),
                size(ScaledPixels(8.0), ScaledPixels(8.0)),
            ),
            content_mask: ContentMask {
                bounds: bounds(
                    point(ScaledPixels(0.0), ScaledPixels(0.0)),
                    size(ScaledPixels(100.0), ScaledPixels(100.0)),
                ),
            },
            color: Hsla::default(),
            tile: AtlasTile {
                texture_id: AtlasTextureId {
                    index: texture_index,
                    kind: AtlasTextureKind::Monochrome,
                },
                tile_id: TileId(tile_id),
                padding: 0,
                bounds: Bounds {
                    origin: Point::default(),
                    size: size(DevicePixels(8), DevicePixels(8)),
                },
            },
            transformation: TransformationMatrix::default(),
        }
    }

    /// 同纹理精灵必须聚成一个批次：排序键按 texture_id 而非 tile_id。
    #[test]
    fn test_sprite_sort_keeps_texture_batches_contiguous() {
        let mut scene = Scene::default();
        // 两张纹理的瓦片 ID 交替递增，按 tile_id 排序会把批次打散成 5 段。
        scene.monochrome_sprites = vec![
            sprite(0, 1, 10),
            sprite(0, 0, 99),
            sprite(0, 1, 11),
            sprite(0, 0, 98),
            sprite(0, 1, 12),
        ];
        scene.finish();

        let texture_order = scene
            .monochrome_sprites
            .iter()
            .map(|sprite| sprite.tile.texture_id.index)
            .collect::<Vec<_>>();
        assert_eq!(texture_order, vec![0, 0, 1, 1, 1]);

        let batches = scene.batches().collect::<Vec<_>>();
        assert_eq!(batches.len(), 2, "同一 order 下应按纹理分成 2 个批次");
        match (&batches[0], &batches[1]) {
            (
                PrimitiveBatch::MonochromeSprites {
                    texture_id: first,
                    range: first_range,
                },
                PrimitiveBatch::MonochromeSprites {
                    texture_id: second,
                    range: second_range,
                },
            ) => {
                assert_eq!(first.index, 0);
                assert_eq!(*first_range, 0..2);
                assert_eq!(second.index, 1);
                assert_eq!(*second_range, 2..5);
            }
            other => panic!("期望两个单色精灵批次，实际为 {other:?}"),
        }
    }

    /// 构造指定矩形的普通四边形（内容蒙版全覆盖，裁剪后即自身边界）。
    fn quad_at(x: f32, y: f32, w: f32, h: f32) -> Quad {
        Quad {
            order: 0,
            border_style: BorderStyle::default(),
            bounds: bounds(
                point(ScaledPixels(x), ScaledPixels(y)),
                size(ScaledPixels(w), ScaledPixels(h)),
            ),
            content_mask: ContentMask {
                bounds: bounds(
                    point(ScaledPixels(0.0), ScaledPixels(0.0)),
                    size(ScaledPixels(1000.0), ScaledPixels(1000.0)),
                ),
            },
            background: Background::default(),
            border_color: Hsla::default(),
            corner_radii: Corners::default(),
            border_widths: Edges::default(),
            continuous_corners: 0,
            transform: TransformationMatrix::default(),
            blend_mode: 0,
            depth: 0.0,
        }
    }

    /// 种类安全的重叠可共享绘制顺序：一串依次重叠的四边形只占一个顺序，
    /// 合批成单个批次（旧口径每个重叠都开新顺序，批次数翻倍）。
    #[test]
    fn test_overlapping_quads_share_order_and_batch() {
        let mut scene = Scene::default();
        scene.insert_primitive(quad_at(0., 0., 10., 10.));
        scene.insert_primitive(quad_at(5., 5., 10., 10.));
        scene.insert_primitive(quad_at(10., 10., 10., 10.));
        scene.finish();

        assert_eq!(
            scene.quads.iter().map(|q| q.order).collect::<Vec<_>>(),
            vec![1, 1, 1],
            "Quad 与 Quad 种类安全，应共享顺序"
        );
        assert_eq!(scene.batches().count(), 1, "同一顺序同一种类应合成一个批次");
    }

    /// 深度缓冲 anti-overdraw：`finish` 排序后按绘制序给每个 quad 赋单调深度。
    #[test]
    fn test_quad_depths_assigned_monotonic() {
        let mut scene = Scene::default();
        scene.insert_primitive(quad_at(0., 0., 10., 10.));
        scene.insert_primitive(quad_at(5., 5., 10., 10.));
        scene.insert_primitive(quad_at(2., 2., 4., 4.));
        scene.finish();

        let scale = quad_depth_scale(scene.quads.len());
        let expected: Vec<f32> = (0..scene.quads.len() as u32)
            .map(|index| quad_depth(index, scale))
            .collect();
        let actual: Vec<f32> = scene.quads.iter().map(|quad| quad.depth).collect();
        assert_eq!(actual, expected, "深度必须按下标经统一 scale 映射");
        for window in actual.windows(2) {
            assert!(window[1] > window[0], "深度必须沿绘制序单调递增");
        }
        assert!(
            actual[actual.len() - 1] <= 1.0 && actual[0] > 0.0,
            "深度必须落在 (0, 1]"
        );
    }

    /// 深度步长：分母为 (数量 + 1) 的下一个幂，并在 2^24 处封顶
    /// （24 位深度缓冲的整数精度边界）。
    #[test]
    fn test_quad_depth_scale_power_of_two_and_cap() {
        assert_eq!(quad_depth_scale(0), 1.0);
        assert_eq!(quad_depth_scale(3), 0.25);
        assert_eq!(quad_depth_scale(4), 0.125);
        let capped = quad_depth_scale(1 << 30);
        assert_eq!(capped, 1.0 / (1 << 24) as f32);
        assert_eq!(quad_depth(1 << 24, capped), 1.0, "深度必须封顶在 1");
    }

    /// 精灵不能与精灵共享绘制顺序（同顺序内按图集纹理排序会破坏绘制次序），
    /// 但可以与排在它之前的种类（如 Quad）共享。
    #[test]
    fn test_sprites_cannot_share_order_with_sprites() {
        let mut scene = Scene::default();
        // 先一个四边形，再两个互相重叠的单色精灵（边界相同）。
        scene.insert_primitive(quad_at(0., 0., 10., 10.));
        scene.insert_primitive(sprite(0, 0, 1));
        scene.insert_primitive(sprite(0, 1, 2));
        scene.finish();

        // 精灵与 Quad 种类安全：共享顺序 1。
        assert_eq!(scene.quads[0].order, 1);
        assert_eq!(scene.monochrome_sprites[0].order, 1);
        // 精灵与精灵不共享：第二个精灵开新顺序 2。
        assert_eq!(scene.monochrome_sprites[1].order, 2);
    }

    /// 图层关闭后才统一落地：层内图元共用一个顺序，整层可与他方共享；
    /// 空层不占顺序。
    #[test]
    fn test_layers_insert_deferred_and_share_order() {
        let mut scene = Scene::default();
        scene.insert_primitive(quad_at(0., 0., 10., 10.));
        scene.push_layer();
        scene.insert_primitive(quad_at(5., 5., 10., 10.));
        scene.insert_primitive(quad_at(6., 6., 10., 10.));
        scene.pop_layer();
        // 空图层不插入排序树，也不产生图元。
        scene.push_layer();
        scene.pop_layer();
        scene.finish();

        assert_eq!(
            scene.quads.iter().map(|q| q.order).collect::<Vec<_>>(),
            vec![1, 1, 1],
            "图层的并集边界与第一个 Quad 相交且种类安全，层内图元应共享顺序 1"
        );
        assert_eq!(scene.batches().count(), 1);
    }

    /// 图层边界必须从首个图元的真实边界开始，不能包含窗口原点。
    ///
    /// 回归：`Bounds::default()` 的原点是 (0,0)，拿它做并集起点会让每个图层都
    /// 变成从左上角开始的矩形，文本行图层因此互相相交、顺序被抬高，最终盖住弹窗。
    #[test]
    fn test_layer_bounds_start_at_first_primitive() {
        let mut scene = Scene::default();
        scene.insert_primitive(quad_at(0., 0., 10., 10.));
        scene.push_layer();
        scene.insert_primitive(quad_at(100., 100., 10., 10.));
        scene.pop_layer();
        scene.finish();

        assert_eq!(
            scene.quads.iter().map(|q| q.order).collect::<Vec<_>>(),
            vec![1, 1],
            "与首个 Quad 不相交的图层不应因默认原点 (0,0) 而抬高顺序"
        );
    }

    /// 嵌套图层在最外层关闭时统一落地；图元在图层内不再即时查询排序树。
    #[test]
    fn test_nested_layers_flush_on_outermost_pop() {
        let mut scene = Scene::default();
        scene.push_layer();
        scene.insert_primitive(quad_at(0., 0., 10., 10.));
        scene.push_layer();
        scene.insert_primitive(quad_at(40., 40., 10., 10.));
        scene.pop_layer();
        // 内层关闭后尚未落地：图元此时还没进入种类列表。
        assert!(scene.quads.is_empty(), "最外层未关闭前不应落地");
        scene.pop_layer();
        scene.finish();

        // 外层只含第一个 Quad，内层只含第二个；两块边界不相交 → 共享顺序 1。
        assert_eq!(
            scene.quads.iter().map(|q| q.order).collect::<Vec<_>>(),
            vec![1, 1]
        );
    }

    /// 嵌套图层：子层的图元必须叠在外层「f 后续绘制」的图元之上
    /// （`Window::paint_layer` 文档承诺）。三者边界两两重叠以强制 order 竞争。
    #[test]
    fn test_nested_layer_child_sits_above_outer_later_sibling() {
        let mut scene = Scene::default();
        scene.push_layer(); // 外层 f
        scene.insert_primitive(quad_at(0., 0., 10., 10.)); // A：f 内、子层之前
        scene.push_layer(); // 子层 g
        scene.insert_primitive(quad_at(2., 2., 10., 10.)); // B：子层内，与 A/C 重叠
        scene.pop_layer(); // g 关闭
        scene.insert_primitive(quad_at(4., 4., 10., 10.)); // C：f 内、子层之后，与 B 重叠
        scene.pop_layer(); // 外层关闭
        scene.finish();

        // 找回每个身份（靠 origin.x）的绘制序（quads 已按绘制序排好）。
        let x_to_draw_index = |x: f32| {
            scene
                .quads
                .iter()
                .position(|q| (q.bounds.origin.x.0 - x).abs() < 0.01)
                .expect("图元应存在")
        };
        let ia = x_to_draw_index(0.);
        let ib = x_to_draw_index(2.);
        let ic = x_to_draw_index(4.);
        assert!(
            ia < ic && ic < ib,
            "绘制序应为 A < C < B（子层 B 叠在 f 后续的 C 之上），实际 A={ia}, C={ic}, B={ib}"
        );
    }

    /// 构造带指定 order、以 `x` 作为身份标记的四边形（排序后靠 `bounds.origin.x` 认人）。
    fn quad_with_order(order: u32, x: f32) -> Quad {
        let mut quad = quad_at(x, 0., 10., 10.);
        quad.order = order;
        quad
    }

    fn quad_identity(quads: &[Quad]) -> Vec<(u32, f32)> {
        quads
            .iter()
            .map(|quad| (quad.order, quad.bounds.origin.x.0))
            .collect()
    }

    /// 就地置换排序必须与稳定 `sort_by_key` 逐位一致（含相等 order 的先后）。
    #[test]
    fn test_finish_sort_matches_stable_sort() {
        // 刻意造出：逆序、重复 order、已有序、单元素等形态。
        let cases: Vec<Vec<(u32, f32)>> = vec![
            vec![(3, 30.), (1, 10.), (2, 20.)],
            vec![(1, 10.), (1, 11.), (0, 0.), (1, 12.)],
            vec![(0, 0.), (1, 10.), (2, 20.)],
            vec![(7, 70.)],
            vec![],
            vec![(2, 20.), (2, 21.), (1, 10.), (2, 22.), (1, 11.), (0, 0.)],
        ];

        for case in cases {
            let mut scene = Scene::default();
            scene.quads = case
                .iter()
                .map(|&(order, x)| quad_with_order(order, x))
                .collect();
            let mut expected = scene.quads.clone();
            expected.sort_by_key(|quad| quad.order);

            scene.finish();

            assert_eq!(
                quad_identity(&scene.quads),
                quad_identity(&expected),
                "输入 {case:?} 的排序结果必须与稳定排序一致"
            );
        }
    }

    /// 重复排序（finish 连调）不得改变结果，也不得破坏深度赋值。
    #[test]
    fn test_finish_is_idempotent() {
        let mut scene = Scene::default();
        scene.quads = vec![
            quad_with_order(2, 20.),
            quad_with_order(0, 0.),
            quad_with_order(1, 10.),
            quad_with_order(1, 11.),
        ];
        scene.finish();
        let once = quad_identity(&scene.quads);
        let depths = scene
            .quads
            .iter()
            .map(|quad| quad.depth)
            .collect::<Vec<_>>();

        scene.finish();

        assert_eq!(quad_identity(&scene.quads), once);
        assert_eq!(
            scene
                .quads
                .iter()
                .map(|quad| quad.depth)
                .collect::<Vec<_>>(),
            depths,
            "第二次 finish 必须给出同样的深度"
        );
    }

    /// 置换可能包含多个环：直接验证 `sort_in_place` 的搬移逻辑。
    #[test]
    fn test_sort_in_place_follows_multiple_cycles() {
        // 键 = 值本身；输入构成 (0 1 2)(3 4) 两个环。
        let mut values = vec![1u32, 2, 0, 4, 3];
        let mut keys = Vec::new();
        sort_in_place(&mut values, &mut keys, |value| (*value, 0));
        assert_eq!(values, vec![0, 1, 2, 3, 4]);

        // 已经有序时不改动、也不清空复用缓冲之外的语义（幂等）。
        let mut values = vec![0u32, 1, 2];
        sort_in_place(&mut values, &mut keys, |value| (*value, 0));
        assert_eq!(values, vec![0, 1, 2]);
    }

    /// 精灵键是 `(order, texture_id)`，稳定性同样要保住（决定批次内绘制顺序）。
    #[test]
    fn test_sprite_sort_is_stable_across_texture_groups() {
        let mut scene = Scene::default();
        scene.monochrome_sprites = vec![
            sprite(2, 1, 21),
            sprite(1, 1, 11),
            sprite(2, 0, 20),
            sprite(1, 0, 10),
            sprite(2, 1, 22),
        ];
        scene.finish();

        let actual = scene
            .monochrome_sprites
            .iter()
            .map(|sprite| {
                (
                    sprite.order,
                    sprite.tile.texture_id.index,
                    sprite.tile.tile_id.0,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            vec![(1, 0, 10), (1, 1, 11), (2, 0, 20), (2, 1, 21), (2, 1, 22)],
            "同键精灵必须保持插入顺序"
        );
    }
}

#[cfg(test)]
mod animation_tests {
    use super::*;
    use crate::{Hsla, TimedTransformation, Transformation, hsla, point, px, size};

    fn animated_sprite(color: Hsla) -> MonochromeSprite {
        MonochromeSprite {
            order: 0,
            animation: SpriteAnimationId::NONE,
            bounds: Bounds {
                origin: point(ScaledPixels(0.0), ScaledPixels(0.0)),
                size: size(ScaledPixels(8.0), ScaledPixels(8.0)),
            },
            content_mask: ContentMask {
                bounds: Bounds {
                    origin: point(ScaledPixels(0.0), ScaledPixels(0.0)),
                    size: size(ScaledPixels(100.0), ScaledPixels(100.0)),
                },
            },
            color,
            tile: AtlasTile {
                texture_id: crate::AtlasTextureId {
                    index: 0,
                    kind: crate::AtlasTextureKind::Monochrome,
                },
                tile_id: crate::TileId(1),
                padding: 0,
                bounds: Bounds {
                    origin: point(crate::DevicePixels(0), crate::DevicePixels(0)),
                    size: size(crate::DevicePixels(8), crate::DevicePixels(8)),
                },
            },
            transformation: TransformationMatrix::default(),
        }
    }

    /// 场景级动画：插入后 sprite 带动画 id；advance 只改矩阵与 alpha；
    /// finish 排序后动画仍指向自己的精灵。
    #[test]
    fn advance_animations_moves_sprite_without_rebuild() {
        use std::time::Duration;

        let mut scene = Scene::default();
        let mut sprite = animated_sprite(Hsla::default());
        // 静态底色（动画 alpha 之前）。
        sprite.color = crate::hsla(0.0, 0.0, 0.0, 1.0);
        let animation = TimedTransformation::rotation(Duration::from_secs(1), Default::default());
        scene.insert_animated_sprite(
            sprite,
            SpriteAnimation::new(animation, point(px(4.), px(4.)), 1.0),
        );
        assert_eq!(scene.monochrome_sprites.len(), 1);
        assert_ne!(
            scene.monochrome_sprites[0].animation,
            SpriteAnimationId::NONE
        );

        // 推进 1/4 个周期 → 旋转 90°（矩阵变化），alpha 仍 1。
        scene.finish();
        scene.advance_animations(Duration::from_millis(250));
        let sprite = &scene.monochrome_sprites[0];
        assert!(
            sprite.transformation.rotation_scale[0][1].abs() > 0.5,
            "1/4 周期应转 90°"
        );
        assert!(sprite.color.a >= 0.99);
        // 没有动画的精灵 id 保持 NONE。
        let plain = animated_sprite(Hsla::default());
        assert_eq!(plain.animation, SpriteAnimationId::NONE);
    }

    /// 动画 alpha 合成进精灵颜色（底色 × alpha）。
    #[test]
    fn advance_animations_applies_alpha() {
        use std::time::Duration;

        let mut scene = Scene::default();
        let animation = TimedTransformation::new(Duration::from_secs(2))
            .from(Default::default())
            .to(Transformation::alpha(0.0));
        let mut sprite = animated_sprite(hsla(0.0, 0.0, 0.0, 1.0));
        sprite.color = hsla(0.0, 0.0, 0.0, 1.0);
        scene.insert_animated_sprite(
            sprite,
            SpriteAnimation::new(animation, point(px(4.), px(4.)), 1.0),
        );
        scene.finish();

        // 半个周期（1s）→ alpha = 0.5。
        scene.advance_animations(Duration::from_secs(1));
        assert!(scene.monochrome_sprites[0].color.a < 0.6, "alpha 应 ≈ 0.5");
    }

    /// 动画 id 越界（图元被裁掉）时 advance 安全跳过。
    #[test]
    fn advance_animations_skips_missing_sprite() {
        use std::time::Duration;

        let mut scene = Scene::default();
        let animation = TimedTransformation::rotation(Duration::from_secs(1), Default::default());
        // 直接塞一条指向不存在精灵的动画（模拟插入失败）。
        scene.animations.push(SpriteAnimation {
            transformation: animation,
            center: point(px(0.), px(0.)),
            scale_factor: 1.0,
            base_color: Hsla::default(),
            sprite_index: 99,
        });
        scene.advance_animations(Duration::from_millis(100)); // 不 panic
    }

    /// 无动画时 animation_frame_interval 为 None；with_max_fps 生效。
    #[test]
    fn animation_frame_interval_reflects_scene() {
        use std::time::Duration;

        let mut scene = Scene::default();
        assert!(scene.animation_frame_interval().is_none());
        let animation = TimedTransformation::new(Duration::from_secs(1)).with_max_fps(15.0);
        let sprite = animated_sprite(Hsla::default());
        scene.insert_animated_sprite(
            sprite,
            SpriteAnimation::new(animation, point(px(0.), px(0.)), 1.0),
        );
        let interval = scene.animation_frame_interval().expect("应有间隔");
        assert!((interval.as_secs_f32() - 1.0 / 15.0).abs() < 1e-4);
    }
}
