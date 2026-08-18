//! 分段导航：带滑动高亮指示器的分段选择控件。

use rgpui::{prelude::FluentBuilder as _, *};
use std::{rc::Rc, time::Duration};

use crate::animation::{durations, easing::easings};

/// 分段导航条目。
#[derive(Clone)]
struct SegmentedNavItem {
    /// 条目标识。
    id: SharedString,
    /// 条目标签。
    label: SharedString,
}

/// 分段导航尺寸。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SegmentedNavSize {
    /// 小尺寸。
    Sm,
    /// 中等尺寸（默认）。
    #[default]
    Md,
    /// 大尺寸。
    Lg,
}

impl SegmentedNavSize {
    /// 获取对应高度。
    fn height(&self) -> Pixels {
        match self {
            Self::Sm => px(32.0),
            Self::Md => px(40.0),
            Self::Lg => px(48.0),
        }
    }

    /// 获取对应文字大小。
    fn text_size(&self) -> Pixels {
        match self {
            Self::Sm => px(12.0),
            Self::Md => px(14.0),
            Self::Lg => px(16.0),
        }
    }

    /// 获取对应水平内边距。
    fn padding_x(&self) -> Pixels {
        match self {
            Self::Sm => px(8.0),
            Self::Md => px(12.0),
            Self::Lg => px(16.0),
        }
    }
}

/// 分段导航状态：管理激活项与滑动动画版本。
pub struct SegmentedNavState {
    /// 当前激活项 id。
    active: SharedString,
    /// 上一个激活项 id。
    previous_active: Option<SharedString>,
    /// 条目列表。
    items: Vec<SegmentedNavItem>,
    /// 动画版本号。
    animation_version: usize,
}

impl SegmentedNavState {
    /// 创建状态。
    pub fn new(active: impl Into<SharedString>) -> Self {
        Self {
            active: active.into(),
            previous_active: None,
            items: Vec::new(),
            animation_version: 0,
        }
    }

    /// 设置激活项。
    pub fn set_active(&mut self, id: impl Into<SharedString>, cx: &mut Context<Self>) {
        let new_id = id.into();
        if self.active != new_id {
            self.previous_active = Some(self.active.clone());
            self.active = new_id;
            self.animation_version = self.animation_version.wrapping_add(1);
            cx.notify();
        }
    }

    /// 获取当前激活项 id。
    pub fn active(&self) -> &SharedString {
        &self.active
    }

    /// 获取当前激活项索引。
    fn _active_index(&self) -> Option<usize> {
        self.items.iter().position(|item| item.id == self.active)
    }
}

/// 分段导航组件。
#[derive(IntoElement)]
pub struct SegmentedNav {
    /// 元素 ID。
    id: ElementId,
    /// 绑定状态实体。
    state: Entity<SegmentedNavState>,
    /// 条目列表。
    items: Vec<SegmentedNavItem>,
    /// 尺寸。
    nav_size: SegmentedNavSize,
    /// 切换回调。
    on_change: Option<Rc<dyn Fn(SharedString, &mut Window, &mut App)>>,
    /// 指示器滑动动画时长。
    duration: Duration,
    /// 用户样式。
    style: StyleRefinement,
}

impl SegmentedNav {
    /// 创建分段导航。
    pub fn new(id: impl Into<ElementId>, state: Entity<SegmentedNavState>) -> Self {
        Self {
            id: id.into(),
            state,
            items: Vec::new(),
            nav_size: SegmentedNavSize::default(),
            on_change: None,
            duration: durations::NORMAL,
            style: StyleRefinement::default(),
        }
    }

    /// 添加一个条目。
    pub fn item(mut self, id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        self.items.push(SegmentedNavItem {
            id: id.into(),
            label: label.into(),
        });
        self
    }

    /// 设置尺寸。
    pub fn size(mut self, size: SegmentedNavSize) -> Self {
        self.nav_size = size;
        self
    }

    /// 设置指示器动画时长。
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// 设置切换回调。
    pub fn on_change<F>(mut self, handler: F) -> Self
    where
        F: Fn(SharedString, &mut Window, &mut App) + 'static,
    {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for SegmentedNav {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for SegmentedNav {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // 先同步条目到状态（需要 &mut cx），再读取主题（&cx）。
        self.state.update(cx, |state, _| {
            state.items = self.items.clone();
        });

        let theme = cx.theme();
        let user_style = self.style;
        let state = self.state.read(cx);
        let active_id = state.active.clone();
        let item_count = self.items.len();
        let active_index = self.items.iter().position(|i| i.id == active_id);
        let animation_version = state.animation_version;
        let duration = self.duration;

        let item_fraction = if item_count > 0 {
            1.0 / item_count as f32
        } else {
            1.0
        };

        div()
            .id(self.id)
            .flex()
            .items_center()
            .relative()
            .bg(theme.tokens.muted)
            .rounded(theme.radius)
            .p(px(4.0))
            .h(self.nav_size.height())
            .when(active_index.is_some(), |this| {
                let idx = active_index.unwrap();
                this.child(
                    div()
                        .id("segmented-indicator")
                        .absolute()
                        .top(px(4.0))
                        .bottom(px(4.0))
                        .rounded(theme.radius)
                        .bg(theme.tokens.background)
                        .shadow(vec![BoxShadow {
                            color: hsla(0.0, 0.0, 0.0, 0.08),
                            offset: point(px(0.0), px(1.0)),
                            blur_radius: px(3.0),
                            spread_radius: px(0.0),
                            inset: false,
                        }])
                        .with_animation(
                            ElementId::Name(format!("seg-slide-{}", animation_version).into()),
                            Animation::new(duration).with_easing(easings::ease_out_cubic),
                            move |el, delta| {
                                let frac = item_fraction;
                                let left_pct = idx as f32 * frac * 100.0;
                                let width_pct = frac * 100.0;
                                el.left(relative(
                                    left_pct / 100.0 * delta + left_pct / 100.0 * (1.0 - delta),
                                ))
                                .w(relative(width_pct / 100.0))
                            },
                        ),
                )
            })
            .children(self.items.iter().enumerate().map(|(idx, item)| {
                let item_id = item.id.clone();
                let is_active = item.id == active_id;
                let on_change = self.on_change.clone();
                let state = self.state.clone();
                let click_id = item_id.clone();

                div()
                    .id(ElementId::Name(format!("seg-item-{}", idx).into()))
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h_full()
                    .px(self.nav_size.padding_x())
                    .text_size(self.nav_size.text_size())
                    .font_weight(if is_active {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(if is_active {
                        theme.tokens.foreground
                    } else {
                        theme.tokens.muted_foreground
                    })
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        state.update(cx, |state, cx| {
                            state.set_active(click_id.clone(), cx);
                        });
                        if let Some(handler) = on_change.as_ref() {
                            handler(item_id.clone(), window, cx);
                        }
                    })
                    .child(item.label.clone())
            }))
            .map(|this| {
                let mut el = this;
                el.style().refine(&user_style);
                el
            })
    }
}
