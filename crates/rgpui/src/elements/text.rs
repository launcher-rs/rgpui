use crate::rgpui_util::ResultExt;
use crate::{
    ActiveTooltip, AnyView, App, Bounds, DispatchPhase, Element, ElementId, GlobalElementId,
    HighlightStyle, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, LayoutId,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, SharedString, Size, TextOverflow,
    TextRun, TextStyle, TooltipId, TruncateFrom, WhiteSpace, Window, WrappedLine,
    WrappedLineLayout, register_tooltip_mouse_handlers, set_tooltip_on_window,
};
use anyhow::Context as _;
use itertools::Itertools;
use smallvec::SmallVec;
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    mem,
    ops::{Deref, DerefMut, Range},
    rc::Rc,
    sync::Arc,
};

/// 渲染文本的 [`Element`]。
///
/// 通常应通过 [`text`] 宏创建 [`Text`] 对象：
/// ```rust
/// # use rgpui::*;
/// # fn render() -> impl IntoElement {
/// div().child(text!("hello"))
/// # }
/// ```
/// ## ID 与无障碍性
///
/// [`Text`] 元素有一个 ID。此 ID 主要用于在无障碍树中生成节点，
/// 使文本对屏幕阅读器和其他辅助技术可见。
///
/// 此 ID 在帧之间是稳定的。如果相同文本（具有相同 ID）出现在
/// 连续两帧中，则不会向屏幕阅读器报告更新。如果文本更改但 ID 保持不变，
/// 屏幕阅读器将收到文本节点内容已更改的通知。**但是**，
/// 如果 ID 更改，屏幕阅读器将收到节点已删除且新节点已添加的通知。
///
/// 使用 [`text`] 宏时，每次宏调用都会获得一个唯一 ID，
/// 从其在源代码中的位置（文件名、行号和列号）派生。例如：
/// ```rust
/// # use rgpui::*;
/// let x = text!("hello");
/// let y = text!("hello");
/// // 不相等，因为不同的 `text!` 调用产生了它们
/// assert_ne!(x.id(), y.id());
///
/// fn make_text(s: &str) -> Text { text!(s) }
/// let x = make_text("hello");
/// let y = make_text("hello");
/// // 相等，因为相同的 `text!` 调用产生了它们
/// assert_eq!(x.id(), y.id());
/// ```
/// 当 [`text`] 调用的内容未更改时，此区分不太相关
/// （但需要注意确保不出现重复 ID）。
///
/// 然而，当 [`text`] 调用的参数*确实*更改时，应考虑
/// 此更改应报告为节点"更新其内容"，还是旧节点被销毁且新节点被创建。
#[derive(Debug, Clone)]
pub struct Text {
    id: Option<ElementId>,
    text: SharedString,
}

impl Text {
    /// 使用特定 ID 创建一个新的 [`Text`] 元素。
    ///
    /// 如果希望自动分配唯一 ID，请使用 [`text`] 宏。
    /// [`Text`] 的文档有关于选择 ID 的更多详情。
    #[inline]
    pub const fn new(id: ElementId, text: SharedString) -> Self {
        Self { id: Some(id), text }
    }

    /// 创建一个对屏幕阅读器不可访问的新 [`Text`] 元素。
    ///
    /// 为了使文本对屏幕阅读器可访问，必须提供 ID。
    /// 如果希望文本可访问，请使用 [`text`] 自动分配 ID，
    /// 或使用 [`Text::new`] 手动分配 ID。
    ///
    /// 此函数适用于自定义 UI 组件内部，
    /// 其中无障碍属性可能在父容器上设置。
    #[inline]
    pub const fn new_inaccessible(text: SharedString) -> Self {
        Self { id: None, text }
    }

    /// 此 [`Text`] 元素的 ID。
    #[inline]
    pub const fn id(&self) -> Option<&ElementId> {
        self.id.as_ref()
    }

    /// 使用给定的 `id` 生成新的 [`Text`]。
    pub fn with_id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// 此 [`Text`] 元素将显示的文本。
    #[inline]
    pub const fn text(&self) -> &SharedString {
        &self.text
    }
}

impl Deref for Text {
    type Target = SharedString;
    fn deref(&self) -> &Self::Target {
        &self.text
    }
}

impl DerefMut for Text {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.text
    }
}

/// [`text`] 宏产生的位置信息的简单哈希函数。不在语义版本控制保证范围内。
/// 性能不是特别重要，因为它仅在 const 上下文中用于小字符串。
#[doc(hidden)]
pub const fn __hash_text_macro_location_unstable_do_not_use(s: &'static str) -> u64 {
    const BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    let bytes = s.as_bytes();
    let mut hash = BASIS;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(PRIME);
        i += 1;
    }
    hash
}

/// 创建一个新的 [`Text`] 元素。
///
/// ```rust
/// # use rgpui::*;
/// let a = text!("hello");
/// let b = text!(id = "farewell-message", "hello");
///
/// ```
///
/// 使用此宏创建的文本是*可访问的*。该宏基于源位置生成 ID。
/// 有关 [`Text`] 元素 ID 重要性的更深入解释，请参阅 [`Text`] 的文档。
#[macro_export]
macro_rules! text {
    (id = $id:expr, $text:expr) => {{ $crate::Text::new($id.into(), $text.into()) }};
    ($text:expr) => {{
        const ID: &'static str = concat!(file!(), "/", line!(), ":", column!());
        const HASH: u64 = $crate::__hash_text_macro_location_unstable_do_not_use(ID);
        $crate::Text::new($crate::ElementId::Integer(HASH), $text.into())
    }};
}

impl IntoElement for Text {
    type Element = Self;
    #[inline]
    fn into_element(self) -> Self::Element {
        self
    }
}

/// Web DOM 后端：把一个文本节点映射为 `<span>` 文本节点。
///
/// 文本样式取自 `Window::text_style()`（text_style_stack 合成的当前文本样式），
/// 布局取 Taffy 结果。DOM 文本节点天然获得浏览器的选择/复制/IME/无障碍能力。
#[cfg(feature = "dom-backend")]
fn dom_text_node(
    text: crate::SharedString,
    bounds: Bounds<Pixels>,
    window: &mut Window,
) -> Option<crate::DomNode> {
    use crate::{DomNode, DomNodeKind, DomStyle};
    let text_style = window.text_style();
    let rem_size = window.rem_size();
    let mut dom_style = DomStyle::from_bounds(bounds);
    dom_style.color = Some(text_style.color);
    dom_style.font_size = Some(text_style.font_size.to_pixels(rem_size));
    dom_style.font_family = Some(text_style.font_family.clone());
    dom_style.font_weight = Some(text_style.font_weight);
    dom_style.font_style = Some(text_style.font_style);
    dom_style.line_height = Some(
        text_style
            .line_height
            .to_pixels(text_style.font_size, rem_size),
    );
    dom_style.text_align = Some(text_style.text_align);
    dom_style.white_space = Some(text_style.white_space);
    Some(DomNode {
        kind: DomNodeKind::Text { text },
        style: dom_style,
        scroll_handle: None,
    })
}

impl Element for Text {
    type RequestLayoutState = TextLayout;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        self.id.clone()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn a11y_role(&self) -> Option<accesskit::Role> {
        if self.id.is_some() {
            Some(accesskit::Role::Label)
        } else {
            None
        }
    }

    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        node.set_value(self.text.to_string());
    }

    #[cfg(feature = "dom-backend")]
    fn dom(
        &self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<crate::DomNode> {
        <SharedString as Element>::dom(&self.text, bounds, window, cx)
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        <SharedString as Element>::request_layout(&mut self.text, id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        <SharedString as Element>::prepaint(
            &mut self.text,
            id,
            inspector_id,
            bounds,
            request_layout,
            window,
            cx,
        )
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        <SharedString as Element>::paint(
            &mut self.text,
            id,
            inspector_id,
            bounds,
            request_layout,
            prepaint,
            window,
            cx,
        );
    }
}

impl Element for &'static str {
    type RequestLayoutState = TextLayout;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut state = TextLayout::default();
        let layout_id = state.layout(SharedString::from(*self), None, window, cx);
        (layout_id, state)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        text_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
        text_layout.prepaint(bounds, self)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        text_layout: &mut TextLayout,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        text_layout.paint(self, window, cx)
    }

    #[cfg(feature = "dom-backend")]
    fn dom(
        &self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        _cx: &mut App,
    ) -> Option<crate::DomNode> {
        dom_text_node(crate::SharedString::from(*self), bounds, window)
    }
}

impl IntoElement for &'static str {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl IntoElement for String {
    type Element = SharedString;

    fn into_element(self) -> Self::Element {
        self.into()
    }
}

impl IntoElement for Cow<'static, str> {
    type Element = SharedString;

    fn into_element(self) -> Self::Element {
        self.into()
    }
}

impl Element for SharedString {
    type RequestLayoutState = TextLayout;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut state = TextLayout::default();
        let layout_id = state.layout(self.clone(), None, window, cx);
        (layout_id, state)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        text_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
        text_layout.prepaint(bounds, self.as_ref())
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        text_layout: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        text_layout.paint(self.as_ref(), window, cx)
    }

    #[cfg(feature = "dom-backend")]
    fn dom(
        &self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        _cx: &mut App,
    ) -> Option<crate::DomNode> {
        dom_text_node(self.clone(), bounds, window)
    }
}

impl IntoElement for SharedString {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// 渲染带有不同样式文本段的文本。
///
/// 调用者负责为每个段设置正确的样式。
/// 对于统一样式的文本，通常可以避免调用此构造函数
/// 而直接传递文本。
pub struct StyledText {
    text: SharedString,
    runs: Option<Vec<TextRun>>,
    /// 已解析的样式片段，供 DOM 后端在 `dom()` 中按段渲染（canvas 模式不消费）。
    ///
    /// `request_layout` 会消费 `runs`/`delayed_highlights`，因此在这里保留一份克隆，
    /// 使晚于布局的 `dom()` 阶段仍能按 `TextRun` 拆分段落、输出带样式的行内 DOM。
    dom_runs: Option<Vec<TextRun>>,
    delayed_highlights: Option<Vec<(Range<usize>, HighlightStyle)>>,
    delayed_font_family_overrides: Option<Vec<(Range<usize>, SharedString)>>,
    layout: TextLayout,
}

impl StyledText {
    /// 从给定字符串构造新的带样式文本元素。
    pub fn new(text: impl Into<SharedString>) -> Self {
        StyledText {
            text: text.into(),
            runs: None,
            dom_runs: None,
            delayed_highlights: None,
            delayed_font_family_overrides: None,
            layout: TextLayout::default(),
        }
    }

    /// 获取此元素的布局。可用于将索引映射到像素反之亦然。
    pub fn layout(&self) -> &TextLayout {
        &self.layout
    }

    /// 设置给定文本的样式属性，
    /// 以及任何已自定义样式的文本范围。
    pub fn with_default_highlights(
        mut self,
        default_style: &TextStyle,
        highlights: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    ) -> Self {
        debug_assert!(
            self.delayed_highlights.is_none(),
            "Can't use `with_default_highlights` and `with_highlights`"
        );
        let runs = Self::compute_runs(&self.text, default_style, highlights);
        self.with_runs(runs)
    }

    /// 设置给定文本的样式属性，
    /// 以及任何已自定义样式的文本范围。
    pub fn with_highlights(
        mut self,
        highlights: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    ) -> Self {
        debug_assert!(
            self.runs.is_none(),
            "Can't use `with_highlights` and `with_default_highlights`"
        );
        self.delayed_highlights = Some(
            highlights
                .into_iter()
                .inspect(|(run, _)| {
                    debug_assert!(self.text.is_char_boundary(run.start));
                    debug_assert!(self.text.is_char_boundary(run.end));
                })
                .collect::<Vec<_>>(),
        );
        self
    }

    fn compute_runs(
        text: &str,
        default_style: &TextStyle,
        highlights: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    ) -> Vec<TextRun> {
        let mut runs = Vec::new();
        let mut ix = 0;
        for (range, highlight) in highlights {
            if ix < range.start {
                debug_assert!(text.is_char_boundary(range.start));
                runs.push(default_style.clone().to_run(range.start - ix));
            }
            debug_assert!(text.is_char_boundary(range.end));
            runs.push(
                default_style
                    .clone()
                    .highlight(highlight)
                    .to_run(range.len()),
            );
            ix = range.end;
        }
        if ix < text.len() {
            runs.push(default_style.to_run(text.len() - ix));
        }
        runs
    }

    /// 覆盖特定字节范围的字体族。
    ///
    /// 这在布局时延迟解析，因此覆盖应用于
    /// 从父元素继承的文本样式之上。
    /// 可与 [`with_highlights`](Self::with_highlights) 组合使用。
    ///
    /// 覆盖必须按范围起始排序且不重叠。
    /// 每个覆盖范围必须落在字符边界上。
    pub fn with_font_family_overrides(
        mut self,
        overrides: impl IntoIterator<Item = (Range<usize>, SharedString)>,
    ) -> Self {
        self.delayed_font_family_overrides = Some(
            overrides
                .into_iter()
                .inspect(|(range, _)| {
                    debug_assert!(self.text.is_char_boundary(range.start));
                    debug_assert!(self.text.is_char_boundary(range.end));
                })
                .collect(),
        );
        self
    }

    fn apply_font_family_overrides(
        runs: &mut [TextRun],
        overrides: &[(Range<usize>, SharedString)],
    ) {
        let mut byte_offset = 0;
        let mut override_idx = 0;
        for run in runs.iter_mut() {
            let run_end = byte_offset + run.len;
            while override_idx < overrides.len() && overrides[override_idx].0.end <= byte_offset {
                override_idx += 1;
            }
            if override_idx < overrides.len() {
                let (ref range, ref family) = overrides[override_idx];
                if byte_offset >= range.start && run_end <= range.end {
                    run.font.family = family.clone();
                }
            }
            byte_offset = run_end;
        }
    }

    /// 设置此文本的文本段。
    pub fn with_runs(mut self, runs: Vec<TextRun>) -> Self {
        let mut text = &*self.text;
        for run in &runs {
            text = text.get(run.len..).unwrap_or_else(|| {
                #[cfg(debug_assertions)]
                panic!("invalid text run. Text: '{text}', run: {run:?}");
                #[cfg(not(debug_assertions))]
                panic!("invalid text run");
            });
        }
        assert!(text.is_empty(), "invalid text run");
        self.runs = Some(runs);
        self
    }
}

impl Element for StyledText {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let font_family_overrides = self.delayed_font_family_overrides.take();
        let mut runs = self.runs.take().or_else(|| {
            self.delayed_highlights.take().map(|delayed_highlights| {
                Self::compute_runs(&self.text, &window.text_style(), delayed_highlights)
            })
        });

        if let Some(ref overrides) = font_family_overrides {
            let runs =
                runs.get_or_insert_with(|| vec![window.text_style().to_run(self.text.len())]);
            Self::apply_font_family_overrides(runs, overrides);
        }

        let layout_id = self
            .layout
            .layout(self.text.clone(), runs.clone(), window, cx);
        // 保留已解析的样式片段供 DOM 后端使用（dom() 晚于本阶段调用）。
        self.dom_runs = runs;
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
        self.layout.prepaint(bounds, &self.text)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.layout.paint(&self.text, window, cx)
    }

    #[cfg(feature = "dom-backend")]
    fn dom(
        &self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        _cx: &mut App,
    ) -> Option<crate::DomNode> {
        use crate::{DomNode, DomNodeKind, DomPosition, DomStyle, DomTextDecoration};

        // 无多段样式：退化为基础文本节点（与 canvas 跳过字形绘制配合，DOM 是唯一文本渲染者）。
        let Some(runs) = &self.dom_runs else {
            return <crate::SharedString as Element>::dom(&self.text, bounds, window, _cx);
        };
        if runs.is_empty() {
            return <crate::SharedString as Element>::dom(&self.text, bounds, window, _cx);
        }

        let text_style = window.text_style();
        let rem_size = window.rem_size();
        let text = self.text.to_string();

        // 父 span：承载整段文本的基础样式，绝对定位到 Taffy 计算出的 bounds。
        let mut parent_style = DomStyle::from_bounds(bounds);
        parent_style.color = Some(text_style.color);
        parent_style.font_size = Some(text_style.font_size.to_pixels(rem_size));
        parent_style.font_family = Some(text_style.font_family.clone());
        parent_style.font_weight = Some(text_style.font_weight);
        parent_style.font_style = Some(text_style.font_style);
        parent_style.line_height = Some(
            text_style
                .line_height
                .to_pixels(text_style.font_size, rem_size),
        );
        parent_style.text_align = Some(text_style.text_align);
        parent_style.white_space = Some(text_style.white_space);

        // 行内子片段：每个 run 一个小 span，按段着色 / 字重 / 字型 / 装饰线。
        let mut byte_offset = 0usize;
        let mut children = Vec::new();
        for run in runs {
            let end = byte_offset + run.len;
            let segment: crate::SharedString = match text.get(byte_offset..end) {
                Some(s) => s.to_string().into(),
                None => String::new().into(),
            };
            byte_offset = end;
            if segment.is_empty() {
                continue;
            }

            let mut child_style = DomStyle::default();
            child_style.position = DomPosition::Static;
            child_style.color = Some(run.color);
            child_style.font_family = Some(run.font.family.clone());
            child_style.font_weight = Some(run.font.weight);
            child_style.font_style = Some(run.font.style);
            if let Some(bg) = run.background_color {
                child_style.background_color = Some(bg);
            }
            if run.underline.is_some() {
                child_style.text_decoration = DomTextDecoration::Underline;
            } else if run.strikethrough.is_some() {
                child_style.text_decoration = DomTextDecoration::LineThrough;
            }

            children.push(DomNode {
                kind: DomNodeKind::Text { text: segment },
                style: child_style,
                scroll_handle: None,
            });
        }

        if children.is_empty() {
            return <crate::SharedString as Element>::dom(&self.text, bounds, window, _cx);
        }

        Some(DomNode {
            kind: DomNodeKind::Element {
                tag: "span",
                attrs: vec![],
                children,
            },
            style: parent_style,
            scroll_handle: None,
        })
    }
}

impl IntoElement for StyledText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// TextElement 的布局。可用于将索引映射到像素反之亦然。
#[derive(Default, Clone)]
pub struct TextLayout(Rc<RefCell<Option<TextLayoutInner>>>);

struct TextLayoutInner {
    len: usize,
    lines: SmallVec<[WrappedLine; 1]>,
    line_height: Pixels,
    wrap_width: Option<Pixels>,
    truncate_width: Option<Pixels>,
    size: Option<Size<Pixels>>,
    bounds: Option<Bounds<Pixels>>,
}

impl TextLayout {
    fn layout(
        &self,
        text: SharedString,
        runs: Option<Vec<TextRun>>,
        window: &mut Window,
        _: &mut App,
    ) -> LayoutId {
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = window.pixel_snap(
            text_style
                .line_height
                .to_pixels(font_size.into(), window.rem_size()),
        );

        let runs = if let Some(runs) = runs {
            runs
        } else {
            vec![text_style.to_run(text.len())]
        };
        window.request_measured_layout(Default::default(), {
            let element_state = self.clone();

            move |known_dimensions, available_space, window, cx| {
                let wrap_width = if text_style.white_space == WhiteSpace::Normal {
                    known_dimensions.width.or(match available_space.width {
                        crate::AvailableSpace::Definite(x) => Some(x),
                        _ => None,
                    })
                } else {
                    None
                };

                let (truncate_width, truncation_affix, truncate_from) =
                    if let Some(text_overflow) = text_style.text_overflow.clone() {
                        let width = known_dimensions.width.or(match available_space.width {
                            crate::AvailableSpace::Definite(x) => match text_style.line_clamp {
                                Some(max_lines) => Some(x * max_lines),
                                None => Some(x),
                            },
                            _ => None,
                        });

                        match text_overflow {
                            TextOverflow::Truncate(s) => (width, s, TruncateFrom::End),
                            TextOverflow::TruncateStart(s) => (width, s, TruncateFrom::Start),
                            TextOverflow::TruncateMiddle(s) => (width, s, TruncateFrom::Middle),
                        }
                    } else {
                        (None, "".into(), TruncateFrom::End)
                    };

                // Only use cached layout if:
                // 1. We have a cached size
                // 2. wrap_width matches (or both are None)
                // 3. truncate_width is None (if truncate_width is Some, we need to re-layout
                //    because the previous layout may have been computed without truncation)
                // 4. the cached layout was not truncated (a truncated layout answers an
                //    unconstrained probe with the truncated size, which poisons intrinsic
                //    sizing with whatever width some earlier measure pass happened to use)
                if let Some(text_layout) = element_state.0.borrow().as_ref()
                    && let Some(size) = text_layout.size
                    && (wrap_width.is_none() || wrap_width == text_layout.wrap_width)
                    && truncate_width.is_none()
                    && text_layout.truncate_width.is_none()
                {
                    return size;
                }

                let mut line_wrapper = cx.text_system().line_wrapper(text_style.font(), font_size);
                let (text, runs) = if let Some(truncate_width) = truncate_width {
                    if let Some(max_lines) = text_style.line_clamp
                        && let Some(wrap_width) = wrap_width
                    {
                        line_wrapper.truncate_wrapped_line(
                            text.clone(),
                            wrap_width,
                            max_lines,
                            &truncation_affix,
                            &runs,
                            truncate_from,
                        )
                    } else if let Some(unclipped) = window
                        .text_system()
                        .shape_text(text.clone(), font_size, &runs, None, None)
                        .log_err()
                        && unclipped
                            .iter()
                            .all(|line| line.size(line_height).width <= truncate_width)
                    {
                        // The truncation decision below sums per-character advances,
                        // which overestimates the shaped width (no kerning), truncating
                        // text that fits exactly in its measured width. Skip truncation
                        // whenever the honestly-shaped text fits; the shaping result
                        // comes from the line layout cache when the same text was
                        // already measured untruncated this frame.
                        (text.clone(), Cow::Borrowed(&*runs))
                    } else {
                        line_wrapper.truncate_line(
                            text.clone(),
                            truncate_width,
                            &truncation_affix,
                            &runs,
                            truncate_from,
                        )
                    }
                } else {
                    (text.clone(), Cow::Borrowed(&*runs))
                };
                let len = text.len();

                let Some(lines) = window
                    .text_system()
                    .shape_text(
                        text,
                        font_size,
                        &runs,
                        wrap_width,            // Wrap if we know the width.
                        text_style.line_clamp, // Limit the number of lines if line_clamp is set.
                    )
                    .log_err()
                else {
                    element_state.0.borrow_mut().replace(TextLayoutInner {
                        lines: Default::default(),
                        len: 0,
                        line_height,
                        wrap_width,
                        truncate_width,
                        size: Some(Size::default()),
                        bounds: None,
                    });
                    return Size::default();
                };

                let mut size: Size<Pixels> = Size::default();
                for line in &lines {
                    let line_size = line.size(line_height);
                    size.height += line_size.height;
                    size.width = size.width.max(line_size.width).ceil();
                }

                element_state.0.borrow_mut().replace(TextLayoutInner {
                    lines,
                    len,
                    line_height,
                    wrap_width,
                    truncate_width,
                    size: Some(size),
                    bounds: None,
                });

                size
            }
        })
    }

    fn prepaint(&self, bounds: Bounds<Pixels>, text: &str) {
        let mut element_state = self.0.borrow_mut();
        let element_state = element_state
            .as_mut()
            .with_context(|| format!("measurement has not been performed on {text}"))
            .unwrap();
        element_state.bounds = Some(bounds);
    }

    fn paint(&self, text: &str, window: &mut Window, cx: &mut App) {
        let element_state = self.0.borrow();
        let element_state = element_state
            .as_ref()
            .with_context(|| format!("measurement has not been performed on {text}"))
            .unwrap();
        let bounds = element_state
            .bounds
            .with_context(|| format!("prepaint has not been performed on {text}"))
            .unwrap();

        let line_height = element_state.line_height;
        let mut line_origin = bounds.origin;
        let text_style = window.text_style();
        for line in &element_state.lines {
            line.paint_background(
                line_origin,
                line_height,
                text_style.text_align,
                Some(bounds),
                window,
                cx,
            )
            .log_err();
            // DOM 覆盖层启用时，文本字形改由 DOM 层渲染（可选中/复制/IME），
            // canvas 不再重复绘制字形，避免双重渲染“重影”；行背景仍由 canvas 绘制。
            #[cfg(feature = "dom-backend")]
            if !window.dom_builder_active() {
                line.paint(
                    line_origin,
                    line_height,
                    text_style.text_align,
                    Some(bounds),
                    window,
                    cx,
                )
                .log_err();
            }
            #[cfg(not(feature = "dom-backend"))]
            line.paint(
                line_origin,
                line_height,
                text_style.text_align,
                Some(bounds),
                window,
                cx,
            )
            .log_err();
            line_origin.y += line.size(line_height).height;
        }
    }

    /// 获取像素位置对应的字节索引。
    pub fn index_for_position(&self, mut position: Point<Pixels>) -> Result<usize, usize> {
        let element_state = self.0.borrow();
        let element_state = element_state
            .as_ref()
            .expect("measurement has not been performed");
        let bounds = element_state
            .bounds
            .expect("prepaint has not been performed");

        if position.y < bounds.top() {
            return Err(0);
        }

        let line_height = element_state.line_height;
        let mut line_origin = bounds.origin;
        let mut line_start_ix = 0;
        for line in &element_state.lines {
            let line_bottom = line_origin.y + line.size(line_height).height;
            if position.y > line_bottom {
                line_origin.y = line_bottom;
                line_start_ix += line.len() + 1;
            } else {
                let position_within_line = position - line_origin;
                match line.index_for_position(position_within_line, line_height) {
                    Ok(index_within_line) => return Ok(line_start_ix + index_within_line),
                    Err(index_within_line) => return Err(line_start_ix + index_within_line),
                }
            }
        }

        Err(line_start_ix.saturating_sub(1))
    }

    /// 获取给定字节索引的像素位置。
    pub fn position_for_index(&self, index: usize) -> Option<Point<Pixels>> {
        let element_state = self.0.borrow();
        let element_state = element_state
            .as_ref()
            .expect("measurement has not been performed");
        let bounds = element_state
            .bounds
            .expect("prepaint has not been performed");
        let line_height = element_state.line_height;

        let mut line_origin = bounds.origin;
        let mut line_start_ix = 0;

        for line in &element_state.lines {
            let line_end_ix = line_start_ix + line.len();
            if index < line_start_ix {
                break;
            } else if index > line_end_ix {
                line_origin.y += line.size(line_height).height;
                line_start_ix = line_end_ix + 1;
                continue;
            } else {
                let ix_within_line = index - line_start_ix;
                return Some(line_origin + line.position_for_index(ix_within_line, line_height)?);
            }
        }

        None
    }

    /// 获取包含给定字节索引的行的布局。
    pub fn line_layout_for_index(&self, index: usize) -> Option<Arc<WrappedLineLayout>> {
        let element_state = self.0.borrow();
        let element_state = element_state
            .as_ref()
            .expect("measurement has not been performed");
        let mut line_start_ix = 0;

        for line in &element_state.lines {
            let line_end_ix = line_start_ix + line.len();
            if index < line_start_ix {
                break;
            } else if index > line_end_ix {
                line_start_ix = line_end_ix + 1;
                continue;
            } else {
                return Some(line.layout.clone());
            }
        }

        None
    }

    /// 按源顺序检索所有行布局。
    pub fn line_layouts(&self) -> SmallVec<[Arc<WrappedLineLayout>; 1]> {
        self.0
            .borrow()
            .as_ref()
            .expect("measurement has not been performed")
            .lines
            .iter()
            .map(|line| line.layout.clone())
            .collect()
    }

    /// 此布局的边界。
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.0.borrow().as_ref().unwrap().bounds.unwrap()
    }

    /// 此布局的行高。
    pub fn line_height(&self) -> Pixels {
        self.0.borrow().as_ref().unwrap().line_height
    }

    /// 底层文本的 UTF-8 长度。
    pub fn len(&self) -> usize {
        self.0.borrow().as_ref().unwrap().len
    }

    /// 此布局的文本。
    pub fn text(&self) -> String {
        self.0
            .borrow()
            .as_ref()
            .unwrap()
            .lines
            .iter()
            .map(|s| &s.text)
            .join("\n")
    }

    /// 此布局的文本（软换行符作为换行符）
    pub fn wrapped_text(&self) -> String {
        let mut accumulator = String::new();

        for wrapped in self.0.borrow().as_ref().unwrap().lines.iter() {
            let mut seen = 0;
            for boundary in wrapped.layout.wrap_boundaries.iter() {
                let index = wrapped.layout.unwrapped_layout.runs[boundary.run_ix].glyphs
                    [boundary.glyph_ix]
                    .index;

                accumulator.push_str(&wrapped.text[seen..index]);
                accumulator.push('\n');
                seen = index;
            }
            accumulator.push_str(&wrapped.text[seen..]);
            accumulator.push('\n');
        }
        // Remove trailing newline
        accumulator.pop();
        accumulator
    }
}

/// 一个可交互的文本元素。
pub struct InteractiveText {
    element_id: ElementId,
    text: StyledText,
    click_listener:
        Option<Box<dyn Fn(&[Range<usize>], InteractiveTextClickEvent, &mut Window, &mut App)>>,
    hover_listener: Option<Box<dyn Fn(Option<usize>, MouseMoveEvent, &mut Window, &mut App)>>,
    tooltip_builder: Option<Rc<dyn Fn(usize, &mut Window, &mut App) -> Option<AnyView>>>,
    tooltip_id: Option<TooltipId>,
    clickable_ranges: Vec<Range<usize>>,
}

struct InteractiveTextClickEvent {
    mouse_down_index: usize,
    mouse_up_index: usize,
}

#[doc(hidden)]
#[derive(Default)]
pub struct InteractiveTextState {
    mouse_down_index: Rc<Cell<Option<usize>>>,
    hovered_index: Rc<Cell<Option<usize>>>,
    active_tooltip: Rc<RefCell<Option<ActiveTooltip>>>,
}

/// InteractiveTest 是 StyledText 的包装器，添加了鼠标交互。
impl InteractiveText {
    /// 从给定文本创建新的 InteractiveText。
    pub fn new(id: impl Into<ElementId>, text: StyledText) -> Self {
        Self {
            element_id: id.into(),
            text,
            click_listener: None,
            hover_listener: None,
            tooltip_builder: None,
            tooltip_id: None,
            clickable_ranges: Vec::new(),
        }
    }

    /// 当用户点击给定范围之一时调用 on_click，传递被点击范围的索引。
    pub fn on_click(
        mut self,
        ranges: Vec<Range<usize>>,
        listener: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.click_listener = Some(Box::new(move |ranges, event, window, cx| {
            for (range_ix, range) in ranges.iter().enumerate() {
                if range.contains(&event.mouse_down_index) && range.contains(&event.mouse_up_index)
                {
                    listener(range_ix, window, cx);
                }
            }
        }));
        self.clickable_ranges = ranges;
        self
    }

    /// 当鼠标在文本中的字符上移动时调用 on_hover，传递
    /// 悬停字符的索引，如果鼠标离开文本则传递 None。
    pub fn on_hover(
        mut self,
        listener: impl Fn(Option<usize>, MouseMoveEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.hover_listener = Some(Box::new(listener));
        self
    }

    /// tooltip 允许你为字符串中的给定字符索引指定工具提示。
    pub fn tooltip(
        mut self,
        builder: impl Fn(usize, &mut Window, &mut App) -> Option<AnyView> + 'static,
    ) -> Self {
        self.tooltip_builder = Some(Rc::new(builder));
        self
    }
}

impl Element for InteractiveText {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.element_id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn a11y_role(&self) -> Option<accesskit::Role> {
        Some(accesskit::Role::Label)
    }

    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        node.set_value(self.text.text.to_string());
    }

    #[cfg(feature = "dom-backend")]
    fn dom(
        &self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<crate::DomNode> {
        self.text.dom(bounds, window, cx)
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.text.request_layout(None, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        window.with_optional_element_state::<InteractiveTextState, _>(
            global_id,
            |interactive_state, window| {
                let mut interactive_state = interactive_state
                    .map(|interactive_state| interactive_state.unwrap_or_default());

                if let Some(interactive_state) = interactive_state.as_mut() {
                    if self.tooltip_builder.is_some() {
                        self.tooltip_id =
                            set_tooltip_on_window(&interactive_state.active_tooltip, window);
                    } else {
                        // If there is no longer a tooltip builder, remove the active tooltip.
                        interactive_state.active_tooltip.take();
                    }
                }

                self.text
                    .prepaint(None, inspector_id, bounds, state, window, cx);
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                (hitbox, interactive_state)
            },
        )
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        let current_view = window.current_view();
        let text_layout = self.text.layout().clone();
        window.with_element_state::<InteractiveTextState, _>(
            global_id.unwrap(),
            |interactive_state, window| {
                let mut interactive_state = interactive_state.unwrap_or_default();
                if let Some(click_listener) = self.click_listener.take() {
                    let mouse_position = window.mouse_position();
                    if let Ok(ix) = text_layout.index_for_position(mouse_position)
                        && self
                            .clickable_ranges
                            .iter()
                            .any(|range| range.contains(&ix))
                    {
                        window.set_cursor_style(crate::CursorStyle::PointingHand, hitbox)
                    }

                    let text_layout = text_layout.clone();
                    let mouse_down = interactive_state.mouse_down_index.clone();
                    if let Some(mouse_down_index) = mouse_down.get() {
                        let hitbox = hitbox.clone();
                        let clickable_ranges = mem::take(&mut self.clickable_ranges);
                        window.on_mouse_event(
                            move |event: &MouseUpEvent, phase, window: &mut Window, cx| {
                                if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) {
                                    if let Ok(mouse_up_index) =
                                        text_layout.index_for_position(event.position)
                                    {
                                        click_listener(
                                            &clickable_ranges,
                                            InteractiveTextClickEvent {
                                                mouse_down_index,
                                                mouse_up_index,
                                            },
                                            window,
                                            cx,
                                        )
                                    }

                                    mouse_down.take();
                                    window.refresh();
                                }
                            },
                        );
                    } else {
                        let hitbox = hitbox.clone();
                        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _| {
                            if phase == DispatchPhase::Bubble
                                && hitbox.is_hovered(window)
                                && let Ok(mouse_down_index) =
                                    text_layout.index_for_position(event.position)
                            {
                                mouse_down.set(Some(mouse_down_index));
                                window.refresh();
                            }
                        });
                    }
                }

                window.on_mouse_event({
                    let mut hover_listener = self.hover_listener.take();
                    let hitbox = hitbox.clone();
                    let text_layout = text_layout.clone();
                    let hovered_index = interactive_state.hovered_index.clone();
                    move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) {
                            let current = hovered_index.get();
                            let updated = text_layout.index_for_position(event.position).ok();
                            if current != updated {
                                hovered_index.set(updated);
                                if let Some(hover_listener) = hover_listener.as_ref() {
                                    hover_listener(updated, event.clone(), window, cx);
                                }
                                cx.notify(current_view);
                            }
                        }
                    }
                });

                if let Some(tooltip_builder) = self.tooltip_builder.clone() {
                    let active_tooltip = interactive_state.active_tooltip.clone();
                    let build_tooltip = Rc::new({
                        let tooltip_is_hoverable = false;
                        let text_layout = text_layout.clone();
                        move |window: &mut Window, cx: &mut App| {
                            text_layout
                                .index_for_position(window.mouse_position())
                                .ok()
                                .and_then(|position| tooltip_builder(position, window, cx))
                                .map(|view| (view, tooltip_is_hoverable))
                        }
                    });

                    // Use bounds instead of testing hitbox since this is called during prepaint.
                    let check_is_hovered_during_prepaint = Rc::new({
                        let source_bounds = hitbox.bounds;
                        let text_layout = text_layout.clone();
                        let pending_mouse_down = interactive_state.mouse_down_index.clone();
                        move |window: &Window| {
                            text_layout
                                .index_for_position(window.mouse_position())
                                .is_ok()
                                && source_bounds.contains(&window.mouse_position())
                                && pending_mouse_down.get().is_none()
                        }
                    });

                    let check_is_hovered = Rc::new({
                        let hitbox = hitbox.clone();
                        let text_layout = text_layout.clone();
                        let pending_mouse_down = interactive_state.mouse_down_index.clone();
                        move |window: &Window| {
                            text_layout
                                .index_for_position(window.mouse_position())
                                .is_ok()
                                && hitbox.is_hovered(window)
                                && pending_mouse_down.get().is_none()
                        }
                    });

                    register_tooltip_mouse_handlers(
                        &active_tooltip,
                        self.tooltip_id,
                        build_tooltip,
                        check_is_hovered,
                        check_is_hovered_during_prepaint,
                        None,
                        window,
                    );
                }

                self.text
                    .paint(None, inspector_id, bounds, &mut (), &mut (), window, cx);

                ((), interactive_state)
            },
        );
    }
}

impl IntoElement for InteractiveText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_into_element_for() {
        use crate::{ParentElement as _, SharedString, div};
        use std::borrow::Cow;

        let _ = div().child("static str");
        let _ = div().child("String".to_string());
        let _ = div().child(Cow::Borrowed("Cow"));
        let _ = div().child(SharedString::from("SharedString"));
    }

    #[test]
    fn text_macro_id() {
        // one call to `text!` = one id
        fn make_text_stable_id(happy: bool) -> Text {
            text!(if happy { "happy" } else { "sad" })
        }

        // two calls to `text!` = two ids
        fn make_text_unstable_id(happy: bool) -> Text {
            if happy { text!("happy") } else { text!("sad") }
        }

        assert_eq!(make_text_stable_id(false).id, make_text_stable_id(true).id);
        assert_ne!(
            make_text_unstable_id(false).id,
            make_text_unstable_id(true).id
        );
    }
}
