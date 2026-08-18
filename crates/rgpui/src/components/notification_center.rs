//! 通知中心组件：通知列表、通知铃铛图标。

use crate::components::empty_state::{EmptyState, EmptyStateSize};
use crate::{prelude::FluentBuilder as _, *};
use std::rc::Rc;

/// 通知变体类型。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum NotificationVariant {
    /// 信息通知。
    #[default]
    Info,
    /// 成功通知。
    Success,
    /// 警告通知。
    Warning,
    /// 错误通知。
    Error,
}

impl NotificationVariant {
    /// 获取变体对应的图标。
    fn icon_name(&self) -> IconName {
        match self {
            NotificationVariant::Info => IconName::Info,
            NotificationVariant::Success => IconName::CircleCheck,
            NotificationVariant::Warning => IconName::TriangleAlert,
            NotificationVariant::Error => IconName::CircleX,
        }
    }

    /// 获取变体对应的强调色。
    fn color(&self, theme: &crate::Theme) -> Hsla {
        match self {
            NotificationVariant::Info => theme.tokens.primary.color,
            NotificationVariant::Success => theme.tokens.success.color,
            NotificationVariant::Warning => theme.tokens.warning.color,
            NotificationVariant::Error => theme.tokens.danger.color,
        }
    }
}

/// 通知上的操作按钮。
#[derive(Clone)]
pub struct NotificationAction {
    /// 按钮标签。
    pub label: SharedString,
    /// 点击回调。
    pub handler: Rc<dyn Fn(&mut Window, &mut App)>,
}

/// 单条通知条目。
#[derive(Clone)]
pub struct NotificationItem {
    /// 通知 ID。
    pub id: ElementId,
    /// 标题。
    pub title: SharedString,
    /// 消息正文。
    pub message: Option<SharedString>,
    /// 时间戳文本。
    pub timestamp: Option<SharedString>,
    /// 变体类型。
    pub variant: NotificationVariant,
    /// 是否已读。
    pub read: bool,
    /// 自定义图标。
    pub icon: Option<IconName>,
    /// 操作按钮。
    pub action: Option<NotificationAction>,
}

impl NotificationItem {
    /// 创建通知条目。
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            message: None,
            timestamp: None,
            variant: NotificationVariant::default(),
            read: false,
            icon: None,
            action: None,
        }
    }

    /// 设置消息正文。
    pub fn message(mut self, message: impl Into<SharedString>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// 设置时间戳文本。
    pub fn timestamp(mut self, timestamp: impl Into<SharedString>) -> Self {
        self.timestamp = Some(timestamp.into());
        self
    }

    /// 设置变体类型。
    pub fn variant(mut self, variant: NotificationVariant) -> Self {
        self.variant = variant;
        self
    }

    /// 设置已读状态。
    pub fn read(mut self, read: bool) -> Self {
        self.read = read;
        self
    }

    /// 设置自定义图标。
    pub fn icon(mut self, icon: impl Into<IconName>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 设置操作按钮。
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some(NotificationAction {
            label: label.into(),
            handler: Rc::new(handler),
        });
        self
    }
}

/// 通知中心状态。
pub struct NotificationCenterState {
    /// 通知列表（新通知插到最前）。
    notifications: Vec<NotificationItem>,
    /// 焦点句柄。
    _focus_handle: FocusHandle,
}

impl NotificationCenterState {
    /// 创建状态。
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            notifications: Vec::new(),
            _focus_handle: cx.focus_handle(),
        }
    }

    /// 添加一条通知。
    pub fn add(&mut self, notification: NotificationItem, cx: &mut Context<Self>) {
        self.notifications.insert(0, notification);
        cx.notify();
    }

    /// 按 ID 移除通知。
    pub fn remove(&mut self, id: &ElementId, cx: &mut Context<Self>) {
        self.notifications.retain(|n| &n.id != id);
        cx.notify();
    }

    /// 将指定通知标记为已读。
    pub fn mark_read(&mut self, id: &ElementId, cx: &mut Context<Self>) {
        if let Some(notification) = self.notifications.iter_mut().find(|n| &n.id == id) {
            notification.read = true;
            cx.notify();
        }
    }

    /// 标记全部已读。
    pub fn mark_all_read(&mut self, cx: &mut Context<Self>) {
        for notification in &mut self.notifications {
            notification.read = true;
        }
        cx.notify();
    }

    /// 清空全部通知。
    pub fn clear_all(&mut self, cx: &mut Context<Self>) {
        self.notifications.clear();
        cx.notify();
    }

    /// 获取未读数量。
    pub fn unread_count(&self) -> usize {
        self.notifications.iter().filter(|n| !n.read).count()
    }

    /// 获取通知列表。
    pub fn notifications(&self) -> &[NotificationItem] {
        &self.notifications
    }

    /// 判断列表是否为空。
    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
    }
}

impl EventEmitter<()> for NotificationCenterState {}

/// 通知中心组件。
#[derive(IntoElement)]
pub struct NotificationCenter {
    /// 状态实体。
    state: Entity<NotificationCenterState>,
    /// 最大可见条数。
    max_visible: usize,
    /// 是否显示时间戳。
    show_timestamps: bool,
    /// 是否按日期分组。
    group_by_date: bool,
    /// 点击通知的回调。
    on_notification_click: Option<Rc<dyn Fn(&NotificationItem, &mut Window, &mut App)>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl NotificationCenter {
    /// 创建通知中心。
    pub fn new(state: Entity<NotificationCenterState>) -> Self {
        Self {
            state,
            max_visible: 10,
            show_timestamps: true,
            group_by_date: false,
            on_notification_click: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置最大可见条数。
    pub fn max_visible(mut self, max: usize) -> Self {
        self.max_visible = max;
        self
    }

    /// 设置是否显示时间戳。
    pub fn show_timestamps(mut self, show: bool) -> Self {
        self.show_timestamps = show;
        self
    }

    /// 设置是否按日期分组。
    pub fn group_by_date(mut self, group: bool) -> Self {
        self.group_by_date = group;
        self
    }

    /// 设置点击通知的回调。
    pub fn on_notification_click(
        mut self,
        handler: impl Fn(&NotificationItem, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_notification_click = Some(Rc::new(handler));
        self
    }
}

impl Styled for NotificationCenter {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for NotificationCenter {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;
        let state = self.state.read(cx);
        let notifications = state.notifications().to_vec();
        let is_empty = notifications.is_empty();
        let total_count = notifications.len();
        let show_more = total_count > self.max_visible;
        let visible_notifications: Vec<_> =
            notifications.into_iter().take(self.max_visible).collect();

        let state_entity = self.state.clone();
        let on_click = self.on_notification_click.clone();
        let show_timestamps = self.show_timestamps;

        let radius_lg = theme.radius_lg;
        let radius = theme.radius;
        let font_family = theme.font_family.clone();
        let popover = theme.tokens.popover;
        let border = theme.tokens.border;
        let foreground = theme.tokens.foreground;
        let muted_foreground = theme.tokens.muted_foreground;
        let accent = theme.tokens.accent;

        let shadow = BoxShadow {
            color: hsla(0.0, 0.0, 0.0, 0.2),
            offset: point(px(0.0), px(4.0)),
            blur_radius: px(12.0),
            spread_radius: px(0.0),
            inset: false,
        };

        div()
            .flex()
            .flex_col()
            .w(px(380.0))
            .max_h(px(500.0))
            .bg(popover)
            .border_1()
            .border_color(border)
            .rounded(radius_lg)
            .shadow(vec![shadow])
            .overflow_hidden()
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(foreground)
                            .font_family(font_family.clone())
                            .child("Notifications"),
                    )
                    .when(!is_empty, {
                        let state_clone = state_entity.clone();
                        |d| {
                            d.child(
                                Button::new("mark-all-read")
                                    .label("Mark all read")
                                    .ghost()
                                    .small()
                                    .on_click(move |_, _, cx| {
                                        state_clone.update(cx, |state, cx| {
                                            state.mark_all_read(cx);
                                        });
                                    }),
                            )
                        }
                    }),
            )
            .when(is_empty, |d| {
                d.child(
                    EmptyState::new("notification-empty", "No notifications")
                        .icon(IconName::Inbox)
                        .description("You're all caught up!")
                        .size(EmptyStateSize::Sm)
                        .py(px(32.0)),
                )
            })
            .when(!is_empty, |d| {
                d.child(
                    div().overflow_y_scrollbar().max_h(px(350.0)).child(
                        div()
                            .flex()
                            .flex_col()
                            .children(visible_notifications.into_iter().map(|notification| {
                                let id = notification.id.clone();
                                let state_for_click = state_entity.clone();
                                let state_for_dismiss = state_entity.clone();
                                let on_click_handler = on_click.clone();
                                let notification_clone = notification.clone();
                                let is_read = notification.read;
                                let variant = notification.variant;
                                let variant_color = variant.color(&theme);

                                div()
                                    .id(id.clone())
                                    .flex()
                                    .gap(px(12.0))
                                    .px(px(16.0))
                                    .py(px(12.0))
                                    .border_b_1()
                                    .border_color(border)
                                    .bg(if is_read {
                                        crate::transparent_black()
                                    } else {
                                        accent.opacity(0.3)
                                    })
                                    .cursor(CursorStyle::PointingHand)
                                    .hover(|style| style.bg(accent))
                                    .on_mouse_down(MouseButton::Left, {
                                        let id = id.clone();
                                        move |_, window, cx| {
                                            state_for_click.update(cx, |state, cx| {
                                                state.mark_read(&id, cx);
                                            });
                                            if let Some(ref handler) = on_click_handler {
                                                handler(&notification_clone, window, cx);
                                            }
                                        }
                                    })
                                    .child(
                                        div().flex_shrink_0().mt(px(2.0)).child(
                                            Icon::new(
                                                notification
                                                    .icon
                                                    .clone()
                                                    .unwrap_or_else(|| variant.icon_name()),
                                            )
                                            .with_size(px(18.0))
                                            .text_color(variant_color),
                                        ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .flex_1()
                                            .gap(px(4.0))
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .gap(px(8.0))
                                                    .child(
                                                        div()
                                                            .text_size(px(13.0))
                                                            .font_weight(if is_read {
                                                                FontWeight::NORMAL
                                                            } else {
                                                                FontWeight::SEMIBOLD
                                                            })
                                                            .text_color(foreground)
                                                            .font_family(font_family.clone())
                                                            .truncate()
                                                            .child(notification.title.clone()),
                                                    )
                                                    .when(
                                                        show_timestamps
                                                            && notification.timestamp.is_some(),
                                                        |d| {
                                                            d.child(
                                                                div()
                                                                    .flex_shrink_0()
                                                                    .text_size(px(11.0))
                                                                    .text_color(muted_foreground)
                                                                    .font_family(
                                                                        font_family.clone(),
                                                                    )
                                                                    .child(
                                                                        notification
                                                                            .timestamp
                                                                            .clone()
                                                                            .unwrap_or_default(),
                                                                    ),
                                                            )
                                                        },
                                                    ),
                                            )
                                            .when_some(notification.message.clone(), |d, msg| {
                                                d.child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .text_color(muted_foreground)
                                                        .font_family(font_family.clone())
                                                        .line_height(relative(1.4))
                                                        .child(msg),
                                                )
                                            })
                                            .when_some(notification.action, |d, action| {
                                                let handler = action.handler.clone();
                                                d.child(
                                                    div().mt(px(4.0)).child(
                                                        Button::new(ElementId::Name(
                                                            format!("action-{:?}", id).into(),
                                                        ))
                                                        .label(action.label.clone())
                                                        .outline()
                                                        .small()
                                                        .on_click(move |_, window, cx| {
                                                            (handler)(window, cx);
                                                        }),
                                                    ),
                                                )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .w(px(20.0))
                                            .h(px(20.0))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded(radius)
                                            .text_color(muted_foreground)
                                            .text_size(px(14.0))
                                            .font_family(font_family.clone())
                                            .hover(|style| style.bg(accent))
                                            .on_mouse_down(MouseButton::Left, {
                                                move |_, _, cx| {
                                                    cx.stop_propagation();
                                                    state_for_dismiss.update(cx, |state, cx| {
                                                        state.remove(&id, cx);
                                                    });
                                                }
                                            })
                                            .child(
                                                Icon::new(IconName::Close)
                                                    .with_size(px(14.0))
                                                    .text_color(muted_foreground),
                                            ),
                                    )
                            })),
                    ),
                )
            })
            .when(show_more, |d| {
                d.child(
                    div()
                        .px(px(16.0))
                        .py(px(8.0))
                        .border_t_1()
                        .border_color(border)
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(muted_foreground)
                                .font_family(font_family.clone())
                                .text_align(TextAlign::Center)
                                .child(format!(
                                    "+ {} more notifications",
                                    total_count - self.max_visible
                                )),
                        ),
                )
            })
            .when(!is_empty, {
                let state_clone = state_entity;
                |d| {
                    d.child(
                        div()
                            .flex()
                            .justify_center()
                            .px(px(16.0))
                            .py(px(8.0))
                            .border_t_1()
                            .border_color(border)
                            .child(
                                Button::new("clear-all")
                                    .label("Clear all")
                                    .ghost()
                                    .small()
                                    .on_click(move |_, _, cx| {
                                        state_clone.update(cx, |state, cx| {
                                            state.clear_all(cx);
                                        });
                                    }),
                            ),
                    )
                }
            })
    }
}

/// 通知铃铛图标组件。
#[derive(IntoElement)]
pub struct NotificationBell {
    /// 元素 ID。
    id: ElementId,
    /// 状态实体。
    state: Entity<NotificationCenterState>,
    /// 点击回调。
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl NotificationBell {
    /// 创建通知铃铛。
    pub fn new(state: Entity<NotificationCenterState>) -> Self {
        Self {
            id: ElementId::Name("notification-bell".into()),
            state,
            on_click: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置元素 ID。
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// 设置点击回调。
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Styled for NotificationBell {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for NotificationBell {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;
        let unread_count = self.state.read(cx).unread_count();
        let on_click = self.on_click.clone();

        let radius = theme.radius;
        let font_family = theme.font_family.clone();
        let foreground = theme.tokens.foreground;
        let accent = theme.tokens.accent;
        let danger = theme.tokens.danger;
        let danger_foreground = theme.tokens.danger_foreground;

        div()
            .id(self.id)
            .relative()
            .w(px(40.0))
            .h(px(40.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(radius)
            .cursor(CursorStyle::PointingHand)
            .hover(|style| style.bg(accent))
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
            .when_some(on_click, |d, handler| {
                d.on_click(move |event, window, cx| {
                    (handler)(event, window, cx);
                })
            })
            .child(
                Icon::new(IconName::Bell)
                    .with_size(px(20.0))
                    .text_color(foreground),
            )
            .when(unread_count > 0, |d| {
                d.child(
                    div()
                        .absolute()
                        .top(px(4.0))
                        .right(px(4.0))
                        .min_w(px(18.0))
                        .h(px(18.0))
                        .px(px(5.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(danger)
                        .text_size(px(10.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(danger_foreground)
                        .font_family(font_family)
                        .child(if unread_count > 99 {
                            "99+".to_string()
                        } else {
                            unread_count.to_string()
                        }),
                )
            })
    }
}
