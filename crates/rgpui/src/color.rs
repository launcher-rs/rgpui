use anyhow::{Context as _, bail};
use schemars::{JsonSchema, json_schema};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, Visitor},
};
use std::borrow::Cow;
use std::{
    fmt::{self, Display, Formatter},
    hash::{Hash, Hasher},
};

/// 将 RGB 十六进制颜色代码转换为颜色类型
pub fn rgb(hex: u32) -> Rgba {
    let [_, r, g, b] = hex.to_be_bytes().map(|b| (b as f32) / 255.0);
    Rgba { r, g, b, a: 1.0 }
}

/// 将 RGBA 十六进制颜色代码转换为 [`Rgba`]
pub fn rgba(hex: u32) -> Rgba {
    let [r, g, b, a] = hex.to_be_bytes().map(|b| (b as f32) / 255.0);
    Rgba { r, g, b, a }
}

/// 从预乘 alpha 的 RGBA 转换为 BGRA
pub fn swap_rgba_pa_to_bgra(color: &mut [u8]) {
    color.swap(0, 2);
    if color[3] > 0 {
        let a = color[3] as f32 / 255.;
        color[0] = (color[0] as f32 / a) as u8;
        color[1] = (color[1] as f32 / a) as u8;
        color[2] = (color[2] as f32 / a) as u8;
    }
}

/// 一个 RGBA 颜色
#[derive(PartialEq, Clone, Copy, Default)]
#[repr(C)]
pub struct Rgba {
    /// 颜色的红色分量，范围为 0.0 到 1.0
    pub r: f32,
    /// 颜色的绿色分量，范围为 0.0 到 1.0
    pub g: f32,
    /// 颜色的蓝色分量，范围为 0.0 到 1.0
    pub b: f32,
    /// 颜色的透明度分量，范围为 0.0 到 1.0
    pub a: f32,
}

impl fmt::Debug for Rgba {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "rgba({:#010x})", u32::from(*self))
    }
}

impl Rgba {
    /// 通过将此颜色与另一颜色混合来创建新的 [`Rgba`] 颜色
    pub fn blend(&self, other: Rgba) -> Self {
        if other.a >= 1.0 {
            other
        } else if other.a <= 0.0 {
            *self
        } else {
            Rgba {
                r: (self.r * (1.0 - other.a)) + (other.r * other.a),
                g: (self.g * (1.0 - other.a)) + (other.g * other.a),
                b: (self.b * (1.0 - other.a)) + (other.b * other.a),
                a: self.a,
            }
        }
    }

    /// 返回一个具有相同红、绿、蓝通道但具有新 alpha 值的 RGBA 颜色。
    pub fn alpha(&self, a: f32) -> Self {
        Rgba {
            r: self.r,
            g: self.g,
            b: self.b,
            a: a.clamp(0., 1.),
        }
    }

    /// 返回一个具有相同红、绿、蓝通道但 alpha 通道乘以给定因子的 RGBA 颜色。
    pub fn opacity(&self, factor: f32) -> Self {
        Rgba {
            r: self.r,
            g: self.g,
            b: self.b,
            a: self.a * factor.clamp(0., 1.),
        }
    }
}

impl From<Rgba> for u32 {
    fn from(rgba: Rgba) -> Self {
        let r = (rgba.r * 255.0) as u32;
        let g = (rgba.g * 255.0) as u32;
        let b = (rgba.b * 255.0) as u32;
        let a = (rgba.a * 255.0) as u32;
        (r << 24) | (g << 16) | (b << 8) | a
    }
}

struct RgbaVisitor;

impl Visitor<'_> for RgbaVisitor {
    type Value = Rgba;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("格式为 #rrggbb 或 #rrggbbaa 的字符串")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Rgba, E> {
        Rgba::try_from(value).map_err(E::custom)
    }
}

impl JsonSchema for Rgba {
    fn schema_name() -> Cow<'static, str> {
        "Rgba".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        json_schema!({
            "type": "string",
            "pattern": "^#([0-9a-fA-F]{3}|[0-9a-fA-F]{4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$"
        })
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(RgbaVisitor)
    }
}

impl Serialize for Rgba {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let r = (self.r * 255.0).round() as u8;
        let g = (self.g * 255.0).round() as u8;
        let b = (self.b * 255.0).round() as u8;
        let a = (self.a * 255.0).round() as u8;

        let s = format!("#{r:02x}{g:02x}{b:02x}{a:02x}");
        serializer.serialize_str(&s)
    }
}

impl From<Hsla> for Rgba {
    fn from(color: Hsla) -> Self {
        let h = color.h;
        let s = color.s;
        let l = color.l;

        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
        let m = l - c / 2.0;
        let cm = c + m;
        let xm = x + m;

        let (r, g, b) = match (h * 6.0).floor() as i32 {
            0 | 6 => (cm, xm, m),
            1 => (xm, cm, m),
            2 => (m, cm, xm),
            3 => (m, xm, cm),
            4 => (xm, m, cm),
            _ => (cm, m, xm),
        };

        Rgba {
            r: r.clamp(0., 1.),
            g: g.clamp(0., 1.),
            b: b.clamp(0., 1.),
            a: color.a,
        }
    }
}

impl TryFrom<&'_ str> for Rgba {
    type Error = anyhow::Error;

    fn try_from(value: &'_ str) -> Result<Self, Self::Error> {
        const RGB: usize = "rgb".len();
        const RGBA: usize = "rgba".len();
        const RRGGBB: usize = "rrggbb".len();
        const RRGGBBAA: usize = "rrggbbaa".len();

        const EXPECTED_FORMATS: &str = "Expected #rgb, #rgba, #rrggbb, or #rrggbbaa";
        const INVALID_UNICODE: &str = "invalid unicode characters in color";

        let Some(("", hex)) = value.trim().split_once('#') else {
            bail!("invalid RGBA hex color: '{value}'. {EXPECTED_FORMATS}");
        };

        let (r, g, b, a) = match hex.len() {
            RGB | RGBA => {
                let r = u8::from_str_radix(
                    hex.get(0..1).with_context(|| {
                        format!("{INVALID_UNICODE}: r component of #rgb/#rgba for value: '{value}'")
                    })?,
                    16,
                )?;
                let g = u8::from_str_radix(
                    hex.get(1..2).with_context(|| {
                        format!("{INVALID_UNICODE}: g component of #rgb/#rgba for value: '{value}'")
                    })?,
                    16,
                )?;
                let b = u8::from_str_radix(
                    hex.get(2..3).with_context(|| {
                        format!("{INVALID_UNICODE}: b component of #rgb/#rgba for value: '{value}'")
                    })?,
                    16,
                )?;
                let a = if hex.len() == RGBA {
                    u8::from_str_radix(
                        hex.get(3..4).with_context(|| {
                            format!("{INVALID_UNICODE}: a component of #rgba for value: '{value}'")
                        })?,
                        16,
                    )?
                } else {
                    0xf
                };

                /// 复制给定的十六进制数字。
                /// 例如，`0xf` -> `0xff`。
                const fn duplicate(value: u8) -> u8 {
                    (value << 4) | value
                }

                (duplicate(r), duplicate(g), duplicate(b), duplicate(a))
            }
            RRGGBB | RRGGBBAA => {
                let r = u8::from_str_radix(
                    hex.get(0..2).with_context(|| {
                        format!(
                            "{}: r component of #rrggbb/#rrggbbaa for value: '{}'",
                            INVALID_UNICODE, value
                        )
                    })?,
                    16,
                )?;
                let g = u8::from_str_radix(
                    hex.get(2..4).with_context(|| {
                        format!(
                            "{INVALID_UNICODE}: g component of #rrggbb/#rrggbbaa for value: '{value}'"
                        )
                    })?,
                    16,
                )?;
                let b = u8::from_str_radix(
                    hex.get(4..6).with_context(|| {
                        format!(
                            "{INVALID_UNICODE}: b component of #rrggbb/#rrggbbaa for value: '{value}'"
                        )
                    })?,
                    16,
                )?;
                let a = if hex.len() == RRGGBBAA {
                    u8::from_str_radix(
                        hex.get(6..8).with_context(|| {
                            format!(
                                "{INVALID_UNICODE}: a component of #rrggbbaa for value: '{value}'"
                            )
                        })?,
                        16,
                    )?
                } else {
                    0xff
                };
                (r, g, b, a)
            }
            _ => bail!("invalid RGBA hex color: '{value}'. {EXPECTED_FORMATS}"),
        };

        Ok(Rgba {
            r: r as f32 / 255.,
            g: g as f32 / 255.,
            b: b as f32 / 255.,
            a: a as f32 / 255.,
        })
    }
}

/// 一个 HSLA 颜色
#[derive(Default, Copy, Clone, Debug)]
#[repr(C)]
pub struct Hsla {
    /// Hue, in a range from 0 to 1
    pub h: f32,

    /// Saturation, in a range from 0 to 1
    pub s: f32,

    /// Lightness, in a range from 0 to 1
    pub l: f32,

    /// Alpha, in a range from 0 to 1
    pub a: f32,
}

#[cfg(feature = "proptest")]
mod property {
    use super::Hsla;
    use proptest::prelude::*;

    impl Hsla {
        /// Proptest [`Strategy`] that produces opaque colors (i.e. alpha = 1).
        ///
        /// For truly arbitrary colors, use the [`Arbitrary`] implementation.
        pub fn opaque_strategy() -> impl Strategy<Value = Self> {
            (0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0).prop_map(|(h, s, l)| Hsla { h, s, l, a: 1. })
        }
    }

    impl Arbitrary for Hsla {
        type Strategy = BoxedStrategy<Self>;
        type Parameters = ();

        fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
            (0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0)
                .prop_map(|(h, s, l, a)| Hsla { h, s, l, a })
                .boxed()
        }
    }
}

impl PartialEq for Hsla {
    fn eq(&self, other: &Self) -> bool {
        self.h
            .total_cmp(&other.h)
            .then(self.s.total_cmp(&other.s))
            .then(self.l.total_cmp(&other.l).then(self.a.total_cmp(&other.a)))
            .is_eq()
    }
}

impl PartialOrd for Hsla {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Hsla {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.h
            .total_cmp(&other.h)
            .then(self.s.total_cmp(&other.s))
            .then(self.l.total_cmp(&other.l).then(self.a.total_cmp(&other.a)))
    }
}

impl Eq for Hsla {}

impl Hash for Hsla {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(u32::from_be_bytes(self.h.to_be_bytes()));
        state.write_u32(u32::from_be_bytes(self.s.to_be_bytes()));
        state.write_u32(u32::from_be_bytes(self.l.to_be_bytes()));
        state.write_u32(u32::from_be_bytes(self.a.to_be_bytes()));
    }
}

impl Display for Hsla {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "hsla({:.2}, {:.2}%, {:.2}%, {:.2})",
            self.h * 360.,
            self.s * 100.,
            self.l * 100.,
            self.a
        )
    }
}

/// 使用普通值构造 [`Hsla`] 对象
pub fn hsla(h: f32, s: f32, l: f32, a: f32) -> Hsla {
    Hsla {
        h: h.clamp(0., 1.),
        s: s.clamp(0., 1.),
        l: l.clamp(0., 1.),
        a: a.clamp(0., 1.),
    }
}

/// [`Hsla`] 中的纯黑色
pub const fn black() -> Hsla {
    Hsla {
        h: 0.,
        s: 0.,
        l: 0.,
        a: 1.,
    }
}

/// [`Hsla`] 中的透明黑色
pub const fn transparent_black() -> Hsla {
    Hsla {
        h: 0.,
        s: 0.,
        l: 0.,
        a: 0.,
    }
}

/// [`Hsla`] 中的透明白色
pub const fn transparent_white() -> Hsla {
    Hsla {
        h: 0.,
        s: 0.,
        l: 1.,
        a: 0.,
    }
}

/// [`Hsla`] 中的不透明灰色，值将被限制在 [0, 1] 范围内
pub fn opaque_grey(lightness: f32, opacity: f32) -> Hsla {
    Hsla {
        h: 0.,
        s: 0.,
        l: lightness.clamp(0., 1.),
        a: opacity.clamp(0., 1.),
    }
}

/// [`Hsla`] 中的纯白色
pub const fn white() -> Hsla {
    Hsla {
        h: 0.,
        s: 0.,
        l: 1.,
        a: 1.,
    }
}

/// [`Hsla`] 中的红色
pub const fn red() -> Hsla {
    Hsla {
        h: 0.,
        s: 1.,
        l: 0.5,
        a: 1.,
    }
}

/// [`Hsla`] 中的蓝色
pub const fn blue() -> Hsla {
    Hsla {
        h: 0.6666666667,
        s: 1.,
        l: 0.5,
        a: 1.,
    }
}

/// [`Hsla`] 中的绿色
pub const fn green() -> Hsla {
    Hsla {
        h: 0.3333333333,
        s: 1.,
        l: 0.25,
        a: 1.,
    }
}

/// [`Hsla`] 中的黄色
pub const fn yellow() -> Hsla {
    Hsla {
        h: 0.1666666667,
        s: 1.,
        l: 0.5,
        a: 1.,
    }
}

impl Hsla {
    /// Converts this HSLA color to an RGBA color.
    pub fn to_rgb(self) -> Rgba {
        self.into()
    }

    /// 红色
    pub const fn red() -> Self {
        red()
    }

    /// 绿色
    pub const fn green() -> Self {
        green()
    }

    /// 蓝色
    pub const fn blue() -> Self {
        blue()
    }

    /// 黑色
    pub const fn black() -> Self {
        black()
    }

    /// 白色
    pub const fn white() -> Self {
        white()
    }

    /// 透明黑色
    pub const fn transparent_black() -> Self {
        transparent_black()
    }

    /// 如果 HSLA 颜色完全透明则返回 true，否则返回 false。
    pub fn is_transparent(&self) -> bool {
        self.a == 0.0
    }

    /// 如果 HSLA 颜色完全不透明则返回 true，否则返回 false。
    pub fn is_opaque(&self) -> bool {
        self.a == 1.0
    }

    /// 基于 `other` 的透明度值，将 `other` 混合到 `self` 之上。结果颜色是 `self` 和 `other` 颜色的组合。
    ///
    /// 如果 `other` 的透明度值为 1.0 或更大，`other` 颜色完全不透明，因此返回 `other` 作为输出颜色。
    /// 如果 `other` 的透明度值为 0.0 或更小，`other` 颜色完全透明，因此返回 `self` 作为输出颜色。
    /// 否则，输出颜色基于 `self` 和 `other` 的加权透明度值计算混合结果。
    ///
    /// 假设：
    /// - 透明度值包含在 [0, 1] 范围内，1 表示完全不透明，0 表示完全透明。
    /// - `self` 和 `other` 的相对贡献基于 `self` 的透明度值 (`self.a`) 和 `other` 的透明度值 (`other.a`)，`self` 贡献 `self.a * (1.0 - other.a)`，`other` 贡献其自身的透明度值。
    /// - RGB 颜色分量包含在 [0, 1] 范围内。
    /// - 如果 `self` 和 `other` 颜色超出有效范围，混合操作的输出和行为未定义。
    pub fn blend(self, other: Hsla) -> Hsla {
        let alpha = other.a;

        if alpha >= 1.0 {
            other
        } else if alpha <= 0.0 {
            self
        } else {
            let converted_self = Rgba::from(self);
            let converted_other = Rgba::from(other);
            let blended_rgb = converted_self.blend(converted_other);
            Hsla::from(blended_rgb)
        }
    }

    /// 返回一个新的 HSLA 颜色，具有相同的色相和亮度，但无饱和度。
    pub fn grayscale(&self) -> Self {
        Hsla {
            h: self.h,
            s: 0.,
            l: self.l,
            a: self.a,
        }
    }

    /// 按给定因子淡化颜色。该因子应在 0.0 到 1.0 之间。
    /// 其中 0.0 将使颜色保持不变，1.0 将完全淡化颜色。
    pub fn fade_out(&mut self, factor: f32) {
        self.a *= 1.0 - factor.clamp(0., 1.);
    }

    /// 将颜色的透明度值乘以给定因子，
    /// 并返回新的 HSLA 颜色。
    ///
    /// 适用于转换带有动态透明度的颜色，
    /// 例如来自外部源的颜色。
    ///
    /// 示例：
    /// ```
    /// let color = rgpui::red();
    /// let faded_color = color.opacity(0.5);
    /// assert_eq!(faded_color.a, 0.5);
    /// ```
    ///
    /// 这将返回透明度减半的红色。
    ///
    /// 示例：
    /// ```
    /// use rgpui::hsla;
    /// let color = hsla(0.7, 1.0, 0.5, 0.7); // 饱和的蓝色
    /// let faded_color = color.opacity(0.16);
    /// assert!((faded_color.a - 0.112).abs() < 1e-6);
    /// ```
    ///
    /// 这将返回透明度约为 ~10% 的蓝色，
    /// 适用于元素的悬停或选中状态。
    ///
    pub fn opacity(&self, factor: f32) -> Self {
        Hsla {
            h: self.h,
            s: self.s,
            l: self.l,
            a: self.a * factor.clamp(0., 1.),
        }
    }

    /// 返回一个新的 HSLA 颜色，具有相同的色相、饱和度
    /// 和亮度，但具有新的透明度值。
    ///
    /// 示例：
    /// ```
    /// let color = rgpui::red();
    /// let red_color = color.alpha(0.25);
    /// assert_eq!(red_color.a, 0.25);
    /// ```
    ///
    /// 这将返回透明度减半的红色。
    ///
    /// 示例：
    /// ```
    /// use rgpui::hsla;
    /// let color = hsla(0.7, 1.0, 0.5, 0.7); // 饱和的蓝色
    /// let faded_color = color.alpha(0.25);
    /// assert_eq!(faded_color.a, 0.25);
    /// ```
    ///
    /// 这将返回透明度为 25% 的蓝色。
    pub fn alpha(&self, a: f32) -> Self {
        Hsla {
            h: self.h,
            s: self.s,
            l: self.l,
            a: a.clamp(0., 1.),
        }
    }
}

impl From<Rgba> for Hsla {
    fn from(color: Rgba) -> Self {
        let r = color.r;
        let g = color.g;
        let b = color.b;

        let max = r.max(g.max(b));
        let min = r.min(g.min(b));
        let delta = max - min;

        let l = (max + min) / 2.0;
        let s = if l == 0.0 || l == 1.0 {
            0.0
        } else if l < 0.5 {
            delta / (2.0 * l)
        } else {
            delta / (2.0 - 2.0 * l)
        };

        let h = if delta == 0.0 {
            0.0
        } else if max == r {
            ((g - b) / delta).rem_euclid(6.0) / 6.0
        } else if max == g {
            ((b - r) / delta + 2.0) / 6.0
        } else {
            ((r - g) / delta + 4.0) / 6.0
        };

        Hsla {
            h,
            s,
            l,
            a: color.a,
        }
    }
}

impl JsonSchema for Hsla {
    fn schema_name() -> Cow<'static, str> {
        Rgba::schema_name()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        Rgba::json_schema(generator)
    }
}

impl Serialize for Hsla {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Rgba::from(*self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Hsla {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Rgba::deserialize(deserializer)?.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[repr(u32)]
pub(crate) enum BackgroundTag {
    Solid = 0,
    LinearGradient = 1,
    PatternSlash = 2,
    Checkerboard = 3,
    RadialGradient = 4,
    ConicGradient = 5,
}

/// 用于颜色插值的颜色空间。
///
/// 参考:
/// - <https://developer.mozilla.org/en-US/docs/Web/CSS/color-interpolation-method>
/// - <https://www.w3.org/TR/css-color-4/#typedef-color-space>
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[repr(C)]
pub enum ColorSpace {
    #[default]
    /// The sRGB color space.
    Srgb = 0,
    /// The Oklab color space.
    Oklab = 1,
}

impl Display for ColorSpace {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ColorSpace::Srgb => write!(f, "sRGB"),
            ColorSpace::Oklab => write!(f, "Oklab"),
        }
    }
}

/// 背景颜色，可以是纯色或线性渐变。
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[repr(C, align(8))]
pub struct Background {
    pub(crate) tag: BackgroundTag,
    pub(crate) color_space: ColorSpace,
    pub(crate) solid: Hsla,
    pub(crate) gradient_angle_or_pattern_height: f32,
    pub(crate) colors: [LinearColorStop; 4],
    pub(crate) stop_count: u32,
    /// Padding for alignment for repr(C) layout.
    pad: u32,
    /// 对齐到 WGSL vec2<f32>（对齐要求 8）
    align_pad: u32,
    pub(crate) center: [f32; 2],
    pub(crate) radius: [f32; 2],
}

impl std::fmt::Debug for Background {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.tag {
            BackgroundTag::Solid => write!(f, "Solid({:?})", self.solid),
            BackgroundTag::LinearGradient => {
                let count = if self.stop_count == 0 {
                    2
                } else {
                    self.stop_count as usize
                };
                write!(
                    f,
                    "LinearGradient({}, {:?})",
                    self.gradient_angle_or_pattern_height,
                    &self.colors[..count]
                )
            }
            BackgroundTag::PatternSlash => write!(
                f,
                "PatternSlash({:?}, {})",
                self.solid, self.gradient_angle_or_pattern_height
            ),
            BackgroundTag::Checkerboard => write!(
                f,
                "Checkerboard({:?}, {})",
                self.solid, self.gradient_angle_or_pattern_height
            ),
            BackgroundTag::RadialGradient => {
                let count = if self.stop_count == 0 {
                    2
                } else {
                    self.stop_count as usize
                };
                write!(
                    f,
                    "RadialGradient(center={:?}, radius={:?}, {:?})",
                    self.center,
                    self.radius,
                    &self.colors[..count]
                )
            }
            BackgroundTag::ConicGradient => {
                let count = if self.stop_count == 0 {
                    2
                } else {
                    self.stop_count as usize
                };
                write!(
                    f,
                    "ConicGradient(center={:?}, angle={}, {:?})",
                    self.center,
                    self.gradient_angle_or_pattern_height,
                    &self.colors[..count]
                )
            }
        }
    }
}

impl Eq for Background {}
impl Default for Background {
    fn default() -> Self {
        Self {
            tag: BackgroundTag::Solid,
            solid: Hsla::default(),
            color_space: ColorSpace::default(),
            gradient_angle_or_pattern_height: 0.0,
            colors: [LinearColorStop::default(); 4],
            stop_count: 0,
            pad: 0,
            align_pad: 0,
            center: [0.5, 0.5],
            radius: [0.5, 0.5],
        }
    }
}

/// 创建哈希图案背景
pub fn pattern_slash(color: impl Into<Hsla>, width: f32, interval: f32) -> Background {
    let width_scaled = (width * 255.0) as u32;
    let interval_scaled = (interval * 255.0) as u32;
    let height = ((width_scaled * 0xFFFF) + interval_scaled) as f32;

    Background {
        tag: BackgroundTag::PatternSlash,
        solid: color.into(),
        gradient_angle_or_pattern_height: height,
        ..Default::default()
    }
}

/// 创建棋盘图案背景
pub fn checkerboard(color: impl Into<Hsla>, size: f32) -> Background {
    Background {
        tag: BackgroundTag::Checkerboard,
        solid: color.into(),
        gradient_angle_or_pattern_height: size,
        ..Default::default()
    }
}

/// 创建纯色背景颜色。
pub fn solid_background(color: impl Into<Hsla>) -> Background {
    Background {
        solid: color.into(),
        ..Default::default()
    }
}

/// 创建 LinearGradient 背景颜色。
///
/// 渐变线的方向角度。`0.` 的值等效于顶部；增加的值从那里顺时针旋转。
///
/// `angle` 是以度为单位的值，范围为 0.0 到 360.0。
///
/// <https://developer.mozilla.org/en-US/docs/Web/CSS/gradient/linear-gradient>
pub fn linear_gradient(
    angle: f32,
    from: impl Into<LinearColorStop>,
    to: impl Into<LinearColorStop>,
) -> Background {
    Background {
        tag: BackgroundTag::LinearGradient,
        gradient_angle_or_pattern_height: angle,
        colors: [
            from.into(),
            to.into(),
            LinearColorStop::default(),
            LinearColorStop::default(),
        ],
        stop_count: 2,
        ..Default::default()
    }
}

/// 创建最多 4 个颜色停止点的线性渐变。
pub fn multi_stop_linear_gradient(angle: f32, stops: &[LinearColorStop]) -> Background {
    let mut colors = [LinearColorStop::default(); 4];
    let count = stops.len().min(4);
    colors[..count].copy_from_slice(&stops[..count]);
    Background {
        tag: BackgroundTag::LinearGradient,
        gradient_angle_or_pattern_height: angle,
        colors,
        stop_count: count as u32,
        ..Default::default()
    }
}

/// 创建径向渐变背景。
///
/// `center_x` 和 `center_y` 是渐变中心点的归一化坐标（0.0 到 1.0）。
/// `radius` 是渐变的半径，使用归一化坐标（0.0 到 1.0）。
/// 最多支持 4 个颜色停止点。
pub fn radial_gradient(
    center_x: f32,
    center_y: f32,
    radius: f32,
    stops: &[LinearColorStop],
) -> Background {
    let mut colors = [LinearColorStop::default(); 4];
    let count = stops.len().min(4);
    colors[..count].copy_from_slice(&stops[..count]);
    Background {
        tag: BackgroundTag::RadialGradient,
        colors,
        stop_count: count as u32,
        center: [center_x, center_y],
        radius: [radius, radius],
        ..Default::default()
    }
}

/// 创建锥形（扫描）渐变背景。
///
/// `center_x` 和 `center_y` 是渐变中心点的归一化坐标（0.0 到 1.0）。
/// `angle_offset` 是起始角度偏移量（度数）。
/// 最多支持 4 个颜色停止点。
pub fn conic_gradient(
    center_x: f32,
    center_y: f32,
    angle_offset: f32,
    stops: &[LinearColorStop],
) -> Background {
    let mut colors = [LinearColorStop::default(); 4];
    let count = stops.len().min(4);
    colors[..count].copy_from_slice(&stops[..count]);
    Background {
        tag: BackgroundTag::ConicGradient,
        gradient_angle_or_pattern_height: angle_offset,
        colors,
        stop_count: count as u32,
        center: [center_x, center_y],
        ..Default::default()
    }
}

/// 线性渐变中的颜色停止点。
///
/// <https://developer.mozilla.org/en-US/docs/Web/CSS/gradient/linear-gradient#linear-color-stop>
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[repr(C)]
pub struct LinearColorStop {
    /// The color of the color stop.
    pub color: Hsla,
    /// The percentage of the gradient, in the range 0.0 to 1.0.
    pub percentage: f32,
}

/// 创建新的线性颜色停止点。
///
/// 渐变的百分比，范围为 0.0 到 1.0。
pub fn linear_color_stop(color: impl Into<Hsla>, percentage: f32) -> LinearColorStop {
    LinearColorStop {
        color: color.into(),
        percentage,
    }
}

impl LinearColorStop {
    /// 返回一个新的颜色停止点，颜色相同，但透明度值已修改。
    pub fn opacity(&self, factor: f32) -> Self {
        Self {
            percentage: self.percentage,
            color: self.color.opacity(factor),
        }
    }
}

impl Background {
    /// 如果是纯色背景则返回纯色，否则返回 None。
    pub fn as_solid(&self) -> Option<Hsla> {
        if self.tag == BackgroundTag::Solid {
            Some(self.solid)
        } else {
            None
        }
    }

    /// Use specified color space for color interpolation.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/color-interpolation-method>
    pub fn color_space(mut self, color_space: ColorSpace) -> Self {
        self.color_space = color_space;
        self
    }

    /// 返回一个新的背景颜色，具有相同的色相、饱和度和亮度，但透明度值已修改。
    pub fn opacity(&self, factor: f32) -> Self {
        let mut background = *self;
        background.solid = background.solid.opacity(factor);
        background.colors = [
            self.colors[0].opacity(factor),
            self.colors[1].opacity(factor),
            self.colors[2].opacity(factor),
            self.colors[3].opacity(factor),
        ];
        background
    }

    /// 返回背景颜色是否透明。
    pub fn is_transparent(&self) -> bool {
        match self.tag {
            BackgroundTag::Solid => self.solid.is_transparent(),
            BackgroundTag::LinearGradient
            | BackgroundTag::RadialGradient
            | BackgroundTag::ConicGradient => self.colors[..self.stop_count as usize]
                .iter()
                .all(|c| c.color.is_transparent()),
            BackgroundTag::PatternSlash => self.solid.is_transparent(),
            BackgroundTag::Checkerboard => self.solid.is_transparent(),
        }
    }
}

impl From<Hsla> for Background {
    fn from(value: Hsla) -> Self {
        Background {
            tag: BackgroundTag::Solid,
            solid: value,
            ..Default::default()
        }
    }
}
impl From<Rgba> for Background {
    fn from(value: Rgba) -> Self {
        Background {
            tag: BackgroundTag::Solid,
            solid: Hsla::from(value),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_deserialize_three_value_hex_to_rgba() {
        let actual: Rgba = serde_json::from_value(json!("#f09")).unwrap();

        assert_eq!(actual, rgba(0xff0099ff))
    }

    #[test]
    fn test_deserialize_four_value_hex_to_rgba() {
        let actual: Rgba = serde_json::from_value(json!("#f09f")).unwrap();

        assert_eq!(actual, rgba(0xff0099ff))
    }

    #[test]
    fn test_deserialize_six_value_hex_to_rgba() {
        let actual: Rgba = serde_json::from_value(json!("#ff0099")).unwrap();

        assert_eq!(actual, rgba(0xff0099ff))
    }

    #[test]
    fn test_deserialize_eight_value_hex_to_rgba() {
        let actual: Rgba = serde_json::from_value(json!("#ff0099ff")).unwrap();

        assert_eq!(actual, rgba(0xff0099ff))
    }

    #[test]
    fn test_deserialize_eight_value_hex_with_padding_to_rgba() {
        let actual: Rgba = serde_json::from_value(json!(" #f5f5f5ff   ")).unwrap();

        assert_eq!(actual, rgba(0xf5f5f5ff))
    }

    #[test]
    fn test_deserialize_eight_value_hex_with_mixed_case_to_rgba() {
        let actual: Rgba = serde_json::from_value(json!("#DeAdbEeF")).unwrap();

        assert_eq!(actual, rgba(0xdeadbeef))
    }

    #[test]
    fn test_background_solid() {
        let color = Hsla::from(rgba(0xff0099ff));
        let mut background = Background::from(color);
        assert_eq!(background.tag, BackgroundTag::Solid);
        assert_eq!(background.solid, color);

        assert_eq!(background.opacity(0.5).solid, color.opacity(0.5));
        assert!(!background.is_transparent());
        background.solid = hsla(0.0, 0.0, 0.0, 0.0);
        assert!(background.is_transparent());
    }

    #[test]
    fn test_background_linear_gradient() {
        let from = linear_color_stop(rgba(0xff0099ff), 0.0);
        let to = linear_color_stop(rgba(0x00ff99ff), 1.0);
        let background = linear_gradient(90.0, from, to);
        assert_eq!(background.tag, BackgroundTag::LinearGradient);
        assert_eq!(background.colors[0], from);
        assert_eq!(background.colors[1], to);

        assert_eq!(background.opacity(0.5).colors[0], from.opacity(0.5));
        assert_eq!(background.opacity(0.5).colors[1], to.opacity(0.5));
        assert!(!background.is_transparent());
        assert!(background.opacity(0.0).is_transparent());
    }
}
