//! 布局引擎：基于 taffy 的 Flexbox / Grid 布局计算。

use crate::collections::{FxHashMap, FxHashSet};
use crate::{
    AbsoluteLength, App, Bounds, DefiniteLength, Edges, GridTemplate, Length, Pixels, Point, Size,
    Style, Window, size,
    util::{
        ceil_to_device_pixel, round_half_toward_zero, round_stroke_to_device_pixel,
        round_to_device_pixel,
    },
};
use stacksafe::{StackSafe, stacksafe};
use std::{fmt::Debug, ops::Range};
use taffy::{
    TaffyTree,
    geometry::{Point as TaffyPoint, Rect as TaffyRect, Size as TaffySize},
    prelude::{max_content, min_content},
    style::AvailableSpace as TaffyAvailableSpace,
    tree::NodeId,
};

type NodeMeasureFn = StackSafe<
    Box<
        dyn FnMut(
            Size<Option<Pixels>>,
            Size<AvailableSpace>,
            &mut Window,
            &mut App,
        ) -> Size<Pixels>,
    >,
>;

struct NodeContext {
    measure: NodeMeasureFn,
}

/// 跨帧保留的布局节点记录（P3a）。
///
/// 节点按 [`LayoutKey`](crate::fast::layout_key::LayoutKey) 复用：
/// 命中后仍全量比对样式与子节点（正确性不依赖键）；
/// 测量节点另比对测量指纹（文本 carry）。
struct RetainedNode {
    node: NodeId,
    children: Vec<NodeId>,
    /// 测量指纹（`request_measured_layout` 调用方提供；`None` 永不 carry）。
    measure_fingerprint: Option<u64>,
    /// 是否带测量闭包（种类切换时重写上下文）。
    has_measure: bool,
    /// 上次测量的文本状态（carry 时交还调用方，使新元素直接持有旧测量）。
    measured_state: Option<crate::TextLayout>,
}

/// 一帧布局保留统计（帧末汇总入 `FrameStats`，量化口径）。
#[derive(Default)]
pub(crate) struct LayoutFrameStats {
    /// 无写入复用（样式／子节点／测量全命中，Taffy 缓存保留）。
    pub(crate) reused_clean: u64,
    /// 命中但改写（样式或子节点或测量变化，仅省分配）。
    pub(crate) rewritten: u64,
    /// 新分配（含同键二次使用的临时节点）。
    pub(crate) allocated: u64,
}

pub struct TaffyLayoutEngine {
    taffy: TaffyTree<NodeContext>,
    absolute_layout_bounds: FxHashMap<LayoutId, Bounds<Pixels>>,
    /// 每个节点未舍入的绝对边框框左上角设备像素坐标。
    absolute_outer_origins: FxHashMap<LayoutId, Point<f32>>,
    computed_layouts: FxHashSet<LayoutId>,
    layout_bounds_scratch_space: Vec<LayoutId>,
    /// 路径键 → 保留节点（跨帧常驻；`end_frame` 释放无人认领者）。
    retained: FxHashMap<crate::fast::layout_key::LayoutKey, RetainedNode>,
    /// 本帧已认领的键（同键二次请求走临时节点）。
    claimed: FxHashSet<crate::fast::layout_key::LayoutKey>,
    /// 本帧已认领的节点（sweep 存活依据）。
    claimed_nodes: FxHashSet<NodeId>,
    /// 本帧临时节点（同键二次使用；帧末释放）。
    transient: Vec<NodeId>,
    /// 帧是否已开（首个请求时开；跳过绘制的帧不开也不释放）。
    frame_open: bool,
    /// 各计算根上次的可用空间（空间变化而样式不变时仍须致脏，否则读缓存旧尺寸）。
    last_spaces: FxHashMap<LayoutId, Size<AvailableSpace>>,
    layout_nodes_reused: u64,
    layout_nodes_rewritten: u64,
    layout_nodes_allocated: u64,
}

const EXPECT_MESSAGE: &str = "we should avoid taffy layout errors by construction if possible";

impl TaffyLayoutEngine {
    pub fn new() -> Self {
        let mut taffy = TaffyTree::new();
        taffy.disable_rounding();
        TaffyLayoutEngine {
            taffy,
            absolute_layout_bounds: FxHashMap::default(),
            absolute_outer_origins: FxHashMap::default(),
            computed_layouts: FxHashSet::default(),
            layout_bounds_scratch_space: Vec::new(),
            retained: FxHashMap::default(),
            claimed: FxHashSet::default(),
            claimed_nodes: FxHashSet::default(),
            transient: Vec::new(),
            frame_open: false,
            last_spaces: FxHashMap::default(),
            layout_nodes_reused: 0,
            layout_nodes_rewritten: 0,
            layout_nodes_allocated: 0,
        }
    }

    /// 开始一帧的布局请求收集（首个请求时惰性调用亦可）。
    ///
    /// 只重置认领集、计算标记与备注缓存；Taffy 树与保留节点跨帧常驻。
    /// `LayoutId` 跨帧稳定，故备注缓存必须每帧清空，否则读到上帧边界。
    pub fn begin_frame(&mut self) {
        self.claimed.clear();
        self.claimed_nodes.clear();
        self.computed_layouts.clear();
        self.absolute_layout_bounds.clear();
        self.absolute_outer_origins.clear();
        self.layout_nodes_reused = 0;
        self.layout_nodes_rewritten = 0;
        self.layout_nodes_allocated = 0;
        self.frame_open = true;
    }

    /// 结束一帧：释放无人认领的保留节点与临时节点，返回本帧统计。
    ///
    /// 未开帧时直接返回零统计（跳过绘制的帧不释放任何节点）。
    /// 保留关闭时恢复旧行为（整树清空，零逐节点开销；基准诚实）。
    pub fn end_frame(&mut self, retain: bool) -> LayoutFrameStats {
        if !self.frame_open {
            return LayoutFrameStats::default();
        }
        if !retain {
            self.taffy.clear();
            self.retained.clear();
            self.transient.clear();
            self.claimed.clear();
            self.claimed_nodes.clear();
            self.last_spaces.clear();
            self.frame_open = false;
            return LayoutFrameStats {
                reused_clean: 0,
                rewritten: 0,
                allocated: self.layout_nodes_allocated,
            };
        }
        let mut removed = Vec::new();
        self.retained.retain(|_, retained| {
            if self.claimed_nodes.contains(&retained.node) {
                return true;
            }
            removed.push(retained.node);
            false
        });
        for node in removed {
            self.taffy.remove(node).expect(EXPECT_MESSAGE);
        }
        for node in self.transient.drain(..) {
            self.taffy.remove(node).expect(EXPECT_MESSAGE);
        }
        self.claimed.clear();
        self.claimed_nodes.clear();
        self.frame_open = false;
        LayoutFrameStats {
            reused_clean: self.layout_nodes_reused,
            rewritten: self.layout_nodes_rewritten,
            allocated: self.layout_nodes_allocated,
        }
    }

    /// 确保帧已开（请求入口调用）。
    fn ensure_frame_open(&mut self) {
        if !self.frame_open {
            self.begin_frame();
        }
    }

    pub fn request_layout(
        &mut self,
        style: Style,
        rem_size: Pixels,
        scale_factor: f32,
        children: &[LayoutId],
        key: crate::fast::layout_key::LayoutKey,
        retain: bool,
    ) -> LayoutId {
        self.ensure_frame_open();
        let taffy_style = style.to_taffy(rem_size, scale_factor);

        // 保留关闭：旧行为（每帧新分配，帧末整树清空；不跟踪临时节点）。
        // 注意 `children_nodes` 在此之后才构造：关闭路径沿用零分配切片。
        if !retain {
            let node = if children.is_empty() {
                self.taffy.new_leaf(taffy_style)
            } else {
                self.taffy.new_with_children(
                    taffy_style,
                    // This is safe because LayoutId is repr(transparent) to taffy::tree::NodeId.
                    LayoutId::to_taffy_slice(children),
                )
            }
            .expect(EXPECT_MESSAGE);
            self.layout_nodes_allocated += 1;
            return LayoutId::from(node);
        }

        let children_nodes: Vec<NodeId> = children.iter().map(|id| NodeId::from(*id)).collect();

        // 同键本帧第二次使用：分配临时节点（帧末释放；内容正确、仅无保留）。
        if !self.claimed.insert(key) {
            let node = if children_nodes.is_empty() {
                self.taffy.new_leaf(taffy_style)
            } else {
                self.taffy.new_with_children(
                    taffy_style,
                    // This is safe because LayoutId is repr(transparent) to taffy::tree::NodeId.
                    LayoutId::to_taffy_slice(children),
                )
            }
            .expect(EXPECT_MESSAGE);
            self.transient.push(node);
            self.layout_nodes_allocated += 1;
            return LayoutId::from(node);
        }

        if let Some(retained) = self.retained.get_mut(&key) {
            let node = retained.node;
            self.claimed_nodes.insert(node);
            let mut clean = true;
            // 样式比对：不写即不脏，Taffy 布局缓存保留（核心收益）。
            if self.taffy.style(node).expect(EXPECT_MESSAGE) != &taffy_style {
                self.taffy
                    .set_style(node, taffy_style)
                    .expect(EXPECT_MESSAGE);
                clean = false;
            }
            if retained.children != children_nodes {
                self.taffy
                    .set_children(node, &children_nodes)
                    .expect(EXPECT_MESSAGE);
                retained.children = children_nodes;
                clean = false;
            }
            // 之前是测量节点、现在不是：清除上下文（致脏，正确）。
            if retained.has_measure {
                self.taffy
                    .set_node_context(node, None)
                    .expect(EXPECT_MESSAGE);
                retained.has_measure = false;
                retained.measure_fingerprint = None;
                clean = false;
            }
            if clean {
                self.layout_nodes_reused += 1;
            } else {
                self.layout_nodes_rewritten += 1;
            }
            return LayoutId::from(node);
        }

        let node = if children_nodes.is_empty() {
            self.taffy.new_leaf(taffy_style)
        } else {
            self.taffy.new_with_children(
                taffy_style,
                // This is safe because LayoutId is repr(transparent) to taffy::tree::NodeId.
                LayoutId::to_taffy_slice(children),
            )
        }
        .expect(EXPECT_MESSAGE);
        self.claimed_nodes.insert(node);
        self.retained.insert(
            key,
            RetainedNode {
                node,
                children: children_nodes,
                measure_fingerprint: None,
                has_measure: false,
                measured_state: None,
            },
        );
        self.layout_nodes_allocated += 1;
        LayoutId::from(node)
    }

    pub fn request_measured_layout(
        &mut self,
        style: Style,
        rem_size: Pixels,
        scale_factor: f32,
        fingerprint: Option<u64>,
        text_state: Option<crate::TextLayout>,
        measure: impl FnMut(
            Size<Option<Pixels>>,
            Size<AvailableSpace>,
            &mut Window,
            &mut App,
        ) -> Size<Pixels>
        + 'static,
        key: crate::fast::layout_key::LayoutKey,
        retain: bool,
    ) -> (LayoutId, Option<crate::TextLayout>) {
        self.ensure_frame_open();
        let taffy_style = style.to_taffy(rem_size, scale_factor);

        // 保留关闭：旧行为（每帧新分配，帧末整树清空；不跟踪临时节点）。
        if !retain {
            let node = self
                .taffy
                .new_leaf_with_context(
                    taffy_style,
                    NodeContext {
                        measure: StackSafe::new(Box::new(measure)),
                    },
                )
                .expect(EXPECT_MESSAGE);
            self.layout_nodes_allocated += 1;
            return (LayoutId::from(node), None);
        }

        // 同键本帧第二次使用：分配临时节点（帧末释放）。
        if !self.claimed.insert(key) {
            let node = self
                .taffy
                .new_leaf_with_context(
                    taffy_style,
                    NodeContext {
                        measure: StackSafe::new(Box::new(measure)),
                    },
                )
                .expect(EXPECT_MESSAGE);
            self.transient.push(node);
            self.layout_nodes_allocated += 1;
            return (LayoutId::from(node), None);
        }

        if let Some(retained) = self.retained.get_mut(&key) {
            let node = retained.node;
            self.claimed_nodes.insert(node);
            // 文本 carry：指纹俱在且相等 → 不碰闭包，节点保持干净，
            // 并把上次测量的文本状态交还调用方（新元素直接持有旧测量）。
            let carried = match (fingerprint, retained.measure_fingerprint) {
                (Some(new), Some(old)) if new == old => retained.measured_state.clone(),
                _ => None,
            };
            let mut clean = carried.is_some();
            if self.taffy.style(node).expect(EXPECT_MESSAGE) != &taffy_style {
                self.taffy
                    .set_style(node, taffy_style)
                    .expect(EXPECT_MESSAGE);
                clean = false;
            }
            if !retained.children.is_empty() {
                self.taffy.set_children(node, &[]).expect(EXPECT_MESSAGE);
                retained.children = Vec::new();
                clean = false;
            }
            if carried.is_none() {
                self.taffy
                    .set_node_context(
                        node,
                        Some(NodeContext {
                            measure: StackSafe::new(Box::new(measure)),
                        }),
                    )
                    .expect(EXPECT_MESSAGE);
                retained.measure_fingerprint = fingerprint;
                retained.measured_state = text_state;
                retained.has_measure = true;
                clean = false;
            }
            if clean {
                self.layout_nodes_reused += 1;
            } else {
                self.layout_nodes_rewritten += 1;
            }
            return (LayoutId::from(node), carried);
        }

        let node = self
            .taffy
            .new_leaf_with_context(
                taffy_style,
                NodeContext {
                    measure: StackSafe::new(Box::new(measure)),
                },
            )
            .expect(EXPECT_MESSAGE);
        self.claimed_nodes.insert(node);
        self.retained.insert(
            key,
            RetainedNode {
                node,
                children: Vec::new(),
                measure_fingerprint: fingerprint,
                has_measure: true,
                measured_state: text_state,
            },
        );
        self.layout_nodes_allocated += 1;
        (LayoutId::from(node), None)
    }

    /// 将给定节点样式的任何 `auto` 尺寸视为填充 `size`。
    ///
    /// 这在布局之前应用于窗口根节点，使其行为类似于 Web 上的根元素，
    /// 除非给定显式尺寸，否则会拉伸以填充初始包含块（视口）。显式样式化的
    /// 尺寸会被保留。
    pub fn stretch_auto_size_to_fill(
        &mut self,
        id: LayoutId,
        size: Size<Pixels>,
        scale_factor: f32,
    ) {
        let style = self.taffy.style(id.0).expect(EXPECT_MESSAGE);
        let stretch_width = style.size.width.is_auto();
        let stretch_height = style.size.height.is_auto();
        if !stretch_width && !stretch_height {
            return;
        }
        let mut stretched = style.clone();
        if stretch_width {
            stretched.size.width =
                taffy::style::Dimension::length(round_to_device_pixel(size.width.0, scale_factor));
        }
        if stretch_height {
            stretched.size.height =
                taffy::style::Dimension::length(round_to_device_pixel(size.height.0, scale_factor));
        }
        // Retained：样式不变不写，否则每帧致脏根节点、布局缓存全废。
        if stretched != *style {
            self.taffy.set_style(id.0, stretched).expect(EXPECT_MESSAGE);
        }
    }

    // Used to understand performance

    #[stacksafe]
    pub fn compute_layout(
        &mut self,
        id: LayoutId,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Leaving this here until we have a better instrumentation approach.
        // println!("Laying out {} children", self.count_all_children(id)?);
        // println!("Max layout depth: {}", self.max_depth(0, id)?);

        // Output the edges (branches) of the tree in Mermaid format for visualization.
        // println!("Edges:");
        // for (a, b) in self.get_edges(id)? {
        // }
        //

        // Taffy 缓存不感知可用空间变化：空间变化而样式不变时仍须致脏，
        // 否则读到旧尺寸下的缓存布局（窗口缩放场景）。
        let space_changed = self
            .last_spaces
            .get(&id)
            .is_none_or(|last| *last != available_space);
        if space_changed {
            self.taffy.mark_dirty(id.into()).expect(EXPECT_MESSAGE);
            self.last_spaces.insert(id, available_space);
        }

        if !self.computed_layouts.insert(id) {
            let stack = &mut self.layout_bounds_scratch_space;
            stack.push(id);
            while let Some(id) = stack.pop() {
                self.absolute_layout_bounds.remove(&id);
                self.absolute_outer_origins.remove(&id);
                stack.extend(
                    self.taffy
                        .children(id.into())
                        .expect(EXPECT_MESSAGE)
                        .into_iter()
                        .map(LayoutId::from),
                );
            }
        }

        let scale_factor = window.scale_factor();

        let transform = |v: AvailableSpace| match v {
            AvailableSpace::Definite(pixels) => {
                AvailableSpace::Definite(Pixels(pixels.0 * scale_factor))
            }
            AvailableSpace::MinContent => AvailableSpace::MinContent,
            AvailableSpace::MaxContent => AvailableSpace::MaxContent,
        };
        let available_space = size(
            transform(available_space.width),
            transform(available_space.height),
        );

        self.taffy
            .compute_layout_with_measure(
                id.into(),
                available_space.into(),
                |known_dimensions, available_space, _id, node_context, _style| {
                    let Some(node_context) = node_context else {
                        return taffy::geometry::Size::default();
                    };

                    let known_dimensions = Size {
                        width: known_dimensions.width.map(|e| Pixels(e / scale_factor)),
                        height: known_dimensions.height.map(|e| Pixels(e / scale_factor)),
                    };

                    let available_space: Size<AvailableSpace> = available_space.into();
                    let untransform = |ev: AvailableSpace| match ev {
                        AvailableSpace::Definite(pixels) => {
                            AvailableSpace::Definite(Pixels(pixels.0 / scale_factor))
                        }
                        AvailableSpace::MinContent => AvailableSpace::MinContent,
                        AvailableSpace::MaxContent => AvailableSpace::MaxContent,
                    };
                    let available_space = size(
                        untransform(available_space.width),
                        untransform(available_space.height),
                    );

                    let measured_size: Size<Pixels> =
                        (node_context.measure)(known_dimensions, available_space, window, cx);
                    snap_measured_size_to_device_pixels(measured_size, scale_factor).into()
                },
            )
            .expect(EXPECT_MESSAGE);
    }

    // Pixel snapping
    //
    // Painting primitives at non-integer pixel coordinates produces blurry
    // output. Pixel snapping converts layout coordinates into integer
    // device-pixel coordinates so painted edges land exactly on physical
    // pixel boundaries.
    //
    // Non-integer coordinates can arise for several reasons, including:
    //   - flex distribution, percentages, centering, and text measurement
    //     can produce fractional element sizes and positions;
    //   - at fractional scale factors (for example 125% or 150%), integer
    //     logical-pixel values can map to non-integer device-pixel values.
    //
    // We pixel-snap by rounding in device-pixel space, after multiplying
    // by `scale_factor`, so that snapping targets physical pixels. Bounds
    // are divided by `scale_factor` before being returned to RGPUI.
    //
    // Midpoints are rounded toward zero. This is a stylistic choice: a
    // 1-logical-pixel line at 150% scale should render as 1 dp rather than
    // 2 dp.
    //
    // Pixel snapping is done in two phases:
    //
    //  1. Pre-layout metric snapping. Before Taffy computes layout, all
    //     authored absolute lengths are rounded in `to_taffy`. This
    //     includes borders, padding, gaps, and explicit sizes.
    //     Custom-measured leaf nodes have their measured sizes rounded up
    //     to integer device-pixel lengths.
    //
    //  2. Post-layout edge snapping. After Taffy resolves the tree, layout
    //     relationships such as flex shares, grid tracks, percentages, and
    //     centering can produce new fractional edge positions. Boxes now
    //     have edges in absolute coordinates, and snapping must decide
    //     where those edges land on the device-pixel grid.
    //
    // Ideally, post-layout snapping would satisfy:
    //
    //  - Edge closure. Two raw layout edges at the same absolute position
    //    should snap to the same pixel column.
    //  - Translation stability. A component's internal geometry should not
    //    change when it moves to a new absolute position.
    //
    // These goals are in tension because rounding is not associative.
    // The simple local schemes make different tradeoffs:
    //
    //  - Absolute edge rounding gives each window coordinate one answer,
    //    so coincident edges always close globally. But a span's snapped
    //    length is `round(far) - round(near)`, which may change by 1 dp
    //    as its absolute origin moves.
    //
    //  - Parent-relative edge rounding rounds each child inside its
    //    parent's coordinate space. This guarantees translation stability,
    //    but a shared edge reached through different parents can
    //    accumulate different rounding, causing non-closure between
    //    cousins.
    //
    //  - Length rounding rounds each width, height, and thickness
    //    independently and then places boxes from those rounded lengths.
    //    Sizes stay stable under translation, but neighboring boxes derive
    //    their shared boundary from different sources, so closure is not
    //    guaranteed.
    //
    // We apply absolute edge rounding for each element's outer box in
    // post-layout rounding to preserve closure. Border and padding widths
    // are not touched by post-layout rounding; they keep their pre-layout
    // rounded value so that they remain stable under translation.
    //
    // This gives both closure and translation stability in the case that
    // all local metrics are integer device-pixel lengths. Pre-layout
    // rounding covers that in most cases. The exception is metrics
    // resolved by layout relationships, such as percentages. Outer box
    // edges will still close globally, and painted border widths are still
    // snapped independently, but the raw content-box origin can carry a
    // 1dp residual into descendants.

    pub fn layout_bounds(&mut self, id: LayoutId, scale_factor: f32) -> Bounds<Pixels> {
        if let Some(layout) = self.absolute_layout_bounds.get(&id).cloned() {
            return layout;
        }

        let layout = self.taffy.layout(id.into()).expect(EXPECT_MESSAGE);
        let layout_location = layout.location;
        let layout_size = layout.size;
        let parent = self.taffy.parent(id.0);

        let absolute_outer_origin = match parent {
            Some(parent_id) => {
                let parent_id = LayoutId::from(parent_id);
                self.layout_bounds(parent_id, scale_factor);
                let parent_origin = *self
                    .absolute_outer_origins
                    .get(&parent_id)
                    .expect("parent absolute outer origin should be cached");
                parent_origin + Point::from(layout_location)
            }
            None => Point::from(layout_location),
        };
        self.absolute_outer_origins
            .insert(id, absolute_outer_origin);

        let absolute_far = absolute_outer_origin + Point::from(Size::from(layout_size));
        let snapped_bounds = Bounds::from_corners(
            absolute_outer_origin.map(round_half_toward_zero),
            absolute_far.map(round_half_toward_zero),
        );

        let bounds = (snapped_bounds / scale_factor).map(Pixels);
        self.absolute_layout_bounds.insert(id, bounds);
        bounds
    }
}

/// 布局节点的唯一标识符，在向 Taffy 请求布局时生成。
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct LayoutId(NodeId);

impl LayoutId {
    fn to_taffy_slice(node_ids: &[Self]) -> &[taffy::NodeId] {
        // SAFETY: LayoutId is repr(transparent) to taffy::tree::NodeId.
        unsafe { std::mem::transmute::<&[LayoutId], &[taffy::NodeId]>(node_ids) }
    }
}

impl std::hash::Hash for LayoutId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        u64::from(self.0).hash(state);
    }
}

impl From<NodeId> for LayoutId {
    fn from(node_id: NodeId) -> Self {
        Self(node_id)
    }
}

impl From<LayoutId> for NodeId {
    fn from(layout_id: LayoutId) -> NodeId {
        layout_id.0
    }
}

fn snap_measured_size_to_device_pixels(size: Size<Pixels>, scale_factor: f32) -> Size<f32> {
    size.map(|d| ceil_to_device_pixel(d.0.max(0.0), scale_factor))
}

fn border_widths_to_taffy(
    widths: &Edges<AbsoluteLength>,
    rem_size: Pixels,
    scale_factor: f32,
) -> TaffyRect<taffy::style::LengthPercentage> {
    let snap = |w: &AbsoluteLength| {
        taffy::style::LengthPercentage::length(round_stroke_to_device_pixel(
            w.to_pixels(rem_size).0,
            scale_factor,
        ))
    };
    TaffyRect {
        top: snap(&widths.top),
        right: snap(&widths.right),
        bottom: snap(&widths.bottom),
        left: snap(&widths.left),
    }
}

trait ToTaffy<Output> {
    fn to_taffy(&self, rem_size: Pixels, scale_factor: f32) -> Output;
}

impl ToTaffy<taffy::style::Style> for Style {
    fn to_taffy(&self, rem_size: Pixels, scale_factor: f32) -> taffy::style::Style {
        use taffy::style_helpers::{fr, length, minmax, repeat};

        fn to_grid_line(
            placement: &Range<crate::GridPlacement>,
        ) -> taffy::Line<taffy::GridPlacement> {
            taffy::Line {
                start: placement.start.into(),
                end: placement.end.into(),
            }
        }

        fn to_grid_repeat<T: taffy::style::CheapCloneStr>(
            unit: &Option<GridTemplate>,
        ) -> Vec<taffy::GridTemplateComponent<T>> {
            unit.map(|template| {
                match template.min_size {
                    // grid-template-*: repeat(<number>, minmax(0, 1fr));
                    crate::GridTemplateMinSize::Zero => {
                        vec![repeat(
                            template.repeat,
                            vec![minmax(length(0.0_f32), fr(1.0_f32))],
                        )]
                    }
                    // grid-template-*: repeat(<number>, minmax(min-content, 1fr));
                    crate::GridTemplateMinSize::MinContent => {
                        vec![repeat(
                            template.repeat,
                            vec![minmax(min_content(), fr(1.0_f32))],
                        )]
                    }
                    // grid-template-*: repeat(<number>, minmax(0, max-content))
                    crate::GridTemplateMinSize::MaxContent => {
                        vec![repeat(
                            template.repeat,
                            vec![minmax(length(0.0_f32), max_content())],
                        )]
                    }
                }
            })
            .unwrap_or_default()
        }

        taffy::style::Style {
            display: self.display.into(),
            overflow: self.overflow.into(),
            scrollbar_width: self.scrollbar_width.to_taffy(rem_size, scale_factor),
            position: self.position.into(),
            inset: self.inset.to_taffy(rem_size, scale_factor),
            size: self.size.to_taffy(rem_size, scale_factor),
            min_size: self.min_size.to_taffy(rem_size, scale_factor),
            max_size: self.max_size.to_taffy(rem_size, scale_factor),
            aspect_ratio: self.aspect_ratio,
            margin: self.margin.to_taffy(rem_size, scale_factor),
            padding: self.padding.to_taffy(rem_size, scale_factor),
            border: border_widths_to_taffy(&self.border_widths, rem_size, scale_factor),
            align_items: self.align_items.map(|x| x.into()),
            align_self: self.align_self.map(|x| x.into()),
            align_content: self.align_content.map(|x| x.into()),
            justify_content: self.justify_content.map(|x| x.into()),
            gap: self.gap.to_taffy(rem_size, scale_factor),
            flex_direction: self.flex_direction.into(),
            flex_wrap: self.flex_wrap.into(),
            flex_basis: self.flex_basis.to_taffy(rem_size, scale_factor),
            flex_grow: self.flex_grow,
            flex_shrink: self.flex_shrink,
            grid_template_rows: to_grid_repeat(&self.grid_rows),
            grid_template_columns: to_grid_repeat(&self.grid_cols),
            grid_row: self
                .grid_location
                .as_ref()
                .map(|location| to_grid_line(&location.row))
                .unwrap_or_default(),
            grid_column: self
                .grid_location
                .as_ref()
                .map(|location| to_grid_line(&location.column))
                .unwrap_or_default(),
            ..Default::default()
        }
    }
}

impl ToTaffy<f32> for AbsoluteLength {
    fn to_taffy(&self, rem_size: Pixels, scale_factor: f32) -> f32 {
        round_to_device_pixel(self.to_pixels(rem_size).0, scale_factor)
    }
}

impl ToTaffy<taffy::style::LengthPercentageAuto> for Length {
    fn to_taffy(
        &self,
        rem_size: Pixels,
        scale_factor: f32,
    ) -> taffy::prelude::LengthPercentageAuto {
        match self {
            Length::Definite(length) => length.to_taffy(rem_size, scale_factor),
            Length::Auto => taffy::prelude::LengthPercentageAuto::auto(),
        }
    }
}

impl ToTaffy<taffy::style::Dimension> for Length {
    fn to_taffy(&self, rem_size: Pixels, scale_factor: f32) -> taffy::prelude::Dimension {
        match self {
            Length::Definite(length) => length.to_taffy(rem_size, scale_factor),
            Length::Auto => taffy::prelude::Dimension::auto(),
        }
    }
}

impl ToTaffy<taffy::style::LengthPercentage> for DefiniteLength {
    fn to_taffy(&self, rem_size: Pixels, scale_factor: f32) -> taffy::style::LengthPercentage {
        match self {
            DefiniteLength::Absolute(length) => length.to_taffy(rem_size, scale_factor),
            DefiniteLength::Fraction(fraction) => {
                taffy::style::LengthPercentage::percent(*fraction)
            }
        }
    }
}

impl ToTaffy<taffy::style::LengthPercentageAuto> for DefiniteLength {
    fn to_taffy(&self, rem_size: Pixels, scale_factor: f32) -> taffy::style::LengthPercentageAuto {
        match self {
            DefiniteLength::Absolute(length) => length.to_taffy(rem_size, scale_factor),
            DefiniteLength::Fraction(fraction) => {
                taffy::style::LengthPercentageAuto::percent(*fraction)
            }
        }
    }
}

impl ToTaffy<taffy::style::Dimension> for DefiniteLength {
    fn to_taffy(&self, rem_size: Pixels, scale_factor: f32) -> taffy::style::Dimension {
        match self {
            DefiniteLength::Absolute(length) => length.to_taffy(rem_size, scale_factor),
            DefiniteLength::Fraction(fraction) => taffy::style::Dimension::percent(*fraction),
        }
    }
}

impl ToTaffy<taffy::style::LengthPercentage> for AbsoluteLength {
    fn to_taffy(&self, rem_size: Pixels, scale_factor: f32) -> taffy::style::LengthPercentage {
        taffy::style::LengthPercentage::length(self.to_taffy(rem_size, scale_factor))
    }
}

impl ToTaffy<taffy::style::LengthPercentageAuto> for AbsoluteLength {
    fn to_taffy(&self, rem_size: Pixels, scale_factor: f32) -> taffy::style::LengthPercentageAuto {
        taffy::style::LengthPercentageAuto::length(self.to_taffy(rem_size, scale_factor))
    }
}

impl ToTaffy<taffy::style::Dimension> for AbsoluteLength {
    fn to_taffy(&self, rem_size: Pixels, scale_factor: f32) -> taffy::style::Dimension {
        taffy::style::Dimension::length(self.to_taffy(rem_size, scale_factor))
    }
}

impl<T, T2> From<TaffyPoint<T>> for Point<T2>
where
    T: Into<T2>,
    T2: Clone + Debug + Default + PartialEq,
{
    fn from(point: TaffyPoint<T>) -> Point<T2> {
        Point {
            x: point.x.into(),
            y: point.y.into(),
        }
    }
}

impl<T, T2> From<Point<T>> for TaffyPoint<T2>
where
    T: Into<T2> + Clone + Debug + Default + PartialEq,
{
    fn from(val: Point<T>) -> Self {
        TaffyPoint {
            x: val.x.into(),
            y: val.y.into(),
        }
    }
}

impl<T, U> ToTaffy<TaffySize<U>> for Size<T>
where
    T: ToTaffy<U> + Clone + Debug + Default + PartialEq,
{
    fn to_taffy(&self, rem_size: Pixels, scale_factor: f32) -> TaffySize<U> {
        TaffySize {
            width: self.width.to_taffy(rem_size, scale_factor),
            height: self.height.to_taffy(rem_size, scale_factor),
        }
    }
}

impl<T, U> ToTaffy<TaffyRect<U>> for Edges<T>
where
    T: ToTaffy<U> + Clone + Debug + Default + PartialEq,
{
    fn to_taffy(&self, rem_size: Pixels, scale_factor: f32) -> TaffyRect<U> {
        TaffyRect {
            top: self.top.to_taffy(rem_size, scale_factor),
            right: self.right.to_taffy(rem_size, scale_factor),
            bottom: self.bottom.to_taffy(rem_size, scale_factor),
            left: self.left.to_taffy(rem_size, scale_factor),
        }
    }
}

impl<T, U> From<TaffySize<T>> for Size<U>
where
    T: Into<U>,
    U: Clone + Debug + Default + PartialEq,
{
    fn from(taffy_size: TaffySize<T>) -> Self {
        Size {
            width: taffy_size.width.into(),
            height: taffy_size.height.into(),
        }
    }
}

impl<T, U> From<Size<T>> for TaffySize<U>
where
    T: Into<U> + Clone + Debug + Default + PartialEq,
{
    fn from(size: Size<T>) -> Self {
        TaffySize {
            width: size.width.into(),
            height: size.height.into(),
        }
    }
}

/// 元素可用于布局的空间
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub enum AvailableSpace {
    /// 可用空间量是指定的像素数
    Definite(Pixels),
    /// 可用空间量是无限的，节点应在最小内容约束下布局
    #[default]
    MinContent,
    /// 可用空间量是无限的，节点应在最大内容约束下布局
    MaxContent,
}

impl AvailableSpace {
    /// 返回宽度和高度都设置为 `AvailableSpace::MinContent` 的 `Size`。
    ///
    /// 当您想为两个维度创建具有最小内容约束的 `Size` 时，此函数很有用。
    ///
    /// # 示例
    ///
    /// ```
    /// use rgpui::AvailableSpace;
    /// let min_content_size = AvailableSpace::min_size();
    /// assert_eq!(min_content_size.width, AvailableSpace::MinContent);
    /// assert_eq!(min_content_size.height, AvailableSpace::MinContent);
    /// ```
    pub const fn min_size() -> Size<Self> {
        Size {
            width: Self::MinContent,
            height: Self::MinContent,
        }
    }
}

impl From<AvailableSpace> for TaffyAvailableSpace {
    fn from(space: AvailableSpace) -> TaffyAvailableSpace {
        match space {
            AvailableSpace::Definite(Pixels(value)) => TaffyAvailableSpace::Definite(value),
            AvailableSpace::MinContent => TaffyAvailableSpace::MinContent,
            AvailableSpace::MaxContent => TaffyAvailableSpace::MaxContent,
        }
    }
}

impl From<TaffyAvailableSpace> for AvailableSpace {
    fn from(space: TaffyAvailableSpace) -> AvailableSpace {
        match space {
            TaffyAvailableSpace::Definite(value) => AvailableSpace::Definite(Pixels(value)),
            TaffyAvailableSpace::MinContent => AvailableSpace::MinContent,
            TaffyAvailableSpace::MaxContent => AvailableSpace::MaxContent,
        }
    }
}

impl From<Pixels> for AvailableSpace {
    fn from(pixels: Pixels) -> Self {
        AvailableSpace::Definite(pixels)
    }
}

impl From<Size<Pixels>> for Size<AvailableSpace> {
    fn from(size: Size<Pixels>) -> Self {
        Size {
            width: AvailableSpace::Definite(size.width),
            height: AvailableSpace::Definite(size.height),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_widths_to_taffy_use_stroke_snapping() {
        let border_widths = Edges {
            top: Pixels(0.0).into(),
            right: Pixels(0.4).into(),
            bottom: Pixels(0.5).into(),
            left: Pixels(1.6).into(),
        };
        let taffy_border = border_widths_to_taffy(&border_widths, Pixels(16.0), 1.0);

        assert_eq!(
            taffy_border.top,
            taffy::style::LengthPercentage::length(0.0)
        );
        assert_eq!(
            taffy_border.right,
            taffy::style::LengthPercentage::length(1.0)
        );
        assert_eq!(
            taffy_border.bottom,
            taffy::style::LengthPercentage::length(1.0)
        );
        assert_eq!(
            taffy_border.left,
            taffy::style::LengthPercentage::length(2.0)
        );
    }
}
