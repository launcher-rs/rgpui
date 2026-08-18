//! 空状态组件：无数据时的占位展示，支持图标、标题、描述与操作按钮。

use rgpui::{prelude::FluentBuilder as _, *};
use std::rc::Rc;

/// 空状态尺寸。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum EmptyStateSize {
    /// 小尺寸。
    Sm,
    /// 中等尺寸（默认）。
    #[default]
    Md,
    /// 大尺寸。
    Lg,
}

impl EmptyStateSize {
    /// 获取图标尺寸。
    fn icon_size(self) -> Pixels {
        match self {
            EmptyStateSize::Sm => px(32.0),
            EmptyStateSize::Md => px(48.0),
            EmptyStateSize::Lg => px(64.0),
        }
    }

    /// 获取标题字号。
    fn title_size(self) -> Pixels {
        match self {
            EmptyStateSize::Sm => px(14.0),
            EmptyStateSize::Md => px(18.0),
            EmptyStateSize::Lg => px(24.0),
        }
    }

    /// 获取描述字号。
    fn description_size(self) -> Pixels {
        match self {
            EmptyStateSize::Sm => px(12.0),
            EmptyStateSize::Md => px(14.0),
            EmptyStateSize::Lg => px(16.0),
        }
    }

    /// 获取内部间距。
    fn gap(self) -> Pixels {
        match self {
            EmptyStateSize::Sm => px(12.0),
            EmptyStateSize::Md => px(16.0),
            EmptyStateSize::Lg => px(20.0),
        }
    }
}

/// 空状态组件。
#[derive(IntoElement)]
pub struct EmptyState {
    /// 元素 ID。
    id: ElementId,
    /// 图标。
    icon: Option<Icon>,
    /// 标题。
    title: SharedString,
    /// 描述。
    description: Option<SharedString>,
    /// 主操作按钮（标签 + 回调）。
    action: Option<(SharedString, Rc<dyn Fn(&mut Window, &mut App)>)>,
    /// 次操作按钮（标签 + 回调）。
    secondary_action: Option<(SharedString, Rc<dyn Fn(&mut Window, &mut App)>)>,
    /// 尺寸。
    size: EmptyStateSize,
    /// 用户样式。
    style: StyleRefinement,
}

impl EmptyState {
    /// 创建空状态组件。
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon: None,
            title: title.into(),
            description: None,
            action: None,
            secondary_action: None,
            size: EmptyStateSize::default(),
            style: StyleRefinement::default(),
        }
    }

    /// 设置图标。
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 设置描述文本。
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 设置主操作按钮。
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(handler)));
        self
    }

    /// 设置次操作按钮。
    pub fn secondary_action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.secondary_action = Some((label.into(), Rc::new(handler)));
        self
    }

    /// 设置尺寸。
    pub fn size(mut self, size: EmptyStateSize) -> Self {
        self.size = size;
        self
    }
}

impl Styled for EmptyState {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for EmptyState {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;
        let icon_size = self.size.icon_size();
        let title_size = self.size.title_size();
        let description_size = self.size.description_size();
        let gap = self.size.gap();
        let id = self.id.clone();
        let font_family = theme.font_family.clone();
        let muted_foreground = theme.tokens.muted_foreground;
        let foreground = theme.tokens.foreground;

        div()
            .id(self.id)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(gap)
            .p(px(24.0))
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
            .when_some(self.icon, |d, icon| {
                d.child(
                    Icon::new(icon)
                        .with_size(icon_size)
                        .text_color(muted_foreground),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(title_size)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(foreground)
                            .font_family(font_family.clone())
                            .text_align(TextAlign::Center)
                            .child(self.title),
                    )
                    .when_some(self.description, |d, desc| {
                        d.child(
                            div()
                                .text_size(description_size)
                                .text_color(muted_foreground)
                                .font_family(font_family.clone())
                                .text_align(TextAlign::Center)
                                .max_w(px(320.0))
                                .child(desc),
                        )
                    }),
            )
            .when(
                self.action.is_some() || self.secondary_action.is_some(),
                |d| {
                    d.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .mt(px(8.0))
                            .when_some(self.action, |d, (label, handler)| {
                                let handler_clone = handler.clone();
                                d.child(
                                    Button::new(ElementId::Name(format!("{}-action", id).into()))
                                        .label(label)
                                        .on_click(move |_, window, cx| {
                                            (handler_clone)(window, cx);
                                        }),
                                )
                            })
                            .when_some(self.secondary_action, |d, (label, handler)| {
                                let handler_clone = handler.clone();
                                d.child(
                                    Button::new(ElementId::Name(
                                        format!("{}-secondary", id).into(),
                                    ))
                                    .label(label)
                                    .ghost()
                                    .on_click(
                                        move |_, window, cx| {
                                            (handler_clone)(window, cx);
                                        },
                                    ),
                                )
                            }),
                    )
                },
            )
    }
}
