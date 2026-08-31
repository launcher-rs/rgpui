//! RGPUI 几何模块，包含用于描述常用单位、概念及其关系的类型和 trait。

use crate::refineable::Refineable;
use anyhow::{Context as _, anyhow};
use core::fmt::Debug;
use derive_more::{Add, AddAssign, Div, DivAssign, Mul, Neg, Sub, SubAssign};
use schemars::{JsonSchema, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::borrow::Cow;
use std::ops::{AddAssign, Range};
use std::{
    cmp::{self, PartialOrd},
    fmt::{self, Display},
    hash::Hash,
    ops::{Add, Div, Mul, MulAssign, Neg, Sub},
};
use taffy::prelude::{TaffyGridLine, TaffyGridSpan};

use crate::{App, DisplayId};

/// 二维笛卡尔空间中的轴。
#[derive(Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum Axis {
    /// Y 轴，即垂直方向（上下）
    Vertical,
    /// X 轴，即水平方向（左右）
    Horizontal,
}

impl Axis {
    /// 将当前轴切换到相反的轴。
    pub fn invert(self) -> Self {
        match self {
            Axis::Vertical => Axis::Horizontal,
            Axis::Horizontal => Axis::Vertical,
        }
    }
}

/// 沿特定轴访问对应单位的 trait。
pub trait Along {
    /// 与该类型关联的单位类型
    type Unit;

    /// 返回沿给定轴的单位值。
    fn along(&self, axis: Axis) -> Self::Unit;

    /// 对沿给定轴的单位应用给定函数，并返回新值。
    fn apply_along(&self, axis: Axis, f: impl FnOnce(Self::Unit) -> Self::Unit) -> Self;
}

/// 描述二维笛卡尔空间中的一个位置。
///
/// 它包含两个公共字段 `x` 和 `y`，表示空间中的坐标。
/// 坐标类型 `T` 可以是任何实现了 `Default`、`Clone` 和 `Debug` 的类型。
///
/// # 示例
///
/// ```
/// # use rgpui::Point;
/// let point = Point { x: 10, y: 20 };
/// println!("{:?}", point); // 输出: Point { x: 10, y: 20 }
/// ```
#[derive(
    Refineable,
    Default,
    Add,
    AddAssign,
    Sub,
    SubAssign,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema,
    Hash,
    Neg,
)]
#[refineable(Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[repr(C)]
pub struct Point<T: Clone + Debug + Default + PartialEq> {
    /// 点的 x 坐标。
    pub x: T,
    /// 点的 y 坐标。
    pub y: T,
}

/// 使用给定的 x 和 y 坐标构造一个新的 `Point<T>`。
///
/// # 参数
///
/// * `x` - 点的 x 坐标。
/// * `y` - 点的 y 坐标。
///
/// # 返回值
///
/// 返回一个具有指定坐标的 `Point<T>`。
///
/// # 示例
///
/// ```
/// use rgpui::point;
/// let p = point(10, 20);
/// assert_eq!(p.x, 10);
/// assert_eq!(p.y, 20);
/// ```
pub const fn point<T: Clone + Debug + Default + PartialEq>(x: T, y: T) -> Point<T> {
    Point { x, y }
}

impl<T: Clone + Debug + Default + PartialEq> Point<T> {
    /// 使用指定的 `x` 和 `y` 坐标创建一个新的 `Point`。
    ///
    /// # 参数
    ///
    /// * `x` - 点的水平坐标。
    /// * `y` - 点的垂直坐标。
    ///
    /// # 示例
    ///
    /// ```
    /// use rgpui::Point;
    /// let p = Point::new(10, 20);
    /// assert_eq!(p.x, 10);
    /// assert_eq!(p.y, 20);
    /// ```
    pub const fn new(x: T, y: T) -> Self {
        Self { x, y }
    }

    /// 通过对两个坐标应用给定函数，将点转换为 `Point<U>`。
    ///
    /// 此方法允许通过指定闭包将 `Point<T>` 转换为 `Point<U>`，闭包定义了两种类型之间的转换方式。
    /// 闭包应用于 `x` 和 `y` 坐标，生成所需类型的新点。
    ///
    /// # 参数
    ///
    /// * `f` - 接受类型 `T` 的值并返回类型 `U` 的值的闭包。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::Point;
    /// let p = Point { x: 3, y: 4 };
    /// let p_float = p.map(|coord| coord as f32);
    /// assert_eq!(p_float, Point { x: 3.0, y: 4.0 });
    /// ```
    #[must_use]
    pub fn map<U: Clone + Debug + Default + PartialEq>(&self, f: impl Fn(T) -> U) -> Point<U> {
        Point {
            x: f(self.x.clone()),
            y: f(self.y.clone()),
        }
    }
}

impl<T: Clone + Debug + Default + PartialEq> Along for Point<T> {
    type Unit = T;

    fn along(&self, axis: Axis) -> T {
        match axis {
            Axis::Horizontal => self.x.clone(),
            Axis::Vertical => self.y.clone(),
        }
    }

    fn apply_along(&self, axis: Axis, f: impl FnOnce(T) -> T) -> Point<T> {
        match axis {
            Axis::Horizontal => Point {
                x: f(self.x.clone()),
                y: self.y.clone(),
            },
            Axis::Vertical => Point {
                x: self.x.clone(),
                y: f(self.y.clone()),
            },
        }
    }
}

impl Point<Pixels> {
    /// 按给定因子缩放点，通常用于根据目标显示器的分辨率调整 UI 元素的大小。
    ///
    /// # 参数
    ///
    /// * `factor` - 应用于 x 和 y 坐标的缩放因子。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Point, Pixels, ScaledPixels};
    /// let p = Point { x: Pixels::from(10.0), y: Pixels::from(20.0) };
    /// let scaled_p = p.scale(1.5);
    /// assert_eq!(scaled_p, Point { x: ScaledPixels::from(15.0), y: ScaledPixels::from(30.0) });
    /// ```
    pub fn scale(&self, factor: f32) -> Point<ScaledPixels> {
        Point {
            x: self.x.scale(factor),
            y: self.y.scale(factor),
        }
    }

    /// 计算从原点 (0, 0) 到该点的欧几里得距离。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Pixels, Point};
    /// let p = Point { x: Pixels::from(3.0), y: Pixels::from(4.0) };
    /// assert_eq!(p.magnitude(), 5.0);
    /// ```
    pub fn magnitude(&self) -> f64 {
        ((self.x.0.powi(2) + self.y.0.powi(2)) as f64).sqrt()
    }
}

impl<T> Point<T>
where
    T: Sub<T, Output = T> + Clone + Debug + Default + PartialEq,
{
    /// 获取相对于给定原点的点位置
    pub fn relative_to(&self, origin: &Point<T>) -> Point<T> {
        point(
            self.x.clone() - origin.x.clone(),
            self.y.clone() - origin.y.clone(),
        )
    }
}

impl<T, Rhs> Mul<Rhs> for Point<T>
where
    T: Mul<Rhs, Output = T> + Clone + Debug + Default + PartialEq,
    Rhs: Clone + Debug,
{
    type Output = Point<T>;

    fn mul(self, rhs: Rhs) -> Self::Output {
        Point {
            x: self.x * rhs.clone(),
            y: self.y * rhs,
        }
    }
}

impl<T, S> MulAssign<S> for Point<T>
where
    T: Mul<S, Output = T> + Clone + Debug + Default + PartialEq,
    S: Clone,
{
    fn mul_assign(&mut self, rhs: S) {
        self.x = self.x.clone() * rhs.clone();
        self.y = self.y.clone() * rhs;
    }
}

impl<T, S> Div<S> for Point<T>
where
    T: Div<S, Output = T> + Clone + Debug + Default + PartialEq,
    S: Clone,
{
    type Output = Self;

    fn div(self, rhs: S) -> Self::Output {
        Self {
            x: self.x / rhs.clone(),
            y: self.y / rhs,
        }
    }
}

impl<T> Point<T>
where
    T: PartialOrd + Clone + Debug + Default + PartialEq,
{
    /// 返回 `self` 和 `other` 中每个维度的最大值组成的新点。
    ///
    /// # 参数
    ///
    /// * `other` - 要与 `self` 比较的另一个 `Point` 的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::Point;
    /// let p1 = Point { x: 3, y: 7 };
    /// let p2 = Point { x: 5, y: 2 };
    /// let max_point = p1.max(&p2);
    /// assert_eq!(max_point, Point { x: 5, y: 7 });
    /// ```
    pub fn max(&self, other: &Self) -> Self {
        Point {
            x: if self.x > other.x {
                self.x.clone()
            } else {
                other.x.clone()
            },
            y: if self.y > other.y {
                self.y.clone()
            } else {
                other.y.clone()
            },
        }
    }

    /// 返回 `self` 和 `other` 中每个维度的最小值组成的新点。
    ///
    /// # 参数
    ///
    /// * `other` - 要与 `self` 比较的另一个 `Point` 的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::Point;
    /// let p1 = Point { x: 3, y: 7 };
    /// let p2 = Point { x: 5, y: 2 };
    /// let min_point = p1.min(&p2);
    /// assert_eq!(min_point, Point { x: 3, y: 2 });
    /// ```
    pub fn min(&self, other: &Self) -> Self {
        Point {
            x: if self.x <= other.x {
                self.x.clone()
            } else {
                other.x.clone()
            },
            y: if self.y <= other.y {
                self.y.clone()
            } else {
                other.y.clone()
            },
        }
    }

    /// 将点限制在指定范围内。
    ///
    /// 给定最小点和最大点，此方法约束当前点的坐标使其不超过最小点和最大点定义的范围。
    /// 如果当前点的坐标小于最小值，则设为最小值；如果大于最大值，则设为最大值。
    ///
    /// # 参数
    ///
    /// * `min` - 表示最小允许坐标的 `Point` 的引用。
    /// * `max` - 表示最大允许坐标的 `Point` 的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::Point;
    /// let p = Point { x: 10, y: 20 };
    /// let min = Point { x: 0, y: 5 };
    /// let max = Point { x: 15, y: 25 };
    /// let clamped_p = p.clamp(&min, &max);
    /// assert_eq!(clamped_p, Point { x: 10, y: 20 });
    ///
    /// let p_out_of_bounds = Point { x: -5, y: 30 };
    /// let clamped_p_out_of_bounds = p_out_of_bounds.clamp(&min, &max);
    /// assert_eq!(clamped_p_out_of_bounds, Point { x: 0, y: 25 });
    /// ```
    pub fn clamp(&self, min: &Self, max: &Self) -> Self {
        self.max(min).min(max)
    }
}

impl<T: Clone + Debug + Default + PartialEq> Clone for Point<T> {
    fn clone(&self) -> Self {
        Self {
            x: self.x.clone(),
            y: self.y.clone(),
        }
    }
}

impl<T: Clone + Debug + Default + PartialEq + Display> Display for Point<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

/// 表示二维空间中具有宽度和高度的尺寸结构。
///
/// 此结构体对类型 `T` 是泛型的，`T` 可以是任何实现了 `Clone`、`Default` 和 `Debug` 的类型。
/// 通常用于指定 UI 元素（如窗口或元素）的尺寸。
#[derive(
    Add, Clone, Copy, Default, Deserialize, Div, Hash, Neg, PartialEq, Refineable, Serialize, Sub,
)]
#[refineable(Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[repr(C)]
pub struct Size<T: Clone + Debug + Default + PartialEq> {
    /// 尺寸的宽度分量。
    pub width: T,
    /// 尺寸的高度分量。
    pub height: T,
}

impl<T: Clone + Debug + Default + PartialEq> Size<T> {
    /// 创建一个新的 Size，是 [`size`] 的同义方法
    pub fn new(width: T, height: T) -> Self {
        size(width, height)
    }
}

/// 使用给定的宽度和高度构造一个新的 `Size<T>`。
///
/// # 参数
///
/// * `width` - `Size` 的宽度分量。
/// * `height` - `Size` 的高度分量。
///
/// # 示例
///
/// ```
/// use rgpui::size;
/// let my_size = size(10, 20);
/// assert_eq!(my_size.width, 10);
/// assert_eq!(my_size.height, 20);
/// ```
pub const fn size<T>(width: T, height: T) -> Size<T>
where
    T: Clone + Debug + Default + PartialEq,
{
    Size { width, height }
}

impl<T> Size<T>
where
    T: Clone + Debug + Default + PartialEq,
{
    /// 对尺寸的宽度和高度应用函数，生成新的 `Size<U>`。
    ///
    /// 此方法允许通过指定闭包将 `Size<T>` 转换为 `Size<U>`，闭包定义了两种类型之间的转换方式。
    /// 闭包应用于 `width` 和 `height`，生成所需类型的新尺寸。
    ///
    /// # 参数
    ///
    /// * `f` - 接受类型 `T` 的值并返回类型 `U` 的值的闭包。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::Size;
    /// let my_size = Size { width: 10, height: 20 };
    /// let my_new_size = my_size.map(|dimension| dimension as f32 * 1.5);
    /// assert_eq!(my_new_size, Size { width: 15.0, height: 30.0 });
    /// ```
    pub fn map<U>(&self, f: impl Fn(T) -> U) -> Size<U>
    where
        U: Clone + Debug + Default + PartialEq,
    {
        Size {
            width: f(self.width.clone()),
            height: f(self.height.clone()),
        }
    }
}

impl<T> Size<T>
where
    T: Clone + Debug + Default + PartialEq + Half,
{
    /// 计算尺寸的中心点。
    pub fn center(&self) -> Point<T> {
        Point {
            x: self.width.half(),
            y: self.height.half(),
        }
    }
}

impl Size<Pixels> {
    /// 按给定因子缩放尺寸。
    ///
    /// 此方法将宽度和高度乘以提供的缩放因子，生成一个新的 `Size<ScaledPixels>`，
    /// 根据因子按比例放大或缩小。
    ///
    /// # 参数
    ///
    /// * `factor` - 应用于宽度和高度的缩放因子。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Size, Pixels, ScaledPixels};
    /// let size = Size { width: Pixels::from(100.0), height: Pixels::from(50.0) };
    /// let scaled_size = size.scale(2.0);
    /// assert_eq!(scaled_size, Size { width: ScaledPixels::from(200.0), height: ScaledPixels::from(100.0) });
    /// ```
    pub fn scale(&self, factor: f32) -> Size<ScaledPixels> {
        Size {
            width: self.width.scale(factor),
            height: self.height.scale(factor),
        }
    }
}

impl<T> Along for Size<T>
where
    T: Clone + Debug + Default + PartialEq,
{
    type Unit = T;

    fn along(&self, axis: Axis) -> T {
        match axis {
            Axis::Horizontal => self.width.clone(),
            Axis::Vertical => self.height.clone(),
        }
    }

    /// 返回沿给定轴的尺寸值。
    fn apply_along(&self, axis: Axis, f: impl FnOnce(T) -> T) -> Self {
        match axis {
            Axis::Horizontal => Size {
                width: f(self.width.clone()),
                height: self.height.clone(),
            },
            Axis::Vertical => Size {
                width: self.width.clone(),
                height: f(self.height.clone()),
            },
        }
    }
}

impl<T> Size<T>
where
    T: PartialOrd + Clone + Debug + Default + PartialEq,
{
    /// 返回 `self` 和 `other` 中宽度和高度的最大值组成的新 `Size`。
    ///
    /// # 参数
    ///
    /// * `other` - 要与 `self` 比较的另一个 `Size` 的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::Size;
    /// let size1 = Size { width: 30, height: 40 };
    /// let size2 = Size { width: 50, height: 20 };
    /// let max_size = size1.max(&size2);
    /// assert_eq!(max_size, Size { width: 50, height: 40 });
    /// ```
    pub fn max(&self, other: &Self) -> Self {
        Size {
            width: if self.width >= other.width {
                self.width.clone()
            } else {
                other.width.clone()
            },
            height: if self.height >= other.height {
                self.height.clone()
            } else {
                other.height.clone()
            },
        }
    }

    /// 返回 `self` 和 `other` 中宽度和高度的最小值组成的新 `Size`。
    ///
    /// # 参数
    ///
    /// * `other` - 要与 `self` 比较的另一个 `Size` 的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::Size;
    /// let size1 = Size { width: 30, height: 40 };
    /// let size2 = Size { width: 50, height: 20 };
    /// let min_size = size1.min(&size2);
    /// assert_eq!(min_size, Size { width: 30, height: 20 });
    /// ```
    pub fn min(&self, other: &Self) -> Self {
        Size {
            width: if self.width >= other.width {
                other.width.clone()
            } else {
                self.width.clone()
            },
            height: if self.height >= other.height {
                other.height.clone()
            } else {
                self.height.clone()
            },
        }
    }
}

impl<T, Rhs> Mul<Rhs> for Size<T>
where
    T: Mul<Rhs, Output = Rhs> + Clone + Debug + Default + PartialEq,
    Rhs: Clone + Debug + Default + PartialEq,
{
    type Output = Size<Rhs>;

    fn mul(self, rhs: Rhs) -> Self::Output {
        Size {
            width: self.width * rhs.clone(),
            height: self.height * rhs,
        }
    }
}

impl<T, S> MulAssign<S> for Size<T>
where
    T: Mul<S, Output = T> + Clone + Debug + Default + PartialEq,
    S: Clone,
{
    fn mul_assign(&mut self, rhs: S) {
        self.width = self.width.clone() * rhs.clone();
        self.height = self.height.clone() * rhs;
    }
}

impl<T> Eq for Size<T> where T: Eq + Clone + Debug + Default + PartialEq {}

impl<T> Debug for Size<T>
where
    T: Clone + Debug + Default + PartialEq,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Size {{ {:?} × {:?} }}", self.width, self.height)
    }
}

impl<T: Clone + Debug + Default + PartialEq + Display> Display for Size<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} × {}", self.width, self.height)
    }
}

impl<T: Clone + Debug + Default + PartialEq> From<Point<T>> for Size<T> {
    fn from(point: Point<T>) -> Self {
        Self {
            width: point.x,
            height: point.y,
        }
    }
}

impl From<Size<Pixels>> for Size<DefiniteLength> {
    fn from(size: Size<Pixels>) -> Self {
        Size {
            width: size.width.into(),
            height: size.height.into(),
        }
    }
}

impl From<Size<Pixels>> for Size<AbsoluteLength> {
    fn from(size: Size<Pixels>) -> Self {
        Size {
            width: size.width.into(),
            height: size.height.into(),
        }
    }
}

impl Size<Length> {
    /// 返回一个宽度和高度都设为填充可用空间的 `Size`。
    ///
    /// 此函数创建一个 `Size` 实例，宽度和高度都设为 `Length::Definite(DefiniteLength::Fraction(1.0))`，
    /// 表示两个维度都占可用空间的 100%。
    ///
    /// # 返回值
    ///
    /// 在布局中使用时将填充可用空间的 `Size<Length>`。
    pub fn full() -> Self {
        Self {
            width: relative(1.).into(),
            height: relative(1.).into(),
        }
    }
}

impl Size<Length> {
    /// 返回一个宽度和高度都设为 `auto` 的 `Size`，允许布局引擎决定尺寸。
    ///
    /// 此函数创建一个 `Size` 实例，宽度和高度都设为 `Length::Auto`，
    /// 表示其尺寸应根据布局上下文（如内容大小或可用空间）来计算。
    ///
    /// # 返回值
    ///
    /// 宽度和高度设为 `Length::Auto` 的 `Size<Length>`。
    pub fn auto() -> Self {
        Self {
            width: Length::Auto,
            height: Length::Auto,
        }
    }
}

/// 表示二维空间中的矩形区域，包含原点和尺寸。
///
/// `Bounds` 结构体对类型 `T` 是泛型的，`T` 表示坐标系统的类型。
/// 原点表示为 `Point<T>`，定义矩形的左上角；
/// 尺寸表示为 `Size<T>`，定义矩形的宽度和高度。
///
/// # 示例
///
/// ```
/// # use rgpui::{Bounds, Point, Size};
/// let origin = Point { x: 0, y: 0 };
/// let size = Size { width: 10, height: 20 };
/// let bounds = Bounds::new(origin, size);
///
/// assert_eq!(bounds.origin, origin);
/// assert_eq!(bounds.size, size);
/// ```
#[derive(Refineable, Copy, Clone, Default, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
#[refineable(Debug)]
#[repr(C)]
pub struct Bounds<T: Clone + Debug + Default + PartialEq> {
    /// 该区域的原点。
    pub origin: Point<T>,
    /// 矩形的尺寸。
    pub size: Size<T>,
}

/// 使用给定的原点和尺寸创建一个 bounds
pub fn bounds<T: Clone + Debug + Default + PartialEq>(
    origin: Point<T>,
    size: Size<T>,
) -> Bounds<T> {
    Bounds { origin, size }
}

impl Bounds<Pixels> {
    /// 为给定显示器或主显示器（未指定时）生成居中的 bounds
    pub fn centered(display_id: Option<DisplayId>, size: Size<Pixels>, cx: &App) -> Self {
        let display = display_id
            .and_then(|id| cx.find_display(id))
            .or_else(|| cx.primary_display());

        display
            .map(|display| Bounds::centered_at(display.bounds().center(), size))
            .unwrap_or_else(|| Bounds {
                origin: point(px(0.), px(0.)),
                size,
            })
    }

    /// 为给定显示器或主显示器（未指定时）生成最大化的 bounds
    pub fn maximized(display_id: Option<DisplayId>, cx: &App) -> Self {
        let display = display_id
            .and_then(|id| cx.find_display(id))
            .or_else(|| cx.primary_display());

        display
            .map(|display| display.bounds())
            .unwrap_or_else(|| Bounds {
                origin: point(px(0.), px(0.)),
                size: size(px(1024.), px(768.)),
            })
    }
}

impl<T> Bounds<T>
where
    T: Clone + Debug + Default + PartialEq,
{
    /// 使用指定的原点和尺寸创建一个新的 `Bounds`。
    ///
    /// # 参数
    ///
    /// * `origin` - 表示 bounds 原点的 `Point<T>`。
    /// * `size` - 表示 bounds 尺寸的 `Size<T>`。
    ///
    /// # 返回值
    ///
    /// 返回具有给定原点和尺寸的 `Bounds<T>`。
    pub fn new(origin: Point<T>, size: Size<T>) -> Self {
        Bounds { origin, size }
    }
}

impl<T> Bounds<T>
where
    T: Sub<Output = T> + Clone + Debug + Default + PartialEq,
{
    /// 从两个角点构造 `Bounds`：左上角和右下角。
    ///
    /// 此函数根据提供的角点计算 `Bounds` 的原点和尺寸。
    /// 原点设为左上角，尺寸由右下角和左上角的 x、y 坐标差值确定。
    ///
    /// # 参数
    ///
    /// * `top_left` - 表示矩形左上角的 `Point<T>`。
    /// * `bottom_right` - 表示矩形右下角的 `Point<T>`。
    ///
    /// # 返回值
    ///
    /// 返回包含两个角点定义区域的 `Bounds<T>`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Bounds, Point};
    /// let top_left = Point { x: 0, y: 0 };
    /// let bottom_right = Point { x: 10, y: 10 };
    /// let bounds = Bounds::from_corners(top_left, bottom_right);
    ///
    /// assert_eq!(bounds.origin, top_left);
    /// assert_eq!(bounds.size.width, 10);
    /// assert_eq!(bounds.size.height, 10);
    /// ```
    pub fn from_corners(top_left: Point<T>, bottom_right: Point<T>) -> Self {
        let origin = Point {
            x: top_left.x.clone(),
            y: top_left.y.clone(),
        };
        let size = Size {
            width: bottom_right.x - top_left.x,
            height: bottom_right.y - top_left.y,
        };
        Bounds { origin, size }
    }
}

impl<T> Bounds<T>
where
    T: Sub<Output = T> + Half + Clone + Debug + Default + PartialEq,
{
    /// 从角点和尺寸构造 `Bounds`。指定的角将放置在给定的原点处。
    pub fn from_anchor_and_size(corner: Anchor, origin: Point<T>, size: Size<T>) -> Bounds<T> {
        let origin = match corner {
            Anchor::TopLeft => origin,
            Anchor::TopRight => Point {
                x: origin.x - size.width.clone(),
                y: origin.y,
            },
            Anchor::BottomLeft => Point {
                x: origin.x,
                y: origin.y - size.height.clone(),
            },
            Anchor::BottomRight => Point {
                x: origin.x - size.width.clone(),
                y: origin.y - size.height.clone(),
            },
            Anchor::TopCenter => Point {
                x: origin.x - size.width.half(),
                y: origin.y,
            },
            Anchor::BottomCenter => Point {
                x: origin.x - size.width.half(),
                y: origin.y - size.height.clone(),
            },
            Anchor::LeftCenter => Point {
                x: origin.x,
                y: origin.y - size.height.half(),
            },
            Anchor::RightCenter => Point {
                x: origin.x - size.width.clone(),
                y: origin.y - size.height.half(),
            },
        };

        Bounds { origin, size }
    }
}

impl<T> Bounds<T>
where
    T: Sub<T, Output = T> + Half + Clone + Debug + Default + PartialEq,
{
    /// 创建以给定点为中心的新 bounds。
    pub fn centered_at(center: Point<T>, size: Size<T>) -> Self {
        let origin = Point {
            x: center.x - size.width.half(),
            y: center.y - size.height.half(),
        };
        Self::new(origin, size)
    }
}

impl<T> Bounds<T>
where
    T: Add<T, Output = T> + Half + Clone + Debug + Default + PartialEq,
{
    /// 返回 bounds 的顶部中心点。
    pub fn top_center(&self) -> Point<T> {
        Point {
            x: self.origin.x.clone() + self.size.width.half(),
            y: self.origin.y.clone(),
        }
    }

    /// 返回 bounds 的底部中心点。
    pub fn bottom_center(&self) -> Point<T> {
        Point {
            x: self.origin.x.clone() + self.size.width.half(),
            y: self.origin.y.clone() + self.size.height.clone(),
        }
    }

    /// 返回 bounds 的左侧中心点。
    pub fn left_center(&self) -> Point<T> {
        Point {
            x: self.origin.x.clone(),
            y: self.origin.y.clone() + self.size.height.half(),
        }
    }

    /// 返回 bounds 的右侧中心点。
    pub fn right_center(&self) -> Point<T> {
        Point {
            x: self.origin.x.clone() + self.size.width.clone(),
            y: self.origin.y.clone() + self.size.height.half(),
        }
    }
}

impl<T> Bounds<T>
where
    T: PartialOrd + Add<T, Output = T> + Clone + Debug + Default + PartialEq,
{
    /// 检查此 `Bounds` 是否与另一个 `Bounds` 相交。
    ///
    /// 两个 `Bounds` 实例相交是指它们在二维空间中存在重叠区域。
    /// 此方法检查两个 bounds 之间是否存在任何重叠区域。
    ///
    /// # 参数
    ///
    /// * `other` - 要检查相交的另一个 `Bounds` 的引用。
    ///
    /// # 返回值
    ///
    /// 如果两个 bounds 之间存在任何相交则返回 `true`，否则返回 `false`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Bounds, Point, Size};
    /// let bounds1 = Bounds {
    ///     origin: Point { x: 0, y: 0 },
    ///     size: Size { width: 10, height: 10 },
    /// };
    /// let bounds2 = Bounds {
    ///     origin: Point { x: 5, y: 5 },
    ///     size: Size { width: 10, height: 10 },
    /// };
    /// let bounds3 = Bounds {
    ///     origin: Point { x: 20, y: 20 },
    ///     size: Size { width: 10, height: 10 },
    /// };
    ///
    /// assert_eq!(bounds1.intersects(&bounds2), true); // 重叠的 bounds
    /// assert_eq!(bounds1.intersects(&bounds3), false); // 不重叠的 bounds
    /// ```
    pub fn intersects(&self, other: &Bounds<T>) -> bool {
        let my_lower_right = self.bottom_right();
        let their_lower_right = other.bottom_right();

        self.origin.x < their_lower_right.x
            && my_lower_right.x > other.origin.x
            && self.origin.y < their_lower_right.y
            && my_lower_right.y > other.origin.y
    }
}

impl<T> Bounds<T>
where
    T: Add<T, Output = T> + Half + Clone + Debug + Default + PartialEq,
{
    /// 返回 bounds 的中心点。
    ///
    /// 通过取原点的 x 和 y 坐标并分别加上 bounds 宽度和高度的一半来计算中心。
    /// 中心表示为 `Point<T>`，其中 `T` 是坐标系统的类型。
    ///
    /// # 返回值
    ///
    /// 表示 bounds 中心的 `Point<T>`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Bounds, Point, Size};
    /// let bounds = Bounds {
    ///     origin: Point { x: 0, y: 0 },
    ///     size: Size { width: 10, height: 20 },
    /// };
    /// let center = bounds.center();
    /// assert_eq!(center, Point { x: 5, y: 10 });
    /// ```
    pub fn center(&self) -> Point<T> {
        Point {
            x: self.origin.x.clone() + self.size.width.clone().half(),
            y: self.origin.y.clone() + self.size.height.clone().half(),
        }
    }
}

impl<T> Bounds<T>
where
    T: Add<T, Output = T> + Clone + Debug + Default + PartialEq,
{
    /// 计算矩形的半周长。
    ///
    /// 半周长计算为矩形宽度和高度之和。
    /// 此方法对类型 `T` 是泛型的，`T` 必须实现 `Sub` trait（用于从 bounds 的原点和尺寸计算宽度和高度）
    /// 以及 `Add` trait（用于将宽度和高度相加得到半周长）。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Bounds, Point, Size};
    /// let bounds = Bounds {
    ///     origin: Point { x: 0, y: 0 },
    ///     size: Size { width: 10, height: 20 },
    /// };
    /// let half_perimeter = bounds.half_perimeter();
    /// assert_eq!(half_perimeter, 30);
    /// ```
    pub fn half_perimeter(&self) -> T {
        self.size.width.clone() + self.size.height.clone()
    }
}

impl<T> Bounds<T>
where
    T: Add<T, Output = T> + Sub<Output = T> + Clone + Debug + Default + PartialEq,
{
    /// 在所有方向上按指定量膨胀 bounds。
    ///
    /// 此方法按给定的 `amount` 扩展 bounds，增大尺寸并调整原点，
    /// 使 bounds 在所有方向上均匀向外扩展。
    /// 结果 bounds 的宽度和高度将增加 `amount` 的两倍（因为在两个方向上都扩展），
    /// 原点将在 x 和 y 方向上各移动 `-amount`。
    ///
    /// # 参数
    ///
    /// * `amount` - 膨胀 bounds 的量。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Bounds, Point, Size};
    /// let mut bounds = Bounds {
    ///     origin: Point { x: 10, y: 10 },
    ///     size: Size { width: 10, height: 10 },
    /// };
    /// let expanded_bounds = bounds.dilate(5);
    /// assert_eq!(expanded_bounds, Bounds {
    ///     origin: Point { x: 5, y: 5 },
    ///     size: Size { width: 20, height: 20 },
    /// });
    /// ```
    #[must_use]
    pub fn dilate(&self, amount: T) -> Bounds<T> {
        let double_amount = amount.clone() + amount.clone();
        Bounds {
            origin: self.origin.clone() - point(amount.clone(), amount),
            size: self.size.clone() + size(double_amount.clone(), double_amount),
        }
    }

    /// 在每个方向上按不同量扩展 bounds。
    #[must_use]
    pub fn extend(&self, amount: Edges<T>) -> Bounds<T> {
        Bounds {
            origin: self.origin.clone() - point(amount.left.clone(), amount.top.clone()),
            size: self.size.clone()
                + size(
                    amount.left.clone() + amount.right.clone(),
                    amount.top.clone() + amount.bottom,
                ),
        }
    }
}

impl<T> Bounds<T>
where
    T: Add<T, Output = T>
        + Sub<T, Output = T>
        + Neg<Output = T>
        + Clone
        + Debug
        + Default
        + PartialEq,
{
    /// 按指定量内缩 bounds。等同于 `dilate` 取负值。
    ///
    /// 注意：如果 `T` 不支持负值，此方法可能会 panic。
    pub fn inset(&self, amount: T) -> Self {
        self.dilate(-amount)
    }
}

impl<T: PartialOrd + Add<T, Output = T> + Sub<Output = T> + Clone + Debug + Default + PartialEq>
    Bounds<T>
{
    /// 计算两个 `Bounds` 对象的交集。
    ///
    /// 此方法计算两个 `Bounds` 的重叠区域。如果 bounds 不相交，
    /// 结果 `Bounds` 的宽度和高度将为零。
    ///
    /// # 参数
    ///
    /// * `other` - 要与之相交的另一个 `Bounds` 的引用。
    ///
    /// # 返回值
    ///
    /// 返回表示相交区域的 `Bounds`。如果没有相交，
    /// 返回的 `Bounds` 的宽度和高度将为零。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Bounds, Point, Size};
    /// let bounds1 = Bounds {
    ///     origin: Point { x: 0, y: 0 },
    ///     size: Size { width: 10, height: 10 },
    /// };
    /// let bounds2 = Bounds {
    ///     origin: Point { x: 5, y: 5 },
    ///     size: Size { width: 10, height: 10 },
    /// };
    /// let intersection = bounds1.intersect(&bounds2);
    ///
    /// assert_eq!(intersection, Bounds {
    ///     origin: Point { x: 5, y: 5 },
    ///     size: Size { width: 5, height: 5 },
    /// });
    /// ```
    pub fn intersect(&self, other: &Self) -> Self {
        let upper_left = self.origin.max(&other.origin);
        let bottom_right = self
            .bottom_right()
            .min(&other.bottom_right())
            .max(&upper_left);
        Self::from_corners(upper_left, bottom_right)
    }

    /// 计算两个 `Bounds` 的并集。
    ///
    /// 此方法计算包含当前 `Bounds` 和 `other` `Bounds` 的最小 `Bounds`。
    /// 结果 `Bounds` 的原点将是两个 `Bounds` 原点的最小值，
    /// 尺寸将包含两个 `Bounds` 的最远边界。
    ///
    /// # 参数
    ///
    /// * `other` - 要与之创建并集的另一个 `Bounds` 的引用。
    ///
    /// # 返回值
    ///
    /// 返回表示两个 `Bounds` 并集的 `Bounds`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Bounds, Point, Size};
    /// let bounds1 = Bounds {
    ///     origin: Point { x: 0, y: 0 },
    ///     size: Size { width: 10, height: 10 },
    /// };
    /// let bounds2 = Bounds {
    ///     origin: Point { x: 5, y: 5 },
    ///     size: Size { width: 15, height: 15 },
    /// };
    /// let union_bounds = bounds1.union(&bounds2);
    ///
    /// assert_eq!(union_bounds, Bounds {
    ///     origin: Point { x: 0, y: 0 },
    ///     size: Size { width: 20, height: 20 },
    /// });
    /// ```
    pub fn union(&self, other: &Self) -> Self {
        let top_left = self.origin.min(&other.origin);
        let bottom_right = self.bottom_right().max(&other.bottom_right());
        Bounds::from_corners(top_left, bottom_right)
    }
}

impl<T> Bounds<T>
where
    T: Add<T, Output = T> + Sub<T, Output = T> + Clone + Debug + Default + PartialEq,
{
    /// 计算外部 bounds 内的可用空间。
    pub fn space_within(&self, outer: &Self) -> Edges<T> {
        Edges {
            top: self.top() - outer.top(),
            right: outer.right() - self.right(),
            bottom: outer.bottom() - self.bottom(),
            left: self.left() - outer.left(),
        }
    }
}

impl<T, Rhs> Mul<Rhs> for Bounds<T>
where
    T: Mul<Rhs, Output = Rhs> + Clone + Debug + Default + PartialEq,
    Point<T>: Mul<Rhs, Output = Point<Rhs>>,
    Rhs: Clone + Debug + Default + PartialEq,
{
    type Output = Bounds<Rhs>;

    fn mul(self, rhs: Rhs) -> Self::Output {
        Bounds {
            origin: self.origin * rhs.clone(),
            size: self.size * rhs,
        }
    }
}

impl<T, S> MulAssign<S> for Bounds<T>
where
    T: Mul<S, Output = T> + Clone + Debug + Default + PartialEq,
    S: Clone,
{
    fn mul_assign(&mut self, rhs: S) {
        self.origin *= rhs.clone();
        self.size *= rhs;
    }
}

impl<T, S> Div<S> for Bounds<T>
where
    Size<T>: Div<S, Output = Size<T>>,
    T: Div<S, Output = T> + Clone + Debug + Default + PartialEq,
    S: Clone,
{
    type Output = Self;

    fn div(self, rhs: S) -> Self {
        Self {
            origin: self.origin / rhs.clone(),
            size: self.size / rhs,
        }
    }
}

impl<T> Add<Point<T>> for Bounds<T>
where
    T: Add<T, Output = T> + Clone + Debug + Default + PartialEq,
{
    type Output = Self;

    fn add(self, rhs: Point<T>) -> Self {
        Self {
            origin: self.origin + rhs,
            size: self.size,
        }
    }
}

impl<T> Sub<Point<T>> for Bounds<T>
where
    T: Sub<T, Output = T> + Clone + Debug + Default + PartialEq,
{
    type Output = Self;

    fn sub(self, rhs: Point<T>) -> Self {
        Self {
            origin: self.origin - rhs,
            size: self.size,
        }
    }
}

impl<T: Clone + Debug + Default + PartialEq> From<Size<T>> for Point<T> {
    fn from(size: Size<T>) -> Self {
        Self {
            x: size.width,
            y: size.height,
        }
    }
}

impl<T> Bounds<T>
where
    T: Add<T, Output = T> + Clone + Debug + Default + PartialEq,
{
    /// 返回 bounds 的上边缘。
    ///
    /// # 返回值
    ///
    /// 类型为 `T` 的值，表示 bounds 上边缘的 y 坐标。
    pub fn top(&self) -> T {
        self.origin.y.clone()
    }

    /// 返回 bounds 的下边缘。
    ///
    /// # 返回值
    ///
    /// 类型为 `T` 的值，表示 bounds 下边缘的 y 坐标。
    pub fn bottom(&self) -> T {
        self.origin.y.clone() + self.size.height.clone()
    }

    /// 返回 bounds 的左边缘。
    ///
    /// # 返回值
    ///
    /// 类型为 `T` 的值，表示 bounds 左边缘的 x 坐标。
    pub fn left(&self) -> T {
        self.origin.x.clone()
    }

    /// 返回 bounds 的右边缘。
    ///
    /// # 返回值
    ///
    /// 类型为 `T` 的值，表示 bounds 右边缘的 x 坐标。
    pub fn right(&self) -> T {
        self.origin.x.clone() + self.size.width.clone()
    }

    /// 返回 bounds 的右上角点。
    ///
    /// # 返回值
    ///
    /// 表示 bounds 右上角的 `Point<T>`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Bounds, Point, Size};
    /// let bounds = Bounds {
    ///     origin: Point { x: 0, y: 0 },
    ///     size: Size { width: 10, height: 20 },
    /// };
    /// let top_right = bounds.top_right();
    /// assert_eq!(top_right, Point { x: 10, y: 0 });
    /// ```
    pub fn top_right(&self) -> Point<T> {
        Point {
            x: self.origin.x.clone() + self.size.width.clone(),
            y: self.origin.y.clone(),
        }
    }

    /// 返回 bounds 的右下角点。
    ///
    /// # 返回值
    ///
    /// 表示 bounds 右下角的 `Point<T>`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Bounds, Point, Size};
    /// let bounds = Bounds {
    ///     origin: Point { x: 0, y: 0 },
    ///     size: Size { width: 10, height: 20 },
    /// };
    /// let bottom_right = bounds.bottom_right();
    /// assert_eq!(bottom_right, Point { x: 10, y: 20 });
    /// ```
    pub fn bottom_right(&self) -> Point<T> {
        Point {
            x: self.origin.x.clone() + self.size.width.clone(),
            y: self.origin.y.clone() + self.size.height.clone(),
        }
    }

    /// 返回 bounds 的左下角点。
    ///
    /// # 返回值
    ///
    /// 表示 bounds 左下角的 `Point<T>`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Bounds, Point, Size};
    /// let bounds = Bounds {
    ///     origin: Point { x: 0, y: 0 },
    ///     size: Size { width: 10, height: 20 },
    /// };
    /// let bottom_left = bounds.bottom_left();
    /// assert_eq!(bottom_left, Point { x: 0, y: 20 });
    /// ```
    pub fn bottom_left(&self) -> Point<T> {
        Point {
            x: self.origin.x.clone(),
            y: self.origin.y.clone() + self.size.height.clone(),
        }
    }
}

impl<T> Bounds<T>
where
    T: Add<T, Output = T> + Half + Clone + Debug + Default + PartialEq,
{
    /// 返回请求的角点。
    ///
    /// # 返回值
    ///
    /// 表示参数请求的 bounds 角点的 `Point<T>`。
    ///
    /// # 示例
    ///
    /// ```
    /// use rgpui::{Bounds, Anchor, Point, Size};
    /// let bounds = Bounds {
    ///     origin: Point { x: 0, y: 0 },
    ///     size: Size { width: 10, height: 20 },
    /// };
    /// let bottom_left = bounds.corner(Anchor::BottomLeft);
    /// assert_eq!(bottom_left, Point { x: 0, y: 20 });
    /// ```
    pub fn corner(&self, corner: Anchor) -> Point<T> {
        match corner {
            Anchor::TopLeft => self.origin.clone(),
            Anchor::TopRight => self.top_right(),
            Anchor::BottomLeft => self.bottom_left(),
            Anchor::BottomRight => self.bottom_right(),
            Anchor::TopCenter => self.top_center(),
            Anchor::BottomCenter => self.bottom_center(),
            Anchor::LeftCenter => self.left_center(),
            Anchor::RightCenter => self.right_center(),
        }
    }
}

impl<T> Bounds<T>
where
    T: Add<T, Output = T> + PartialOrd + Clone + Debug + Default + PartialEq,
{
    /// 检查给定的点是否在 bounds 内。
    ///
    /// 此方法判断一个点是否位于 bounds 定义的矩形内（包括边界）。
    /// 如果点的 x 坐标大于等于左边缘且小于等于右边缘，
    /// y 坐标大于等于上边缘且小于等于下边缘，则认为该点在内部。
    ///
    /// # 参数
    ///
    /// * `point` - 要检查的 `Point<T>` 的引用。
    ///
    /// # 返回值
    ///
    /// 如果点在 bounds 内则返回 `true`，否则返回 `false`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Point, Bounds, Size};
    /// let bounds = Bounds {
    ///     origin: Point { x: 0, y: 0 },
    ///     size: Size { width: 10, height: 10 },
    /// };
    /// let inside_point = Point { x: 5, y: 5 };
    /// let outside_point = Point { x: 15, y: 15 };
    ///
    /// assert!(bounds.contains(&inside_point));
    /// assert!(!bounds.contains(&outside_point));
    /// ```
    pub fn contains(&self, point: &Point<T>) -> bool {
        point.x >= self.origin.x
            && point.x < self.origin.x.clone() + self.size.width.clone()
            && point.y >= self.origin.y
            && point.y < self.origin.y.clone() + self.size.height.clone()
    }

    /// 检查此 bounds 是否完全包含在另一个 bounds 内。
    ///
    /// 此方法判断当前 bounds 是否完全被给定的 bounds 包围。
    /// 如果 bounds 的原点（左上角）和右下角都包含在另一个 bounds 内，
    /// 则认为此 bounds 被包含在另一个 bounds 内。
    ///
    /// # 参数
    ///
    /// * `other` - 可能包含此 bounds 的另一个 `Bounds` 的引用。
    ///
    /// # 返回值
    ///
    /// 如果此 bounds 完全在另一个 bounds 内则返回 `true`，否则返回 `false`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Bounds, Point, Size};
    /// let outer_bounds = Bounds {
    ///     origin: Point { x: 0, y: 0 },
    ///     size: Size { width: 20, height: 20 },
    /// };
    /// let inner_bounds = Bounds {
    ///     origin: Point { x: 5, y: 5 },
    ///     size: Size { width: 10, height: 10 },
    /// };
    /// let overlapping_bounds = Bounds {
    ///     origin: Point { x: 15, y: 15 },
    ///     size: Size { width: 10, height: 10 },
    /// };
    ///
    /// assert!(inner_bounds.is_contained_within(&outer_bounds));
    /// assert!(!overlapping_bounds.is_contained_within(&outer_bounds));
    /// ```
    pub fn is_contained_within(&self, other: &Self) -> bool {
        other.contains(&self.origin) && other.contains(&self.bottom_right())
    }

    /// 对 bounds 的原点和尺寸应用函数，生成新的 `Bounds<U>`。
    ///
    /// 此方法允许通过指定闭包将 `Bounds<T>` 转换为 `Bounds<U>`，闭包定义了两种类型之间的转换方式。
    /// 闭包应用于 `origin` 和 `size` 字段，生成所需类型的新 bounds。
    ///
    /// # 参数
    ///
    /// * `f` - 接受类型 `T` 的值并返回类型 `U` 的值的闭包。
    ///
    /// # 返回值
    ///
    /// 返回原点和尺寸通过给定函数映射后的新 `Bounds<U>`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Bounds, Point, Size};
    /// let bounds = Bounds {
    ///     origin: Point { x: 10.0, y: 10.0 },
    ///     size: Size { width: 10.0, height: 20.0 },
    /// };
    /// let new_bounds = bounds.map(|value| value as f64 * 1.5);
    ///
    /// assert_eq!(new_bounds, Bounds {
    ///     origin: Point { x: 15.0, y: 15.0 },
    ///     size: Size { width: 15.0, height: 30.0 },
    /// });
    /// ```
    pub fn map<U>(&self, f: impl Fn(T) -> U) -> Bounds<U>
    where
        U: Clone + Debug + Default + PartialEq,
    {
        Bounds {
            origin: self.origin.map(&f),
            size: self.size.map(f),
        }
    }

    /// 对 bounds 的原点应用函数，生成具有新原点的新 `Bounds`
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Bounds, Point, Size};
    /// let bounds = Bounds {
    ///     origin: Point { x: 10.0, y: 10.0 },
    ///     size: Size { width: 10.0, height: 20.0 },
    /// };
    /// let new_bounds = bounds.map_origin(|value| value * 1.5);
    ///
    /// assert_eq!(new_bounds, Bounds {
    ///     origin: Point { x: 15.0, y: 15.0 },
    ///     size: Size { width: 10.0, height: 20.0 },
    /// });
    /// ```
    pub fn map_origin(self, f: impl Fn(T) -> T) -> Bounds<T> {
        Bounds {
            origin: self.origin.map(f),
            size: self.size,
        }
    }

    /// 对 bounds 的尺寸应用函数，生成具有新尺寸的新 `Bounds`
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Bounds, Point, Size};
    /// let bounds = Bounds {
    ///     origin: Point { x: 10.0, y: 10.0 },
    ///     size: Size { width: 10.0, height: 20.0 },
    /// };
    /// let new_bounds = bounds.map_size(|value| value * 1.5);
    ///
    /// assert_eq!(new_bounds, Bounds {
    ///     origin: Point { x: 10.0, y: 10.0 },
    ///     size: Size { width: 15.0, height: 30.0 },
    /// });
    /// ```
    pub fn map_size(self, f: impl Fn(T) -> T) -> Bounds<T> {
        Bounds {
            origin: self.origin,
            size: self.size.map(f),
        }
    }
}

impl<T> Bounds<T>
where
    T: Add<T, Output = T> + Sub<T, Output = T> + PartialOrd + Clone + Debug + Default + PartialEq,
{
    /// 将点转换为此 Bounds 定义的坐标空间
    pub fn localize(&self, point: &Point<T>) -> Option<Point<T>> {
        self.contains(point)
            .then(|| point.relative_to(&self.origin))
    }
}

/// 检查 bounds 是否表示空区域。
///
/// # 返回值
///
/// 如果 bounds 的宽度或高度小于或等于零则返回 `true`，表示空区域。
impl<T: PartialOrd + Clone + Debug + Default + PartialEq> Bounds<T> {
    /// 检查 bounds 是否表示空区域。
    ///
    /// # 返回值
    ///
    /// 如果 bounds 的宽度或高度小于或等于零则返回 `true`，表示空区域。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.size.width <= T::default() || self.size.height <= T::default()
    }
}

impl<T: Clone + Debug + Default + PartialEq + Display + Add<T, Output = T>> Display for Bounds<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} - {} (size {})",
            self.origin,
            self.bottom_right(),
            self.size
        )
    }
}

impl Size<DevicePixels> {
    /// 将尺寸从物理像素转换为逻辑像素。
    pub fn to_pixels(self, scale_factor: f32) -> Size<Pixels> {
        size(
            px(self.width.0 as f32 / scale_factor),
            px(self.height.0 as f32 / scale_factor),
        )
    }
}

impl Size<Pixels> {
    /// 将尺寸从逻辑像素转换为物理像素。
    pub fn to_device_pixels(self, scale_factor: f32) -> Size<DevicePixels> {
        size(
            DevicePixels((self.width.0 * scale_factor).round() as i32),
            DevicePixels((self.height.0 * scale_factor).round() as i32),
        )
    }
}

impl Bounds<Pixels> {
    /// 按给定因子缩放 bounds，通常用于调整显示缩放。
    ///
    /// 此方法将 bounds 的原点和尺寸乘以提供的缩放因子，
    /// 生成一个新的 `Bounds<ScaledPixels>`，根据因子按比例放大或缩小。
    /// 这可用于确保 bounds 在不同显示密度下正确缩放。
    ///
    /// # 参数
    ///
    /// * `factor` - 应用于原点和尺寸的缩放因子，通常是显示器的缩放因子。
    ///
    /// # 返回值
    ///
    /// 返回表示缩放后 bounds 的新 `Bounds<ScaledPixels>`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Bounds, Point, Size, Pixels, ScaledPixels, DevicePixels};
    /// let bounds = Bounds {
    ///     origin: Point { x: Pixels::from(10.0), y: Pixels::from(20.0) },
    ///     size: Size { width: Pixels::from(30.0), height: Pixels::from(40.0) },
    /// };
    /// let display_scale_factor = 2.0;
    /// let scaled_bounds = bounds.scale(display_scale_factor);
    /// assert_eq!(scaled_bounds, Bounds {
    ///     origin: Point {
    ///         x: ScaledPixels::from(20.0),
    ///         y: ScaledPixels::from(40.0),
    ///     },
    ///     size: Size {
    ///         width: ScaledPixels::from(60.0),
    ///         height: ScaledPixels::from(80.0)
    ///     },
    /// });
    /// ```
    pub fn scale(&self, factor: f32) -> Bounds<ScaledPixels> {
        Bounds {
            origin: self.origin.scale(factor),
            size: self.size.scale(factor),
        }
    }

    /// 将 bounds 从逻辑像素转换为物理像素
    pub fn to_device_pixels(self, factor: f32) -> Bounds<DevicePixels> {
        Bounds {
            origin: point(
                DevicePixels((self.origin.x.0 * factor).round() as i32),
                DevicePixels((self.origin.y.0 * factor).round() as i32),
            ),
            size: self.size.to_device_pixels(factor),
        }
    }
}

impl Bounds<DevicePixels> {
    /// 将 bounds 从物理像素转换为逻辑像素
    pub fn to_pixels(self, scale_factor: f32) -> Bounds<Pixels> {
        Bounds {
            origin: point(
                px(self.origin.x.0 as f32 / scale_factor),
                px(self.origin.y.0 as f32 / scale_factor),
            ),
            size: self.size.to_pixels(scale_factor),
        }
    }
}

/// 表示二维空间中盒子的边距，如内边距或外边距。
///
/// 每个字段表示盒子一侧的边距大小：`top`、`right`、`bottom` 和 `left`。
///
/// # 示例
///
/// ```
/// # use rgpui::Edges;
/// let edges = Edges {
///     top: 10.0,
///     right: 20.0,
///     bottom: 30.0,
///     left: 40.0,
/// };
///
/// assert_eq!(edges.top, 10.0);
/// assert_eq!(edges.right, 20.0);
/// assert_eq!(edges.bottom, 30.0);
/// assert_eq!(edges.left, 40.0);
/// ```
#[derive(Refineable, Clone, Default, Debug, Eq, PartialEq)]
#[refineable(Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[repr(C)]
pub struct Edges<T: Clone + Debug + Default + PartialEq> {
    /// 上边距的大小。
    pub top: T,
    /// 右边距的大小。
    pub right: T,
    /// 下边距的大小。
    pub bottom: T,
    /// 左边距的大小。
    pub left: T,
}

impl<T> Mul for Edges<T>
where
    T: Mul<Output = T> + Clone + Debug + Default + PartialEq,
{
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            top: self.top.clone() * rhs.top,
            right: self.right.clone() * rhs.right,
            bottom: self.bottom.clone() * rhs.bottom,
            left: self.left * rhs.left,
        }
    }
}

impl<T, S> MulAssign<S> for Edges<T>
where
    T: Mul<S, Output = T> + Clone + Debug + Default + PartialEq,
    S: Clone,
{
    fn mul_assign(&mut self, rhs: S) {
        self.top = self.top.clone() * rhs.clone();
        self.right = self.right.clone() * rhs.clone();
        self.bottom = self.bottom.clone() * rhs.clone();
        self.left = self.left.clone() * rhs;
    }
}

impl<T: Clone + Debug + Default + PartialEq + Copy> Copy for Edges<T> {}

impl<T: Clone + Debug + Default + PartialEq> Edges<T> {
    /// 构造所有边设置为相同值的 `Edges`。
    ///
    /// 此函数创建一个 `Edges` 实例，`top`、`right`、`bottom` 和 `left` 字段都初始化为参数提供的相同值。
    /// 当需要统一的边距（如所有边大小相同的内边距或外边距）时，这很有用。
    ///
    /// # 参数
    ///
    /// * `value` - 设置给四个边的值。
    ///
    /// # 返回值
    ///
    /// 所有边设置为给定值的 `Edges` 实例。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::Edges;
    /// let uniform_edges = Edges::all(10.0);
    /// assert_eq!(uniform_edges.top, 10.0);
    /// assert_eq!(uniform_edges.right, 10.0);
    /// assert_eq!(uniform_edges.bottom, 10.0);
    /// assert_eq!(uniform_edges.left, 10.0);
    /// ```
    pub fn all(value: T) -> Self {
        Self {
            top: value.clone(),
            right: value.clone(),
            bottom: value.clone(),
            left: value,
        }
    }

    /// 对 `Edges` 的每个字段应用函数，生成新的 `Edges<U>`。
    ///
    /// 此方法允许通过指定闭包将 `Edges<T>` 转换为 `Edges<U>`，闭包定义了两种类型之间的转换方式。
    /// 闭包应用于每个字段（`top`、`right`、`bottom`、`left`），生成所需类型的新边距。
    ///
    /// # 参数
    ///
    /// * `f` - 接受类型 `T` 值的引用并返回类型 `U` 的值的闭包。
    ///
    /// # 返回值
    ///
    /// 返回每个字段通过给定函数映射后的新 `Edges<U>`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::Edges;
    /// let edges = Edges { top: 10, right: 20, bottom: 30, left: 40 };
    /// let edges_float = edges.map(|&value| value as f32 * 1.1);
    /// assert_eq!(edges_float, Edges { top: 11.0, right: 22.0, bottom: 33.0, left: 44.0 });
    /// ```
    pub fn map<U>(&self, f: impl Fn(&T) -> U) -> Edges<U>
    where
        U: Clone + Debug + Default + PartialEq,
    {
        Edges {
            top: f(&self.top),
            right: f(&self.right),
            bottom: f(&self.bottom),
            left: f(&self.left),
        }
    }

    /// 检查是否有任何边满足给定的谓词。
    ///
    /// 此方法将谓词函数应用于 `Edges` 的每个字段，如果任何字段满足谓词则返回 `true`。
    ///
    /// # 参数
    ///
    /// * `predicate` - 接受类型 `T` 值的引用并返回 `bool` 的闭包。
    ///
    /// # 返回值
    ///
    /// 如果谓词对任何边值返回 `true` 则返回 `true`，否则返回 `false`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::Edges;
    /// let edges = Edges {
    ///     top: 10,
    ///     right: 0,
    ///     bottom: 5,
    ///     left: 0,
    /// };
    ///
    /// assert!(edges.any(|value| *value == 0));
    /// assert!(edges.any(|value| *value > 0));
    /// assert!(!edges.any(|value| *value > 10));
    /// ```
    pub fn any<F: Fn(&T) -> bool>(&self, predicate: F) -> bool {
        predicate(&self.top)
            || predicate(&self.right)
            || predicate(&self.bottom)
            || predicate(&self.left)
    }
}

impl Edges<Length> {
    /// 将 `Edges` 结构体的边设置为 `auto`，这是一种特殊值，允许布局引擎自动确定边的大小。
    ///
    /// 通常在边的确切大小不重要或大小应根据内容或容器计算的布局上下文中使用。
    ///
    /// # 返回值
    ///
    /// 返回所有边设置为 `Length::Auto` 的 `Edges<Length>`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Edges, Length};
    /// let auto_edges = Edges::auto();
    /// assert_eq!(auto_edges.top, Length::Auto);
    /// assert_eq!(auto_edges.right, Length::Auto);
    /// assert_eq!(auto_edges.bottom, Length::Auto);
    /// assert_eq!(auto_edges.left, Length::Auto);
    /// ```
    pub fn auto() -> Self {
        Self {
            top: Length::Auto,
            right: Length::Auto,
            bottom: Length::Auto,
            left: Length::Auto,
        }
    }

    /// 将 `Edges` 结构体的边设置为零，表示无大小或厚度。
    ///
    /// 通常用于指定盒子（如内边距或外边距区域）没有边距，
    /// 使其在布局计算中不存在或不可见。
    ///
    /// # 返回值
    ///
    /// 返回所有边设置为零长度的 `Edges<Length>`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{DefiniteLength, Edges, Length, Pixels};
    /// let no_edges = Edges::<Length>::zero();
    /// assert_eq!(no_edges.top, Length::Definite(DefiniteLength::from(Pixels::ZERO)));
    /// assert_eq!(no_edges.right, Length::Definite(DefiniteLength::from(Pixels::ZERO)));
    /// assert_eq!(no_edges.bottom, Length::Definite(DefiniteLength::from(Pixels::ZERO)));
    /// assert_eq!(no_edges.left, Length::Definite(DefiniteLength::from(Pixels::ZERO)));
    /// ```
    pub fn zero() -> Self {
        Self {
            top: px(0.).into(),
            right: px(0.).into(),
            bottom: px(0.).into(),
            left: px(0.).into(),
        }
    }
}

impl Edges<DefiniteLength> {
    /// 将 `Edges` 结构体的边设置为零，表示无大小或厚度。
    ///
    /// 通常用于指定盒子（如内边距或外边距区域）没有边距，
    /// 使其在布局计算中不存在或不可见。
    ///
    /// # 返回值
    ///
    /// 返回所有边设置为零长度的 `Edges<DefiniteLength>`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{px, DefiniteLength, Edges};
    /// let no_edges = Edges::<DefiniteLength>::zero();
    /// assert_eq!(no_edges.top, DefiniteLength::from(px(0.)));
    /// assert_eq!(no_edges.right, DefiniteLength::from(px(0.)));
    /// assert_eq!(no_edges.bottom, DefiniteLength::from(px(0.)));
    /// assert_eq!(no_edges.left, DefiniteLength::from(px(0.)));
    /// ```
    pub fn zero() -> Self {
        Self {
            top: px(0.).into(),
            right: px(0.).into(),
            bottom: px(0.).into(),
            left: px(0.).into(),
        }
    }

    /// 根据父元素尺寸和 REM 大小将 `DefiniteLength` 转换为 `Pixels`。
    ///
    /// 此方法允许将 `DefiniteLength` 值转换为像素，考虑父元素的大小（对于百分比长度）
    /// 和 rem 单位的大小（对于 rem 长度）。
    ///
    /// # 参数
    ///
    /// * `parent_size` - 表示父元素尺寸的 `Size<AbsoluteLength>`。
    /// * `rem_size` - 表示一个 REM 单位大小的 `Pixels`。
    ///
    /// # 返回值
    ///
    /// 返回长度转换为像素的 `Edges<Pixels>`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Edges, DefiniteLength, px, AbsoluteLength, rems, Size};
    /// let edges = Edges {
    ///     top: DefiniteLength::Absolute(AbsoluteLength::Pixels(px(10.0))),
    ///     right: DefiniteLength::Fraction(0.5),
    ///     bottom: DefiniteLength::Absolute(AbsoluteLength::Rems(rems(2.0))),
    ///     left: DefiniteLength::Fraction(0.25),
    /// };
    /// let parent_size = Size {
    ///     width: AbsoluteLength::Pixels(px(200.0)),
    ///     height: AbsoluteLength::Pixels(px(100.0)),
    /// };
    /// let rem_size = px(16.0);
    /// let edges_in_pixels = edges.to_pixels(parent_size, rem_size);
    ///
    /// assert_eq!(edges_in_pixels.top, px(10.0)); // 绝对长度（像素）
    /// assert_eq!(edges_in_pixels.right, px(100.0)); // 父宽度的 50%
    /// assert_eq!(edges_in_pixels.bottom, px(32.0)); // 2 rems
    /// assert_eq!(edges_in_pixels.left, px(50.0)); // 父宽度的 25%
    /// ```
    pub fn to_pixels(self, parent_size: Size<AbsoluteLength>, rem_size: Pixels) -> Edges<Pixels> {
        Edges {
            top: self.top.to_pixels(parent_size.height, rem_size),
            right: self.right.to_pixels(parent_size.width, rem_size),
            bottom: self.bottom.to_pixels(parent_size.height, rem_size),
            left: self.left.to_pixels(parent_size.width, rem_size),
        }
    }
}

impl Edges<AbsoluteLength> {
    /// 将 `Edges` 结构体的边设置为零，表示无大小或厚度。
    ///
    /// 通常用于指定盒子（如内边距或外边距区域）没有边距，
    /// 使其在布局计算中不存在或不可见。
    ///
    /// # 返回值
    ///
    /// 返回所有边设置为零长度的 `Edges<AbsoluteLength>`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{AbsoluteLength, Edges, Pixels};
    /// let no_edges = Edges::<AbsoluteLength>::zero();
    /// assert_eq!(no_edges.top, AbsoluteLength::Pixels(Pixels::ZERO));
    /// assert_eq!(no_edges.right, AbsoluteLength::Pixels(Pixels::ZERO));
    /// assert_eq!(no_edges.bottom, AbsoluteLength::Pixels(Pixels::ZERO));
    /// assert_eq!(no_edges.left, AbsoluteLength::Pixels(Pixels::ZERO));
    /// ```
    pub fn zero() -> Self {
        Self {
            top: px(0.).into(),
            right: px(0.).into(),
            bottom: px(0.).into(),
            left: px(0.).into(),
        }
    }

    /// 根据 `rem_size` 将 `AbsoluteLength` 转换为 `Pixels`。
    ///
    /// 如果 `AbsoluteLength` 已经是像素单位，则直接返回对应的 `Pixels` 值。
    /// 如果 `AbsoluteLength` 是 rem 单位，则将 rem 数乘以 `rem_size` 转换为像素。
    ///
    /// # 参数
    ///
    /// * `rem_size` - 一个 rem 单位的像素大小。
    ///
    /// # 返回值
    ///
    /// 返回长度转换为像素的 `Edges<Pixels>`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Edges, AbsoluteLength, Pixels, px, rems};
    /// let edges = Edges {
    ///     top: AbsoluteLength::Pixels(px(10.0)),
    ///     right: AbsoluteLength::Rems(rems(1.0)),
    ///     bottom: AbsoluteLength::Pixels(px(20.0)),
    ///     left: AbsoluteLength::Rems(rems(2.0)),
    /// };
    /// let rem_size = px(16.0);
    /// let edges_in_pixels = edges.to_pixels(rem_size);
    ///
    /// assert_eq!(edges_in_pixels.top, px(10.0)); // 已经是像素
    /// assert_eq!(edges_in_pixels.right, px(16.0)); // 1 rem 转换为像素
    /// assert_eq!(edges_in_pixels.bottom, px(20.0)); // 已经是像素
    /// assert_eq!(edges_in_pixels.left, px(32.0)); // 2 rems 转换为像素
    /// ```
    pub fn to_pixels(self, rem_size: Pixels) -> Edges<Pixels> {
        Edges {
            top: self.top.to_pixels(rem_size),
            right: self.right.to_pixels(rem_size),
            bottom: self.bottom.to_pixels(rem_size),
            left: self.left.to_pixels(rem_size),
        }
    }
}

impl Edges<Pixels> {
    /// 按给定因子缩放 `Edges<Pixels>`，返回 `Edges<ScaledPixels>`。
    ///
    /// 此方法通常用于调整不同显示密度或缩放因子下的边距大小。
    ///
    /// # 参数
    ///
    /// * `factor` - 应用于每条边的缩放因子。
    ///
    /// # 返回值
    ///
    /// 返回新的 `Edges<ScaledPixels>`，其中每条边是原始边乘以给定因子的结果。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Edges, Pixels, ScaledPixels};
    /// let edges = Edges {
    ///     top: Pixels::from(10.0),
    ///     right: Pixels::from(20.0),
    ///     bottom: Pixels::from(30.0),
    ///     left: Pixels::from(40.0),
    /// };
    /// let scaled_edges = edges.scale(2.0);
    /// assert_eq!(scaled_edges.top, ScaledPixels::from(20.0));
    /// assert_eq!(scaled_edges.right, ScaledPixels::from(40.0));
    /// assert_eq!(scaled_edges.bottom, ScaledPixels::from(60.0));
    /// assert_eq!(scaled_edges.left, ScaledPixels::from(80.0));
    /// ```
    pub fn scale(&self, factor: f32) -> Edges<ScaledPixels> {
        Edges {
            top: self.top.scale(factor),
            right: self.right.scale(factor),
            bottom: self.bottom.scale(factor),
            left: self.left.scale(factor),
        }
    }

    /// 返回任意边的最大值。
    ///
    /// # 返回值
    ///
    /// 四条边中最大的 `Pixels` 值。
    pub fn max(&self) -> Pixels {
        self.top.max(self.right).max(self.bottom).max(self.left)
    }
}

impl From<f32> for Edges<Pixels> {
    fn from(val: f32) -> Self {
        let val: Pixels = val.into();
        val.into()
    }
}

impl From<Pixels> for Edges<Pixels> {
    fn from(val: Pixels) -> Self {
        Edges {
            top: val,
            right: val,
            bottom: val,
            left: val,
        }
    }
}

/// 标识二维盒子上的参考点，用于锚定定位元素。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    /// 左上角
    TopLeft,
    /// 右上角
    TopRight,
    /// 左下角
    BottomLeft,
    /// 右下角
    BottomRight,
    /// 顶部中心位置
    TopCenter,
    /// 底部中心位置
    BottomCenter,
    /// 左侧中心位置
    LeftCenter,
    /// 右侧中心位置
    RightCenter,
}

impl Anchor {
    /// 返回完全相反的角。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::Anchor;
    /// assert_eq!(Anchor::TopLeft.opposite(), Anchor::BottomRight);
    /// ```
    #[must_use]
    pub fn opposite(self) -> Self {
        match self {
            Anchor::TopLeft => Anchor::BottomRight,
            Anchor::TopRight => Anchor::BottomLeft,
            Anchor::BottomLeft => Anchor::TopRight,
            Anchor::BottomRight => Anchor::TopLeft,
            Anchor::TopCenter => Anchor::BottomCenter,
            Anchor::BottomCenter => Anchor::TopCenter,
            Anchor::LeftCenter => Anchor::RightCenter,
            Anchor::RightCenter => Anchor::LeftCenter,
        }
    }

    /// 返回沿给定轴对面的角。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Axis, Anchor};
    /// let result = Anchor::TopLeft.other_side_along(Axis::Horizontal);
    /// assert_eq!(result, Anchor::TopRight);
    /// ```
    #[must_use]
    pub fn other_side_along(self, axis: Axis) -> Self {
        match axis {
            Axis::Vertical => match self {
                Anchor::TopLeft => Anchor::BottomLeft,
                Anchor::TopRight => Anchor::BottomRight,
                Anchor::BottomLeft => Anchor::TopLeft,
                Anchor::BottomRight => Anchor::TopRight,
                Anchor::TopCenter => Anchor::BottomCenter,
                Anchor::BottomCenter => Anchor::TopCenter,
                Anchor::LeftCenter => Anchor::LeftCenter,
                Anchor::RightCenter => Anchor::RightCenter,
            },
            Axis::Horizontal => match self {
                Anchor::TopLeft => Anchor::TopRight,
                Anchor::TopRight => Anchor::TopLeft,
                Anchor::BottomLeft => Anchor::BottomRight,
                Anchor::BottomRight => Anchor::BottomLeft,
                Anchor::TopCenter => Anchor::TopCenter,
                Anchor::BottomCenter => Anchor::BottomCenter,
                Anchor::LeftCenter => Anchor::RightCenter,
                Anchor::RightCenter => Anchor::LeftCenter,
            },
        }
    }

    /// 如果在中心位置则返回 true。
    #[inline]
    pub fn is_center(&self) -> bool {
        matches!(
            self,
            Self::TopCenter | Self::BottomCenter | Self::LeftCenter | Self::RightCenter
        )
    }
}

/// 表示二维空间中盒子的圆角，如边框圆角。
///
/// 每个字段表示盒子一侧圆角的大小：`top_left`、`top_right`、`bottom_right` 和 `bottom_left`。
#[derive(Refineable, Clone, Default, Debug, Eq, PartialEq)]
#[refineable(Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[repr(C)]
pub struct Corners<T: Clone + Debug + Default + PartialEq> {
    /// 与左上角关联的值。
    pub top_left: T,
    /// 与右上角关联的值。
    pub top_right: T,
    /// 与右下角关联的值。
    pub bottom_right: T,
    /// 与左下角关联的值。
    pub bottom_left: T,
}

impl<T> Corners<T>
where
    T: Add<T, Output = T> + Half + Clone + Debug + Default + PartialEq,
{
    /// 构造所有角设置为相同值的 `Corners`。
    ///
    /// 此函数创建一个 `Corners` 实例，`top_left`、`top_right`、`bottom_right` 和 `bottom_left` 字段
    /// 都初始化为参数提供的相同值。当需要统一的圆角（如矩形上统一的边框圆角）时，这很有用。
    ///
    /// # 参数
    ///
    /// * `value` - 设置给四个角的值。
    ///
    /// # 返回值
    ///
    /// 所有角设置为给定值的 `Corners` 实例。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::Corners;
    /// let uniform_corners = Corners::all(5.0);
    /// assert_eq!(uniform_corners.top_left, 5.0);
    /// assert_eq!(uniform_corners.top_right, 5.0);
    /// assert_eq!(uniform_corners.bottom_right, 5.0);
    /// assert_eq!(uniform_corners.bottom_left, 5.0);
    /// ```
    pub fn all(value: T) -> Self {
        Self {
            top_left: value.clone(),
            top_right: value.clone(),
            bottom_right: value.clone(),
            bottom_left: value,
        }
    }

    /// 返回请求的角值，支持所有八个角位置。
    ///
    /// 对于四个基本角（TopLeft、TopRight、BottomLeft、BottomRight），
    /// 直接返回对应的字段值。
    ///
    /// 对于中心位置（TopCenter、BottomCenter、LeftCenter、RightCenter），
    /// 计算两个相邻角的平均值。
    ///
    /// # 返回值
    ///
    /// 表示参数请求的角的类型 `T` 的值。
    ///
    /// # 示例
    ///
    /// 基本角位置：
    ///
    /// ```
    /// # use rgpui::{Anchor, Corners};
    /// let corners = Corners {
    ///     top_left: 10,
    ///     top_right: 20,
    ///     bottom_left: 30,
    ///     bottom_right: 40
    /// };
    /// assert_eq!(corners.corner(Anchor::TopLeft), 10);
    /// assert_eq!(corners.corner(Anchor::BottomRight), 40);
    /// ```
    ///
    /// 中心位置（计算为相邻角的平均值）：
    ///
    /// ```
    /// # use rgpui::{Anchor, Corners};
    /// let corners = Corners {
    ///     top_left: 10,
    ///     top_right: 20,
    ///     bottom_left: 30,
    ///     bottom_right: 40
    /// };
    /// assert_eq!(corners.corner(Anchor::TopCenter), 15);
    /// assert_eq!(corners.corner(Anchor::BottomCenter), 35);
    /// assert_eq!(corners.corner(Anchor::LeftCenter), 20);
    /// assert_eq!(corners.corner(Anchor::RightCenter), 30);
    /// ```
    #[must_use]
    pub fn corner(&self, corner: Anchor) -> T {
        match corner {
            Anchor::TopLeft => self.top_left.clone(),
            Anchor::TopRight => self.top_right.clone(),
            Anchor::BottomLeft => self.bottom_left.clone(),
            Anchor::BottomRight => self.bottom_right.clone(),
            Anchor::TopCenter => (self.top_left.clone() + self.top_right.clone()).half(),
            Anchor::BottomCenter => (self.bottom_left.clone() + self.bottom_right.clone()).half(),
            Anchor::LeftCenter => (self.top_left.clone() + self.bottom_left.clone()).half(),
            Anchor::RightCenter => (self.top_right.clone() + self.bottom_right.clone()).half(),
        }
    }
}

impl Corners<AbsoluteLength> {
    /// 根据提供的 rem 大小将 `AbsoluteLength` 转换为 `Pixels`。
    ///
    /// # 参数
    ///
    /// * `rem_size` - 一个 REM 单位的像素大小，用于当 `AbsoluteLength` 为 REM 单位时的转换。
    ///
    /// # 返回值
    ///
    /// 返回每个角的长度转换为像素的 `Corners<Pixels>` 实例。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Corners, AbsoluteLength, Pixels, Rems, Size};
    /// let corners = Corners {
    ///     top_left: AbsoluteLength::Pixels(Pixels::from(15.0)),
    ///     top_right: AbsoluteLength::Rems(Rems(1.0)),
    ///     bottom_right: AbsoluteLength::Pixels(Pixels::from(30.0)),
    ///     bottom_left: AbsoluteLength::Rems(Rems(2.0)),
    /// };
    /// let rem_size = Pixels::from(16.0);
    /// let corners_in_pixels = corners.to_pixels(rem_size);
    ///
    /// assert_eq!(corners_in_pixels.top_left, Pixels::from(15.0));
    /// assert_eq!(corners_in_pixels.top_right, Pixels::from(16.0)); // 1 rem 转换为像素
    /// assert_eq!(corners_in_pixels.bottom_right, Pixels::from(30.0));
    /// assert_eq!(corners_in_pixels.bottom_left, Pixels::from(32.0)); // 2 rems 转换为像素
    /// ```
    pub fn to_pixels(self, rem_size: Pixels) -> Corners<Pixels> {
        Corners {
            top_left: self.top_left.to_pixels(rem_size),
            top_right: self.top_right.to_pixels(rem_size),
            bottom_right: self.bottom_right.to_pixels(rem_size),
            bottom_left: self.bottom_left.to_pixels(rem_size),
        }
    }
}

impl Corners<Pixels> {
    /// 按给定因子缩放 `Corners<Pixels>`，返回 `Corners<ScaledPixels>`。
    ///
    /// 此方法通常用于调整不同显示密度或缩放因子下的圆角大小。
    ///
    /// # 参数
    ///
    /// * `factor` - 应用于每个角的缩放因子。
    ///
    /// # 返回值
    ///
    /// 返回新的 `Corners<ScaledPixels>`，其中每个角是原始角乘以给定因子的结果。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Corners, Pixels, ScaledPixels};
    /// let corners = Corners {
    ///     top_left: Pixels::from(10.0),
    ///     top_right: Pixels::from(20.0),
    ///     bottom_right: Pixels::from(30.0),
    ///     bottom_left: Pixels::from(40.0),
    /// };
    /// let scaled_corners = corners.scale(2.0);
    /// assert_eq!(scaled_corners.top_left, ScaledPixels::from(20.0));
    /// assert_eq!(scaled_corners.top_right, ScaledPixels::from(40.0));
    /// assert_eq!(scaled_corners.bottom_right, ScaledPixels::from(60.0));
    /// assert_eq!(scaled_corners.bottom_left, ScaledPixels::from(80.0));
    /// ```
    #[must_use]
    pub fn scale(&self, factor: f32) -> Corners<ScaledPixels> {
        Corners {
            top_left: self.top_left.scale(factor),
            top_right: self.top_right.scale(factor),
            bottom_right: self.bottom_right.scale(factor),
            bottom_left: self.bottom_left.scale(factor),
        }
    }

    /// 返回任意角的最大值。
    ///
    /// # 返回值
    ///
    /// 四个角中最大的 `Pixels` 值。
    #[must_use]
    pub fn max(&self) -> Pixels {
        self.top_left
            .max(self.top_right)
            .max(self.bottom_right)
            .max(self.bottom_left)
    }
}

impl<T: Div<f32, Output = T> + Ord + Clone + Debug + Default + PartialEq> Corners<T> {
    /// 将圆角半径限制为不超过四边形最短边的一半。
    ///
    /// # 参数
    ///
    /// * `size` - 限制圆角半径大小的四边形尺寸。
    ///
    /// # 返回值
    ///
    /// 限制后适合的圆角半径值。
    #[must_use]
    pub fn clamp_radii_for_quad_size(self, size: Size<T>) -> Corners<T> {
        let max = cmp::min(size.width, size.height) / 2.;
        Corners {
            top_left: cmp::min(self.top_left, max.clone()),
            top_right: cmp::min(self.top_right, max.clone()),
            bottom_right: cmp::min(self.bottom_right, max.clone()),
            bottom_left: cmp::min(self.bottom_left, max),
        }
    }
}

impl<T: Clone + Debug + Default + PartialEq> Corners<T> {
    /// 对 `Corners` 的每个字段应用函数，生成新的 `Corners<U>`。
    ///
    /// 此方法允许通过指定闭包将 `Corners<T>` 转换为 `Corners<U>`，闭包定义了两种类型之间的转换方式。
    /// 闭包应用于每个字段（`top_left`、`top_right`、`bottom_right`、`bottom_left`），生成所需类型的新圆角。
    ///
    /// # 参数
    ///
    /// * `f` - 接受类型 `T` 值的引用并返回类型 `U` 的值的闭包。
    ///
    /// # 返回值
    ///
    /// 返回每个字段通过给定函数映射后的新 `Corners<U>`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{Corners, Pixels, Rems};
    /// let corners = Corners {
    ///     top_left: Pixels::from(10.0),
    ///     top_right: Pixels::from(20.0),
    ///     bottom_right: Pixels::from(30.0),
    ///     bottom_left: Pixels::from(40.0),
    /// };
    /// let corners_in_rems = corners.map(|&px| Rems(f32::from(px) / 16.0));
    /// assert_eq!(corners_in_rems, Corners {
    ///     top_left: Rems(0.625),
    ///     top_right: Rems(1.25),
    ///     bottom_right: Rems(1.875),
    ///     bottom_left: Rems(2.5),
    /// });
    /// ```
    #[must_use]
    pub fn map<U>(&self, f: impl Fn(&T) -> U) -> Corners<U>
    where
        U: Clone + Debug + Default + PartialEq,
    {
        Corners {
            top_left: f(&self.top_left),
            top_right: f(&self.top_right),
            bottom_right: f(&self.bottom_right),
            bottom_left: f(&self.bottom_left),
        }
    }
}

impl<T> Mul for Corners<T>
where
    T: Mul<Output = T> + Clone + Debug + Default + PartialEq,
{
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            top_left: self.top_left.clone() * rhs.top_left,
            top_right: self.top_right.clone() * rhs.top_right,
            bottom_right: self.bottom_right.clone() * rhs.bottom_right,
            bottom_left: self.bottom_left * rhs.bottom_left,
        }
    }
}

impl<T, S> MulAssign<S> for Corners<T>
where
    T: Mul<S, Output = T> + Clone + Debug + Default + PartialEq,
    S: Clone,
{
    fn mul_assign(&mut self, rhs: S) {
        self.top_left = self.top_left.clone() * rhs.clone();
        self.top_right = self.top_right.clone() * rhs.clone();
        self.bottom_right = self.bottom_right.clone() * rhs.clone();
        self.bottom_left = self.bottom_left.clone() * rhs;
    }
}

impl<T> Copy for Corners<T> where T: Copy + Clone + Debug + Default + PartialEq {}

impl From<f32> for Corners<Pixels> {
    fn from(val: f32) -> Self {
        Corners {
            top_left: val.into(),
            top_right: val.into(),
            bottom_right: val.into(),
            bottom_left: val.into(),
        }
    }
}

impl From<Pixels> for Corners<Pixels> {
    fn from(val: Pixels) -> Self {
        Corners {
            top_left: val,
            top_right: val,
            bottom_right: val,
            bottom_left: val,
        }
    }
}

/// 表示弧度角。
#[derive(
    Clone,
    Copy,
    Default,
    Add,
    AddAssign,
    Sub,
    SubAssign,
    Neg,
    Div,
    DivAssign,
    PartialEq,
    Serialize,
    Deserialize,
    Debug,
)]
#[repr(transparent)]
pub struct Radians(pub f32);

/// 从原始值创建一个 `Radians`。
pub fn radians(value: f32) -> Radians {
    Radians(value)
}

/// 表示百分比值的类型。
#[derive(
    Clone,
    Copy,
    Default,
    Add,
    AddAssign,
    Sub,
    SubAssign,
    Neg,
    Div,
    DivAssign,
    PartialEq,
    Serialize,
    Deserialize,
    Debug,
)]
#[repr(transparent)]
pub struct Percentage(pub f32);

/// 从完整圆的百分比生成一个 `Radians`。
pub fn percentage(value: f32) -> Percentage {
    debug_assert!(
        (0.0..=1.0).contains(&value),
        "Percentage must be between 0 and 1"
    );
    Percentage(value)
}

impl From<Percentage> for Radians {
    fn from(value: Percentage) -> Self {
        radians(value.0 * std::f32::consts::PI * 2.0)
    }
}

/// 表示以像素为单位的长度，UI 框架中的基本度量单位。
///
/// `Pixels` 是一个表示绝对像素长度的值类型，用于指定 UI 中的尺寸、位置和距离。
/// 它是所有视觉元素和布局计算的基本度量单位。
///
/// 内部值为 `f32`，允许亚像素精度，这在抗锯齿和动画中很有用。
/// 但当应用于实际像素网格时，该值通常会四舍五入到最近的整数。
///
/// # 示例
///
/// ```
/// use rgpui::{Pixels, ScaledPixels};
///
/// // 定义 10 像素的长度
/// let length = Pixels::from(10.0);
///
/// // 定义长度并按 2 倍因子缩放
/// let scaled_length = length.scale(2.0);
/// assert_eq!(scaled_length, ScaledPixels::from(20.0));
/// ```
#[derive(
    Clone,
    Copy,
    Default,
    Add,
    AddAssign,
    Sub,
    SubAssign,
    Neg,
    Div,
    DivAssign,
    PartialEq,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[repr(transparent)]
pub struct Pixels(pub(crate) f32);

impl Div for Pixels {
    type Output = f32;

    fn div(self, rhs: Self) -> Self::Output {
        self.0 / rhs.0
    }
}

impl std::ops::DivAssign for Pixels {
    fn div_assign(&mut self, rhs: Self) {
        *self = Self(self.0 / rhs.0);
    }
}

impl std::ops::RemAssign for Pixels {
    fn rem_assign(&mut self, rhs: Self) {
        self.0 %= rhs.0;
    }
}

impl std::ops::Rem for Pixels {
    type Output = Self;

    fn rem(self, rhs: Self) -> Self {
        Self(self.0 % rhs.0)
    }
}

impl Mul<f32> for Pixels {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Self(self.0 * rhs)
    }
}

impl Mul<Pixels> for f32 {
    type Output = Pixels;

    fn mul(self, rhs: Pixels) -> Self::Output {
        rhs * self
    }
}

impl Mul<usize> for Pixels {
    type Output = Self;

    fn mul(self, rhs: usize) -> Self {
        self * (rhs as f32)
    }
}

impl Mul<Pixels> for usize {
    type Output = Pixels;

    fn mul(self, rhs: Pixels) -> Pixels {
        rhs * self
    }
}

impl MulAssign<f32> for Pixels {
    fn mul_assign(&mut self, rhs: f32) {
        self.0 *= rhs;
    }
}

impl Display for Pixels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}px", self.0)
    }
}

impl Debug for Pixels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl std::iter::Sum for Pixels {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, |a, b| a + b)
    }
}

impl<'a> std::iter::Sum<&'a Pixels> for Pixels {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, |a, b| a + *b)
    }
}

impl TryFrom<&'_ str> for Pixels {
    type Error = anyhow::Error;

    fn try_from(value: &'_ str) -> Result<Self, Self::Error> {
        value
            .strip_suffix("px")
            .context("expected 'px' suffix")
            .and_then(|number| Ok(number.parse()?))
            .map(Self)
    }
}

impl Pixels {
    /// 表示零像素。
    pub const ZERO: Pixels = Pixels(0.0);
    /// `Pixels` 能表示的最大值。
    pub const MAX: Pixels = Pixels(f32::MAX);
    /// `Pixels` 能表示的最小值。
    pub const MIN: Pixels = Pixels(f32::MIN);

    /// 返回此 `Pixels` 的原始 `f32` 值。
    pub fn as_f32(self) -> f32 {
        self.0
    }

    /// 将 `Pixels` 值向下取整到最近的整数。
    ///
    /// # 返回值
    ///
    /// 返回向下取整后的新 `Pixels` 实例。
    pub fn floor(&self) -> Self {
        Self(self.0.floor())
    }

    /// 将 `Pixels` 值四舍五入到最近的整数。
    ///
    /// # 返回值
    ///
    /// 返回四舍五入后的新 `Pixels` 实例。
    pub fn round(&self) -> Self {
        Self(self.0.round())
    }

    /// 将 `Pixels` 值向上取整到最近的整数。
    ///
    /// # 返回值
    ///
    /// 返回向上取整后的新 `Pixels` 实例。
    pub fn ceil(&self) -> Self {
        Self(self.0.ceil())
    }

    /// 按给定因子缩放 `Pixels` 值，生成 `ScaledPixels`。
    ///
    /// 此方法用于在显示缩放因子（如高 DPI 或 Retina 显示器）下调整像素值，
    /// 这些显示器的像素密度更高，因此需要缩放以保持视觉一致性和可读性。
    ///
    /// 结果 `ScaledPixels` 表示缩放后的值，可用于考虑显示缩放的渲染计算。
    #[must_use]
    pub fn scale(&self, factor: f32) -> ScaledPixels {
        ScaledPixels(self.0 * factor)
    }

    /// 将 `Pixels` 值提升到给定幂次。
    ///
    /// # 参数
    ///
    /// * `exponent` - 用于提升 `Pixels` 值的指数。
    ///
    /// # 返回值
    ///
    /// 返回提升到给定指数后的新 `Pixels` 实例。
    pub fn pow(&self, exponent: f32) -> Self {
        Self(self.0.powf(exponent))
    }

    /// 返回 `Pixels` 的绝对值。
    ///
    /// # 返回值
    ///
    /// 返回原始 `Pixels` 绝对值的新实例。
    pub fn abs(&self) -> Self {
        Self(self.0.abs())
    }

    /// 返回 `Pixels` 值的符号。
    ///
    /// # 返回值
    ///
    /// 返回：
    /// * `1.0` 如果值为正
    /// * `-1.0` 如果值为负
    pub fn signum(&self) -> f32 {
        self.0.signum()
    }

    /// 返回 `Pixels` 的 f64 值。
    ///
    /// # 返回值
    ///
    /// `Pixels` 的 f64 值。
    pub fn to_f64(self) -> f64 {
        self.0 as f64
    }
}

impl Eq for Pixels {}

impl PartialOrd for Pixels {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Pixels {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl std::hash::Hash for Pixels {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl From<f64> for Pixels {
    fn from(pixels: f64) -> Self {
        Pixels(pixels as f32)
    }
}

impl From<f32> for Pixels {
    fn from(pixels: f32) -> Self {
        Pixels(pixels)
    }
}

impl From<Pixels> for f32 {
    fn from(pixels: Pixels) -> Self {
        pixels.0
    }
}

impl From<&Pixels> for f32 {
    fn from(pixels: &Pixels) -> Self {
        pixels.0
    }
}

impl From<Pixels> for f64 {
    fn from(pixels: Pixels) -> Self {
        pixels.0 as f64
    }
}

impl From<Pixels> for u32 {
    fn from(pixels: Pixels) -> Self {
        pixels.0 as u32
    }
}

impl From<&Pixels> for u32 {
    fn from(pixels: &Pixels) -> Self {
        pixels.0 as u32
    }
}

impl From<u32> for Pixels {
    fn from(pixels: u32) -> Self {
        Pixels(pixels as f32)
    }
}

impl From<Pixels> for usize {
    fn from(pixels: Pixels) -> Self {
        pixels.0 as usize
    }
}

impl From<usize> for Pixels {
    fn from(pixels: usize) -> Self {
        Pixels(pixels as f32)
    }
}

/// 表示显示设备上的物理像素。
///
/// `DevicePixels` 是一个度量单位，指设备屏幕上的实际像素。
/// 此类型用于需要精确像素操作的场景，如渲染图形或与在像素级别操作的硬件交互。
/// 与可能受设备缩放因子影响的逻辑像素不同，`DevicePixels` 始终对应显示设备上的真实像素。
#[derive(
    Add,
    AddAssign,
    Clone,
    Copy,
    Default,
    Div,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    Sub,
    SubAssign,
    Serialize,
    Deserialize,
)]
#[repr(transparent)]
pub struct DevicePixels(pub i32);

impl DevicePixels {
    /// 将 `DevicePixels` 值转换为内存中表示所需的字节数。
    ///
    /// 此函数在处理需要存储在缓冲区中的图形数据时很有用，
    /// 如图像或帧缓冲区，其中每个像素可能由特定字节数表示。
    ///
    /// # 参数
    ///
    /// * `bytes_per_pixel` - 表示单个像素所用的字节数。
    ///
    /// # 返回值
    ///
    /// 内存中表示 `DevicePixels` 值所需的字节数。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::DevicePixels;
    /// let pixels = DevicePixels(10); // 10 个设备像素
    /// let bytes_per_pixel = 4; // 假设每个像素由 4 字节表示（如 RGBA）
    /// let total_bytes = pixels.to_bytes(bytes_per_pixel);
    /// assert_eq!(total_bytes, 40); // 10 像素 * 4 字节/像素 = 40 字节
    /// ```
    pub fn to_bytes(self, bytes_per_pixel: u8) -> u32 {
        self.0 as u32 * bytes_per_pixel as u32
    }
}

impl fmt::Debug for DevicePixels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} px (device)", self.0)
    }
}

impl From<DevicePixels> for i32 {
    fn from(device_pixels: DevicePixels) -> Self {
        device_pixels.0
    }
}

impl From<i32> for DevicePixels {
    fn from(device_pixels: i32) -> Self {
        DevicePixels(device_pixels)
    }
}

impl From<u32> for DevicePixels {
    fn from(device_pixels: u32) -> Self {
        DevicePixels(device_pixels as i32)
    }
}

impl From<DevicePixels> for u32 {
    fn from(device_pixels: DevicePixels) -> Self {
        device_pixels.0 as u32
    }
}

impl From<DevicePixels> for u64 {
    fn from(device_pixels: DevicePixels) -> Self {
        device_pixels.0 as u64
    }
}

impl From<u64> for DevicePixels {
    fn from(device_pixels: u64) -> Self {
        DevicePixels(device_pixels as i32)
    }
}

impl From<DevicePixels> for usize {
    fn from(device_pixels: DevicePixels) -> Self {
        device_pixels.0 as usize
    }
}

impl From<usize> for DevicePixels {
    fn from(device_pixels: usize) -> Self {
        DevicePixels(device_pixels as i32)
    }
}

/// 表示考虑设备缩放因子的缩放像素。
///
/// `ScaledPixels` 用于确保 UI 元素在不同像素密度的设备上显示正确的大小。
/// 当设备具有更高的缩放因子（如 Retina 显示器）时，单个逻辑像素可能对应多个物理像素。
/// 通过使用 `ScaledPixels`，可以以适当缩放的方式指定尺寸和位置，
/// 从而在不同显示分辨率下正确缩放。
#[derive(Clone, Copy, Default, Add, AddAssign, Sub, SubAssign, Div, DivAssign, PartialEq)]
#[repr(transparent)]
pub struct ScaledPixels(pub f32);

impl ScaledPixels {
    /// 返回此 `ScaledPixels` 的原始 `f32` 值。
    pub fn as_f32(self) -> f32 {
        self.0
    }

    /// 将 `ScaledPixels` 值向下取整到最近的整数。
    ///
    /// # 返回值
    ///
    /// 返回向下取整后的新 `ScaledPixels` 实例。
    pub fn floor(&self) -> Self {
        Self(self.0.floor())
    }

    /// 将 `ScaledPixels` 值四舍五入到最近的整数。
    ///
    /// # 返回值
    ///
    /// 返回四舍五入后的新 `ScaledPixels` 实例。
    pub fn round(&self) -> Self {
        Self(self.0.round())
    }

    /// 将 `ScaledPixels` 值向上取整到最近的整数。
    ///
    /// # 返回值
    ///
    /// 返回向上取整后的新 `ScaledPixels` 实例。
    pub fn ceil(&self) -> Self {
        Self(self.0.ceil())
    }
}

impl Eq for ScaledPixels {}

impl PartialOrd for ScaledPixels {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScaledPixels {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl Debug for ScaledPixels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}px (scaled)", self.0)
    }
}

impl From<ScaledPixels> for DevicePixels {
    fn from(scaled: ScaledPixels) -> Self {
        DevicePixels(scaled.0.ceil() as i32)
    }
}

impl From<DevicePixels> for ScaledPixels {
    fn from(device: DevicePixels) -> Self {
        ScaledPixels(device.0 as f32)
    }
}

impl From<ScaledPixels> for f64 {
    fn from(scaled_pixels: ScaledPixels) -> Self {
        scaled_pixels.0 as f64
    }
}

impl From<ScaledPixels> for u32 {
    fn from(pixels: ScaledPixels) -> Self {
        pixels.0 as u32
    }
}

impl From<f32> for ScaledPixels {
    fn from(pixels: f32) -> Self {
        Self(pixels)
    }
}

impl Div for ScaledPixels {
    type Output = f32;

    fn div(self, rhs: Self) -> Self::Output {
        self.0 / rhs.0
    }
}

impl std::ops::DivAssign for ScaledPixels {
    fn div_assign(&mut self, rhs: Self) {
        *self = Self(self.0 / rhs.0);
    }
}

impl std::ops::RemAssign for ScaledPixels {
    fn rem_assign(&mut self, rhs: Self) {
        self.0 %= rhs.0;
    }
}

impl std::ops::Rem for ScaledPixels {
    type Output = Self;

    fn rem(self, rhs: Self) -> Self {
        Self(self.0 % rhs.0)
    }
}

impl Mul<f32> for ScaledPixels {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Self(self.0 * rhs)
    }
}

impl Mul<ScaledPixels> for f32 {
    type Output = ScaledPixels;

    fn mul(self, rhs: ScaledPixels) -> Self::Output {
        rhs * self
    }
}

impl Mul<usize> for ScaledPixels {
    type Output = Self;

    fn mul(self, rhs: usize) -> Self {
        self * (rhs as f32)
    }
}

impl Mul<ScaledPixels> for usize {
    type Output = ScaledPixels;

    fn mul(self, rhs: ScaledPixels) -> ScaledPixels {
        rhs * self
    }
}

impl MulAssign<f32> for ScaledPixels {
    fn mul_assign(&mut self, rhs: f32) {
        self.0 *= rhs;
    }
}

/// 表示以 rem 为单位的长度，一种基于窗口字体大小的单位，可通过 [`Window::set_rem_size`][set_rem_size] 设置。
///
/// Rems 用于定义可缩放且在不同 UI 元素间保持一致的长度。
/// `1rem` 的值通常等于根元素的字体大小（通常是浏览器中的 `<html>` 元素），
/// 使其成为适应用户文本大小偏好的灵活单位。在此框架中，`rems` 具有类似用途，
/// 允许可缩放和可访问的设计，可适应不同的显示设置或用户偏好。
///
/// 例如，如果根元素的字体大小为 `16px`，则 `1rem` 等于 `16px`。`2rems` 的长度则为 `32px`。
///
/// [set_rem_size]: crate::Window::set_rem_size
#[derive(Clone, Copy, Default, Add, Sub, Mul, Div, Neg, PartialEq)]
pub struct Rems(pub f32);

impl Rems {
    /// 零长度。
    pub const ZERO: Self = Self(0.0);
    /// 将此 Rem 值转换为像素。
    pub fn to_pixels(self, rem_size: Pixels) -> Pixels {
        self * rem_size
    }
    /// 从像素转换为 Rem
    pub fn from_pixels(length: Pixels, window: &rgpui::Window) -> Self {
        Self(length / window.rem_size())
    }
}

impl Mul<Pixels> for Rems {
    type Output = Pixels;

    fn mul(self, other: Pixels) -> Pixels {
        Pixels(self.0 * other.0)
    }
}

impl AddAssign<Rems> for Rems {
    fn add_assign(&mut self, rhs: Rems) {
        self.0 += rhs.0
    }
}

impl Display for Rems {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}rem", self.0)
    }
}

impl Debug for Rems {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl TryFrom<&'_ str> for Rems {
    type Error = anyhow::Error;

    fn try_from(value: &'_ str) -> Result<Self, Self::Error> {
        value
            .strip_suffix("rem")
            .context("expected 'rem' suffix")
            .and_then(|number| Ok(number.parse()?))
            .map(Self)
    }
}

/// 表示以像素或 rem 为单位的绝对长度。
///
/// `AbsoluteLength` 可以是固定像素数（不受当前字体大小影响的绝对度量），
/// 或 rem 数（相对于根元素的字体大小）。用于指定独立于或与排版比例相关的尺寸。
#[derive(Clone, Copy, Neg, PartialEq)]
pub enum AbsoluteLength {
    /// 以像素为单位的长度。
    Pixels(Pixels),
    /// 以 rem 为单位的长度。
    Rems(Rems),
}

impl AbsoluteLength {
    /// 检查绝对长度是否为零。
    pub fn is_zero(&self) -> bool {
        match self {
            AbsoluteLength::Pixels(px) => px.0 == 0.0,
            AbsoluteLength::Rems(rems) => rems.0 == 0.0,
        }
    }
}

impl From<Pixels> for AbsoluteLength {
    fn from(pixels: Pixels) -> Self {
        AbsoluteLength::Pixels(pixels)
    }
}

impl From<Rems> for AbsoluteLength {
    fn from(rems: Rems) -> Self {
        AbsoluteLength::Rems(rems)
    }
}

impl AbsoluteLength {
    /// 根据给定的 `rem_size` 将 `AbsoluteLength` 转换为 `Pixels`。
    ///
    /// # 参数
    ///
    /// * `rem_size` - 一个 rem 的像素大小。
    ///
    /// # 返回值
    ///
    /// 返回 `AbsoluteLength` 转换为 `Pixels` 的结果。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{AbsoluteLength, Pixels, Rems};
    /// let length_in_pixels = AbsoluteLength::Pixels(Pixels::from(42.0));
    /// let length_in_rems = AbsoluteLength::Rems(Rems(2.0));
    /// let rem_size = Pixels::from(16.0);
    ///
    /// assert_eq!(length_in_pixels.to_pixels(rem_size), Pixels::from(42.0));
    /// assert_eq!(length_in_rems.to_pixels(rem_size), Pixels::from(32.0));
    /// ```
    pub fn to_pixels(self, rem_size: Pixels) -> Pixels {
        match self {
            AbsoluteLength::Pixels(pixels) => pixels,
            AbsoluteLength::Rems(rems) => rems.to_pixels(rem_size),
        }
    }

    /// 根据给定的 `rem_size` 将 `AbsoluteLength` 转换为 `Rems`。
    ///
    /// # 参数
    ///
    /// * `rem_size` - 一个 rem 的像素大小。
    ///
    /// # 返回值
    ///
    /// 返回 `AbsoluteLength` 转换为 `Rems` 的结果。
    pub fn to_rems(self, rem_size: Pixels) -> Rems {
        match self {
            AbsoluteLength::Pixels(pixels) => Rems(pixels.0 / rem_size.0),
            AbsoluteLength::Rems(rems) => rems,
        }
    }
}

impl Default for AbsoluteLength {
    fn default() -> Self {
        px(0.).into()
    }
}

impl Display for AbsoluteLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pixels(pixels) => write!(f, "{pixels}"),
            Self::Rems(rems) => write!(f, "{rems}"),
        }
    }
}

impl Debug for AbsoluteLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

const EXPECTED_ABSOLUTE_LENGTH: &str = "number with 'px' or 'rem' suffix";

impl TryFrom<&'_ str> for AbsoluteLength {
    type Error = anyhow::Error;

    fn try_from(value: &'_ str) -> Result<Self, Self::Error> {
        if let Ok(pixels) = value.try_into() {
            Ok(Self::Pixels(pixels))
        } else if let Ok(rems) = value.try_into() {
            Ok(Self::Rems(rems))
        } else {
            Err(anyhow!(
                "invalid AbsoluteLength '{value}', expected {EXPECTED_ABSOLUTE_LENGTH}"
            ))
        }
    }
}

impl JsonSchema for AbsoluteLength {
    fn schema_name() -> Cow<'static, str> {
        "AbsoluteLength".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        json_schema!({
            "type": "string",
            "pattern": r"^-?\d+(\.\d+)?(px|rem)$"
        })
    }
}

impl<'de> Deserialize<'de> for AbsoluteLength {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct StringVisitor;

        impl de::Visitor<'_> for StringVisitor {
            type Value = AbsoluteLength;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{EXPECTED_ABSOLUTE_LENGTH}")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                AbsoluteLength::try_from(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(StringVisitor)
    }
}

impl Serialize for AbsoluteLength {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{self}"))
    }
}

/// 非自动长度，可用像素、rem 或父元素百分比定义。
///
/// 此枚举表示具有特定值的长度，与由上下文自动确定的长度相对。
/// 它包括以像素或 rem 为单位的绝对长度，以及作为父元素尺寸比例的相对长度。
#[derive(Clone, Copy, Neg, PartialEq)]
pub enum DefiniteLength {
    /// 以像素或 rem 为单位的绝对长度。
    Absolute(AbsoluteLength),
    /// 以父元素尺寸比例表示的相对长度，介于 0 和 1 之间。
    Fraction(f32),
}

impl DefiniteLength {
    /// 根据给定的 `base_size` 和 `rem_size` 将 `DefiniteLength` 转换为 `Pixels`。
    ///
    /// 如果 `DefiniteLength` 是绝对长度，将直接转换为 `Pixels`。
    /// 如果是比例值，将乘以 `base_size` 以获取像素长度。
    ///
    /// # 参数
    ///
    /// * `base_size` - 应用比例的基准 `AbsoluteLength` 尺寸。
    /// * `rem_size` - 一个 rem 的像素大小，用于将 rem 转换为像素。
    ///
    /// # 返回值
    ///
    /// 返回 `DefiniteLength` 转换为 `Pixels` 的结果。
    ///
    /// # 示例
    ///
    /// ```
    /// # use rgpui::{DefiniteLength, AbsoluteLength, Pixels, px, rems};
    /// let length_in_pixels = DefiniteLength::Absolute(AbsoluteLength::Pixels(px(42.0)));
    /// let length_in_rems = DefiniteLength::Absolute(AbsoluteLength::Rems(rems(2.0)));
    /// let length_as_fraction = DefiniteLength::Fraction(0.5);
    /// let base_size = AbsoluteLength::Pixels(px(100.0));
    /// let rem_size = px(16.0);
    ///
    /// assert_eq!(length_in_pixels.to_pixels(base_size, rem_size), Pixels::from(42.0));
    /// assert_eq!(length_in_rems.to_pixels(base_size, rem_size), Pixels::from(32.0));
    /// assert_eq!(length_as_fraction.to_pixels(base_size, rem_size), Pixels::from(50.0));
    /// ```
    pub fn to_pixels(self, base_size: AbsoluteLength, rem_size: Pixels) -> Pixels {
        match self {
            DefiniteLength::Absolute(size) => size.to_pixels(rem_size),
            DefiniteLength::Fraction(fraction) => match base_size {
                AbsoluteLength::Pixels(px) => px * fraction,
                AbsoluteLength::Rems(rems) => rems * rem_size * fraction,
            },
        }
    }
}

impl Debug for DefiniteLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for DefiniteLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DefiniteLength::Absolute(length) => write!(f, "{length}"),
            DefiniteLength::Fraction(fraction) => write!(f, "{}%", (fraction * 100.0) as i32),
        }
    }
}

const EXPECTED_DEFINITE_LENGTH: &str = "expected number with 'px', 'rem', or '%' suffix";

impl TryFrom<&'_ str> for DefiniteLength {
    type Error = anyhow::Error;

    fn try_from(value: &'_ str) -> Result<Self, Self::Error> {
        if let Some(percentage) = value.strip_suffix('%') {
            let fraction: f32 = percentage.parse::<f32>().with_context(|| {
                format!("invalid DefiniteLength '{value}', expected {EXPECTED_DEFINITE_LENGTH}")
            })?;
            Ok(DefiniteLength::Fraction(fraction / 100.0))
        } else if let Ok(absolute_length) = value.try_into() {
            Ok(DefiniteLength::Absolute(absolute_length))
        } else {
            Err(anyhow!(
                "invalid DefiniteLength '{value}', expected {EXPECTED_DEFINITE_LENGTH}"
            ))
        }
    }
}

impl JsonSchema for DefiniteLength {
    fn schema_name() -> Cow<'static, str> {
        "DefiniteLength".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        json_schema!({
            "type": "string",
            "pattern": r"^-?\d+(\.\d+)?(px|rem|%)$"
        })
    }
}

impl<'de> Deserialize<'de> for DefiniteLength {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct StringVisitor;

        impl de::Visitor<'_> for StringVisitor {
            type Value = DefiniteLength;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{EXPECTED_DEFINITE_LENGTH}")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                DefiniteLength::try_from(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(StringVisitor)
    }
}

impl Serialize for DefiniteLength {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{self}"))
    }
}

impl From<Pixels> for DefiniteLength {
    fn from(pixels: Pixels) -> Self {
        Self::Absolute(pixels.into())
    }
}

impl From<Rems> for DefiniteLength {
    fn from(rems: Rems) -> Self {
        Self::Absolute(rems.into())
    }
}

impl From<AbsoluteLength> for DefiniteLength {
    fn from(length: AbsoluteLength) -> Self {
        Self::Absolute(length)
    }
}

impl Default for DefiniteLength {
    fn default() -> Self {
        Self::Absolute(AbsoluteLength::default())
    }
}

/// 可用像素、rem、父元素百分比或 auto 定义的长度。
#[derive(Clone, Copy, PartialEq)]
pub enum Length {
    /// 以像素、rem 或父元素尺寸比例指定的确定长度。
    Definite(DefiniteLength),
    /// 由使用上下文自动确定的自动长度。
    Auto,
}

impl Debug for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Length::Definite(definite_length) => write!(f, "{}", definite_length),
            Length::Auto => write!(f, "auto"),
        }
    }
}

const EXPECTED_LENGTH: &str = "expected 'auto' or number with 'px', 'rem', or '%' suffix";

impl TryFrom<&'_ str> for Length {
    type Error = anyhow::Error;

    fn try_from(value: &'_ str) -> Result<Self, Self::Error> {
        if value == "auto" {
            Ok(Length::Auto)
        } else if let Ok(definite_length) = value.try_into() {
            Ok(Length::Definite(definite_length))
        } else {
            Err(anyhow!(
                "invalid Length '{value}', expected {EXPECTED_LENGTH}"
            ))
        }
    }
}

impl JsonSchema for Length {
    fn schema_name() -> Cow<'static, str> {
        "Length".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        json_schema!({
            "type": "string",
            "pattern": r"^(auto|-?\d+(\.\d+)?(px|rem|%))$"
        })
    }
}

impl<'de> Deserialize<'de> for Length {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct StringVisitor;

        impl de::Visitor<'_> for StringVisitor {
            type Value = Length;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{EXPECTED_LENGTH}")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Length::try_from(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(StringVisitor)
    }
}

impl Serialize for Length {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{self}"))
    }
}

/// 构造表示父元素尺寸相对比例的 `DefiniteLength`。
///
/// 此函数创建一个 `DefiniteLength`，它是父元素尺寸的指定比例。
/// 比例应为 0.0 到 1.0 之间的浮点数，其中 1.0 表示父元素尺寸的 100%。
///
/// # 参数
///
/// * `fraction` - 父元素尺寸的比例，介于 0.0 和 1.0 之间。
///
/// # 返回值
///
/// 表示父元素尺寸相对比例的 `DefiniteLength`。
pub const fn relative(fraction: f32) -> DefiniteLength {
    DefiniteLength::Fraction(fraction)
}

/// 返回黄金比例，即 `~(1.0 + sqrt(5.0)) / 2.0`。
pub const fn phi() -> DefiniteLength {
    relative(1.618_034)
}

/// 构造表示以 rem 为单位长度的 `Rems` 值。
///
/// # 参数
///
/// * `rems` - 长度的 rem 数。
///
/// # 返回值
///
/// 表示指定 rem 数的 `Rems`。
pub const fn rems(rems: f32) -> Rems {
    Rems(rems)
}

/// 构造表示以像素为单位长度的 `Pixels` 值。
///
/// # 参数
///
/// * `pixels` - 长度的像素数。
///
/// # 返回值
///
/// 表示指定像素数的 `Pixels`。
pub const fn px(pixels: f32) -> Pixels {
    Pixels(pixels)
}

/// 返回表示自动长度的 `Length`。
///
/// `auto` 长度常用于布局计算中，其中长度应由布局上下文本身而非显式设置来确定。
/// 这在 CSS 中常用于 `width`、`height`、`margin`、`padding` 等属性，
/// 其中 `auto` 可用于指示布局引擎根据其他因素（如容器大小或内容的固有大小）来计算尺寸。
///
/// # 返回值
///
/// 设置为 `Auto` 的 `Length` 变体。
pub const fn auto() -> Length {
    Length::Auto
}

impl From<Pixels> for Length {
    fn from(pixels: Pixels) -> Self {
        Self::Definite(pixels.into())
    }
}

impl From<Rems> for Length {
    fn from(rems: Rems) -> Self {
        Self::Definite(rems.into())
    }
}

impl From<DefiniteLength> for Length {
    fn from(length: DefiniteLength) -> Self {
        Self::Definite(length)
    }
}

impl From<AbsoluteLength> for Length {
    fn from(length: AbsoluteLength) -> Self {
        Self::Definite(length.into())
    }
}

impl Default for Length {
    fn default() -> Self {
        Self::Definite(DefiniteLength::default())
    }
}

impl From<()> for Length {
    fn from(_: ()) -> Self {
        Self::Definite(DefiniteLength::default())
    }
}

/// 网格布局中的位置。
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct GridLocation {
    /// 此项目在网格中使用的行。
    pub row: Range<GridPlacement>,
    /// 此项目在网格中使用的列。
    pub column: Range<GridPlacement>,
}

/// 项目在网格布局列或行中的放置方式。
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize, JsonSchema, Default)]
pub enum GridPlacement {
    /// 放置此项目的网格线索引。
    Line(i16),
    /// 跨越的网格线数。
    Span(u16),
    /// 自动确定放置方式，等同于 Span(1)
    #[default]
    Auto,
}

impl From<GridPlacement> for taffy::GridPlacement {
    fn from(placement: GridPlacement) -> Self {
        match placement {
            GridPlacement::Line(index) => taffy::GridPlacement::from_line_index(index),
            GridPlacement::Span(span) => taffy::GridPlacement::from_span(span),
            GridPlacement::Auto => taffy::GridPlacement::Auto,
        }
    }
}

/// 提供可以计算值一半的 trait。
///
/// `Half` trait 用于可以被均匀分割的类型，返回表示原始值一半的同一类型的新实例。
/// 这常用于表示度量或尺寸的类型，如长度或像素，
/// 其中在布局计算或动画中减半是常见操作。
pub trait Half {
    /// 返回当前值的一半。
    ///
    /// # 返回值
    ///
    /// 实现类型的新实例，表示原始值的一半。
    fn half(&self) -> Self;
}

impl Half for i32 {
    fn half(&self) -> Self {
        self / 2
    }
}

impl Half for f32 {
    fn half(&self) -> Self {
        self / 2.
    }
}

impl Half for DevicePixels {
    fn half(&self) -> Self {
        Self(self.0 / 2)
    }
}

impl Half for ScaledPixels {
    fn half(&self) -> Self {
        Self(self.0 / 2.)
    }
}

impl Half for Pixels {
    fn half(&self) -> Self {
        Self(self.0 / 2.)
    }
}

impl Half for Rems {
    fn half(&self) -> Self {
        Self(self.0 / 2.)
    }
}

/// 用于检查值是否为零的 trait。
///
/// 此 trait 提供一种方法来确定值是否为零。
/// 它为各种数值和长度相关类型实现了该 trait，其中零的概念是适用的。
/// 这可用于比较、优化或确定操作是否具有中性效果。
pub trait IsZero {
    /// 确定值是否为零。
    ///
    /// # 返回值
    ///
    /// 如果值为零则返回 `true`，否则返回 `false`。
    fn is_zero(&self) -> bool;
}

impl IsZero for DevicePixels {
    fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl IsZero for ScaledPixels {
    fn is_zero(&self) -> bool {
        self.0 == 0.
    }
}

impl IsZero for Pixels {
    fn is_zero(&self) -> bool {
        self.0 == 0.
    }
}

impl IsZero for Rems {
    fn is_zero(&self) -> bool {
        self.0 == 0.
    }
}

impl IsZero for AbsoluteLength {
    fn is_zero(&self) -> bool {
        match self {
            AbsoluteLength::Pixels(pixels) => pixels.is_zero(),
            AbsoluteLength::Rems(rems) => rems.is_zero(),
        }
    }
}

impl IsZero for DefiniteLength {
    fn is_zero(&self) -> bool {
        match self {
            DefiniteLength::Absolute(length) => length.is_zero(),
            DefiniteLength::Fraction(fraction) => *fraction == 0.,
        }
    }
}

impl IsZero for Length {
    fn is_zero(&self) -> bool {
        match self {
            Length::Definite(length) => length.is_zero(),
            Length::Auto => false,
        }
    }
}

impl<T: IsZero + Clone + Debug + Default + PartialEq> IsZero for Point<T> {
    fn is_zero(&self) -> bool {
        self.x.is_zero() && self.y.is_zero()
    }
}

impl<T> IsZero for Size<T>
where
    T: IsZero + Clone + Debug + Default + PartialEq,
{
    fn is_zero(&self) -> bool {
        self.width.is_zero() || self.height.is_zero()
    }
}

impl<T: IsZero + Clone + Debug + Default + PartialEq> IsZero for Bounds<T> {
    fn is_zero(&self) -> bool {
        self.size.is_zero()
    }
}

impl<T> IsZero for Corners<T>
where
    T: IsZero + Clone + Debug + Default + PartialEq,
{
    fn is_zero(&self) -> bool {
        self.top_left.is_zero()
            && self.top_right.is_zero()
            && self.bottom_right.is_zero()
            && self.bottom_left.is_zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds_intersects() {
        let bounds1 = Bounds {
            origin: Point { x: 0.0, y: 0.0 },
            size: Size {
                width: 5.0,
                height: 5.0,
            },
        };
        let bounds2 = Bounds {
            origin: Point { x: 4.0, y: 4.0 },
            size: Size {
                width: 5.0,
                height: 5.0,
            },
        };
        let bounds3 = Bounds {
            origin: Point { x: 10.0, y: 10.0 },
            size: Size {
                width: 5.0,
                height: 5.0,
            },
        };

        // Test Case 1: Intersecting bounds
        assert!(bounds1.intersects(&bounds2));

        // Test Case 2: Non-Intersecting bounds
        assert!(!bounds1.intersects(&bounds3));

        // Test Case 3: Bounds intersecting with themselves
        assert!(bounds1.intersects(&bounds1));
    }
}
