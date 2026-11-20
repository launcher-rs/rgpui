//! 软件适配器部分重绘的判定层（D4a）。
//!
//! 对标 gpui-fast `3268abe`：每帧把场景图元压成「签名」，与上一帧比对得出
//! [`Plan`]——无变化可复用整帧，小变化只需重画 damage 矩形，整像素垂直滚动
//! 可以搬移上一帧的像素。本阶段**只做判定**，不改绘制路径：接入点在
//! `DirectXRenderer::draw`，默认关闭，用 `RGPUI_PARTIAL_REDRAW=1` 打开后
//! 计算并记录计划，真正的 canvas / scissor / 滚动搬移留给 D4b。
//!
//! 适配 rgpui 的差异（不能照搬上游）：
//! - `Quad.depth` 是 D2 排序的副产物，必须排除在签名之外；
//! - rgpui 的 quad 带 `transform` / `blend_mode` / `continuous_corners`，
//!   多色精灵带 `transformation`，都要进签名，否则变形图元会被误判成可搬移；
//! - `Scene.layers` 不公开（D1 的图层是挂起落地，不是滚动层），故不检查滚动
//!   层；`surfaces` 非空时保守地整帧绘制。
//!
//! D4b 在此之上实现 canvas 双缓冲：damage 区 scissor 重绘、整像素滚动搬移，
//! 并同步维护深度纹理（rgpui 的 anti-overdraw 需要）。只在软件适配器且
//! `RGPUI_PARTIAL_REDRAW=1` 时启用，默认走整帧路径。

use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
};

use anyhow::{Context as _, Result};
use rgpui::{
    AtlasTextureId, AtlasTile, Background, Bounds, ContentMask, Corners, Edges, Hsla, IsZero,
    ScaledPixels, Scene, TransformationMatrix,
};
use windows::Win32::{
    Foundation::RECT,
    Graphics::{
        Direct3D11::{
            D3D11_BIND_RENDER_TARGET, D3D11_BOX, D3D11_CULL_BACK, D3D11_FILL_SOLID,
            D3D11_RASTERIZER_DESC, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
            ID3D11DepthStencilView, ID3D11Device, ID3D11DeviceContext, ID3D11DeviceContext1,
            ID3D11RasterizerState, ID3D11RenderTargetView, ID3D11Texture2D, ID3D11View,
        },
        Dxgi::{Common::DXGI_SAMPLE_DESC, IDXGIAdapter1},
    },
};
use windows::core::Interface;

/// damage 超过窗口这个比例就整帧绘制。
const MAX_DAMAGE: f32 = 0.6;
/// 超过这个数量的 damage 矩形先合并成外接框。
const MAX_RECTS: usize = 8;
/// 超过这个数量的矩形先按窗口横向带归并。
const MAX_MERGED: usize = 64;
/// 一帧滚动至少要这么多同形图元投票。
const MIN_MOVED: usize = 4;
/// 两次分别算出的坐标允许的最后几位差。
const SAME: f32 = 1.0 / 1024.0;

/// 设备像素里的一个矩形。
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Area {
    pub(crate) x0: f32,
    pub(crate) y0: f32,
    pub(crate) x1: f32,
    pub(crate) y1: f32,
}

impl Area {
    pub(crate) fn of(b: &Bounds<ScaledPixels>) -> Area {
        Area {
            x0: b.origin.x.0,
            y0: b.origin.y.0,
            x1: b.origin.x.0 + b.size.width.0,
            y1: b.origin.y.0 + b.size.height.0,
        }
    }

    fn and(&self, o: &Area) -> Area {
        Area {
            x0: self.x0.max(o.x0),
            y0: self.y0.max(o.y0),
            x1: self.x1.min(o.x1),
            y1: self.y1.min(o.y1),
        }
    }

    fn union(&self, o: &Area) -> Area {
        Area {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
        }
    }

    fn is_empty(&self) -> bool {
        !(self.x1 > self.x0 && self.y1 > self.y0)
    }

    fn area(&self) -> f32 {
        if self.is_empty() {
            0.0
        } else {
            (self.x1 - self.x0) * (self.y1 - self.y0)
        }
    }

    fn moved(&self, dy: f32) -> Area {
        Area {
            y0: self.y0 + dy,
            y1: self.y1 + dy,
            ..*self
        }
    }

    fn grown(&self, by: f32) -> Area {
        Area {
            x0: self.x0 - by,
            y0: self.y0 - by,
            x1: self.x1 + by,
            y1: self.y1 + by,
        }
    }

    fn touches(&self, o: &Area) -> bool {
        self.x0 <= o.x1 && o.x0 <= self.x1 && self.y0 <= o.y1 && o.y0 <= self.y1
    }

    fn within(&self, o: &Area) -> bool {
        self.is_empty()
            || (self.x0 >= o.x0 - SAME
                && self.y0 >= o.y0 - SAME
                && self.x1 <= o.x1 + SAME
                && self.y1 <= o.y1 + SAME)
    }

    fn same(&self, o: &Area) -> bool {
        (self.is_empty() && o.is_empty())
            || ((self.x0 - o.x0).abs() < SAME
                && (self.y0 - o.y0).abs() < SAME
                && (self.x1 - o.x1).abs() < SAME
                && (self.y1 - o.y1).abs() < SAME)
    }

    /// 覆盖到的整像素矩形。
    fn pixels(&self) -> Area {
        Area {
            x0: self.x0.floor(),
            y0: self.y0.floor(),
            x1: self.x1.ceil(),
            y1: self.y1.ceil(),
        }
    }

    fn on_grid(&self) -> bool {
        let on = |v: f32| (v - v.round()).abs() < SAME;
        on(self.x0) && on(self.y0) && on(self.x1) && on(self.y1)
    }

    fn bits(&self) -> [u32; 4] {
        [
            self.x0.to_bits(),
            self.y0.to_bits(),
            self.x1.to_bits(),
            self.y1.to_bits(),
        ]
    }

    /// 转成整数像素矩形，供 scissor / ClearView 使用。
    fn rect(&self) -> RECT {
        RECT {
            left: self.x0 as i32,
            top: self.y0 as i32,
            right: self.x1 as i32,
            bottom: self.y1 as i32,
        }
    }
}

/// 一个图元在比对眼中的样子：除位置以外的外观、绘制顺序、裁剪与可达范围。
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sig {
    /// 除位置、顺序、裁剪以外的外观 hash。
    key: u64,
    order: u32,
    /// 图元自身的边界。
    bounds: Area,
    /// 内容裁剪。
    mask: Area,
    /// 图元可能碰到的像素（含抗锯齿 / 阴影余量，且裁到 mask 内）。
    reach: Area,
    /// 在任意裁剪下都是同一种颜色（实心直角无边框矩形）。
    plain: bool,
    /// 图集瓦片自上一帧重传过：无论字节是否相同都算变化。
    fresh: bool,
}

/// 图元外观的增量 hash。
struct Key(std::collections::hash_map::DefaultHasher);

impl Key {
    fn new(kind: u8) -> Key {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        kind.hash(&mut hasher);
        Key(hasher)
    }

    fn f(mut self, v: f32) -> Key {
        v.to_bits().hash(&mut self.0);
        self
    }

    fn u(mut self, v: u64) -> Key {
        v.hash(&mut self.0);
        self
    }

    /// 保守地把整个值 Debug 进 hash：用于字段多且可能变化的复合图元。
    fn debug(mut self, v: &dyn std::fmt::Debug) -> Key {
        // Debug 在单次进程内稳定，且这里只做「同帧两版是否相同」的比对。
        format!("{v:?}").hash(&mut self.0);
        self
    }

    fn color(self, c: &Hsla) -> Key {
        self.f(c.h).f(c.s).f(c.l).f(c.a)
    }

    fn corners(self, c: &Corners<ScaledPixels>) -> Key {
        self.f(c.top_left.0)
            .f(c.top_right.0)
            .f(c.bottom_right.0)
            .f(c.bottom_left.0)
    }

    fn edges(self, e: &Edges<ScaledPixels>) -> Key {
        self.f(e.top.0).f(e.right.0).f(e.bottom.0).f(e.left.0)
    }

    fn size(self, b: &Bounds<ScaledPixels>) -> Key {
        self.f(b.size.width.0).f(b.size.height.0)
    }

    fn tile(self, t: &AtlasTile) -> Key {
        self.u(t.texture_id.index as u64)
            .u(t.texture_id.kind as u64)
            .u(t.tile_id.0 as u64)
            .u(t.bounds.origin.x.0 as u64)
            .u(t.bounds.origin.y.0 as u64)
            .u(t.bounds.size.width.0 as u64)
            .u(t.bounds.size.height.0 as u64)
    }

    fn background(mut self, background: &Background) -> Key {
        match background.as_solid() {
            Some(color) => self.u(0).color(&color),
            None => {
                format!("{background:?}").hash(&mut self.0);
                self
            }
        }
    }

    /// 变形精灵的可达范围随矩阵变化，位置也进了 hash：搬移后它是另一个图元。
    fn transformation(self, t: &TransformationMatrix, bounds: &Bounds<ScaledPixels>) -> Key {
        let key = self
            .f(t.rotation_scale[0][0])
            .f(t.rotation_scale[0][1])
            .f(t.rotation_scale[1][0])
            .f(t.rotation_scale[1][1])
            .f(t.translation[0])
            .f(t.translation[1]);
        if is_unit(t) {
            key
        } else {
            key.f(bounds.origin.x.0).f(bounds.origin.y.0)
        }
    }

    fn done(self) -> u64 {
        self.0.finish()
    }
}

fn is_unit(t: &TransformationMatrix) -> bool {
    t.rotation_scale == [[1.0, 0.0], [0.0, 1.0]] && t.translation == [0.0, 0.0]
}

/// 矩阵把 bounds 映射到的外接框。行列两种读法都算，取并集以覆盖 shader 的
/// 任一种实现。
fn transformed(bounds: &Bounds<ScaledPixels>, t: &TransformationMatrix) -> Area {
    let area = Area::of(bounds);
    if is_unit(t) {
        return area;
    }
    let m = t.rotation_scale;
    let mut out: Option<Area> = None;
    for (x, y) in [
        (area.x0, area.y0),
        (area.x1, area.y0),
        (area.x0, area.y1),
        (area.x1, area.y1),
    ] {
        for (tx, ty) in [
            (m[0][0] * x + m[0][1] * y, m[1][0] * x + m[1][1] * y),
            (m[0][0] * x + m[1][0] * y, m[0][1] * x + m[1][1] * y),
        ] {
            let (tx, ty) = (tx + t.translation[0], ty + t.translation[1]);
            let point = Area {
                x0: tx,
                y0: ty,
                x1: tx,
                y1: ty,
            };
            out = Some(out.map_or(point, |o| o.union(&point)));
        }
    }
    out.unwrap_or(area)
}

fn sig(
    key: u64,
    order: u32,
    bounds: &Bounds<ScaledPixels>,
    mask: &ContentMask<ScaledPixels>,
    grow: f32,
) -> Sig {
    let bounds = Area::of(bounds);
    let mask = Area::of(&mask.bounds);
    Sig {
        key,
        order,
        bounds,
        mask,
        reach: bounds.grown(grow).and(&mask),
        plain: false,
        fresh: false,
    }
}

/// 把场景压成签名；`None` 表示这帧不能比较（有原生 surface）。
///
/// `uploaded` 是自上一帧以来重传的图集瓦片，`dynamic` 判断纹理是否为动态纹理。
pub(crate) fn signatures(
    scene: &Scene,
    uploaded: &HashSet<(AtlasTextureId, u32)>,
    dynamic: &dyn Fn(AtlasTextureId) -> bool,
) -> Option<Vec<Sig>> {
    if !scene.surfaces.is_empty() {
        return None;
    }
    let fresh = |tile: &AtlasTile| {
        uploaded.contains(&(tile.texture_id, tile.tile_id.0)) || dynamic(tile.texture_id)
    };
    let mut out = Vec::with_capacity(scene.len());
    for q in &scene.quads {
        let solid = q.background.as_solid();
        // 注意：q.depth 是 D2 排序的副产物，进 key 会把纯排序变化误判为内容变化。
        let key = Key::new(1)
            .size(&q.bounds)
            .u(q.border_style as u64)
            .background(&q.background)
            .color(&q.border_color)
            .corners(&q.corner_radii)
            .edges(&q.border_widths)
            .transformation(&q.transform, &q.bounds)
            .u(q.blend_mode as u64)
            .u(q.continuous_corners as u64)
            .done();
        let c = &q.corner_radii;
        let w = &q.border_widths;
        let mut s = sig(key, q.order, &q.bounds, &q.content_mask, 0.0);
        s.plain = solid.is_some()
            && is_unit(&q.transform)
            && q.blend_mode == 0
            && q.continuous_corners == 0
            && c.top_left.is_zero()
            && c.top_right.is_zero()
            && c.bottom_left.is_zero()
            && c.bottom_right.is_zero()
            && w.top.is_zero()
            && w.right.is_zero()
            && w.bottom.is_zero()
            && w.left.is_zero();
        out.push(s);
    }
    for s in &scene.shadows {
        let key = Key::new(2)
            .size(&s.bounds)
            .f(s.blur_radius.0)
            .corners(&s.corner_radii)
            .color(&s.color)
            .f(s.element_bounds.origin.x.0 - s.bounds.origin.x.0)
            .f(s.element_bounds.origin.y.0 - s.bounds.origin.y.0)
            .size(&s.element_bounds)
            .corners(&s.element_corner_radii)
            .u(s.inset as u64)
            .done();
        let grow = 3.0 * s.blur_radius.0.max(0.0) + 1.0;
        out.push(sig(key, s.order, &s.bounds, &s.content_mask, grow));
    }
    for u in &scene.underlines {
        let key = Key::new(3)
            .size(&u.bounds)
            .color(&u.color)
            .f(u.thickness.0)
            .u((u.wavy != 0) as u64)
            .done();
        out.push(sig(key, u.order, &u.bounds, &u.content_mask, 1.0));
    }
    for g in &scene.monochrome_sprites {
        let t = &g.transformation;
        let key = Key::new(4)
            .size(&g.bounds)
            .tile(&g.tile)
            .color(&g.color)
            .transformation(t, &g.bounds)
            .done();
        let mut s = sig(key, g.order, &g.bounds, &g.content_mask, 1.0);
        s.reach = transformed(&g.bounds, t).grown(1.0).and(&s.mask);
        s.fresh = fresh(&g.tile);
        out.push(s);
    }
    for g in &scene.subpixel_sprites {
        let t = &g.transformation;
        let key = Key::new(5)
            .size(&g.bounds)
            .tile(&g.tile)
            .color(&g.color)
            .transformation(t, &g.bounds)
            .done();
        let mut s = sig(key, g.order, &g.bounds, &g.content_mask, 1.0);
        s.reach = transformed(&g.bounds, t).grown(1.0).and(&s.mask);
        s.fresh = fresh(&g.tile);
        out.push(s);
    }
    for p in &scene.polychrome_sprites {
        // rgpui 的多色精灵带变换矩阵：可达范围按矩阵算，变形的不参与搬移。
        let t = &p.transformation;
        let key = Key::new(6)
            .size(&p.bounds)
            .tile(&p.tile)
            .u(p.grayscale as u64)
            .f(p.opacity)
            .corners(&p.corner_radii)
            .transformation(t, &p.bounds)
            .done();
        let mut s = sig(key, p.order, &p.bounds, &p.content_mask, 1.0);
        s.reach = transformed(&p.bounds, t).grown(1.0).and(&s.mask);
        s.fresh = fresh(&p.tile);
        out.push(s);
    }
    for p in &scene.paths {
        // 路径按顶点所处的具体位置为键：移动过的路径算新图元。
        // rgpui 的 Path 字段比上游多，用 Debug 保守覆盖。
        let key = Key::new(7).debug(p).done();
        out.push(sig(key, p.order, &p.bounds, &p.content_mask, 1.0));
    }
    Some(out)
}

/// 如何由上一帧得到这一帧。
#[derive(Debug, PartialEq)]
pub(crate) struct Plan {
    /// 上一帧的 `.0` 搬到 `.1`（同尺寸、整像素）。
    pub(crate) shift: Option<(Area, Area)>,
    /// 场景需要重画的矩形（整像素，窗口内）。
    pub(crate) damage: Vec<Area>,
}

fn placed(s: &Sig) -> (u64, [u32; 4], [u32; 4]) {
    (s.key, s.bounds.bits(), s.mask.bits())
}

/// 坐标量化到 1/64 像素，用来找候选配对。
fn coarse(v: f32) -> i64 {
    (v * 64.0).round() as i64
}

/// 除垂直位置以外相同的图元。
fn shape(s: &Sig) -> (u64, i64, i64, i64) {
    (
        s.key,
        coarse(s.bounds.x0),
        coarse(s.bounds.x1 - s.bounds.x0),
        coarse(s.bounds.y1 - s.bounds.y0),
    )
}

/// 保留图元的绘制顺序是否仍递增：参数是（旧顺序，新顺序）。
fn orders_agree(pairs: &mut [(u32, u32)]) -> bool {
    pairs.sort_unstable();
    let mut below = 0u32;
    let mut level = 0u32;
    let mut last_old = None;
    for &(old, new) in pairs.iter() {
        if last_old != Some(old) {
            below = level;
            last_old = Some(old);
        }
        if new < below {
            return false;
        }
        level = level.max(new);
    }
    true
}

/// 裁剪到窗口、对齐到整像素、按需归并 damage 矩形。
fn tidy(rects: Vec<Area>, view: &Area) -> Vec<Area> {
    let mut rects: Vec<Area> = rects
        .into_iter()
        .map(|r| r.pixels().and(view))
        .filter(|r| !r.is_empty())
        .collect();
    if rects.len() > MAX_MERGED {
        // 一整页都变了：按窗口横向带归并，多画一些仍然正确。
        let band = ((view.y1 - view.y0) / 16.0).max(1.0);
        let mut bands: HashMap<i64, Area> = HashMap::new();
        for r in &rects {
            bands
                .entry((r.y0 / band).floor() as i64)
                .and_modify(|b| *b = b.union(r))
                .or_insert(*r);
        }
        rects = bands.into_values().collect();
    }
    loop {
        let mut merged = false;
        let mut i = 0;
        while i < rects.len() {
            let mut j = i + 1;
            while j < rects.len() {
                if rects[i].touches(&rects[j]) {
                    rects[i] = rects[i].union(&rects[j]);
                    rects.swap_remove(j);
                    merged = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
        if !merged {
            break;
        }
    }
    if rects.len() > MAX_RECTS {
        let all = rects.iter().skip(1).fold(rects[0], |a, r| a.union(r));
        return vec![all];
    }
    rects
}

/// 由 `old` 在窗口 `view` 里得到 `new` 的计划；`None` 表示整帧绘制。
pub(crate) fn plan(old: &[Sig], new: &[Sig], view: Area) -> Option<Plan> {
    let mut left: HashMap<_, Vec<usize>> = HashMap::new();
    for (i, s) in old.iter().enumerate().rev() {
        left.entry(placed(s)).or_default().push(i);
    }
    let mut stayed: Vec<Option<usize>> = vec![None; new.len()];
    let mut old_kept = vec![false; old.len()];
    for (i, s) in new.iter().enumerate() {
        if s.fresh {
            continue;
        }
        if let Some(j) = left.get_mut(&placed(s)).and_then(Vec::pop) {
            stayed[i] = Some(j);
            old_kept[j] = true;
        }
    }
    let changed_new: Vec<usize> = (0..new.len()).filter(|i| stayed[*i].is_none()).collect();
    let changed_old: Vec<usize> = (0..old.len()).filter(|j| !old_kept[*j]).collect();
    let mut orders: Vec<(u32, u32)> = stayed
        .iter()
        .enumerate()
        .filter_map(|(i, j)| j.map(|j| (old[j].order, new[i].order)))
        .collect();

    // 变化了的同形图元投票：它们可能只是整幅移动过。
    let mut by_shape: HashMap<_, Vec<usize>> = HashMap::new();
    for &j in &changed_old {
        by_shape.entry(shape(&old[j])).or_default().push(j);
    }
    let mut votes: HashMap<i32, usize> = HashMap::new();
    let mut fractional = false;
    for &i in &changed_new {
        if new[i].fresh {
            continue;
        }
        for &j in by_shape.get(&shape(&new[i])).map_or(&[][..], Vec::as_slice) {
            let d = new[i].bounds.y0 - old[j].bounds.y0;
            if d.round() == 0.0 {
                continue;
            }
            if (d - d.round()).abs() < SAME {
                *votes.entry(d.round() as i32).or_default() += 1;
            } else {
                fractional = true;
            }
        }
    }
    let scroll = votes
        .iter()
        .max_by_key(|(dy, n)| (**n, -dy.abs()))
        .filter(|(_, n)| **n >= MIN_MOVED)
        .map(|(dy, _)| *dy as f32);
    let Some(dy) = scroll else {
        if fractional && !votes.is_empty() {
            return None;
        }
        if !orders_agree(&mut orders) {
            return None;
        }
        let damage = tidy(
            changed_old
                .iter()
                .map(|&j| old[j].reach)
                .chain(changed_new.iter().map(|&i| new[i].reach))
                .collect(),
            &view,
        );
        let total: f32 = damage.iter().map(Area::area).sum();
        return (total <= MAX_DAMAGE * view.area()).then_some(Plan {
            shift: None,
            damage,
        });
    };

    // 滚动的裁剪区：跟着 dy 移动的图元的 mask 并集。
    let mut region: Option<Area> = None;
    for &i in &changed_new {
        let moved = by_shape.get(&shape(&new[i])).is_some_and(|js| {
            js.iter()
                .any(|&j| (new[i].bounds.y0 - old[j].bounds.y0 - dy).abs() < SAME)
        });
        if moved && !new[i].fresh {
            region = Some(region.map_or(new[i].mask, |r| r.union(&new[i].mask)));
        }
    }
    let region = region?.and(&view);
    if region.is_empty() || !region.on_grid() || dy.abs() >= region.y1 - region.y0 {
        return None;
    }
    let region = region.pixels();
    let to = Area {
        y0: region.y0 + dy.max(0.0),
        y1: region.y1 + dy.min(0.0),
        ..region
    };
    let from = to.moved(-dy);

    // 跟着滚动一起搬的图元：同形、位置差 dy、完全在裁剪区内、落点处内容一致。
    let mut pool: HashMap<_, Vec<usize>> = HashMap::new();
    for &j in &changed_old {
        pool.entry((shape(&old[j]), coarse(old[j].bounds.y0 + dy)))
            .or_default()
            .push(j);
    }
    let mut moved_new = vec![false; new.len()];
    let mut moved_old = vec![false; old.len()];
    for &i in &changed_new {
        let s = &new[i];
        if s.fresh || !s.reach.within(&region) {
            continue;
        }
        let Some(candidates) = pool.get_mut(&(shape(s), coarse(s.bounds.y0))) else {
            continue;
        };
        let fits = |j: &usize| {
            let o = &old[*j];
            o.bounds.moved(dy).same(&s.bounds)
                && o.reach.within(&region)
                && o.reach.moved(dy).and(&to).same(&s.reach.and(&to))
        };
        if let Some(k) = candidates.iter().position(fits) {
            let j = candidates.remove(k);
            moved_new[i] = true;
            moved_old[j] = true;
            orders.push((old[j].order, s.order));
        }
    }
    if !orders_agree(&mut orders) {
        return None;
    }

    let mut damage = Vec::new();
    // 滚动露出来的条带。
    damage.push(if dy < 0.0 {
        Area {
            y0: region.y1 + dy,
            ..region
        }
    } else {
        Area {
            y1: region.y0 + dy,
            ..region
        }
    });
    let copied = |r: &Area| r.and(&from).moved(dy);
    for &j in &changed_old {
        if !moved_old[j] {
            damage.push(old[j].reach);
            damage.push(copied(&old[j].reach));
        }
    }
    for &i in &changed_new {
        if !moved_new[i] {
            damage.push(new[i].reach);
        }
    }
    // 停在裁剪区上、下面像素被搬走的非滚动图元。
    for (i, s) in new.iter().enumerate() {
        if stayed[i].is_none() || !s.reach.touches(&region) {
            continue;
        }
        let over = s.reach.and(&region);
        if s.plain {
            let reach = dy.abs();
            if s.reach.y0 > region.y0 + SAME {
                let top = Area {
                    y1: (over.y0 + reach).min(over.y1),
                    ..over
                };
                damage.extend([top, copied(&top)]);
            }
            if s.reach.y1 < region.y1 - SAME {
                let bottom = Area {
                    y0: (over.y1 - reach).max(over.y0),
                    ..over
                };
                damage.extend([bottom, copied(&bottom)]);
            }
        } else {
            damage.push(over);
            damage.push(copied(&over));
        }
    }
    let damage = tidy(damage, &view);
    let total: f32 = damage.iter().map(Area::area).sum();
    (total <= MAX_DAMAGE * view.area()).then_some(Plan {
        shift: Some((from, to)),
        damage,
    })
}

// --- Canvas 绘制 --------------------------------------------------------------

/// 把场景画进 canvas 所需的操作，由 `DirectXRenderer` 实现。
///
/// 这样 `partial` 不持有渲染器的私有字段，也便于测试里换假实现。
pub(crate) trait Drawer {
    /// 这一帧清屏用的颜色。
    fn clear(&self) -> [f32; 4];
    /// 准备绘制（上传场景缓冲、绑定深度态等）。
    fn prepare(&mut self) -> Result<()>;
    /// 把 `color`/`depth` 绑定为渲染目标并清屏；`clear` 为清屏色。
    fn begin(
        &mut self,
        color: &Option<ID3D11RenderTargetView>,
        depth: &Option<ID3D11DepthStencilView>,
        clear: [f32; 4],
    ) -> Result<()>;
    /// 把上传好的场景画一遍（不透明通道 + 混合通道）。
    fn draw(&mut self) -> Result<()>;
}

/// scissor 测试开关的 RAII guard：无论中途成功失败，都恢复原先绑定的
/// 光栅化状态与 scissor 矩形。
struct Scissor<'a> {
    context: &'a ID3D11DeviceContext,
    bound: Option<ID3D11RasterizerState>,
}

impl<'a> Scissor<'a> {
    fn on(device: &ID3D11Device, context: &'a ID3D11DeviceContext) -> Result<Scissor<'a>> {
        // SAFETY: 设备线程上的状态查询，输出指针指向局部变量。
        unsafe {
            let bound = context.RSGetState().ok();
            let mut desc = D3D11_RASTERIZER_DESC {
                FillMode: D3D11_FILL_SOLID,
                CullMode: D3D11_CULL_BACK,
                DepthClipEnable: true.into(),
                ..Default::default()
            };
            if let Some(bound) = &bound {
                bound.GetDesc(&mut desc);
            }
            desc.ScissorEnable = true.into();
            let mut scissored = None;
            device.CreateRasterizerState(&desc, Some(&mut scissored))?;
            context.RSSetState(scissored.as_ref());
            Ok(Scissor { context, bound })
        }
    }
}

impl Drop for Scissor<'_> {
    fn drop(&mut self) {
        // SAFETY: 设备线程上恢复原先绑定的状态。
        unsafe { self.context.RSSetState(self.bound.as_ref()) };
    }
}

/// 两张交替的 canvas：颜色纹理 + 深度纹理。当前帧从上一帧拷贝，只在
/// damage 区重画；滚动时把整块的像素和深度一起搬移。
struct Canvas {
    size: (u32, u32),
    color: [ID3D11Texture2D; 2],
    views: [Option<ID3D11RenderTargetView>; 2],
    depth: [ID3D11Texture2D; 2],
    depth_views: [Option<ID3D11DepthStencilView>; 2],
    current: usize,
    last: Vec<Sig>,
    clear: [f32; 4],
}

impl Canvas {
    fn new(device: &ID3D11Device, size: (u32, u32)) -> Result<Canvas> {
        let make = || -> Result<(ID3D11Texture2D, Option<ID3D11RenderTargetView>)> {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: size.0,
                Height: size.1,
                MipLevels: 1,
                ArraySize: 1,
                Format: crate::directx_renderer::RENDER_TARGET_FORMAT,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let mut texture = None;
            let mut view = None;
            // SAFETY: 合法的描述符，输出指针指向局部变量。
            unsafe {
                device.CreateTexture2D(&desc, None, Some(&mut texture))?;
                let texture = texture.context("创建 canvas 纹理失败")?;
                device.CreateRenderTargetView(&texture, None, Some(&mut view))?;
                Ok((texture, view))
            }
        };
        let (color0, view0) = make()?;
        let (color1, view1) = make()?;
        let (depth0, depth_view0) = make_depth(device, size)?;
        let (depth1, depth_view1) = make_depth(device, size)?;
        Ok(Canvas {
            size,
            color: [color0, color1],
            views: [view0, view1],
            depth: [depth0, depth1],
            depth_views: [depth_view0, depth_view1],
            current: 0,
            last: Vec::new(),
            clear: [0.0; 4],
        })
    }

    /// 用 `plan` 从上一帧造出当前帧；`None` 表示整帧绘制。返回是否画成功。
    #[allow(clippy::too_many_arguments)]
    fn make(
        &mut self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        context1: &ID3D11DeviceContext1,
        plan: Option<&Plan>,
        drawer: &mut dyn Drawer,
    ) -> Result<bool> {
        drawer.prepare()?;
        let next = 1 - self.current;
        let Some(plan) = plan else {
            drawer.begin(&self.views[next], &self.depth_views[next], self.clear)?;
            drawer.draw()?;
            self.current = next;
            return Ok(true);
        };
        // 什么都没变：沿用上一张 canvas，不画。
        if plan.shift.is_none() && plan.damage.is_empty() {
            return Ok(true);
        }
        drawer.begin(&self.views[next], &self.depth_views[next], self.clear)?;
        // SAFETY: 设备线程上的调用；矩形都落在 canvas 内。
        unsafe {
            context.CopyResource(&self.color[next], &self.color[self.current]);
            context.CopyResource(&self.depth[next], &self.depth[self.current]);
            if let Some((from, to)) = &plan.shift {
                let box_from = D3D11_BOX {
                    left: from.x0 as u32,
                    top: from.y0 as u32,
                    front: 0,
                    right: from.x1 as u32,
                    bottom: from.y1 as u32,
                    back: 1,
                };
                // 颜色与深度一起搬，保持 anti-overdraw 的深度与像素对应。
                context.CopySubresourceRegion(
                    &self.color[next],
                    0,
                    to.x0 as u32,
                    to.y0 as u32,
                    0,
                    &self.color[self.current],
                    0,
                    Some(&box_from),
                );
                context.CopySubresourceRegion(
                    &self.depth[next],
                    0,
                    to.x0 as u32,
                    to.y0 as u32,
                    0,
                    &self.depth[self.current],
                    0,
                    Some(&box_from),
                );
            }
        }
        let rects: Vec<RECT> = plan.damage.iter().map(Area::rect).collect();
        if !rects.is_empty() {
            // ClearView 只清 damage 矩形，颜色与深度都要清：深度留着会误剔除重画。
            unsafe {
                if let Some(view) = self.views[next].as_ref() {
                    context1.ClearView(&view.cast::<ID3D11View>()?, &self.clear, Some(&rects));
                }
                if let Some(view) = self.depth_views[next].as_ref() {
                    // rgpui 的深度是「越大越近」，clear 为 0.0（见 pre_draw）。
                    context1.ClearView(&view.cast::<ID3D11View>()?, &[0.0; 4], Some(&rects));
                }
            }
            let _scissor = Scissor::on(device, context)?;
            for rect in &rects {
                // SAFETY: 设备线程上的调用；rect 落在 canvas 内。
                unsafe { context.RSSetScissorRects(Some(std::slice::from_ref(rect))) };
                drawer.draw()?;
            }
        }
        self.current = next;
        Ok(true)
    }
}

/// 一个窗口的部分重绘状态：是否启用、canvas、上一帧计划。
#[derive(Default)]
pub(crate) struct Partial {
    on: bool,
    canvas: Option<Canvas>,
    /// 最近一帧的计划，供 WARP 端到端测试断言。
    pub(crate) last_plan: Option<Plan>,
}

impl Partial {
    /// 是否启用实际的部分重绘绘制。
    ///
    /// 与上游不同：rgpui 这边默认**不**在软件适配器上自动开启——D4b 的 canvas
    /// 路径缺少上游那套 579 行像素对拍测试兜底，贸然默认会让 CI/无显卡环境
    /// 静默走新路径。只有显式 `RGPUI_PARTIAL_REDRAW=1` 才启用。
    /// 软件适配器信息仅用于日志提示，不参与开关。
    pub(crate) fn for_adapter(adapter: &IDXGIAdapter1) -> Partial {
        let on = matches!(std::env::var("RGPUI_PARTIAL_REDRAW").as_deref(), Ok("1"));
        if on {
            let software = unsafe { adapter.GetDesc1() }.is_ok_and(|desc| {
                use windows::Win32::Graphics::Dxgi::DXGI_ADAPTER_FLAG_SOFTWARE;
                desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0
                    || (desc.VendorId, desc.DeviceId) == (0x1414, 0x8c)
            });
            log::info!(
                target: "rgpui::partial",
                "部分重绘已开启（软件适配器: {software}）"
            );
        }
        Partial {
            on,
            ..Default::default()
        }
    }

    /// 是否处于部分重绘模式（软件适配器 + 开关）。
    pub(crate) fn is_on(&self) -> bool {
        self.on
    }

    /// 设备或场景重置后丢掉上一帧。
    pub(crate) fn reset(&mut self) {
        self.canvas = None;
        self.last_plan = None;
    }

    /// 把这一帧画进 canvas。返回画好的 canvas 纹理（调用方拷回 swap chain），
    /// `None` 表示这帧交给整帧路径。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn frame(
        &mut self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        context1: &ID3D11DeviceContext1,
        size: (u32, u32),
        scene: &Scene,
        uploaded: &HashSet<(AtlasTextureId, u32)>,
        drawer: &mut dyn Drawer,
    ) -> Result<Option<ID3D11Texture2D>> {
        self.last_plan = None;
        if !self.on {
            return Ok(None);
        }
        // 有原生 surface 时签名不可用，整帧绘制。
        let Some(signatures) = signatures(scene, uploaded, &|_| false) else {
            self.canvas = None;
            return Ok(None);
        };
        if self.canvas.as_ref().is_none_or(|c| c.size != size) {
            self.canvas = Some(Canvas::new(device, size)?);
        }
        let canvas = self.canvas.as_mut().context("canvas missing")?;
        let view = Area {
            x0: 0.0,
            y0: 0.0,
            x1: size.0 as f32,
            y1: size.1 as f32,
        };
        let clear = drawer.clear();
        let plan = if canvas.last.is_empty() || canvas.clear != clear {
            None
        } else {
            plan(&canvas.last, &signatures, view)
        };
        if !canvas.make(device, context, context1, plan.as_ref(), drawer)? {
            self.canvas = None;
            return Ok(None);
        }
        canvas.last = signatures;
        canvas.clear = clear;
        self.last_plan = plan;
        Ok(Some(canvas.color[canvas.current].clone()))
    }
}

/// 造一张与 canvas 同尺寸的 D32_FLOAT 深度纹理及视图。
fn make_depth(
    device: &ID3D11Device,
    size: (u32, u32),
) -> Result<(ID3D11Texture2D, Option<ID3D11DepthStencilView>)> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: size.0,
        Height: size.1,
        MipLevels: 1,
        ArraySize: 1,
        Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_D32_FLOAT,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: windows::Win32::Graphics::Direct3D11::D3D11_BIND_DEPTH_STENCIL.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut texture = None;
    let mut view = None;
    // SAFETY: 合法的描述符，输出指针指向局部变量。
    unsafe {
        device.CreateTexture2D(&desc, None, Some(&mut texture))?;
        let texture = texture.context("创建 canvas 深度纹理失败")?;
        device.CreateDepthStencilView(&texture, None, Some(&mut view))?;
        Ok((texture, view))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig_at(key: u64, order: u32, x: f32, y: f32, w: f32, h: f32) -> Sig {
        Sig {
            key,
            order,
            bounds: Area {
                x0: x,
                y0: y,
                x1: x + w,
                y1: y + h,
            },
            mask: Area {
                x0: 0.0,
                y0: 0.0,
                x1: 100.0,
                y1: 100.0,
            },
            reach: Area {
                x0: x,
                y0: y,
                x1: x + w,
                y1: y + h,
            },
            plain: false,
            fresh: false,
        }
    }

    fn view() -> Area {
        Area {
            x0: 0.0,
            y0: 0.0,
            x1: 100.0,
            y1: 100.0,
        }
    }

    #[test]
    fn identical_frames_need_no_damage() {
        let old = vec![sig_at(1, 0, 1.0, 1.0, 10.0, 10.0)];
        let plan = plan(&old, &old, view()).expect("相同帧应有计划");
        assert_eq!(plan.shift, None);
        assert!(plan.damage.is_empty());
    }

    #[test]
    fn one_changed_primitive_damages_only_its_reach() {
        let old = vec![
            sig_at(1, 0, 1.0, 1.0, 10.0, 10.0),
            sig_at(2, 1, 50.0, 50.0, 10.0, 10.0),
        ];
        let mut new = old.clone();
        new[1].key = 3;
        let plan = plan(&old, &new, view()).expect("小变化应有计划");
        assert_eq!(plan.shift, None);
        assert_eq!(plan.damage.len(), 1);
        assert_eq!(
            plan.damage[0],
            Area {
                x0: 50.0,
                y0: 50.0,
                x1: 60.0,
                y1: 60.0
            }
        );
    }

    #[test]
    fn whole_pixel_scroll_moves_a_clip() {
        let old: Vec<Sig> = (0..6)
            .map(|i| sig_at(1, i, 10.0, 10.0 + i as f32 * 10.0, 8.0, 8.0))
            .collect();
        let new: Vec<Sig> = (0..6)
            .map(|i| sig_at(1, i, 10.0, 8.0 + i as f32 * 10.0, 8.0, 8.0))
            .collect();
        let plan = plan(&old, &new, view()).expect("整像素滚动应有计划");
        let (from, to) = plan.shift.expect("应有搬移");
        assert_eq!((from.y1 - from.y0), (to.y1 - to.y0));
        assert_eq!((from.y0 - to.y0).abs(), 2.0);
        assert!(!plan.damage.is_empty());
    }

    #[test]
    fn fractional_scroll_is_not_copied() {
        let old: Vec<Sig> = (0..6)
            .map(|i| sig_at(1, i, 10.0, 10.0 + i as f32 * 10.0, 8.0, 8.0))
            .collect();
        let new: Vec<Sig> = (0..6)
            .map(|i| sig_at(1, i, 10.0, 10.5 + i as f32 * 10.0, 8.0, 8.0))
            .collect();
        // 分数偏移不产生搬移：退回按 damage 重画，否则复制会错位半个像素。
        let plan = plan(&old, &new, view()).expect("小图元的分数偏移仍可按 damage 重画");
        assert_eq!(plan.shift, None);
        assert!(!plan.damage.is_empty());
    }

    #[test]
    fn reversed_orders_are_rejected() {
        let old = vec![
            sig_at(1, 0, 1.0, 1.0, 5.0, 5.0),
            sig_at(2, 1, 10.0, 10.0, 5.0, 5.0),
        ];
        let new = vec![
            sig_at(1, 1, 1.0, 1.0, 5.0, 5.0),
            sig_at(2, 0, 10.0, 10.0, 5.0, 5.0),
        ];
        assert!(plan(&old, &new, view()).is_none());
    }

    #[test]
    fn too_much_damage_is_rejected() {
        let old = vec![sig_at(1, 0, 0.0, 0.0, 90.0, 90.0)];
        let mut new = old.clone();
        new[0].key = 2;
        assert!(plan(&old, &new, view()).is_none());
    }

    #[test]
    fn fresh_tiles_damage_both_old_and_new_place() {
        let old = vec![sig_at(1, 0, 1.0, 1.0, 5.0, 5.0)];
        let mut new = vec![sig_at(1, 0, 20.0, 20.0, 5.0, 5.0)];
        new[0].fresh = true;
        let plan = plan(&old, &new, view()).expect("新鲜图元仍可规划 damage");
        // 旧位置要擦掉，新位置要画上：两处不相交的 damage。
        assert_eq!(plan.damage.len(), 2);
    }

    #[test]
    fn area_helpers_round_to_pixels() {
        let a = Area {
            x0: 1.2,
            y0: 2.1,
            x1: 5.7,
            y1: 6.8,
        };
        assert_eq!(
            a.pixels(),
            Area {
                x0: 1.0,
                y0: 2.0,
                x1: 6.0,
                y1: 7.0
            }
        );
        assert!(!a.on_grid());
        assert!(a.union(&view()).on_grid());
    }

    /// 假的绘制器：记录 prepare / begin / draw 的调用次数。
    #[derive(Default)]
    struct CountingDrawer {
        prepares: usize,
        begins: usize,
        draws: usize,
    }

    impl Drawer for CountingDrawer {
        fn clear(&self) -> [f32; 4] {
            [0.0; 4]
        }
        fn prepare(&mut self) -> Result<()> {
            self.prepares += 1;
            Ok(())
        }
        fn begin(
            &mut self,
            _color: &Option<ID3D11RenderTargetView>,
            _depth: &Option<ID3D11DepthStencilView>,
            _clear: [f32; 4],
        ) -> Result<()> {
            self.begins += 1;
            Ok(())
        }
        fn draw(&mut self) -> Result<()> {
            self.draws += 1;
            Ok(())
        }
    }

    fn warp_device() -> Option<(ID3D11Device, ID3D11DeviceContext, ID3D11DeviceContext1)> {
        use windows::Win32::{
            Foundation::HMODULE,
            Graphics::{
                Direct3D::D3D_DRIVER_TYPE_WARP,
                Direct3D11::{
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
                },
            },
        };
        let mut device = None;
        let mut context = None;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_WARP,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        }
        .ok()?;
        let device = device?;
        let context = context?;
        let context1 = context.cast::<ID3D11DeviceContext1>().ok()?;
        Some((device, context, context1))
    }

    #[test]
    fn canvas_control_flow_whole_change_and_no_change() {
        let Some((device, context, context1)) = warp_device() else {
            return;
        };
        let mut canvas = Canvas::new(&device, (8, 8)).expect("canvas 应能创建");
        let mut drawer = CountingDrawer::default();

        // 整帧：没有上一帧签名时 plan=None，begin + draw 各一次。
        canvas
            .make(&device, &context, &context1, None, &mut drawer)
            .expect("整帧应成功");
        assert_eq!((drawer.begins, drawer.draws), (1, 1));
        canvas.last = vec![sig_at(1, 0, 0.0, 0.0, 4.0, 4.0)];
        canvas.clear = [0.0; 4];

        // 无变化的空 plan：不 begin 也不 draw。
        let before = (drawer.begins, drawer.draws);
        let no_change = Plan {
            shift: None,
            damage: Vec::new(),
        };
        canvas
            .make(&device, &context, &context1, Some(&no_change), &mut drawer)
            .expect("无变化帧应成功");
        assert_eq!((drawer.begins, drawer.draws), before);

        // 一个 damage 矩形：begin + draw 各一次（外加 prepare）。
        canvas
            .make(
                &device,
                &context,
                &context1,
                Some(&Plan {
                    shift: None,
                    damage: vec![Area {
                        x0: 0.0,
                        y0: 0.0,
                        x1: 4.0,
                        y1: 4.0,
                    }],
                }),
                &mut drawer,
            )
            .expect("damage 帧应成功");
        assert_eq!(drawer.begins, before.0 + 1);
        assert_eq!(drawer.draws, before.1 + 1);
        assert_eq!(drawer.prepares, 3);
    }
}
