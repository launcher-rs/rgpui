use std::{fs, path::Path, sync::Arc};

use crate::ResultExt;
use crate::{
    App, Asset, Bounds, Element, GlobalElementId, Hitbox, InspectorElementId, InteractiveElement,
    Interactivity, IntoElement, LayoutId, Pixels, Point, Radians, SharedString, Size,
    StyleRefinement, Styled, TransformationMatrix, Window, point, px, radians, size,
};

/// SVG 元素，用于渲染 SVG 矢量图形。
pub struct Svg {
    /// 交互状态管理
    interactivity: Interactivity,
    /// 可选变换参数
    transformation: Option<Transformation>,
    /// 通过 AssetSource 加载的 SVG 路径
    path: Option<SharedString>,
    /// 直接文件系统路径（不经过 AssetSource）
    external_path: Option<SharedString>,
}

/// 创建一个新的 SVG 元素。
#[track_caller]
pub fn svg() -> Svg {
    Svg {
        interactivity: Interactivity::new(),
        transformation: None,
        path: None,
        external_path: None,
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

    /// 对 SVG 元素应用变换。
    /// 注意：这不会影响元素的 hitbox 或布局，仅影响渲染效果。
    pub fn with_transformation(mut self, transformation: Transformation) -> Self {
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
                if let Some((path, color)) = self.path.as_ref().zip(style.text.color) {
                    let transformation = self
                        .transformation
                        .as_ref()
                        .map(|transformation| {
                            transformation.into_matrix(bounds.center(), window.scale_factor())
                        })
                        .unwrap_or_default();

                    window
                        .paint_svg(bounds, path.clone(), None, transformation, color, cx)
                        .log_err();
                } else if let Some((path, color)) =
                    self.external_path.as_ref().zip(style.text.color)
                {
                    let Some(bytes) = window
                        .use_asset::<SvgAsset>(path, cx)
                        .and_then(|asset| asset.log_err())
                    else {
                        return;
                    };

                    let transformation = self
                        .transformation
                        .as_ref()
                        .map(|transformation| {
                            transformation.into_matrix(bounds.center(), window.scale_factor())
                        })
                        .unwrap_or_default();

                    window
                        .paint_svg(
                            bounds,
                            path.clone(),
                            Some(&bytes),
                            transformation,
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
}

impl Default for Transformation {
    fn default() -> Self {
        Self {
            scale: size(1.0, 1.0),
            translate: point(px(0.0), px(0.0)),
            rotate: radians(0.0),
        }
    }
}

impl Transformation {
    /// 创建一个只包含缩放的新变换。
    pub fn scale(scale: Size<f32>) -> Self {
        Self {
            scale,
            translate: point(px(0.0), px(0.0)),
            rotate: radians(0.0),
        }
    }

    /// 创建一个只包含平移的新变换。
    pub fn translate(translate: Point<Pixels>) -> Self {
        Self {
            scale: size(1.0, 1.0),
            translate,
            rotate: radians(0.0),
        }
    }

    /// 创建一个只包含旋转的新变换（弧度）。
    pub fn rotate(rotate: impl Into<Radians>) -> Self {
        let rotate = rotate.into();
        Self {
            scale: size(1.0, 1.0),
            translate: point(px(0.0), px(0.0)),
            rotate,
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

    /// 将变换转换为变换矩阵。从底部开始读矩阵乘法顺序。
    fn into_matrix(self, center: Point<Pixels>, scale_factor: f32) -> TransformationMatrix {
        // 注意：将矩阵乘法序列从底部开始阅读
        TransformationMatrix::unit()
            .translate(center.scale(scale_factor) + self.translate.scale(scale_factor))
            .rotate(self.rotate)
            .scale(self.scale)
            .translate(center.scale(-scale_factor))
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
