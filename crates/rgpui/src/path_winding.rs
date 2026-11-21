//! 路径的逐像素环绕数填充（Slug 式解析抗锯齿，对齐上游 `ffe0fa29e4`）。
//!
//! [`PathBuilder`](crate::PathBuilder) 把轮廓转成闭合的单调二次曲线链。要绘制
//! [`Path`](crate::Path)，我们在 GPU 上逐像素计算环绕数：从像素中心向左、向上
//! 投射线并统计与路径曲线的交叉（Slug / Eric Lengyel 的做法），半像素内的交叉
//! 按比例计入部分覆盖，再按填充规则换算成覆盖度。
//!
//! 为了减少每条射线需要检测的曲线数量，CPU 侧先把曲线按方形 cell 分桶：每个
//! cell 列出与它相交或临近的曲线。cell 尺寸按路径逐个权衡分桶成本与着色成本
//! （见 `cell_size`）。
//!
//! cell 列表之外的曲线不可能贡献部分覆盖，所以分桶时把它们沿「cell 左上角向左
//! 射线」的环绕数贡献累加成 *backdrop*。每个像素的环绕数从 backdrop 起步，加上
//! 列表曲线的贡献，并对列表曲线端点在参考角与像素之间的部分做修正。
//!
//! 列表与 backdrop 相同的相邻 cell 合并成矩形 [`PathTile`]，上传后由 GPU 绘制。
//! 例如一个大矩形会变成九个瓦片：四角、四边、一个内部。

use crate::{Bounds, PathCurve, PathTile, Point, ScaledPixels, point};
use std::cell::RefCell;

/// 瓦片按「边界外扩这个距离」收集曲线。其余曲线距瓦片至少半像素，不可能贡献
/// 部分覆盖；多出的 1/256 覆盖光栅化器把瓦片边缘对齐到子像素网格的误差。
const MARGIN: f32 = 0.5 + 1.0 / 256.0;

/// 不带 IEEE NaN 语义的 `f32::min` / `max` / `clamp`（标准实现每次调用要多一次
/// 比较与混合，x86-64 基线指令集上代价可观）。
trait FastFloat {
    fn fast_min(self, other: f32) -> f32;
    fn fast_max(self, other: f32) -> f32;
    fn fast_clamp(self, min: f32, max: f32) -> f32;
}

impl FastFloat for f32 {
    #[inline(always)]
    fn fast_min(self, other: f32) -> f32 {
        if self < other { self } else { other }
    }

    #[inline(always)]
    fn fast_max(self, other: f32) -> f32 {
        if self > other { self } else { other }
    }

    #[inline(always)]
    fn fast_clamp(self, min: f32, max: f32) -> f32 {
        self.fast_max(min).fast_min(max)
    }
}

/// 一条路径的曲线按某个显示比例分桶后的结果。
#[derive(Debug, Default)]
pub(crate) struct PathTiles {
    bounds: Bounds<ScaledPixels>,
    tiles: Vec<PathTile>,
    /// 所有瓦片的曲线列表，拼接存储。
    curves: Vec<PathCurve>,
}

impl PathTiles {
    /// 曲线包围盒（设备像素）。
    pub(crate) fn bounds(&self) -> Bounds<ScaledPixels> {
        self.bounds
    }

    /// 涂刷边界：瓦片边界外扩抗锯齿边距。
    pub(crate) fn painted_bounds(&self) -> Bounds<ScaledPixels> {
        if self.tiles.is_empty() {
            return self.bounds;
        }
        self.bounds.dilate(ScaledPixels(MARGIN))
    }

    /// 全部瓦片。
    pub(crate) fn tiles(&self) -> &[PathTile] {
        &self.tiles
    }

    /// 全部曲线（按瓦片列表顺序拼接）。
    pub(crate) fn curves(&self) -> &[PathCurve] {
        &self.curves
    }

    /// 丢弃完全落在 `mask` 之外的瓦片。
    pub(crate) fn retain_visible(&mut self, mask: Bounds<ScaledPixels>) {
        self.tiles.retain(|tile| {
            tile.top_left.x < mask.right().0
                && tile.top_left.y < mask.bottom().0
                && tile.bottom_right.x > mask.origin.x.0
                && tile.bottom_right.y > mask.origin.y.0
        });
    }
}

/// 把曲线按显示比例缩放后分桶成瓦片。`even_odd` 决定填充规则口径。
pub(crate) fn bin(curves: &[PathCurve], even_odd: bool, scale: f32) -> PathTiles {
    debug_assert!(scale > 0.0 && scale.is_finite(), "非法缩放比例 {scale}");
    if curves.is_empty() {
        return PathTiles::default();
    }
    BINNER.with_borrow_mut(|binner| {
        let (min, max, length) = binner.scale(curves, scale);
        let Some(grid) = Grid::covering(min, max, curves.len(), length) else {
            log::warn!("路径跨越 {min:?} 到 {max:?} 设备像素溢出；跳过绘制");
            return PathTiles::default();
        };
        binner.find_spans(&grid);
        binner.fill_cells();
        let (tiles, curves) = binner.merge_cells(&grid, even_odd);
        let device = |p: Point<f32>| point(ScaledPixels(p.x), ScaledPixels(p.y));
        PathTiles {
            bounds: Bounds::from_corners(device(min), device(max)),
            tiles,
            curves,
        }
    })
}

/// 选出使「分桶 + 着色」总耗时估计最小的 2 的幂 cell 尺寸。cell 越小，每条曲线
/// 被列入越多 cell（CPU 成本升高）；越大，shader 要在更多像素上求值每条曲线
/// （GPU 成本升高）。
///
/// 成本模型（以「shader 对一条曲线在一个像素求值一次」为单位）：
///
/// ```text
/// cost = listings * (LISTING_CPU_COST + area) + empty_cells * EMPTY_CELL_CPU_COST
/// ```
fn cell_size(curves: f32, length: f32, size: Point<f32>) -> f32 {
    // 一次列入与一个空 cell 的 CPU 时间（换算成 shader 求值次数）。在台式机
    // （RX 6600）实测约 634 / 207，集成显卡（HD 4600）约 68 / 21；两者之间取
    // 在两台机器上实测总耗时损失最小的值。给「列入了曲线的 cell」计步零成本：
    // 计一步会让虚线选到更大的 cell，省下的 CPU 时间远抵不过多花的 GPU 时间。
    const LISTING_CPU_COST: f32 = 320.0;
    const EMPTY_CELL_CPU_COST: f32 = 95.0;
    // 更小的 cell 省 GPU 时间，但在真实路径上实测省下的 GPU 时间抵不过多花的
    // CPU 时间（两台机器皆然），故设下限。
    const MIN_CELL_SIZE: f32 = 16.0;
    // 当成本模型低估空 cell 数量（例如大部分曲线挤进大路径的少数 cell）时，
    // 限制网格的内存与遍历时间。
    const MAX_GRID_CELLS: f32 = 4096.0;

    let grid_cells = |cell_size: f32| {
        ((size.x / cell_size).floor() + 1.0) * ((size.y / cell_size).floor() + 1.0)
    };
    let extent = size.x.fast_max(size.y);
    let mut cell_size = MIN_CELL_SIZE;
    while grid_cells(cell_size) > MAX_GRID_CELLS {
        cell_size *= 2.0;
    }
    let mut best = (f32::INFINITY, cell_size);
    loop {
        let listings = if cell_size >= extent {
            curves
        } else {
            // 单调曲线每条大约跨过 `(w + h) / cell_size` 条网格线。
            curves + length / cell_size
        };
        let area = cell_size.fast_min(size.x) * cell_size.fast_min(size.y) / 2.0;
        let empty_cells = (grid_cells(cell_size) - listings).fast_max(0.0);
        let cost = listings * (LISTING_CPU_COST + area) + empty_cells * EMPTY_CELL_CPU_COST;
        if cost < best.0 {
            best = (cost, cell_size);
        }
        if cell_size >= extent {
            return best.1;
        }
        cell_size *= 2.0;
    }
}

/// 按填充规则判断环绕数是否填充。
fn fills(winding: i32, even_odd: bool) -> bool {
    if even_odd {
        winding % 2 != 0
    } else {
        winding != 0
    }
}

#[derive(Default)]
struct Binner {
    /// 输入曲线，已缩放到设备像素。
    input: Vec<PathCurve>,
    spans: Vec<Span>,
    /// 每个 cell 的列表在 `keys` 里的结束位置。分桶期间复用（计数排序思路）：
    /// `find_spans` 先在这里存每 cell 的计数，`fill_cells` 把计数转成起点并在
    /// 填充时前移。
    cell_ends: Vec<usize>,
    /// 每个 cell 的 backdrop 步进：跨越该 cell 所在行顶、位于其左侧、且未被
    /// 列入的曲线的贡献。
    steps: Vec<i32>,
    /// 每个 cell 的列表项：曲线最左 x 的高位 + 曲线下标。
    keys: Vec<u64>,
    /// 当前行的一串 run。
    runs: Vec<Run>,
    /// 上一行的 run（当前行可以向下延伸它们的瓦片）。
    above: Vec<Run>,
}

thread_local! {
    static BINNER: RefCell<Binner> = Default::default();
}

/// 一条曲线在某一行内连续占据的 cell 区段。
struct Span {
    cell: usize,
    count: usize,
    key: u64,
}

impl Binner {
    /// 把曲线缩放到设备像素，返回包围盒与总长度（用于 cell 尺寸成本模型）。
    fn scale(&mut self, curves: &[PathCurve], scale: f32) -> (Point<f32>, Point<f32>, f32) {
        let mut min = point(f32::INFINITY, f32::INFINITY);
        let mut max = point(f32::NEG_INFINITY, f32::NEG_INFINITY);
        let mut length = 0.0;
        self.input.clear();
        self.input.extend(curves.iter().map(|curve| {
            let curve = curve.scaled(scale);
            // 单调曲线的包围盒由端点即可确定。
            min.x = min.x.fast_min(curve.min_x());
            max.x = max.x.fast_max(curve.max_x());
            min.y = min.y.fast_min(curve.p0.y);
            max.y = max.y.fast_max(curve.p1.y);
            length += curve.max_x() - curve.min_x() + curve.p1.y - curve.p0.y;
            curve
        }));
        (min, max, length)
    }

    /// 第一遍：数出每个 cell 的列表长度，并在列表区段右侧第一个 cell 记录
    /// 跨越行顶的曲线对 backdrop 的步进。
    fn find_spans(&mut self, grid: &Grid) {
        let cells = grid.rows * grid.columns;
        self.spans.clear();
        self.cell_ends.clear();
        self.cell_ends.resize(cells, 0);
        self.steps.clear();
        self.steps.resize(cells, 0);
        for (index, curve) in self.input.iter().enumerate() {
            // key 先按最左 x 排（shader 早退依赖），再按下标排（保证全序）。
            let key = u64::from(ordered_bits(curve.min_x())) << 32 | index as u64;
            for row in grid.row_of(curve.p0.y - MARGIN)..=grid.row_of(curve.p1.y + MARGIN) {
                let row_top = grid.y(row);
                let (low, high) = curve.x_range(row_top - MARGIN, grid.y(row + 1) + MARGIN);
                let first = grid.column_of(low - MARGIN);
                let last = grid.column_of(high + MARGIN);
                let cell = row * grid.columns + first;
                let count = last + 1 - first;
                self.spans.push(Span { cell, count, key });
                for listings in self
                    .cell_ends
                    .get_mut(cell..cell + count)
                    .unwrap_or_default()
                {
                    *listings += 1;
                }

                // 若该曲线对 cell 的 backdrop 有贡献，把贡献记在其区段右侧的
                // 第一个 cell 上。
                if curve.crosses(row_top)
                    && last + 1 < grid.columns
                    && let Some(step) = self.steps.get_mut(cell + count)
                {
                    *step += curve.step();
                }
            }
        }
    }

    /// 第二遍：把计数转成起点（计数排序），按起点填入列表并逐 cell 排序。
    fn fill_cells(&mut self) {
        let mut total = 0;
        for end in self.cell_ends.iter_mut() {
            let count = *end;
            *end = total;
            total += count;
        }
        self.keys.clear();
        self.keys.resize(total, 0);
        for span in &self.spans {
            for cursor in self
                .cell_ends
                .get_mut(span.cell..span.cell + span.count)
                .unwrap_or_default()
            {
                if let Some(key) = self.keys.get_mut(*cursor) {
                    *key = span.key;
                }
                *cursor += 1;
            }
        }
        let mut start = 0;
        for &end in &self.cell_ends {
            self.keys
                .get_mut(start..end)
                .unwrap_or_default()
                .sort_unstable();
            start = end;
        }
    }

    /// 第三遍：逐 cell 算出涂刷矩形，同一行内同色相邻 cell 合并成 run，
    /// 上下行同列表同 backdrop 的 run 向下延伸同一瓦片。
    fn merge_cells(&mut self, grid: &Grid, even_odd: bool) -> (Vec<PathTile>, Vec<PathCurve>) {
        let Binner {
            input,
            steps,
            cell_ends,
            keys,
            runs,
            above,
            ..
        } = self;
        let mut tiles: Vec<PathTile> = Vec::new();
        // 每个列表项至多复制一次。
        let mut curves = Vec::with_capacity(keys.len());
        above.clear();
        let mut cell_start = 0;
        for row in 0..grid.rows {
            runs.clear();
            let mut backdrop = 0;
            for column in 0..grid.columns {
                let index = row * grid.columns + column;
                backdrop += steps.get(index).copied().unwrap_or(0);
                let cell_end = cell_ends.get(index).copied().unwrap_or(cell_start);
                let list = (cell_start, cell_end);
                cell_start = cell_end;

                let cell = grid.cell(column, row);
                let quad = if list.0 == list.1 {
                    fills(backdrop, even_odd).then(|| (cell.top_left(), cell.bottom_right()))
                } else {
                    let mut measure = Measure::new(cell, backdrop);
                    for curve in listed(keys, input, list) {
                        measure.add(curve);
                    }
                    measure.quad(even_odd)
                };
                let Some((top_left, bottom_right)) = quad else {
                    continue;
                };
                let run = Run {
                    start: column,
                    end: column + 1,
                    backdrop,
                    list,
                    top_left,
                    bottom_right,
                    tile: 0,
                };
                match runs.last_mut() {
                    Some(last) if last.end == run.start && last.shades_like(&run, keys) => {
                        last.end = run.end;
                        extend_quad(&mut last.top_left, &mut last.bottom_right, &run);
                    }
                    _ => runs.push(run),
                }
            }

            // 两行的 run 都按列序排列。
            let mut above_runs = above.iter().peekable();
            for run in runs.iter_mut() {
                while above_runs
                    .next_if(|other| other.start < run.start)
                    .is_some()
                {}
                let matching = above_runs.next_if(|other| {
                    other.start == run.start && other.end == run.end && other.shades_like(run, keys)
                });
                run.tile = match matching {
                    Some(above_run) => {
                        if let Some(tile) = tiles.get_mut(above_run.tile) {
                            extend_quad(&mut tile.top_left, &mut tile.bottom_right, run);
                        }
                        above_run.tile
                    }
                    None => {
                        let curve_start = curves.len();
                        // 瓦片按曲线范围裁剪，因此可以断定矩形之外的像素不被填充。
                        let mut curves_top_left = point(f32::INFINITY, f32::INFINITY);
                        let mut curves_bottom_right = point(f32::NEG_INFINITY, f32::NEG_INFINITY);
                        for curve in listed(keys, input, run.list) {
                            // 光栅化器会把瓦片边缘对齐到子像素网格，可能涂到瓦片
                            // 外一像素；裁剪处那一像素同时也在矩形外，会被误填。
                            // 把矩形外扩超过最大对齐误差防止这种情况。
                            const BOX_MARGIN: f32 = MARGIN + 1.0 / 16.0;

                            curves_top_left.x =
                                curves_top_left.x.fast_min(curve.min_x() - BOX_MARGIN);
                            curves_top_left.y = curves_top_left.y.fast_min(curve.p0.y - BOX_MARGIN);
                            curves_bottom_right.x =
                                curves_bottom_right.x.fast_max(curve.max_x() + BOX_MARGIN);
                            curves_bottom_right.y =
                                curves_bottom_right.y.fast_max(curve.p1.y + BOX_MARGIN);
                            curves.push(*curve);
                        }
                        tiles.push(PathTile {
                            paint: 0,
                            curve_start: curve_start as u32,
                            curve_count: (curves.len() - curve_start) as u32,
                            top_left: run.top_left,
                            bottom_right: run.bottom_right,
                            reference: grid.cell(run.start, row).top_left(),
                            backdrop: run.backdrop,
                            curves_top_left,
                            curves_bottom_right,
                        });
                        tiles.len() - 1
                    }
                };
            }
            std::mem::swap(above, runs);
        }
        (tiles, curves)
    }
}

/// 把有限浮点数按大小序映射为整数。
fn ordered_bits(x: f32) -> u32 {
    let bits = x.to_bits();
    if bits >> 31 == 1 {
        !bits
    } else {
        bits | 1 << 31
    }
}

/// 一行内列区间 `start..end` 上、着色完全相同的连续 cell。
#[derive(Clone, Copy)]
struct Run {
    start: usize,
    end: usize,
    backdrop: i32,
    /// cell 的列表，在 `keys` 中的区间。
    list: (usize, usize),
    top_left: Point<f32>,
    bottom_right: Point<f32>,
    tile: usize,
}

impl Run {
    /// 两者 backdrop 相同且列表曲线一致。
    fn shades_like(&self, other: &Run, keys: &[u64]) -> bool {
        self.backdrop == other.backdrop
            && keys.get(self.list.0..self.list.1) == keys.get(other.list.0..other.list.1)
    }
}

/// 把 run 的涂刷矩形并入目标矩形。
fn extend_quad(top_left: &mut Point<f32>, bottom_right: &mut Point<f32>, run: &Run) {
    top_left.x = top_left.x.fast_min(run.top_left.x);
    top_left.y = top_left.y.fast_min(run.top_left.y);
    bottom_right.x = bottom_right.x.fast_max(run.bottom_right.x);
    bottom_right.y = bottom_right.y.fast_max(run.bottom_right.y);
}

/// 按 `keys` 区间取出曲线列表。
fn listed<'a>(
    keys: &'a [u64],
    input: &'a [PathCurve],
    list: (usize, usize),
) -> impl Iterator<Item = &'a PathCurve> {
    keys.get(list.0..list.1)
        .unwrap_or_default()
        .iter()
        .filter_map(move |&key| input.get(key as u32 as usize))
}

/// 分桶网格。
struct Grid {
    left: f32,
    top: f32,
    cell_size: f32,
    columns: usize,
    rows: usize,
}

impl Grid {
    /// 覆盖 `[min, max]` 的网格；包围盒溢出时返回 `None`。
    fn covering(min: Point<f32>, max: Point<f32>, curves: usize, length: f32) -> Option<Self> {
        let left = (min.x - MARGIN).floor();
        let top = (min.y - MARGIN).floor();
        let size = point(max.x + MARGIN - left, max.y + MARGIN - top);
        if !(size.x.is_finite() && size.y.is_finite()) {
            return None;
        }
        let cell_size = cell_size(curves as f32, length, size);
        Some(Self {
            left,
            top,
            cell_size,
            columns: (size.x / cell_size) as usize + 1,
            rows: (size.y / cell_size) as usize + 1,
        })
    }

    fn column_of(&self, x: f32) -> usize {
        let column = (x - self.left) / self.cell_size;
        column.fast_clamp(0.0, (self.columns - 1) as f32) as u32 as usize
    }

    fn row_of(&self, y: f32) -> usize {
        let row = (y - self.top) / self.cell_size;
        row.fast_clamp(0.0, (self.rows - 1) as f32) as u32 as usize
    }

    fn x(&self, column: usize) -> f32 {
        self.left + column as f32 * self.cell_size
    }

    fn y(&self, row: usize) -> f32 {
        self.top + row as f32 * self.cell_size
    }

    fn cell(&self, column: usize, row: usize) -> Cell {
        Cell {
            left: self.x(column),
            top: self.y(row),
            right: self.x(column + 1),
            bottom: self.y(row + 1),
        }
    }
}

#[derive(Clone, Copy)]
struct Cell {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl Cell {
    fn top_left(self) -> Point<f32> {
        point(self.left, self.top)
    }

    fn bottom_right(self) -> Point<f32> {
        point(self.right, self.bottom)
    }
}

/// 累计 cell 内曲线的边界与环绕数，用来裁剪 cell 的涂刷矩形。
struct Measure {
    cell: Cell,
    backdrop: i32,
    min: Point<f32>,
    max: Point<f32>,
    /// 曲线左方与上方之外没有修正，环绕数即 backdrop。所有曲线右侧是
    /// backdrop 加上跨越 cell 顶的曲线；所有曲线下方是按 cell 左缘端点
    /// 修正后的 backdrop。
    right_winding: i32,
    bottom_winding: i32,
}

impl Measure {
    fn new(cell: Cell, backdrop: i32) -> Self {
        Self {
            cell,
            backdrop,
            min: point(f32::INFINITY, f32::INFINITY),
            max: point(f32::NEG_INFINITY, f32::NEG_INFINITY),
            right_winding: backdrop,
            bottom_winding: backdrop,
        }
    }

    #[inline(always)]
    fn add(&mut self, curve: &PathCurve) {
        let cell = self.cell;
        self.min.x = self.min.x.fast_min(curve.min_x());
        self.max.x = self.max.x.fast_max(curve.max_x());
        self.min.y = self.min.y.fast_min(curve.p0.y);
        self.max.y = self.max.y.fast_max(curve.p1.y);
        if curve.crosses(cell.top) {
            self.right_winding += curve.step();
        }
        for (endpoint, step) in curve.endpoints() {
            if endpoint.x <= cell.left && endpoint.y > cell.top {
                self.bottom_winding -= step;
            }
        }
    }

    /// 涂刷矩形的左上与右下角（若无则 `None`）：即 cell 按曲线范围收边——
    /// 收边之外由未列入曲线的环绕数保证不填充。
    #[inline(always)]
    fn quad(&self, even_odd: bool) -> Option<(Point<f32>, Point<f32>)> {
        let cell = self.cell;
        let clamp_x = |x: f32| x.fast_clamp(cell.left, cell.right);
        let clamp_y = |y: f32| y.fast_clamp(cell.top, cell.bottom);
        let unfilled = !fills(self.backdrop, even_odd);
        let left = if unfilled {
            clamp_x(self.min.x - MARGIN)
        } else {
            cell.left
        };
        let top = if unfilled {
            clamp_y(self.min.y - MARGIN)
        } else {
            cell.top
        };
        let right = if fills(self.right_winding, even_odd) {
            cell.right
        } else {
            clamp_x(self.max.x + MARGIN)
        };
        let bottom = if fills(self.bottom_winding, even_odd) {
            cell.bottom
        } else {
            clamp_y(self.max.y + MARGIN)
        };
        (left < right && top < bottom).then(|| (point(left, top), point(right, bottom)))
    }
}

impl PathCurve {
    fn scaled(&self, scale: f32) -> Self {
        let scale_point = |p: Point<f32>| point(p.x * scale, p.y * scale);
        Self {
            p0: scale_point(self.p0),
            p1: scale_point(self.p1),
            a: scale_point(self.a),
            b: scale_point(self.b),
            sign: self.sign,
            pad: 0,
        }
    }

    fn min_x(&self) -> f32 {
        self.p0.x.fast_min(self.p1.x)
    }

    fn max_x(&self) -> f32 {
        self.p0.x.fast_max(self.p1.x)
    }

    fn step(&self) -> i32 {
        if self.sign < 0.0 { -1 } else { 1 }
    }

    /// 曲线是否跨越水平线 `y`。半开区间口径：两条曲线在同一线上相接时恰有
    /// 一条计入跨越。
    fn crosses(&self, y: f32) -> bool {
        self.p0.y <= y && y < self.p1.y
    }

    /// 每个端点及其在 shader 侧的修正步进。
    fn endpoints(&self) -> [(Point<f32>, i32); 2] {
        [(self.p0, self.step()), (self.p1, -self.step())]
    }

    /// 曲线在两个高度之间的 x 范围。
    fn x_range(&self, top: f32, bottom: f32) -> (f32, f32) {
        let top = top.fast_max(self.p0.y);
        let bottom = bottom.fast_min(self.p1.y);
        let x_top = if top <= self.p0.y {
            self.p0.x
        } else {
            self.x_at_y(top)
        };
        let x_bottom = if bottom >= self.p1.y {
            self.p1.x
        } else {
            self.x_at_y(bottom)
        };
        // 收敛到曲线自身范围，防止舍入把范围带出去。
        (
            x_top.fast_min(x_bottom).fast_max(self.min_x()),
            x_top.fast_max(x_bottom).fast_min(self.max_x()),
        )
    }

    fn x_at_y(&self, y: f32) -> f32 {
        if self.a.y == 0.0 {
            if self.b.y == 0.0 {
                return self.p0.x;
            }
            let t = ((y - self.p0.y) / self.b.y).fast_clamp(0.0, 1.0);
            return (self.a.x * t + self.b.x) * t + self.p0.x;
        }
        let t = monotone_quadratic_root(self.a.y, self.b.y, self.p0.y - y, 1.0);
        (self.a.x * t + self.b.x) * t + self.p0.x
    }
}

/// 解 a*t² + b*t + c = 0 在单调段上位于 [0, 1] 的根。
fn monotone_quadratic_root(a: f32, b: f32, c: f32, direction: f32) -> f32 {
    let discriminant = (b * b - 4.0 * a * c).fast_max(0.0);
    let denominator = b + direction * discriminant.sqrt();
    if denominator == 0.0 {
        return 0.0;
    }
    (-2.0 * c / denominator).fast_clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContentMask, Outline, PaintedPath, Path, PathBuilder, Scene, black, px, size};

    /// 跨越多个 cell 的矩形总是九个瓦片：四条边带、四个角、一个内部。
    #[test]
    fn rectangle_is_nine_tiles() {
        let path = square(100., 40., 900., 600.);
        assert_eq!(path.bin(1.25).tiles().len(), 9);
    }

    /// 远大于窗口的路径按粗 cell 分桶，绘制时只保留 clip 可见的瓦片。
    #[test]
    fn huge_path_paints_only_visible_tiles() {
        let path = square(-20000., -20000., 20000., 20000.);
        let tile_count = path.bin(1.5).tiles().len();
        assert!(tile_count < 10_000, "{tile_count} tiles");

        let mask = Bounds {
            origin: point(px(300.), px(200.)),
            size: size(px(120.), px(90.)),
        };
        let mut scene = Scene::default();
        scene.insert_primitive(PaintedPath::new(
            path,
            1.5,
            ContentMask { bounds: mask },
            black().into(),
        ));
        scene.finish();
        let painted: usize = scene.paths.iter().map(PaintedPath::tile_count).sum();
        assert!(painted > 0 && painted < 20, "{painted} tiles");
    }

    /// 在某个比例或任何比例下尺寸溢出的路径被跳过（不炸、不绘制）。
    #[test]
    fn overflowing_paths_are_skipped() {
        let radius = f32::MAX / 4.0;
        let path = square(-radius, -radius, radius, radius);
        assert!(!path.bin(1.0).tiles().is_empty());
        assert!(path.bin(4.0).tiles().is_empty());
        let radius = f32::MAX * 0.75;
        let path = square(-radius, -radius, radius, radius);
        assert!(path.bin(1.0).tiles().is_empty());
    }

    /// 含非有限坐标点的整条路径构建失败。只丢弃所在线段会留下开口轮廓。
    #[test]
    fn non_finite_point_fails_path() {
        // 直接构建 Outline：lyon 在 debug 构建下断言坐标有限。
        let mut outline = Outline::default();
        outline.move_to(point(0., 0.));
        outline.line_to(point(10., 0.));
        outline.line_to(point(10., 10.));
        outline.move_to(point(20., 0.));
        outline.line_to(point(f32::NAN, 5.));
        outline.line_to(point(30., 10.));
        assert!(outline.into_curves().is_err());
    }

    /// shader 的 backdrop 修正要求相接曲线共享位级相同的端点。断点表现为
    /// 条纹而非失败，所以直接校验。
    #[test]
    fn endpoints_balance() {
        let mut curved_fill = PathBuilder::fill();
        curved_fill.move_to(point(px(450.3), px(280.7)));
        curved_fill.curve_to(point(px(530.3), px(230.7)), point(px(450.3), px(230.7)));
        curved_fill.arc_to(
            point(px(30.), px(30.)),
            px(0.),
            false,
            false,
            point(px(590.3), px(230.7)),
        );
        curved_fill.close();

        let mut stroke = PathBuilder::stroke(px(2.));
        stroke.move_to(point(px(80.3), px(500.7)));
        stroke.cubic_bezier_to(
            point(px(900.), px(250.)),
            point(px(300.), px(50.)),
            point(px(700.), px(800.)),
        );

        let mut dashed = PathBuilder::stroke(px(3.)).dash_array(&[px(9.), px(4.)]);
        dashed.move_to(point(px(50.3), px(10.7)));
        dashed.arc_to(
            point(px(40.), px(40.)),
            px(0.),
            false,
            true,
            point(px(90.3), px(50.7)),
        );

        for (name, builder) in [
            ("curved fill", curved_fill),
            ("stroke", stroke),
            ("dashed stroke", dashed),
        ] {
            let (curves, _) = builder.outline().unwrap();
            let mut balance = std::collections::HashMap::<(u32, u32), i32>::default();
            for curve in &curves {
                for (endpoint, step) in curve.endpoints() {
                    *balance
                        .entry((endpoint.x.to_bits(), endpoint.y.to_bits()))
                        .or_default() += step;
                }
            }
            assert!(!balance.is_empty(), "{name} 没有产出曲线");
            for ((x, y), sum) in balance {
                assert_eq!(
                    sum,
                    0,
                    "{name}: 端点 ({}, {}) 不平衡",
                    f32::from_bits(x),
                    f32::from_bits(y),
                );
            }
        }
    }

    fn square(left: f32, top: f32, right: f32, bottom: f32) -> Path {
        let mut builder = PathBuilder::fill();
        let corners = [(left, top), (right, top), (right, bottom), (left, bottom)];
        let points: Vec<crate::Point<crate::Pixels>> =
            corners.iter().map(|&(x, y)| point(px(x), px(y))).collect();
        builder.add_polygon(&points, true);
        builder.build().unwrap()
    }
}
