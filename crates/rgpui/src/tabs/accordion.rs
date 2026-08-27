use std::{cell::RefCell, collections::HashSet, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use crate::{
    AnyElement, App, AvailableSpace, Bounds, ContentMask, Element, ElementId, GlobalElementId,
    InspectorElementId, InteractiveElement as _, IntoElement, LayoutId, ParentElement, Pixels,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, Style, StyleRefinement,
    Styled, Window, div, percentage, prelude::FluentBuilder as _, px, relative, rems, size,
};

#[cfg(feature = "dom-backend")]
use crate::{DomNode, DomNodeKind, DomOverflow, DomStyle};

use crate::{
    ActiveTheme as _, ElementSize, Icon, IconName, Sizable, StyledExt as _, ease_out_cubic, h_flex,
    v_flex,
};

/// 展开/折叠动画的持续时间。
const ANIMATION_DURATION: Duration = Duration::from_millis(200);

/// 手风琴组件，一个可同时展开多个项目（或单个）的垂直堆叠列表。
#[derive(IntoElement)]
pub struct Accordion {
    id: ElementId,
    style: StyleRefinement,
    multiple: bool,
    size: ElementSize,
    bordered: bool,
    disabled: bool,
    children: Vec<AccordionItem>,
    on_toggle_click: Option<Arc<dyn Fn(&[usize], &mut Window, &mut App) + Send + Sync>>,
}

impl Accordion {
    /// 使用给定 ID 创建一个新的手风琴。
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            multiple: false,
            size: ElementSize::default(),
            bordered: true,
            children: Vec::new(),
            disabled: false,
            on_toggle_click: None,
        }
    }

    /// 设置是否允许多个项目同时展开，默认为 false。
    pub fn multiple(mut self, multiple: bool) -> Self {
        self.multiple = multiple;
        self
    }

    /// 设置项目之间是否显示边框，默认为 true。
    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    /// 设置手风琴是否禁用，默认为 false。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 向手风琴中添加一个 [`AccordionItem`]。
    pub fn item<F>(mut self, child: F) -> Self
    where
        F: FnOnce(AccordionItem) -> AccordionItem,
    {
        let item = child(AccordionItem::new());
        self.children.push(item);
        self
    }

    /// 设置手风琴展开状态变化时的回调。
    ///
    /// 第一个参数 `Vec<usize>` 是当前已展开项目的索引列表。
    pub fn on_toggle_click(
        mut self,
        on_toggle_click: impl Fn(&[usize], &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_toggle_click = Some(Arc::new(on_toggle_click));
        self
    }
}

impl Sizable for Accordion {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl Styled for Accordion {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Accordion {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let open_ixs = Rc::new(RefCell::new(HashSet::new()));
        let is_multiple = self.multiple;
        let last_ix = self.children.len().saturating_sub(1);

        v_flex()
            .id(self.id)
            .size_full()
            // 带边框的手风琴是单个圆角卡片，项目之间由分隔线连接。
            .when(self.bordered, |this| {
                this.border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius_lg)
                    .overflow_hidden()
            })
            .refine_style(&self.style)
            .children(
                self.children
                    .into_iter()
                    .enumerate()
                    .map(|(ix, accordion)| {
                        if accordion.open {
                            open_ixs.borrow_mut().insert(ix);
                        }

                        accordion
                            .index(ix)
                            .last(ix == last_ix)
                            .with_size(self.size)
                            .disabled(self.disabled)
                            .on_toggle_click({
                                let open_ixs = Rc::clone(&open_ixs);
                                move |open, _, _| {
                                    let mut open_ixs = open_ixs.borrow_mut();
                                    if *open {
                                        if !is_multiple {
                                            open_ixs.clear();
                                        }
                                        open_ixs.insert(ix);
                                    } else {
                                        open_ixs.remove(&ix);
                                    }
                                }
                            })
                    }),
            )
            .when_some(
                self.on_toggle_click.filter(|_| !self.disabled),
                move |this, on_toggle_click| {
                    let open_ixs = Rc::clone(&open_ixs);
                    this.on_click(move |_, window, cx| {
                        let open_ixs: Vec<usize> = open_ixs.borrow().iter().map(|&ix| ix).collect();

                        on_toggle_click(&open_ixs, window, cx);
                    })
                },
            )
    }
}

/// [`AccordionItem`] 的内容，在切换时对高度进行动画处理。
///
/// 关闭时仍保留在元素树中，只是高度被裁剪为零，以便动画折叠并保持自然高度测量。
struct AccordionContent {
    id: ElementId,
    open: bool,
    child: AnyElement,
}

#[derive(Clone, Copy)]
struct AccordionContentState {
    open: bool,
    /// 当前动画开始时的进度。
    from: f32,
    started_at: Option<Instant>,
    /// 上次 prepaint 时测量的自然高度。
    height: Option<Pixels>,
}

impl AccordionContentState {
    fn new(open: bool) -> Self {
        Self {
            open,
            from: if open { 1. } else { 0. },
            started_at: None,
            height: None,
        }
    }

    /// 从 0.（关闭）到 1.（展开）的进度，超过持续时间后结束动画。
    fn progress(&mut self) -> f32 {
        let target = if self.open { 1. } else { 0. };
        let Some(started_at) = self.started_at else {
            return target;
        };

        let t = started_at.elapsed().as_secs_f32() / ANIMATION_DURATION.as_secs_f32();
        if t >= 1. {
            self.started_at = None;
            return target;
        }

        self.from + (target - self.from) * ease_out_cubic(t)
    }
}

impl AccordionContent {
    fn new(id: impl Into<ElementId>, open: bool, child: AnyElement) -> Self {
        Self {
            id: id.into(),
            open,
            child,
        }
    }
}

impl IntoElement for AccordionContent {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for AccordionContent {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let open = self.open;
        let (progress, height) = window.with_element_state(
            global_id.expect("AccordionContent 必须有 id"),
            |state: Option<AccordionContentState>, window| {
                let mut state = state.unwrap_or_else(|| AccordionContentState::new(open));

                if state.open != open {
                    state.from = state.progress();
                    state.open = open;
                    state.started_at = Some(Instant::now());
                }

                let progress = state.progress();
                if state.started_at.is_some() {
                    window.request_animation_frame();
                }

                ((progress, state.height), state)
            },
        );

        let mut style = Style::default();
        style.size.width = relative(1.).into();
        match height {
            // 尚未测量，让内容自行布局。
            None if open => {}
            None => style.size.height = px(0.).into(),
            Some(height) => style.size.height = (height * progress).into(),
        }

        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // 按自然高度布局，`bounds` 裁剪可见部分。
        let available = size(
            AvailableSpace::Definite(bounds.size.width),
            AvailableSpace::MinContent,
        );
        let measured = self.child.layout_as_root(available, window, cx);

        let changed = window.with_element_state(
            global_id.expect("AccordionContent 必须有 id"),
            |state: Option<AccordionContentState>, _| {
                let mut state = state.unwrap_or_else(|| AccordionContentState::new(self.open));
                let changed = state.height != Some(measured.height);
                state.height = Some(measured.height);
                (changed, state)
            },
        );

        // 测量的高度只在下一次 `request_layout` 中使用，请求该帧，
        // 否则新高度永远不会被绘制。
        if changed {
            window.request_animation_frame();
        }

        // 这里也使用内容遮罩，使隐藏的内容不接收鼠标事件。
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            self.child.prepaint_at(bounds.origin, window, cx);
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            self.child.paint(window, cx);
        });
    }

    #[cfg(feature = "dom-backend")]
    fn dom(&self, bounds: Bounds<Pixels>, _window: &mut Window, _cx: &mut App) -> Option<DomNode> {
        // 折叠内容靠 `bounds` 高度（动画进度 × 自然高度）裁剪，
        // DOM 模式以 `overflow: hidden` 的包裹层承载，子内容在其下自动定位。
        let mut style = DomStyle::from_bounds(bounds);
        style.overflow = DomOverflow::Hidden;
        Some(DomNode {
            kind: DomNodeKind::Element {
                tag: "div",
                attrs: Vec::new(),
                children: Vec::new(),
            },
            style,
            scroll_handle: None,
        })
    }
}

/// 手风琴项目，可展开以显示与其关联的内容。
#[derive(IntoElement)]
pub struct AccordionItem {
    index: usize,
    last: bool,
    style: StyleRefinement,
    hover_style: Option<StyleRefinement>,
    title_style: StyleRefinement,
    content_style: StyleRefinement,
    icon: Option<Icon>,
    title: AnyElement,
    children: Vec<AnyElement>,
    open: bool,
    size: ElementSize,
    disabled: bool,
    on_toggle_click: Option<Arc<dyn Fn(&bool, &mut Window, &mut App)>>,
}

impl AccordionItem {
    /// 创建一个新的手风琴项目。
    pub fn new() -> Self {
        Self {
            index: 0,
            last: false,
            style: StyleRefinement::default(),
            hover_style: None,
            title_style: StyleRefinement::default(),
            content_style: StyleRefinement::default(),
            icon: None,
            title: SharedString::default().into_any_element(),
            children: Vec::new(),
            open: false,
            disabled: false,
            on_toggle_click: None,
            size: ElementSize::default(),
        }
    }

    /// 设置手风琴项目的图标。
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 设置手风琴项目的标题。
    pub fn title(mut self, title: impl IntoElement) -> Self {
        self.title = title.into_any_element();
        self
    }

    /// 设置项目是否展开。
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// 设置项目是否禁用。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 为标题行设置额外的样式。
    pub fn title_style(mut self, style: StyleRefinement) -> Self {
        self.title_style = style;
        self
    }

    /// 设置鼠标悬停标题行时的样式。
    ///
    /// 默认没有悬停样式。标题行是切换项目展开的部分，
    /// 因此悬停反馈属于标题行而不是整个项目。
    pub fn hover(mut self, f: impl FnOnce(StyleRefinement) -> StyleRefinement) -> Self {
        self.hover_style = Some(f(StyleRefinement::default()));
        self
    }

    /// 为标题下方的内容设置额外的样式。
    pub fn content_style(mut self, style: StyleRefinement) -> Self {
        self.content_style = style;
        self
    }

    fn index(mut self, index: usize) -> Self {
        self.index = index;
        self
    }

    fn last(mut self, last: bool) -> Self {
        self.last = last;
        self
    }

    fn on_toggle_click(
        mut self,
        on_toggle_click: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_click = Some(Arc::new(on_toggle_click));
        self
    }
}

impl ParentElement for AccordionItem {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Sizable for AccordionItem {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl Styled for AccordionItem {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for AccordionItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let text_size = match self.size {
            ElementSize::XSmall => rems(0.8125),
            ElementSize::Large => rems(1.0),
            _ => rems(0.875),
        };

        div().flex_1().child(
            v_flex()
                .w_full()
                .bg(cx.theme().tokens.accordion)
                .overflow_hidden()
                // 每个项目下方用一条线分隔。
                .when(!self.last, |this| {
                    this.border_b_1().border_color(cx.theme().border)
                })
                .text_size(text_size)
                .refine_style(&self.style)
                .child(
                    h_flex()
                        .id(self.index)
                        .role(Role::Button)
                        .aria_expanded(self.open)
                        .justify_between()
                        .gap_3()
                        .font_medium()
                        .map(|this| match self.size {
                            ElementSize::XSmall => this.py_1().px_1p5(),
                            ElementSize::Small => this.py_1p5().px_2(),
                            ElementSize::Large => this.py_3().px_4(),
                            _ => this.py_2().px_3(),
                        })
                        .when(self.open, |this| this.text_color(cx.theme().foreground))
                        .refine_style(&self.title_style)
                        .child(
                            h_flex()
                                .items_center()
                                .map(|this| match self.size {
                                    ElementSize::XSmall => this.gap_1(),
                                    ElementSize::Small => this.gap_1(),
                                    _ => this.gap_2(),
                                })
                                .when_some(self.icon, |this, icon| {
                                    this.child(icon.with_size(self.size))
                                })
                                .child(self.title),
                        )
                        .when(!self.disabled, |this| {
                            this.when_some(self.hover_style, |this, hover_style| {
                                this.hover(move |this| this.refine_style(&hover_style))
                            })
                            .child(
                                Icon::new(IconName::ChevronDown)
                                    .xsmall()
                                    .text_color(cx.theme().muted_foreground)
                                    .rotate(percentage(if self.open { 0.5 } else { 0. })),
                            )
                            .when_some(
                                self.on_toggle_click,
                                |this, on_toggle_click| {
                                    this.on_click({
                                        move |_, window, cx| {
                                            on_toggle_click(&!self.open, window, cx);
                                        }
                                    })
                                },
                            )
                        }),
                )
                .child(AccordionContent::new(
                    ("content", self.index),
                    self.open,
                    div()
                        // 无顶部内边距，标题自带底部内边距。
                        .map(|this| match self.size {
                            ElementSize::XSmall => this.pb_1().px_1p5(),
                            ElementSize::Small => this.pb_1p5().px_2(),
                            ElementSize::Large => this.pb_3().px_4(),
                            _ => this.pb_2().px_3(),
                        })
                        .refine_style(&self.content_style)
                        .children(self.children)
                        .into_any_element(),
                )),
        )
    }
}
