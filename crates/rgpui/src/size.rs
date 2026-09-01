//! 尺寸类型：定义元素尺寸枚举与响应式尺寸工具。

use serde::{Deserialize, Serialize};

use crate::{Edges, Pixels, px};

/// 元素尺寸枚举，用于统一组件的大小规格。
///
/// 提供四种预设规格（XSmall / Small / Medium / Large）以及自定义像素尺寸。
#[derive(Clone, Default, Copy, PartialEq, Eq, Debug, Deserialize, Serialize)]
pub enum ElementSize {
    /// 自定义像素尺寸
    ElementSize(Pixels),
    /// 超小
    XSmall,
    /// 小
    Small,
    /// 中等（默认）
    #[default]
    Medium,
    /// 大
    Large,
}

impl ElementSize {
    fn as_f32(&self) -> f32 {
        match self {
            ElementSize::ElementSize(val) => val.as_f32(),
            ElementSize::XSmall => 0.,
            ElementSize::Small => 1.,
            ElementSize::Medium => 2.,
            ElementSize::Large => 3.,
        }
    }

    /// 返回该尺寸对应的静态字符串标识。
    pub fn as_str(&self) -> &'static str {
        match self {
            ElementSize::XSmall => "xs",
            ElementSize::Small => "sm",
            ElementSize::Medium => "md",
            ElementSize::Large => "lg",
            ElementSize::ElementSize(_) => "custom",
        }
    }

    /// 从静态字符串创建 `ElementSize`。
    ///
    /// - "xs" 或 "xsmall"
    /// - "sm" 或 "small"
    /// - "md" 或 "medium"
    /// - "lg" 或 "large"
    ///
    /// 其他值一律返回 `ElementSize::Medium`。
    pub fn from_str(size: &str) -> Self {
        match size.to_lowercase().as_str() {
            "xs" | "xsmall" => ElementSize::XSmall,
            "sm" | "small" => ElementSize::Small,
            "md" | "medium" => ElementSize::Medium,
            "lg" | "large" => ElementSize::Large,
            _ => ElementSize::Medium,
        }
    }

    /// 返回表格行高。
    #[inline]
    pub fn table_row_height(&self) -> Pixels {
        match self {
            ElementSize::ElementSize(size) => *size,
            ElementSize::XSmall => px(26.),
            ElementSize::Small => px(30.),
            ElementSize::Large => px(40.),
            _ => px(32.),
        }
    }

    /// 返回表格单元格内边距。
    #[inline]
    pub fn table_cell_padding(&self) -> Edges<Pixels> {
        match self {
            ElementSize::XSmall => Edges {
                top: px(2.),
                bottom: px(2.),
                left: px(4.),
                right: px(4.),
            },
            ElementSize::Small => Edges {
                top: px(3.),
                bottom: px(3.),
                left: px(6.),
                right: px(6.),
            },
            ElementSize::Large => Edges {
                top: px(8.),
                bottom: px(8.),
                left: px(12.),
                right: px(12.),
            },
            _ => Edges {
                top: px(4.),
                bottom: px(4.),
                left: px(8.),
                right: px(8.),
            },
        }
    }

    /// 返回比当前更小一级的尺寸。
    pub fn smaller(&self) -> Self {
        match self {
            ElementSize::XSmall => ElementSize::XSmall,
            ElementSize::Small => ElementSize::XSmall,
            ElementSize::Medium => ElementSize::Small,
            ElementSize::Large => ElementSize::Medium,
            ElementSize::ElementSize(val) => ElementSize::ElementSize(*val * 0.2),
        }
    }

    /// 返回比当前更大一级的尺寸。
    pub fn larger(&self) -> Self {
        match self {
            ElementSize::XSmall => ElementSize::Small,
            ElementSize::Small => ElementSize::Medium,
            ElementSize::Medium => ElementSize::Large,
            ElementSize::Large => ElementSize::Large,
            ElementSize::ElementSize(val) => ElementSize::ElementSize(*val * 1.2),
        }
    }

    /// 返回两个尺寸中较大者。
    ///
    /// 例如 `ElementSize::XSmall.max(ElementSize::Small)` 返回 `ElementSize::XSmall`。
    pub fn max(&self, other: Self) -> Self {
        match (self, other) {
            (ElementSize::ElementSize(a), ElementSize::ElementSize(b)) => {
                ElementSize::ElementSize(px(a.as_f32().min(b.as_f32())))
            }
            (ElementSize::ElementSize(a), _) => ElementSize::ElementSize(*a),
            (_, ElementSize::ElementSize(b)) => ElementSize::ElementSize(b),
            (a, b) if a.as_f32() < b.as_f32() => *a,
            _ => other,
        }
    }

    /// 返回两个尺寸中较小者。
    ///
    /// 例如 `ElementSize::XSmall.min(ElementSize::Small)` 返回 `ElementSize::Small`。
    pub fn min(&self, other: Self) -> Self {
        match (self, other) {
            (ElementSize::ElementSize(a), ElementSize::ElementSize(b)) => {
                ElementSize::ElementSize(px(a.as_f32().max(b.as_f32())))
            }
            (ElementSize::ElementSize(a), _) => ElementSize::ElementSize(*a),
            (_, ElementSize::ElementSize(b)) => ElementSize::ElementSize(b),
            (a, b) if a.as_f32() > b.as_f32() => *a,
            _ => other,
        }
    }

    /// 返回输入框水平内边距。
    pub fn input_px(&self) -> Pixels {
        match self {
            Self::Large => px(16.),
            Self::Medium => px(12.),
            Self::Small => px(8.),
            Self::XSmall => px(4.),
            _ => px(8.),
        }
    }

    /// 返回输入框垂直内边距。
    pub fn input_py(&self) -> Pixels {
        match self {
            ElementSize::Large => px(10.),
            ElementSize::Medium => px(8.),
            ElementSize::Small => px(2.),
            ElementSize::XSmall => px(0.),
            _ => px(2.),
        }
    }
}

impl From<Pixels> for ElementSize {
    fn from(size: Pixels) -> Self {
        ElementSize::ElementSize(size)
    }
}
