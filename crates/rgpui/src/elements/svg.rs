use std::{
    fs,
    hash::{Hash, Hasher},
    path::Path,
    sync::Arc,
    time::Duration,
};

use crate::ResultExt;
use crate::{
    App, Asset, Bounds, Element, GlobalElementId, Hitbox, InspectorElementId, InteractiveElement,
    Interactivity, IntoElement, LayoutId, Pixels, Point, Radians, SharedString, Size,
    SpriteAnimation, StyleRefinement, Styled, TransformationMatrix, Window, point, px, radians,
    size,
};

/// SVG 元素，用于渲染 SVG 矢量图形。
pub struct Svg {
    /// 交互状态管理
    interactivity: Interactivity,
    /// 可选变换参数（静态变换或定时动画变换）
    transformation: Option<TimedTransformation>,
    /// 通过 AssetSource 加载的 SVG 路径
    path: Option<SharedString>,
    /// 直接文件系统路径（不经过 AssetSource）
    external_path: Option<SharedString>,
    /// 原始 SVG 二进制数据
    data: Option<Arc<[u8]>>,
    /// 基于数据哈希生成的确定性缓存路径
    data_path: Option<SharedString>,
}

/// 创建一个新的 SVG 元素。
#[track_caller]
pub fn svg() -> Svg {
    Svg {
        interactivity: Interactivity::new(),
        transformation: None,
        path: None,
        external_path: None,
        data: None,
        data_path: None,
    }
}

impl Svg {
    /// 通过资源系统（AssetSource）设置 SVG 文件的路径。
    pub fn path(mut self, path: impl Into<SharedString>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// 通过文件系统直接设置 SVG 文件的路径（不经过 AssetSource）。
    pub fn external_path(mut self, path: impl Into<SharedString>) -> Self {
        self.external_path = Some(path.into());
        self
    }

    /// 设置此元素的原始 SVG 数据。
    /// SVG 将直接从提供的字节渲染。
    pub fn data(mut self, data: &[u8]) -> Self {
        // 基于数据哈希生成确定性的唯一路径，用于缓存。
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        data.hash(&mut hasher);
        let hash = hasher.finish();
        let path = SharedString::from(format!("__binary_svg__{}", hash));
        self.data = Some(Arc::from(data));
        self.data_path = Some(path);
        self
    }

    /// 是否携带了嵌入式 SVG 数据。
    ///
    /// 仅测试用：断言内置图标不依赖运行时资源（宿主 `AssetSource`）。
    #[cfg(test)]
    pub(crate) fn has_embedded_data(&self) -> bool {
        self.data.is_some()
    }

    /// 对 SVG 元素应用静态变换。
    /// 注意：这不会影响元素的 hitbox 或布局，仅影响渲染效果。
    pub fn with_transformation(mut self, transformation: Transformation) -> Self {
        self.transformation = Some(transformation.into());
        self
    }

    /// 让 SVG 在两个变换之间连续动画，相位锁定到 App 的同步动画时钟
    /// （口径同 `Animation::repeat_synced`）。
    ///
    /// SVG 只绘制一次；之后的帧由窗口在已有场景里移动它并重新呈现，
    /// 不重绘这个视图。
    ///
    /// 与 `with_transformation` 一样不影响布局与命中测试；SVG 仍按其
    /// **起始**变换的边界裁剪与排序，动画幅度请保持在附近。
    pub fn with_timed_transformation(mut self, transformation: TimedTransformation) -> Self {
        self.transformation = Some(transformation);
        self
    }
}

impl Element for Svg {
    type RequestLayoutState = ();
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<crate::ElementId> {
        self.interactivity.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.interactivity.source_location()
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = self.interactivity.request_layout(
            global_id,
            inspector_id,
            window,
            cx,
            |style, window, cx| window.request_layout(style, None, cx),
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Hitbox> {
        self.interactivity.prepaint(
            global_id,
            inspector_id,
            bounds,
            bounds.size,
            window,
            cx,
            |_, _, hitbox, _, _| hitbox,
        )
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        hitbox: &mut Option<Hitbox>,
        window: &mut Window,
        cx: &mut App,
    ) where
        Self: Sized,
    {
        self.interactivity.paint(
            global_id,
            inspector_id,
            bounds,
            hitbox.as_ref(),
            window,
            cx,
            |style, window, cx| {
                let center = bounds.center();
                let scale_factor = window.scale_factor();
                // 有动画且系统未要求减少动态效果时交给场景级动画；否则
                // （含 reduce motion）按起始变换静态绘制。
                let animation = self
                    .transformation
                    .filter(|timed| timed.is_animated() && !cx.reduce_motion())
                    .map(|timed| SpriteAnimation::new(timed, center, scale_factor));
                let pose = self.transformation.map(|timed| timed.from);
                let transformation = pose
                    .map(|pose| pose.into_matrix(center, scale_factor))
                    .unwrap_or_default();
                // 场景在每次呈现前对动画精灵合成 alpha，所以这里传的是动画
                // alpha 之前的底色；静态路径则直接把 alpha 乘进颜色。
                let alpha = match animation {
                    Some(_) => 1.0,
                    None => pose.map_or(1.0, |pose| pose.alpha_factor()),
                };
                let color = style.text.color.map(|color| color.opacity(alpha));

                if let Some((data, path)) = self.data.as_ref().zip(self.data_path.as_ref()) {
                    if let Some(color) = color {
                        window
                            .paint_animated_svg(
                                bounds,
                                path.clone(),
                                Some(&**data),
                                transformation,
                                animation,
                                color,
                                cx,
                            )
                            .log_err();
                    }
                } else if let Some((path, color)) = self.external_path.as_ref().zip(color) {
                    let Some(bytes) = window
                        .use_asset::<SvgAsset>(path, cx)
                        .and_then(|asset| asset.log_err())
                    else {
                        return;
                    };

                    window
                        .paint_animated_svg(
                            bounds,
                            path.clone(),
                            Some(&bytes),
                            transformation,
                            animation,
                            color,
                            cx,
                        )
                        .log_err();
                } else if let Some((path, color)) = self.path.as_ref().zip(color) {
                    window
                        .paint_animated_svg(
                            bounds,
                            path.clone(),
                            None,
                            transformation,
                            animation,
                            color,
                            cx,
                        )
                        .log_err();
                }
            },
        )
    }
}

impl IntoElement for Svg {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Styled for Svg {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.interactivity.base_style
    }
}

impl InteractiveElement for Svg {
    fn interactivity(&mut self) -> &mut Interactivity {
        &mut self.interactivity
    }
}

/// 应用于 SVG 元素的变换参数。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transformation {
    /// 缩放因子
    scale: Size<f32>,
    /// 平移偏移量
    translate: Point<Pixels>,
    /// 旋转角度（弧度）
    rotate: Radians,
    /// 不透明度倍率（1.0 = 不额外改透明度）。
    alpha: f32,
}

impl Default for Transformation {
    fn default() -> Self {
        Self {
            scale: size(1.0, 1.0),
            translate: point(px(0.0), px(0.0)),
            rotate: radians(0.0),
            alpha: 1.0,
        }
    }
}

impl Transformation {
    /// 创建一个只包含缩放的新变换。
    pub fn scale(scale: Size<f32>) -> Self {
        Self {
            scale,
            ..Self::default()
        }
    }

    /// 创建一个只包含平移的新变换。
    pub fn translate(translate: Point<Pixels>) -> Self {
        Self {
            translate,
            ..Self::default()
        }
    }

    /// 创建一个只包含旋转的新变换（弧度）。
    pub fn rotate(rotate: impl Into<Radians>) -> Self {
        let rotate = rotate.into();
        Self {
            rotate,
            ..Self::default()
        }
    }

    /// 创建一个只把不透明度乘以 `alpha` 的新变换（用于淡入淡出动画）。
    pub fn alpha(alpha: f32) -> Self {
        Self {
            alpha,
            ..Self::default()
        }
    }

    /// 更新变换的缩放因子。
    pub fn with_scaling(mut self, scale: Size<f32>) -> Self {
        self.scale = scale;
        self
    }

    /// 更新变换的平移值。
    pub fn with_translation(mut self, translate: Point<Pixels>) -> Self {
        self.translate = translate;
        self
    }

    /// 更新变换的旋转角度。
    pub fn with_rotation(mut self, rotate: impl Into<Radians>) -> Self {
        self.rotate = rotate.into();
        self
    }

    /// 更新变换的不透明度倍率。
    pub fn with_alpha(mut self, alpha: f32) -> Self {
        self.alpha = alpha;
        self
    }

    /// 取变换的不透明度倍率（供动画精灵合成底色透明度用）。
    pub(crate) fn alpha_factor(self) -> f32 {
        self.alpha
    }

    /// 将变换转换为变换矩阵。从底部开始读矩阵乘法顺序。
    pub(crate) fn into_matrix(
        self,
        center: Point<Pixels>,
        scale_factor: f32,
    ) -> TransformationMatrix {
        // 注意：将矩阵乘法序列从底部开始阅读
        TransformationMatrix::unit()
            .translate(center.scale(scale_factor) + self.translate.scale(scale_factor))
            .rotate(self.rotate)
            .scale(self.scale)
            .translate(center.scale(-scale_factor))
    }
}

/// 某个值在一个 [`TimedTransformation`] 周期内随时间的缓动函数。
pub type EasingFn = fn(f32) -> f32;

/// 一段时间内在两个 [`Transformation`] 之间连续变换的定义，
/// 供 [`Svg::with_timed_transformation`] 使用。
///
/// 变换的每个分量各有自己的 [`EasingFn`]，所以例如一个 SVG 可以「先快速放大、
/// 再匀速淡出」。动画由场景在每次呈现前推进（`Scene::advance_animations`），
/// 不重绘视图。
#[derive(Clone, Copy, Debug)]
pub struct TimedTransformation {
    period: Duration,
    from: Transformation,
    to: Transformation,
    scale_easing: EasingFn,
    translation_easing: EasingFn,
    rotation_easing: EasingFn,
    alpha_easing: EasingFn,
    alternate: bool,
    min_frame_interval: Option<Duration>,
}

/// [`TimedTransformation::at`] 的结果：该时刻的变换（alpha 在 `transformation` 里）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TimedTransformationPose {
    pub(crate) transformation: Transformation,
}

impl TimedTransformationPose {
    /// 该时刻的 alpha 倍率。
    pub(crate) fn alpha_factor(self) -> f32 {
        self.transformation.alpha
    }
}

fn linear(t: f32) -> f32 {
    t
}

impl TimedTransformation {
    /// 创建一个每 `period` 重复一次的定时变换。
    ///
    /// 在 [`Self::from`] 与 [`Self::to`] 取值不同之前它不产生动画。
    pub fn new(period: Duration) -> Self {
        Self {
            period,
            from: Transformation::default(),
            to: Transformation::default(),
            scale_easing: linear,
            translation_easing: linear,
            rotation_easing: linear,
            alpha_easing: linear,
            alternate: false,
            min_frame_interval: None,
        }
    }

    /// 创建一个每 `period` 顺时针转一整圈的定时变换。
    pub fn rotation(period: Duration, from: Transformation) -> Self {
        use std::f32::consts::TAU;
        Self::new(period)
            .from(from)
            .to(from.with_rotation(radians(from.rotate.0 + TAU)))
    }

    /// 这个变换是否真的会让精灵动起来。
    pub(crate) fn is_animated(&self) -> bool {
        !self.period.is_zero() && self.from != self.to
    }

    /// 设定每个周期开始时的变换。
    pub fn from(mut self, transformation: Transformation) -> Self {
        self.from = transformation;
        self
    }

    /// 设定每个周期结束时的变换。
    pub fn to(mut self, transformation: Transformation) -> Self {
        self.to = transformation;
        self
    }

    /// 设定所有分量的缓动函数。
    pub fn with_easing(mut self, easing: EasingFn) -> Self {
        self.scale_easing = easing;
        self.translation_easing = easing;
        self.rotation_easing = easing;
        self.alpha_easing = easing;
        self
    }

    /// 设定缩放分量的缓动函数。
    pub fn with_scale_easing(mut self, easing: EasingFn) -> Self {
        self.scale_easing = easing;
        self
    }

    /// 设定平移分量的缓动函数。
    pub fn with_translation_easing(mut self, easing: EasingFn) -> Self {
        self.translation_easing = easing;
        self
    }

    /// 设定旋转分量的缓动函数。
    pub fn with_rotation_easing(mut self, easing: EasingFn) -> Self {
        self.rotation_easing = easing;
        self
    }

    /// 设定 alpha 分量的缓动函数。
    pub fn with_alpha_easing(mut self, easing: EasingFn) -> Self {
        self.alpha_easing = easing;
        self
    }

    /// 每个周期内从起点走到终点再走回来，而不是直接跳回起点。
    pub fn alternate(mut self) -> Self {
        self.alternate = true;
        self
    }

    /// 当只有定时变换需要出帧时，每秒最多呈现 `max_fps` 帧。
    ///
    /// 窗口里所有动画取最宽松的一档；非有限或非正数的值被忽略。
    pub fn with_max_fps(mut self, max_fps: f32) -> Self {
        if max_fps.is_finite() && max_fps > 0.0 {
            self.min_frame_interval = Some(Duration::from_secs_f64(1.0 / max_fps as f64));
        }
        self
    }

    /// 该变换要求的相邻呈现最小间隔（未设置时为 `None`）。
    pub(crate) fn min_frame_interval(&self) -> Option<Duration> {
        self.min_frame_interval
    }

    /// 动画时钟走过 `elapsed` 之后的变换。
    pub(crate) fn at(&self, elapsed: Duration) -> TimedTransformationPose {
        let period_nanos = self.period.as_nanos();
        if period_nanos == 0 {
            return TimedTransformationPose {
                transformation: self.from,
            };
        }
        let mut delta = Duration::from_nanos((elapsed.as_nanos() % period_nanos) as u64)
            .as_secs_f32()
            / self.period.as_secs_f32();
        if self.alternate {
            delta = 1.0 - (2.0 * delta - 1.0).abs();
        }

        let interpolate = |from: f32, to: f32, easing: EasingFn| from + (to - from) * easing(delta);
        let (from, to) = (self.from, self.to);
        TimedTransformationPose {
            transformation: Transformation {
                scale: size(
                    interpolate(from.scale.width, to.scale.width, self.scale_easing),
                    interpolate(from.scale.height, to.scale.height, self.scale_easing),
                ),
                translate: point(
                    px(interpolate(
                        from.translate.x.0,
                        to.translate.x.0,
                        self.translation_easing,
                    )),
                    px(interpolate(
                        from.translate.y.0,
                        to.translate.y.0,
                        self.translation_easing,
                    )),
                ),
                rotate: crate::radians(interpolate(
                    from.rotate.0,
                    to.rotate.0,
                    self.rotation_easing,
                )),
                alpha: interpolate(from.alpha, to.alpha, self.alpha_easing),
            },
        }
    }
}

impl From<Transformation> for TimedTransformation {
    /// 一个始终停在 `transformation` 的「定时变换」。
    fn from(transformation: Transformation) -> Self {
        Self::new(Duration::ZERO)
            .from(transformation)
            .to(transformation)
    }
}

/// 用于 external_path 的内部资源类型，直接从文件系统加载 SVG 数据。
enum SvgAsset {}

impl Asset for SvgAsset {
    type Source = SharedString;
    type Output = Result<Arc<[u8]>, Arc<std::io::Error>>;

    /// 从文件系统读取 SVG 文件内容。
    fn load(
        source: Self::Source,
        _cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        async move {
            let bytes = fs::read(Path::new(source.as_ref())).map_err(|e| Arc::new(e))?;
            let bytes = Arc::from(bytes);
            Ok(bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::size;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// 旋转：一圈 1s，四分之一时间转 90°；half-turn 与末帧回到起点。
    #[test]
    fn rotation_interpolates_and_wraps() {
        let animated =
            TimedTransformation::rotation(Duration::from_secs(1), Transformation::default());
        let quarter = animated.at(Duration::from_millis(250)).transformation;
        assert!(close(quarter.rotate.0, std::f32::consts::FRAC_PI_2));
        let half = animated.at(Duration::from_millis(500)).transformation;
        assert!(close(half.rotate.0, std::f32::consts::PI));
        // 一个周期后回到起点。
        let wrapped = animated.at(Duration::from_secs(1)).transformation;
        assert!(close(wrapped.rotate.0, 0.0));
        // 周期内不 animated 定义的两个变换相同 → is_animated false。
        assert!(!TimedTransformation::new(Duration::from_secs(1)).is_animated());
        assert!(animated.is_animated());
    }

    /// 缩放 + alpha + alternate：来回一个周期内到峰值再回起点。
    #[test]
    fn scale_alpha_and_alternate() {
        let animated = TimedTransformation::new(Duration::from_secs(2))
            .from(Transformation::alpha(1.0))
            .to(Transformation::scale(size(2.0, 2.0)).with_alpha(0.0))
            .alternate();
        // 交替：2s 周期的一半（1s）处到达峰值。
        let mid = animated.at(Duration::from_secs(1));
        assert!(close(mid.transformation.scale.width, 2.0));
        assert!(close(mid.alpha_factor(), 0.0));
        // 交替：一个周期末回到起点。
        let end = animated.at(Duration::from_secs(2));
        assert!(close(end.transformation.scale.width, 1.0));
        assert!(close(end.alpha_factor(), 1.0));
    }

    /// with_max_fps → min_frame_interval；非法值被忽略。
    #[test]
    fn max_fps_sets_min_frame_interval() {
        let capped = TimedTransformation::new(Duration::from_secs(1)).with_max_fps(15.0);
        let interval = capped.min_frame_interval().expect("15fps 应有间隔");
        assert!(close(interval.as_secs_f32(), 1.0 / 15.0));
        // 0 / 负数 / NaN 被忽略。
        assert!(
            TimedTransformation::new(Duration::from_secs(1))
                .with_max_fps(0.0)
                .min_frame_interval()
                .is_none()
        );
        assert!(
            TimedTransformation::new(Duration::from_secs(1))
                .with_max_fps(f32::NAN)
                .min_frame_interval()
                .is_none()
        );
    }

    /// at(period=0) 返回 from（无动画时静态）。
    #[test]
    fn zero_period_returns_from() {
        let still: TimedTransformation = Transformation::default().into();
        assert!(!still.is_animated());
        let pose = still.at(Duration::from_secs(10));
        assert!(close(pose.transformation.scale.width, 1.0));
    }
}
