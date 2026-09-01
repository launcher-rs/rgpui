//! 通知组件，用于显示各种类型的消息通知和提示。

use std::{
    any::TypeId,
    borrow::Cow,
    collections::{HashMap, VecDeque},
    rc::Rc,
    time::Duration,
};

use crate::{
    ActiveTheme as _, Anchor, Animation, AnimationExt as _, AnyElement, App, AppContext,
    ClickEvent, Context, DismissEvent, ElementId, Entity, EventEmitter, Hsla, Icon, IconName,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement, StyleRefinement, Styled, Subscription, Window, div,
    prelude::FluentBuilder, px,
};
use crate::{Button, ButtonVariants as _, Sizable as _, StyledExt as _, cubic_bezier};
use crate::{h_flex, v_flex};

/// 通知类型，用于决定通知的默认图标与配色。
#[derive(Debug, Clone, Copy, Default)]
pub enum NotificationType {
    /// 信息
    #[default]
    Info,
    /// 成功
    Success,
    /// 警告
    Warning,
    /// 错误
    Error,
}

impl NotificationType {
    fn icon(&self, cx: &App) -> Icon {
        match self {
            Self::Info => Icon::new(IconName::Info).text_color(cx.theme().info),
            Self::Success => Icon::new(IconName::CircleCheck).text_color(cx.theme().success),
            Self::Warning => Icon::new(IconName::TriangleAlert).text_color(cx.theme().warning),
            Self::Error => Icon::new(IconName::CircleX).text_color(cx.theme().danger),
        }
    }

    /// 返回变体对应的强调色，用于左侧边框等视觉区分。
    fn variant_color(&self, cx: &App) -> Hsla {
        match self {
            Self::Info => cx.theme().info,
            Self::Success => cx.theme().success,
            Self::Warning => cx.theme().warning,
            Self::Error => cx.theme().danger,
        }
    }
}

/// 通知的唯一标识，用于精确定位需要关闭的通知。
#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub enum NotificationId {
    /// 仅按类型标识
    Id(TypeId),
    /// 按类型 + 元素标识
    IdAndElementId(TypeId, ElementId),
}

impl From<TypeId> for NotificationId {
    fn from(type_id: TypeId) -> Self {
        Self::Id(type_id)
    }
}

impl From<(TypeId, ElementId)> for NotificationId {
    fn from((type_id, id): (TypeId, ElementId)) -> Self {
        Self::IdAndElementId(type_id, id)
    }
}

/// 通知元素。
pub struct Notification {
    /// 用于保证通知唯一性的标识。
    /// 当推送相同 id 的通知时，之前的通知会被替换。
    ///
    /// None 表示通知将追加到列表末尾。
    id: NotificationId,
    style: StyleRefinement,
    type_: Option<NotificationType>,
    title: Option<SharedString>,
    message: Option<SharedString>,
    icon: Option<Icon>,
    autohide: bool,
    action_builder: Option<Rc<dyn Fn(&mut Self, &mut Window, &mut Context<Self>) -> Button>>,
    content_builder: Option<Rc<dyn Fn(&mut Self, &mut Window, &mut Context<Self>) -> AnyElement>>,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    closing: bool,
}

impl From<String> for Notification {
    fn from(s: String) -> Self {
        Self::new().message(s)
    }
}

impl From<SharedString> for Notification {
    fn from(s: SharedString) -> Self {
        Self::new().message(s)
    }
}

impl From<&str> for Notification {
    fn from(s: &str) -> Self {
        Self::new().message(s)
    }
}

impl<'a> From<Cow<'a, str>> for Notification {
    fn from(s: Cow<'a, str>) -> Self {
        Self::new().message(s)
    }
}

impl<T> From<(NotificationType, T)> for Notification
where
    T: Into<SharedString>,
{
    fn from((type_, content): (NotificationType, T)) -> Self {
        Self::new().message(content).with_type(type_)
    }
}

struct DefaultIdType;

impl Notification {
    /// 创建一个新通知。
    ///
    /// 默认 id 是一个随机 UUID。
    pub fn new() -> Self {
        let id: SharedString = uuid::Uuid::new_v4().to_string().into();
        let id = (TypeId::of::<DefaultIdType>(), id.into());

        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: None,
            message: None,
            type_: None,
            icon: None,
            autohide: true,
            action_builder: None,
            content_builder: None,
            on_click: None,
            on_close: None,
            closing: false,
        }
    }

    /// 设置通知的消息，默认是 None。
    pub fn message(mut self, message: impl Into<SharedString>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// 创建一个指定消息的信息通知。
    pub fn info(message: impl Into<SharedString>) -> Self {
        Self::new()
            .message(message)
            .with_type(NotificationType::Info)
    }

    /// 创建一个指定消息的成功通知。
    pub fn success(message: impl Into<SharedString>) -> Self {
        Self::new()
            .message(message)
            .with_type(NotificationType::Success)
    }

    /// 创建一个指定消息的警告通知。
    pub fn warning(message: impl Into<SharedString>) -> Self {
        Self::new()
            .message(message)
            .with_type(NotificationType::Warning)
    }

    /// 创建一个指定消息的错误通知。
    pub fn error(message: impl Into<SharedString>) -> Self {
        Self::new()
            .message(message)
            .with_type(NotificationType::Error)
    }

    /// 设置通知的类型，用于唯一标识通知。
    ///
    /// ```rs
    /// struct MyNotificationKind;
    /// let notification = Notification::new().message("Hello").id::<MyNotificationKind>();
    /// ```
    pub fn id<T: Sized + 'static>(mut self) -> Self {
        self.id = TypeId::of::<T>().into();
        self
    }

    /// 设置通知的类型和 id，用于唯一标识通知。
    pub fn id1<T: Sized + 'static>(mut self, key: impl Into<ElementId>) -> Self {
        self.id = (TypeId::of::<T>(), key.into()).into();
        self
    }

    /// 设置通知的标题，默认是 None。
    ///
    /// 如果标题为 None，则通知不显示标题。
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// 设置通知的图标。
    ///
    /// 如果图标为 None，则使用类型的默认图标。
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 设置通知的类型，默认是 NotificationType::Info。
    pub fn with_type(mut self, type_: NotificationType) -> Self {
        self.type_ = Some(type_);
        self
    }

    /// 设置通知是否自动隐藏，默认是 true。
    pub fn autohide(mut self, autohide: bool) -> Self {
        self.autohide = autohide;
        self
    }

    /// 设置通知的点击回调。
    pub fn on_click(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    /// 设置通知的关闭回调。
    ///
    /// 在通知被任何方式关闭时触发
    /// （关闭按钮、中键点击、自动隐藏、点击处理器或程序化关闭）。
    pub fn on_close(mut self, on_close: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(on_close));
        self
    }

    /// 设置通知的操作按钮。
    ///
    /// 设置操作后，通知将不会自动隐藏。
    pub fn action<F>(mut self, action: F) -> Self
    where
        F: Fn(&mut Self, &mut Window, &mut Context<Self>) -> Button + 'static,
    {
        self.action_builder = Some(Rc::new(action));
        self.autohide = false;
        self
    }

    /// 关闭通知。
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.closing = true;
        cx.notify();

        let on_close = self.on_close.clone();
        // 在 0.15 秒后关闭通知以显示关闭动画
        cx.spawn_in(window, async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_secs_f32(0.15))
                .await;
            _ = view.update_in(cx, |view, _, cx| {
                view.closing = false;
                cx.emit(DismissEvent);
                cx.notify();
            });
            if let Some(on_close) = on_close {
                _ = cx.update(|window, cx| on_close(window, cx));
            }
        })
        .detach();
    }

    /// 设置通知的内容。
    pub fn content(
        mut self,
        content: impl Fn(&mut Self, &mut Window, &mut Context<Self>) -> AnyElement + 'static,
    ) -> Self {
        self.content_builder = Some(Rc::new(content));
        self
    }
}

impl EventEmitter<DismissEvent> for Notification {}
impl FluentBuilder for Notification {}
impl Styled for Notification {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Render for Notification {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = self
            .content_builder
            .clone()
            .map(|builder| builder(self, window, cx));
        let action = self
            .action_builder
            .clone()
            .map(|builder| builder(self, window, cx).small().mr_3p5());

        let closing = self.closing;
        let icon = match self.type_ {
            None => self.icon.clone(),
            Some(type_) => Some(type_.icon(cx)),
        };
        let has_icon = icon.is_some();
        let placement = cx.theme().notification.placement;
        let variant_color = self.type_.map(|t| t.variant_color(cx));

        h_flex()
            .id("notification")
            .group("")
            .occlude()
            .relative()
            .w_112()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().tokens.popover)
            .rounded(cx.theme().radius_lg)
            .shadow_md()
            .py_3p5()
            .px_4()
            .gap_3()
            .refine_style(&self.style)
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w_1()
                    .rounded_l(cx.theme().radius_lg)
                    .when_some(variant_color, |this, color| this.bg(color)),
            )
            .when_some(icon, |this, icon| {
                this.child(div().absolute().top(px(18.)).left_4().child(icon))
            })
            .child(
                v_flex()
                    .flex_1()
                    .overflow_hidden()
                    .when(has_icon, |this| this.pl_6())
                    .when_some(self.title.clone(), |this, title| {
                        this.child(div().text_sm().font_semibold().child(title))
                    })
                    .when_some(self.message.clone(), |this, message| {
                        this.child(div().text_sm().child(message))
                    })
                    .when_some(content, |this, content| this.child(content)),
            )
            .when_some(action, |this, action| this.child(action))
            .child(
                div()
                    .absolute()
                    .top_1()
                    .right_1()
                    .invisible()
                    .group_hover("", |this| this.visible())
                    .child(
                        Button::new("close")
                            .icon(IconName::Close)
                            .ghost()
                            .xsmall()
                            .on_click(cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                this.dismiss(window, cx);
                            })),
                    ),
            )
            .when_some(self.on_click.clone(), |this, on_click| {
                this.on_click(cx.listener(move |view, event, window, cx| {
                    view.dismiss(window, cx);
                    on_click(event, window, cx);
                }))
            })
            .on_aux_click(cx.listener(move |view, event: &ClickEvent, window, cx| {
                if event.is_middle_click() {
                    view.dismiss(window, cx);
                }
            }))
            .with_animation(
                ElementId::NamedInteger("slide-down".into(), closing as u64),
                Animation::new(Duration::from_secs_f64(0.25))
                    .with_easing(cubic_bezier(0.4, 0., 0.2, 1.)),
                move |this, delta| {
                    if closing {
                        let opacity = 1. - delta;
                        let that = this
                            .shadow_none()
                            .opacity(opacity)
                            .when(opacity < 0.85, |this| this.shadow_none());
                        match placement {
                            Anchor::TopRight | Anchor::BottomRight => {
                                let x_offset = px(0.) + delta * px(45.);
                                that.left(px(0.) + x_offset)
                            }
                            Anchor::TopLeft | Anchor::BottomLeft => {
                                let x_offset = px(0.) - delta * px(45.);
                                that.left(px(0.) + x_offset)
                            }
                            Anchor::TopCenter => {
                                let y_offset = px(0.) - delta * px(45.);
                                that.top(px(0.) + y_offset)
                            }
                            Anchor::BottomCenter => {
                                let y_offset = px(0.) + delta * px(45.);
                                that.top(px(0.) + y_offset)
                            }
                            _ => that,
                        }
                    } else {
                        let y_offset = match placement {
                            Anchor::TopLeft | Anchor::TopRight | Anchor::TopCenter => {
                                px(-45.) + delta * px(45.)
                            }
                            Anchor::BottomLeft | Anchor::BottomRight | Anchor::BottomCenter => {
                                px(45.) - delta * px(45.)
                            }
                            _ => px(0.),
                        };
                        let opacity = delta;
                        this.top(px(0.) + y_offset)
                            .opacity(opacity)
                            .when(opacity < 0.85, |this| this.shadow_none())
                    }
                },
            )
    }
}

/// 通知列表。
pub struct NotificationList {
    /// 将被自动隐藏的通知。
    pub(crate) notifications: VecDeque<Entity<Notification>>,
    expanded: bool,
    _subscriptions: HashMap<NotificationId, Subscription>,
}

impl NotificationList {
    /// 创建一个通知列表。
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            notifications: VecDeque::new(),
            expanded: false,
            _subscriptions: HashMap::new(),
        }
    }

    /// 推送一个通知。
    pub fn push(
        &mut self,
        notification: impl Into<Notification>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let notification = notification.into();
        let id = notification.id.clone();
        let autohide = notification.autohide;

        // 按 id 移除通知，保证唯一性
        self.notifications.retain(|note| note.read(cx).id != id);

        let notification = cx.new(|_| notification);

        self._subscriptions.insert(
            id.clone(),
            cx.subscribe(&notification, move |view, _, _: &DismissEvent, cx| {
                view.notifications.retain(|note| id != note.read(cx).id);
                view._subscriptions.remove(&id);
            }),
        );

        self.notifications.push_back(notification.clone());
        if autohide {
            // 休眠 5 秒后自动隐藏通知
            cx.spawn_in(window, async move |_, cx| {
                cx.background_executor().timer(Duration::from_secs(5)).await;

                if let Err(err) =
                    notification.update_in(cx, |note, window, cx| note.dismiss(window, cx))
                {
                    tracing::error!("failed to auto hide notification: {:?}", err);
                }
            })
            .detach();
        }
        cx.notify();
    }

    /// 关闭指定 id 的通知。
    pub fn close(
        &mut self,
        id: impl Into<NotificationId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id: NotificationId = id.into();
        if let Some(n) = self.notifications.iter().find(|n| n.read(cx).id == id) {
            n.update(cx, |note, cx| note.dismiss(window, cx))
        }
        cx.notify();
    }

    /// 关闭所有 id 匹配给定 [`TypeId`] 的通知，无论它们是通过 [`Notification::id`] 还是 [`Notification::id1`] 注册的。
    pub fn close_by_type(&mut self, type_id: TypeId, window: &mut Window, cx: &mut Context<Self>) {
        let matched: Vec<_> = self
            .notifications
            .iter()
            .filter(|n| match &n.read(cx).id {
                NotificationId::Id(t) | NotificationId::IdAndElementId(t, _) => *t == type_id,
            })
            .cloned()
            .collect();
        for n in matched {
            n.update(cx, |note, cx| note.dismiss(window, cx));
        }
        cx.notify();
    }

    /// 清空所有通知。
    pub fn clear(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.notifications.clear();
        cx.notify();
    }

    /// 获取当前通知列表。
    pub fn notifications(&self) -> Vec<Entity<Notification>> {
        self.notifications.iter().cloned().collect()
    }
}

impl Render for NotificationList {
    fn render(
        &mut self,
        window: &mut crate::Window,
        cx: &mut crate::Context<Self>,
    ) -> impl IntoElement {
        let size = window.viewport_size();
        let max_items = cx.theme().notification.max_items;
        let items = self
            .notifications
            .iter()
            .rev()
            .take(max_items)
            .rev()
            .cloned();

        let placement = cx.theme().notification.placement;
        let margins = &cx.theme().notification.margins;

        v_flex()
            .id("notification-list")
            .max_h(size.height)
            .pt(margins.top)
            .pb(margins.bottom)
            .gap_3()
            .when(
                matches!(placement, Anchor::TopRight),
                |this| this.pr(margins.right), // 忽略左边距
            )
            .when(
                matches!(placement, Anchor::TopLeft),
                |this| this.pl(margins.left), // 忽略右边距
            )
            .when(
                matches!(placement, Anchor::BottomLeft),
                |this| this.flex_col_reverse().pl(margins.left), // 忽略右边距
            )
            .when(
                matches!(placement, Anchor::BottomRight),
                |this| this.flex_col_reverse().pr(margins.right), // 忽略左边距
            )
            .when(matches!(placement, Anchor::BottomCenter), |this| {
                this.flex_col_reverse()
            })
            .on_hover(cx.listener(|view, hovered, _, cx| {
                view.expanded = *hovered;
                cx.notify()
            }))
            .children(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::{TestAppContext, VisualTestContext};

    struct FooKind;
    struct BarKind;

    struct TestRoot {
        list: Entity<NotificationList>,
    }

    impl Render for TestRoot {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.list.clone()
        }
    }

    fn ids(list: &Entity<NotificationList>, cx: &mut VisualTestContext) -> Vec<NotificationId> {
        list.read_with(cx, |l, cx| {
            l.notifications
                .iter()
                .map(|n| n.read(cx).id.clone())
                .collect()
        })
    }

    /// 驱动关闭动画定时器并传播产生的 `DismissEvent`，
    /// 使关闭的通知从列表中移除。
    fn flush_dismiss(cx: &mut VisualTestContext) {
        cx.background_executor
            .advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
    }

    #[rgpui::test]
    fn close_by_type_removes_id_and_all_id1_of_same_type(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_global(Theme::default()));
        let (root, cx) = cx.add_window_view(|window, cx| TestRoot {
            list: cx.new(|cx| NotificationList::new(window, cx)),
        });
        let list = root.read_with(cx, |r, _| r.list.clone());

        list.update_in(cx, |list, window, cx| {
            list.push(
                Notification::info("plain").id::<FooKind>().autohide(false),
                window,
                cx,
            );
            list.push(
                Notification::info("a").id1::<FooKind>(1).autohide(false),
                window,
                cx,
            );
            list.push(
                Notification::info("b").id1::<FooKind>(2).autohide(false),
                window,
                cx,
            );
            list.push(
                Notification::info("bar").id::<BarKind>().autohide(false),
                window,
                cx,
            );
        });
        cx.run_until_parked();
        assert_eq!(ids(&list, cx).len(), 4);

        list.update_in(cx, |list, window, cx| {
            list.close_by_type(TypeId::of::<FooKind>(), window, cx);
        });
        flush_dismiss(cx);

        let remaining = ids(&list, cx);
        assert_eq!(
            remaining,
            vec![NotificationId::Id(TypeId::of::<BarKind>())],
            "只有 BarKind 通知应该保留"
        );
    }

    #[rgpui::test]
    fn close_with_id_and_element_id_removes_only_matching_key(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_global(Theme::default()));
        let (root, cx) = cx.add_window_view(|window, cx| TestRoot {
            list: cx.new(|cx| NotificationList::new(window, cx)),
        });
        let list = root.read_with(cx, |r, _| r.list.clone());

        list.update_in(cx, |list, window, cx| {
            list.push(
                Notification::info("a").id1::<FooKind>(1).autohide(false),
                window,
                cx,
            );
            list.push(
                Notification::info("b").id1::<FooKind>(2).autohide(false),
                window,
                cx,
            );
            list.push(
                Notification::info("plain").id::<FooKind>().autohide(false),
                window,
                cx,
            );
        });

        list.update_in(cx, |list, window, cx| {
            list.close(
                (TypeId::of::<FooKind>(), ElementId::from(1usize)),
                window,
                cx,
            );
        });
        flush_dismiss(cx);

        let remaining = ids(&list, cx);
        assert_eq!(remaining.len(), 2);
        assert!(remaining.contains(&NotificationId::IdAndElementId(
            TypeId::of::<FooKind>(),
            ElementId::from(2usize),
        )));
        assert!(remaining.contains(&NotificationId::Id(TypeId::of::<FooKind>())));
    }

    #[rgpui::test]
    fn close_with_only_type_id_does_not_match_id1_entries(cx: &mut TestAppContext) {
        // 普通的 `close(TypeId)` 形式（用于旧代码路径）必须保持其窄语义：
        // 只匹配 `NotificationId::Id`，不匹配 `NotificationId::IdAndElementId`。
        // 新的 `close_by_type` 是宽泛的形式。
        cx.update(|cx| cx.set_global(Theme::default()));
        let (root, cx) = cx.add_window_view(|window, cx| TestRoot {
            list: cx.new(|cx| NotificationList::new(window, cx)),
        });
        let list = root.read_with(cx, |r, _| r.list.clone());

        list.update_in(cx, |list, window, cx| {
            list.push(
                Notification::info("a").id1::<FooKind>(1).autohide(false),
                window,
                cx,
            );
        });

        list.update_in(cx, |list, window, cx| {
            list.close(TypeId::of::<FooKind>(), window, cx);
        });
        flush_dismiss(cx);

        assert_eq!(ids(&list, cx).len(), 1, "id1 条目应该保持不变");
    }

    #[rgpui::test]
    fn close_by_type_with_no_match_is_noop(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_global(Theme::default()));
        let (root, cx) = cx.add_window_view(|window, cx| TestRoot {
            list: cx.new(|cx| NotificationList::new(window, cx)),
        });
        let list = root.read_with(cx, |r, _| r.list.clone());

        list.update_in(cx, |list, window, cx| {
            list.push(
                Notification::info("bar").id::<BarKind>().autohide(false),
                window,
                cx,
            );
        });

        list.update_in(cx, |list, window, cx| {
            list.close_by_type(TypeId::of::<FooKind>(), window, cx);
        });
        flush_dismiss(cx);

        assert_eq!(ids(&list, cx).len(), 1);
    }
}
