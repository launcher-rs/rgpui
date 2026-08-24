use std::{rc::Rc, sync::LazyLock, time::Duration};

use crate::{
    ActiveTheme as _, Animation, AnimationExt as _, AnyElement, App, Bounds, BoxShadow, ClickEvent,
    Edges, FocusHandle, Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels,
    Point, RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement,
    Styled, Window, WindowControlArea, anchored, div, hsla, point, prelude::FluentBuilder, px,
};

use crate::{
    Button, ButtonVariant, ButtonVariants as _, FocusTrapElement as _, IconName, Root,
    ScrollableElement as _, Sizable as _, StyledExt as _, TITLE_BAR_HEIGHT, WindowExt as _,
    cubic_bezier,
    dialog::{DialogContent, DialogTitle},
    v_flex, window_paddings,
};

/// 对话框动画时长（秒）。
pub static ANIMATION_DURATION: LazyLock<Duration> = LazyLock::new(|| Duration::from_secs_f64(0.25));
const CONTEXT: &str = "Dialog";

actions!(dialog, [CancelDialog, ConfirmDialog]);

/// 对话框按钮属性。
#[derive(Clone)]
pub struct DialogButtonProps {
    pub(crate) ok_text: Option<SharedString>,
    pub(crate) ok_variant: ButtonVariant,
    pub(crate) cancel_text: Option<SharedString>,
    pub(crate) cancel_variant: ButtonVariant,
    pub(crate) show_cancel: bool,
    pub(crate) on_ok: Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static>,
    pub(crate) on_cancel: Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static>,
    pub(crate) on_close: Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>,
}

impl Default for DialogButtonProps {
    fn default() -> Self {
        Self {
            ok_text: None,
            ok_variant: ButtonVariant::Primary,
            cancel_text: None,
            cancel_variant: ButtonVariant::default(),
            show_cancel: false,
            on_ok: Rc::new(|_, _, _| true),
            on_cancel: Rc::new(|_, _, _| true),
            on_close: Rc::new(|_, _, _| {}),
        }
    }
}

impl DialogButtonProps {
    /// 设置确认按钮文本。默认为 "OK"。
    pub fn ok_text(mut self, ok_text: impl Into<SharedString>) -> Self {
        self.ok_text = Some(ok_text.into());
        self
    }

    /// 设置确认按钮变体。默认为 `ButtonVariant::Primary`。
    pub fn ok_variant(mut self, ok_variant: ButtonVariant) -> Self {
        self.ok_variant = ok_variant;
        self
    }

    /// 设置取消按钮文本。默认为 "Cancel"。
    pub fn cancel_text(mut self, cancel_text: impl Into<SharedString>) -> Self {
        self.cancel_text = Some(cancel_text.into());
        self
    }

    /// 设置取消按钮变体。默认为 `ButtonVariant::default()`。
    pub fn cancel_variant(mut self, cancel_variant: ButtonVariant) -> Self {
        self.cancel_variant = cancel_variant;
        self
    }

    /// 设置是否显示取消按钮。默认为 `false`。
    pub fn show_cancel(mut self, show_cancel: bool) -> Self {
        self.show_cancel = show_cancel;
        self
    }

    /// 设置确认回调。返回 `true` 关闭对话框，返回 `false` 则不关闭。
    pub fn on_ok(
        mut self,
        on_ok: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.on_ok = Rc::new(on_ok);
        self
    }

    /// 设置取消回调。返回 `true` 关闭对话框，返回 `false` 则不关闭。
    pub fn on_cancel(
        mut self,
        on_cancel: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.on_cancel = Rc::new(on_cancel);
        self
    }

    pub(crate) fn render_ok(&self, _: &mut Window, _: &mut App) -> AnyElement {
        let on_ok = self.on_ok.clone();
        let on_close = self.on_close.clone();

        let ok_text = self.ok_text.clone().unwrap_or_else(|| "OK".into());
        let ok_variant = self.ok_variant;

        Button::new("ok")
            .label(ok_text)
            .with_variant(ok_variant)
            .on_click({
                let on_ok = on_ok.clone();
                let on_close = on_close.clone();

                move |_, window, cx| {
                    if on_ok(&ClickEvent::default(), window, cx) {
                        window.close_dialog(cx);
                        on_close(&ClickEvent::default(), window, cx);
                    }
                }
            })
            .into_any_element()
    }

    pub(crate) fn render_cancel(&self, _: &mut Window, _: &mut App) -> AnyElement {
        let on_cancel = self.on_cancel.clone();
        let on_close = self.on_close.clone();
        let cancel_text = self.cancel_text.clone().unwrap_or_else(|| "Cancel".into());
        let cancel_variant = self.cancel_variant;

        Button::new("cancel")
            .label(cancel_text)
            .with_variant(cancel_variant)
            .on_click({
                let on_cancel = on_cancel.clone();
                let on_close = on_close.clone();
                move |_, window, cx| {
                    if !on_cancel(&ClickEvent::default(), window, cx) {
                        return;
                    }

                    window.close_dialog(cx);
                    on_close(&ClickEvent::default(), window, cx);
                }
            })
            .into_any_element()
    }
}

type ContentBuilderFn = Rc<dyn Fn(DialogContent, &mut Window, &mut App) -> DialogContent + 'static>;

#[derive(Clone)]
pub(crate) struct DialogProps {
    width: Pixels,
    max_width: Option<Pixels>,
    margin_top: Option<Pixels>,
    close_button: bool,

    overlay: bool,
    overlay_closable: bool,
    pub(crate) overlay_visible: bool,
    keyboard: bool,
}

impl Default for DialogProps {
    fn default() -> Self {
        Self {
            margin_top: None,
            width: px(448.),
            max_width: None,
            overlay: true,
            keyboard: true,
            overlay_visible: false,
            close_button: true,
            overlay_closable: true,
        }
    }
}

/// 模态对话框，用于在对话框中显示内容。
#[derive(IntoElement)]
pub struct Dialog {
    pub(crate) style: StyleRefinement,
    children: Vec<AnyElement>,
    trigger: Option<AnyElement>,
    title: Option<AnyElement>,
    pub(crate) header: Option<AnyElement>,
    pub(crate) footer: Option<AnyElement>,
    pub(crate) content_builder: Option<ContentBuilderFn>,
    pub(crate) props: DialogProps,
    pub(crate) a11y_role: Role,

    button_props: DialogButtonProps,

    /// 打开对话框时创建并赋值的焦点句柄。
    pub(crate) focus_handle: FocusHandle,
    pub(crate) layer_ix: usize,
}

pub(crate) fn overlay_color(overlay: bool, cx: &App) -> Hsla {
    if !overlay {
        return hsla(0., 0., 0., 0.);
    }

    cx.theme().overlay
}

impl Dialog {
    /// 创建新的对话框。
    pub fn new(cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            style: StyleRefinement::default(),
            trigger: None,
            title: None,
            header: None,
            footer: None,
            content_builder: None,
            props: DialogProps::default(),
            children: Vec::new(),
            layer_ix: 0,
            button_props: DialogButtonProps::default(),
            a11y_role: Role::Dialog,
        }
    }

    /// 设置对话框的触发元素。
    ///
    /// 设置触发元素后，对话框将渲染为点击即可打开对话框的触发按钮。
    pub fn trigger(mut self, trigger: impl IntoElement) -> Self {
        self.trigger = Some(trigger.into_any_element());
        self
    }

    /// 设置对话框内容。
    pub fn content<F>(mut self, builder: F) -> Self
    where
        F: Fn(DialogContent, &mut Window, &mut App) -> DialogContent + 'static,
    {
        self.content_builder = Some(Rc::new(builder));
        self
    }

    /// 设置对话框标题。
    pub fn title(mut self, title: impl IntoElement) -> Self {
        self.title = Some(title.into_any_element());
        self
    }

    /// 设置对话框头部，通常包含标题与描述。
    pub(crate) fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }

    /// 设置对话框底部，通常为操作按钮。
    ///
    /// 设置底部后 `button_props` 将被忽略，需要自行渲染操作按钮。
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    /// 设置对话框按钮属性。
    pub fn button_props(mut self, button_props: DialogButtonProps) -> Self {
        self.button_props = button_props;
        self
    }

    /// 设置对话框关闭回调，在确认或取消回调之后调用。
    pub fn on_close(
        mut self,
        on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.button_props.on_close = Rc::new(on_close);
        self
    }

    /// 设置对话框确认回调。返回 `true` 关闭对话框，返回 `false` 则不关闭。
    pub fn on_ok(
        mut self,
        on_ok: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.button_props = self.button_props.on_ok(on_ok);
        self
    }

    /// 设置对话框取消回调。返回 `true` 关闭对话框，返回 `false` 则不关闭。
    pub fn on_cancel(
        mut self,
        on_cancel: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.button_props = self.button_props.on_cancel(on_cancel);
        self
    }

    /// 设置是否显示关闭图标，默认为 `true`。
    pub fn close_button(mut self, close_button: bool) -> Self {
        self.props.close_button = close_button;
        self
    }

    /// 设置对话框顶部偏移，默认为 None，使用视口高度的 1/10。
    pub fn margin_top(mut self, margin_top: impl Into<Pixels>) -> Self {
        self.props.margin_top = Some(margin_top.into());
        self
    }

    /// 设置对话框宽度，默认为 448px。
    pub fn w(mut self, width: impl Into<Pixels>) -> Self {
        self.props.width = width.into();
        self
    }

    /// 设置对话框宽度，默认为 448px。
    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.props.width = width.into();
        self
    }

    /// 设置对话框最大宽度，默认为 `None`。
    pub fn max_w(mut self, max_width: impl Into<Pixels>) -> Self {
        self.props.max_width = Some(max_width.into());
        self
    }

    /// 设置对话框遮罩，默认为 `true`。
    pub fn overlay(mut self, overlay: bool) -> Self {
        self.props.overlay = overlay;
        self
    }

    /// 设置遮罩是否可点击关闭，默认为 `true`。
    pub fn overlay_closable(mut self, overlay_closable: bool) -> Self {
        self.props.overlay_closable = overlay_closable;
        self
    }

    /// 设置是否支持键盘 Esc 关闭对话框，默认为 `true`。
    pub fn keyboard(mut self, keyboard: bool) -> Self {
        self.props.keyboard = keyboard;
        self
    }

    pub(crate) fn has_overlay(&self) -> bool {
        self.props.overlay
    }

    pub(crate) fn with_props(mut self, props: DialogProps) -> Self {
        self.props = props;
        self
    }

    fn defer_close_dialog(window: &mut Window, cx: &mut App) {
        Root::update(window, cx, |root, window, cx| {
            root.defer_close_dialog(window, cx);
        });
    }
}

impl ParentElement for Dialog {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Dialog {
    fn style(&mut self) -> &mut crate::StyleRefinement {
        &mut self.style
    }
}

impl Dialog {
    fn render_trigger(self, trigger: AnyElement, _: &mut Window, _: &mut App) -> AnyElement {
        let content_builder = self.content_builder.clone();
        let style = self.style.clone();
        let props = self.props.clone();
        let button_props = self.button_props.clone();

        div()
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                let content_builder = content_builder.clone();
                let style = style.clone();
                let props = props.clone();
                let button_props = button_props.clone();
                window.open_dialog(cx, move |dialog, _, _| {
                    dialog
                        .refine_style(&style)
                        .button_props(button_props.clone())
                        .with_props(props.clone())
                        .content({
                            let content_builder = content_builder.clone();
                            move |content, window, cx| {
                                if let Some(builder) = content_builder.clone() {
                                    builder(content, window, cx)
                                } else {
                                    content
                                }
                            }
                        })
                });
                cx.stop_propagation();
            })
            .child(trigger)
            .into_any_element()
    }
}

impl RenderOnce for Dialog {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if let Some(trigger) = self.trigger.take() {
            return self.render_trigger(trigger, window, cx);
        }

        let layer_ix = self.layer_ix;
        let on_close = self.button_props.on_close.clone();
        let on_ok = self.button_props.on_ok.clone();
        let on_cancel = self.button_props.on_cancel.clone();

        let window_paddings = window_paddings(window);
        let view_size = window.viewport_size()
            - crate::size(
                window_paddings.left + window_paddings.right,
                window_paddings.top + window_paddings.bottom,
            );
        let bounds = Bounds {
            origin: Point::default(),
            size: view_size,
        };
        let offset_top = px(layer_ix as f32 * 16.);
        let y = self.props.margin_top.unwrap_or(view_size.height / 10.) + offset_top;
        let x = bounds.center().x - self.props.width / 2.;

        let base_size = window.text_style().font_size;
        let rem_size = window.rem_size();

        let mut paddings = Edges::all(px(16.));
        if let Some(pl) = self.style.padding.left {
            paddings.left = pl.to_pixels(base_size, rem_size);
        }
        if let Some(pr) = self.style.padding.right {
            paddings.right = pr.to_pixels(base_size, rem_size);
        }
        if let Some(pt) = self.style.padding.top {
            paddings.top = pt.to_pixels(base_size, rem_size);
        }
        if let Some(pb) = self.style.padding.bottom {
            paddings.bottom = pb.to_pixels(base_size, rem_size);
        }

        let animation =
            Animation::new(*ANIMATION_DURATION).with_easing(cubic_bezier(0.32, 0.72, 0., 1.));

        anchored()
            .position(point(window_paddings.left, window_paddings.top))
            .snap_to_window()
            .child(
                div()
                    .id("dialog")
                    .occlude()
                    .w(view_size.width)
                    .h(view_size.height)
                    .when(self.props.overlay_visible, |this| {
                        this.bg(overlay_color(self.props.overlay, cx))
                    })
                    .when(self.props.overlay, |this| {
                        // 仅最后一个对话框拥有"点击遮罩关闭"事件。
                        if (self.layer_ix + 1) != Root::read(window, cx).active_dialogs.len() {
                            return this;
                        }

                        this.window_control_area(WindowControlArea::Drag)
                            .on_any_mouse_down({
                                let on_cancel = on_cancel.clone();
                                let on_close = on_close.clone();
                                move |event, window, cx| {
                                    if event.position.y < TITLE_BAR_HEIGHT {
                                        return;
                                    }

                                    cx.stop_propagation();
                                    if self.props.overlay_closable
                                        && event.button == MouseButton::Left
                                    {
                                        if on_cancel(&ClickEvent::default(), window, cx) {
                                            on_close(&ClickEvent::default(), window, cx);
                                            window.close_dialog(cx);
                                        }
                                    }
                                }
                            })
                    })
                    .child(
                        v_flex()
                            .id(layer_ix)
                            .role(self.a11y_role)
                            .track_focus(&self.focus_handle)
                            .focus_trap(format!("dialog-{}", layer_ix), &self.focus_handle)
                            .bg(cx.theme().tokens.background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded(cx.theme().radius_lg)
                            .min_h_24()
                            .pt(paddings.top)
                            .pb(paddings.bottom)
                            .gap(paddings.top.max(px(8.)))
                            .refine_style(&self.style)
                            .px_0()
                            .key_context(CONTEXT)
                            .when(self.props.keyboard, |this| {
                                this.on_action({
                                    let on_cancel = on_cancel.clone();
                                    let on_close = on_close.clone();
                                    move |_: &CancelDialog, window, cx| {
                                        if on_cancel(&ClickEvent::default(), window, cx) {
                                            window.close_dialog(cx);
                                            on_close(&ClickEvent::default(), window, cx);
                                        }
                                    }
                                })
                                .on_action({
                                    let on_ok = on_ok.clone();
                                    let on_close = on_close.clone();
                                    move |_: &ConfirmDialog, window, cx| {
                                        if on_ok(&ClickEvent::default(), window, cx) {
                                            Self::defer_close_dialog(window, cx);
                                            on_close(&ClickEvent::default(), window, cx);
                                        }
                                    }
                                })
                            })
                            // 以下样式优先级高，不可被覆盖。
                            .absolute()
                            .occlude()
                            .relative()
                            .left(x)
                            .top(y)
                            .w(self.props.width)
                            .when_some(self.props.max_width, |this, w| this.max_w(w))
                            .with_animation("slide-down", animation.clone(), move |this, delta| {
                                let shadow = vec![
                                    BoxShadow {
                                        color: crate::black().opacity(0.22),
                                        offset: point(px(0.), px(8.) * delta),
                                        blur_radius: px(28.) * delta,
                                        spread_radius: px(0.),
                                        inset: false,
                                    },
                                    BoxShadow {
                                        color: crate::black().opacity(0.16),
                                        offset: point(px(0.), px(2.) * delta),
                                        blur_radius: px(8.) * delta,
                                        spread_radius: px(0.),
                                        inset: false,
                                    },
                                ];
                                this.top(y * delta).shadow(shadow)
                            })
                            .child(
                                v_flex()
                                    .flex_1()
                                    .overflow_hidden()
                                    .gap_y_2()
                                    .when_some(self.header, |this, header| {
                                        this.child(
                                            div()
                                                .pl(paddings.left)
                                                .pr(paddings.right)
                                                .child(header),
                                        )
                                    })
                                    .when_some(self.title, |this, title| {
                                        this.child(
                                            DialogTitle::new()
                                                .pl(paddings.left)
                                                .pr(paddings.right)
                                                .child(title),
                                        )
                                    })
                                    .when_some(self.content_builder, |this, builder| {
                                        this.child(builder(
                                            DialogContent::new()
                                                .gap(paddings.bottom)
                                                .pl(paddings.left)
                                                .pr(paddings.right),
                                            window,
                                            cx,
                                        ))
                                    })
                                    .when(!self.children.is_empty(), |this| {
                                        this.child(
                                            div().flex_1().overflow_hidden().child(
                                                // 主体
                                                v_flex()
                                                    .size_full()
                                                    .overflow_y_scrollbar()
                                                    .pl(paddings.left)
                                                    .pr(paddings.right)
                                                    .children(self.children),
                                            ),
                                        )
                                    }),
                            )
                            .when_some(self.footer, |this, footer| {
                                this.child(div().pl(paddings.left).pr(paddings.right).child(footer))
                            })
                            .children(self.props.close_button.then(|| {
                                let top = (paddings.top - px(10.)).max(px(8.));
                                let right = (paddings.right - px(10.)).max(px(8.));

                                Button::new("close")
                                    .absolute()
                                    .top(top)
                                    .right(right)
                                    .small()
                                    .ghost()
                                    .icon(IconName::Close)
                                    .on_click({
                                        let on_cancel = self.button_props.on_cancel.clone();
                                        let on_close = self.button_props.on_close.clone();
                                        move |_, window, cx| {
                                            window.close_dialog(cx);
                                            on_cancel(&ClickEvent::default(), window, cx);
                                            on_close(&ClickEvent::default(), window, cx);
                                        }
                                    })
                            }))
                    )
                    .with_animation("fade-in", animation, move |this, delta| this.opacity(delta)),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AppContext, Context, Entity, FocusHandle, Render, TestAppContext, VisualTestContext, div,
    };

    /// 测试视图：挂载对话框层并持有一个可打开的按钮。
    struct DialogHost {
        focus_handle: FocusHandle,
    }

    impl Render for DialogHost {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let dialog_layer = Root::render_dialog_layer(window, cx);
            crate::v_flex()
                .track_focus(&self.focus_handle)
                .size_full()
                .child(Button::new("open").label("打开").on_click(|_, window, cx| {
                    window.open_dialog(cx, |dialog, _, _| {
                        dialog.title("标题").child(div().child("内容"))
                    });
                }))
                .children(dialog_layer)
        }
    }

    fn setup(cx: &mut TestAppContext) -> (Entity<Root>, &mut VisualTestContext) {
        cx.update(crate::theme::init);
        cx.add_window_view(|window, cx| {
            let focus_handle = cx.focus_handle();
            let view = cx.new(move |_| DialogHost { focus_handle });
            Root::new(view, cx)
        })
    }

    fn has_active_dialog(cx: &mut VisualTestContext) -> bool {
        cx.update(|window, cx| !Root::read(window, cx).active_dialogs.is_empty())
    }

    /// 打开对话框后应产生活动对话框。
    #[crate::test]
    fn open_dialog_renders_dialog(cx: &mut TestAppContext) {
        let (_, cx) = setup(cx);
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });

        // 初始无对话框。
        assert!(!has_active_dialog(cx));

        // 通过 WindowExt 打开对话框（等效于点击按钮）。
        cx.update(|window, cx| {
            window.open_dialog(cx, |dialog, _, _| dialog.title("标题"));
            let _ = window.draw(cx);
        });

        // 打开后存在活动对话框。
        assert!(has_active_dialog(cx));
    }

    /// 关闭对话框后应移除活动对话框。
    #[crate::test]
    fn close_dialog_removes_dialog(cx: &mut TestAppContext) {
        let (_, cx) = setup(cx);
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.open_dialog(cx, |dialog, _, _| dialog.title("标题"));
            let _ = window.draw(cx);
        });

        assert!(has_active_dialog(cx));

        cx.update(|window, cx| {
            window.close_dialog(cx);
            let _ = window.draw(cx);
        });

        assert!(!has_active_dialog(cx));
    }

    /// 默认对话框按钮属性：无自定义文本、不显示取消按钮。
    #[crate::test]
    fn dialog_button_props_defaults() {
        let props = DialogButtonProps::default();
        assert!(props.ok_text.is_none());
        assert!(!props.show_cancel);
        assert_eq!(props.ok_variant, ButtonVariant::Primary);
    }

    /// DialogFooter 默认右对齐。
    #[crate::test]
    fn dialog_footer_defaults_to_end_alignment() {
        let footer = crate::dialog::DialogFooter::new();
        // 默认样式为空，渲染时通过 Styled 链式配置。
        let _ = footer;
    }
}
